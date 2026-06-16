use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use uuid::Uuid;

use crate::agent::update::push_agent_update;
use crate::config::Config;
use crate::db::{self, Db};

use super::db::{
    create_deploy_run, get_deploy_run, update_deploy_run, DeployHostResult, DeployHostStatus,
    DeployRunStatus,
};

static DEPLOY_RESTART_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(ECONNRESET|socket hang up|fetch failed|aborted)").unwrap()
});

const MAX_LOG_CHARS: usize = 12_000;

fn truncate_log(text: &str) -> String {
    if text.len() <= MAX_LOG_CHARS {
        text.to_string()
    } else {
        format!("…{}", &text[text.len().saturating_sub(MAX_LOG_CHARS)..])
    }
}

fn initial_result(hostname: &str, status: DeployHostStatus) -> DeployHostResult {
    DeployHostResult {
        hostname: hostname.to_string(),
        status,
        exit_code: None,
        message: if status == DeployHostStatus::Offline {
            "Node offline".into()
        } else {
            "Pending".into()
        },
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: None,
        started_at: None,
        finished_at: None,
    }
}

pub fn start_agent_deploy(
    db: Arc<Db>,
    config: Arc<Config>,
    hostnames: Option<Vec<String>>,
) -> Result<String, String> {
    if !config.deploy_enabled {
        return Err("Deploy is disabled (set DEPLOY_ENABLED=true)".into());
    }
    if config.agent_http_port == 0 {
        return Err("AGENT_HTTP_PORT must be set for remote deploy".into());
    }

    let conn = db.lock();
    let machines = db::list_machines(&conn).map_err(|e| e.to_string())?;
    let known: std::collections::HashSet<_> = machines.iter().map(|m| m.hostname.as_str()).collect();

    let targets: Vec<String> = match hostnames {
        Some(list) if !list.is_empty() => list.into_iter().filter(|h| known.contains(h.as_str())).collect(),
        _ => machines.iter().map(|m| m.hostname.clone()).collect(),
    };

    if targets.is_empty() {
        return Err("No matching machines to deploy".into());
    }

    let mut results = HashMap::new();
    for hostname in &targets {
        let machine = machines.iter().find(|m| &m.hostname == hostname).unwrap();
        let online = db::is_online(&machine.last_seen, config.offline_threshold_ms);
        results.insert(
            hostname.clone(),
            initial_result(
                hostname,
                if online {
                    DeployHostStatus::Pending
                } else {
                    DeployHostStatus::Offline
                },
            ),
        );
    }

    let run_id = Uuid::new_v4().to_string();
    create_deploy_run(&conn, &run_id, &targets, &results).map_err(|e| e.to_string())?;
    drop(conn);

    tokio::spawn(run_deploy_job(db, config, run_id.clone(), targets));
    Ok(run_id)
}

async fn run_deploy_job(db: Arc<Db>, config: Arc<Config>, run_id: String, targets: Vec<String>) {
    let mut handles = Vec::new();
    for hostname in targets {
        let db = db.clone();
        let config = config.clone();
        let run_id = run_id.clone();
        handles.push(tokio::spawn(async move {
            {
                let conn = db.lock();
                let run = get_deploy_run(&conn, &run_id).ok().flatten();
                let offline = run
                    .as_ref()
                    .and_then(|r| r.results.get(&hostname))
                    .is_some_and(|r| r.status == DeployHostStatus::Offline);
                if run.is_none() || offline {
                    return;
                }
            }

            patch_deploy_host(
                &db,
                &run_id,
                &hostname,
                DeployHostResult {
                    status: DeployHostStatus::Running,
                    message: "Running update…".into(),
                    started_at: Some(chrono::Utc::now().to_rfc3339()),
                    ..initial_result(&hostname, DeployHostStatus::Running)
                },
            );

            let finished_at = chrono::Utc::now().to_rfc3339();
            match push_agent_update(&hostname, config.agent_http_port, &config.ingest_token).await {
                Ok(result) if result.ok => {
                    patch_deploy_host(
                        &db,
                        &run_id,
                        &hostname,
                        DeployHostResult {
                            status: DeployHostStatus::Success,
                            exit_code: Some(result.exit_code),
                            stdout: truncate_log(&result.stdout),
                            stderr: truncate_log(&result.stderr),
                            duration_ms: Some(result.duration_ms),
                            finished_at: Some(finished_at.clone()),
                            message: if result.restarting.unwrap_or(false) {
                                "Updated; agent restarted".into()
                            } else {
                                "Update completed".into()
                            },
                            ..initial_result(&hostname, DeployHostStatus::Success)
                        },
                    );
                }
                Ok(result) => {
                    patch_deploy_host(
                        &db,
                        &run_id,
                        &hostname,
                        DeployHostResult {
                            status: DeployHostStatus::Failed,
                            exit_code: Some(result.exit_code),
                            stdout: truncate_log(&result.stdout),
                            stderr: truncate_log(&result.stderr),
                            duration_ms: Some(result.duration_ms),
                            finished_at: Some(finished_at.clone()),
                            message: format!("Update failed (exit {})", result.exit_code),
                            ..initial_result(&hostname, DeployHostStatus::Failed)
                        },
                    );
                }
                Err(msg) => {
                    let likely_restart = DEPLOY_RESTART_RE.is_match(&msg);
                    if likely_restart {
                        patch_deploy_host(
                            &db,
                            &run_id,
                            &hostname,
                            DeployHostResult {
                                status: DeployHostStatus::Success,
                                exit_code: Some(0),
                                stderr: truncate_log(&msg),
                                finished_at: Some(finished_at),
                                message: "Agent restarted (connection closed during deploy)".into(),
                                ..initial_result(&hostname, DeployHostStatus::Success)
                            },
                        );
                    } else {
                        patch_deploy_host(
                            &db,
                            &run_id,
                            &hostname,
                            DeployHostResult {
                                status: DeployHostStatus::Failed,
                                stderr: truncate_log(&msg),
                                finished_at: Some(finished_at),
                                message: msg,
                                ..initial_result(&hostname, DeployHostStatus::Failed)
                            },
                        );
                    }
                }
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let conn = db.lock();
    if let Some(run) = get_deploy_run(&conn, &run_id).ok().flatten() {
        let attempted: Vec<_> = run
            .results
            .values()
            .filter(|r| r.status != DeployHostStatus::Offline)
            .collect();
        let all_failed = !attempted.is_empty()
            && attempted.iter().all(|r| r.status == DeployHostStatus::Failed);
        let status = if all_failed {
            DeployRunStatus::Failed
        } else {
            DeployRunStatus::Completed
        };
        let _ = update_deploy_run(&conn, &run_id, status, &run.results);
    }
}

fn patch_deploy_host(db: &Db, run_id: &str, hostname: &str, patch: DeployHostResult) {
    let conn = db.lock();
    if let Some(mut run) = get_deploy_run(&conn, run_id).ok().flatten() {
        if let Some(existing) = run.results.get_mut(hostname) {
            merge_deploy_result(existing, &patch);
        } else {
            run.results.insert(hostname.to_string(), patch);
        }
        let _ = update_deploy_run(&conn, run_id, DeployRunStatus::Running, &run.results);
    }
}

fn merge_deploy_result(existing: &mut DeployHostResult, patch: &DeployHostResult) {
    if patch.status != DeployHostStatus::Pending {
        existing.status = patch.status;
    }
    if !patch.message.is_empty() {
        existing.message = patch.message.clone();
    }
    if patch.exit_code.is_some() {
        existing.exit_code = patch.exit_code;
    }
    if !patch.stdout.is_empty() {
        existing.stdout = patch.stdout.clone();
    }
    if !patch.stderr.is_empty() {
        existing.stderr = patch.stderr.clone();
    }
    if patch.duration_ms.is_some() {
        existing.duration_ms = patch.duration_ms;
    }
    if patch.started_at.is_some() {
        existing.started_at = patch.started_at.clone();
    }
    if patch.finished_at.is_some() {
        existing.finished_at = patch.finished_at.clone();
    }
}
