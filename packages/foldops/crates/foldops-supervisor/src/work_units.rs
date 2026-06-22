use foldops_types::IngestPayload;
use rusqlite::{params, Connection};

use crate::db::SnapshotRow;

const BACKFILL_META_KEY: &str = "work_units_backfill_v1";

#[derive(Debug, Clone, PartialEq)]
pub struct WorkUnitKey {
    pub project: String,
    pub run: f64,
    pub clone: f64,
    pub gen: f64,
}

#[derive(Debug, Clone)]
pub struct ActiveWorkUnit {
    pub key: WorkUnitKey,
    pub started_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone)]
pub struct CompletedWorkUnit {
    pub id: i64,
    pub project: String,
    pub run: f64,
    pub clone: f64,
    pub gen: f64,
    pub started_at: String,
    pub stopped_at: String,
}

pub fn init_work_unit_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS foldops_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS active_work_units (
            hostname TEXT PRIMARY KEY,
            project TEXT NOT NULL,
            run REAL NOT NULL,
            clone REAL NOT NULL,
            gen REAL NOT NULL,
            started_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            FOREIGN KEY (hostname) REFERENCES machines(hostname)
        );

        CREATE TABLE IF NOT EXISTS completed_work_units (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            hostname TEXT NOT NULL,
            project TEXT NOT NULL,
            run REAL NOT NULL,
            clone REAL NOT NULL,
            gen REAL NOT NULL,
            started_at TEXT NOT NULL,
            stopped_at TEXT NOT NULL,
            FOREIGN KEY (hostname) REFERENCES machines(hostname)
        );

        CREATE INDEX IF NOT EXISTS idx_completed_wu_hostname_stopped
            ON completed_work_units(hostname, stopped_at DESC);

        CREATE UNIQUE INDEX IF NOT EXISTS idx_completed_wu_identity
            ON completed_work_units(hostname, project, run, clone, gen, started_at);
        ",
    )
}

pub fn maybe_backfill_from_snapshots(conn: &Connection) -> rusqlite::Result<()> {
    if backfill_done(conn)? {
        return Ok(());
    }

    let mut stmt = conn.prepare("SELECT hostname FROM machines ORDER BY hostname")?;
    let hostnames: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    for hostname in hostnames {
        let snapshots = crate::db::get_snapshots_since(conn, &hostname, "1970-01-01T00:00:00Z")?;
        replay_snapshots(conn, &hostname, &snapshots)?;
    }

    mark_backfill_done(conn)
}

pub fn process_ingest(conn: &Connection, payload: &IngestPayload) -> rusqlite::Result<()> {
    let key = work_unit_key_from_fah(&payload.fah);
    apply_transition(conn, &payload.hostname, key.as_ref(), &payload.timestamp)
}

pub fn list_completed(
    conn: &Connection,
    hostname: &str,
    limit: i64,
) -> rusqlite::Result<Vec<CompletedWorkUnit>> {
    let mut stmt = conn.prepare(
        "SELECT id, hostname, project, run, clone, gen, started_at, stopped_at
         FROM completed_work_units
         WHERE hostname = ?1
         ORDER BY stopped_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![hostname, limit], |row| {
            Ok(CompletedWorkUnit {
                id: row.get(0)?,
                project: row.get(2)?,
                run: row.get(3)?,
                clone: row.get(4)?,
                gen: row.get(5)?,
                started_at: row.get(6)?,
                stopped_at: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn count_completed(conn: &Connection, hostname: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM completed_work_units WHERE hostname = ?1",
        params![hostname],
        |row| row.get(0),
    )
}

pub fn get_active_work_unit(
    conn: &Connection,
    hostname: &str,
) -> rusqlite::Result<Option<ActiveWorkUnit>> {
    get_active(conn, hostname)
}

fn work_unit_key_from_fah(fah: &foldops_types::Fah) -> Option<WorkUnitKey> {
    Some(WorkUnitKey {
        project: fah.project.clone()?,
        run: fah.run?,
        clone: fah.clone?,
        gen: fah.gen?,
    })
}

fn work_unit_key_from_snapshot(row: &SnapshotRow) -> Option<WorkUnitKey> {
    Some(WorkUnitKey {
        project: row.project.clone()?,
        run: row.run?,
        clone: row.clone?,
        gen: row.gen?,
    })
}

fn keys_equal(a: &WorkUnitKey, b: &WorkUnitKey) -> bool {
    a.project == b.project
        && (a.run - b.run).abs() < f64::EPSILON
        && (a.clone - b.clone).abs() < f64::EPSILON
        && (a.gen - b.gen).abs() < f64::EPSILON
}

fn get_active(conn: &Connection, hostname: &str) -> rusqlite::Result<Option<ActiveWorkUnit>> {
    let mut stmt = conn.prepare(
        "SELECT hostname, project, run, clone, gen, started_at, last_seen_at
         FROM active_work_units WHERE hostname = ?1",
    )?;
    let mut rows = stmt.query(params![hostname])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(ActiveWorkUnit {
            key: WorkUnitKey {
                project: row.get(1)?,
                run: row.get(2)?,
                clone: row.get(3)?,
                gen: row.get(4)?,
            },
            started_at: row.get(5)?,
            last_seen_at: row.get(6)?,
        }));
    }
    Ok(None)
}

fn insert_completed(
    conn: &Connection,
    hostname: &str,
    key: &WorkUnitKey,
    started_at: &str,
    stopped_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO completed_work_units (
            hostname, project, run, clone, gen, started_at, stopped_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            hostname,
            key.project,
            key.run,
            key.clone,
            key.gen,
            started_at,
            stopped_at,
        ],
    )?;
    Ok(())
}

fn upsert_active(
    conn: &Connection,
    hostname: &str,
    key: &WorkUnitKey,
    started_at: &str,
    last_seen_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO active_work_units (
            hostname, project, run, clone, gen, started_at, last_seen_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(hostname) DO UPDATE SET
            project = excluded.project,
            run = excluded.run,
            clone = excluded.clone,
            gen = excluded.gen,
            started_at = excluded.started_at,
            last_seen_at = excluded.last_seen_at",
        params![
            hostname,
            key.project,
            key.run,
            key.clone,
            key.gen,
            started_at,
            last_seen_at,
        ],
    )?;
    Ok(())
}

fn clear_active(conn: &Connection, hostname: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM active_work_units WHERE hostname = ?1",
        params![hostname],
    )?;
    Ok(())
}

fn apply_transition(
    conn: &Connection,
    hostname: &str,
    next_key: Option<&WorkUnitKey>,
    observed_at: &str,
) -> rusqlite::Result<()> {
    let active = get_active(conn, hostname)?;

    match (active.as_ref(), next_key) {
        (None, None) => Ok(()),
        (None, Some(key)) => upsert_active(conn, hostname, key, observed_at, observed_at),
        (Some(active), None) => {
            insert_completed(
                conn,
                hostname,
                &active.key,
                &active.started_at,
                observed_at,
            )?;
            clear_active(conn, hostname)
        }
        (Some(active), Some(key)) if keys_equal(&active.key, key) => {
            conn.execute(
                "UPDATE active_work_units SET last_seen_at = ?2 WHERE hostname = ?1",
                params![hostname, observed_at],
            )?;
            Ok(())
        }
        (Some(active), Some(key)) => {
            insert_completed(
                conn,
                hostname,
                &active.key,
                &active.started_at,
                observed_at,
            )?;
            upsert_active(conn, hostname, key, observed_at, observed_at)
        }
    }
}

fn replay_snapshots(
    conn: &Connection,
    hostname: &str,
    snapshots: &[SnapshotRow],
) -> rusqlite::Result<()> {
    clear_active(conn, hostname)?;
    for snapshot in snapshots {
        let key = work_unit_key_from_snapshot(snapshot);
        apply_transition(conn, hostname, key.as_ref(), &snapshot.created_at)?;
    }
    Ok(())
}

fn backfill_done(conn: &Connection) -> rusqlite::Result<bool> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM foldops_meta WHERE key = ?1",
            params![BACKFILL_META_KEY],
            |row| row.get(0),
        )
        .ok();
    Ok(value.as_deref() == Some("done"))
}

fn mark_backfill_done(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO foldops_meta (key, value) VALUES (?1, 'done')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![BACKFILL_META_KEY],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use foldops_types::{
        Disk, Fah, FahSystemdStatus, IngestPayload, Maintenance, Memory, Network, System,
    };

    fn sample_payload(
        hostname: &str,
        timestamp: &str,
        project: Option<&str>,
        run: Option<f64>,
        clone: Option<f64>,
        gen: Option<f64>,
    ) -> IngestPayload {
        IngestPayload {
            hostname: hostname.into(),
            timestamp: timestamp.into(),
            nodeId: None,
            installationRole: None,
            foldingosVersion: None,
            primaryIpv4: None,
            logs: None,
            system: System {
                uptime: 100.0,
                loadAvg: [0.1, 0.2, 0.3],
                cpuUsage: 10.0,
                memory: Memory {
                    total: 1.0,
                    used: 0.5,
                    free: 0.5,
                    percent: 50.0,
                },
                disk: Disk {
                    total: 1.0,
                    used: 0.5,
                    free: 0.5,
                    percent: 50.0,
                },
                network: Network {
                    rxBytes: 0,
                    txBytes: 0,
                    rxSec: None,
                    txSec: None,
                },
                cpuTemp: None,
                chassisTemp: None,
            },
            fah: Fah {
                systemdStatus: FahSystemdStatus::Active,
                activeClientVersion: None,
                expectedClientVersion: None,
                clientInstalled: None,
                clientVerified: None,
                acquisitionFailures: None,
                acquisitionNextAttemptUnix: None,
                acquisitionLastFailureReason: None,
                logPath: None,
                logReadable: None,
                project: project.map(str::to_string),
                run,
                clone,
                gen,
                progress: Some(10.0),
                ppd: Some(1000.0),
                tpf: None,
                foldingState: None,
                unitState: None,
                foldingDetail: None,
                recentErrors: vec![],
                statsDonor: None,
                statsTeam: None,
                configUsername: None,
                configTeam: None,
                configPasskeyConfigured: None,
                configCpus: None,
            },
            maintenance: Maintenance {
                aptUpdatesAvailable: 0,
                rebootRequired: false,
            },
        }
    }

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn records_completion_when_assignment_changes() {
        let conn = test_conn();
        db::ingest_snapshot(
            &conn,
            &sample_payload("fah-01", "2026-06-11T10:00:00Z", Some("18400"), Some(0.0), Some(0.0), Some(0.0)),
        )
        .unwrap();
        db::ingest_snapshot(
            &conn,
            &sample_payload("fah-01", "2026-06-11T12:00:00Z", Some("18400"), Some(0.0), Some(1.0), Some(0.0)),
        )
        .unwrap();

        let completed = list_completed(&conn, "fah-01", 10).unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].project, "18400");
        assert_eq!(completed[0].clone, 0.0);
        assert_eq!(completed[0].started_at, "2026-06-11T10:00:00Z");
        assert_eq!(completed[0].stopped_at, "2026-06-11T12:00:00Z");

        let active = get_active(&conn, "fah-01").unwrap().unwrap();
        assert_eq!(active.key.clone, 1.0);
    }

    #[test]
    fn backfill_reconstructs_history_from_snapshots() {
        let conn = test_conn();
        db::ingest_snapshot(
            &conn,
            &sample_payload("fah-01", "2026-06-11T10:00:00Z", Some("18400"), Some(0.0), Some(0.0), Some(0.0)),
        )
        .unwrap();
        db::ingest_snapshot(
            &conn,
            &sample_payload("fah-01", "2026-06-11T12:00:00Z", Some("18400"), Some(0.0), Some(1.0), Some(0.0)),
        )
        .unwrap();

        conn.execute("DELETE FROM completed_work_units", []).unwrap();
        conn.execute("DELETE FROM active_work_units", []).unwrap();
        conn.execute("DELETE FROM foldops_meta", []).unwrap();

        maybe_backfill_from_snapshots(&conn).unwrap();

        let completed = list_completed(&conn, "fah-01", 10).unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].stopped_at, "2026-06-11T12:00:00Z");
    }
}
