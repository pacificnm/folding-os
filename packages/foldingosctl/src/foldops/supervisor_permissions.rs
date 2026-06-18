use std::fs;
use std::path::Path;

use crate::foldops::util::{FOLDOPS_AGENT_ENV, FOLDOPS_SUPERVISOR_ENV};
use crate::foldops::util::file_exists;
use crate::paths::AppliancePaths;

const FOLDOPS_GROUP_CONFIG_MODE: u32 = 0o640;
const FOLDOPS_GROUP_DIR_MODE: u32 = 0o750;

pub fn ensure_supervisor_fleet_automation_permissions(paths: &AppliancePaths) -> Result<(), String> {
    let gid = foldops_group_gid()?;
    ensure_group_directory(&paths.provision_enrollments_dir, 0o2775, gid)
        .map_err(|error| format!("configure enrollment permissions: {error}"))?;
    ensure_group_file(&paths.boot_allowlist, 0o664, gid)
        .map_err(|error| format!("configure boot allowlist permissions: {error}"))?;
    ensure_group_file(&paths.boot_install_disk_allowlist, 0o664, gid)
        .map_err(|error| format!("configure boot install-disk allowlist permissions: {error}"))?;
    if let Some(parent) = paths.foldops_assigned_manifest.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("ensure foldops config directory: {error}"))?;
    }
    ensure_group_file(&paths.foldops_assigned_manifest, 0o664, gid)
        .map_err(|error| format!("configure assigned foldops manifest permissions: {error}"))?;
    if let Some(parent) = paths.tools_assigned_version.parent() {
        ensure_group_directory(parent, 0o2775, gid)
            .map_err(|error| format!("configure tools assignment permissions: {error}"))?;
    }
    ensure_foldops_config_group_readable(paths)?;
    ensure_recovery_state_accessible(paths)?;
    Ok(())
}

/// Database, backup output, and config trees must be readable when recovery
/// export runs without sudo delegation (e.g. operator CLI as root).
pub fn ensure_recovery_state_accessible(paths: &AppliancePaths) -> Result<(), String> {
    let gid = foldops_group_gid()?;
    if paths.foldops_db.is_file() {
        ensure_group_file(&paths.foldops_db, FOLDOPS_GROUP_CONFIG_MODE, gid).map_err(|error| {
            format!(
                "configure {} permissions for recovery export: {error}",
                paths.foldops_db.display()
            )
        })?;
    }
    ensure_group_directory(&paths.foldops_backups_dir, 0o2775, gid).map_err(|error| {
        format!(
            "configure {} permissions for recovery export: {error}",
            paths.foldops_backups_dir.display()
        )
    })?;
    ensure_config_tree_group_readable(&paths.foldops_config_dir, gid)?;
    Ok(())
}

fn ensure_config_tree_group_readable(root: &Path, gid: u32) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(root).map_err(|error| format!("read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            ensure_group_directory(&path, FOLDOPS_GROUP_DIR_MODE, gid).map_err(|error| {
                format!("configure {} permissions for recovery export: {error}", path.display())
            })?;
            ensure_config_tree_group_readable(&path, gid)?;
            continue;
        }
        if path.is_file() {
            ensure_group_file(&path, FOLDOPS_GROUP_CONFIG_MODE, gid).map_err(|error| {
                format!("configure {} permissions for recovery export: {error}", path.display())
            })?;
        }
    }
    Ok(())
}

/// FoldOps env files and ingest token must be readable by the foldops service user
/// so delegated recovery export/import can include `/data/config/foldops/`.
pub fn ensure_foldops_config_group_readable(paths: &AppliancePaths) -> Result<(), String> {
    let gid = foldops_group_gid()?;
    for path in [
        Path::new(FOLDOPS_SUPERVISOR_ENV),
        Path::new(FOLDOPS_AGENT_ENV),
        paths.foldops_ingest_token.as_path(),
    ] {
        if path.is_file() {
            ensure_group_file(path, FOLDOPS_GROUP_CONFIG_MODE, gid).map_err(|error| {
                format!("configure {} permissions for recovery export: {error}", path.display())
            })?;
        }
    }
    Ok(())
}

fn foldops_group_gid() -> Result<u32, String> {
    users::get_group_by_name("foldops")
        .ok_or_else(|| "lookup foldops group: group not found".to_string())
        .map(|group| group.gid())
}

#[cfg(unix)]
fn ensure_group_directory(path: &Path, mode: u32, gid: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(path).map_err(|error| error.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| error.to_string())?;
    nix::unistd::chown(path, Some(nix::unistd::Uid::from_raw(0)), Some(nix::unistd::Gid::from_raw(gid)))
        .map_err(|error| error.to_string())
}

#[cfg(unix)]
fn ensure_group_file(path: &Path, mode: u32, gid: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|error| error.to_string())?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| error.to_string())?;
    nix::unistd::chown(path, Some(nix::unistd::Uid::from_raw(0)), Some(nix::unistd::Gid::from_raw(gid)))
        .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn ensure_group_directory(_path: &Path, _mode: u32, _gid: u32) -> Result<(), String> {
    Err("supervisor fleet permissions require unix".into())
}

#[cfg(not(unix))]
fn ensure_group_file(_path: &Path, _mode: u32, _gid: u32) -> Result<(), String> {
    Err("supervisor fleet permissions require unix".into())
}

#[allow(dead_code)]
pub fn foldops_tls_files_exist(paths: &AppliancePaths) -> bool {
    file_exists(&paths.foldops_tls_dir.join("cert.pem"))
        && file_exists(&paths.foldops_tls_dir.join("key.pem"))
        && file_exists(&paths.foldops_tls_dir.join("ca.pem"))
}
