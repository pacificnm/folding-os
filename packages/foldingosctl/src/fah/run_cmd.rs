use std::fs;
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::paths::AppliancePaths;

use super::manifest::{load_fah_manifest, validate_foldingos_compatibility};
use super::util::{fah_executable_for_version, read_fah_current_version};
use super::verify_install::{fah_installation_verified, verify_fah_installed_version};

pub fn fah_run(paths: &AppliancePaths) -> Result<(), String> {
    let manifest = load_fah_manifest(paths, &paths.fah_embedded_manifest)?;
    validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;

    let active_version = read_fah_current_version(paths)
        .map_err(|error| format!("no active Folding@home installation: {error}"))?;
    if !fah_installation_verified(paths, &active_version, &manifest) {
        return Err("active Folding@home installation is not verified".into());
    }
    verify_fah_installed_version(paths, &active_version, &manifest)?;
    if let Err(error) = fs::metadata(&paths.fah_runtime_config) {
        return Err(format!("runtime configuration is missing: {error}"));
    }

    let executable = fah_executable_for_version(paths, &active_version, &manifest.executable_path)?;
    verify_fah_executable_under_resolved_current(paths, &executable)?;

    let mut command = Command::new(&executable);
    command.args(&manifest.arguments);
    Err(command.exec().to_string())
}

fn verify_fah_executable_under_resolved_current(
    paths: &AppliancePaths,
    executable: &std::path::Path,
) -> Result<(), String> {
    let current_path = paths.fah_current_link();
    let resolved_current = fs::canonicalize(&current_path)
        .map_err(|error| format!("resolve current symlink: {error}"))?;

    let executable_clean = clean_path(executable);
    let resolved_current_clean = clean_path(&resolved_current);
    if executable_clean != resolved_current_clean
        && !executable_clean.starts_with(&path_with_trailing_sep(&resolved_current_clean))
    {
        return Err("resolved executable escapes verified active installation".into());
    }
    Ok(())
}

fn clean_path(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

fn path_with_trailing_sep(path: &std::path::Path) -> std::path::PathBuf {
    let mut out = path.to_path_buf();
    if !out.as_os_str().is_empty() {
        out.push("");
    }
    out
}
