use rusqlite::{params, Connection};

use super::types::{AlertKind, AlertSeverity};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AlertRow {
    pub id: String,
    pub hostname: String,
    pub kind: AlertKind,
    pub severity: AlertSeverity,
    pub message: String,
    pub details: Option<String>,
    pub active: i64,
    pub fired_at: String,
    pub resolved_at: Option<String>,
    pub last_notified_at: Option<String>,
}

pub fn init_alert_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS alerts (
            id TEXT PRIMARY KEY,
            hostname TEXT NOT NULL,
            kind TEXT NOT NULL,
            severity TEXT NOT NULL,
            message TEXT NOT NULL,
            details TEXT,
            active INTEGER NOT NULL DEFAULT 1,
            fired_at TEXT NOT NULL,
            resolved_at TEXT,
            last_notified_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_alerts_active ON alerts(active, fired_at DESC);
        CREATE INDEX IF NOT EXISTS idx_alerts_fired ON alerts(fired_at DESC);
        ",
    )
}

fn map_alert(row: &rusqlite::Row<'_>) -> rusqlite::Result<AlertRow> {
    let kind_str: String = row.get(2)?;
    let severity_str: String = row.get(3)?;
    Ok(AlertRow {
        id: row.get(0)?,
        hostname: row.get(1)?,
        kind: AlertKind::from_str(&kind_str).unwrap_or(AlertKind::FahErrors),
        severity: match severity_str.as_str() {
            "critical" => AlertSeverity::Critical,
            "warning" => AlertSeverity::Warning,
            _ => AlertSeverity::Info,
        },
        message: row.get(4)?,
        details: row.get(5)?,
        active: row.get(6)?,
        fired_at: row.get(7)?,
        resolved_at: row.get(8)?,
        last_notified_at: row.get(9)?,
    })
}

pub fn list_active_alerts(conn: &Connection) -> rusqlite::Result<Vec<AlertRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, hostname, kind, severity, message, details, active, fired_at, resolved_at, last_notified_at
         FROM alerts WHERE active = 1 ORDER BY severity DESC, fired_at DESC",
    )?;
    let rows = stmt
        .query_map([], map_alert)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn get_alert(conn: &Connection, id: &str) -> rusqlite::Result<Option<AlertRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, hostname, kind, severity, message, details, active, fired_at, resolved_at, last_notified_at
         FROM alerts WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(map_alert(row)?));
    }
    Ok(None)
}

pub fn list_alert_history(
    conn: &Connection,
    limit: i64,
    status: &str,
    hostname: Option<&str>,
) -> rusqlite::Result<Vec<AlertRow>> {
    let mut sql = String::from(
        "SELECT id, hostname, kind, severity, message, details, active, fired_at, resolved_at, last_notified_at
         FROM alerts",
    );
    let mut conditions = Vec::new();
    if status == "active" {
        conditions.push("active = 1");
    } else if status == "resolved" {
        conditions.push("active = 0");
    }
    if hostname.is_some() {
        conditions.push("hostname = ?1");
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY fired_at DESC LIMIT ?2");

    let mut stmt = conn.prepare(&sql)?;
    let rows = if let Some(host) = hostname {
        stmt.query_map(params![host, limit], map_alert)?
    } else {
        stmt.query_map(params![limit], map_alert)?
    };
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn count_alerts_by_status(conn: &Connection) -> rusqlite::Result<(i64, i64, i64)> {
    let mut stmt = conn.prepare("SELECT active, COUNT(*) FROM alerts GROUP BY active")?;
    let mut active = 0i64;
    let mut resolved = 0i64;
    for (a, n) in stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
        .flatten()
    {
        if a == 1 {
            active = n;
        } else {
            resolved = n;
        }
    }
    Ok((active, resolved, active + resolved))
}

pub fn upsert_active_alert(
    conn: &Connection,
    row: &AlertRowInput,
) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let existing = get_alert(conn, &row.id)?;
    let fired_at = if existing.as_ref().is_some_and(|e| e.active == 1) {
        existing.unwrap().fired_at
    } else {
        now.clone()
    };

    conn.execute(
        "INSERT INTO alerts (
            id, hostname, kind, severity, message, details, active,
            fired_at, resolved_at, last_notified_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, NULL, ?8)
        ON CONFLICT(id) DO UPDATE SET
            severity = ?4, message = ?5, details = ?6, active = 1,
            fired_at = ?7, resolved_at = NULL, last_notified_at = ?8",
        params![
            row.id,
            row.hostname,
            row.kind.as_str(),
            row.severity.as_str(),
            row.message,
            row.details,
            fired_at,
            row.notified_at,
        ],
    )?;
    Ok(())
}

pub struct AlertRowInput {
    pub id: String,
    pub hostname: String,
    pub kind: AlertKind,
    pub severity: AlertSeverity,
    pub message: String,
    pub details: Option<String>,
    pub notified_at: String,
}

pub fn record_recovery_alert(
    conn: &Connection,
    id: &str,
    hostname: &str,
    message: &str,
    notified_at: &str,
) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO alerts (
            id, hostname, kind, severity, message, details, active,
            fired_at, resolved_at, last_notified_at
        ) VALUES (?1, ?2, 'node_online', 'info', ?3, NULL, 0, ?4, ?4, ?5)
        ON CONFLICT(id) DO UPDATE SET
            message = ?3, fired_at = ?4, resolved_at = ?4, last_notified_at = ?5",
        params![id, hostname, message, now, notified_at],
    )?;
    Ok(())
}

pub fn resolve_alert(conn: &Connection, id: &str, notified_at: &str) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE alerts SET active = 0, resolved_at = ?2, last_notified_at = ?3 WHERE id = ?1",
        params![id, now, notified_at],
    )?;
    Ok(())
}
