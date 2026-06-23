use std::path::PathBuf;

use foldops_types::ControlAction;
use serde::Serialize;

use crate::fah::collect_fah_status;
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
    pub fah_log_path: PathBuf,
    pub fah_db_path: PathBuf,
    pub fah_work_dir: PathBuf,
    pub fah_ws_host: String,
    pub fah_ws_port: u16,
}

pub async fn get_control_status(ctx: &ControlContext) -> ControlStatus {
    let foldops_agent = systemd_is_active(FOLDOPS_AGENT_UNIT).await;
    let fah_client = systemd_is_active(FAH_CLIENT_UNIT).await;
    let (fah_folding_state, fah_unit_state, fah_folding_detail) = if fah_client == "active" {
        let collected = collect_fah_status(
            &ctx.fah_log_path,
            &ctx.fah_db_path,
            &ctx.fah_work_dir,
            &ctx.fah_ws_host,
            ctx.fah_ws_port,
        )
        .await;
        folding_status_from_collected_state(collected.state)
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

fn folding_status_from_collected_state(
    state: crate::fah::FahLogState,
) -> (String, Option<String>, Option<String>) {
    (
        state
            .folding_state
            .unwrap_or_else(|| "unknown".into()),
        state.unit_state,
        state.folding_detail,
    )
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

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        return Ok((stdout, stderr));
    }

    let message = if stderr.is_empty() {
        format!("systemctl {action} {unit} failed ({})", output.status)
    } else {
        stderr.clone()
    };
    Err(message)
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
