use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use foldops_types::{is_control_action, validate_ingest_payload, IngestPayload};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::agent::control::{fetch_agent_control_status, push_agent_control};
use crate::agent::logs::{fetch_live_agent_logs, LogSource};
use crate::alerts::engine::{
    alerts_status_json, list_active_alerts_json, list_alert_history_json, run_test_alert,
};
use crate::config::Config;
use crate::db::{self, Db, SnapshotRow};
use crate::deploy::db::{get_deploy_run, list_deploy_runs};
use crate::deploy::start_agent_deploy;
use crate::fah_projects::fetch_fah_project;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<Config>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/ingest", post(ingest))
        .route("/machines", get(list_machines))
        .route("/machines/{name}", get(get_machine))
        .route("/machines/{name}/logs", get(machine_logs))
        .route("/machines/{name}/control/status", get(control_status))
        .route("/machines/{name}/control", post(control_action))
        .route("/deploy/runs", get(deploy_runs))
        .route("/deploy/runs/{id}", get(deploy_run))
        .route("/deploy/agents", post(deploy_agents))
        .route("/alerts/status", get(alerts_status))
        .route("/alerts/test", post(alerts_test))
        .route("/alerts/history", get(alerts_history))
        .route("/alerts", get(alerts_active))
        .route("/projects/{id}", get(project_detail))
        .route("/snapshots/{name}", get(snapshots))
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
    let latest = db::get_latest_snapshot(&conn, &machine.hostname).ok().flatten();
    Json(json!({
        "hostname": machine.hostname,
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

        let latest = db::get_latest_snapshot(&conn, &machine.hostname).ok().flatten();
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
                tracing::warn!(hostname = %ctx.hostname, source = source_str, error = %live_error, "live log fetch failed");
                let slice = ctx.cached_lines[ctx.cached_lines.len().saturating_sub(lines as usize)..].to_vec();
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
        }))
        .into_response(),
        Err(msg) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": msg })),
        )
            .into_response(),
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
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "error": msg })),
                )
                    .into_response()
            }
        }
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
        Err(msg) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": msg })),
        )
            .into_response(),
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
        Err(msg) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": msg })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    status: Option<String>,
    limit: Option<i64>,
    hostname: Option<String>,
}

async fn alerts_history(State(state): State<AppState>, Query(q): Query<HistoryQuery>) -> Json<Value> {
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
    Json(list_alert_history_json(
        &state.db,
        limit,
        status,
        hostname,
    ))
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

pub fn spawn_alert_eval(state: AppState) {
    let db = state.db.clone();
    let config = state.config.alert_config.clone();
    tokio::spawn(async move {
        crate::alerts::engine::run_alert_evaluation(&db, &config).await;
    });
}
