use foldops_types::ControlAction;
use serde::Serialize;

use crate::fah::{send_fah_finish, send_fah_pause, send_fah_resume};

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
}

pub struct ControlContext {
    pub allow_reboot: bool,
    pub fah_ws_host: String,
    pub fah_ws_port: u16,
}

pub async fn get_control_status() -> ControlStatus {
    ControlStatus {
        foldops_agent: systemd_is_active("foldops-agent").await,
        fah_client: systemd_is_active("fah-client").await,
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

pub async fn execute_control_action(
    action: ControlAction,
    ctx: &ControlContext,
) -> ControlResult {
    let action_str = action.as_str().to_string();

    let fail = |message: String, stdout: String, stderr: String| ControlResult {
        ok: false,
        action: action_str.clone(),
        message,
        stdout,
        stderr,
    };

    match action {
        ControlAction::AgentStart => match run_systemctl("start", "foldops-agent").await {
            Ok((stdout, stderr)) => ControlResult {
                ok: true,
                action: action_str,
                message: "foldops-agent started".into(),
                stdout,
                stderr,
            },
            Err(e) => fail(e, String::new(), String::new()),
        },
        ControlAction::AgentStop => match run_systemctl("stop", "foldops-agent").await {
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
        ControlAction::FahStart => match run_systemctl("start", "fah-client").await {
            Ok((stdout, stderr)) => ControlResult {
                ok: true,
                action: action_str,
                message: "fah-client started".into(),
                stdout,
                stderr,
            },
            Err(e) => fail(e, String::new(), String::new()),
        },
        ControlAction::FahStop => match run_systemctl("stop", "fah-client").await {
            Ok((stdout, stderr)) => ControlResult {
                ok: true,
                action: action_str,
                message: "fah-client stopped".into(),
                stdout,
                stderr,
            },
            Err(e) => fail(e, String::new(), String::new()),
        },
        ControlAction::FahRestart => match run_systemctl("restart", "fah-client").await {
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
        ControlAction::FahResume => match send_fah_resume(&ctx.fah_ws_host, ctx.fah_ws_port).await {
            Ok(()) => ControlResult {
                ok: true,
                action: action_str,
                message: "FAH folding resumed".into(),
                stdout: String::new(),
                stderr: String::new(),
            },
            Err(msg) => fail(msg.clone(), String::new(), msg),
        },
        ControlAction::FahFinish => match send_fah_finish(&ctx.fah_ws_host, ctx.fah_ws_port).await {
            Ok(()) => ControlResult {
                ok: true,
                action: action_str,
                message: "FAH finish command sent (completes WU then pauses)".into(),
                stdout: String::new(),
                stderr: String::new(),
            },
            Err(msg) => fail(msg.clone(), String::new(), msg),
        },
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
