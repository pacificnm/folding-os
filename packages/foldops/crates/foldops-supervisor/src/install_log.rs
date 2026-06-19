use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_LOG_PATH: &str = "/data/foldops/software-install.jsonl";
const MAX_FIELD_BYTES: usize = 16_384;
const DEFAULT_LIST_LIMIT: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallLogEntry {
    pub timestamp: String,
    pub phase: String,
    pub operation: String,
    pub command: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stdout: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stderr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
}

pub fn log_path() -> PathBuf {
    std::env::var("FOLDOPS_SOFTWARE_INSTALL_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_LOG_PATH))
}

pub fn ensure_ready() -> Result<(), String> {
    let path = log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create install log directory: {error}"))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open install log {}: {error}", path.display()))?;
    Ok(())
}

pub fn append_event(
    phase: &str,
    operation: &str,
    command: &str,
    ok: bool,
    exit_code: Option<i32>,
    message: &str,
    stdout: &str,
    stderr: &str,
    detail: Option<Value>,
) {
    let entry = InstallLogEntry {
        timestamp: Utc::now().to_rfc3339(),
        phase: phase.to_string(),
        operation: operation.to_string(),
        command: command.to_string(),
        ok,
        exit_code,
        message: truncate_field(message),
        stdout: truncate_field(stdout),
        stderr: truncate_field(stderr),
        detail,
    };
    if let Err(error) = append_entry(&entry) {
        tracing::warn!(error = %error, "failed to append software install log entry");
    }
}

pub fn append_entry(entry: &InstallLogEntry) -> Result<(), String> {
    let path = log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create install log directory: {error}"))?;
    }

    let line = serde_json::to_string(entry).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open install log {}: {error}", path.display()))?;
    file.write_all(line.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|error| format!("write install log: {error}"))
}

pub fn list_entries(limit: usize) -> Result<(PathBuf, Vec<InstallLogEntry>), String> {
    let path = log_path();
    if !path.is_file() {
        return Ok((path, Vec::new()));
    }

    let file = fs::File::open(&path)
        .map_err(|error| format!("open install log {}: {error}", path.display()))?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|error| format!("read install log: {error}"))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<InstallLogEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(error) => entries.push(InstallLogEntry {
                timestamp: Utc::now().to_rfc3339(),
                phase: "log".into(),
                operation: "parse".into(),
                command: String::new(),
                ok: false,
                exit_code: None,
                message: format!("failed to parse install log line: {error}"),
                stdout: truncate_field(line),
                stderr: String::new(),
                detail: None,
            }),
        }
    }

    let keep = limit.max(1).min(1_000);
    if entries.len() > keep {
        entries.drain(0..entries.len() - keep);
    }
    Ok((path, entries))
}

pub fn list_response(limit: Option<usize>) -> Result<Value, String> {
    let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT);
    let (path, entries) = list_entries(limit)?;
    Ok(serde_json::json!({
        "path": path.to_string_lossy(),
        "updated_at": Utc::now().to_rfc3339(),
        "entries": entries,
    }))
}

pub fn should_log_command_args(args: &[&str]) -> bool {
    match args.first() {
        Some(&"foldops") => matches!(args.get(1), Some(&"acquire")),
        Some(&"tools") => matches!(args.get(1), Some(&"acquire")),
        Some(&"provision") => matches!(args.get(1), Some(&"assign") | Some(&"assign-local")),
        Some(&"registry") => matches!(
            args.get(1),
            Some(&"import-foldops-manifest") | Some(&"import-tools-release")
        ),
        _ => false,
    }
}

pub fn phase_for_command_args(args: &[&str]) -> &'static str {
    match args.first() {
        Some(&"foldops") | Some(&"tools") => "apply",
        Some(&"provision") => "assign",
        Some(&"registry") => "import",
        _ => "ctl",
    }
}

pub fn log_command_output(
    args: &[&str],
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    ok: bool,
    message: &str,
    detail: Option<Value>,
) {
    if !should_log_command_args(args) {
        return;
    }
    append_event(
        phase_for_command_args(args),
        args.get(1).unwrap_or(&""),
        &args.join(" "),
        ok,
        exit_code,
        message,
        stdout,
        stderr,
        detail,
    );
}

fn truncate_field(value: &str) -> String {
    if value.len() <= MAX_FIELD_BYTES {
        return value.to_string();
    }
    format!("{}… [truncated]", &value[..MAX_FIELD_BYTES])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn test_log_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "foldops-install-log-{}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn append_and_list_round_trip() {
        let path = test_log_path();
        let _ = fs::remove_file(&path);
        env::set_var("FOLDOPS_SOFTWARE_INSTALL_LOG", &path);

        append_event(
            "assign",
            "assign-local",
            "provision assign-local --foldops-manifest 0.1.0-24",
            true,
            Some(0),
            "updated 1 record",
            r#"{"ok":true}"#,
            "",
            None,
        );
        append_event(
            "apply",
            "acquire",
            "foldops acquire",
            false,
            Some(1),
            "permission denied",
            "",
            "permission denied",
            None,
        );

        let (_, entries) = list_entries(10).expect("list");
        assert_eq!(entries.len(), 2);
        assert!(entries[0].ok);
        assert!(!entries[1].ok);

        let _ = fs::remove_file(path);
        env::remove_var("FOLDOPS_SOFTWARE_INSTALL_LOG");
    }

    #[test]
    fn filters_logged_commands() {
        assert!(should_log_command_args(&["foldops", "acquire"]));
        assert!(should_log_command_args(&["provision", "assign-local"]));
        assert!(!should_log_command_args(&["inspect", "foldops"]));
    }
}
