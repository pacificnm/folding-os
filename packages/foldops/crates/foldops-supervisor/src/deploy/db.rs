use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployRunStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployHostStatus {
    Pending,
    Running,
    Success,
    Failed,
    Offline,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployHostResult {
    pub hostname: String,
    pub status: DeployHostStatus,
    pub exit_code: Option<i32>,
    pub message: String,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: Option<i64>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployRun {
    pub id: String,
    pub created_at: String,
    pub status: DeployRunStatus,
    pub hostnames: Vec<String>,
    pub results: std::collections::HashMap<String, DeployHostResult>,
}

pub fn init_deploy_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS deploy_runs (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            status TEXT NOT NULL,
            hostnames TEXT NOT NULL,
            results TEXT NOT NULL DEFAULT '{}'
        );
        CREATE INDEX IF NOT EXISTS idx_deploy_runs_created ON deploy_runs(created_at DESC);
        ",
    )
}

fn parse_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeployRun> {
    let status: String = row.get(2)?;
    Ok(DeployRun {
        id: row.get(0)?,
        created_at: row.get(1)?,
        status: match status.as_str() {
            "failed" => DeployRunStatus::Failed,
            "completed" => DeployRunStatus::Completed,
            _ => DeployRunStatus::Running,
        },
        hostnames: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
        results: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
    })
}

pub fn create_deploy_run(
    conn: &Connection,
    id: &str,
    hostnames: &[String],
    results: &std::collections::HashMap<String, DeployHostResult>,
) -> rusqlite::Result<DeployRun> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO deploy_runs (id, created_at, status, hostnames, results) VALUES (?1, ?2, 'running', ?3, ?4)",
        params![
            id,
            now,
            serde_json::to_string(hostnames).unwrap_or_default(),
            serde_json::to_string(results).unwrap_or_default(),
        ],
    )?;
    Ok(DeployRun {
        id: id.to_string(),
        created_at: now,
        status: DeployRunStatus::Running,
        hostnames: hostnames.to_vec(),
        results: results.clone(),
    })
}

pub fn update_deploy_run(
    conn: &Connection,
    id: &str,
    status: DeployRunStatus,
    results: &std::collections::HashMap<String, DeployHostResult>,
) -> rusqlite::Result<()> {
    let status_str = match status {
        DeployRunStatus::Running => "running",
        DeployRunStatus::Completed => "completed",
        DeployRunStatus::Failed => "failed",
    };
    conn.execute(
        "UPDATE deploy_runs SET status = ?2, results = ?3 WHERE id = ?1",
        params![id, status_str, serde_json::to_string(results).unwrap_or_default()],
    )?;
    Ok(())
}

pub fn get_deploy_run(conn: &Connection, id: &str) -> rusqlite::Result<Option<DeployRun>> {
    let mut stmt = conn.prepare("SELECT id, created_at, status, hostnames, results FROM deploy_runs WHERE id = ?1")?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(parse_run(row)?));
    }
    Ok(None)
}

pub fn list_deploy_runs(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<DeployRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, status, hostnames, results FROM deploy_runs ORDER BY created_at DESC LIMIT ?1",
    )?;
    let runs = stmt
        .query_map(params![limit], parse_run)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(runs)
}
