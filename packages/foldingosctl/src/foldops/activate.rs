use std::fs;
use std::path::{Path, PathBuf};

use crate::foldops::util::{
    path_within_root, FOLDOPS_VERIFICATION_PATH_PREFIX, validate_foldops_release_label,
};
use crate::foldops_manifest::FoldOpsPackage;
use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;

pub fn foldops_activate(paths: &AppliancePaths, release: &str) -> Result<(), String> {
    validate_foldops_release_label(release)?;
    let manifest = crate::foldops::util::resolve_effective_foldops_manifest(paths)?;
    crate::foldops::util::validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;
    if release != manifest.manifest_release {
        return Err(format!(
            "release {release} does not match approved manifest release {}",
            manifest.manifest_release
        ));
    }
    let role = crate::role::read_active_installation_role(paths)?;
    let packages = crate::foldops_manifest::foldops_packages_for_role(&manifest, &role)?;
    if !super::verify::foldops_installation_verified(paths, release, &role, &packages)? {
        return Err(format!("release {release} is not a verified installation"));
    }

    if let Ok(current_release) = read_foldops_current_release(paths) {
        if current_release == release {
            crate::automation::say_stdout(format!(
                "FoldOps release {release} is already active."
            ));
            return Ok(());
        }
    }

    activate_foldops_current_symlink(paths, release)?;
    crate::automation::say_stdout(format!(
        "Activated FoldOps release {release} at {}.",
        paths.foldops_current_link().display()
    ));
    Ok(())
}

pub fn read_foldops_current_release(paths: &AppliancePaths) -> Result<String, String> {
    let current_path = paths.foldops_current_link();
    let target = fs::read_link(&current_path).map_err(|error| error.to_string())?;
    let target = target.to_string_lossy();
    if target.starts_with('/') {
        return Err("current must be a relative symlink".into());
    }
    let cleaned = Path::new(target.as_ref())
        .components()
        .fold(PathBuf::new(), |mut acc, component| {
            use std::path::Component;
            match component {
                Component::Normal(part) => acc.push(part),
                Component::ParentDir => {
                    acc.pop();
                }
                _ => {}
            }
            acc
        });
    if cleaned.to_string_lossy() != target || target.contains("..") {
        return Err("current must not contain path traversal".into());
    }
    let release_dir = paths.foldops_apps_root.join(target.as_ref());
    let metadata = fs::metadata(&release_dir)
        .map_err(|_| "current does not reference an installed release".to_string())?;
    if !metadata.is_dir() {
        return Err("current does not reference an installed release".into());
    }
    Ok(target.into_owned())
}

fn activate_foldops_current_symlink(paths: &AppliancePaths, release: &str) -> Result<(), String> {
    if release.contains('/') {
        return Err("activation release must not contain path separators".into());
    }
    let release_dir = paths.foldops_apps_root.join(release);
    let metadata = fs::metadata(&release_dir)
        .map_err(|error| format!("verified release directory is missing: {error}"))?;
    if !metadata.is_dir() {
        return Err("activation target is not a directory".into());
    }

    let current_path = paths.foldops_current_link();
    let temp_path = paths.foldops_apps_root.join(".current.tmp-activate");
    if temp_path.exists() {
        fs::remove_file(&temp_path)
            .map_err(|error| format!("remove stale activation symlink: {error}"))?;
    }
    std::os::unix::fs::symlink(release, &temp_path)
        .map_err(|error| format!("create activation symlink: {error}"))?;
    if let Err(error) = fs::rename(&temp_path, &current_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("activate current symlink: {error}"));
    }

    let dir = fs::File::open(&paths.foldops_apps_root)
        .map_err(|error| format!("open apps root for sync: {error}"))?;
    dir.sync_all()
        .map_err(|error| format!("sync apps root: {error}"))?;
    Ok(())
}

pub fn write_foldops_verified_marker(
    root: &Path,
    release: &str,
    role: &str,
    packages: &[FoldOpsPackage],
) -> Result<(), String> {
    let mut lines = vec![
        format!("manifest_release={release}"),
        format!("installation_role={role}"),
    ];
    for pkg in packages {
        lines.push(format!("package_{}_sha256={}", pkg.name, pkg.sha256));
    }
    let marker_path = root.join(crate::foldops::util::FOLDOPS_VERIFIED_MARKER);
    let mut content = lines.join("\n");
    content.push('\n');
    atomic_write(&marker_path, content.as_bytes(), 0o644)
}

pub fn foldops_verification_target_at_root(
    release_root: &Path,
    verification_path: &str,
) -> Result<PathBuf, String> {
    if !verification_path.starts_with(FOLDOPS_VERIFICATION_PATH_PREFIX) {
        return Err("manifest verification_path is invalid".into());
    }
    let relative = verification_path
        .trim_start_matches(FOLDOPS_VERIFICATION_PATH_PREFIX)
        .trim_start_matches('/');
    if relative.is_empty() || relative.contains("..") {
        return Err("manifest verification_path is invalid".into());
    }
    let target = release_root.join(relative);
    if !path_within_root(release_root, &target) {
        return Err("resolved verification path escapes release directory".into());
    }
    Ok(target)
}
