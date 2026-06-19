use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};

use crate::foldops_manifest::{FoldOpsManifest, FoldOpsPackage};
use crate::paths::AppliancePaths;

pub const FOLDOPS_VERIFIED_MARKER: &str = ".foldingos-verified";
pub const FOLDOPS_VERIFICATION_PATH_PREFIX: &str = "/data/apps/foldops/current/";
pub const FOLDOPS_MANIFEST_PLACEHOLDER: &str = "REQUIRED_BEFORE_RELEASE";
pub const FOLDOPS_HTTPS_PORT: u16 = 3443;
pub const FOLDOPS_SUPERVISOR_LOOPBACK_PORT: u16 = 3000;
pub const FOLDOPS_PROVISION_SERVICE: &str = "foldingos-foldops-provision.service";
pub const FOLDOPS_SERVE_HTTPS_SERVICE: &str = "foldingos-foldops-serve-https.service";
pub const FOLDOPS_SUPERVISOR_SERVICE: &str = "foldingos-foldops-supervisor.service";
pub const FOLDOPS_AGENT_SERVICE: &str = "foldingos-foldops-agent.service";
pub const FOLDOPS_SUPERVISOR_ENV: &str = "/data/config/foldops/supervisor.env";
pub const FOLDOPS_AGENT_ENV: &str = "/data/config/foldops/agent.env";
pub const PROVISIONED_FOLDOPS_INGEST_TOKEN: &str =
    "/boot/efi/foldingos/provision/foldops-ingest-token";
pub const OS_RELEASE_PATH: &str = "/usr/lib/os-release";

pub fn foldops_downloads_dir(paths: &AppliancePaths) -> PathBuf {
    paths.foldops_apps_root.join(".downloads")
}

pub fn foldops_acquire_state_path(_paths: &AppliancePaths) -> PathBuf {
    PathBuf::from("/data/state/foldops/acquire.state")
}

pub fn foldops_web_root(paths: &AppliancePaths) -> PathBuf {
    paths
        .foldops_apps_root
        .join("current/foldops-web/usr/share/foldops/web")
}

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

pub fn validate_foldingos_compatibility(minimum_version: &str) -> Result<(), String> {
    let current_version = os_release_value("VERSION_ID")?;
    if current_version != minimum_version {
        return Err(format!(
            "manifest requires FoldingOS {minimum_version} but image reports {current_version}"
        ));
    }
    Ok(())
}

pub fn os_release_value(key: &str) -> Result<String, String> {
    let file =
        fs::File::open(OS_RELEASE_PATH).map_err(|error| format!("read os-release: {error}"))?;
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

pub fn validate_foldops_release_label(release: &str) -> Result<(), String> {
    let release = release.trim();
    if release.is_empty() {
        return Err("release must be non-empty".into());
    }
    let cleaned = Path::new(release);
    if cleaned.components().count() != 1
        || release.contains("..")
        || release.contains('/')
        || release.contains('\\')
    {
        return Err("release must not contain path separators or traversal".into());
    }
    Ok(())
}

pub fn foldops_staged_artifact_path(
    paths: &AppliancePaths,
    artifact_format: &str,
    pkg: &FoldOpsPackage,
) -> PathBuf {
    let suffix = if artifact_format == "layout-tar-zst" {
        ".tar.zst"
    } else {
        ".deb"
    };
    foldops_downloads_dir(paths).join(format!("{}_{}{suffix}", pkg.name, pkg.version))
}

pub fn embedded_foldops_bundle_path(
    paths: &AppliancePaths,
    manifest_release: &str,
    architecture: &str,
    pkg: &FoldOpsPackage,
) -> PathBuf {
    paths
        .foldops_embedded_cache_root
        .join(manifest_release)
        .join(format!("{}-{}.tar.zst", pkg.name, architecture))
}

pub fn embedded_bootstrap_cache_available(
    paths: &AppliancePaths,
    manifest_release: &str,
    artifact_format: &str,
    architecture: &str,
    packages: &[FoldOpsPackage],
) -> bool {
    if artifact_format != "layout-tar-zst" {
        return false;
    }
    if validate_foldops_release_label(manifest_release).is_err() {
        return false;
    }
    packages.iter().all(|pkg| {
        let path = embedded_foldops_bundle_path(paths, manifest_release, architecture, pkg);
        path.is_file() && crate::foldops::extract::verify_foldops_artifact_file(&path, pkg).is_ok()
    })
}

pub fn file_exists(path: &Path) -> bool {
    fs::metadata(path)
        .map(|info| !info.is_dir())
        .unwrap_or(false)
}

pub fn remove_tree(path: &Path) -> Result<(), String> {
    fs::remove_dir_all(path).map_err(|error| format!("remove {}: {error}", path.display()))
}

pub fn load_foldops_manifest_from_allowed_path(
    paths: &AppliancePaths,
    path: &Path,
) -> Result<FoldOpsManifest, String> {
    if path != paths.foldops_embedded_manifest && path != paths.foldops_assigned_manifest {
        return Err("manifest path is not allowed".into());
    }
    let content = fs::read_to_string(path).map_err(|error| format!("read manifest: {error}"))?;
    if content.contains(FOLDOPS_MANIFEST_PLACEHOLDER) {
        return Err(format!(
            "manifest contains unresolved placeholder \"{FOLDOPS_MANIFEST_PLACEHOLDER}\""
        ));
    }
    let manifest = crate::foldops_manifest::parse_foldops_manifest(&content)?;
    crate::foldops_manifest::validate_foldops_manifest(&manifest)?;
    Ok(manifest)
}

pub fn resolve_effective_foldops_manifest(
    paths: &AppliancePaths,
) -> Result<FoldOpsManifest, String> {
    if assigned_foldops_manifest_present(&paths.foldops_assigned_manifest) {
        load_foldops_manifest_from_allowed_path(paths, &paths.foldops_assigned_manifest)
            .map_err(|error| format!("assigned manifest: {error}"))
    } else {
        load_foldops_manifest_from_allowed_path(paths, &paths.foldops_embedded_manifest)
    }
}

fn assigned_foldops_manifest_present(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

pub fn foldops_supervisor_host_from_url(raw_url: &str) -> Result<String, String> {
    let raw_url = raw_url.trim();
    if raw_url.is_empty() {
        return Err("supervisor URL is empty".into());
    }
    let parsed =
        url::Url::parse(raw_url).map_err(|error| format!("supervisor URL is invalid: {error}"))?;
    let host = parsed.host_str().unwrap_or_default();
    if host.is_empty() {
        return Err("supervisor URL host is empty".into());
    }
    Ok(host.to_string())
}

pub(crate) fn clean_path(path: &Path) -> PathBuf {
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

pub(crate) fn path_with_trailing_sep(path: &Path) -> PathBuf {
    let mut out = path.to_path_buf();
    if !out.as_os_str().is_empty() {
        out.push("");
    }
    out
}

pub(crate) fn path_within_root(root: &Path, candidate: &Path) -> bool {
    let root_clean = clean_path(root);
    let candidate_clean = clean_path(candidate);
    candidate_clean == root_clean
        || candidate_clean.starts_with(&path_with_trailing_sep(&root_clean))
}
