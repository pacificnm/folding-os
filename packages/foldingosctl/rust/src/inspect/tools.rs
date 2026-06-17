use std::fs;

use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

use crate::paths::AppliancePaths;

static SHA256_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").expect("sha256 pattern compiles"));

#[derive(Debug, Deserialize)]
struct ToolsAssignment {
    schema_version: i32,
    tools_version: String,
    artifact_url: String,
    artifact_size: i64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct ToolsActiveState {
    schema_version: i32,
    tools_version: String,
    sha256: String,
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

fn resolve_effective_tools_assignment(
    paths: &AppliancePaths,
) -> Result<Option<ToolsAssignment>, String> {
    if paths.tools_assigned_version.exists() {
        return load_tools_assignment(&paths.tools_assigned_version)
            .map(Some)
            .map_err(|error| error.to_string());
    }
    match load_tools_assignment(&paths.tools_bootstrap_manifest) {
        Ok(assignment) => Ok(Some(assignment)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("bootstrap tools manifest: {error}")),
    }
}

fn load_tools_assignment(path: &std::path::Path) -> Result<ToolsAssignment, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let assignment: ToolsAssignment = serde_json::from_str(&content)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    validate_tools_assignment(&assignment)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(assignment)
}

fn load_tools_active_state(path: &std::path::Path) -> Result<ToolsActiveState, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let state: ToolsActiveState = serde_json::from_str(&content)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if state.schema_version != 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "active tools state schema_version is unsupported",
        ));
    }
    Ok(state)
}

fn validate_tools_assignment(assignment: &ToolsAssignment) -> Result<(), String> {
    if assignment.schema_version != 1 {
        return Err("tools assignment schema_version must be 1".into());
    }
    if assignment.tools_version.trim().is_empty() {
        return Err("tools version must be non-empty".into());
    }
    if !SHA256_PATTERN.is_match(&assignment.sha256) {
        return Err("sha256 must be a 64-character lowercase hex digest".into());
    }
    if assignment.artifact_size <= 0 {
        return Err("artifact_size must be positive".into());
    }
    Ok(())
}

fn tools_installation_verified(paths: &AppliancePaths, assignment: &ToolsAssignment) -> bool {
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

fn hash_file_at_path(path: &std::path::Path, expected_size: i64) -> Result<String, String> {
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
