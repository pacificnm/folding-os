use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::automation_policy::is_foldops_automation_user;
use crate::foldops;
use crate::paths::AppliancePaths;

const FOLDINGOSCTL_PATH: &str = "/usr/bin/foldingosctl";

pub fn prepare_recovery_access(paths: &AppliancePaths) -> Result<(), String> {
    if is_foldops_automation_user() {
        return Ok(());
    }
    match foldops::prepare_recovery_config_permissions(paths) {
        Ok(()) => Ok(()),
        Err(error) if error.contains("group not found") => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn should_delegate_recovery_to_root(_paths: &AppliancePaths) -> bool {
    // Recovery reads the full supervisor state tree, not only env files. The
    // foldops service user always runs export through sudo so operators never
    // need shell access to fix permissions.
    is_foldops_automation_user()
}

pub fn should_delegate_recovery_import_to_root() -> bool {
    is_foldops_automation_user()
}

pub fn delegate_recovery_export(
    output_path: Option<&Path>,
    include_secrets: bool,
) -> Result<Value, String> {
    let mut command = Command::new("sudo");
    command.arg(FOLDINGOSCTL_PATH);
    command.args(["recovery", "export", "--format", "json"]);
    if include_secrets {
        command.arg("--include-secrets");
    }
    if let Some(output) = output_path {
        command.arg("--output");
        command.arg(output);
    }
    run_delegated_automation(command)
}

pub fn delegate_recovery_import(archive_path: &Path, dry_run: bool) -> Result<Value, String> {
    let mut command = Command::new("sudo");
    command.arg(FOLDINGOSCTL_PATH);
    command.args(["recovery", "import", "--format", "json"]);
    if dry_run {
        command.arg("--dry-run");
    }
    command.arg(archive_path);
    run_delegated_automation(command)
}

fn run_delegated_automation(mut command: Command) -> Result<Value, String> {
    let output = command
        .output()
        .map_err(|error| format!("run privileged recovery command: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(format!(
            "privileged recovery command failed (exit {}): {detail}",
            output.status.code().unwrap_or(-1)
        ));
    }
    parse_automation_data(stdout.trim())
}

fn parse_automation_data(stdout: &str) -> Result<Value, String> {
    let envelope: Value =
        serde_json::from_str(stdout).map_err(|error| format!("parse recovery JSON: {error}"))?;
    if envelope.get("ok").and_then(|value| value.as_bool()) != Some(true) {
        let message = envelope
            .pointer("/error/message")
            .and_then(|value| value.as_str())
            .unwrap_or("recovery command failed");
        return Err(message.to_string());
    }
    envelope
        .get("data")
        .cloned()
        .ok_or_else(|| "recovery JSON response did not include data".into())
}
