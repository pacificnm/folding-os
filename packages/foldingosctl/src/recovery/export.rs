use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::automation_policy::require_supervisor_automation_mutation;
use crate::config_host::read_hostname;
use crate::paths::AppliancePaths;
use crate::recovery::bundle::{
    collect_export_files, export_timestamp_compact, export_timestamp_rfc3339_utc, hash_file,
    read_os_release_version, resolve_export_source, sanitize_hostname_for_filename,
    write_tar_zst_archive, BundleFileEntry, RecoveryManifest, BACKUP_RETENTION,
    MANIFEST_SCHEMA_VERSION, MAX_EXPORT_BYTES,
};
use crate::role::require_supervisor_role;

pub fn recovery_export(
    paths: &AppliancePaths,
    output_path: Option<&Path>,
    include_secrets: bool,
) -> Result<serde_json::Value, String> {
    require_supervisor_role(paths)?;
    require_supervisor_automation_mutation(paths, "recovery", "export")?;

    let files = collect_export_files(paths, include_secrets)?;
    let db_backup = backup_foldops_db(paths)?;

    let unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs() as i64;
    let hostname = read_hostname(paths)?;
    let export_timestamp = export_timestamp_rfc3339_utc(unix);

    let mut manifest_files = Vec::with_capacity(files.len());
    let mut total_bytes = 0_u64;
    for file in &files {
        let source = resolve_export_source(file, db_backup.as_deref())?;
        let (sha256, size_bytes) = hash_file(source)?;
        total_bytes = total_bytes.saturating_add(size_bytes);
        if total_bytes > MAX_EXPORT_BYTES {
            cleanup_db_backup(db_backup.as_deref());
            return Err(format!(
                "recovery export exceeds maximum supported size of {MAX_EXPORT_BYTES} bytes"
            ));
        }
        manifest_files.push(BundleFileEntry {
            path: file.archive_path.clone(),
            sha256,
            size_bytes,
        });
    }

    let manifest = RecoveryManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        export_timestamp: export_timestamp.clone(),
        hostname: hostname.clone(),
        foldingos_version: read_os_release_version(),
        include_secrets,
        files: manifest_files,
        archive_sha256: String::new(),
    };

    fs::create_dir_all(&paths.foldops_backups_dir)
        .map_err(|error| format!("create backups directory: {error}"))?;

    let filename = format!(
        "foldingos-supervisor-backup-{}-{}.tar.zst",
        sanitize_hostname_for_filename(&hostname),
        export_timestamp_compact(unix)
    );
    let output = output_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| paths.foldops_backups_dir.join(&filename));

    if let Err(error) = write_tar_zst_archive(&output, &manifest, &files, db_backup.as_deref()) {
        cleanup_db_backup(db_backup.as_deref());
        let _ = fs::remove_file(&output);
        return Err(error);
    }
    cleanup_db_backup(db_backup.as_deref());

    let (sha256, size_bytes) = hash_file(&output).map_err(|error| {
        let _ = fs::remove_file(&output);
        error
    })?;

    if output.starts_with(&paths.foldops_backups_dir) {
        if let Err(error) = prune_old_backups(&paths.foldops_backups_dir) {
            let _ = fs::remove_file(&output);
            return Err(error);
        }
    }

    Ok(serde_json::json!({
        "ok": true,
        "path": output.display().to_string(),
        "sha256": sha256,
        "size_bytes": size_bytes,
        "hostname": hostname,
        "export_timestamp": export_timestamp,
        "include_secrets": include_secrets,
        "file_count": manifest.files.len(),
        "download_url": "/api/recovery/export/latest",
    }))
}

fn backup_foldops_db(paths: &AppliancePaths) -> Result<Option<PathBuf>, String> {
    if !paths.foldops_db.is_file() {
        return Ok(None);
    }
    let destination =
        std::env::temp_dir().join(format!("foldingos-recovery-db-{}", std::process::id()));
    if destination.exists() {
        fs::remove_file(&destination).map_err(|error| error.to_string())?;
    }
    let source = Connection::open(&paths.foldops_db)
        .map_err(|error| format!("open foldops database: {error}"))?;
    let mut destination_conn = Connection::open(&destination)
        .map_err(|error| format!("create foldops database backup: {error}"))?;
    let backup = rusqlite::backup::Backup::new(&source, &mut destination_conn)
        .map_err(|error| format!("start foldops database backup: {error}"))?;
    backup
        .run_to_completion(100, std::time::Duration::from_millis(250), None)
        .map_err(|error| format!("complete foldops database backup: {error}"))?;
    Ok(Some(destination))
}

fn cleanup_db_backup(path: Option<&Path>) {
    if let Some(path) = path {
        let _ = fs::remove_file(path);
    }
}

fn prune_old_backups(backups_dir: &Path) -> Result<(), String> {
    let mut archives = fs::read_dir(backups_dir)
        .map_err(|error| format!("read backups directory: {error}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "zst")
        })
        .collect::<Vec<_>>();
    archives.sort_by_key(|entry| {
        entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
    });
    while archives.len() > BACKUP_RETENTION {
        let oldest = archives.remove(0);
        fs::remove_file(oldest.path())
            .map_err(|error| format!("remove old backup {}: {error}", oldest.path().display()))?;
    }
    Ok(())
}
