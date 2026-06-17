use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use crate::paths::AppliancePaths;

pub const FAH_VERIFIED_MARKER_NAME: &str = ".foldingos-verified";
pub const FAH_EXECUTABLE_PATH_PREFIX: &str = "/data/apps/fah/current/";
pub const FAH_SERVICE_NAME: &str = "folding-at-home.service";
pub const FAH_SERVICE_GID: u32 = 200;
pub const OS_RELEASE_PATH: &str = "/usr/lib/os-release";

pub fn parse_key_value_lines(content: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(key.trim().to_string(), value.trim().to_string());
    }
    values
}

pub fn os_release_value(key: &str) -> Result<String, String> {
    let file = fs::File::open(OS_RELEASE_PATH).map_err(|error| error.to_string())?;
    let prefix = format!("{key}=");
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| error.to_string())?;
        let line = line.trim();
        if let Some(value) = line.strip_prefix(&prefix) {
            return Ok(value.trim_matches('"').to_string());
        }
    }
    Ok(String::new())
}

pub fn read_fah_current_version(paths: &AppliancePaths) -> Result<String, String> {
    let current_path = paths.fah_current_link();
    let target = fs::read_link(&current_path).map_err(|error| error.to_string())?;
    let target = target.to_string_lossy();
    if target.starts_with('/') {
        return Err("current must be a relative symlink".into());
    }
    let cleaned = Path::new(target.as_ref())
        .components()
        .fold(String::new(), |mut acc, component| {
            use std::path::Component;
            match component {
                Component::Normal(part) => {
                    if !acc.is_empty() {
                        acc.push('/');
                    }
                    acc.push_str(&part.to_string_lossy());
                }
                Component::ParentDir => acc.clear(),
                _ => {}
            }
            acc
        });
    if cleaned.is_empty() || cleaned.contains("..") || cleaned != target {
        return Err("current must not contain path traversal".into());
    }
    let version_dir = paths.fah_apps_root.join(&cleaned);
    let metadata = fs::metadata(&version_dir)
        .map_err(|_| "current does not reference an installed version".to_string())?;
    if !metadata.is_dir() {
        return Err("current does not reference an installed version".into());
    }
    Ok(cleaned)
}

pub fn fah_executable_in_root(root: &Path, manifest_executable_path: &str) -> Result<PathBuf, String> {
    if !manifest_executable_path.starts_with(FAH_EXECUTABLE_PATH_PREFIX) {
        return Err("manifest executable_path is invalid".into());
    }
    let relative = manifest_executable_path
        .strip_prefix(FAH_EXECUTABLE_PATH_PREFIX)
        .unwrap_or("");
    if relative.is_empty() || relative.contains("..") {
        return Err("manifest executable_path is invalid".into());
    }
    let executable = root.join(relative);
    if !path_within_root(root, &executable) {
        return Err("resolved executable escapes installation directory".into());
    }
    Ok(executable)
}

fn path_within_root(root: &Path, candidate: &Path) -> bool {
    let root_clean = clean_path(root);
    let candidate_clean = clean_path(candidate);
    candidate_clean == root_clean || candidate_clean.starts_with(&path_with_trailing_sep(&root_clean))
}

fn clean_path(path: &Path) -> PathBuf {
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

fn path_with_trailing_sep(path: &Path) -> PathBuf {
    let mut out = path.to_path_buf();
    if !out.as_os_str().is_empty() {
        out.push("");
    }
    out
}

pub fn fah_executable_for_version(
    paths: &AppliancePaths,
    version: &str,
    manifest_executable_path: &str,
) -> Result<PathBuf, String> {
    fah_executable_in_root(&paths.fah_version_dir(version), manifest_executable_path)
}

pub fn require_fah_root_ownership() -> bool {
    nix::unistd::geteuid().is_root()
}

pub fn is_root_owned(metadata: &fs::Metadata) -> bool {
    metadata.uid() == 0 && metadata.gid() == 0
}

pub fn format_go_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if hours > 0 {
        format!("{hours}h{minutes}m{seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m{seconds}s")
    } else {
        format!("{seconds}s")
    }
}

pub fn remove_fah_path(path: &Path) -> Result<(), String> {
    fs::remove_dir_all(path).map_err(|error| format!("remove {}: {error}", path.display()))
}
