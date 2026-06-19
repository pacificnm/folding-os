use std::fs;
use std::path::{Path, PathBuf};

use crate::foldops::util::file_exists;
use crate::foldops::util::{FOLDOPS_AGENT_ENV, FOLDOPS_SUPERVISOR_ENV};
use crate::paths::AppliancePaths;

const FOLDOPS_GROUP_CONFIG_MODE: u32 = 0o640;
const FOLDOPS_GROUP_DIR_MODE: u32 = 0o750;

pub fn ensure_supervisor_fleet_automation_permissions(
    paths: &AppliancePaths,
) -> Result<(), String> {
    let gid = foldops_group_gid()?;
    ensure_group_directory(&paths.provision_enrollments_dir, 0o2775, gid)
        .map_err(|error| format!("configure enrollment permissions: {error}"))?;
    ensure_group_file(&paths.boot_allowlist, 0o664, gid)
        .map_err(|error| format!("configure boot allowlist permissions: {error}"))?;
    ensure_group_file(&paths.boot_install_disk_allowlist, 0o664, gid)
        .map_err(|error| format!("configure boot install-disk allowlist permissions: {error}"))?;
    ensure_software_assignment_paths_writable(paths)?;
    ensure_foldops_config_group_readable(paths)?;
    ensure_recovery_state_accessible(paths)?;
    ensure_supervisor_database_writable(paths)?;
    ensure_supervisor_registry_writable(paths)?;
    ensure_supervisor_acquire_state_writable(paths)?;
    crate::software_install_log::ensure_ready()?;
    Ok(())
}

/// Agent nodes pull supervisor-assigned software pins into
/// `/data/config/foldops/assigned-manifest.toml` and
/// `/data/config/tools/assigned-version.json` before acquire.
pub fn ensure_agent_software_assignment_permissions(paths: &AppliancePaths) -> Result<(), String> {
    ensure_software_assignment_paths_writable(paths)
}

fn ensure_software_assignment_paths_writable(paths: &AppliancePaths) -> Result<(), String> {
    let gid = foldops_group_gid()?;
    if let Some(parent) = paths.foldops_assigned_manifest.parent() {
        ensure_group_directory(parent, 0o2775, gid).map_err(|error| {
            format!("configure foldops assignment directory permissions: {error}")
        })?;
    }
    ensure_group_file(&paths.foldops_assigned_manifest, 0o664, gid)
        .map_err(|error| format!("configure assigned foldops manifest permissions: {error}"))?;
    if let Some(parent) = paths.tools_assigned_version.parent() {
        ensure_group_directory(parent, 0o2775, gid).map_err(|error| {
            format!("configure tools assignment directory permissions: {error}")
        })?;
    }
    if paths.tools_assigned_version.is_file() {
        ensure_group_file(&paths.tools_assigned_version, 0o664, gid)
            .map_err(|error| format!("configure assigned tools version permissions: {error}"))?;
    }
    Ok(())
}

pub fn ensure_supervisor_acquire_state_writable(paths: &AppliancePaths) -> Result<(), String> {
    let gid = foldops_group_gid()?;
    ensure_group_directory(&paths.foldops_apps_root, 0o2775, gid).map_err(|error| {
        format!(
            "configure {} permissions for foldops acquire: {error}",
            paths.foldops_apps_root.display()
        )
    })?;
    ensure_group_directory(
        paths
            .foldops_provisioned_marker
            .parent()
            .expect("foldops provisioned marker has a parent"),
        0o2775,
        gid,
    )
    .map_err(|error| format!("configure foldops state directory permissions: {error}"))?;
    ensure_group_directory(
        paths
            .tools_active_state
            .parent()
            .expect("tools active state path has a parent"),
        0o2775,
        gid,
    )
    .map_err(|error| format!("configure tools state directory permissions: {error}"))?;
    for path in [
        crate::foldops::util::foldops_acquire_state_path(paths),
        paths
            .tools_active_state
            .parent()
            .expect("tools active state path has a parent")
            .join("acquire.state"),
        paths.foldops_provisioned_marker.clone(),
        paths.tools_active_state.clone(),
    ] {
        if path.is_file() {
            ensure_group_file(&path, 0o664, gid).map_err(|error| {
                format!(
                    "configure {} permissions for acquire state: {error}",
                    path.display()
                )
            })?;
        }
    }
    Ok(())
}

/// Config trees and backup output must be group-readable for recovery export
/// when the foldops service user runs `recovery export` without sudo.
/// Database permissions are handled by [`ensure_supervisor_database_writable`].
pub fn ensure_recovery_state_accessible(paths: &AppliancePaths) -> Result<(), String> {
    let gid = foldops_group_gid()?;
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
    let entries =
        fs::read_dir(root).map_err(|error| format!("read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            ensure_group_directory(&path, FOLDOPS_GROUP_DIR_MODE, gid).map_err(|error| {
                format!(
                    "configure {} permissions for recovery export: {error}",
                    path.display()
                )
            })?;
            ensure_config_tree_group_readable(&path, gid)?;
            continue;
        }
        if path.is_file() {
            ensure_group_file(&path, FOLDOPS_GROUP_CONFIG_MODE, gid).map_err(|error| {
                format!(
                    "configure {} permissions for recovery export: {error}",
                    path.display()
                )
            })?;
        }
    }
    Ok(())
}

pub fn ensure_supervisor_registry_writable(paths: &AppliancePaths) -> Result<(), String> {
    let gid = foldops_group_gid()?;
    if let Some(parent) = paths.foldops_registry_releases_dir.parent() {
        ensure_group_directory(parent, 0o2775, gid).map_err(|error| {
            format!("configure foldops registry directory permissions: {error}")
        })?;
    }
    ensure_group_directory(&paths.foldops_registry_releases_dir, 0o2775, gid)
        .map_err(|error| format!("configure foldops registry releases permissions: {error}"))?;
    ensure_group_registry_index(
        &paths.foldops_registry_index,
        0o664,
        gid,
        FOLDOPS_REGISTRY_INDEX_EMPTY,
    )
    .map_err(|error| format!("configure foldops registry index permissions: {error}"))?;
    if let Some(parent) = paths.tools_registry_releases_dir.parent() {
        ensure_group_directory(parent, 0o2775, gid)
            .map_err(|error| format!("configure tools registry directory permissions: {error}"))?;
    }
    ensure_group_directory(&paths.tools_registry_releases_dir, 0o2775, gid)
        .map_err(|error| format!("configure tools registry releases permissions: {error}"))?;
    ensure_group_registry_index(
        &paths.tools_registry_index,
        0o664,
        gid,
        TOOLS_REGISTRY_INDEX_EMPTY,
    )
    .map_err(|error| format!("configure tools registry index permissions: {error}"))?;
    Ok(())
}

const FOLDOPS_REGISTRY_INDEX_EMPTY: &str = "{\"schema_version\":1,\"releases\":[]}\n";
const TOOLS_REGISTRY_INDEX_EMPTY: &str = "{\"schema_version\":1,\"versions\":[]}\n";

/// The supervisor service runs as the foldops user and must write the SQLite DB
/// (including WAL sidecar files created in journal_mode=WAL).
pub fn ensure_supervisor_database_writable(paths: &AppliancePaths) -> Result<(), String> {
    let gid = foldops_group_gid()?;
    if let Some(parent) = paths.foldops_db.parent() {
        ensure_group_directory(parent, 0o2775, gid).map_err(|error| {
            format!(
                "configure {} permissions for supervisor database access: {error}",
                parent.display()
            )
        })?;
    }
    if !paths.foldops_db.is_file() {
        return Ok(());
    }
    for path in foldops_sqlite_paths(&paths.foldops_db) {
        if path.is_file() {
            ensure_group_file(&path, 0o660, gid).map_err(|error| {
                format!(
                    "configure {} permissions for supervisor database access: {error}",
                    path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn foldops_sqlite_paths(db: &Path) -> [PathBuf; 3] {
    let wal = foldops_sqlite_sidecar(db, "-wal");
    let shm = foldops_sqlite_sidecar(db, "-shm");
    [db.to_path_buf(), wal, shm]
}

fn foldops_sqlite_sidecar(db: &Path, suffix: &str) -> PathBuf {
    let mut path = db.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
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
                format!(
                    "configure {} permissions for recovery export: {error}",
                    path.display()
                )
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
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| error.to_string())?;
    nix::unistd::chown(
        path,
        Some(nix::unistd::Uid::from_raw(0)),
        Some(nix::unistd::Gid::from_raw(gid)),
    )
    .map_err(|error| error.to_string())
}

#[cfg(unix)]
fn ensure_group_registry_index(
    path: &Path,
    mode: u32,
    gid: u32,
    empty_content: &str,
) -> Result<(), String> {
    let needs_seed = match fs::read_to_string(path) {
        Ok(content) => content.trim().is_empty(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error.to_string()),
    };
    if needs_seed {
        fs::write(path, empty_content).map_err(|error| error.to_string())?;
    }
    ensure_group_file(path, mode, gid)
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
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| error.to_string())?;
    nix::unistd::chown(
        path,
        Some(nix::unistd::Uid::from_raw(0)),
        Some(nix::unistd::Gid::from_raw(gid)),
    )
    .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn ensure_group_registry_index(
    _path: &Path,
    _mode: u32,
    _gid: u32,
    _empty_content: &str,
) -> Result<(), String> {
    Err("supervisor fleet permissions require unix".into())
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
