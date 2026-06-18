use std::path::{Path, PathBuf};

use chrono::Utc;
use foldops_types::{
    Disk, Fah, FahSystemdStatus, IngestPayload, Maintenance, Memory, Network, System,
};
use serde::Deserialize;
use serde_json::Value;

use crate::collector::{network_with_rates, FahStats};

const DEFAULT_FOLDINGOSCTL_PATH: &str = "/usr/bin/foldingosctl";
const DEFAULT_INSTALLATION_ROLE_PATH: &str = "/data/config/installation-role";

#[derive(Debug, Clone)]
pub struct DelegatedCollectConfig<'a> {
    pub foldingosctl_path: &'a Path,
    pub fah_stats: FahStats,
}

pub fn foldingos_delegation_enabled(installation_role_path: &Path) -> bool {
    match std::env::var("FOLDINGOS_DELEGATION").as_deref() {
        Ok("1") | Ok("true") | Ok("TRUE") => return true,
        Ok("0") | Ok("false") | Ok("FALSE") => return false,
        _ => {}
    }
    installation_role_path.is_file()
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

pub async fn collect_delegated_snapshot(config: DelegatedCollectConfig<'_>) -> IngestPayload {
    let mut warnings: Vec<String> = Vec::new();

    let node = match run_inspect(config.foldingosctl_path, "node").await {
        Ok(data) => Some(parse_inspect_node(data)),
        Err(error) => {
            warnings.push(format!("inspect node failed: {error}"));
            None
        }
    };
    let system = match run_inspect(config.foldingosctl_path, "system").await {
        Ok(data) => Some(parse_inspect_system(data)),
        Err(error) => {
            warnings.push(format!("inspect system failed: {error}"));
            None
        }
    };
    let fah = match run_inspect(config.foldingosctl_path, "fah").await {
        Ok(data) => Some(parse_inspect_fah(data)),
        Err(error) => {
            warnings.push(format!("inspect fah failed: {error}"));
            None
        }
    };
    let update = match run_inspect(config.foldingosctl_path, "update").await {
        Ok(data) => Some(parse_inspect_update(data)),
        Err(error) => {
            warnings.push(format!("inspect update failed: {error}"));
            None
        }
    };
    for subcommand in ["commissioning", "foldops", "tools"] {
        if let Err(error) = run_inspect(config.foldingosctl_path, subcommand).await {
            warnings.push(format!("inspect {subcommand} failed: {error}"));
        }
    }

    for warning in &warnings {
        tracing::warn!(warning = %warning, "foldingosctl inspect partial failure");
    }

    let hostname = node
        .as_ref()
        .map(|value| value.hostname.clone())
        .or_else(fallback_hostname)
        .unwrap_or_else(|| "unknown".into());

    let system_payload = system.unwrap_or_else(default_system_payload);
    let fah_payload = fah
        .map(|value| fah_to_payload(value, &config.fah_stats))
        .unwrap_or_else(|| empty_fah_payload(&config.fah_stats));
    let maintenance = update
        .map(|value| Maintenance {
            aptUpdatesAvailable: 0,
            rebootRequired: value.reboot_required,
        })
        .unwrap_or(Maintenance {
            aptUpdatesAvailable: 0,
            rebootRequired: false,
        });

    IngestPayload {
        hostname,
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        nodeId: node.as_ref().map(|value| value.node_id.clone()),
        installationRole: node.as_ref().map(|value| value.installation_role.clone()),
        foldingosVersion: node.as_ref().map(|value| value.foldingos_version.clone()),
        system: system_payload,
        fah: fah_payload,
        maintenance,
        logs: None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InspectCommandError {
    #[error("foldingosctl exited with status {status}: {message}")]
    CommandFailed { status: i32, message: String },
    #[error("failed to execute foldingosctl: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("foldingosctl output was not valid UTF-8")]
    InvalidUtf8,
    #[error("foldingosctl output was not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("foldingosctl returned failure for {command}: [{code}] {message}")]
    InspectFailed {
        command: String,
        code: String,
        message: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AutomationCommandError {
    #[error("foldingosctl exited with status {status}: {message}")]
    CommandFailed { status: i32, message: String },
    #[error("failed to execute foldingosctl: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("foldingosctl output was not valid UTF-8")]
    InvalidUtf8,
    #[error("foldingosctl output was not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("foldingosctl rejected {command}: [{code}] {message}")]
    CommandRejected {
        command: String,
        code: String,
        message: String,
    },
}

const FOLDINGHOME_CANDIDATES_DIR: &str = "/data/config/candidates";

pub fn write_foldinghome_candidate(content: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(FOLDINGHOME_CANDIDATES_DIR)
        .map_err(|error| format!("create candidates dir: {error}"))?;
    let path = PathBuf::from(format!(
        "{FOLDINGHOME_CANDIDATES_DIR}/foldinghome-{}.toml",
        Utc::now().timestamp_millis()
    ));
    std::fs::write(&path, content).map_err(|error| format!("write candidate: {error}"))?;
    Ok(path)
}

pub async fn activate_foldinghome_config(
    foldingosctl_path: &Path,
    candidate_path: &Path,
) -> Result<Value, AutomationCommandError> {
    let candidate = candidate_path
        .to_str()
        .ok_or_else(|| AutomationCommandError::CommandRejected {
            command: "config activate foldinghome".into(),
            code: "invalid_input".into(),
            message: "candidate path is not valid UTF-8".into(),
        })?;
    run_automation(
        foldingosctl_path,
        &["config", "activate", "foldinghome", candidate],
    )
    .await
}

async fn run_automation(
    foldingosctl_path: &Path,
    command_args: &[&str],
) -> Result<Value, AutomationCommandError> {
    let mut args = Vec::with_capacity(command_args.len() + 2);
    args.extend_from_slice(command_args);
    args.push("--format");
    args.push("json");

    let output = tokio::process::Command::new(foldingosctl_path)
        .args(&args)
        .output()
        .await?;

    let stdout = String::from_utf8(output.stdout).map_err(|_| AutomationCommandError::InvalidUtf8)?;
    let envelope: AutomationEnvelope = serde_json::from_str(stdout.trim())?;

    if !output.status.success() || !envelope.ok {
        if let Some(error) = envelope.error {
            return Err(AutomationCommandError::CommandRejected {
                command: envelope.command,
                code: error.code,
                message: error.message,
            });
        }
        return Err(AutomationCommandError::CommandFailed {
            status: output.status.code().unwrap_or(-1),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    envelope.data.ok_or_else(|| AutomationCommandError::CommandRejected {
        command: envelope.command,
        code: "missing_data".into(),
        message: "automation response did not include data".into(),
    })
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

#[derive(Debug, Default, Deserialize)]
struct InspectNodeData {
    node_id: String,
    hostname: String,
    installation_role: String,
    foldingos_version: String,
}

#[derive(Debug, Default, Deserialize)]
struct InspectSystemMemory {
    total_bytes: u64,
    used_bytes: u64,
    free_bytes: u64,
    used_percent: f64,
}

#[derive(Debug, Default, Deserialize)]
struct InspectSystemFilesystem {
    total_bytes: u64,
    used_bytes: u64,
    free_bytes: u64,
    used_percent: f64,
}

#[derive(Debug, Default, Deserialize)]
struct InspectSystemNetwork {
    rx_bytes: u64,
    tx_bytes: u64,
}

#[derive(Debug, Default, Deserialize)]
struct InspectSystemData {
    uptime_seconds: f64,
    load_average: [f64; 3],
    memory: InspectSystemMemory,
    root_filesystem: InspectSystemFilesystem,
    primary_network: Option<InspectSystemNetwork>,
    cpu_temp_celsius: Option<f64>,
    chassis_temp_celsius: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct InspectFahRuntime {
    project: Option<String>,
    run: Option<i64>,
    clone: Option<i64>,
    gen: Option<i64>,
    progress: Option<f64>,
    ppd: Option<f64>,
    tpf: Option<String>,
    #[serde(default)]
    recent_errors: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct InspectFahData {
    service_active: bool,
    runtime: InspectFahRuntime,
}

#[derive(Debug, Default, Deserialize)]
struct InspectUpdateData {
    reboot_required: bool,
}

async fn run_inspect(foldingosctl_path: &Path, subcommand: &str) -> Result<Value, InspectCommandError> {
    let output = tokio::process::Command::new(foldingosctl_path)
        .args(["inspect", subcommand, "--format", "json"])
        .output()
        .await?;

    let stdout = String::from_utf8(output.stdout).map_err(|_| InspectCommandError::InvalidUtf8)?;
    let envelope: AutomationEnvelope = serde_json::from_str(stdout.trim())?;

    if !output.status.success() || !envelope.ok {
        if let Some(error) = envelope.error {
            return Err(InspectCommandError::InspectFailed {
                command: envelope.command,
                code: error.code,
                message: error.message,
            });
        }
        return Err(InspectCommandError::CommandFailed {
            status: output.status.code().unwrap_or(-1),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    envelope
        .data
        .ok_or_else(|| InspectCommandError::InspectFailed {
            command: envelope.command,
            code: "missing_data".into(),
            message: "inspect response did not include data".into(),
        })
}

fn parse_inspect_node(value: Value) -> InspectNodeData {
    serde_json::from_value(value).unwrap_or_default()
}

fn parse_inspect_system(value: Value) -> System {
    let data: InspectSystemData = serde_json::from_value(value).unwrap_or_default();
    let network = data
        .primary_network
        .map(|network| network_with_rates(network.rx_bytes, network.tx_bytes))
        .unwrap_or(Network {
            rxBytes: 0,
            txBytes: 0,
            rxSec: None,
            txSec: None,
        });

    System {
        uptime: data.uptime_seconds,
        loadAvg: data.load_average,
        cpuUsage: 0.0,
        memory: Memory {
            total: data.memory.total_bytes as f64,
            used: data.memory.used_bytes as f64,
            free: data.memory.free_bytes as f64,
            percent: data.memory.used_percent,
        },
        disk: Disk {
            total: data.root_filesystem.total_bytes as f64,
            used: data.root_filesystem.used_bytes as f64,
            free: data.root_filesystem.free_bytes as f64,
            percent: data.root_filesystem.used_percent,
        },
        network,
        cpuTemp: data.cpu_temp_celsius,
        chassisTemp: data.chassis_temp_celsius,
    }
}

fn parse_inspect_fah(value: Value) -> InspectFahData {
    serde_json::from_value(value).unwrap_or_default()
}

fn parse_inspect_update(value: Value) -> InspectUpdateData {
    serde_json::from_value(value).unwrap_or_default()
}

fn fah_to_payload(data: InspectFahData, stats: &FahStats) -> Fah {
    Fah {
        systemdStatus: if data.service_active {
            FahSystemdStatus::Active
        } else {
            FahSystemdStatus::Inactive
        },
        project: data.runtime.project,
        run: data.runtime.run.map(|value| value as f64),
        clone: data.runtime.clone.map(|value| value as f64),
        gen: data.runtime.gen.map(|value| value as f64),
        progress: data.runtime.progress,
        ppd: data.runtime.ppd,
        tpf: data.runtime.tpf,
        recentErrors: data.runtime.recent_errors,
        statsDonor: stats.donor.clone(),
        statsTeam: stats.team.clone(),
    }
}

fn empty_fah_payload(stats: &FahStats) -> Fah {
    Fah {
        systemdStatus: FahSystemdStatus::Unknown,
        project: None,
        run: None,
        clone: None,
        gen: None,
        progress: None,
        ppd: None,
        tpf: None,
        recentErrors: vec![],
        statsDonor: stats.donor.clone(),
        statsTeam: stats.team.clone(),
    }
}

fn fallback_hostname() -> Option<String> {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
}

fn default_system_payload() -> System {
    System {
        uptime: 0.0,
        loadAvg: [0.0, 0.0, 0.0],
        cpuUsage: 0.0,
        memory: Memory {
            total: 0.0,
            used: 0.0,
            free: 0.0,
            percent: 0.0,
        },
        disk: Disk {
            total: 0.0,
            used: 0.0,
            free: 0.0,
            percent: 0.0,
        },
        network: Network {
            rxBytes: 0,
            txBytes: 0,
            rxSec: None,
            txSec: None,
        },
        cpuTemp: None,
        chassisTemp: None,
    }
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
    async fn delegated_snapshot_maps_inspect_json_to_ingest_payload() {
        let temp = TempDir::new().expect("tempdir");
        let script = r#"#!/bin/sh
case "$1:$2" in
  inspect:node)
    printf '%s' '{"schema_version":1,"ok":true,"command":"inspect node","data":{"node_id":"550e8400-e29b-41d4-a716-446655440000","hostname":"folding-test","installation_role":"agent","foldingos_version":"0.1.0","kernel_version":"go1.22","mac_addresses":["52:54:00:12:34:56"]}}'
    ;;
  inspect:system)
    printf '%s' '{"schema_version":1,"ok":true,"command":"inspect system","data":{"uptime_seconds":3600,"load_average":[0.1,0.2,0.3],"memory":{"total_bytes":1000,"used_bytes":400,"free_bytes":600,"used_percent":40},"root_filesystem":{"mountpoint":"/","total_bytes":2000,"used_bytes":500,"free_bytes":1500,"used_percent":25},"primary_network":{"interface":"eth0","rx_bytes":100,"tx_bytes":50},"cpu_temp_celsius":42.5}}'
    ;;
  inspect:fah)
    printf '%s' '{"schema_version":1,"ok":true,"command":"inspect fah","data":{"service_active":true,"verified":true,"runtime":{"project":"18400","run":0,"clone":1,"gen":2,"progress":12.5,"ppd":250000,"recent_errors":[]}}}'
    ;;
  inspect:update)
    printf '%s' '{"schema_version":1,"ok":true,"command":"inspect update","data":{"current_image_version":"0.1.0","reboot_required":true}}'
    ;;
  *)
    printf '%s' '{"schema_version":1,"ok":false,"command":"inspect unknown","error":{"code":"invalid_input","message":"unknown"}}'
    exit 1
    ;;
esac
"#;
        let foldingosctl = write_mock_foldingosctl(&temp, script);
        let payload = collect_delegated_snapshot(DelegatedCollectConfig {
            foldingosctl_path: &foldingosctl,
            fah_stats: FahStats {
                donor: Some("donor".into()),
                team: Some("123456".into()),
            },
        })
        .await;

        assert_eq!(payload.hostname, "folding-test");
        assert_eq!(
            payload.nodeId.as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(payload.installationRole.as_deref(), Some("agent"));
        assert_eq!(payload.foldingosVersion.as_deref(), Some("0.1.0"));
        assert_eq!(payload.system.uptime, 3600.0);
        assert_eq!(payload.system.memory.percent, 40.0);
        assert_eq!(payload.fah.systemdStatus, FahSystemdStatus::Active);
        assert_eq!(payload.fah.project.as_deref(), Some("18400"));
        assert!(payload.maintenance.rebootRequired);
        assert!(payload.logs.is_none());
    }

    #[test]
    fn delegation_enabled_when_installation_role_exists() {
        let temp = TempDir::new().expect("tempdir");
        let role_path = temp.path().join("installation-role");
        assert!(!foldingos_delegation_enabled(&role_path));
        fs::write(&role_path, "agent\n").expect("write role");
        assert!(foldingos_delegation_enabled(&role_path));
    }

    #[tokio::test]
    async fn activate_foldinghome_config_parses_automation_json() {
        let temp = TempDir::new().expect("tempdir");
        let candidate = temp.path().join("candidate.toml");
        fs::write(&candidate, "schema_version = 1\n").expect("write candidate");
        let script = r#"#!/bin/sh
if [ "$1" = "config" ] && [ "$2" = "activate" ] && [ "$3" = "foldinghome" ]; then
  printf '%s' '{"schema_version":1,"ok":true,"command":"config activate foldinghome","data":{"domain":"foldinghome","candidate":"'"$4"'","activated":true}}'
  exit 0
fi
exit 1
"#;
        let foldingosctl = write_mock_foldingosctl(&temp, script);
        let data = activate_foldinghome_config(&foldingosctl, &candidate)
            .await
            .expect("activate foldinghome");
        assert_eq!(data.get("domain").and_then(|value| value.as_str()), Some("foldinghome"));
        assert_eq!(
            data.get("activated").and_then(|value| value.as_bool()),
            Some(true)
        );
    }
}
