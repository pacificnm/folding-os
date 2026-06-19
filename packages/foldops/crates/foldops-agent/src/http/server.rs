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
use crate::foldingos::{
    activate_foldinghome_config, foldops_acquire, set_fah_passkey, sync_software_assignments,
    tools_acquire, write_foldinghome_candidate, AutomationCommandError,
};
use crate::ingest::IngestClient;
use crate::log_tail::read_log_tail_default;
use crate::node_control::{
    execute_control_action, get_control_status, schedule_agent_self_restart, ControlContext,
};
use crate::update::{is_update_in_flight, run_agent_update, schedule_post_update_restart};

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    ingest: Arc<IngestClient>,
}

#[derive(Debug, Deserialize)]
struct LogQuery {
    lines: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ControlBody {
    action: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FoldinghomeConfigBody {
    config: String,
    #[serde(default)]
    passkey: String,
}

#[derive(Serialize)]
struct FoldinghomeConfigResponse {
    ok: bool,
    domain: &'static str,
    candidate: String,
    activated: bool,
}

pub async fn start_agent_http(config: Arc<Config>, ingest: Arc<IngestClient>) {
    if config.agent_http_port == 0 {
        return;
    }

    let state = AppState {
        config: config.clone(),
        ingest,
    };
    let app = Router::new()
        .route("/logs/fah", get(logs_fah))
        .route("/logs/work", get(logs_work))
        .route("/control/status", get(control_status))
        .route("/control", post(control_action))
        .route("/config/foldinghome", post(foldinghome_config))
        .route("/inspect/foldops", get(inspect_foldops))
        .route("/inspect/tools", get(inspect_tools))
        .route("/software/foldops-acquire", post(software_foldops_acquire))
        .route("/software/tools-acquire", post(software_tools_acquire))
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

async fn logs_fah(State(state): State<AppState>, Query(q): Query<LogQuery>) -> Response {
    let lines = clamp_lines(q.lines);
    match read_log_tail_default(&state.config.fah_log_path, lines).await {
        Some(tail) => Json(serde_json::json!({
            "source": "fah",
            "path": tail.path,
            "lines": tail.lines,
        }))
        .into_response(),
        None => Json(serde_json::json!({
            "source": "fah",
            "path": state.config.fah_log_path,
            "lines": [],
        }))
        .into_response(),
    }
}

async fn logs_work(State(state): State<AppState>, Query(q): Query<LogQuery>) -> Response {
    let lines = clamp_lines(q.lines);
    let work_path = match get_newest_work_log_path(&state.config.fah_work_dir).await {
        Some(p) => p,
        None => state.config.fah_log_path.clone(),
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
    Json(
        get_control_status(&ControlContext {
            allow_reboot: state.config.controls_allow_reboot,
            fah_ws_host: state.config.fah_ws_host.clone(),
            fah_ws_port: state.config.fah_ws_port,
        })
        .await,
    )
    .into_response()
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

async fn foldinghome_config(
    State(state): State<AppState>,
    Json(body): Json<FoldinghomeConfigBody>,
) -> Response {
    if !state.config.config_enabled {
        return json_error(StatusCode::FORBIDDEN, "Remote config push disabled");
    }

    if body.config.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "Missing config body");
    }

    let passkey = body.passkey.trim();
    if !passkey.is_empty() {
        tracing::info!("writing Folding@home passkey secret before config activate");
        match set_fah_passkey(&state.config.foldingosctl_path, passkey).await {
            Ok(_) => {}
            Err(error) => {
                return json_error(StatusCode::BAD_REQUEST, &error.to_string());
            }
        }
    }

    let candidate_path = match write_foldinghome_candidate(body.config.trim()) {
        Ok(path) => path,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error),
    };

    tracing::info!(
        candidate = %candidate_path.display(),
        "delegating foldinghome config activate to foldingosctl"
    );

    match activate_foldinghome_config(&state.config.foldingosctl_path, &candidate_path).await {
        Ok(data) => {
            let candidate = data
                .get("candidate")
                .and_then(|value| value.as_str())
                .unwrap_or(candidate_path.to_str().unwrap_or_default())
                .to_string();
            let activated = data
                .get("activated")
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            tracing::info!(
                candidate = %candidate,
                activated,
                "foldinghome config activate succeeded"
            );
            if let Err(error) = state.ingest.collect_and_post().await {
                tracing::warn!(error = %error, "post-config ingest failed");
            }
            Json(FoldinghomeConfigResponse {
                ok: true,
                domain: "foldinghome",
                candidate,
                activated,
            })
            .into_response()
        }
        Err(AutomationCommandError::CommandRejected {
            command,
            code,
            message,
        }) => {
            tracing::warn!(
                command = %command,
                code = %code,
                message = %message,
                candidate = %candidate_path.display(),
                "foldinghome config activate rejected"
            );
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": message,
                    "code": code,
                    "command": command,
                })),
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!(
                error = %error,
                candidate = %candidate_path.display(),
                "foldinghome config activate failed"
            );
            json_error(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string())
        }
    }
}

async fn inspect_foldops(State(state): State<AppState>) -> Response {
    if !state.config.uses_foldingos_delegation() {
        return json_error(
            StatusCode::FORBIDDEN,
            "FoldingOS delegation not enabled on this agent",
        );
    }

    match crate::foldingos::inspect_subcommand(&state.config.foldingosctl_path, "foldops").await {
        Ok(data) => Json(serde_json::json!({ "data": data })).into_response(),
        Err(error) => json_error(StatusCode::BAD_GATEWAY, &error.to_string()),
    }
}

async fn inspect_tools(State(state): State<AppState>) -> Response {
    if !state.config.uses_foldingos_delegation() {
        return json_error(
            StatusCode::FORBIDDEN,
            "FoldingOS delegation not enabled on this agent",
        );
    }

    match crate::foldingos::inspect_subcommand(&state.config.foldingosctl_path, "tools").await {
        Ok(data) => Json(serde_json::json!({ "data": data })).into_response(),
        Err(error) => json_error(StatusCode::BAD_GATEWAY, &error.to_string()),
    }
}

#[derive(Serialize)]
struct SoftwareAcquireResponse {
    ok: bool,
    data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    restarted: Option<bool>,
}

async fn software_foldops_acquire(State(state): State<AppState>) -> Response {
    if !state.config.uses_foldingos_delegation() {
        return json_error(
            StatusCode::FORBIDDEN,
            "FoldingOS delegation not enabled on this agent",
        );
    }

    if let Err(error) = sync_software_assignments(&state.config.foldingosctl_path).await {
        return json_error(StatusCode::BAD_GATEWAY, &error.to_string());
    }

    match foldops_acquire(&state.config.foldingosctl_path).await {
        Ok(data) => {
            let acquired = data.get("acquired").and_then(|value| value.as_bool()) == Some(true);
            let restarted = acquired.then_some(true);
            Json(SoftwareAcquireResponse {
                ok: true,
                data,
                restarted,
            })
            .into_response()
        }
        Err(AutomationCommandError::CommandRejected {
            command,
            code,
            message,
        }) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": message,
                "code": code,
                "command": command,
            })),
        )
            .into_response(),
        Err(error) => json_error(StatusCode::BAD_GATEWAY, &error.to_string()),
    }
}

async fn software_tools_acquire(State(state): State<AppState>) -> Response {
    if !state.config.uses_foldingos_delegation() {
        return json_error(
            StatusCode::FORBIDDEN,
            "FoldingOS delegation not enabled on this agent",
        );
    }

    if let Err(error) = sync_software_assignments(&state.config.foldingosctl_path).await {
        return json_error(StatusCode::BAD_GATEWAY, &error.to_string());
    }

    match tools_acquire(&state.config.foldingosctl_path).await {
        Ok(data) => {
            let acquired = data.get("acquired").and_then(|value| value.as_bool()) == Some(true);
            let restarted = acquired.then_some(true);
            Json(SoftwareAcquireResponse {
                ok: true,
                data,
                restarted,
            })
            .into_response()
        }
        Err(AutomationCommandError::CommandRejected {
            command,
            code,
            message,
        }) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": message,
                "code": code,
                "command": command,
            })),
        )
            .into_response(),
        Err(error) => json_error(StatusCode::BAD_GATEWAY, &error.to_string()),
    }
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
