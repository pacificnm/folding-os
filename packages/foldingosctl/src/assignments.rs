use std::fs;

use serde::Deserialize;

use crate::enrollment::EnrollmentRecord;
use crate::fs_atomic::atomic_write;
use crate::identity::read_node_id;
use crate::inspect::ToolsAssignment;
use crate::paths::AppliancePaths;
use crate::registry_image::{
    is_bootstrap_assignment_label, load_foldops_registry_entry, load_tools_registry_entry,
};
use crate::role::require_supervisor_role;

pub fn apply_local_software_assignments(
    paths: &AppliancePaths,
    record: &EnrollmentRecord,
) -> Result<(), String> {
    apply_assigned_foldops_manifest_for_release(paths, &record.desired_foldops_manifest_release)?;
    apply_assigned_tools_version_for_release(paths, &record.desired_tools_version)
}

pub fn apply_assigned_foldops_manifest_for_release(
    paths: &AppliancePaths,
    release: &str,
) -> Result<(), String> {
    if is_bootstrap_assignment_label(release) {
        return clear_assigned_foldops_manifest(paths);
    }
    let entry = load_foldops_registry_entry(paths, release)?;
    if entry.rollout_state != "ready" {
        return Err(format!(
            "assigned foldops manifest \"{release}\" is not ready for rollout"
        ));
    }
    write_assigned_foldops_manifest(paths, &entry.manifest_toml)
}

pub fn apply_assigned_tools_version_for_release(
    paths: &AppliancePaths,
    version: &str,
) -> Result<(), String> {
    if is_bootstrap_assignment_label(version) {
        return clear_assigned_tools_version(paths);
    }
    let entry = load_tools_registry_entry(paths, version)?;
    if entry.rollout_state != "ready" {
        return Err(format!(
            "assigned tools version \"{version}\" is not ready for rollout"
        ));
    }
    write_assigned_tools_version(paths, &entry.assignment)
}

fn write_assigned_foldops_manifest(paths: &AppliancePaths, content: &str) -> Result<(), String> {
    let content = content.trim();
    if content.is_empty() {
        return clear_assigned_foldops_manifest(paths);
    }
    if let Some(parent) = paths.foldops_assigned_manifest.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut payload = content.to_string();
    payload.push('\n');
    atomic_write(&paths.foldops_assigned_manifest, payload.as_bytes(), 0o644)
}

fn clear_assigned_foldops_manifest(paths: &AppliancePaths) -> Result<(), String> {
    match fs::remove_file(&paths.foldops_assigned_manifest) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("clear assigned foldops manifest: {error}")),
    }
}

fn write_assigned_tools_version(paths: &AppliancePaths, assignment: &ToolsAssignment) -> Result<(), String> {
    if let Some(parent) = paths.tools_assigned_version.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = serde_json::to_string_pretty(assignment).map_err(|error| error.to_string())?;
    let mut content = content;
    content.push('\n');
    atomic_write(&paths.tools_assigned_version, content.as_bytes(), 0o644)
}

fn clear_assigned_tools_version(paths: &AppliancePaths) -> Result<(), String> {
    match fs::remove_file(&paths.tools_assigned_version) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("clear assigned tools version: {error}")),
    }
}

pub fn should_apply_local_supervisor_assignments(
    paths: &AppliancePaths,
    _scope: &str,
    target_node_id: &str,
) -> Result<bool, String> {
    if require_supervisor_role(paths).is_err() {
        return Ok(false);
    }
    let local_node_id = read_node_id(paths).unwrap_or_default();
    Ok(target_node_id.trim() == local_node_id)
}

pub fn apply_supervisor_local_assignments_if_needed(
    paths: &AppliancePaths,
    scope: &str,
    target_node_id: &str,
    record: &EnrollmentRecord,
) -> Result<(), String> {
    if should_apply_local_supervisor_assignments(paths, scope, target_node_id)? {
        apply_local_software_assignments(paths, record)?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct DesiredVersionApiResponse {
    #[serde(default)]
    desired_foldops_manifest: String,
    #[serde(default)]
    desired_tools_assignment: Option<ToolsAssignment>,
}

pub fn sync_local_software_assignments_from_supervisor(
    paths: &AppliancePaths,
    supervisor_url: &str,
    node_id: &str,
    token: &str,
) -> Result<(), String> {
    let endpoint = crate::provision::util::join_supervisor_url(
        supervisor_url,
        &format!("/v1/agents/desired-version?node_id={node_id}"),
    )?;
    let (status, body) = crate::provision::util::http_get_json(
        &endpoint,
        &[("X-FoldingOS-Enrollment-Token", token)],
    )?;
    if status != 200 {
        return Err(format!(
            "desired-version query failed with status {status}: {}",
            body.trim()
        ));
    }
    let response: DesiredVersionApiResponse =
        serde_json::from_str(&body).map_err(|error| error.to_string())?;
    if !response.desired_foldops_manifest.trim().is_empty() {
        write_assigned_foldops_manifest(paths, response.desired_foldops_manifest.trim())?;
    } else {
        clear_assigned_foldops_manifest(paths)?;
    }
    if let Some(assignment) = response.desired_tools_assignment {
        write_assigned_tools_version(paths, &assignment)?;
    } else {
        clear_assigned_tools_version(paths)?;
    }
    Ok(())
}
