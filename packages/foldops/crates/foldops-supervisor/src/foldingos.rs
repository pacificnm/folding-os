use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_FOLDINGOSCTL_PATH: &str = "/usr/bin/foldingosctl";
const DEFAULT_INSTALLATION_ROLE_PATH: &str = "/data/config/installation-role";

#[derive(Debug, Clone, Copy)]
pub struct FleetDelegateConfig<'a> {
    pub foldingosctl_path: &'a Path,
}

pub fn foldingos_delegation_enabled(installation_role_path: &Path) -> bool {
    match std::env::var("FOLDINGOS_DELEGATION").as_deref() {
        Ok("1") | Ok("true") | Ok("TRUE") => return true,
        Ok("0") | Ok("false") | Ok("FALSE") => return false,
        _ => {}
    }
    installation_role_path.is_file()
}

pub fn read_installation_role(installation_role_path: &Path) -> Option<String> {
    std::fs::read_to_string(installation_role_path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn default_installation_role_path() -> PathBuf {
    std::env::var("FOLDINGOS_INSTALLATION_ROLE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_INSTALLATION_ROLE_PATH))
}

pub fn default_foldingosctl_path() -> PathBuf {
    std::env::var("FOLDINGOSCTL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_FOLDINGOSCTL_PATH))
}

pub fn supervisor_fleet_delegation_enabled(installation_role_path: &Path) -> bool {
    if !foldingos_delegation_enabled(installation_role_path) {
        return false;
    }
    read_installation_role(installation_role_path).as_deref() == Some("supervisor")
}

#[derive(Debug, thiserror::Error)]
pub enum FleetCommandError {
    #[error("foldingosctl exited with status {status}: {message}")]
    CommandFailed { status: i32, message: String },
    #[error("failed to execute foldingosctl: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("foldingosctl output was not valid UTF-8")]
    InvalidUtf8,
    #[error("foldingosctl output was not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("foldingosctl returned failure for {command}: [{code}] {message}")]
    CommandRejected {
        command: String,
        code: String,
        message: String,
    },
}

#[derive(Debug, Deserialize)]
struct AutomationEnvelope {
    ok: bool,
    command: String,
    data: Option<Value>,
    error: Option<AutomationErrorBody>,
}

#[derive(Debug, Deserialize)]
struct AutomationErrorBody {
    code: String,
    message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AllowBootDevice {
    pub mac_address: String,
    #[serde(default)]
    pub install_disk: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AllowBootResult {
    pub mac_address: String,
    pub already_allowed: bool,
    #[serde(default)]
    pub install_disk: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AllowBootRequest {
    pub mac_address: String,
    pub install_disk: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnrollmentRecord {
    pub node_id: String,
    pub installation_role: String,
    pub hostname: String,
    pub current_image_version: String,
    pub desired_image_version: String,
    #[serde(default)]
    pub desired_foldops_manifest_release: String,
    #[serde(default)]
    pub desired_tools_version: String,
    pub foldingos_version: String,
    #[serde(default)]
    pub last_update_status: String,
    #[serde(default)]
    pub registered_at: String,
    #[serde(default)]
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryEntry {
    pub foldingos_version: String,
    pub rollout_state: String,
    pub image_sha256: String,
    pub image_size_bytes: i64,
    #[serde(default)]
    pub git_revision: String,
    #[serde(default)]
    pub import_timestamp: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssignResult {
    pub scope: String,
    pub updated_count: i64,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub image_version: Option<String>,
    #[serde(default)]
    pub foldops_manifest_release: Option<String>,
    #[serde(default)]
    pub tools_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AssignRequest {
    pub node_id: Option<String>,
    pub all: bool,
    pub image_version: Option<String>,
    pub foldops_manifest_release: Option<String>,
    pub tools_version: Option<String>,
}

pub async fn list_enrollments(
    config: FleetDelegateConfig<'_>,
) -> Result<Vec<EnrollmentRecord>, FleetCommandError> {
    let data = run_automation(config.foldingosctl_path, &["provision", "list-enrollments"]).await?;
    let enrollments = data
        .get("enrollments")
        .cloned()
        .unwrap_or(Value::Array(vec![]));
    Ok(serde_json::from_value(enrollments)?)
}

pub async fn inspect_node(
    config: FleetDelegateConfig<'_>,
) -> Result<Value, FleetCommandError> {
    run_automation(config.foldingosctl_path, &["inspect", "node"]).await
}

pub async fn inspect_foldops(
    config: FleetDelegateConfig<'_>,
) -> Result<Value, FleetCommandError> {
    run_automation(config.foldingosctl_path, &["inspect", "foldops"]).await
}

pub async fn inspect_tools(
    config: FleetDelegateConfig<'_>,
) -> Result<Value, FleetCommandError> {
    run_automation(config.foldingosctl_path, &["inspect", "tools"]).await
}

pub async fn foldops_acquire(
    config: FleetDelegateConfig<'_>,
) -> Result<Value, FleetCommandError> {
    run_automation(config.foldingosctl_path, &["foldops", "acquire"]).await
}

pub async fn tools_acquire(
    config: FleetDelegateConfig<'_>,
) -> Result<Value, FleetCommandError> {
    run_automation(config.foldingosctl_path, &["tools", "acquire"]).await
}

pub async fn list_allow_boot(
    config: FleetDelegateConfig<'_>,
) -> Result<Vec<AllowBootDevice>, FleetCommandError> {
    let data = run_automation(config.foldingosctl_path, &["provision", "list-allow-boot"]).await?;
    let devices = data
        .get("devices")
        .cloned()
        .unwrap_or(Value::Array(vec![]));
    Ok(serde_json::from_value(devices)?)
}

pub async fn provision_allow_boot(
    config: FleetDelegateConfig<'_>,
    request: AllowBootRequest,
) -> Result<AllowBootResult, FleetCommandError> {
    let mut args = vec!["provision", "allow-boot"];
    if let Some(disk) = request.install_disk.as_deref() {
        args.push("--disk");
        args.push(disk);
    }
    args.push(&request.mac_address);

    let data = run_automation(config.foldingosctl_path, &args).await?;
    Ok(serde_json::from_value(data)?)
}

pub async fn list_registry(
    config: FleetDelegateConfig<'_>,
) -> Result<Vec<RegistryEntry>, FleetCommandError> {
    let data = run_automation(config.foldingosctl_path, &["registry", "list"]).await?;
    let versions = data
        .get("versions")
        .cloned()
        .unwrap_or(Value::Array(vec![]));
    Ok(serde_json::from_value(versions)?)
}

pub async fn show_registry(
    config: FleetDelegateConfig<'_>,
    version: &str,
) -> Result<RegistryEntry, FleetCommandError> {
    let data = run_automation(
        config.foldingosctl_path,
        &["registry", "show", version],
    )
    .await?;
    Ok(serde_json::from_value(data)?)
}

pub async fn provision_assign(
    config: FleetDelegateConfig<'_>,
    request: AssignRequest,
) -> Result<AssignResult, FleetCommandError> {
    let mut args = vec!["provision", "assign"];
    if request.all {
        args.push("--all");
    } else if let Some(node_id) = request.node_id.as_deref() {
        args.push("--node");
        args.push(node_id);
    }
    if let Some(version) = request.image_version.as_deref() {
        args.push("--version");
        args.push(version);
    }
    if let Some(release) = request.foldops_manifest_release.as_deref() {
        args.push("--foldops-manifest");
        args.push(release);
    }
    if let Some(version) = request.tools_version.as_deref() {
        args.push("--tools-version");
        args.push(version);
    }

    let data = run_automation(config.foldingosctl_path, &args).await?;
    Ok(serde_json::from_value(data)?)
}

async fn run_automation(
    foldingosctl_path: &Path,
    command_args: &[&str],
) -> Result<Value, FleetCommandError> {
    let mut args = Vec::with_capacity(command_args.len() + 2);
    args.extend_from_slice(command_args);
    args.push("--format");
    args.push("json");

    let output = tokio::process::Command::new(foldingosctl_path)
        .args(&args)
        .output()
        .await?;

    let stdout = String::from_utf8(output.stdout).map_err(|_| FleetCommandError::InvalidUtf8)?;
    let envelope: AutomationEnvelope = serde_json::from_str(stdout.trim())?;

    if !output.status.success() || !envelope.ok {
        if let Some(error) = envelope.error {
            return Err(FleetCommandError::CommandRejected {
                command: envelope.command,
                code: error.code,
                message: error.message,
            });
        }
        return Err(FleetCommandError::CommandFailed {
            status: output.status.code().unwrap_or(-1),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    envelope.data.ok_or_else(|| FleetCommandError::CommandRejected {
        command: envelope.command,
        code: "missing_data".into(),
        message: "automation response did not include data".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn write_mock_foldingosctl(dir: &TempDir, script: &str) -> PathBuf {
        let path = dir.path().join("foldingosctl");
        fs::write(&path, script).expect("write mock foldingosctl");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod mock foldingosctl");
        path
    }

    #[tokio::test]
    async fn list_enrollments_parses_automation_json() {
        let temp = TempDir::new().expect("tempdir");
        let script = r#"#!/bin/sh
if [ "$1" = "provision" ] && [ "$2" = "list-enrollments" ]; then
  printf '%s' '{"schema_version":1,"ok":true,"command":"provision list-enrollments","data":{"enrollments":[{"schema_version":1,"node_id":"550e8400-e29b-41d4-a716-446655440000","installation_role":"agent","hostname":"folding-test","current_image_version":"0.1.0","desired_image_version":"0.1.0","foldingos_version":"0.1.0","registered_at":"2026-01-01T00:00:00Z","last_seen_at":"2026-01-02T00:00:00Z"}]}}'
  exit 0
fi
exit 1
"#;
        let foldingosctl = write_mock_foldingosctl(&temp, script);
        let records = list_enrollments(FleetDelegateConfig {
            foldingosctl_path: &foldingosctl,
        })
        .await
        .expect("list enrollments");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].node_id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(records[0].hostname, "folding-test");
    }

    #[tokio::test]
    async fn list_allow_boot_parses_automation_json() {
        let temp = TempDir::new().expect("tempdir");
        let script = r#"#!/bin/sh
if [ "$1" = "provision" ] && [ "$2" = "list-allow-boot" ]; then
  printf '%s' '{"schema_version":1,"ok":true,"command":"provision list-allow-boot","data":{"devices":[{"mac_address":"00:be:43:e7:59:5e"},{"mac_address":"52:54:00:12:34:56","install_disk":"/dev/sda"}]}}'
  exit 0
fi
exit 1
"#;
        let foldingosctl = write_mock_foldingosctl(&temp, script);
        let devices = list_allow_boot(FleetDelegateConfig {
            foldingosctl_path: &foldingosctl,
        })
        .await
        .expect("list allow boot");
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].mac_address, "00:be:43:e7:59:5e");
        assert_eq!(
            devices[1].install_disk.as_deref(),
            Some("/dev/sda")
        );
    }

    #[tokio::test]
    async fn provision_allow_boot_parses_automation_json() {
        let temp = TempDir::new().expect("tempdir");
        let script = r#"#!/bin/sh
if [ "$1" = "provision" ] && [ "$2" = "allow-boot" ]; then
  printf '%s' '{"schema_version":1,"ok":true,"command":"provision allow-boot","data":{"mac_address":"00:be:43:e7:59:5e","already_allowed":false}}'
  exit 0
fi
exit 1
"#;
        let foldingosctl = write_mock_foldingosctl(&temp, script);
        let result = provision_allow_boot(
            FleetDelegateConfig {
                foldingosctl_path: &foldingosctl,
            },
            AllowBootRequest {
                mac_address: "00:be:43:e7:59:5e".into(),
                install_disk: None,
            },
        )
        .await
        .expect("allow boot");
        assert_eq!(result.mac_address, "00:be:43:e7:59:5e");
        assert!(!result.already_allowed);
    }

    #[tokio::test]
    async fn provision_assign_parses_result_summary() {
        let temp = TempDir::new().expect("tempdir");
        let script = r#"#!/bin/sh
if [ "$1" = "provision" ] && [ "$2" = "assign" ]; then
  printf '%s' '{"schema_version":1,"ok":true,"command":"provision assign","data":{"scope":"node","updated_count":1,"node_id":"550e8400-e29b-41d4-a716-446655440000","image_version":"0.2.0"}}'
  exit 0
fi
exit 1
"#;
        let foldingosctl = write_mock_foldingosctl(&temp, script);
        let result = provision_assign(
            FleetDelegateConfig {
                foldingosctl_path: &foldingosctl,
            },
            AssignRequest {
                node_id: Some("550e8400-e29b-41d4-a716-446655440000".into()),
                all: false,
                image_version: Some("0.2.0".into()),
                foldops_manifest_release: None,
                tools_version: None,
            },
        )
        .await
        .expect("assign");
        assert_eq!(result.updated_count, 1);
        assert_eq!(result.image_version.as_deref(), Some("0.2.0"));
    }

    #[test]
    fn supervisor_delegation_requires_supervisor_role() {
        let temp = TempDir::new().expect("tempdir");
        let role_path = temp.path().join("installation-role");
        assert!(!supervisor_fleet_delegation_enabled(&role_path));

        fs::write(&role_path, "agent\n").expect("write role");
        assert!(!supervisor_fleet_delegation_enabled(&role_path));

        fs::write(&role_path, "supervisor\n").expect("write role");
        assert!(supervisor_fleet_delegation_enabled(&role_path));
    }
}
