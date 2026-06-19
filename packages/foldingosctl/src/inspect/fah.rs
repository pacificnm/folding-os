use std::fs;
use std::process::Command;

use regex::Regex;
use std::sync::LazyLock;

use crate::config::load_effective_config_for_domain;
use crate::inspect::commissioning::read_current_release;
use crate::paths::AppliancePaths;

static FAH_PROJECT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Project:\s*(\d+)\s*\(\s*Run\s*(\d+)\s*,\s*Clone\s*(\d+)\s*,\s*Gen\s*(\d+)\s*\)")
        .expect("fah project pattern compiles")
});
static FAH_PROGRESS_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)Progress:\s*([\d.]+)\s*%").expect("fah progress pattern compiles"));
static FAH_STEPS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Completed\s+(\d+)\s+out\s+of\s+(\d+)\s+steps\s+\(([\d.]+)%\)")
        .expect("fah steps pattern compiles")
});
static FAH_PPD_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)PPD[:\s]+([\d,.]+)").expect("fah ppd pattern compiles"));
static FAH_TPF_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)TPF[:\s]+([\d:]+(?:\.\d+)?)").expect("fah tpf pattern compiles"));
static FAH_ERROR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(ERROR|FATAL|Exception|failed)\b").expect("fah error pattern compiles")
});
static FAH_CLIENT_VERSION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*client_version\s*=\s*"([^"]+)""#).expect("fah client version pattern compiles")
});
static FAH_SHA256_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").expect("sha256 pattern compiles"));

pub fn inspect_fah(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let mut data = serde_json::json!({
        "service_active": systemd_unit_is_active("folding-at-home.service"),
        "verified": false,
        "runtime": {
            "recent_errors": [],
        },
        "log_path": paths.fah_log.to_string_lossy(),
    });

    if let Ok(version) = read_current_release(&paths.fah_apps_root) {
        data["active_client_version"] = serde_json::Value::String(version.clone());
        if let Ok(manifest) = fs::read_to_string(&paths.fah_embedded_manifest) {
            if let Some(verified) = fah_installation_verified(&paths.fah_apps_root, &version, &manifest) {
                data["verified"] = serde_json::Value::Bool(verified);
            }
        }
    }

    data["runtime"] = parse_fah_log_state(&paths.fah_log);
    if let Some(configuration) = foldinghome_configuration(paths) {
        data["configuration"] = configuration;
    }
    Ok(data)
}

fn foldinghome_configuration(paths: &AppliancePaths) -> Option<serde_json::Value> {
    let merged = load_effective_config_for_domain(paths, "foldinghome").ok()?;
    let username = merged
        .get("identity.username")
        .map(|value| value.text.as_str())
        .unwrap_or("Anonymous");
    let team = merged
        .get("identity.team")
        .map(|value| value.ival)
        .unwrap_or(0);
    let passkey_secret = merged
        .get("identity.passkey_secret")
        .map(|value| value.text.as_str())
        .unwrap_or_default();
    let passkey_configured = !passkey_secret.is_empty()
        && paths.secrets_dir().join(passkey_secret).is_file();
    Some(serde_json::json!({
        "username": username,
        "team": team,
        "passkey_configured": passkey_configured,
    }))
}

fn systemd_unit_is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn fah_installation_verified(apps_root: &std::path::Path, version: &str, manifest: &str) -> Option<bool> {
    let client_version = parse_fah_client_version(manifest)?;
    let sha256 = parse_fah_sha256(manifest)?;
    let marker_path = apps_root.join(version).join(crate::paths::FAH_VERIFIED_MARKER);
    let marker = fs::read_to_string(marker_path).ok()?;
    let values = parse_key_value_lines(&marker);
    if values.get("client_version") != Some(&client_version) {
        return Some(false);
    }
    if values.get("artifact_sha256") != Some(&sha256) {
        return Some(false);
    }
    let executable_path = parse_fah_executable_path(manifest)?;
    let executable = apps_root.join(version).join(executable_path.strip_prefix("/data/apps/fah/current/").unwrap_or(&executable_path));
    fs::metadata(executable).ok().map(|meta| meta.is_file())
}

fn parse_fah_client_version(manifest: &str) -> Option<String> {
    FAH_CLIENT_VERSION_PATTERN
        .captures(manifest)
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()))
}

fn parse_fah_sha256(manifest: &str) -> Option<String> {
    let pattern = Regex::new(r#"(?m)^\s*sha256\s*=\s*"([^"]+)""#).ok()?;
    pattern
        .captures(manifest)
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()))
        .filter(|value| FAH_SHA256_PATTERN.is_match(value))
}

fn parse_fah_executable_path(manifest: &str) -> Option<String> {
    let pattern = Regex::new(r#"(?m)^\s*executable_path\s*=\s*"([^"]+)""#).ok()?;
    pattern
        .captures(manifest)
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()))
}

fn parse_key_value_lines(content: &str) -> std::collections::HashMap<String, String> {
    let mut values = std::collections::HashMap::new();
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(key.trim().to_string(), value.trim().to_string());
    }
    values
}

fn parse_fah_log_state(path: &std::path::Path) -> serde_json::Value {
    let mut runtime = serde_json::json!({
        "recent_errors": [],
    });
    let Ok(content) = fs::read_to_string(path) else {
        return runtime;
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(500);
    let mut recent_errors = Vec::new();
    for line in &lines[start..] {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(captures) = FAH_PROJECT_PATTERN.captures(line) {
            runtime["project"] = serde_json::Value::String(captures[1].to_string());
            if let Ok(run) = captures[2].parse::<i64>() {
                runtime["run"] = serde_json::json!(run);
            }
            if let Ok(clone) = captures[3].parse::<i64>() {
                runtime["clone"] = serde_json::json!(clone);
            }
            if let Ok(gen) = captures[4].parse::<i64>() {
                runtime["gen"] = serde_json::json!(gen);
            }
        }
        if let Some(captures) = FAH_PROGRESS_PATTERN.captures(line) {
            if let Ok(progress) = captures[1].parse::<f64>() {
                runtime["progress"] = serde_json::json!(progress);
            }
        }
        if let Some(captures) = FAH_STEPS_PATTERN.captures(line) {
            if let Ok(progress) = captures[3].parse::<f64>() {
                runtime["progress"] = serde_json::json!(progress);
            }
        }
        if let Some(captures) = FAH_PPD_PATTERN.captures(line) {
            let raw = captures[1].replace(',', "");
            if let Ok(ppd) = raw.parse::<f64>() {
                runtime["ppd"] = serde_json::json!(ppd);
            }
        }
        if let Some(captures) = FAH_TPF_PATTERN.captures(line) {
            runtime["tpf"] = serde_json::Value::String(captures[1].to_string());
        }
        if FAH_ERROR_PATTERN.is_match(line) {
            recent_errors.push(line.to_string());
        }
    }
    if recent_errors.len() > 10 {
        recent_errors = recent_errors.split_off(recent_errors.len() - 10);
    }
    runtime["recent_errors"] = serde_json::Value::Array(
        recent_errors
            .into_iter()
            .map(serde_json::Value::String)
            .collect(),
    );
    runtime
}
