use std::collections::BTreeMap;
use std::fs;

use regex::Regex;
use std::sync::LazyLock;

use crate::automation_policy::require_supervisor_automation_mutation;
use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;
use crate::role::require_supervisor_role;

static MAC_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9a-f]{2}:){5}[0-9a-f]{2}$").expect("mac pattern compiles"));
static DISK_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^/dev/(sd[a-z]+|vd[a-z]+|nvme[0-9]+n[0-9]+)$").expect("disk pattern compiles")
});

pub fn list_allow_boot(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    require_supervisor_role(paths)?;
    let devices = collect_boot_allow_devices(paths)?;
    Ok(serde_json::json!({ "devices": devices }))
}

pub fn allow_boot(paths: &AppliancePaths, args: &[String]) -> Result<serde_json::Value, String> {
    let (mac, install_disk) = parse_allow_boot_args(args)?;
    allow_boot_mac(paths, &mac, install_disk.as_deref())
}

pub fn deny_boot(paths: &AppliancePaths, args: &[String]) -> Result<serde_json::Value, String> {
    if args.len() != 1 {
        return Err(format!(
            "deny-boot requires exactly one MAC address (got {})",
            args.len()
        ));
    }
    deny_boot_mac(paths, &args[0])
}

fn parse_allow_boot_args(args: &[String]) -> Result<(String, Option<String>), String> {
    let mut install_disk = None;
    let mut remaining = args;
    while !remaining.is_empty() {
        if remaining[0] == "--disk" {
            let disk = remaining
                .get(1)
                .ok_or_else(|| "--disk requires a whole-disk device path".to_string())?;
            install_disk = Some(disk.clone());
            remaining = &remaining[2..];
            continue;
        }
        if remaining.len() != 1 {
            return Err(format!("unexpected allow-boot argument {:?}", remaining[0]));
        }
        return Ok((remaining[0].clone(), install_disk));
    }
    Err("allow-boot requires a MAC address".into())
}

fn allow_boot_mac(
    paths: &AppliancePaths,
    mac: &str,
    install_disk: Option<&str>,
) -> Result<serde_json::Value, String> {
    require_supervisor_role(paths)?;
    require_supervisor_automation_mutation(paths, "provision", "allow-boot")?;
    let mac = parse_mac_address(mac)?;
    let install_disk = match install_disk {
        Some(disk) => Some(parse_install_disk_path(disk)?),
        None => None,
    };
    let mut allowed = read_boot_allowlist_entries(paths)?;
    let already_allowed = allowed.iter().any(|value| value == &mac);
    if already_allowed {
        if let Some(disk) = install_disk.as_deref() {
            save_boot_install_disk_mapping(paths, &mac, disk)?;
            return Ok(serde_json::json!({
                "mac_address": mac,
                "install_disk": disk,
                "already_allowed": true,
            }));
        }
        return Ok(serde_json::json!({
            "mac_address": mac,
            "already_allowed": true,
        }));
    }
    allowed.push(mac.clone());
    save_boot_allowlist(paths, &allowed)?;
    if let Some(disk) = install_disk.as_deref() {
        save_boot_install_disk_mapping(paths, &mac, disk)?;
        return Ok(serde_json::json!({
            "mac_address": mac,
            "install_disk": disk,
            "already_allowed": false,
        }));
    }
    Ok(serde_json::json!({
        "mac_address": mac,
        "already_allowed": false,
    }))
}

fn deny_boot_mac(paths: &AppliancePaths, mac: &str) -> Result<serde_json::Value, String> {
    require_supervisor_role(paths)?;
    require_supervisor_automation_mutation(paths, "provision", "deny-boot")?;
    let mac = parse_mac_address(mac)?;
    let mut allowed = read_boot_allowlist_entries(paths)?;
    let was_allowed = allowed.iter().any(|value| value == &mac);
    if !was_allowed {
        return Ok(serde_json::json!({
            "mac_address": mac,
            "already_removed": true,
        }));
    }
    allowed.retain(|value| value != &mac);
    save_boot_allowlist(paths, &allowed)?;
    remove_boot_install_disk_mapping(paths, &mac)?;
    Ok(serde_json::json!({
        "mac_address": mac,
        "already_removed": false,
    }))
}

fn collect_boot_allow_devices(paths: &AppliancePaths) -> Result<Vec<serde_json::Value>, String> {
    let macs = read_boot_allowlist_entries(paths)?;
    let mut sorted = macs;
    sorted.sort();
    let disk_mappings = read_boot_install_disk_allowlist_entries(paths)?;
    Ok(sorted
        .into_iter()
        .map(|mac| {
            let mut device = serde_json::json!({ "mac_address": mac });
            if let Some(disk) =
                disk_mappings.get(device["mac_address"].as_str().unwrap_or_default())
            {
                device["install_disk"] = serde_json::Value::String(disk.clone());
            }
            device
        })
        .collect())
}

pub(crate) fn read_boot_allowlist_entries(paths: &AppliancePaths) -> Result<Vec<String>, String> {
    let content = match fs::read_to_string(&paths.boot_allowlist) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut allowed = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Ok(mac) = parse_mac_address(line) {
            allowed.push(mac);
        }
    }
    Ok(allowed)
}

fn save_boot_allowlist(paths: &AppliancePaths, allowed: &[String]) -> Result<(), String> {
    let mut content = allowed.join("\n");
    if !allowed.is_empty() {
        content.push('\n');
    }
    atomic_write(&paths.boot_allowlist, content.as_bytes(), 0o664)?;
    finalize_boot_allowlist_metadata(&paths.boot_allowlist)
}

pub(crate) fn boot_install_disk_for_mac(paths: &AppliancePaths, mac: &str) -> String {
    let mac = normalize_mac(mac);
    if mac.is_empty() {
        return String::new();
    }
    read_boot_install_disk_allowlist_entries(paths)
        .ok()
        .and_then(|mappings| mappings.get(&mac).cloned())
        .unwrap_or_default()
}

fn read_boot_install_disk_allowlist_entries(
    paths: &AppliancePaths,
) -> Result<BTreeMap<String, String>, String> {
    let content = match fs::read_to_string(&paths.boot_install_disk_allowlist) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut mappings = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(mac_raw) = fields.next() else {
            continue;
        };
        let Some(disk_raw) = fields.next() else {
            continue;
        };
        if let (Ok(mac), Ok(disk)) = (
            parse_mac_address(mac_raw),
            parse_install_disk_path(disk_raw),
        ) {
            mappings.insert(mac, disk);
        }
    }
    Ok(mappings)
}

fn save_boot_install_disk_mapping(
    paths: &AppliancePaths,
    mac: &str,
    install_disk: &str,
) -> Result<(), String> {
    let mut mappings = read_boot_install_disk_allowlist_entries(paths)?;
    mappings.insert(mac.to_string(), install_disk.to_string());
    write_boot_install_disk_mappings(paths, &mappings)
}

fn remove_boot_install_disk_mapping(paths: &AppliancePaths, mac: &str) -> Result<(), String> {
    let mut mappings = read_boot_install_disk_allowlist_entries(paths)?;
    mappings.remove(mac);
    write_boot_install_disk_mappings(paths, &mappings)
}

fn write_boot_install_disk_mappings(
    paths: &AppliancePaths,
    mappings: &BTreeMap<String, String>,
) -> Result<(), String> {
    let lines: Vec<String> = mappings
        .iter()
        .map(|(mac, disk)| format!("{mac} {disk}"))
        .collect();
    let mut content = lines.join("\n");
    if !lines.is_empty() {
        content.push('\n');
    }
    atomic_write(
        &paths.boot_install_disk_allowlist,
        content.as_bytes(),
        0o664,
    )?;
    finalize_boot_allowlist_metadata(&paths.boot_install_disk_allowlist)
}

fn finalize_boot_allowlist_metadata(path: &std::path::Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use nix::unistd::{chown, geteuid, Group, Uid};
        use std::os::unix::fs::PermissionsExt;

        if geteuid() != Uid::from_raw(0) {
            return Ok(());
        }
        let gid = Group::from_name("foldops")
            .map_err(|error| format!("resolve foldops group: {error}"))?
            .map(|group| group.gid)
            .ok_or_else(|| "foldops group is not configured".to_string())?;
        chown(path, Some(Uid::from_raw(0)), Some(gid))
            .map_err(|error| format!("restore boot allowlist ownership: {error}"))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o664))
            .map_err(|error| format!("restore boot allowlist mode: {error}"))?;
    }
    Ok(())
}

fn parse_mac_address(value: &str) -> Result<String, String> {
    let mac = normalize_mac(value);
    if !MAC_PATTERN.is_match(&mac) {
        return Err(format!("invalid MAC address \"{value}\""));
    }
    Ok(mac)
}

pub(crate) fn normalize_mac(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', ":")
}

fn parse_install_disk_path(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("install disk path is empty".into());
    }
    if !DISK_PATTERN.is_match(path) {
        return Err(format!(
            "install disk must be a whole-disk device path such as /dev/sda or /dev/nvme0n1: \"{path}\""
        ));
    }
    Ok(path.to_string())
}
