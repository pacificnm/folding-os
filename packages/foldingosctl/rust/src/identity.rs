use std::fs;
use std::net::IpAddr;
use std::process::Command;
use std::str::FromStr;

use regex::Regex;
use std::sync::LazyLock;

use crate::config_host::read_hostname;
use crate::paths::AppliancePaths;

static UUID_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
        .expect("uuid pattern compiles")
});

pub fn read_node_id(paths: &AppliancePaths) -> Result<String, String> {
    let content = fs::read(paths.node_id_path())
        .map_err(|error| format!("read node id: {error}"))?;
    parse_node_id_file(&content)
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
    let output = Command::new("ip")
        .args(["-o", "link", "show", "up"])
        .output()
        .map_err(|error| format!("list network interfaces: {error}"))?;
    if !output.status.success() {
        return Err("list network interfaces failed".into());
    }
    let mut addresses = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.contains(" lo:") || line.contains(": lo:") {
            continue;
        }
        let Some(index) = line.find("link/ether ") else {
            continue;
        };
        let rest = &line[index + "link/ether ".len()..];
        let mac = rest.split_whitespace().next().unwrap_or_default();
        if !mac.is_empty() {
            addresses.push(mac.to_string());
        }
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
