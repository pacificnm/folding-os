mod apply;
mod assign_local;
mod upstream;

pub use apply::{
    apply_local, fleet_apply_foldops, fleet_apply_tools, ApplyLocalRequest,
    FleetSoftwareApplyRequest,
};
pub use assign_local::{ensure_foldops_release_imported, ensure_tools_release_imported};

use std::sync::Arc;

use chrono::Utc;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::agent::inspect::{fetch_agent_inspect_foldops, fetch_agent_inspect_tools};
use crate::config::Config;
use crate::db::{self, Db};
use crate::foldingos::{self, EnrollmentRecord, FleetDelegateConfig};
use crate::software::upstream::{
    apply_pending, select_foldops_latest, select_tools_latest, update_available, UpstreamCache,
    UpstreamError, UpstreamLatest,
};

pub struct SoftwareService {
    cache: Mutex<UpstreamCache>,
}

struct AgentSnapshot {
    hostname: String,
    node_id: String,
    online: bool,
    assigned_foldops: String,
    assigned_tools: String,
    is_supervisor: bool,
}

impl Default for SoftwareService {
    fn default() -> Self {
        Self {
            cache: Mutex::new(UpstreamCache::default()),
        }
    }
}

pub async fn build_updates_response(
    db: &Db,
    config: &Config,
    software: &SoftwareService,
    refresh: bool,
) -> Result<Value, UpstreamError> {
    let foldingosctl_path = &config.foldingosctl_path;
    let delegate = || FleetDelegateConfig { foldingosctl_path };

    let enrollments = foldingos::list_enrollments(delegate())
        .await
        .map_err(|error| UpstreamError::FetchFailed {
            channel: "enrollments",
            message: error.to_string(),
        })?;

    let node =
        foldingos::inspect_node(delegate())
            .await
            .map_err(|error| UpstreamError::FetchFailed {
                channel: "inspect node",
                message: error.to_string(),
            })?;

    let supervisor_hostname = node
        .get("hostname")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let supervisor_node_id = node
        .get("node_id")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let foldingos_version = node
        .get("foldingos_version")
        .and_then(|value| value.as_str())
        .unwrap_or("0.1.0")
        .to_string();

    let mut cache = software.cache.lock().await;
    let indexes = cache
        .load_indexes(
            &config.packages_foldops_index_url,
            &config.packages_tools_index_url,
            refresh,
        )
        .await?;
    let upstream = UpstreamLatest {
        foldops: select_foldops_latest(&indexes.foldops, &foldingos_version),
        tools: select_tools_latest(&indexes.tools, &foldingos_version),
    };

    let supervisor_foldops = foldingos::inspect_foldops(delegate()).await.ok();
    let supervisor_tools = foldingos::inspect_tools(delegate()).await.ok();

    let supervisor_active_foldops = string_field(&supervisor_foldops, "active_manifest_release");
    let supervisor_assigned_foldops = first_non_empty(&[
        string_field(&supervisor_foldops, "assigned_manifest_release"),
        supervisor_assignment_foldops(&enrollments, &supervisor_hostname),
    ]);
    let supervisor_active_tools = first_non_empty(&[
        string_field(&supervisor_tools, "active_tools_version"),
        string_field(&supervisor_tools, "effective_tools_version"),
    ]);
    let supervisor_assigned_tools = first_non_empty(&[
        string_field(&supervisor_tools, "assigned_tools_version"),
        supervisor_assignment_tools(&enrollments, &supervisor_hostname),
    ]);

    let agent_snapshots: Vec<AgentSnapshot> = {
        let conn = db.lock();
        agent_enrollments(&enrollments)
            .into_iter()
            .map(|record| {
                let machine = db::get_machine_by_node_id(&conn, &record.node_id)
                    .ok()
                    .flatten()
                    .or_else(|| db::get_machine(&conn, &record.hostname).ok().flatten());
                let online = machine
                    .as_ref()
                    .map(|machine| db::is_online(&machine.last_seen, config.offline_threshold_ms))
                    .unwrap_or(false);
                AgentSnapshot {
                    hostname: record.hostname.clone(),
                    node_id: record.node_id.clone(),
                    online,
                    assigned_foldops: record.desired_foldops_manifest_release.clone(),
                    assigned_tools: record.desired_tools_version.clone(),
                    is_supervisor: record.hostname == supervisor_hostname,
                }
            })
            .collect()
    };

    let mut agents = Vec::new();
    for snapshot in agent_snapshots {
        let (active_foldops, active_tools) = if snapshot.is_supervisor {
            (
                supervisor_active_foldops.clone(),
                supervisor_active_tools.clone(),
            )
        } else if snapshot.online {
            fetch_agent_versions(
                &snapshot.hostname,
                config.agent_http_port,
                &config.ingest_token,
            )
            .await
        } else {
            (String::new(), String::new())
        };

        let assigned_foldops = first_non_empty(&[snapshot.assigned_foldops]);
        let assigned_tools = first_non_empty(&[snapshot.assigned_tools]);

        agents.push(json!({
            "hostname": snapshot.hostname,
            "node_id": snapshot.node_id,
            "online": snapshot.online,
            "active_foldops_manifest_release": empty_as_null(&active_foldops),
            "assigned_foldops_manifest_release": empty_as_null(&assigned_foldops),
            "active_tools_version": empty_as_null(&active_tools),
            "assigned_tools_version": empty_as_null(&assigned_tools),
            "foldops_apply_pending": apply_pending(&assigned_foldops, &active_foldops),
            "tools_apply_pending": apply_pending(&assigned_tools, &active_tools),
        }));
    }

    Ok(json!({
        "checked_at": Utc::now().to_rfc3339(),
        "upstream": upstream_json(&upstream),
        "supervisor": {
            "hostname": supervisor_hostname,
            "node_id": empty_as_null(&supervisor_node_id),
            "active_foldops_manifest_release": empty_as_null(&supervisor_active_foldops),
            "assigned_foldops_manifest_release": empty_as_null(&supervisor_assigned_foldops),
            "active_tools_version": empty_as_null(&supervisor_active_tools),
            "assigned_tools_version": empty_as_null(&supervisor_assigned_tools),
            "foldops_apply_pending": apply_pending(
                &supervisor_assigned_foldops,
                &supervisor_active_foldops,
            ),
            "tools_apply_pending": apply_pending(
                &supervisor_assigned_tools,
                &supervisor_active_tools,
            ),
            "foldops_update_available": update_available(
                &upstream_latest_foldops(&upstream),
                &supervisor_active_foldops,
                &supervisor_assigned_foldops,
            ),
            "tools_update_available": update_available(
                &upstream_latest_tools(&upstream),
                &supervisor_active_tools,
                &supervisor_assigned_tools,
            ),
        },
        "agents": agents,
    }))
}

fn upstream_json(upstream: &UpstreamLatest) -> Value {
    json!({
        "foldops": upstream.foldops.as_ref().map(|entry| json!({
            "latest_manifest_release": entry.manifest_release,
            "published_at": entry.published_at,
        })).unwrap_or(Value::Null),
        "tools": upstream.tools.as_ref().map(|entry| json!({
            "latest_tools_version": entry.tools_version,
            "published_at": entry.published_at,
        })).unwrap_or(Value::Null),
    })
}

fn upstream_latest_foldops(upstream: &UpstreamLatest) -> String {
    upstream
        .foldops
        .as_ref()
        .map(|entry| entry.manifest_release.as_str())
        .unwrap_or("")
        .to_string()
}

fn upstream_latest_tools(upstream: &UpstreamLatest) -> String {
    upstream
        .tools
        .as_ref()
        .map(|entry| entry.tools_version.as_str())
        .unwrap_or("")
        .to_string()
}

async fn fetch_agent_versions(hostname: &str, port: u16, token: &str) -> (String, String) {
    let foldops = fetch_agent_inspect_foldops(hostname, port, token)
        .await
        .ok()
        .map(|data| string_field(&Some(data), "active_manifest_release"))
        .unwrap_or_default();
    let tools = fetch_agent_inspect_tools(hostname, port, token)
        .await
        .ok()
        .map(|data| {
            first_non_empty(&[
                string_field(&Some(data.clone()), "active_tools_version"),
                string_field(&Some(data), "effective_tools_version"),
            ])
        })
        .unwrap_or_default();
    (foldops, tools)
}

fn agent_enrollments(enrollments: &[EnrollmentRecord]) -> Vec<&EnrollmentRecord> {
    enrollments
        .iter()
        .filter(|record| record.installation_role == "agent")
        .collect()
}

fn supervisor_assignment_foldops(enrollments: &[EnrollmentRecord], hostname: &str) -> String {
    enrollments
        .iter()
        .find(|record| record.hostname == hostname)
        .map(|record| record.desired_foldops_manifest_release.clone())
        .unwrap_or_default()
}

fn supervisor_assignment_tools(enrollments: &[EnrollmentRecord], hostname: &str) -> String {
    enrollments
        .iter()
        .find(|record| record.hostname == hostname)
        .map(|record| record.desired_tools_version.clone())
        .unwrap_or_default()
}

fn string_field(data: &Option<Value>, key: &str) -> String {
    data.as_ref()
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn first_non_empty(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

fn empty_as_null(value: &str) -> Value {
    if value.trim().is_empty() {
        Value::Null
    } else {
        Value::String(value.to_string())
    }
}

pub fn software_service() -> Arc<SoftwareService> {
    Arc::new(SoftwareService::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_as_null_converts_blank_strings() {
        assert!(empty_as_null("").is_null());
        assert_eq!(empty_as_null("0.1.0-1"), Value::String("0.1.0-1".into()));
    }
}
