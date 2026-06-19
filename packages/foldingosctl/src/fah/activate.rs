use std::fs::{self, OpenOptions};
use std::os::unix::fs::symlink;

use crate::paths::AppliancePaths;
use crate::process::{command_output, run_command};

use super::manifest::{
    load_fah_manifest, validate_fah_version_label, validate_foldingos_compatibility,
};
use super::util::{read_fah_current_version, FAH_SERVICE_NAME};
use super::verify_install::{fah_installation_verified, verify_fah_installed_version};

pub fn fah_activate(paths: &AppliancePaths, version: &str) -> Result<(), String> {
    validate_fah_version_label(version)?;
    let manifest = load_fah_manifest(paths, &paths.fah_embedded_manifest)?;
    validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;
    if version != manifest.client_version {
        return Err(format!(
            "version {version} does not match approved manifest client {}",
            manifest.client_version
        ));
    }
    verify_fah_installed_version(paths, version, &manifest)?;
    if !fah_installation_verified(paths, version, &manifest) {
        return Err(format!("version {version} is not a verified installation"));
    }

    if let Ok(current_version) = read_fah_current_version(paths) {
        if current_version == version {
            println!("Folding@home {version} is already active.");
            return restart_fah_service_after_activation();
        }
    }

    activate_fah_current_symlink(paths, version)?;
    println!(
        "Activated Folding@home {version} at {}.",
        paths.fah_current_link().display()
    );
    restart_fah_service_after_activation()
}

pub(crate) fn activate_fah_current_symlink(
    paths: &AppliancePaths,
    version: &str,
) -> Result<(), String> {
    if version.contains(std::path::MAIN_SEPARATOR) {
        return Err("activation version must not contain path separators".into());
    }
    let version_dir = paths.fah_version_dir(version);
    let metadata = fs::metadata(&version_dir)
        .map_err(|error| format!("verified version directory is missing: {error}"))?;
    if !metadata.is_dir() {
        return Err("activation target is not a directory".into());
    }

    let current_path = paths.fah_current_link();
    let temp_path = paths.fah_apps_root.join(".current.tmp-activate");
    if temp_path.exists() {
        fs::remove_file(&temp_path)
            .map_err(|error| format!("remove stale activation symlink: {error}"))?;
    }
    symlink(version, &temp_path).map_err(|error| format!("create activation symlink: {error}"))?;
    if let Err(error) = fs::rename(&temp_path, &current_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("activate current symlink: {error}"));
    }

    let dir = OpenOptions::new()
        .read(true)
        .open(&paths.fah_apps_root)
        .map_err(|error| format!("open apps root for sync: {error}"))?;
    dir.sync_all()
        .map_err(|error| format!("sync apps root: {error}"))?;
    Ok(())
}

fn restart_fah_service_after_activation() -> Result<(), String> {
    let state = command_output(
        "systemctl",
        &["show", "-p", "LoadState", "--value", FAH_SERVICE_NAME],
    )
    .map_err(|error| format!("inspect {FAH_SERVICE_NAME}: {error}"))?;
    if state.trim() != "loaded" {
        return Ok(());
    }
    run_command("systemctl", &["try-restart", FAH_SERVICE_NAME])
        .map_err(|error| format!("restart {FAH_SERVICE_NAME}: {error}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use super::*;
    use crate::paths::AppliancePaths;

    fn test_paths(root: &std::path::Path) -> AppliancePaths {
        let mut paths = AppliancePaths::default();
        paths.fah_apps_root = root.to_path_buf();
        paths
    }

    #[test]
    fn activate_fah_current_symlink_sets_relative_target() {
        let root = std::env::temp_dir().join(format!("fah-activate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("8.5.6")).expect("mkdir");
        let paths = test_paths(&root);

        activate_fah_current_symlink(&paths, "8.5.6").expect("activate");

        let target = fs::read_link(paths.fah_current_link()).expect("readlink");
        assert_eq!(target.to_string_lossy(), "8.5.6");
        assert!(!target.is_absolute());
    }

    #[test]
    fn activate_fah_preserves_previous_current_on_failure() {
        let root = std::env::temp_dir().join(format!("fah-activate-fail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("8.5.6")).expect("mkdir");
        symlink("8.5.6", root.join("current")).expect("symlink");
        let paths = test_paths(&root);

        assert!(activate_fah_current_symlink(&paths, "9.9.9").is_err());
        let target = fs::read_link(paths.fah_current_link()).expect("readlink");
        assert_eq!(target.to_string_lossy(), "8.5.6");
    }

    #[test]
    fn activate_fah_replaces_existing_current_atomically() {
        let root =
            std::env::temp_dir().join(format!("fah-activate-replace-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for version in ["8.5.5", "8.5.6"] {
            fs::create_dir_all(root.join(version)).expect("mkdir");
        }
        symlink("8.5.5", root.join("current")).expect("symlink");
        let paths = test_paths(&root);

        activate_fah_current_symlink(&paths, "8.5.6").expect("activate");
        let target = fs::read_link(paths.fah_current_link()).expect("readlink");
        assert_eq!(target.to_string_lossy(), "8.5.6");
        assert!(root.join("8.5.5").is_dir());
    }
}
