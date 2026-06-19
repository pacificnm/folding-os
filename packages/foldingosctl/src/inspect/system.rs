use std::fs;
use std::io::{BufRead, BufReader};
use std::process::Command;

use nix::sys::statvfs::statvfs;
use regex::Regex;
use std::sync::LazyLock;

use crate::identity::candidate_network_interfaces;
use crate::paths::AppliancePaths;

static TEMP_INPUT_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+$").expect("temperature pattern compiles"));

pub fn inspect_system(_paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let (uptime_seconds, load_average) = read_uptime_and_load()?;
    let memory = read_memory_usage()?;
    let (total, used, free, used_percent) = read_root_filesystem_usage()?;
    let mut data = serde_json::json!({
        "uptime_seconds": uptime_seconds,
        "load_average": load_average,
        "memory": memory,
        "root_filesystem": {
            "mountpoint": "/",
            "total_bytes": total,
            "used_bytes": used,
            "free_bytes": free,
            "used_percent": used_percent,
        },
    });
    if let Ok(network) = read_primary_network_counters() {
        data["primary_network"] = network;
    }
    let (cpu_temp, chassis_temp) = read_temperatures_from_sysfs();
    if let Some(value) = cpu_temp {
        data["cpu_temp_celsius"] = serde_json::json!(value);
    }
    if let Some(value) = chassis_temp {
        data["chassis_temp_celsius"] = serde_json::json!(value);
    }
    Ok(data)
}

fn read_uptime_and_load() -> Result<(f64, [f64; 3]), String> {
    let uptime_content = fs::read_to_string("/proc/uptime")
        .map_err(|error| format!("read /proc/uptime: {error}"))?;
    let uptime_field = uptime_content
        .split_whitespace()
        .next()
        .ok_or_else(|| "invalid /proc/uptime".to_string())?;
    let uptime_seconds: f64 = uptime_field
        .parse()
        .map_err(|error| format!("parse uptime: {error}"))?;

    let load_content = fs::read_to_string("/proc/loadavg")
        .map_err(|error| format!("read /proc/loadavg: {error}"))?;
    let fields: Vec<&str> = load_content.split_whitespace().collect();
    if fields.len() < 3 {
        return Err("invalid /proc/loadavg".into());
    }
    let mut load_average = [0.0; 3];
    for (index, field) in fields.iter().take(3).enumerate() {
        load_average[index] = field
            .parse()
            .map_err(|error| format!("parse load average: {error}"))?;
    }
    Ok((uptime_seconds, load_average))
}

fn read_memory_usage() -> Result<serde_json::Value, String> {
    let file =
        fs::File::open("/proc/meminfo").map_err(|error| format!("open /proc/meminfo: {error}"))?;
    let reader = BufReader::new(file);
    let mut total_kb = 0_u64;
    let mut available_kb = 0_u64;
    for line in reader.lines() {
        let line = line.map_err(|error| format!("read /proc/meminfo: {error}"))?;
        if let Some(value) = line.strip_prefix("MemTotal:") {
            total_kb = parse_meminfo_kb(value)?;
        } else if let Some(value) = line.strip_prefix("MemAvailable:") {
            available_kb = parse_meminfo_kb(value)?;
        }
    }
    if total_kb == 0 {
        return Err("memory total is unavailable".into());
    }
    let total_bytes = total_kb * 1024;
    let free_bytes = available_kb * 1024;
    let used_bytes = total_bytes.saturating_sub(free_bytes);
    let used_percent = round_percent(used_bytes, total_bytes);
    Ok(serde_json::json!({
        "total_bytes": total_bytes,
        "used_bytes": used_bytes,
        "free_bytes": free_bytes,
        "used_percent": used_percent,
    }))
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

fn read_root_filesystem_usage() -> Result<(u64, u64, u64, f64), String> {
    let stat = statvfs("/").map_err(|error| format!("stat root filesystem: {error}"))?;
    let block_size = stat.block_size() as u64;
    let total = stat.blocks() as u64 * block_size;
    let free = stat.blocks_available() as u64 * block_size;
    let used = total.saturating_sub(free);
    let used_percent = round_percent(used, total);
    Ok((total, used, free, used_percent))
}

fn round_percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    ((used as f64 / total as f64) * 1000.0).round() / 10.0
}

fn read_primary_network_counters() -> Result<serde_json::Value, String> {
    let output = Command::new("networkctl")
        .args(["--no-legend", "--no-pager", "list"])
        .output()
        .map_err(|error| format!("networkctl list: {error}"))?;
    if !output.status.success() {
        return Err("networkctl list failed".into());
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    let interfaces = candidate_network_interfaces(&listing)?;
    for interface in interfaces {
        if let Ok((rx_bytes, tx_bytes)) = read_interface_counters(&interface) {
            return Ok(serde_json::json!({
                "interface": interface,
                "rx_bytes": rx_bytes,
                "tx_bytes": tx_bytes,
            }));
        }
    }
    Err("network interface counters unavailable".into())
}

fn read_interface_counters(interface_name: &str) -> Result<(u64, u64), String> {
    let file =
        fs::File::open("/proc/net/dev").map_err(|error| format!("open /proc/net/dev: {error}"))?;
    let reader = BufReader::new(file);
    let prefix = format!("{interface_name}:");
    for line in reader.lines() {
        let line = line.map_err(|error| format!("read /proc/net/dev: {error}"))?;
        let line = line.trim();
        if !line.starts_with(&prefix) {
            continue;
        }
        let parts: Vec<&str> = line
            .trim_start_matches(&prefix)
            .split_whitespace()
            .collect();
        if parts.len() < 9 {
            return Err(format!("invalid /proc/net/dev entry for {interface_name}"));
        }
        let rx_bytes: u64 = parts[0]
            .parse()
            .map_err(|error| format!("parse rx bytes: {error}"))?;
        let tx_bytes: u64 = parts[8]
            .parse()
            .map_err(|error| format!("parse tx bytes: {error}"))?;
        return Ok((rx_bytes, tx_bytes));
    }
    Err(format!(
        "network interface {interface_name} not found in /proc/net/dev"
    ))
}

fn read_temperatures_from_sysfs() -> (Option<f64>, Option<f64>) {
    let (mut cpu_temp, mut chassis_temp) = read_hwmon_temperatures();
    if cpu_temp.is_none() {
        cpu_temp = read_thermal_zone_temperature("x86_pkg_temp");
    }
    if chassis_temp.is_none() {
        chassis_temp = read_thermal_zone_temperature("acpitz");
    }
    (cpu_temp, chassis_temp)
}

fn read_hwmon_temperatures() -> (Option<f64>, Option<f64>) {
    let entries = match fs::read_dir("/sys/class/hwmon") {
        Ok(entries) => entries,
        Err(_) => return (None, None),
    };
    let mut cpu_temp = None;
    let mut chassis_temp = None;
    for entry in entries.flatten() {
        let base = entry.path();
        let name = fs::read_to_string(base.join("name"))
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        for index in 1..=16 {
            let input_path = base.join(format!("temp{index}_input"));
            let Ok(content) = fs::read_to_string(&input_path) else {
                break;
            };
            let Some(temp) = parse_temperature_input(content.trim()) else {
                continue;
            };
            let label = fs::read_to_string(base.join(format!("temp{index}_label")))
                .unwrap_or_default()
                .to_lowercase();
            if cpu_temp.is_none()
                && (label.contains("package")
                    || label.contains("cpu")
                    || name.contains("k10temp")
                    || name.contains("coretemp"))
            {
                cpu_temp = Some(temp);
            } else if chassis_temp.is_none()
                && (label.contains("syst")
                    || label.contains("board")
                    || label.contains("chassis")
                    || name.contains("acpitz"))
            {
                chassis_temp = Some(temp);
            }
        }
    }
    (cpu_temp, chassis_temp)
}

fn read_thermal_zone_temperature(match_text: &str) -> Option<f64> {
    let entries = fs::read_dir("/sys/class/thermal").ok()?;
    let match_lower = match_text.to_lowercase();
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("thermal_zone")
        {
            continue;
        }
        let base = entry.path();
        let zone_type = fs::read_to_string(base.join("type"))
            .unwrap_or_default()
            .to_lowercase();
        if !zone_type.contains(&match_lower) {
            continue;
        }
        let content = fs::read_to_string(base.join("temp")).ok()?;
        if let Some(temp) = parse_temperature_input(content.trim()) {
            return Some(temp);
        }
    }
    None
}

fn parse_temperature_input(raw: &str) -> Option<f64> {
    if !TEMP_INPUT_PATTERN.is_match(raw) {
        return None;
    }
    let mut value: f64 = raw.parse().ok()?;
    if value <= 0.0 {
        return None;
    }
    if value > 200.0 {
        value = ((value / 1000.0) * 10.0).round() / 10.0;
    } else {
        value = (value * 10.0).round() / 10.0;
    }
    Some(value)
}
