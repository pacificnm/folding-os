use std::fs;
use std::path::{Component, Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use url::Url;

use crate::paths::AppliancePaths;

pub const TOOLS_APPROVED_ORIGIN: &str = "packages.folding-os.com";
pub const TOOLS_ARTIFACT_BASENAME: &str = "foldingosctl-x86_64";
const TOOLS_ASSIGNMENT_SCHEMA_VERSION: i32 = 1;

static SHA256_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").expect("sha256 pattern compiles"));

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct ToolsAssignment {
    pub schema_version: i32,
    pub tools_version: String,
    pub artifact_url: String,
    pub artifact_size: i64,
    pub sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolsActiveState {
    pub schema_version: i32,
    pub tools_version: String,
    pub sha256: String,
    #[serde(default)]
    pub installed_at_unix: i64,
}

pub fn inspect_tools(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    Ok(collect_inspect_tools_data(paths)?)
}

pub fn collect_inspect_tools_data(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let mut binary = serde_json::json!({
        "path": paths.tools_binary.to_string_lossy(),
    });
    if let Ok(metadata) = fs::metadata(&paths.tools_binary) {
        binary["size_bytes"] = serde_json::json!(metadata.len());
        binary["mod_time_unix"] = serde_json::json!(metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0));
    }

    let mut data = serde_json::json!({
        "verified": false,
        "binary": binary,
    });

    if let Ok(assignment) = load_tools_assignment(&paths.tools_bootstrap_manifest) {
        data["bootstrap_tools_version"] = serde_json::Value::String(assignment.tools_version);
    } else if let Err(error) = load_tools_assignment(&paths.tools_bootstrap_manifest) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(format!("bootstrap tools manifest: {error}"));
        }
    }

    if paths.tools_assigned_version.exists() {
        let assignment = load_tools_assignment(&paths.tools_assigned_version)
            .map_err(|error| format!("assigned tools manifest: {error}"))?;
        data["assigned_tools_version"] = serde_json::Value::String(assignment.tools_version);
    }

    if let Ok(state) = load_tools_active_state(&paths.tools_active_state) {
        if !state.tools_version.is_empty() {
            data["active_tools_version"] = serde_json::Value::String(state.tools_version);
        }
    }

    if let Some(assignment) = resolve_effective_tools_assignment(paths)? {
        data["effective_tools_version"] = serde_json::Value::String(assignment.tools_version.clone());
        data["verified"] = serde_json::json!(tools_installation_verified(paths, &assignment));
    }

    Ok(data)
}

pub fn resolve_effective_tools_assignment(
    paths: &AppliancePaths,
) -> Result<Option<ToolsAssignment>, String> {
    match load_tools_assignment(&paths.tools_assigned_version) {
        Ok(assignment) => return Ok(Some(assignment)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    match load_tools_assignment(&paths.tools_bootstrap_manifest) {
        Ok(assignment) => Ok(Some(assignment)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("bootstrap tools manifest: {error}")),
    }
}

pub fn parse_tools_assignment(content: &[u8]) -> Result<ToolsAssignment, String> {
    serde_json::from_slice(content).map_err(|error| format!("parse tools assignment JSON: {error}"))
}

pub fn load_tools_assignment(path: &Path) -> Result<ToolsAssignment, std::io::Error> {
    let content = fs::read(path)?;
    let assignment = parse_tools_assignment(&content)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    validate_tools_assignment(&assignment)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(assignment)
}

pub fn load_tools_active_state(path: &Path) -> Result<ToolsActiveState, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let state: ToolsActiveState = serde_json::from_str(&content)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if state.schema_version != TOOLS_ASSIGNMENT_SCHEMA_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "active tools state schema_version is unsupported",
        ));
    }
    validate_tools_version_label(&state.tools_version)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if !SHA256_PATTERN.is_match(&state.sha256) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "active tools state sha256 is invalid",
        ));
    }
    Ok(state)
}

pub fn save_tools_active_state(path: &Path, mut state: ToolsActiveState) -> Result<(), String> {
    state.schema_version = TOOLS_ASSIGNMENT_SCHEMA_VERSION;
    let content = serde_json::to_vec(&state).map_err(|error| error.to_string())?;
    let mut content_with_newline = content;
    content_with_newline.push(b'\n');
    crate::fs_atomic::atomic_write(path, &content_with_newline, 0o644)
}

fn validate_tools_version_label(version: &str) -> Result<(), String> {
    let version = version.trim();
    if version.is_empty() {
        return Err("tools version must be non-empty".into());
    }
    let cleaned = PathBuf::from(version);
    if cleaned.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) || version.contains('/') || version.contains('\\') {
        return Err("tools version must not contain path separators or traversal".into());
    }
    if cleaned != Path::new(version) {
        return Err("tools version must not contain path separators or traversal".into());
    }
    Ok(())
}

fn validate_tools_assignment(assignment: &ToolsAssignment) -> Result<(), String> {
    if assignment.schema_version != TOOLS_ASSIGNMENT_SCHEMA_VERSION {
        return Err("tools assignment schema_version must be 1".into());
    }
    validate_tools_version_label(&assignment.tools_version)?;
    if !SHA256_PATTERN.is_match(&assignment.sha256) {
        return Err("sha256 must be a 64-character lowercase hex digest".into());
    }
    if assignment.artifact_size <= 0 {
        return Err("artifact_size must be positive".into());
    }

    let artifact_url = Url::parse(&assignment.artifact_url)
        .map_err(|error| format!("artifact_url is invalid: {error}"))?;
    if artifact_url.scheme() != "https" {
        return Err("artifact_url must use HTTPS".into());
    }
    if artifact_url.host_str() != Some(TOOLS_APPROVED_ORIGIN) {
        return Err(format!(
            "artifact_url must use HTTPS from the approved official origin: {TOOLS_APPROVED_ORIGIN}"
        ));
    }
    let path = artifact_url.path();
    let version_dir = format!("/foldingos-tools/{}/", assignment.tools_version);
    if !path.contains(&version_dir) {
        return Err("artifact_url must reference the assigned tools version directory".into());
    }
    if !path.ends_with(&format!("/{TOOLS_ARTIFACT_BASENAME}"))
        && !path.ends_with(TOOLS_ARTIFACT_BASENAME)
    {
        return Err("artifact_url must reference the foldingosctl-x86_64 artifact".into());
    }
    Ok(())
}

pub fn tools_installation_verified(paths: &AppliancePaths, assignment: &ToolsAssignment) -> bool {
    let Ok(state) = load_tools_active_state(&paths.tools_active_state) else {
        return false;
    };
    if state.tools_version != assignment.tools_version || state.sha256 != assignment.sha256 {
        return false;
    }
    hash_file_at_path(&paths.tools_binary, assignment.artifact_size)
        .ok()
        .is_some_and(|digest| digest == assignment.sha256)
}

pub fn validate_tools_assignment_public(assignment: &ToolsAssignment) -> Result<(), String> {
    validate_tools_assignment(assignment)
}

pub fn hash_file_at_path(path: &std::path::Path, expected_size: i64) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    let mut written = 0_i64;
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)
            .map_err(|error| format!("hash artifact: {error}"))?;
        if read == 0 {
            break;
        }
        written += read as i64;
        if written > expected_size {
            return Err(format!(
                "artifact size {written} does not match expected size {expected_size}"
            ));
        }
        hasher.update(&buffer[..read]);
    }
    if written != expected_size {
        return Err(format!(
            "artifact size {written} does not match expected size {expected_size}"
        ));
    }
    Ok(format!("{:x}", hasher.finalize()))
}
