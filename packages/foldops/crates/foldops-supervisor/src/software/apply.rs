use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};

use crate::agent::software::{push_foldops_acquire, push_tools_acquire};
use crate::config::Config;
use crate::db::{self, Db};
use crate::foldingos::{self, FleetCommandError, FleetDelegateConfig};
use crate::software::upstream::apply_pending;

#[derive(Debug, Deserialize)]
pub struct FleetSoftwareApplyRequest {
    pub hostnames: Option<Vec<String>>,
    pub all: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ApplyLocalRequest {
    pub foldops: Option<bool>,
    pub tools: Option<bool>,
    pub force: Option<bool>,
}

pub async fn fleet_apply_foldops(
    db: &Db,
    config: &Config,
    request: FleetSoftwareApplyRequest,
) -> Result<Value, String> {
    if !config.uses_supervisor_fleet_delegation() {
        return Err(
            "FoldingOS fleet delegation is unavailable on this host (requires supervisor role with foldingosctl)"
                .into(),
        );
    }
    if config.agent_http_port == 0 {
        return Err("Agent HTTP proxy is disabled (AGENT_HTTP_PORT=0)".into());
    }

    let targets = resolve_fleet_targets(db, config, &request)?;
    let token = config.ingest_token.clone();
    let port = config.agent_http_port;

    let mut results = Vec::new();
    for target in targets {
        if !target.online {
            results.push(json!({
                "hostname": target.hostname,
                "ok": false,
                "error": "Node offline",
                "message": "Node offline; software apply skipped",
            }));
            continue;
        }

        match push_foldops_acquire(&target.hostname, port, &token).await {
            Ok(body) => results.push(fleet_apply_success_result(&target.hostname, "foldops", &body)),
            Err(error) => results.push(json!({
                "hostname": target.hostname,
                "ok": false,
                "error": error,
                "message": error,
            })),
        }
    }

    Ok(json!({ "results": results }))
}

pub async fn fleet_apply_tools(
    db: &Db,
    config: &Config,
    request: FleetSoftwareApplyRequest,
) -> Result<Value, String> {
    if !config.uses_supervisor_fleet_delegation() {
        return Err(
            "FoldingOS fleet delegation is unavailable on this host (requires supervisor role with foldingosctl)"
                .into(),
        );
    }
    if config.agent_http_port == 0 {
        return Err("Agent HTTP proxy is disabled (AGENT_HTTP_PORT=0)".into());
    }

    let targets = resolve_fleet_targets(db, config, &request)?;
    let token = config.ingest_token.clone();
    let port = config.agent_http_port;

    let mut results = Vec::new();
    for target in targets {
        if !target.online {
            results.push(json!({
                "hostname": target.hostname,
                "ok": false,
                "error": "Node offline",
                "message": "Node offline; software apply skipped",
            }));
            continue;
        }

        match push_tools_acquire(&target.hostname, port, &token).await {
            Ok(body) => results.push(fleet_apply_success_result(&target.hostname, "tools", &body)),
            Err(error) => results.push(json!({
                "hostname": target.hostname,
                "ok": false,
                "error": error,
                "message": error,
            })),
        }
    }

    Ok(json!({ "results": results }))
}

struct FleetTarget {
    hostname: String,
    online: bool,
}

fn resolve_fleet_targets(
    db: &Db,
    config: &Config,
    request: &FleetSoftwareApplyRequest,
) -> Result<Vec<FleetTarget>, String> {
    let all = request.all.unwrap_or(false);
    let hostnames = request
        .hostnames
        .as_ref()
        .map(|list| {
            list.iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if all && !hostnames.is_empty() {
        return Err("Use either all=true or hostnames, not both".into());
    }
    if !all && hostnames.is_empty() {
        return Err("Apply requires all=true or hostnames".into());
    }

    let conn = db.lock();
    if all {
        let machines = db::list_machines(&conn).map_err(|error| error.to_string())?;
        return Ok(machines
            .into_iter()
            .filter(|machine| machine.installation_role.as_deref() == Some("agent"))
            .map(|machine| FleetTarget {
                online: db::is_online(&machine.last_seen, config.offline_threshold_ms),
                hostname: machine.hostname,
            })
            .collect());
    }

    Ok(hostnames
        .into_iter()
        .map(|hostname| {
            let online = db::get_machine(&conn, &hostname)
                .ok()
                .flatten()
                .map(|machine| db::is_online(&machine.last_seen, config.offline_threshold_ms))
                .unwrap_or(false);
            FleetTarget { hostname, online }
        })
        .collect())
}

fn fleet_apply_success_result(hostname: &str, kind: &str, body: &Value) -> Value {
    let data = body.get("data").unwrap_or(body);
    let message = data
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or(match kind {
            "foldops" => "foldops acquire completed",
            _ => "tools acquire completed",
        });

    match kind {
        "foldops" => json!({
            "hostname": hostname,
            "ok": true,
            "active_manifest_release": data.get("manifest_release").cloned().unwrap_or(Value::Null),
            "message": message,
        }),
        _ => json!({
            "hostname": hostname,
            "ok": true,
            "active_tools_version": data.get("tools_version").cloned().unwrap_or(Value::Null),
            "message": message,
        }),
    }
}

pub async fn apply_local(
    config: &Config,
    request: ApplyLocalRequest,
) -> Result<Value, String> {
    if !config.uses_supervisor_fleet_delegation() {
        return Err(
            "FoldingOS fleet delegation is unavailable on this host (requires supervisor role with foldingosctl)"
                .into(),
        );
    }

    let apply_foldops = request.foldops.unwrap_or(false);
    let apply_tools = request.tools.unwrap_or(false);
    if !apply_foldops && !apply_tools {
        return Err("Apply-local requires foldops=true and/or tools=true".into());
    }

    let force = request.force.unwrap_or(false);
    let delegate = FleetDelegateConfig {
        foldingosctl_path: &config.foldingosctl_path,
    };

    let mut results = Vec::new();

    if apply_foldops {
        results.push(
            apply_local_foldops(&delegate, force)
                .await
                .unwrap_or_else(|error| local_apply_error("foldops", &error)),
        );
    }
    if apply_tools {
        results.push(
            apply_local_tools(&delegate, force)
                .await
                .unwrap_or_else(|error| local_apply_error("tools", &error)),
        );
    }

    Ok(json!({ "results": results }))
}

async fn apply_local_foldops(
    config: &FleetDelegateConfig<'_>,
    force: bool,
) -> Result<Value, FleetCommandError> {
    if !force {
        let inspect = foldingos::inspect_foldops(*config).await?;
        let assigned = string_field(&inspect, "assigned_manifest_release");
        let active = string_field(&inspect, "active_manifest_release");
        if !apply_pending(&assigned, &active) {
            return Ok(json!({
                "component": "foldops",
                "ok": true,
                "skipped": true,
                "message": "assigned FoldOps manifest matches active release",
            }));
        }
    }

    let data = foldingos::foldops_acquire(*config).await?;
    Ok(json!({
        "component": "foldops",
        "ok": true,
        "skipped": false,
        "active_manifest_release": data.get("manifest_release").cloned().unwrap_or(Value::Null),
        "message": data.get("message").cloned().unwrap_or(Value::String("foldops acquire completed".into())),
    }))
}

async fn apply_local_tools(
    config: &FleetDelegateConfig<'_>,
    force: bool,
) -> Result<Value, FleetCommandError> {
    if !force {
        let inspect = foldingos::inspect_tools(*config).await?;
        let assigned = string_field(&inspect, "assigned_tools_version");
        let active = string_field(&inspect, "active_tools_version");
        if assigned.is_empty() {
            return Ok(json!({
                "component": "tools",
                "ok": true,
                "skipped": true,
                "message": "no supervisor-assigned tools version is configured",
            }));
        }
        if !apply_pending(&assigned, &active) {
            return Ok(json!({
                "component": "tools",
                "ok": true,
                "skipped": true,
                "message": "assigned tools version matches active release",
            }));
        }
    }

    let data = foldingos::tools_acquire(*config).await?;
    Ok(json!({
        "component": "tools",
        "ok": true,
        "skipped": false,
        "active_tools_version": data.get("tools_version").cloned().unwrap_or(Value::Null),
        "message": data.get("message").cloned().unwrap_or(Value::String("tools acquire completed".into())),
    }))
}

fn local_apply_error(component: &str, error: &FleetCommandError) -> Value {
    json!({
        "component": component,
        "ok": false,
        "error": error.to_string(),
        "message": error.to_string(),
    })
}

fn string_field(data: &Value, key: &str) -> String {
    data.get(key)
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

mod tests {
    use super::*;

    #[test]
    fn fleet_apply_success_result_includes_manifest_release() {
        let body = json!({
            "ok": true,
            "data": {
                "manifest_release": "0.1.0-2",
                "message": "foldops acquire completed",
            }
        });
        let result = fleet_apply_success_result("agent-01", "foldops", &body);
        assert_eq!(result["ok"], true);
        assert_eq!(result["active_manifest_release"], "0.1.0-2");
    }
}
