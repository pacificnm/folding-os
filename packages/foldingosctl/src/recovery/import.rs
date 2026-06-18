use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tar::Archive;

use crate::automation_policy::require_supervisor_automation_mutation;
use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;
use crate::recovery::bundle::{
    hash_file, validate_restore_target, RecoveryManifest, MANIFEST_SCHEMA_VERSION,
    MAX_EXPORT_BYTES,
};
use crate::recovery::privilege::{
    delegate_recovery_import, prepare_recovery_access, should_delegate_recovery_import_to_root,
};
use crate::role::require_supervisor_role;

pub struct ImportOptions {
    pub dry_run: bool,
}

pub fn recovery_import(
    paths: &AppliancePaths,
    archive_path: &Path,
    options: ImportOptions,
) -> Result<serde_json::Value, String> {
    require_supervisor_role(paths)?;

    if should_delegate_recovery_import_to_root() {
        return delegate_recovery_import(archive_path, options.dry_run);
    }

    prepare_recovery_access(paths)?;
    require_supervisor_automation_mutation(paths, "recovery", "import")?;

    let temp_dir = std::env::temp_dir().join(format!(
        "foldingos-recovery-import-{}",
        std::process::id()
    ));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&temp_dir).map_err(|error| error.to_string())?;

    let result = import_inner(paths, archive_path, &temp_dir, options);
    let _ = fs::remove_dir_all(&temp_dir);
    result
}

fn import_inner(
    paths: &AppliancePaths,
    archive_path: &Path,
    temp_dir: &Path,
    options: ImportOptions,
) -> Result<serde_json::Value, String> {
    extract_archive(archive_path, temp_dir)?;
    let manifest_path = temp_dir.join("manifest.json");
    let manifest_content = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read manifest.json: {error}"))?;
    let manifest: RecoveryManifest = serde_json::from_str(&manifest_content)
        .map_err(|error| format!("parse manifest.json: {error}"))?;
    validate_manifest(&manifest, temp_dir, archive_path)?;

    if options.dry_run {
        return Ok(serde_json::json!({
            "dry_run": true,
            "restored_files": manifest.files.len(),
            "hostname": manifest.hostname,
            "export_timestamp": manifest.export_timestamp,
        }));
    }

    for entry in &manifest.files {
        validate_restore_target(&entry.path, manifest.include_secrets)?;
        let staged = temp_dir.join(&entry.path);
        let destination = PathBuf::from("/").join(&entry.path);
        restore_file(&staged, &destination, entry.size_bytes)?;
    }

    prepare_recovery_access(paths)?;
    restart_supervisor_services()?;

    Ok(serde_json::json!({
        "dry_run": false,
        "restored_files": manifest.files.len(),
        "hostname": manifest.hostname,
        "export_timestamp": manifest.export_timestamp,
    }))
}

fn extract_archive(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let input = fs::File::open(archive_path)
        .map_err(|error| format!("open archive {}: {error}", archive_path.display()))?;
    let decoder = zstd::Decoder::new(input).map_err(|error| error.to_string())?;
    let mut archive = Archive::new(decoder);
    archive
        .unpack(destination)
        .map_err(|error| format!("extract archive: {error}"))?;
    Ok(())
}

fn validate_manifest(
    manifest: &RecoveryManifest,
    temp_dir: &Path,
    archive_path: &Path,
) -> Result<(), String> {
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported recovery manifest schema version {}",
            manifest.schema_version
        ));
    }
    if manifest.files.is_empty() {
        return Err("recovery manifest contains no files".into());
    }

    if !manifest.archive_sha256.is_empty() {
        let (archive_sha256, _) = hash_file(archive_path)?;
        if archive_sha256 != manifest.archive_sha256 {
            return Err("recovery archive checksum does not match manifest".into());
        }
    }

    let mut total_size = 0_u64;
    for entry in &manifest.files {
        validate_restore_target(&entry.path, manifest.include_secrets)?;
        let staged = temp_dir.join(&entry.path);
        if !staged.is_file() {
            return Err(format!("archive is missing file {}", entry.path));
        }
        let (digest, size) = hash_file(&staged)?;
        if digest != entry.sha256 || size != entry.size_bytes {
            return Err(format!(
                "manifest verification failed for {} (size or digest mismatch)",
                entry.path
            ));
        }
        total_size = total_size.saturating_add(size);
        if total_size > MAX_EXPORT_BYTES {
            return Err("recovery archive exceeds maximum supported export size".into());
        }
    }

    Ok(())
}

fn restore_file(staged: &Path, destination: &Path, expected_size: u64) -> Result<(), String> {
    let content = fs::read(staged)
        .map_err(|error| format!("read staged {}: {error}", staged.display()))?;
    if content.len() as u64 != expected_size {
        return Err(format!(
            "staged file {} size {} does not match manifest {}",
            staged.display(),
            content.len(),
            expected_size
        ));
    }
    let destination_text = destination.to_string_lossy();
    let mode = if destination_text.contains("ingest-token") || destination_text.contains("/tls/") {
        0o640
    } else if destination_text.contains("enrollments/") {
        0o664
    } else if destination_text.ends_with("foldops.db") {
        0o640
    } else if destination_text.contains("/config/foldops/") {
        0o640
    } else {
        0o644
    };
    atomic_write(destination, &content, mode)
}

fn restart_supervisor_services() -> Result<(), String> {
    for unit in [
        "foldingos-foldops-supervisor.service",
        "foldingos-provision.service",
        "foldingos-foldops-serve-https.service",
    ] {
        let status = Command::new("systemctl")
            .args(["try-restart", unit])
            .status()
            .map_err(|error| format!("restart {unit}: {error}"))?;
        if !status.success() {
            return Err(format!("systemctl try-restart {unit} failed"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery::export::recovery_export;
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn test_paths(root: &Path) -> AppliancePaths {
        let data_root = root.join("data");
        let config = data_root.join("config/foldops");
        let enrollments = data_root.join("provision/enrollments");
        fs::create_dir_all(&config).unwrap();
        fs::create_dir_all(enrollments.parent().unwrap()).unwrap();
        fs::create_dir_all(&enrollments).unwrap();
        fs::write(config.join("ingest-token"), "token\n").unwrap();
        fs::write(enrollments.join("index.json"), "[]\n").unwrap();
        fs::write(
            data_root.join("config/system.toml"),
            "[identity]\nhostname = \"test-supervisor\"\n",
        )
        .unwrap();
        let db_path = data_root.join("foldops/foldops.db");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("CREATE TABLE test(id INTEGER); INSERT INTO test VALUES (1);")
                .unwrap();
        }
        AppliancePaths {
            config_dir: data_root.join("config"),
            foldops_db: db_path,
            foldops_backups_dir: data_root.join("foldops/backups"),
            foldops_config_dir: config,
            provision_enrollments_dir: enrollments,
            active_installation_role: data_root.join("config/installation-role"),
            data_root,
            ..AppliancePaths::default()
        }
    }

    #[test]
    fn export_and_dry_run_import_round_trip() {
        let temp = TempDir::new().expect("tempdir");
        let paths = test_paths(temp.path());
        fs::create_dir_all(paths.active_installation_role.parent().unwrap()).unwrap();
        fs::write(&paths.active_installation_role, "supervisor\n").unwrap();
        crate::automation_policy::set_test_username(Some("foldingos-admin"));

        let export = recovery_export(&paths, None, false).expect("export");
        let archive = export
            .get("path")
            .and_then(|value| value.as_str())
            .expect("path");
        let import = recovery_import(
            &paths,
            Path::new(archive),
            ImportOptions { dry_run: true },
        )
        .expect("import dry run");
        assert_eq!(import.get("dry_run"), Some(&serde_json::Value::Bool(true)));

        crate::automation_policy::set_test_username(None);
    }
}
