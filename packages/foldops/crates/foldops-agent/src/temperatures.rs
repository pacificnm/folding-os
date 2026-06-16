use std::path::Path;

use regex::Regex;
use std::sync::LazyLock;

static CPU_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(package id \d+|tctl|cpu|core 0|x86_pkg_temp|cpu-thermal|soc-thermal|k10temp)").unwrap()
});
static CHASSIS_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(syst|system|chassis|mb|motherboard|board|tmpin0|ambient|composite|pch|chipset|nvme|acpitz)").unwrap()
});
static PACKAGE_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)package id").unwrap());
static SYST_LABEL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^syst$").unwrap());
static SYSTEM_TEMP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^system temperature$").unwrap());
static THERMAL_CPU_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"x86_pkg_temp|cpu-thermal|soc-thermal|Processor").unwrap());
static THERMAL_CHASSIS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"acpitz|pch|chassis|board").unwrap());

pub struct Temperatures {
    pub cpu_temp: Option<f64>,
    pub chassis_temp: Option<f64>,
}

fn millidegrees_to_c(value: f64) -> f64 {
    ((value / 1000.0) * 10.0).round() / 10.0
}

fn parse_temp_input(raw: &str) -> Option<f64> {
    let n: f64 = raw.trim().parse().ok()?;
    if !n.is_finite() || n <= 0.0 {
        return None;
    }
    if n > 200.0 {
        Some(millidegrees_to_c(n))
    } else {
        Some((n * 10.0).round() / 10.0)
    }
}

async fn read_file_trim(path: &Path) -> Option<String> {
    tokio::fs::read_to_string(path)
        .await
        .ok()
        .map(|s| s.trim().to_string())
}

async fn read_hwmon_temps() -> (Option<f64>, Option<f64>) {
    let mut cpu = None;
    let mut chassis = None;

    let mut entries = match tokio::fs::read_dir("/sys/class/hwmon").await {
        Ok(e) => e,
        Err(_) => return (cpu, chassis),
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let base = entry.path();
        let name = read_file_trim(&base.join("name"))
            .await
            .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());

        let mut index = 1;
        loop {
            let label_path = base.join(format!("temp{index}_label"));
            let input_path = base.join(format!("temp{index}_input"));
            let input = read_file_trim(&input_path).await;
            if input.is_none() {
                break;
            }
            let Some(temp) = input.as_deref().and_then(parse_temp_input) else {
                index += 1;
                continue;
            };

            let label = read_file_trim(&label_path).await.unwrap_or_default();
            let tag = format!("{label} {name}").trim().to_string();

            if cpu.is_none()
                && (PACKAGE_ID_RE.is_match(&tag)
                    || CPU_LABEL.is_match(&tag)
                    || name == "coretemp"
                    || name == "k10temp")
            {
                cpu = Some(temp);
            }

            if chassis.is_none()
                && (SYST_LABEL_RE.is_match(&label)
                    || SYSTEM_TEMP_RE.is_match(&label)
                    || CHASSIS_LABEL.is_match(&tag))
            {
                chassis = Some(temp);
            }

            index += 1;
        }
    }

    (cpu, chassis)
}

async fn read_thermal_zone_temps() -> (Option<f64>, Option<f64>) {
    let mut cpu = None;
    let mut chassis = None;

    let mut entries = match tokio::fs::read_dir("/sys/class/thermal").await {
        Ok(e) => e,
        Err(_) => return (cpu, chassis),
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        if !entry.file_name().to_string_lossy().starts_with("thermal_zone") {
            continue;
        }
        let base = entry.path();
        let zone_type = read_file_trim(&base.join("type")).await.unwrap_or_default();
        let temp_raw = read_file_trim(&base.join("temp")).await;
        let Some(temp) = temp_raw.as_deref().and_then(parse_temp_input) else {
            continue;
        };

        if cpu.is_none() && THERMAL_CPU_RE.is_match(&zone_type) {
            cpu = Some(temp);
        }
        if chassis.is_none() && THERMAL_CHASSIS_RE.is_match(&zone_type) {
            chassis = Some(temp);
        }
    }

    (cpu, chassis)
}

async fn read_sensors_json() -> (Option<f64>, Option<f64>) {
    let mut cpu = None;
    let mut chassis = None;

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::process::Command::new("sensors").arg("-j").output(),
    )
    .await;

    let Ok(Ok(output)) = output else {
        return (cpu, chassis);
    };
    if !output.status.success() {
        return (cpu, chassis);
    }

    let Ok(data) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return (cpu, chassis);
    };
    let Some(chips) = data.as_object() else {
        return (cpu, chassis);
    };

    for chip in chips.values() {
        let Some(chip_obj) = chip.as_object() else { continue };
        for (sensor_name, sensor) in chip_obj {
            let Some(readings) = sensor.as_object() else { continue };
            for (key, val) in readings {
                let Some(num) = val.as_f64().filter(|&n| n > 0.0) else { continue };
                let Some(idx) = key.strip_prefix("temp").and_then(|rest| rest.strip_suffix("_input"))
                else {
                    continue;
                };
                let label_key = format!("temp{idx}_label");
                let label = readings
                    .get(&label_key)
                    .and_then(|l| l.as_str())
                    .unwrap_or(sensor_name);
                let temp = (num * 10.0).round() / 10.0;
                if cpu.is_none() && CPU_LABEL.is_match(label) {
                    cpu = Some(temp);
                }
                if chassis.is_none() && CHASSIS_LABEL.is_match(label) {
                    chassis = Some(temp);
                }
            }
        }
    }

    (cpu, chassis)
}

fn read_sysinfo_cpu_temp() -> Option<f64> {
    use sysinfo::Components;
    let components = Components::new_with_refreshed_list();
    let mut candidates = Vec::new();
    for c in components.iter() {
        if let Some(t) = c.temperature().filter(|&t| t > 0.0) {
            candidates.push(t as f64);
        }
    }
    if candidates.is_empty() {
        return None;
    }
    Some((candidates.into_iter().fold(f64::NAN, f64::max) * 10.0).round() / 10.0)
}

pub async fn collect_temperatures() -> Temperatures {
    let (hw_cpu, hw_chassis) = read_hwmon_temps().await;
    let (th_cpu, th_chassis) = read_thermal_zone_temps().await;
    let (sens_cpu, sens_chassis) = read_sensors_json().await;
    let si_cpu = read_sysinfo_cpu_temp();

    Temperatures {
        cpu_temp: hw_cpu.or(th_cpu).or(sens_cpu).or(si_cpu),
        chassis_temp: hw_chassis.or(th_chassis).or(sens_chassis),
    }
}
