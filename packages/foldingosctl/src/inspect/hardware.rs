use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::paths::AppliancePaths;

const PLACEHOLDER_VALUES: &[&str] = &[
    "",
    "Not Specified",
    "Not Available",
    "To Be Filled By O.E.M.",
    "To be filled by O.E.M.",
    "Default string",
    "System Product Name",
    "System Version",
    "System Serial Number",
];

pub fn inspect_hardware(_paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let cpu = read_cpu_profile()?;
    let mut data = serde_json::json!({
        "cpu": cpu,
        "memory": read_memory_profile()?,
    });

    if let Some(board) = read_dmi_group(&[
        ("vendor", "board_vendor"),
        ("product", "board_name"),
        ("version", "board_version"),
    ]) {
        data["board"] = board;
    }
    if let Some(system) = read_dmi_group(&[
        ("vendor", "sys_vendor"),
        ("product", "product_name"),
        ("family", "product_family"),
        ("version", "product_version"),
        ("sku", "product_sku"),
    ]) {
        data["system"] = system;
    }
    if let Some(chassis) = read_dmi_group(&[
        ("vendor", "chassis_vendor"),
        ("type_code", "chassis_type"),
        ("version", "chassis_version"),
    ]) {
        data["chassis"] = chassis;
    }
    if let Some(bios) = read_dmi_group(&[
        ("vendor", "bios_vendor"),
        ("version", "bios_version"),
        ("date", "bios_date"),
    ]) {
        data["bios"] = bios;
    }

    let storage = read_storage_devices()?;
    if !storage.is_empty() {
        data["storage"] = serde_json::Value::Array(storage);
    }

    let network = read_network_adapters()?;
    if !network.is_empty() {
        data["network"] = serde_json::Value::Array(network);
    }

    let pci_devices = read_pci_devices()?;
    if !pci_devices.is_empty() {
        data["pci_devices"] = serde_json::Value::Array(pci_devices);
    }

    Ok(data)
}

fn read_cpu_profile() -> Result<serde_json::Value, String> {
    let file =
        fs::File::open("/proc/cpuinfo").map_err(|error| format!("open /proc/cpuinfo: {error}"))?;
    let reader = BufReader::new(file);

    let mut model_name: Option<String> = None;
    let mut vendor_id: Option<String> = None;
    let mut physical_cores: Option<u32> = None;
    let mut siblings: Option<u32> = None;
    let mut logical_threads = 0_u32;

    for line in reader.lines() {
        let line = line.map_err(|error| format!("read /proc/cpuinfo: {error}"))?;
        if let Some(value) = line.strip_prefix("model name") {
            if model_name.is_none() {
                model_name = parse_cpuinfo_value(value);
            }
        } else if let Some(value) = line.strip_prefix("vendor_id") {
            if vendor_id.is_none() {
                vendor_id = parse_cpuinfo_value(value);
            }
        } else if let Some(value) = line.strip_prefix("cpu cores") {
            if physical_cores.is_none() {
                physical_cores = parse_cpuinfo_value(value).and_then(|text| text.parse().ok());
            }
        } else if let Some(value) = line.strip_prefix("siblings") {
            if siblings.is_none() {
                siblings = parse_cpuinfo_value(value).and_then(|text| text.parse().ok());
            }
        } else if line.starts_with("processor") {
            logical_threads += 1;
        }
    }

    if logical_threads == 0 {
        return Err("cpu thread count unavailable".into());
    }

    let physical = physical_cores.or(siblings).unwrap_or(logical_threads);
    let mut cpu = serde_json::json!({
        "model": model_name.unwrap_or_else(|| "unknown".into()),
        "physical_cores": physical,
        "logical_threads": logical_threads,
        "architecture": std::env::consts::ARCH,
    });
    if let Some(vendor) = vendor_id {
        cpu["vendor"] = serde_json::Value::String(vendor);
    }
    Ok(cpu)
}

fn parse_cpuinfo_value(line: &str) -> Option<String> {
    let value = line.split(':').nth(1)?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn read_memory_profile() -> Result<serde_json::Value, String> {
    let file =
        fs::File::open("/proc/meminfo").map_err(|error| format!("open /proc/meminfo: {error}"))?;
    let reader = BufReader::new(file);
    let mut total_kb = 0_u64;
    for line in reader.lines() {
        let line = line.map_err(|error| format!("read /proc/meminfo: {error}"))?;
        if let Some(value) = line.strip_prefix("MemTotal:") {
            total_kb = parse_meminfo_kb(value)?;
            break;
        }
    }
    if total_kb == 0 {
        return Err("memory total is unavailable".into());
    }

    let mut profile = serde_json::json!({
        "total_bytes": total_kb * 1024,
        "module_slots_detected": count_dmi_memory_entries(),
    });
    if let Some(modules) = read_memory_modules_from_dmi_entries() {
        profile["modules"] = serde_json::Value::Array(modules);
    }
    Ok(profile)
}

fn parse_meminfo_kb(line: &str) -> Result<u64, String> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let value = fields
        .first()
        .ok_or_else(|| format!("invalid meminfo line {line:?}"))?;
    value
        .parse()
        .map_err(|error| format!("parse meminfo kb: {error}"))
}

fn count_dmi_memory_entries() -> u32 {
    let entries = match fs::read_dir("/sys/firmware/dmi/entries") {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("17-")
        })
        .count() as u32
}

fn read_memory_modules_from_dmi_entries() -> Option<Vec<serde_json::Value>> {
    let entries = fs::read_dir("/sys/firmware/dmi/entries").ok()?;
    let mut modules = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("17-") {
            continue;
        }
        let raw_path = entry.path().join("raw");
        let raw = fs::read(raw_path).ok()?;
        if let Some(module) = parse_smbios_memory_device(&raw) {
            modules.push(module);
        }
    }
    if modules.is_empty() {
        None
    } else {
        Some(modules)
    }
}

fn parse_smbios_memory_device(raw: &[u8]) -> Option<serde_json::Value> {
    if raw.len() < 0x15 {
        return None;
    }
    let size_bytes = memory_device_size_bytes(raw[0x0C], raw[0x0D])?;
    let speed_mts = u16::from_le_bytes([raw[0x15], raw[0x16]]);
    let mut module = serde_json::json!({
        "size_bytes": size_bytes,
    });
    if speed_mts != 0 {
        module["speed_mts"] = serde_json::json!(speed_mts);
    }
    if let Some(manufacturer) = smbios_string(raw, raw.get(0x17).copied().unwrap_or(0)) {
        module["manufacturer"] = serde_json::Value::String(manufacturer);
    }
    if let Some(locator) = smbios_string(raw, raw.get(0x10).copied().unwrap_or(0)) {
        module["locator"] = serde_json::Value::String(locator);
    }
    Some(module)
}

fn memory_device_size_bytes(lsb: u8, msb: u8) -> Option<u64> {
    let value = u16::from_le_bytes([lsb, msb]);
    match value {
        0x0000 => None,
        0xFFFF => None,
        0x7FFF => None,
        0x8000..=0xFFFE => Some((value as u64 - 0x8000) * 1024 * 1024),
        size_kb => Some(size_kb as u64 * 1024),
    }
}

fn smbios_string(raw: &[u8], index: u8) -> Option<String> {
    if index == 0 {
        return None;
    }
    let mut strings = raw.split(|byte| *byte == 0).skip(1);
    let selected = strings.nth((index - 1) as usize)?;
    let text = String::from_utf8_lossy(selected).trim().to_string();
    if is_placeholder_value(&text) {
        None
    } else {
        Some(text)
    }
}

fn read_dmi_group(fields: &[(&str, &str)]) -> Option<serde_json::Value> {
    let mut object = serde_json::Map::new();
    for (key, file_name) in fields {
        if let Some(value) = read_dmi_field(file_name) {
            object.insert((*key).to_string(), serde_json::Value::String(value));
        }
    }
    if object.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(object))
    }
}

fn read_dmi_field(file_name: &str) -> Option<String> {
    let path = format!("/sys/class/dmi/id/{file_name}");
    let value = fs::read_to_string(path).ok()?.trim().to_string();
    if is_placeholder_value(&value) {
        None
    } else {
        Some(value)
    }
}

fn is_placeholder_value(value: &str) -> bool {
    PLACEHOLDER_VALUES
        .iter()
        .any(|placeholder| placeholder.eq_ignore_ascii_case(value))
}

fn read_storage_devices() -> Result<Vec<serde_json::Value>, String> {
    let entries = fs::read_dir("/sys/block").map_err(|error| format!("read /sys/block: {error}"))?;
    let mut devices = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("read /sys/block entry: {error}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_block_device(&name) {
            continue;
        }
        let base = entry.path();
        let size_bytes = read_u64_file(base.join("size")).unwrap_or(0) * 512;
        let model = read_trimmed_file(base.join("device/model"));
        let rotational = read_bool_file(base.join("queue/rotational"));
        let mut device = serde_json::json!({
            "name": name,
            "size_bytes": size_bytes,
        });
        if let Some(model) = model.filter(|value| !is_placeholder_value(value)) {
            device["model"] = serde_json::Value::String(model);
        }
        if let Some(rotational) = rotational {
            device["rotational"] = serde_json::Value::Bool(rotational);
        }
        devices.push(device);
    }
    devices.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    Ok(devices)
}

fn should_skip_block_device(name: &str) -> bool {
    name.starts_with("loop")
        || name.starts_with("ram")
        || name.starts_with("dm-")
        || name.starts_with("sr")
        || name.starts_with("fd")
}

fn read_network_adapters() -> Result<Vec<serde_json::Value>, String> {
    let entries =
        fs::read_dir("/sys/class/net").map_err(|error| format!("read /sys/class/net: {error}"))?;
    let mut adapters = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("read /sys/class/net entry: {error}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" {
            continue;
        }
        let base = entry.path();
        let mac_address = read_trimmed_file(base.join("address"));
        let operstate = read_trimmed_file(base.join("operstate"));
        let speed_mbps = read_u64_file(base.join("speed")).and_then(|value| {
            if value == 0 || value == u64::MAX {
                None
            } else {
                Some(value)
            }
        });
        let pci_address = fs::read_link(base.join("device"))
            .ok()
            .and_then(|path| pci_address_from_device_path(&path));
        let mut adapter = serde_json::json!({ "name": name });
        if let Some(mac_address) = mac_address.filter(|value| !value.is_empty()) {
            adapter["mac_address"] = serde_json::Value::String(mac_address);
        }
        if let Some(operstate) = operstate {
            adapter["operstate"] = serde_json::Value::String(operstate);
        }
        if let Some(speed_mbps) = speed_mbps {
            adapter["speed_mbps"] = serde_json::json!(speed_mbps);
        }
        if let Some(pci_address) = pci_address {
            adapter["pci_address"] = serde_json::Value::String(pci_address);
        }
        adapters.push(adapter);
    }
    adapters.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    Ok(adapters)
}

fn pci_address_from_device_path(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| name.contains(':'))
}

fn read_pci_devices() -> Result<Vec<serde_json::Value>, String> {
    let entries = fs::read_dir("/sys/bus/pci/devices")
        .map_err(|error| format!("read /sys/bus/pci/devices: {error}"))?;
    let mut devices = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("read pci device entry: {error}"))?;
        let address = entry.file_name().to_string_lossy().to_string();
        let base = entry.path();
        let vendor_id = read_trimmed_file(base.join("vendor"));
        let device_id = read_trimmed_file(base.join("device"));
        let class_id = read_trimmed_file(base.join("class"));
        let mut device = serde_json::json!({ "address": address });
        if let Some(vendor_id) = vendor_id {
            device["vendor_id"] = serde_json::Value::String(vendor_id);
        }
        if let Some(device_id) = device_id {
            device["device_id"] = serde_json::Value::String(device_id);
        }
        if let Some(class_id) = class_id {
            device["class_id"] = serde_json::Value::String(class_id);
        }
        devices.push(device);
    }
    devices.sort_by(|left, right| {
        left["address"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["address"].as_str().unwrap_or_default())
    });
    Ok(devices)
}

fn read_trimmed_file(path: PathBuf) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_u64_file(path: PathBuf) -> Option<u64> {
    read_trimmed_file(path)?.parse().ok()
}

fn read_bool_file(path: PathBuf) -> Option<bool> {
    match read_trimmed_file(path)?.as_str() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_device_size_parses_kilobyte_and_megabyte_forms() {
        assert_eq!(memory_device_size_bytes(0x00, 0x04), Some(1024 * 1024));
        assert_eq!(memory_device_size_bytes(0x04, 0x80), Some(4 * 1024 * 1024));
        assert_eq!(memory_device_size_bytes(0x00, 0x00), None);
    }

    #[test]
    fn placeholder_values_are_filtered() {
        assert!(is_placeholder_value("Not Specified"));
        assert!(is_placeholder_value("To be filled by O.E.M."));
        assert!(!is_placeholder_value("Dell Inc."));
    }

    #[test]
    fn smbios_string_selects_indexed_string() {
        let raw = b"formatted-area\x00Vendor\x00DIMM A\x00";
        assert_eq!(smbios_string(raw, 1), Some("Vendor".to_string()));
        assert_eq!(smbios_string(raw, 2), Some("DIMM A".to_string()));
    }
}
