use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

use crate::config::Config;
use crate::foldingos::{self, FleetCommandError, FleetDelegateConfig};

pub const BACKUPS_DIR: &str = "/data/foldops/backups";

pub async fn create_export(config: &Config, include_secrets: bool) -> Result<Value, String> {
    let delegate = FleetDelegateConfig {
        foldingosctl_path: &config.foldingosctl_path,
    };
    let data = foldingos::recovery_export(delegate, include_secrets)
        .await
        .map_err(fleet_command_message)?;

    tracing::info!(
        path = data.get("path").and_then(|value| value.as_str()),
        sha256 = data.get("sha256").and_then(|value| value.as_str()),
        size_bytes = data.get("size_bytes").and_then(|value| value.as_u64()),
        include_secrets,
        "supervisor recovery export created"
    );

    Ok(data)
}

pub fn latest_backup_path(backups_dir: &Path) -> Result<PathBuf, String> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    let entries = std::fs::read_dir(backups_dir)
        .map_err(|error| format!("read backups directory {}: {error}", backups_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !name.starts_with("foldingos-supervisor-backup-") || !name.ends_with(".tar.zst") {
            continue;
        }
        let modified = entry
            .metadata()
            .map_err(|error| error.to_string())?
            .modified()
            .map_err(|error| error.to_string())?;
        match &newest {
            Some((current, _)) if modified <= *current => {}
            _ => newest = Some((modified, path)),
        }
    }
    newest
        .map(|(_, path)| path)
        .ok_or_else(|| "no recovery export is available".into())
}

fn fleet_command_message(error: FleetCommandError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn latest_backup_path_selects_newest_matching_archive() {
        let temp = TempDir::new().expect("tempdir");
        let older = temp
            .path()
            .join("foldingos-supervisor-backup-host-20260101T120000.tar.zst");
        let newer = temp
            .path()
            .join("foldingos-supervisor-backup-host-20260102T120000.tar.zst");
        fs::write(&older, b"old").expect("write older");
        fs::write(&newer, b"new").expect("write newer");
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&newer, b"newer").expect("touch newer");

        let latest = latest_backup_path(temp.path()).expect("latest backup");
        assert_eq!(latest, newer);
    }
}
