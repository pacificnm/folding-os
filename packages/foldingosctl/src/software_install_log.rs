use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use serde_json::Value;

use crate::registry_image::current_import_timestamp;

const DEFAULT_LOG_PATH: &str = "/data/foldops/software-install.jsonl";
const MAX_FIELD_BYTES: usize = 16_384;

pub fn log_path() -> PathBuf {
    std::env::var("FOLDOPS_SOFTWARE_INSTALL_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_LOG_PATH))
}

pub fn ensure_ready() -> Result<(), String> {
    let path = log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create install log directory {}: {error}", parent.display()))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open install log {}: {error}", path.display()))?;
    Ok(())
}

pub fn should_log_command(command: &str) -> bool {
    let parts: Vec<&str> = command.split_whitespace().collect();
    match parts.as_slice() {
        ["foldops", "acquire"] => true,
        ["tools", "acquire"] => true,
        ["provision", "assign" | "assign-local"] => true,
        ["registry", "import-foldops-manifest" | "import-tools-release"] => true,
        _ => false,
    }
}

pub fn log_automation_outcome(
    command: &str,
    ok: bool,
    data: &Value,
    error_message: Option<&str>,
) {
    if !should_log_command(command) {
        return;
    }

    let phase = match command.split_whitespace().next().unwrap_or("") {
        "foldops" | "tools" => "apply",
        "provision" => "assign",
        "registry" => "import",
        _ => "ctl",
    };
    let operation = command
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string();
    let message = if ok {
        data.get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("command completed")
            .to_string()
    } else {
        error_message.unwrap_or("command failed").to_string()
    };
    let stdout = if ok {
        serde_json::to_string(data).unwrap_or_default()
    } else {
        String::new()
    };
    let detail = if ok { Some(data.clone()) } else { None };

    if let Err(error) = append_event(
        phase,
        &operation,
        command,
        ok,
        if ok { Some(0) } else { Some(1) },
        &message,
        &stdout,
        if ok { "" } else { &message },
        detail,
    ) {
        eprintln!("foldingosctl: install log: {error}");
    }
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
) -> Result<(), String> {
    let entry = serde_json::json!({
        "timestamp": current_import_timestamp(),
        "phase": phase,
        "operation": operation,
        "command": command,
        "ok": ok,
        "exit_code": exit_code,
        "message": truncate_field(message),
        "stdout": truncate_field(stdout),
        "stderr": truncate_field(stderr),
        "detail": detail,
    });
    append_json_line(&entry)
}

fn append_json_line(entry: &Value) -> Result<(), String> {
    ensure_ready()?;
    let path = log_path();
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
    use std::io::{BufRead, BufReader};

    fn list_entries(limit: usize) -> Result<(PathBuf, Vec<Value>), String> {
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
            if let Ok(entry) = serde_json::from_str::<Value>(line) {
                entries.push(entry);
            }
        }
        let keep = limit.max(1).min(1_000);
        if entries.len() > keep {
            entries.drain(0..entries.len() - keep);
        }
        Ok((path, entries))
    }

    #[test]
    fn logs_software_commands_only() {
        assert!(should_log_command("foldops acquire"));
        assert!(should_log_command("provision assign-local"));
        assert!(!should_log_command("inspect foldops"));
    }

    #[test]
    fn append_and_read_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "foldingosctl-install-log-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        env::set_var("FOLDOPS_SOFTWARE_INSTALL_LOG", &path);

        log_automation_outcome(
            "tools acquire",
            true,
            &serde_json::json!({ "message": "ok", "tools_version": "0.1.0-1" }),
            None,
        );
        let (_, entries) = list_entries(10).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["ok"], true);

        let _ = fs::remove_file(path);
        env::remove_var("FOLDOPS_SOFTWARE_INSTALL_LOG");
    }
}
