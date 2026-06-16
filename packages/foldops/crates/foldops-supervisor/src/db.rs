use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use foldops_types::{FahSystemdStatus, IngestPayload};
use rusqlite::{params, Connection};

pub struct Db(Mutex<Connection>);

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL")?;
        init_schema(&conn)?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Debug, Clone)]
pub struct MachineRow {
    pub hostname: String,
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SnapshotRow {
    pub id: i64,
    pub hostname: String,
    pub created_at: String,
    pub payload: String,
    pub fah_status: String,
    pub project: Option<String>,
    pub run: Option<f64>,
    pub clone: Option<f64>,
    pub gen: Option<f64>,
    pub progress: Option<f64>,
    pub ppd: Option<f64>,
    pub cpu_usage: Option<f64>,
    pub memory_percent: Option<f64>,
    pub disk_percent: Option<f64>,
    pub cpu_temp: Option<f64>,
    pub chassis_temp: Option<f64>,
    pub apt_updates: i64,
    pub reboot_required: i64,
}

fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS machines (
            hostname TEXT PRIMARY KEY,
            first_seen TEXT NOT NULL,
            last_seen TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            hostname TEXT NOT NULL,
            created_at TEXT NOT NULL,
            payload TEXT NOT NULL,
            fah_status TEXT NOT NULL,
            project TEXT,
            run REAL,
            clone REAL,
            gen REAL,
            progress REAL,
            ppd REAL,
            cpu_usage REAL,
            memory_percent REAL,
            disk_percent REAL,
            apt_updates INTEGER NOT NULL DEFAULT 0,
            reboot_required INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (hostname) REFERENCES machines(hostname)
        );

        CREATE INDEX IF NOT EXISTS idx_snapshots_hostname_created
            ON snapshots(hostname, created_at DESC);
        ",
    )?;
    migrate_schema(conn)?;
    crate::deploy::db::init_deploy_tables(conn)?;
    crate::alerts::db::init_alert_tables(conn)?;
    Ok(())
}

fn migrate_schema(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(snapshots)")?;
    let names: Vec<String> = stmt
        .query_map([], |row| row.get(1))?
        .filter_map(|r| r.ok())
        .collect();
    if !names.iter().any(|n| n == "cpu_temp") {
        conn.execute_batch("ALTER TABLE snapshots ADD COLUMN cpu_temp REAL")?;
    }
    if !names.iter().any(|n| n == "chassis_temp") {
        conn.execute_batch("ALTER TABLE snapshots ADD COLUMN chassis_temp REAL")?;
    }
    Ok(())
}

pub fn systemd_status_str(status: &FahSystemdStatus) -> &'static str {
    match status {
        FahSystemdStatus::Active => "active",
        FahSystemdStatus::Inactive => "inactive",
        FahSystemdStatus::Failed => "failed",
        FahSystemdStatus::Unknown => "unknown",
    }
}

pub fn ingest_snapshot(conn: &Connection, payload: &IngestPayload) -> rusqlite::Result<()> {
    let now = payload.timestamp.clone();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO machines (hostname, first_seen, last_seen) VALUES (?1, ?2, ?2)
         ON CONFLICT(hostname) DO UPDATE SET last_seen = ?2",
        params![payload.hostname, now],
    )?;
    tx.execute(
        "INSERT INTO snapshots (
            hostname, created_at, payload, fah_status, project, run, clone, gen,
            progress, ppd, cpu_usage, memory_percent, disk_percent,
            cpu_temp, chassis_temp, apt_updates, reboot_required
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            payload.hostname,
            now,
            serde_json::to_string(payload).unwrap_or_default(),
            systemd_status_str(&payload.fah.systemdStatus),
            payload.fah.project,
            payload.fah.run,
            payload.fah.clone,
            payload.fah.gen,
            payload.fah.progress,
            payload.fah.ppd,
            payload.system.cpuUsage,
            payload.system.memory.percent,
            payload.system.disk.percent,
            payload.system.cpuTemp,
            payload.system.chassisTemp,
            payload.maintenance.aptUpdatesAvailable,
            if payload.maintenance.rebootRequired { 1 } else { 0 },
        ],
    )?;
    tx.commit()
}

pub fn list_machines(conn: &Connection) -> rusqlite::Result<Vec<MachineRow>> {
    let mut stmt = conn.prepare(
        "SELECT hostname, first_seen, last_seen FROM machines ORDER BY hostname",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(MachineRow {
                hostname: row.get(0)?,
                first_seen: row.get(1)?,
                last_seen: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn get_machine(conn: &Connection, hostname: &str) -> rusqlite::Result<Option<MachineRow>> {
    let mut stmt = conn.prepare(
        "SELECT hostname, first_seen, last_seen FROM machines WHERE hostname = ?1",
    )?;
    let mut rows = stmt.query(params![hostname])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(MachineRow {
            hostname: row.get(0)?,
            first_seen: row.get(1)?,
            last_seen: row.get(2)?,
        }));
    }
    Ok(None)
}

fn map_snapshot(row: &rusqlite::Row<'_>) -> rusqlite::Result<SnapshotRow> {
    Ok(SnapshotRow {
        id: row.get(0)?,
        hostname: row.get(1)?,
        created_at: row.get(2)?,
        payload: row.get(3)?,
        fah_status: row.get(4)?,
        project: row.get(5)?,
        run: row.get(6)?,
        clone: row.get(7)?,
        gen: row.get(8)?,
        progress: row.get(9)?,
        ppd: row.get(10)?,
        cpu_usage: row.get(11)?,
        memory_percent: row.get(12)?,
        disk_percent: row.get(13)?,
        cpu_temp: row.get(14)?,
        chassis_temp: row.get(15)?,
        apt_updates: row.get(16)?,
        reboot_required: row.get(17)?,
    })
}

pub fn get_latest_snapshot(conn: &Connection, hostname: &str) -> rusqlite::Result<Option<SnapshotRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, hostname, created_at, payload, fah_status, project, run, clone, gen,
                progress, ppd, cpu_usage, memory_percent, disk_percent, cpu_temp, chassis_temp,
                apt_updates, reboot_required
         FROM snapshots WHERE hostname = ?1 ORDER BY created_at DESC LIMIT 1",
    )?;
    let mut rows = stmt.query(params![hostname])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(map_snapshot(row)?));
    }
    Ok(None)
}

pub fn get_snapshots(
    conn: &Connection,
    hostname: &str,
    limit: i64,
) -> rusqlite::Result<Vec<SnapshotRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, hostname, created_at, payload, fah_status, project, run, clone, gen,
                progress, ppd, cpu_usage, memory_percent, disk_percent, cpu_temp, chassis_temp,
                apt_updates, reboot_required
         FROM snapshots WHERE hostname = ?1 ORDER BY created_at DESC LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![hostname, limit], map_snapshot)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn get_snapshots_since(
    conn: &Connection,
    hostname: &str,
    since_iso: &str,
) -> rusqlite::Result<Vec<SnapshotRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, hostname, created_at, payload, fah_status, project, run, clone, gen,
                progress, ppd, cpu_usage, memory_percent, disk_percent, cpu_temp, chassis_temp,
                apt_updates, reboot_required
         FROM snapshots WHERE hostname = ?1 AND created_at >= ?2 ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![hostname, since_iso], map_snapshot)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn is_online(last_seen: &str, threshold_ms: u64) -> bool {
    chrono::DateTime::parse_from_rfc3339(last_seen)
        .map(|t| {
            let elapsed = chrono::Utc::now()
                .signed_duration_since(t.with_timezone(&chrono::Utc))
                .num_milliseconds();
            elapsed >= 0 && (elapsed as u64) < threshold_ms
        })
        .unwrap_or(false)
}
