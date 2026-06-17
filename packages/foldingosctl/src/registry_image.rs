use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::fs_atomic::{atomic_write, contains_string};
use crate::paths::AppliancePaths;

pub const RELEASE_IMAGE_SIZE_BYTES: i64 = 4_294_967_296;

const VALID_ROLLOUT_STATES: &[&str] = &["staged", "ready", "retired"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub schema_version: i32,
    pub foldingos_version: String,
    pub git_revision: String,
    pub image_sha256: String,
    pub image_size_bytes: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub retrieval_url: String,
    pub verification_method: String,
    pub import_timestamp: String,
    pub rollout_state: String,
    pub local_image_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryIndex {
    pub schema_version: i32,
    pub versions: Vec<String>,
}

pub fn list_registry(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let index = load_registry_index(paths)?;
    if index.versions.is_empty() {
        return Ok(serde_json::json!({ "versions": [] }));
    }
    let mut versions = index.versions;
    versions.sort();
    let mut entries = Vec::with_capacity(versions.len());
    for version in versions {
        entries.push(load_registry_entry(paths, &version)?);
    }
    Ok(serde_json::json!({ "versions": entries }))
}

pub fn show_registry(paths: &AppliancePaths, version: &str) -> Result<serde_json::Value, String> {
    let entry = load_registry_entry(paths, version)?;
    Ok(serde_json::to_value(entry).map_err(|error| error.to_string())?)
}

pub fn load_registry_entry(paths: &AppliancePaths, version: &str) -> Result<RegistryEntry, String> {
    let content = fs::read_to_string(paths.registry_entry_path(version))
        .map_err(|error| format!("read registry entry for {version}: {error}"))?;
    let entry: RegistryEntry = serde_json::from_str(&content)
        .map_err(|error| format!("invalid registry entry for {version}: {error}"))?;
    validate_registry_entry(entry)
}

pub fn load_registry_index(paths: &AppliancePaths) -> Result<RegistryIndex, String> {
    match fs::read_to_string(&paths.registry_index) {
        Ok(content) => {
            let index: RegistryIndex = serde_json::from_str(&content)
                .map_err(|error| format!("invalid registry index: {error}"))?;
            if index.schema_version != 1 {
                return Err(format!(
                    "unsupported registry index schema version {}",
                    index.schema_version
                ));
            }
            Ok(index)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(RegistryIndex {
            schema_version: 1,
            versions: Vec::new(),
        }),
        Err(error) => Err(format!("read registry index: {error}")),
    }
}

fn validate_registry_entry(mut entry: RegistryEntry) -> Result<RegistryEntry, String> {
    if entry.schema_version != 1 {
        return Err(format!(
            "unsupported registry entry schema version {}",
            entry.schema_version
        ));
    }
    entry.foldingos_version = entry.foldingos_version.trim().to_string();
    if entry.foldingos_version.is_empty() {
        return Err("registry entry missing foldingos_version".into());
    }
    entry.git_revision = entry.git_revision.trim().to_string();
    if entry.git_revision.is_empty() {
        return Err("registry entry missing git_revision".into());
    }
    entry.image_sha256 = entry.image_sha256.trim().to_lowercase();
    if entry.image_sha256.len() != 64 {
        return Err("registry entry image_sha256 must be 64 lowercase hex characters".into());
    }
    if entry.image_size_bytes <= 0 {
        return Err("registry entry image_size_bytes must be positive".into());
    }
    entry.rollout_state = entry.rollout_state.trim().to_string();
    if !VALID_ROLLOUT_STATES.contains(&entry.rollout_state.as_str()) {
        return Err(format!("unsupported rollout state \"{}\"", entry.rollout_state));
    }
    entry.local_image_path = entry.local_image_path.trim().to_string();
    if entry.local_image_path.is_empty() {
        return Err("registry entry missing local_image_path".into());
    }
    if entry.verification_method.is_empty() {
        entry.verification_method = "sha256".into();
    }
    if entry.verification_method != "sha256" {
        return Err(format!(
            "unsupported verification method \"{}\"",
            entry.verification_method
        ));
    }
    Ok(entry)
}

pub fn registry_image_path(paths: &AppliancePaths, version: &str) -> PathBuf {
    paths
        .registry_images_dir
        .join(format!("foldingos-x86_64-{version}.img"))
}

pub fn read_embedded_build_revision(paths: &AppliancePaths) -> String {
    match fs::read_to_string(&paths.embedded_build_revision) {
        Ok(content) => {
            let revision = content.trim();
            if revision.is_empty() {
                "unknown".into()
            } else {
                revision.to_string()
            }
        }
        Err(_) => "unknown".into(),
    }
}

pub fn read_upstream_releases_url(paths: &AppliancePaths) -> Result<Option<String>, String> {
    match fs::read_to_string(&paths.upstream_releases_url) {
        Ok(content) => {
            let url = content.trim();
            if url.is_empty() {
                return Ok(None);
            }
            if !url.starts_with("https://") {
                return Err(format!("upstream releases URL must use HTTPS: {url:?}"));
            }
            Ok(Some(url.to_string()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read upstream releases URL: {error}")),
    }
}

pub fn save_registry_entry(paths: &AppliancePaths, mut entry: RegistryEntry) -> Result<(), String> {
    if entry.import_timestamp.is_empty() {
        entry.import_timestamp = current_import_timestamp();
    }
    let validated = validate_registry_entry(entry)?;
    let content = serde_json::to_string_pretty(&validated)
        .map_err(|error| error.to_string())?;
    atomic_write(
        &paths.registry_entry_path(&validated.foldingos_version),
        format!("{content}\n").as_bytes(),
        0o644,
    )?;
    let mut index = load_registry_index(paths)?;
    if !contains_string(&index.versions, &validated.foldingos_version) {
        index.versions.push(validated.foldingos_version);
    }
    save_registry_index(paths, &index)
}

fn save_registry_index(paths: &AppliancePaths, index: &RegistryIndex) -> Result<(), String> {
    let mut index = index.clone();
    index.schema_version = 1;
    index.versions.sort();
    let content = serde_json::to_string_pretty(&index)
        .map_err(|error| error.to_string())?;
    atomic_write(&paths.registry_index, format!("{content}\n").as_bytes(), 0o644)
}

pub fn verify_registry_image_file(
    path: &Path,
    expected_digest: &str,
    expected_size: i64,
) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() as i64 != expected_size {
        return Err(format!(
            "image size {} does not match expected {expected_size}",
            metadata.len()
        ));
    }
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_digest {
        return Err("image SHA-256 does not match registry metadata".into());
    }
    Ok(())
}

pub fn current_import_timestamp() -> String {
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".into())
}

#[derive(Debug, Deserialize)]
pub struct FoldOpsRegistryEntry {
    pub schema_version: i32,
    pub manifest_release: String,
    pub manifest_toml: String,
    pub rollout_state: String,
}

#[derive(Debug, Deserialize)]
pub struct ToolsRegistryEntry {
    pub schema_version: i32,
    pub tools_version: String,
    pub assignment: crate::inspect::ToolsAssignment,
    pub rollout_state: String,
}

pub fn load_foldops_registry_entry(
    paths: &AppliancePaths,
    release: &str,
) -> Result<FoldOpsRegistryEntry, String> {
    let release = release.trim();
    validate_release_label(release)?;
    let content = fs::read_to_string(paths.foldops_registry_entry_path(release))
        .map_err(|error| format!("read foldops registry entry: {error}"))?;
    let entry: FoldOpsRegistryEntry = serde_json::from_str(&content)
        .map_err(|error| format!("invalid foldops manifest registry entry: {error}"))?;
    if entry.schema_version != 1 {
        return Err(format!(
            "unsupported foldops manifest registry schema version {}",
            entry.schema_version
        ));
    }
    if entry.manifest_release.trim() != release {
        return Err(format!(
            "foldops manifest release {:?} does not match registry entry {:?}",
            entry.manifest_release, release
        ));
    }
    if entry.manifest_toml.trim().is_empty() {
        return Err("foldops manifest registry entry is missing manifest_toml".into());
    }
    validate_rollout_state(&entry.rollout_state)?;
    Ok(entry)
}

pub fn load_tools_registry_entry(
    paths: &AppliancePaths,
    version: &str,
) -> Result<ToolsRegistryEntry, String> {
    let version = version.trim();
    validate_tools_version_label(version)?;
    let content = fs::read_to_string(paths.tools_registry_entry_path(version))
        .map_err(|error| format!("read tools registry entry: {error}"))?;
    let entry: ToolsRegistryEntry = serde_json::from_str(&content)
        .map_err(|error| format!("invalid tools version registry entry: {error}"))?;
    if entry.schema_version != 1 {
        return Err(format!(
            "unsupported tools version registry schema version {}",
            entry.schema_version
        ));
    }
    if entry.tools_version.trim() != version {
        return Err(format!(
            "tools assignment version {:?} does not match registry entry {:?}",
            entry.assignment.tools_version, version
        ));
    }
    validate_rollout_state(&entry.rollout_state)?;
    Ok(entry)
}

pub fn validate_rollout_state(state: &str) -> Result<(), String> {
    let state = state.trim();
    if state.is_empty() || VALID_ROLLOUT_STATES.contains(&state) {
        Ok(())
    } else {
        Err(format!("unsupported rollout state \"{state}\""))
    }
}

pub fn is_bootstrap_assignment_label(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "bootstrap" | "current"
    )
}

fn validate_release_label(release: &str) -> Result<(), String> {
    if release.is_empty() || release.contains('/') || release.contains('\\') || release.contains("..") {
        return Err("release must be non-empty and must not contain path separators or traversal".into());
    }
    Ok(())
}

fn validate_tools_version_label(version: &str) -> Result<(), String> {
    if version.is_empty() || version.contains('/') || version.contains('\\') || version.contains("..") {
        return Err("tools version must be non-empty and must not contain path separators or traversal".into());
    }
    Ok(())
}
