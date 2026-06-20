use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use foldops_types::{is_control_action, validate_ingest_payload, IngestPayload};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::agent::config::{
    build_foldinghome_candidate_toml, normalize_passkey_input, push_foldinghome_config,
    validate_foldinghome_candidate, write_supervisor_candidate, FoldinghomeConfigRequest,
};
use crate::agent::control::{fetch_agent_control_status, push_agent_control};
use crate::agent::logs::{fetch_live_agent_logs, LogSource};
use crate::alerts::engine::{
    alerts_status_json, list_active_alerts_json, list_alert_history_json, run_test_alert,
};
use crate::config::Config;
use crate::db::{self, Db, MachineRow, SnapshotRow};
use crate::deploy::db::{get_deploy_run, list_deploy_runs};
use crate::deploy::start_agent_deploy;
use crate::fah_projects::fetch_fah_project;
use crate::foldingos::{
    self, AllowBootRequest, AssignRequest, FleetCommandError, FleetDelegateConfig,
};
use crate::install_log;
use crate::recovery::{self, BACKUPS_DIR};
use crate::services;
use crate::software::{
    self, apply_local, ensure_foldops_release_imported, ensure_tools_release_imported,
    fleet_apply_foldops, fleet_apply_tools, ApplyLocalRequest, FleetSoftwareApplyRequest,
    SoftwareService,
};
use crate::supervisor_logs::{fetch_supervisor_logs, SupervisorLogSource};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<Config>,
    pub software: Arc<SoftwareService>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/ingest", post(ingest))
        .route("/machines", get(list_machines))
        .route("/machines/{name}", get(get_machine))
        .route("/machines/{name}/logs", get(machine_logs))
        .route("/machines/{name}/control/status", get(control_status))
        .route("/machines/{name}/control", post(control_action))
        .route(
            "/machines/{name}/config/foldinghome",
            post(foldinghome_config),
        )
        .route("/deploy/runs", get(deploy_runs))
        .route("/deploy/runs/{id}", get(deploy_run))
        .route("/deploy/agents", post(deploy_agents))
        .route("/alerts/status", get(alerts_status))
        .route("/alerts/test", post(alerts_test))
        .route("/alerts/history", get(alerts_history))
        .route("/alerts", get(alerts_active))
        .route("/projects/{id}", get(project_detail))
        .route("/snapshots/{name}", get(snapshots))
        .route("/fleet/enrollments", get(fleet_enrollments))
        .route(
            "/fleet/allow-boot",
            get(fleet_allow_boot)
                .post(fleet_allow_boot_create)
                .delete(fleet_allow_boot_delete),
        )
        .route("/fleet/registry", get(fleet_registry))
        .route("/fleet/registry/{version}", get(fleet_registry_show))
        .route("/fleet/assign", post(fleet_assign))
        .route(
            "/fleet/software/apply-foldops",
            post(fleet_software_apply_foldops),
        )
        .route(
            "/fleet/software/apply-tools",
            post(fleet_software_apply_tools),
        )
        .route("/software/updates", get(software_updates))
        .route("/software/install-log", get(software_install_log))
        .route("/software/apply-local", post(software_apply_local))
        .route("/supervisor/logs", get(supervisor_logs))
        .route("/recovery/export", post(recovery_export_create))
        .route("/recovery/export/latest", get(recovery_export_latest))
        .route("/services", get(services_list))
        .route("/services/restart", post(services_restart))
        .route("/services/restart-all", post(services_restart_all))
        .with_state(state)
}

#[allow(clippy::result_large_err)]
fn require_auth(headers: &HeaderMap, token: &str) -> Result<(), Response> {
    let header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !header.starts_with("Bearer ") {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Missing bearer token" })),
        )
            .into_response());
    }
    if &header[7..] != token {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Invalid token" })),
        )
            .into_response());
    }
    Ok(())
}

fn parse_payload(row: &SnapshotRow) -> Option<IngestPayload> {
    serde_json::from_str(&row.payload).ok()
}

fn snapshot_summary(row: Option<&SnapshotRow>) -> Option<Value> {
    let row = row?;
    let payload = parse_payload(row)?;
    Some(json!({
        "id": row.id,
        "created_at": row.created_at,
        "fah_status": row.fah_status,
        "project": row.project,
        "run": row.run,
        "clone": row.clone,
        "gen": row.gen,
        "progress": row.progress,
        "ppd": row.ppd,
        "cpu_usage": row.cpu_usage,
        "memory_percent": row.memory_percent,
        "disk_percent": row.disk_percent,
        "cpu_temp": row.cpu_temp.or(payload.system.cpuTemp),
        "chassis_temp": row.chassis_temp.or(payload.system.chassisTemp),
        "apt_updates": row.apt_updates,
        "reboot_required": row.reboot_required == 1,
        "payload": payload,
    }))
}

async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<IngestPayload>,
) -> Response {
    if let Err(resp) = require_auth(&headers, &state.config.ingest_token) {
        return resp;
    }
    if let Err(e) = validate_ingest_payload(&payload) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid payload", "details": e.to_string() })),
        )
            .into_response();
    }

    let conn = state.db.lock();
    match db::ingest_snapshot(&conn, &payload) {
        Ok(()) => {
            drop(conn);
            spawn_alert_eval(state.clone());
            Json(json!({ "ok": true, "hostname": payload.hostname })).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "ingest error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to store snapshot" })),
            )
                .into_response()
        }
    }
}

async fn list_machines(State(state): State<AppState>) -> Json<Value> {
    let conn = state.db.lock();
    let machines: Vec<Value> = db::list_machines(&conn)
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let latest = db::get_latest_snapshot(&conn, &m.hostname).ok().flatten();
            let online = db::is_online(&m.last_seen, state.config.offline_threshold_ms);
            json!({
                "hostname": m.hostname,
                "node_id": m.node_id,
                "installation_role": m.installation_role,
                "foldingos_version": m.foldingos_version,
                "first_seen": m.first_seen,
                "last_seen": m.last_seen,
                "online": online,
                "latest": snapshot_summary(latest.as_ref()),
            })
        })
        .collect();

    let farm_ppd: f64 = machines
        .iter()
        .filter_map(|m| {
            if m.get("online")?.as_bool()? {
                m.get("latest")?.get("ppd")?.as_f64()
            } else {
                None
            }
        })
        .sum();
    let farm_ppd = (farm_ppd * 100.0).round() / 100.0;

    Json(json!({ "machines": machines, "farm_ppd": farm_ppd }))
}

async fn get_machine(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let conn = state.db.lock();
    let Some(machine) = db::get_machine(&conn, &name).ok().flatten() else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Machine not found" })),
        )
            .into_response();
    };
    let latest = db::get_latest_snapshot(&conn, &machine.hostname)
        .ok()
        .flatten();
    Json(json!({
        "hostname": machine.hostname,
        "node_id": machine.node_id,
        "installation_role": machine.installation_role,
        "foldingos_version": machine.foldingos_version,
        "first_seen": machine.first_seen,
        "last_seen": machine.last_seen,
        "online": db::is_online(&machine.last_seen, state.config.offline_threshold_ms),
        "latest": snapshot_summary(latest.as_ref()),
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
struct LogsQuery {
    source: Option<String>,
    lines: Option<u32>,
    live: Option<String>,
}

#[axum::debug_handler]
async fn machine_logs(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<LogsQuery>,
) -> Response {
    let source_str = q.source.as_deref().unwrap_or("fah");
    let Some(source) = LogSource::parse(source_str) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "source must be fah or work" })),
        )
            .into_response();
    };
    let lines = q.lines.unwrap_or(200).clamp(1, 500);
    let want_live = q.live.as_deref() != Some("0");

    struct LogsCtx {
        hostname: String,
        cached_lines: Vec<String>,
        cached_path: Option<String>,
        updated_at: Option<String>,
        online: bool,
    }

    let ctx = {
        let conn = state.db.lock();
        let Some(machine) = db::get_machine(&conn, &name).ok().flatten() else {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Machine not found" })),
            )
                .into_response();
        };

        let latest = db::get_latest_snapshot(&conn, &machine.hostname)
            .ok()
            .flatten();
        let payload = latest.as_ref().and_then(parse_payload);
        let (cached_lines, cached_path) = match source {
            LogSource::Fah => (
                payload
                    .as_ref()
                    .and_then(|p| p.logs.as_ref())
                    .map(|l| l.fah.clone())
                    .unwrap_or_default(),
                payload
                    .as_ref()
                    .and_then(|p| p.logs.as_ref())
                    .and_then(|l| l.fahPath.clone()),
            ),
            LogSource::Work => (
                payload
                    .as_ref()
                    .and_then(|p| p.logs.as_ref())
                    .map(|l| l.work.clone())
                    .unwrap_or_default(),
                payload
                    .as_ref()
                    .and_then(|p| p.logs.as_ref())
                    .and_then(|l| l.workPath.clone()),
            ),
        };

        LogsCtx {
            hostname: machine.hostname.clone(),
            cached_lines,
            cached_path,
            updated_at: latest.map(|r| r.created_at),
            online: db::is_online(&machine.last_seen, state.config.offline_threshold_ms),
        }
    };

    let agent_port = state.config.agent_http_port;
    let token = state.config.ingest_token.clone();

    if want_live && ctx.online && agent_port > 0 {
        match fetch_live_agent_logs(&ctx.hostname, agent_port, &token, source, lines).await {
            Ok((path, live_lines)) => {
                return Json(json!({
                    "hostname": ctx.hostname,
                    "source": source_str,
                    "lines": live_lines,
                    "path": if path.is_empty() { ctx.cached_path } else { Some(path) },
                    "updated_at": chrono::Utc::now().to_rfc3339(),
                    "live": true,
                    "online": true,
                }))
                .into_response();
            }
            Err(live_error) => {
                if is_expected_missing_live_log(source, &live_error) {
                    tracing::info!(hostname = %ctx.hostname, source = source_str, error = %live_error, "live log not available yet");
                    let slice = ctx.cached_lines
                        [ctx.cached_lines.len().saturating_sub(lines as usize)..]
                        .to_vec();
                    return Json(json!({
                        "hostname": ctx.hostname,
                        "source": source_str,
                        "lines": slice,
                        "path": ctx.cached_path,
                        "updated_at": ctx.updated_at,
                        "live": false,
                        "online": true,
                    }))
                    .into_response();
                }
                tracing::warn!(hostname = %ctx.hostname, source = source_str, error = %live_error, "live log fetch failed");
                let slice = ctx.cached_lines
                    [ctx.cached_lines.len().saturating_sub(lines as usize)..]
                    .to_vec();
                return Json(json!({
                    "hostname": ctx.hostname,
                    "source": source_str,
                    "lines": slice,
                    "path": ctx.cached_path,
                    "updated_at": ctx.updated_at,
                    "live": false,
                    "online": true,
                    "live_error": live_error,
                    "live_url": format!("http://{}:{agent_port}/logs/{source_str}", ctx.hostname),
                    "warning": format!("Live pull failed: {live_error}"),
                }))
                .into_response();
            }
        }
    }

    let slice = ctx.cached_lines[ctx.cached_lines.len().saturating_sub(lines as usize)..].to_vec();
    Json(json!({
        "hostname": ctx.hostname,
        "source": source_str,
        "lines": slice,
        "path": ctx.cached_path,
        "updated_at": ctx.updated_at,
        "live": false,
        "online": ctx.online,
    }))
    .into_response()
}

fn is_expected_missing_live_log(source: LogSource, error: &str) -> bool {
    source == LogSource::Fah && error.eq_ignore_ascii_case("FAH log not readable")
}

#[axum::debug_handler]
async fn control_status(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    if !state.config.control_enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Remote control disabled (set CONTROL_ENABLED=true)" })),
        )
            .into_response();
    }

    let proxy = {
        let conn = state.db.lock();
        let Some(machine) = db::get_machine(&conn, &name).ok().flatten() else {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Machine not found" })),
            )
                .into_response();
        };
        if !db::is_online(&machine.last_seen, state.config.offline_threshold_ms) {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Node offline" })),
            )
                .into_response();
        }
        (
            machine.hostname.clone(),
            state.config.agent_http_port,
            state.config.ingest_token.clone(),
        )
    };

    match fetch_agent_control_status(&proxy.0, proxy.1, &proxy.2).await {
        Ok(status) => Json(json!({
            "hostname": proxy.0,
            "foldops_agent": status.foldops_agent,
            "fah_client": status.fah_client,
            "fah_folding_state": status.fah_folding_state,
            "fah_unit_state": status.fah_unit_state,
            "fah_folding_detail": status.fah_folding_detail,
        }))
        .into_response(),
        Err(msg) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ControlBody {
    action: Option<String>,
}

async fn control_action(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<ControlBody>,
) -> Response {
    if !state.config.control_enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Remote control disabled (set CONTROL_ENABLED=true)" })),
        )
            .into_response();
    }

    let action = body.action.as_deref().unwrap_or("");
    if action.is_empty() || !is_control_action(action) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid or missing action" })),
        )
            .into_response();
    }

    let proxy = {
        let conn = state.db.lock();
        let Some(machine) = db::get_machine(&conn, &name).ok().flatten() else {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Machine not found" })),
            )
                .into_response();
        };
        if !db::is_online(&machine.last_seen, state.config.offline_threshold_ms) {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Node offline" })),
            )
                .into_response();
        }
        (
            machine.hostname.clone(),
            state.config.agent_http_port,
            state.config.ingest_token.clone(),
            action.to_string(),
        )
    };

    match push_agent_control(&proxy.0, proxy.1, &proxy.2, &proxy.3).await {
        Ok(result) => Json(json!({
            "hostname": proxy.0,
            "ok": result.ok,
            "action": result.action,
            "message": result.message,
            "stdout": result.stdout,
            "stderr": result.stderr,
        }))
        .into_response(),
        Err(msg) => {
            let likely_restart = proxy.3 == "agent.restart"
                && regex::Regex::new(r"(?i)(ECONNRESET|socket hang up|fetch failed)")
                    .unwrap()
                    .is_match(&msg);
            if likely_restart {
                Json(json!({
                    "hostname": proxy.0,
                    "ok": true,
                    "action": proxy.3,
                    "message": "Agent restarted (connection closed)",
                    "stdout": "",
                    "stderr": msg,
                }))
                .into_response()
            } else {
                (StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))).into_response()
            }
        }
    }
}

async fn foldinghome_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<FoldinghomeConfigRequest>,
) -> Response {
    if !state.config.config_enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Remote config push disabled" })),
        )
            .into_response();
    }

    let username = body.username.trim();
    if username.is_empty() || username.as_bytes().len() > 128 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "username must contain 1 through 128 UTF-8 bytes" })),
        )
            .into_response();
    }
    if body.team < 0 || body.team > 2_147_483_647 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "team is outside the supported range" })),
        )
            .into_response();
    }

    let passkey = match normalize_passkey_input(&body.passkey) {
        Ok(value) => value,
        Err(error) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response();
        }
    };

    let proxy = {
        let conn = state.db.lock();
        let Some(machine) = db::get_machine(&conn, &name).ok().flatten() else {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Machine not found" })),
            )
                .into_response();
        };
        if !db::is_online(&machine.last_seen, state.config.offline_threshold_ms) {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Node offline" })),
            )
                .into_response();
        }
        let passkey_configured = db::get_latest_snapshot(&conn, &machine.hostname)
            .ok()
            .flatten()
            .and_then(|snapshot| parse_payload(&snapshot))
            .and_then(|payload| payload.fah.configPasskeyConfigured)
            .unwrap_or(false);
        (
            machine.hostname.clone(),
            state.config.agent_http_port,
            state.config.ingest_token.clone(),
            passkey_configured,
        )
    };

    let mut passkey_secret = if !passkey.is_empty() {
        "fah-passkey".to_string()
    } else {
        body.passkey_secret.trim().to_string()
    };
    let preserving_existing_passkey = passkey.is_empty() && passkey_secret.is_empty() && proxy.3;
    if preserving_existing_passkey {
        passkey_secret = "fah-passkey".to_string();
    }

    let validation_secret = if passkey.is_empty() && !preserving_existing_passkey {
        passkey_secret.as_str()
    } else {
        ""
    };
    let validation_toml = build_foldinghome_candidate_toml(username, body.team, validation_secret);
    let candidate_path = match write_supervisor_candidate(&validation_toml) {
        Ok(path) => path,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": error })),
            )
                .into_response();
        }
    };

    if let Err(error) =
        validate_foldinghome_candidate(&state.config.foldingosctl_path, &candidate_path).await
    {
        let _ = std::fs::remove_file(&candidate_path);
        tracing::warn!(error = %error, "supervisor rejected foldinghome candidate");
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response();
    }
    let _ = std::fs::remove_file(&candidate_path);

    let config_toml =
        build_foldinghome_candidate_toml(username, body.team, passkey_secret.as_str());

    tracing::info!(hostname = %proxy.0, "proxying foldinghome config push to agent");

    match push_foldinghome_config(
        &proxy.0,
        proxy.1,
        &proxy.2,
        &config_toml,
        if passkey.is_empty() {
            None
        } else {
            Some(passkey.as_str())
        },
        !passkey_secret.is_empty(),
    )
    .await
    {
        Ok(result) => {
            let direct_ingest = if let Some(snapshot) = result.snapshot.as_ref() {
                let conn = state.db.lock();
                match db::ingest_snapshot(&conn, snapshot) {
                    Ok(()) => Some(Ok(())),
                    Err(error) => Some(Err(error.to_string())),
                }
            } else {
                None
            };
            let direct_ingested = direct_ingest
                .as_ref()
                .map(|result| result.is_ok())
                .unwrap_or(false);
            let ingest_error = direct_ingest
                .as_ref()
                .and_then(|result| result.as_ref().err().cloned())
                .or(result.ingest_error.clone());

            if let Some(error) = direct_ingest
                .as_ref()
                .and_then(|result| result.as_ref().err())
            {
                tracing::warn!(
                    hostname = %proxy.0,
                    error = %error,
                    "failed to store post-config snapshot returned by agent"
                );
            }

            Json(json!({
                "hostname": proxy.0,
                "ok": result.ok,
                "domain": result.domain,
                "candidate": result.candidate,
                "activated": result.activated,
                "ingested": direct_ingested || result.ingested.unwrap_or(false),
                "ingest_error": ingest_error,
            }))
            .into_response()
        }
        Err(msg) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))).into_response(),
    }
}

async fn deploy_runs(State(state): State<AppState>) -> Json<Value> {
    let conn = state.db.lock();
    let runs = list_deploy_runs(&conn, 25).unwrap_or_default();
    Json(json!({ "runs": runs }))
}

async fn deploy_run(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let conn = state.db.lock();
    match get_deploy_run(&conn, &id).ok().flatten() {
        Some(run) => Json(json!(run)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Deploy run not found" })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct DeployBody {
    hostnames: Option<Vec<String>>,
}

async fn deploy_agents(State(state): State<AppState>, Json(body): Json<DeployBody>) -> Response {
    if !state.config.deploy_enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Deploy is disabled (set DEPLOY_ENABLED=true)" })),
        )
            .into_response();
    }

    let hostnames = body
        .hostnames
        .map(|list| {
            list.into_iter()
                .filter(|h| !h.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|list| !list.is_empty());

    match start_agent_deploy(state.db.clone(), state.config.clone(), hostnames) {
        Ok(run_id) => (
            StatusCode::ACCEPTED,
            Json(json!({ "run_id": run_id, "status": "running" })),
        )
            .into_response(),
        Err(msg) => (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))).into_response(),
    }
}

async fn alerts_status(State(state): State<AppState>) -> Json<Value> {
    Json(alerts_status_json(&state.config.alert_config))
}

async fn alerts_test(State(state): State<AppState>) -> Response {
    let cfg = &state.config.alert_config;
    if cfg.webhook_url.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "ALERT_WEBHOOK_URL is not set" })),
        )
            .into_response();
    }
    if !cfg.enabled {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Alerts disabled — set ALERTS_ENABLED=true or ALERT_WEBHOOK_URL" })),
        )
            .into_response();
    }

    match run_test_alert(cfg).await {
        Ok(()) => Json(json!({
            "ok": true,
            "message": "Test notification sent",
            "status": alerts_status_json(cfg),
        }))
        .into_response(),
        Err(msg) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    status: Option<String>,
    limit: Option<i64>,
    hostname: Option<String>,
}

async fn alerts_history(
    State(state): State<AppState>,
    Query(q): Query<HistoryQuery>,
) -> Json<Value> {
    let status_param = q.status.as_deref().unwrap_or("all");
    let status = match status_param {
        "active" | "resolved" | "all" => status_param,
        _ => "all",
    };
    let limit = q.limit.unwrap_or(100).min(500);
    let hostname = q
        .hostname
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    Json(list_alert_history_json(&state.db, limit, status, hostname))
}

async fn alerts_active(State(state): State<AppState>) -> Json<Value> {
    Json(list_active_alerts_json(&state.db))
}

async fn project_detail(State(_state): State<AppState>, Path(id): Path<String>) -> Response {
    let project_id: i64 = match id.trim().parse() {
        Ok(n) if n > 0 => n,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid project id" })),
            )
                .into_response();
        }
    };

    match fetch_fah_project(project_id).await {
        Ok(Some(project)) => Json(json!(project)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Project not found" })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "FAH project fetch error");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "Failed to fetch project from Folding@home" })),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct SnapshotsQuery {
    limit: Option<i64>,
}

async fn snapshots(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<SnapshotsQuery>,
) -> Response {
    let conn = state.db.lock();
    let Some(_machine) = db::get_machine(&conn, &name).ok().flatten() else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Machine not found" })),
        )
            .into_response();
    };
    let limit = q.limit.unwrap_or(100).min(500);
    let rows = db::get_snapshots(&conn, &name, limit).unwrap_or_default();
    let snapshots: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "created_at": row.created_at,
                "summary": {
                    "fah_status": row.fah_status,
                    "project": row.project,
                    "progress": row.progress,
                    "ppd": row.ppd,
                    "cpu_usage": row.cpu_usage,
                    "memory_percent": row.memory_percent,
                    "disk_percent": row.disk_percent,
                    "cpu_temp": row.cpu_temp,
                    "chassis_temp": row.chassis_temp,
                },
                "payload": parse_payload(&row),
            })
        })
        .collect();

    Json(json!({ "hostname": name, "snapshots": snapshots })).into_response()
}

fn fleet_delegation_unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "FoldingOS fleet delegation is unavailable on this host (requires supervisor role with foldingosctl)"
        })),
    )
        .into_response()
}

fn fleet_command_error(error: FleetCommandError) -> Response {
    let (status, code) = match &error {
        FleetCommandError::CommandRejected { code, .. }
            if code == "role_required" || code == "automation_denied" =>
        {
            (StatusCode::FORBIDDEN, code.as_str())
        }
        FleetCommandError::CommandRejected { code, .. } if code == "permission_denied" => {
            (StatusCode::FORBIDDEN, code.as_str())
        }
        FleetCommandError::CommandRejected { code, .. }
            if code == "invalid_input" || code == "not_found" =>
        {
            (StatusCode::BAD_REQUEST, code.as_str())
        }
        _ => (StatusCode::BAD_GATEWAY, "foldingosctl_error"),
    };
    (
        status,
        Json(json!({
            "error": error.to_string(),
            "code": code,
        })),
    )
        .into_response()
}

fn machine_ingest_summary(
    conn: &rusqlite::Connection,
    machine: &MachineRow,
    offline_threshold_ms: u64,
) -> Value {
    let latest = db::get_latest_snapshot(conn, &machine.hostname)
        .ok()
        .flatten();
    json!({
        "hostname": machine.hostname,
        "node_id": machine.node_id,
        "installation_role": machine.installation_role,
        "foldingos_version": machine.foldingos_version,
        "last_seen": machine.last_seen,
        "online": db::is_online(&machine.last_seen, offline_threshold_ms),
        "latest": snapshot_summary(latest.as_ref()),
    })
}

async fn fleet_enrollments(State(state): State<AppState>) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let config = FleetDelegateConfig {
        foldingosctl_path: &state.config.foldingosctl_path,
    };

    let enrollments = match foldingos::list_enrollments(config).await {
        Ok(records) => records,
        Err(error) => return fleet_command_error(error),
    };

    let conn = state.db.lock();
    let correlated: Vec<Value> = enrollments
        .into_iter()
        .map(|record| {
            let machine = db::get_machine_by_node_id(&conn, &record.node_id)
                .ok()
                .flatten()
                .or_else(|| db::get_machine(&conn, &record.hostname).ok().flatten());
            let ingest = machine
                .as_ref()
                .map(|machine| {
                    machine_ingest_summary(&conn, machine, state.config.offline_threshold_ms)
                })
                .unwrap_or(Value::Null);
            json!({
                "node_id": record.node_id,
                "installation_role": record.installation_role,
                "hostname": record.hostname,
                "current_image_version": record.current_image_version,
                "desired_image_version": record.desired_image_version,
                "desired_foldops_manifest_release": empty_as_null(&record.desired_foldops_manifest_release),
                "desired_tools_version": empty_as_null(&record.desired_tools_version),
                "foldingos_version": record.foldingos_version,
                "last_update_status": empty_as_null(&record.last_update_status),
                "registered_at": empty_as_null(&record.registered_at),
                "last_seen_at": empty_as_null(&record.last_seen_at),
                "ingest": ingest,
            })
        })
        .collect();

    Json(json!({ "enrollments": correlated })).into_response()
}

async fn fleet_allow_boot(State(state): State<AppState>) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let config = FleetDelegateConfig {
        foldingosctl_path: &state.config.foldingosctl_path,
    };

    let devices = match foldingos::list_allow_boot(config).await {
        Ok(devices) => devices,
        Err(error) => return fleet_command_error(error),
    };

    let enrollments = match foldingos::list_enrollments(config).await {
        Ok(records) => records,
        Err(error) => return fleet_command_error(error),
    };

    let conn = state.db.lock();
    let correlated = correlate_allow_boot_devices(
        devices,
        &enrollments,
        &conn,
        state.config.offline_threshold_ms,
    );

    Json(json!({ "devices": correlated })).into_response()
}

fn normalize_mac_address(value: &str) -> String {
    let hex: String = value
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase();
    if hex.len() != 12 {
        return value.trim().to_ascii_lowercase();
    }
    hex.as_bytes()
        .chunks(2)
        .map(|pair| std::str::from_utf8(pair).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(":")
}

fn correlate_allow_boot_devices(
    devices: Vec<foldingos::AllowBootDevice>,
    enrollments: &[foldingos::EnrollmentRecord],
    conn: &rusqlite::Connection,
    offline_threshold_ms: u64,
) -> Vec<Value> {
    let mut mac_to_enrollment: std::collections::HashMap<String, &foldingos::EnrollmentRecord> =
        std::collections::HashMap::new();
    for enrollment in enrollments {
        for mac in &enrollment.mac_addresses {
            mac_to_enrollment.insert(normalize_mac_address(mac), enrollment);
        }
    }

    devices
        .into_iter()
        .map(|device| {
            let mac = normalize_mac_address(&device.mac_address);
            let enrollment = mac_to_enrollment.get(&mac).copied();
            let machine = enrollment.and_then(|record| {
                db::get_machine_by_node_id(conn, &record.node_id)
                    .ok()
                    .flatten()
                    .or_else(|| db::get_machine(conn, &record.hostname).ok().flatten())
            });
            let online = machine
                .as_ref()
                .map(|row| db::is_online(&row.last_seen, offline_threshold_ms));
            let primary_ipv4 = machine.as_ref().and_then(|row| {
                db::get_latest_snapshot(conn, &row.hostname)
                    .ok()
                    .flatten()
                    .and_then(|snapshot| parse_payload(&snapshot))
                    .and_then(|payload| payload.primaryIpv4.clone())
            });
            let install_status = if enrollment.is_some() {
                "installed"
            } else {
                "pending"
            };
            let network_status = match (install_status, online) {
                ("pending", _) => "awaiting_install",
                (_, Some(true)) => "online",
                (_, Some(false)) => "offline",
                _ => "installed",
            };
            json!({
                "mac_address": device.mac_address,
                "install_disk": device.install_disk,
                "install_status": install_status,
                "network_status": network_status,
                "hostname": enrollment.map(|record| record.hostname.clone()),
                "node_id": enrollment.map(|record| record.node_id.clone()),
                "primary_ipv4": primary_ipv4,
                "online": online,
                "registered_at": enrollment
                    .map(|record| empty_as_null(&record.registered_at))
                    .unwrap_or(Value::Null),
                "last_seen_at": enrollment
                    .map(|record| empty_as_null(&record.last_seen_at))
                    .unwrap_or(Value::Null),
            })
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct FleetAllowBootBody {
    mac_address: Option<String>,
    install_disk: Option<String>,
}

async fn fleet_allow_boot_create(
    State(state): State<AppState>,
    Json(body): Json<FleetAllowBootBody>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let mac_address = body
        .mac_address
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let Some(mac_address) = mac_address else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "mac_address is required" })),
        )
            .into_response();
    };

    let config = FleetDelegateConfig {
        foldingosctl_path: &state.config.foldingosctl_path,
    };

    match foldingos::provision_allow_boot(
        config,
        AllowBootRequest {
            mac_address,
            install_disk: trim_optional(body.install_disk),
        },
    )
    .await
    {
        Ok(result) => Json(json!({ "ok": true, "result": result })).into_response(),
        Err(error) => fleet_command_error(error),
    }
}

async fn fleet_allow_boot_delete(
    State(state): State<AppState>,
    Json(body): Json<FleetAllowBootBody>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let mac_address = body
        .mac_address
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let Some(mac_address) = mac_address else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "mac_address is required" })),
        )
            .into_response();
    };

    let config = FleetDelegateConfig {
        foldingosctl_path: &state.config.foldingosctl_path,
    };

    match foldingos::provision_deny_boot(config, foldingos::DenyBootRequest { mac_address }).await {
        Ok(result) => Json(json!({ "ok": true, "result": result })).into_response(),
        Err(error) => fleet_command_error(error),
    }
}

async fn fleet_registry(State(state): State<AppState>) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let config = FleetDelegateConfig {
        foldingosctl_path: &state.config.foldingosctl_path,
    };

    match foldingos::list_registry(config).await {
        Ok(versions) => Json(json!({ "versions": versions })).into_response(),
        Err(error) => fleet_command_error(error),
    }
}

async fn fleet_registry_show(
    State(state): State<AppState>,
    Path(version): Path<String>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let version = version.trim();
    if version.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Registry version is required" })),
        )
            .into_response();
    }

    let config = FleetDelegateConfig {
        foldingosctl_path: &state.config.foldingosctl_path,
    };

    match foldingos::show_registry(config, version).await {
        Ok(entry) => Json(json!(entry)).into_response(),
        Err(error) => fleet_command_error(error),
    }
}

#[derive(Debug, Deserialize)]
struct FleetAssignBody {
    local: Option<bool>,
    node_id: Option<String>,
    all: Option<bool>,
    version: Option<String>,
    foldops_manifest: Option<String>,
    tools_version: Option<String>,
}

async fn fleet_assign(
    State(state): State<AppState>,
    Json(body): Json<FleetAssignBody>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let mut local = body.local.unwrap_or(false);
    let all = body.all.unwrap_or(false);
    let node_id = body
        .node_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let image_version = trim_optional(body.version);
    let foldops_manifest = trim_optional(body.foldops_manifest);
    let tools_version = trim_optional(body.tools_version);

    if !local
        && !all
        && node_id.is_none()
        && (foldops_manifest.is_some() || tools_version.is_some())
    {
        local = true;
    }

    if local && all {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Use either local=true or all=true, not both" })),
        )
            .into_response();
    }
    if local && node_id.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Use either local=true or node_id, not both" })),
        )
            .into_response();
    }
    if all && node_id.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Use either all=true or node_id, not both" })),
        )
            .into_response();
    }
    if !all && node_id.is_none() && !local {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Assignment requires local=true, all=true, or node_id" })),
        )
            .into_response();
    }

    if image_version.is_none() && foldops_manifest.is_none() && tools_version.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Assignment requires at least one of version, foldops_manifest, or tools_version" })),
        )
            .into_response();
    }

    let config = FleetDelegateConfig {
        foldingosctl_path: &state.config.foldingosctl_path,
    };

    let request_detail = json!({
        "local": local,
        "all": all,
        "node_id": node_id,
        "foldops_manifest": foldops_manifest,
        "tools_version": tools_version,
        "image_version": image_version,
    });

    if let Some(release) = foldops_manifest.as_deref() {
        if let Err(error) = ensure_foldops_release_imported(&state.config, release).await {
            install_log::append_event(
                "api",
                "fleet_assign",
                "POST /software/fleet/assign",
                false,
                None,
                &error,
                "",
                "",
                Some(request_detail.clone()),
            );
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response();
        }
    }

    if let Some(version) = tools_version.as_deref() {
        if let Err(error) = ensure_tools_release_imported(&state.config, version).await {
            install_log::append_event(
                "api",
                "fleet_assign",
                "POST /software/fleet/assign",
                false,
                None,
                &error,
                "",
                "",
                Some(request_detail.clone()),
            );
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response();
        }
    }

    let assign_request = AssignRequest {
        node_id,
        all,
        image_version: image_version.clone(),
        foldops_manifest_release: foldops_manifest.clone(),
        tools_version: tools_version.clone(),
    };

    let assign_result = if local {
        foldingos::provision_assign_local(config, assign_request).await
    } else {
        foldingos::provision_assign(config, assign_request).await
    };

    match assign_result {
        Ok(result) => {
            install_log::append_event(
                "api",
                "fleet_assign",
                "POST /fleet/assign",
                true,
                None,
                "assignment completed",
                "",
                "",
                Some(json!({ "request": request_detail, "result": result })),
            );
            Json(json!({ "ok": true, "result": result })).into_response()
        }
        Err(error) => {
            install_log::append_event(
                "api",
                "fleet_assign",
                "POST /fleet/assign",
                false,
                None,
                &error.to_string(),
                "",
                "",
                Some(request_detail),
            );
            fleet_command_error(error)
        }
    }
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn empty_as_null(value: &str) -> Value {
    if value.trim().is_empty() {
        Value::Null
    } else {
        Value::String(value.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct SoftwareUpdatesQuery {
    refresh: Option<bool>,
}

async fn software_updates(
    State(state): State<AppState>,
    Query(query): Query<SoftwareUpdatesQuery>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let refresh = query.refresh.unwrap_or(false);
    match software::build_updates_response(&state.db, &state.config, &state.software, refresh).await
    {
        Ok(body) => Json(body).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

async fn fleet_software_apply_foldops(
    State(state): State<AppState>,
    Json(body): Json<FleetSoftwareApplyRequest>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    match fleet_apply_foldops(&state.db, &state.config, body).await {
        Ok(body) => Json(body).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response(),
    }
}

async fn fleet_software_apply_tools(
    State(state): State<AppState>,
    Json(body): Json<FleetSoftwareApplyRequest>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    match fleet_apply_tools(&state.db, &state.config, body).await {
        Ok(body) => Json(body).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response(),
    }
}

async fn software_apply_local(
    State(state): State<AppState>,
    Json(body): Json<ApplyLocalRequest>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let request_detail = serde_json::to_value(&body).unwrap_or(Value::Null);
    match apply_local(&state.config, body).await {
        Ok(response) => {
            install_log::append_event(
                "api",
                "apply_local",
                "POST /software/apply-local",
                true,
                None,
                "apply-local completed",
                "",
                "",
                Some(json!({ "request": request_detail, "response": response })),
            );
            Json(response).into_response()
        }
        Err(error) => {
            install_log::append_event(
                "api",
                "apply_local",
                "POST /software/apply-local",
                false,
                None,
                &error,
                "",
                "",
                Some(json!({ "request": request_detail })),
            );
            (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct InstallLogQuery {
    limit: Option<usize>,
}

async fn software_install_log(Query(query): Query<InstallLogQuery>) -> Response {
    match install_log::list_response(query.limit) {
        Ok(body) => Json(body).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct SupervisorLogsQuery {
    source: Option<String>,
    lines: Option<u32>,
}

async fn supervisor_logs(
    State(state): State<AppState>,
    Query(query): Query<SupervisorLogsQuery>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let source_str = query.source.as_deref().unwrap_or("foldops");
    let Some(source) = SupervisorLogSource::parse(source_str) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "source must be foldops or foldingosctl" })),
        )
            .into_response();
    };
    let lines = query.lines.unwrap_or(300).clamp(1, 500);

    match fetch_supervisor_logs(source, lines).await {
        Ok((path, log_lines)) => Json(json!({
            "source": source_str,
            "lines": log_lines,
            "path": path,
            "updated_at": chrono::Utc::now().to_rfc3339(),
            "live": true,
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(source = source_str, error = %error, "supervisor log fetch failed");
            (StatusCode::BAD_GATEWAY, Json(json!({ "error": error }))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct RecoveryExportBody {
    #[serde(default)]
    include_secrets: bool,
}

async fn recovery_export_create(
    State(state): State<AppState>,
    Json(body): Json<RecoveryExportBody>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    match recovery::create_export(&state.config, body.include_secrets).await {
        Ok(body) => Json(body).into_response(),
        Err(error) => {
            tracing::error!(error = %error, "recovery export failed");
            if error.contains("foldingosctl") {
                return (StatusCode::BAD_GATEWAY, Json(json!({ "error": error }))).into_response();
            }
            (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
        }
    }
}

async fn recovery_export_latest(State(state): State<AppState>) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let backups_dir = std::path::Path::new(BACKUPS_DIR);
    let path = match recovery::latest_backup_path(backups_dir) {
        Ok(path) => path,
        Err(error) => {
            return (StatusCode::NOT_FOUND, Json(json!({ "error": error }))).into_response();
        }
    };

    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("foldingos-supervisor-backup.tar.zst");
    let file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("open backup archive: {error}") })),
            )
                .into_response();
        }
    };
    let stream = tokio_util::io::ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zstd")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(body)
        .unwrap_or_else(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("build download response: {error}") })),
            )
                .into_response()
        })
}

#[derive(Debug, Deserialize)]
struct ServicesRestartBody {
    unit: String,
}

async fn services_list(State(state): State<AppState>) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    match services::list_services(&state.config).await {
        Ok(body) => Json(body).into_response(),
        Err(error) => {
            tracing::error!(error = %error, "services list failed");
            if error.contains("foldingosctl") {
                return (StatusCode::BAD_GATEWAY, Json(json!({ "error": error }))).into_response();
            }
            (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
        }
    }
}

async fn services_restart(
    State(state): State<AppState>,
    Json(body): Json<ServicesRestartBody>,
) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    let unit = body.unit.trim();
    if unit.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "unit is required" })),
        )
            .into_response();
    }

    match services::restart_service(&state.config, unit).await {
        Ok(body) => Json(body).into_response(),
        Err(error) => {
            tracing::error!(unit = %unit, error = %error, "service restart failed");
            if error.contains("foldingosctl") {
                return (StatusCode::BAD_GATEWAY, Json(json!({ "error": error }))).into_response();
            }
            (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
        }
    }
}

async fn services_restart_all(State(state): State<AppState>) -> Response {
    if !state.config.uses_supervisor_fleet_delegation() {
        return fleet_delegation_unavailable();
    }

    match services::restart_all_services(&state.config).await {
        Ok(body) => Json(body).into_response(),
        Err(error) => {
            tracing::error!(error = %error, "restart all services failed");
            if error.contains("foldingosctl") {
                return (StatusCode::BAD_GATEWAY, Json(json!({ "error": error }))).into_response();
            }
            (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
        }
    }
}

pub fn spawn_alert_eval(state: AppState) {
    let db = state.db.clone();
    let config = state.config.alert_config.clone();
    tokio::spawn(async move {
        crate::alerts::engine::run_alert_evaluation(&db, &config).await;
    });
}
