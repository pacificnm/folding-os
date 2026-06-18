use std::fs;
use std::path::Path;

use crate::foldops::util::file_exists;
use crate::paths::AppliancePaths;

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
