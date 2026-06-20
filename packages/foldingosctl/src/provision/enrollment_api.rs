use serde::{Deserialize, Serialize};

use crate::enrollment::{load_enrollment_record, save_enrollment_record, EnrollmentRecord};
use crate::inspect::ToolsAssignment;
use crate::paths::AppliancePaths;
use crate::provision::assign::{assign_software_versions, AssignmentUpdate};
use crate::provision::util::{rfc3339_now, validate_enrollment_token};
use crate::registry_image::{
    is_bootstrap_assignment_label, load_foldops_registry_entry, load_tools_registry_entry,
};

#[derive(Debug, Deserialize)]
pub struct AgentRegistrationRequest {
    pub schema_version: i32,
    pub node_id: String,
    pub enrollment_token: String,
    pub installation_role: String,
    pub current_image_version: String,
    pub foldingos_version: String,
    pub hostname: String,
    pub mac_addresses: Vec<String>,
    #[serde(default)]
    pub fah_active: bool,
}

#[derive(Debug, Serialize)]
pub struct DesiredVersionResponse {
    pub schema_version: i32,
    pub node_id: String,
    pub current_image_version: String,
    pub desired_version: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub desired_foldops_manifest_release: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub desired_foldops_manifest: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub desired_tools_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desired_tools_assignment: Option<ToolsAssignment>,
}

#[derive(Debug, Deserialize)]
pub struct RolloutAssignRequest {
    pub schema_version: i32,
    pub enrollment_token: String,
    pub scope: String,
    #[serde(default)]
    pub node_id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub foldops_manifest_release: String,
    #[serde(default)]
    pub tools_version: String,
}

pub fn register_agent(
    paths: &AppliancePaths,
    request: AgentRegistrationRequest,
) -> Result<EnrollmentRecord, String> {
    let validated = validate_registration_request(request)?;
    validate_enrollment_token(paths, &validated.enrollment_token)?;

    let now = rfc3339_now();
    let mut record = EnrollmentRecord {
        schema_version: 1,
        node_id: validated.node_id.clone(),
        installation_role: validated.installation_role,
        registered_at: now.clone(),
        last_seen_at: now,
        mac_addresses: validated.mac_addresses,
        current_image_version: validated.current_image_version,
        foldingos_version: validated.foldingos_version,
        hostname: validated.hostname,
        fah_active: Some(validated.fah_active),
        desired_image_version: "current".into(),
        desired_foldops_manifest_release: String::new(),
        desired_tools_version: String::new(),
        last_update_status: String::new(),
        last_update_version: String::new(),
        last_update_message: String::new(),
        last_update_at: String::new(),
    };

    if let Ok(existing) = load_enrollment_record(paths, &validated.node_id) {
        record.registered_at = existing.registered_at;
        record.desired_image_version = existing.desired_image_version;
        record.desired_foldops_manifest_release = existing.desired_foldops_manifest_release;
        record.desired_tools_version = existing.desired_tools_version;
    }

    save_enrollment_record(paths, record.clone())?;
    Ok(record)
}

fn validate_registration_request(
    mut request: AgentRegistrationRequest,
) -> Result<AgentRegistrationRequest, String> {
    if request.schema_version != 1 {
        return Err(format!(
            "unsupported registration schema version {}",
            request.schema_version
        ));
    }
    request.node_id = request.node_id.trim().to_string();
    if !crate::enrollment::is_valid_node_id(&request.node_id) {
        return Err("registration node_id is invalid".into());
    }
    request.enrollment_token = request.enrollment_token.trim().to_string();
    if request.enrollment_token.is_empty() {
        return Err("registration enrollment_token is required".into());
    }
    request.installation_role = request.installation_role.trim().to_string();
    if request.installation_role != "agent" {
        return Err(format!(
            "registration role must be agent, found {:?}",
            request.installation_role
        ));
    }
    if request.current_image_version.trim().is_empty() {
        return Err("registration current_image_version is required".into());
    }
    if request.foldingos_version.trim().is_empty() {
        return Err("registration foldingos_version is required".into());
    }
    if request.hostname.trim().is_empty() {
        return Err("registration hostname is required".into());
    }
    if request.mac_addresses.is_empty() {
        return Err("registration mac_addresses is required".into());
    }
    Ok(request)
}

pub fn desired_version_for_node(
    paths: &AppliancePaths,
    node_id: &str,
) -> Result<DesiredVersionResponse, String> {
    let mut record = load_enrollment_record(paths, node_id).map_err(|error| {
        if error.contains("No such file") || error.contains("not found") {
            "agent is not registered".to_string()
        } else {
            error
        }
    })?;

    record.last_seen_at = rfc3339_now();
    save_enrollment_record(paths, record.clone())?;

    let mut desired = record.desired_image_version.clone();
    if desired == record.current_image_version {
        desired = "current".into();
    }

    Ok(DesiredVersionResponse {
        schema_version: 2,
        node_id: record.node_id,
        current_image_version: record.current_image_version,
        desired_version: desired,
        desired_foldops_manifest_release: record.desired_foldops_manifest_release.clone(),
        desired_foldops_manifest: resolve_desired_foldops_manifest_toml(
            paths,
            &record.desired_foldops_manifest_release,
        ),
        desired_tools_version: record.desired_tools_version.clone(),
        desired_tools_assignment: resolve_desired_tools_assignment(
            paths,
            &record.desired_tools_version,
        ),
    })
}

fn resolve_desired_foldops_manifest_toml(paths: &AppliancePaths, release: &str) -> String {
    let release = release.trim();
    if is_bootstrap_assignment_label(release) {
        return String::new();
    }
    load_foldops_registry_entry(paths, release)
        .map(|entry| entry.manifest_toml)
        .unwrap_or_default()
}

fn resolve_desired_tools_assignment(
    paths: &AppliancePaths,
    version: &str,
) -> Option<ToolsAssignment> {
    let version = version.trim();
    if is_bootstrap_assignment_label(version) {
        return None;
    }
    load_tools_registry_entry(paths, version)
        .ok()
        .map(|entry| entry.assignment)
}

pub fn handle_rollout_assign(
    paths: &AppliancePaths,
    assign: RolloutAssignRequest,
) -> Result<serde_json::Value, String> {
    if assign.schema_version != 1 {
        return Err("unsupported rollout assignment schema version".into());
    }
    validate_enrollment_token(paths, assign.enrollment_token.trim())?;

    let mut update = AssignmentUpdate::default();
    if !assign.version.trim().is_empty() {
        update.image_version = Some(assign.version.trim().to_string());
    }
    if !assign.foldops_manifest_release.trim().is_empty() {
        update.foldops_manifest_release = Some(assign.foldops_manifest_release.trim().to_string());
    }
    if !assign.tools_version.trim().is_empty() {
        update.tools_version = Some(assign.tools_version.trim().to_string());
    }
    if update.image_version.is_none()
        && update.foldops_manifest_release.is_none()
        && update.tools_version.is_none()
    {
        return Err(
            "assignment requires at least one of --version, --foldops-manifest, or --tools-version"
                .into(),
        );
    }

    let updated = assign_software_versions(
        paths,
        assign.scope.trim(),
        assign.node_id.trim(),
        update.clone(),
    )?;

    Ok(serde_json::json!({
        "schema_version": 1,
        "updated_agents": updated,
        "assigned_image_version": update.image_version.as_deref().unwrap_or("").trim(),
        "assigned_foldops_manifest_release": update.foldops_manifest_release.as_deref().unwrap_or("").trim(),
        "assigned_tools_version": update.tools_version.as_deref().unwrap_or("").trim(),
    }))
}
