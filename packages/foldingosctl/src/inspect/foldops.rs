use std::fs;

use crate::inspect::commissioning::{parse_manifest_release, read_current_release};
use crate::paths::AppliancePaths;

pub fn inspect_foldops(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    Ok(collect_inspect_foldops_data(paths)?)
}

pub fn collect_inspect_foldops_data(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let mut data = serde_json::json!({
        "packages_acquired": paths.foldops_current_link().exists(),
        "provisioned": paths.foldops_provisioned_marker.exists(),
    });

    if let Ok(content) = fs::read_to_string(&paths.foldops_embedded_manifest) {
        if let Some(release) = parse_manifest_release(&content) {
            data["bootstrap_manifest_release"] = serde_json::Value::String(release);
        }
    }

    if paths.foldops_assigned_manifest.exists() {
        let content = fs::read_to_string(&paths.foldops_assigned_manifest)
            .map_err(|error| format!("read assigned foldops manifest: {error}"))?;
        if let Some(release) = parse_manifest_release(&content) {
            data["assigned_manifest_release"] = serde_json::Value::String(release);
        }
    }

    if let Ok(release) = read_current_release(&paths.foldops_apps_root) {
        data["active_manifest_release"] = serde_json::Value::String(release);
    }

    if let Ok(manifest_path) = resolve_effective_foldops_manifest_path(paths) {
        let content = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read effective foldops manifest: {error}"))?;
        if let Some(release) = parse_manifest_release(&content) {
            data["effective_manifest_release"] = serde_json::Value::String(release);
        }
    }

    Ok(data)
}

fn resolve_effective_foldops_manifest_path(
    paths: &AppliancePaths,
) -> Result<std::path::PathBuf, String> {
    if paths.foldops_assigned_manifest.exists() {
        return Ok(paths.foldops_assigned_manifest.clone());
    }
    Ok(paths.foldops_embedded_manifest.clone())
}
