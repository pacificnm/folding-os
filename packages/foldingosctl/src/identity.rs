use std::fs;
use std::net::IpAddr;
use std::process::Command;
use std::str::FromStr;

use getrandom::getrandom;
use regex::Regex;
use std::sync::LazyLock;

use crate::config::{effective_config, parse_domain, HOSTNAME_PATTERN};
use crate::config_host::read_hostname;
use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;
use crate::process::run_command;

static UUID_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
        .expect("uuid pattern compiles")
});

pub fn read_node_id(paths: &AppliancePaths) -> Result<String, String> {
    let content = fs::read(paths.node_id_path())
        .map_err(|error| format!("read node id: {error}"))?;
    parse_node_id_file(&content)
}

pub fn ensure_identity(paths: &AppliancePaths) -> Result<(), String> {
    let node_id = ensure_node_id_file(paths)?;
    let system = effective_config(paths, "system", false)?;
    let values = parse_domain("system", &system, true)?;
    let mut hostname = values
        .get("identity.hostname")
        .map(|value| value.text.clone())
        .unwrap_or_default();
    if hostname.is_empty() {
        let compact: String = node_id.replace('-', "");
        hostname = format!("folding-{}", &compact[..compact.len().min(6)]);
        let generated = format!(
            "schema_version = 1\n\n[identity]\nhostname = {hostname:?}\n"
        );
        atomic_write(&paths.system_config_path(), generated.as_bytes(), 0o644)?;
        effective_config(paths, "system", true)?;
    }
    if !HOSTNAME_PATTERN.is_match(&hostname) {
        return Err("effective hostname is invalid".into());
    }
    run_command("hostnamectl", &["set-hostname", "--static", &hostname])
}

fn ensure_node_id_file(paths: &AppliancePaths) -> Result<String, String> {
    let node_id_path = paths.node_id_path();
    match fs::read(&node_id_path) {
        Ok(content) => match parse_node_id_file(&content) {
            Ok(node_id) => {
                if String::from_utf8_lossy(&content).trim() != node_id {
                    atomic_write(
                        &node_id_path,
                        format!("{node_id}\n").as_bytes(),
                        0o644,
                    )?;
                }
                Ok(node_id)
            }
            Err(_) => {
                let node_id = new_uuid()?;
                atomic_write(
                    &node_id_path,
                    format!("{node_id}\n").as_bytes(),
                    0o644,
                )?;
                Ok(node_id)
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let node_id = new_uuid()?;
            atomic_write(
                &node_id_path,
                format!("{node_id}\n").as_bytes(),
                0o644,
            )?;
            Ok(node_id)
        }
        Err(error) => Err(error.to_string()),
    }
}

fn new_uuid() -> Result<String, String> {
    let mut value = [0u8; 16];
    getrandom(&mut value).map_err(|error| error.to_string())?;
    value[6] = (value[6] & 0x0f) | 0x40;
    value[8] = (value[8] & 0x3f) | 0x80;
    Ok(format_uuid_bytes(&value))
}

fn parse_node_id_file(content: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(content).trim().to_string();
    if UUID_PATTERN.is_match(&text) {
        return Ok(text);
    }
    let raw = content;
    if raw.len() == 16 {
        return Ok(normalize_uuid_bytes(raw));
    }
    Err("existing node identity is invalid".into())
}

fn normalize_uuid_bytes(value: &[u8]) -> String {
    let mut fixed = value.to_vec();
    fixed[6] = (fixed[6] & 0x0f) | 0x40;
    fixed[8] = (fixed[8] & 0x3f) | 0x80;
    format_uuid_bytes(&fixed)
}

fn format_uuid_bytes(value: &[u8]) -> String {
    format!(
        "{}-{}-{}-{}-{}",
        hex::encode(&value[0..4]),
        hex::encode(&value[4..6]),
        hex::encode(&value[6..8]),
        hex::encode(&value[8..10]),
        hex::encode(&value[10..16]),
    )
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

pub fn read_installed_foldingos_version() -> Result<String, String> {
    let content = fs::read_to_string("/usr/lib/os-release")
        .map_err(|error| format!("read os-release: {error}"))?;
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("VERSION_ID=") {
            return Ok(value.trim_matches('"').to_string());
        }
    }
    Err("installed FoldingOS version is unavailable".into())
}

pub fn collect_mac_addresses() -> Result<Vec<String>, String> {
    collect_mac_addresses_from_sys_class_net(std::path::Path::new("/sys/class/net"))
}

fn collect_mac_addresses_from_sys_class_net(net_dir: &std::path::Path) -> Result<Vec<String>, String> {
    let mut addresses = Vec::new();
    let entries = fs::read_dir(net_dir)
        .map_err(|error| format!("list network interfaces: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("list network interfaces: {error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "lo" {
            continue;
        }
        let interface_dir = entry.path();
        let operstate = fs::read_to_string(interface_dir.join("operstate"))
            .unwrap_or_default();
        if operstate.trim() != "up" {
            continue;
        }
        let mac = fs::read_to_string(interface_dir.join("address"))
            .map_err(|error| format!("read interface {name} address: {error}"))?
            .trim()
            .to_string();
        if mac.is_empty() || mac == "00:00:00:00:00:00" {
            continue;
        }
        addresses.push(mac);
    }
    addresses.sort();
    if addresses.is_empty() {
        return Err("no active network interface MAC addresses found".into());
    }
    Ok(addresses)
}

pub fn routable_ipv4_address() -> Option<String> {
    let output = Command::new("networkctl")
        .args(["--no-legend", "--no-pager", "list"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    let interfaces = candidate_network_interfaces(&listing).ok()?;
    for interface in interfaces {
        let status = Command::new("networkctl")
            .args(["--no-legend", "--no-pager", "status", &interface])
            .output()
            .ok()?;
        if !status.status.success() {
            continue;
        }
        if let Some(address) = parse_ipv4_address(&String::from_utf8_lossy(&status.stdout)) {
            return Some(address);
        }
    }
    None
}

pub(crate) fn candidate_network_interfaces(listing: &str) -> Result<Vec<String>, String> {
    let mut routable = Vec::new();
    let mut fallback = Vec::new();
    for line in listing.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2 {
            continue;
        }
        let (name, is_routable) = if fields[0].parse::<u32>().is_ok() && fields.len() >= 4 {
            let name = fields[1];
            if name == "lo" {
                continue;
            }
            (name, fields[2..].contains(&"routable"))
        } else {
            let name = fields[0];
            if name == "lo" {
                continue;
            }
            (name, fields[1..].contains(&"routable"))
        };
        if is_routable {
            routable.push(name.to_string());
        } else {
            fallback.push(name.to_string());
        }
    }
    if !routable.is_empty() {
        return Ok(routable);
    }
    if !fallback.is_empty() {
        return Ok(fallback);
    }
    Err("no wired network interface found".into())
}

fn parse_ipv4_address(status: &str) -> Option<String> {
    for line in status.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Address:") {
            continue;
        }
        for token in trimmed.split_whitespace().skip(1) {
            if let Ok(ip) = IpAddr::from_str(token.split('/').next()?) {
                if matches!(ip, IpAddr::V4(ipv4) if !ipv4.is_loopback() && !ipv4.is_unspecified()) {
                    return Some(ip.to_string());
                }
            }
        }
    }
    None
}

fn read_kernel_version() -> String {
    Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

pub fn read_node_identity(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let node_id = read_node_id(paths)?;
    let hostname = read_hostname(paths)?;
    let role = crate::role::read_active_installation_role(paths)?;
    let foldingos_version = read_installed_foldingos_version()?;
    let kernel_version = read_kernel_version();
    let mac_addresses = collect_mac_addresses()?;
    let primary_ipv4 = routable_ipv4_address();
    let mut data = serde_json::json!({
        "node_id": node_id,
        "hostname": hostname,
        "installation_role": role,
        "foldingos_version": foldingos_version,
        "kernel_version": kernel_version,
        "mac_addresses": mac_addresses,
    });
    if let Some(address) = primary_ipv4 {
        data["primary_ipv4"] = serde_json::Value::String(address);
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn write_interface(root: &Path, name: &str, operstate: &str, address: &str) {
        let interface_dir = root.join(name);
        fs::create_dir_all(&interface_dir).unwrap();
        fs::write(interface_dir.join("operstate"), operstate).unwrap();
        fs::write(interface_dir.join("address"), address).unwrap();
    }

    #[test]
    fn collect_mac_addresses_reads_up_interfaces_from_sysfs() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-mac-collect-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        write_interface(&root, "lo", "unknown", "00:00:00:00:00:00");
        write_interface(&root, "eth0", "up", "52:54:00:12:34:56");
        write_interface(&root, "eth1", "down", "00:be:43:e7:59:5e");

        let macs = collect_mac_addresses_from_sys_class_net(&root).unwrap();
        assert_eq!(macs, vec!["52:54:00:12:34:56".to_string()]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn collect_mac_addresses_rejects_empty_allowlist() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-mac-empty-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        write_interface(&root, "eth0", "down", "52:54:00:12:34:56");

        assert!(collect_mac_addresses_from_sys_class_net(&root).is_err());

        let _ = fs::remove_dir_all(root);
    }
}
