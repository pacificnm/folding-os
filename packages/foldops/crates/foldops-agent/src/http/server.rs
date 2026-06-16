use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use foldops_types::ControlAction;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::config::Config;
use crate::fah::get_newest_work_log_path;
use crate::log_tail::read_log_tail_default;
use crate::node_control::{
    execute_control_action, get_control_status, schedule_agent_self_restart, ControlContext,
};
use crate::update::{is_update_in_flight, run_agent_update, schedule_post_update_restart};

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
}

#[derive(Debug, Deserialize)]
struct LogQuery {
    lines: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ControlBody {
    action: Option<String>,
}

pub async fn start_agent_http(config: Arc<Config>) {
    if config.agent_http_port == 0 {
        return;
    }

    let state = AppState { config: config.clone() };
    let app = Router::new()
        .route("/logs/fah", get(logs_fah))
        .route("/logs/work", get(logs_work))
        .route("/control/status", get(control_status))
        .route("/control", post(control_action))
        .route("/update", post(update_agent))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.agent_http_port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(addr = %addr, error = %e, "failed to bind agent HTTP");
            return;
        }
    };

    tracing::info!(addr = %addr, "FoldOps agent HTTP listening");

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!(error = %e, "agent HTTP server exited");
    }
}

async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    let authorized = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|h| h == format!("Bearer {}", state.config.agent_token));

    if authorized {
        next.run(req).await
    } else {
        json_error(StatusCode::UNAUTHORIZED, "Unauthorized")
    }
}

async fn logs_fah(
    State(state): State<AppState>,
    Query(q): Query<LogQuery>,
) -> Response {
    let lines = clamp_lines(q.lines);
    match read_log_tail_default(&state.config.fah_log_path, lines).await {
        Some(tail) => Json(serde_json::json!({
            "source": "fah",
            "path": tail.path,
            "lines": tail.lines,
        }))
        .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "FAH log not readable",
                "path": state.config.fah_log_path,
            })),
        )
            .into_response(),
    }
}

async fn logs_work(State(state): State<AppState>, Query(q): Query<LogQuery>) -> Response {
    let lines = clamp_lines(q.lines);
    let work_path = match get_newest_work_log_path(&state.config.fah_work_dir).await {
        Some(p) => p,
        None => return json_error(StatusCode::NOT_FOUND, "No work unit log found"),
    };

    match read_log_tail_default(&work_path, lines).await {
        Some(tail) => Json(serde_json::json!({
            "source": "work",
            "path": tail.path,
            "lines": tail.lines,
        }))
        .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Work log not readable",
                "path": work_path,
            })),
        )
            .into_response(),
    }
}

async fn control_status(State(state): State<AppState>) -> Response {
    if !state.config.controls_enabled {
        return json_error(
            StatusCode::FORBIDDEN,
            "Controls disabled (set CONTROLS_ENABLED=true)",
        );
    }
    Json(get_control_status().await).into_response()
}

async fn control_action(State(state): State<AppState>, Json(body): Json<ControlBody>) -> Response {
    if !state.config.controls_enabled {
        return json_error(
            StatusCode::FORBIDDEN,
            "Controls disabled (set CONTROLS_ENABLED=true)",
        );
    }

    let Some(action_str) = body.action else {
        return json_error(StatusCode::BAD_REQUEST, "Invalid or missing action");
    };

    let Ok(action) = ControlAction::try_from(action_str.as_str()) else {
        return json_error(StatusCode::BAD_REQUEST, "Invalid or missing action");
    };

    let ctx = ControlContext {
        allow_reboot: state.config.controls_allow_reboot,
        fah_ws_host: state.config.fah_ws_host.clone(),
        fah_ws_port: state.config.fah_ws_port,
    };

    let result = execute_control_action(action, &ctx).await;
    let restart = result.ok && action == ControlAction::AgentRestart;
    let response = Json(result).into_response();

    if restart {
        schedule_agent_self_restart();
    }

    response
}

#[derive(Serialize)]
struct UpdateResponse {
    ok: bool,
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    restarting: Option<bool>,
}

async fn update_agent(State(state): State<AppState>) -> Response {
    if !state.config.update_enabled {
        return json_error(
            StatusCode::FORBIDDEN,
            "Updates disabled (set UPDATE_ENABLED=true)",
        );
    }

    if is_update_in_flight() {
        return json_error(StatusCode::CONFLICT, "Update already in progress");
    }

    match run_agent_update(&state.config.foldops_root, &state.config.update_script).await {
        Ok(result) if result.ok => {
            let body = UpdateResponse {
                ok: true,
                exit_code: 0,
                stdout: result.stdout,
                stderr: result.stderr,
                duration_ms: result.duration_ms,
                restarting: Some(true),
            };
            schedule_post_update_restart();
            Json(body).into_response()
        }
        Ok(result) => Json(UpdateResponse {
            ok: false,
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
            duration_ms: result.duration_ms,
            restarting: None,
        })
        .into_response(),
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

fn clamp_lines(lines: Option<usize>) -> usize {
    lines.unwrap_or(200).clamp(1, 500)
}

fn json_error(status: StatusCode, error: &str) -> Response {
    (status, Json(serde_json::json!({ "error": error }))).into_response()
}
