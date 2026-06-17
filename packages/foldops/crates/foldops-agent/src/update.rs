use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static UPDATE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

pub struct UpdateResult {
    pub ok: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

pub fn is_update_in_flight() -> bool {
    UPDATE_IN_FLIGHT.load(Ordering::SeqCst)
}

pub async fn run_agent_update(root: &Path, script_path: &Path) -> Result<UpdateResult, String> {
    if UPDATE_IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return Err("Update already in progress".into());
    }

    let started = Instant::now();
    let result = run_agent_update_inner(root, script_path, started).await;
    UPDATE_IN_FLIGHT.store(false, Ordering::SeqCst);
    result
}

async fn run_agent_update_inner(
    root: &Path,
    script_path: &Path,
    started: Instant,
) -> Result<UpdateResult, String> {
    let meta = tokio::fs::metadata(script_path).await.map_err(|_| {
        format!(
            "Update script not found or not executable: {}",
            script_path.display()
        )
    })?;

    if !meta.is_file() {
        return Err(format!(
            "Update script not found or not executable: {}",
            script_path.display()
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 == 0 {
            return Err(format!(
                "Update script not found or not executable: {}",
                script_path.display()
            ));
        }
    }

    let output = tokio::process::Command::new("bash")
        .arg(script_path)
        .current_dir(root)
        .env("FOLDOPS_ROOT", root)
        .output()
        .await;

    let duration_ms = started.elapsed().as_millis() as u64;

    match output {
        Ok(o) if o.status.success() => Ok(UpdateResult {
            ok: true,
            exit_code: 0,
            stdout: String::from_utf8_lossy(&o.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).trim().to_string(),
            duration_ms,
        }),
        Ok(o) => Ok(UpdateResult {
            ok: false,
            exit_code: o.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&o.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).trim().to_string(),
            duration_ms,
        }),
        Err(e) => Ok(UpdateResult {
            ok: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: e.to_string(),
            duration_ms,
        }),
    }
}

pub async fn restart_foldops_agent() -> Result<(), String> {
    tokio::process::Command::new("systemctl")
        .args(["restart", "foldops-agent"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn schedule_agent_self_restart() {
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        if let Err(e) = restart_foldops_agent().await {
            tracing::error!(error = %e, "agent restart failed");
        }
    });
}

pub fn schedule_post_update_restart() {
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        if let Err(e) = restart_foldops_agent().await {
            tracing::error!(error = %e, "[agent-http] restart failed");
        }
    });
}
