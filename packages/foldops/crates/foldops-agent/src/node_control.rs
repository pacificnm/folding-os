use foldops_types::ControlAction;
use serde::Serialize;

use crate::fah::query_fah_websocket_activity;
use crate::fah::{send_fah_finish, send_fah_pause, send_fah_resume};
use crate::foldingos::{FAH_CLIENT_UNIT, FOLDOPS_AGENT_UNIT};

#[derive(Debug, Clone, Serialize)]
pub struct ControlResult {
    pub ok: bool,
    pub action: String,
    pub message: String,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlStatus {
    pub foldops_agent: String,
    pub fah_client: String,
    pub fah_folding_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fah_unit_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fah_folding_detail: Option<String>,
}

pub struct ControlContext {
    pub allow_reboot: bool,
    pub fah_ws_host: String,
    pub fah_ws_port: u16,
}

pub async fn get_control_status(ctx: &ControlContext) -> ControlStatus {
    let foldops_agent = systemd_is_active(FOLDOPS_AGENT_UNIT).await;
    let fah_client = systemd_is_active(FAH_CLIENT_UNIT).await;
    let (fah_folding_state, fah_unit_state, fah_folding_detail) = if fah_client == "active" {
        summarize_fah_folding(query_fah_websocket_activity(&ctx.fah_ws_host, ctx.fah_ws_port).await)
    } else {
        (
            "stopped".into(),
            None,
            Some("FAH service is not running".into()),
        )
    };

    ControlStatus {
        foldops_agent,
        fah_client,
        fah_folding_state,
        fah_unit_state,
        fah_folding_detail,
    }
}

fn summarize_fah_folding(
    activity: Option<crate::fah::FahWsActivity>,
) -> (String, Option<String>, Option<String>) {
    let Some(activity) = activity else {
        return (
            "unreachable".into(),
            None,
            Some("FAH WebSocket unavailable on port 7396".into()),
        );
    };

    let unit_state = activity.unit_state.trim().to_uppercase();
    let detail = activity
        .detail
        .clone()
        .or_else(|| format_fah_activity_detail(&activity));
    let fah_folding_state = match unit_state.as_str() {
        "RUN" if activity.project.is_some() => "folding".to_string(),
        "RUN" => "waiting".to_string(),
        "PAUSE" => "paused".to_string(),
        "FINISH" => "finishing".to_string(),
        "DOWNLOAD" | "UPLOAD" | "READY" | "CORE" => unit_state.to_lowercase(),
        "" => "idle".to_string(),
        other => other.to_lowercase(),
    };

    (
        fah_folding_state,
        if unit_state.is_empty() {
            None
        } else {
            Some(unit_state)
        },
        detail,
    )
}

fn format_fah_activity_detail(activity: &crate::fah::FahWsActivity) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(project) = activity.project.as_deref() {
        parts.push(format!("project {project}"));
    }
    if let Some(progress) = activity.progress {
        parts.push(format!("{progress:.1}%"));
    }
    if let Some(ppd) = activity.ppd {
        parts.push(format!("{} PPD", format_ppd(ppd)));
    }
    if parts.is_empty() {
        if activity.unit_state.eq_ignore_ascii_case("RUN") {
            Some("No work unit assigned".into())
        } else {
            None
        }
    } else {
        Some(parts.join(" · "))
    }
}

fn format_ppd(ppd: f64) -> String {
    if ppd >= 1_000_000.0 {
        format!("{:.2}M", ppd / 1_000_000.0)
    } else if ppd >= 1_000.0 {
        format!("{:.0}k", ppd / 1_000.0)
    } else {
        format!("{ppd:.0}")
    }
}

async fn systemd_is_active(unit: &str) -> String {
    match tokio::process::Command::new("systemctl")
        .args(["is-active", unit])
        .output()
        .await
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "inactive".into(),
    }
}

async fn run_systemctl(action: &str, unit: &str) -> Result<(String, String), String> {
    let output = tokio::process::Command::new("systemctl")
        .args([action, unit])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    Ok((
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    ))
}

pub async fn execute_control_action(action: ControlAction, ctx: &ControlContext) -> ControlResult {
    let action_str = action.as_str().to_string();

    let fail = |message: String, stdout: String, stderr: String| ControlResult {
        ok: false,
        action: action_str.clone(),
        message,
        stdout,
        stderr,
    };

    match action {
        ControlAction::AgentStart => match run_systemctl("start", FOLDOPS_AGENT_UNIT).await {
            Ok((stdout, stderr)) => ControlResult {
                ok: true,
                action: action_str,
                message: "foldops-agent started".into(),
                stdout,
                stderr,
            },
            Err(e) => fail(e, String::new(), String::new()),
        },
        ControlAction::AgentStop => match run_systemctl("stop", FOLDOPS_AGENT_UNIT).await {
            Ok((stdout, stderr)) => ControlResult {
                ok: true,
                action: action_str,
                message: "foldops-agent stopped".into(),
                stdout,
                stderr,
            },
            Err(e) => fail(e, String::new(), String::new()),
        },
        ControlAction::AgentRestart => ControlResult {
            ok: true,
            action: action_str,
            message: "foldops-agent will restart".into(),
            stdout: String::new(),
            stderr: String::new(),
        },
        ControlAction::FahStart => match run_systemctl("start", FAH_CLIENT_UNIT).await {
            Ok((stdout, stderr)) => ControlResult {
                ok: true,
                action: action_str,
                message: "fah-client started".into(),
                stdout,
                stderr,
            },
            Err(e) => fail(e, String::new(), String::new()),
        },
        ControlAction::FahStop => match run_systemctl("stop", FAH_CLIENT_UNIT).await {
            Ok((stdout, stderr)) => ControlResult {
                ok: true,
                action: action_str,
                message: "fah-client stopped".into(),
                stdout,
                stderr,
            },
            Err(e) => fail(e, String::new(), String::new()),
        },
        ControlAction::FahRestart => match run_systemctl("restart", FAH_CLIENT_UNIT).await {
            Ok((stdout, stderr)) => ControlResult {
                ok: true,
                action: action_str,
                message: "fah-client restarted".into(),
                stdout,
                stderr,
            },
            Err(e) => fail(e, String::new(), String::new()),
        },
        ControlAction::FahPause => match send_fah_pause(&ctx.fah_ws_host, ctx.fah_ws_port).await {
            Ok(()) => ControlResult {
                ok: true,
                action: action_str,
                message: "FAH pause command sent".into(),
                stdout: String::new(),
                stderr: String::new(),
            },
            Err(msg) => fail(msg.clone(), String::new(), msg),
        },
        ControlAction::FahResume => {
            match send_fah_resume(&ctx.fah_ws_host, ctx.fah_ws_port).await {
                Ok(()) => ControlResult {
                    ok: true,
                    action: action_str,
                    message: "FAH folding resumed".into(),
                    stdout: String::new(),
                    stderr: String::new(),
                },
                Err(msg) => fail(msg.clone(), String::new(), msg),
            }
        }
        ControlAction::FahFinish => {
            match send_fah_finish(&ctx.fah_ws_host, ctx.fah_ws_port).await {
                Ok(()) => ControlResult {
                    ok: true,
                    action: action_str,
                    message: "FAH finish command sent (completes WU then pauses)".into(),
                    stdout: String::new(),
                    stderr: String::new(),
                },
                Err(msg) => fail(msg.clone(), String::new(), msg),
            }
        }
        ControlAction::HostReboot => {
            if !ctx.allow_reboot {
                return fail(
                    "Host reboot disabled (set CONTROLS_ALLOW_REBOOT=true)".into(),
                    String::new(),
                    String::new(),
                );
            }
            let _ = tokio::process::Command::new("systemctl")
                .arg("reboot")
                .output()
                .await;
            ControlResult {
                ok: true,
                action: action_str,
                message: "Reboot initiated".into(),
                stdout: String::new(),
                stderr: String::new(),
            }
        }
    }
}

pub fn schedule_agent_self_restart() {
    crate::update::schedule_agent_self_restart();
}
