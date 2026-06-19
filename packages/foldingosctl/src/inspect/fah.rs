use std::fs;
use std::process::Command;

use regex::Regex;
use rusqlite::OptionalExtension;
use serde_json::Value;
use std::sync::LazyLock;
use std::time::Duration;

use crate::config::load_effective_config_for_domain;
use crate::inspect::commissioning::read_current_release;
use crate::paths::AppliancePaths;

static FAH_PROJECT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)Project:\s*(\d+)\s*\(\s*Run\s*(\d+)\s*,\s*Clone\s*(\d+)\s*,\s*Gen\s*(\d+)\s*\)",
    )
    .expect("fah project pattern compiles")
});
static FAH_PROGRESS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Progress:\s*([\d.]+)\s*%").expect("fah progress pattern compiles")
});
static FAH_STEPS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Completed\s+(\d+)\s+out\s+of\s+(\d+)\s+steps\s+\(([\d.]+)%\)")
        .expect("fah steps pattern compiles")
});
static FAH_PPD_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)PPD[:\s]+([\d,.]+)").expect("fah ppd pattern compiles"));
static FAH_TPF_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)TPF[:\s]+([\d:]+(?:\.\d+)?)").expect("fah tpf pattern compiles")
});
static FAH_ERROR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(ERROR|FATAL|Exception|failed)\b").expect("fah error pattern compiles")
});
static FAH_CLIENT_VERSION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*client_version\s*=\s*"([^"]+)""#)
        .expect("fah client version pattern compiles")
});
static FAH_SHA256_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").expect("sha256 pattern compiles"));
static FAH_RUNTIME_TOKEN_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<(?:account-token|passkey)\b[^>]*\bv=['"]([^'"]+)['"]"#)
        .expect("fah runtime token pattern compiles")
});
static FAH_RUNTIME_USER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<user\b[^>]*\bv=['"]([^'"]+)['"]"#)
        .expect("fah runtime user pattern compiles")
});
static FAH_RUNTIME_TEAM_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<team\b[^>]*\bv=['"](\d+)['"]"#).expect("fah runtime team pattern compiles")
});
static FAH_RUNTIME_CPUS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<cpus\b[^>]*\bv=['"](\d+)['"]"#).expect("fah runtime cpus pattern compiles")
});

pub fn inspect_fah(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let service_active = systemd_unit_is_active("folding-at-home.service");
    let mut data = serde_json::json!({
        "service_active": service_active,
        "installed": false,
        "verified": false,
        "runtime": {
            "recent_errors": [],
        },
        "log_path": paths.fah_log.to_string_lossy(),
        "log_readable": fs::File::open(&paths.fah_log).is_ok(),
    });

    let manifest = fs::read_to_string(&paths.fah_embedded_manifest).ok();
    if let Some(manifest) = manifest.as_deref() {
        if let Some(expected_version) = parse_fah_client_version(manifest) {
            data["expected_client_version"] = serde_json::Value::String(expected_version);
        }
    }

    if let Ok(version) = read_current_release(&paths.fah_apps_root) {
        data["installed"] = serde_json::Value::Bool(true);
        data["active_client_version"] = serde_json::Value::String(version.clone());
        if let Some(manifest) = manifest.as_deref() {
            if let Some(verified) =
                fah_installation_verified(&paths.fah_apps_root, &version, manifest)
            {
                data["verified"] = serde_json::Value::Bool(verified);
            }
        }
    }

    let db_summary = read_fah_client_db_summary(paths);
    data["acquisition"] = fah_acquisition_state(paths);
    let mut runtime = parse_fah_log_state(&paths.fah_log);
    apply_fah_activity_summary(&mut runtime, service_active, &db_summary);
    data["runtime"] = runtime;
    if let Some(configuration) = foldinghome_configuration(paths, db_summary.cpus) {
        data["configuration"] = configuration;
    }
    Ok(data)
}

fn fah_acquisition_state(paths: &AppliancePaths) -> serde_json::Value {
    match crate::fah::load_fah_acquire_state(paths) {
        Ok(state) => serde_json::json!({
            "consecutive_failures": state.consecutive_failures,
            "next_attempt_unix": state.next_attempt_unix,
            "last_failure_reason": state.last_failure_reason,
        }),
        Err(error) => serde_json::json!({
            "state_error": error,
        }),
    }
}

fn foldinghome_configuration(
    paths: &AppliancePaths,
    db_cpus: Option<i64>,
) -> Option<serde_json::Value> {
    let runtime = read_runtime_config_summary(&paths.fah_runtime_config);
    let merged = load_effective_config_for_domain(paths, "foldinghome").ok();
    if merged.is_none() && !runtime.has_configuration() && db_cpus.is_none() {
        return None;
    }
    let username = runtime
        .username
        .as_deref()
        .or_else(|| {
            merged
                .as_ref()
                .and_then(|config| config.get("identity.username"))
                .map(|value| value.text.as_str())
        })
        .unwrap_or("Anonymous");
    let team = runtime.team.unwrap_or_else(|| {
        merged
            .as_ref()
            .and_then(|config| config.get("identity.team"))
            .map(|value| value.ival)
            .unwrap_or(0)
    });
    let passkey_secret = merged
        .as_ref()
        .and_then(|config| config.get("identity.passkey_secret"))
        .map(|value| value.text.as_str())
        .unwrap_or_default();
    let secret_file_configured =
        !passkey_secret.is_empty() && paths.secrets_dir().join(passkey_secret).is_file();
    let passkey_configured = runtime.passkey_configured || secret_file_configured;
    let cpus = runtime
        .cpus
        .or_else(|| {
            merged
                .as_ref()
                .and_then(|config| config.get("resources.cpus"))
                .map(|value| value.ival)
                .filter(|value| *value > 0)
        })
        .or(db_cpus);
    Some(serde_json::json!({
        "username": username,
        "team": team,
        "passkey_configured": passkey_configured,
        "cpus": cpus,
    }))
}

#[derive(Default)]
struct RuntimeConfigSummary {
    username: Option<String>,
    team: Option<i64>,
    cpus: Option<i64>,
    passkey_configured: bool,
}

impl RuntimeConfigSummary {
    fn has_configuration(&self) -> bool {
        self.username.is_some()
            || self.team.is_some()
            || self.cpus.is_some()
            || self.passkey_configured
    }
}

fn read_runtime_config_summary(path: &std::path::Path) -> RuntimeConfigSummary {
    let Ok(content) = fs::read_to_string(path) else {
        return RuntimeConfigSummary::default();
    };
    RuntimeConfigSummary {
        username: capture_runtime_attr(&FAH_RUNTIME_USER_PATTERN, &content),
        team: capture_runtime_attr(&FAH_RUNTIME_TEAM_PATTERN, &content)
            .and_then(|value| value.parse::<i64>().ok()),
        cpus: capture_runtime_attr(&FAH_RUNTIME_CPUS_PATTERN, &content)
            .and_then(|value| value.parse::<i64>().ok()),
        passkey_configured: capture_runtime_attr(&FAH_RUNTIME_TOKEN_PATTERN, &content)
            .is_some_and(|value| !value.trim().is_empty()),
    }
}

fn capture_runtime_attr(pattern: &Regex, content: &str) -> Option<String> {
    pattern
        .captures(content)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
}

#[derive(Default)]
struct FahClientDbSummary {
    readable: bool,
    cpus: Option<i64>,
    paused: bool,
    finish: bool,
    failed: Option<String>,
    unit: Option<FahDbUnitSummary>,
}

#[derive(Clone)]
struct FahDbUnitSummary {
    unit_state: Option<String>,
    project: Option<String>,
    progress: Option<f64>,
    ppd: Option<f64>,
}

fn read_fah_client_db_summary(paths: &AppliancePaths) -> FahClientDbSummary {
    let db_path = paths
        .fah_log
        .parent()
        .map(|parent| parent.join("client.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("/data/fah/client.db"));
    if !db_path.exists() {
        return FahClientDbSummary::default();
    }

    let Ok(connection) =
        rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return FahClientDbSummary::default();
    };
    let _ = connection.busy_timeout(Duration::from_secs(2));

    let mut summary = FahClientDbSummary {
        readable: true,
        ..FahClientDbSummary::default()
    };

    if sqlite_table_exists(&connection, "groups") {
        if let Some(group) = connection
            .query_row("SELECT value FROM groups WHERE name = ''", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        {
            summary.cpus = group_i64(&group, "cpus");
            summary.paused = group_bool(&group, "paused").unwrap_or(false);
            summary.finish = group_bool(&group, "finish").unwrap_or(false);
            summary.failed = group_string(&group, "failed");
        }
    }

    summary.unit = read_best_fah_unit(&connection);
    summary
}

fn sqlite_table_exists(connection: &rusqlite::Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .unwrap_or(false)
}

fn group_value<'a>(group: &'a Value, key: &str) -> Option<&'a Value> {
    group
        .get("config")
        .and_then(|config| config.get(key))
        .or_else(|| group.get(key))
}

fn group_bool(group: &Value, key: &str) -> Option<bool> {
    group_value(group, key).and_then(Value::as_bool)
}

fn group_i64(group: &Value, key: &str) -> Option<i64> {
    group_value(group, key).and_then(json_i64)
}

fn group_string(group: &Value, key: &str) -> Option<String> {
    group_value(group, key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn read_best_fah_unit(connection: &rusqlite::Connection) -> Option<FahDbUnitSummary> {
    if !sqlite_table_exists(connection, "units") {
        return None;
    }

    let mut statement = connection.prepare("SELECT value FROM units").ok()?;
    let units = statement
        .query_map([], |row| row.get::<_, String>(0))
        .ok()?
        .filter_map(|row| row.ok())
        .filter_map(|raw| parse_fah_unit_summary(&raw))
        .collect::<Vec<_>>();
    units.into_iter().max_by(|left, right| {
        score_fah_unit(left)
            .partial_cmp(&score_fah_unit(right))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn parse_fah_unit_summary(raw: &str) -> Option<FahDbUnitSummary> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let object = value.as_object()?;
    let state_value = object
        .get("state")
        .filter(|state| state.is_object())
        .unwrap_or(&value);
    let state_object = state_value.as_object()?;
    let unit_state = state_object
        .get("state")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_uppercase())
        .filter(|value| !value.is_empty());
    let project = extract_unit_project(state_value).or_else(|| extract_unit_project(&value));
    let progress = unit_progress_percent(state_object);
    let ppd = state_object
        .get("ppd")
        .and_then(json_f64)
        .or_else(|| object.get("ppd").and_then(json_f64))
        .filter(|value| *value > 0.0);

    if unit_state.is_none() && project.is_none() && progress.is_none() && ppd.is_none() {
        return None;
    }

    Some(FahDbUnitSummary {
        unit_state,
        project,
        progress,
        ppd,
    })
}

fn extract_unit_project(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    [
        object
            .get("assignment")
            .and_then(|assignment| assignment.get("project")),
        object
            .get("assignment")
            .and_then(|assignment| assignment.get("data"))
            .and_then(|data| data.get("project")),
        object
            .get("data")
            .and_then(|data| data.get("assignment"))
            .and_then(|assignment| assignment.get("data"))
            .and_then(|data| data.get("project")),
        object.get("project"),
    ]
    .into_iter()
    .flatten()
    .find_map(json_string_or_number)
}

fn unit_progress_percent(object: &serde_json::Map<String, Value>) -> Option<f64> {
    object
        .get("wu_progress")
        .and_then(json_f64)
        .or_else(|| object.get("progress").and_then(json_f64))
        .map(|progress| {
            if progress <= 1.0 {
                progress * 100.0
            } else {
                progress
            }
        })
        .filter(|progress| *progress >= 0.0)
        .map(|progress| (progress * 1000.0).round() / 1000.0)
}

fn json_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_f64().map(|value| value.round() as i64))
}

fn json_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|value| value as f64))
        .or_else(|| value.as_u64().map(|value| value as f64))
}

fn json_string_or_number(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| json_f64(value).map(|value| format!("{value:.0}")))
}

fn score_fah_unit(unit: &FahDbUnitSummary) -> f64 {
    let mut score = unit.progress.unwrap_or(0.0);
    if unit.project.is_some() {
        score += 100.0;
    }
    if unit.ppd.is_some() {
        score += 200.0;
    }
    match unit.unit_state.as_deref().unwrap_or("") {
        "RUN" => score + 1000.0,
        "DOWNLOAD" | "UPLOAD" | "READY" | "CORE" => score + 500.0,
        "PAUSE" | "FINISH" => score + 400.0,
        _ => score,
    }
}

fn apply_fah_activity_summary(
    runtime: &mut Value,
    service_active: bool,
    db_summary: &FahClientDbSummary,
) {
    let Some(runtime_object) = runtime.as_object_mut() else {
        return;
    };

    if !service_active {
        runtime_object.insert("folding_state".into(), serde_json::json!("stopped"));
        runtime_object.insert(
            "folding_detail".into(),
            serde_json::json!("FAH service is not running"),
        );
        return;
    }

    if db_summary.paused {
        runtime_object.insert("folding_state".into(), serde_json::json!("paused"));
        runtime_object.insert("unit_state".into(), serde_json::json!("PAUSE"));
        runtime_object.insert("folding_detail".into(), serde_json::json!("Paused"));
        return;
    }

    if db_summary.finish {
        runtime_object.insert("folding_state".into(), serde_json::json!("finishing"));
        runtime_object.insert("unit_state".into(), serde_json::json!("FINISH"));
        runtime_object.insert(
            "folding_detail".into(),
            serde_json::json!("Completing current work unit"),
        );
        return;
    }

    if let Some(unit) = db_summary.unit.as_ref() {
        let unit_state = unit.unit_state.as_deref().unwrap_or("").to_uppercase();
        let project_present = unit.project.is_some()
            || runtime_object
                .get("project")
                .and_then(Value::as_str)
                .is_some();
        let folding_state = folding_state_from_unit(&unit_state, project_present);
        runtime_object.insert("folding_state".into(), serde_json::json!(folding_state));
        if !unit_state.is_empty() {
            runtime_object.insert("unit_state".into(), serde_json::json!(unit_state));
        }
        if let Some(detail) = format_fah_activity_detail(runtime_object, unit) {
            runtime_object.insert("folding_detail".into(), serde_json::json!(detail));
        }
        return;
    }

    if let Some(project) = runtime_object
        .get("project")
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        runtime_object.insert("folding_state".into(), serde_json::json!("folding"));
        runtime_object.insert(
            "folding_detail".into(),
            serde_json::json!(format!("project {project}")),
        );
        return;
    }

    if db_summary.readable {
        runtime_object.insert("folding_state".into(), serde_json::json!("waiting"));
        runtime_object.insert("unit_state".into(), serde_json::json!("RUN"));
        runtime_object.insert(
            "folding_detail".into(),
            serde_json::json!("No work unit assigned"),
        );
    }

    if let Some(failed) = db_summary.failed.as_deref() {
        runtime_object.insert("folding_detail".into(), serde_json::json!(failed));
    }
}

fn folding_state_from_unit(unit_state: &str, project_present: bool) -> &'static str {
    match unit_state {
        "RUN" if project_present => "folding",
        "RUN" => "waiting",
        "PAUSE" => "paused",
        "FINISH" => "finishing",
        "DOWNLOAD" => "download",
        "UPLOAD" => "upload",
        "READY" => "ready",
        "CORE" => "core",
        "" => "idle",
        _ => "unknown",
    }
}

fn format_fah_activity_detail(
    runtime: &serde_json::Map<String, Value>,
    unit: &FahDbUnitSummary,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(project) = unit
        .project
        .as_deref()
        .or_else(|| runtime.get("project").and_then(Value::as_str))
    {
        parts.push(format!("project {project}"));
    }
    if let Some(progress) = unit
        .progress
        .or_else(|| runtime.get("progress").and_then(json_f64))
    {
        parts.push(format!("{progress:.1}%"));
    }
    if let Some(ppd) = unit.ppd.or_else(|| runtime.get("ppd").and_then(json_f64)) {
        parts.push(format!("{} PPD", format_ppd(ppd)));
    }
    if parts.is_empty() && unit.unit_state.as_deref() == Some("RUN") {
        Some("No work unit assigned".into())
    } else if parts.is_empty() {
        None
    } else {
        Some(parts.join(" - "))
    }
}

fn format_ppd(ppd: f64) -> String {
    if ppd >= 1_000_000.0 {
        format!("{:.2}M", ppd / 1_000_000.0)
    } else if ppd >= 1_000.0 {
        format!("{:.0}k", ppd / 1_000.0)
    } else {
        format!("{ppd:.0}")
    }
}

fn systemd_unit_is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn fah_installation_verified(
    apps_root: &std::path::Path,
    version: &str,
    manifest: &str,
) -> Option<bool> {
    let client_version = parse_fah_client_version(manifest)?;
    let sha256 = parse_fah_sha256(manifest)?;
    let marker_path = apps_root
        .join(version)
        .join(crate::paths::FAH_VERIFIED_MARKER);
    let marker = fs::read_to_string(marker_path).ok()?;
    let values = parse_key_value_lines(&marker);
    if values.get("client_version") != Some(&client_version) {
        return Some(false);
    }
    if values.get("artifact_sha256") != Some(&sha256) {
        return Some(false);
    }
    let executable_path = parse_fah_executable_path(manifest)?;
    let executable = apps_root.join(version).join(
        executable_path
            .strip_prefix("/data/apps/fah/current/")
            .unwrap_or(&executable_path),
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    const TEST_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn runtime_config_summary_reads_account_token_user_and_team() {
        let root = tempfile_dir("runtime-config-summary");
        let config = root.join("config.xml");
        fs::write(
            &config,
            r#"<config>
  <!-- Account -->
  <account-token v='FAKEFAHAccountTokenForTestsOnly1234567890XX'/>
  <machine-name v='folding-425564'/>

  <!-- User Information -->
  <team v='1068254'/>
  <user v='FoldingOS'/>
  <cpus v='4'/>
</config>
"#,
        )
        .unwrap();

        let summary = read_runtime_config_summary(&config);

        assert!(summary.passkey_configured);
        assert_eq!(summary.username.as_deref(), Some("FoldingOS"));
        assert_eq!(summary.team, Some(1_068_254));
        assert_eq!(summary.cpus, Some(4));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fah_client_db_summary_reads_cpus_and_activity() {
        let root = tempfile_dir("fah-client-db-summary");
        let paths = AppliancePaths {
            fah_log: root.join("fah/log.txt"),
            ..AppliancePaths::default()
        };
        fs::create_dir_all(paths.fah_log.parent().unwrap()).unwrap();
        let db_path = paths.fah_log.parent().unwrap().join("client.db");
        let connection = rusqlite::Connection::open(&db_path).expect("open sqlite");
        connection
            .execute(
                "CREATE TABLE groups (name TEXT PRIMARY KEY, value TEXT)",
                [],
            )
            .expect("create groups");
        connection
            .execute("CREATE TABLE units (value TEXT)", [])
            .expect("create units");
        connection
            .execute(
                "INSERT INTO groups (name, value) VALUES ('', ?1)",
                [r#"{"config":{"cpus":4,"paused":false,"finish":false}}"#],
            )
            .expect("insert group");
        connection
            .execute(
                "INSERT INTO units (value) VALUES (?1)",
                [r#"{"state":{"state":"RUN","wu_progress":0.125,"ppd":250000,"assignment":{"project":18400}}}"#],
            )
            .expect("insert unit");
        drop(connection);

        let summary = read_fah_client_db_summary(&paths);
        assert!(summary.readable);
        assert_eq!(summary.cpus, Some(4));
        assert_eq!(
            summary
                .unit
                .as_ref()
                .and_then(|unit| unit.unit_state.as_deref()),
            Some("RUN")
        );

        let mut runtime = serde_json::json!({"recent_errors":[]});
        apply_fah_activity_summary(&mut runtime, true, &summary);
        assert_eq!(runtime["folding_state"], "folding");
        assert_eq!(runtime["unit_state"], "RUN");
        assert_eq!(
            runtime["folding_detail"],
            "project 18400 - 12.5% - 250k PPD"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_fah_reports_installation_and_acquisition_state() {
        let root = tempfile_dir("inspect-fah-state");
        let paths = AppliancePaths {
            fah_apps_root: root.join("apps/fah"),
            fah_embedded_manifest: root.join("manifests/fah.toml"),
            fah_acquire_state: root.join("state/fah-acquire.state"),
            fah_log: root.join("fah/log.txt"),
            ..AppliancePaths::default()
        };

        fs::create_dir_all(paths.fah_apps_root.join("8.5.6/usr/bin")).unwrap();
        fs::create_dir_all(paths.fah_embedded_manifest.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.fah_acquire_state.parent().unwrap()).unwrap();
        fs::create_dir_all(paths.fah_log.parent().unwrap()).unwrap();
        fs::write(
            &paths.fah_embedded_manifest,
            format!(
                r#"schema_version = 1
client_version = "8.5.6"
architecture = "x86_64"
artifact_url = "https://download.foldingathome.org/releases/public/fah-client/debian-stable-64bit/v8.5/fah-client_8.5.6_amd64.deb"
artifact_size = 1
sha256 = "{TEST_SHA256}"
artifact_format = "deb"
minimum_foldingos_version = "0.1.0"
terms_url = "https://foldingathome.org/faq/opensource/"
executable_path = "/data/apps/fah/current/usr/bin/fah-client"
arguments = ["--config=/run/foldingos/fah/config.xml"]
"#
            ),
        )
        .unwrap();
        fs::write(paths.fah_apps_root.join("8.5.6/usr/bin/fah-client"), "").unwrap();
        fs::write(
            paths.fah_apps_root.join("8.5.6/.foldingos-verified"),
            format!("client_version=8.5.6\nartifact_sha256={TEST_SHA256}\n"),
        )
        .unwrap();
        symlink("8.5.6", paths.fah_current_link()).unwrap();
        fs::write(
            &paths.fah_acquire_state,
            "consecutive_failures=2\nnext_attempt_unix=1800000000\nlast_failure_reason=network is not online\n",
        )
        .unwrap();
        fs::write(&paths.fah_log, "Project: 18400 (Run 0, Clone 1, Gen 2)\n").unwrap();

        let data = inspect_fah(&paths).unwrap();
        assert_eq!(data["installed"], true);
        assert_eq!(data["verified"], true);
        assert_eq!(data["active_client_version"], "8.5.6");
        assert_eq!(data["expected_client_version"], "8.5.6");
        assert_eq!(data["log_readable"], true);
        assert_eq!(data["acquisition"]["consecutive_failures"], 2);
        assert_eq!(data["acquisition"]["next_attempt_unix"], 1_800_000_000);
        assert_eq!(
            data["acquisition"]["last_failure_reason"],
            "network is not online"
        );

        let _ = fs::remove_dir_all(root);
    }

    fn tempfile_dir(label: &str) -> std::path::PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "foldingosctl-{}-{}-{label}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
