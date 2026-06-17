use serde::{Deserialize, Serialize};

use crate::config_host::read_hostname;
use crate::identity::{collect_mac_addresses, ensure_identity, read_installed_foldingos_version, read_node_id};
use crate::paths::AppliancePaths;
use crate::provision::util::{
    empty_human_result, fah_service_active, http_post_json, join_supervisor_url, mark_agent_enrolled,
    agent_enrollment_node_id, read_enrollment_token, read_supervisor_base_url,
};
use crate::role::require_agent_role;

#[derive(Serialize)]
struct AgentRegistrationBody {
    schema_version: i32,
    node_id: String,
    enrollment_token: String,
    installation_role: String,
    current_image_version: String,
    foldingos_version: String,
    hostname: String,
    mac_addresses: Vec<String>,
    #[serde(skip_serializing_if = "is_false")]
    fah_active: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub fn provision_enroll(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    require_agent_role(paths)?;
    ensure_identity(paths)?;

    let supervisor_url = read_supervisor_base_url(paths)?;
    if supervisor_url.is_empty() {
        if read_enrollment_token(paths).is_ok() {
            return Err("supervisor URL is not configured for network-provisioned agent".into());
        }
        println!("Supervisor URL is not configured; agent enrollment skipped.");
        return Ok(empty_human_result());
    }

    let node_id = read_node_id(paths)?;
    if let Ok(enrolled_id) = agent_enrollment_node_id(paths) {
        if enrolled_id == node_id {
            println!("Agent {node_id} is already enrolled.");
            return Ok(empty_human_result());
        }
        return Err(format!(
            "local enrollment state {enrolled_id:?} does not match node identity {node_id:?}"
        ));
    } else if paths.agent_enrollment_state.exists() {
        return Err(agent_enrollment_node_id(paths).unwrap_err());
    }

    let token = read_enrollment_token(paths)
        .map_err(|error| format!("agent enrollment token is not configured: {error}"))?;
    let version = read_installed_foldingos_version()?;
    let hostname = read_hostname(paths)?;
    let mac_addresses = collect_mac_addresses()?;

    let body = AgentRegistrationBody {
        schema_version: 1,
        node_id: node_id.clone(),
        enrollment_token: token,
        installation_role: "agent".into(),
        current_image_version: version.clone(),
        foldingos_version: version,
        hostname,
        mac_addresses,
        fah_active: fah_service_active(),
    };
    let endpoint = join_supervisor_url(&supervisor_url, "/v1/agents/register")?;
    let payload = serde_json::to_string(&body).map_err(|error| error.to_string())?;
    let (status, response_body) = http_post_json(&endpoint, &payload, &[])?;
    if status != 200 {
        return Err(format!(
            "supervisor registration failed with status {status}: {}",
            response_body.trim()
        ));
    }
    let _: serde_json::Value =
        serde_json::from_str(&response_body).map_err(|error| error.to_string())?;
    mark_agent_enrolled(paths, &node_id)?;
    println!("Agent {node_id} enrolled with supervisor {supervisor_url}.");
    Ok(empty_human_result())
}

#[derive(Deserialize)]
pub struct DesiredVersionApiResponse {
    pub desired_version: String,
    #[serde(default)]
    pub desired_foldops_manifest: String,
    #[serde(default)]
    pub desired_tools_assignment: Option<crate::inspect::ToolsAssignment>,
}

pub fn query_desired_version(
    supervisor_url: &str,
    node_id: &str,
    token: &str,
) -> Result<String, String> {
    let endpoint = join_supervisor_url(
        supervisor_url,
        &format!("/v1/agents/desired-version?node_id={node_id}"),
    )?;
    let (status, body) = http_get_json(
        &endpoint,
        &[("X-FoldingOS-Enrollment-Token", token)],
    )?;
    if status != 200 {
        return Err(format!(
            "desired-version query failed with status {status}: {}",
            body.trim()
        ));
    }
    let result: DesiredVersionApiResponse =
        serde_json::from_str(&body).map_err(|error| error.to_string())?;
    Ok(result.desired_version.trim().to_string())
}

fn http_get_json(url: &str, headers: &[(&str, &str)]) -> Result<(u16, String), String> {
    crate::provision::util::http_get_json(url, headers)
}
