use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::state::FahLogState;

const ACTIVE_STATES: &[&str] = &["RUN", "DOWNLOAD", "UPLOAD", "READY"];
const DB_READ_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbSource {
    Sqlite3Cli,
    Rusqlite,
}

impl DbSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sqlite3Cli => "sqlite3-cli",
            Self::Rusqlite => "rusqlite",
        }
    }
}

pub struct FahDbParseResult {
    pub state: Option<FahLogState>,
    pub error: Option<String>,
    pub source: Option<DbSource>,
}

#[derive(Debug)]
struct FahUnitState {
    state: Option<String>,
    progress: Option<f64>,
    wu_progress: Option<f64>,
    ppd: Option<f64>,
    eta: Option<String>,
    run_time: Option<f64>,
    project: Option<String>,
    run: Option<f64>,
    clone: Option<f64>,
    gen: Option<f64>,
}

fn normalize_unit_row(raw: &Value) -> Option<FahUnitState> {
    let obj = raw.as_object()?;
    let inner = obj.get("state");

    let slot = if let Some(inner_val) = inner {
        if inner_val.is_object() {
            inner_val
        } else if inner_val.is_string()
            || obj.contains_key("wu_progress")
            || obj.contains_key("ppd")
            || obj.contains_key("assignment")
        {
            raw
        } else {
            return None;
        }
    } else if obj.contains_key("wu_progress")
        || obj.contains_key("ppd")
        || obj.contains_key("assignment")
    {
        raw
    } else {
        return None;
    };

    parse_unit_state(slot)
}

fn parse_unit_state(v: &Value) -> Option<FahUnitState> {
    let obj = v.as_object()?;

    let project = extract_project(v);

    Some(FahUnitState {
        state: obj.get("state").and_then(|s| s.as_str()).map(str::to_string),
        progress: json_num_opt(obj.get("progress")),
        wu_progress: json_num_opt(obj.get("wu_progress")),
        ppd: json_num_opt(obj.get("ppd")),
        eta: obj.get("eta").and_then(|e| e.as_str()).map(str::to_string),
        run_time: json_num_opt(obj.get("run_time")),
        project,
        run: obj
            .get("wu")
            .and_then(|w| w.get("run"))
            .and_then(json_num),
        clone: obj
            .get("wu")
            .and_then(|w| w.get("clone"))
            .and_then(json_num),
        gen: obj
            .get("wu")
            .and_then(|w| w.get("gen"))
            .and_then(json_num),
    })
}

fn json_num_opt(v: Option<&Value>) -> Option<f64> {
    v.and_then(json_num)
}

fn json_num(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn extract_project(unit: &Value) -> Option<String> {
    let obj = unit.as_object()?;
    let candidates = [
        obj.get("assignment")
            .and_then(|a| a.get("project"))
            .and_then(json_num),
        obj.get("assignment")
            .and_then(|a| a.get("data"))
            .and_then(|d| d.get("project"))
            .and_then(json_num),
        obj.get("data")
            .and_then(|d| d.get("assignment"))
            .and_then(|a| a.get("data"))
            .and_then(|d| d.get("project"))
            .and_then(json_num),
        obj.get("project").and_then(json_num),
    ];
    candidates
        .into_iter()
        .flatten()
        .next()
        .map(|n| format!("{n:.0}").trim_end_matches(".0").to_string())
}

fn progress_percent(unit: &FahUnitState) -> Option<f64> {
    if let Some(wp) = unit.wu_progress.filter(|&p| p >= 0.0) {
        let p = if wp <= 1.0 { wp * 100.0 } else { wp };
        return Some((p * 1000.0).round() / 1000.0);
    }
    if let Some(p) = unit.progress.filter(|&p| p > 0.0) {
        let p = if p <= 1.0 { p * 100.0 } else { p };
        return Some((p * 1000.0).round() / 1000.0);
    }
    None
}

fn unit_has_metrics(unit: &FahUnitState) -> bool {
    progress_percent(unit).is_some_and(|p| p > 0.0)
        || unit.ppd.is_some_and(|p| p > 0.0)
        || unit.eta.as_ref().is_some_and(|e| !e.trim().is_empty())
        || unit.wu_progress.is_some_and(|p| p > 0.0)
}

fn format_tpf(run_time_sec: f64, wu_progress: f64) -> Option<String> {
    if wu_progress <= 0.0 || run_time_sec <= 0.0 {
        return None;
    }
    let total_sec = run_time_sec / wu_progress;
    let h = (total_sec / 3600.0).floor() as u32;
    let m = ((total_sec % 3600.0) / 60.0).floor() as u32;
    let s = (total_sec % 60.0).floor() as u32;
    Some(format!("{h}:{m:02}:{s:02}"))
}

fn unit_to_state(unit: &FahUnitState) -> Option<FahLogState> {
    if unit.project.is_none() && !unit_has_metrics(unit) {
        return None;
    }

    let wu_progress = unit.wu_progress.unwrap_or(0.0);
    let tpf = unit
        .eta
        .as_ref()
        .filter(|e| !e.trim().is_empty())
        .cloned()
        .or_else(|| {
            unit.run_time
                .and_then(|rt| format_tpf(rt, wu_progress))
        });

    Some(FahLogState {
        project: unit.project.clone(),
        run: unit.run,
        clone: unit.clone,
        gen: unit.gen,
        progress: progress_percent(unit),
        ppd: unit.ppd.filter(|p| *p > 0.0),
        tpf,
        recent_errors: vec![],
    })
}

fn pick_best_unit(units: &[FahUnitState]) -> Option<FahLogState> {
    let active: HashSet<&str> = ACTIVE_STATES.iter().copied().collect();
    let mut best: Option<(FahLogState, f64)> = None;

    for unit in units {
        let parsed = unit_to_state(unit)?;
        let status = unit.state.as_deref().unwrap_or("");
        let mut score = progress_percent(unit).unwrap_or(0.0);
        if unit.ppd.is_some() {
            score += 200.0;
        }
        if status == "RUN" {
            score += 1000.0;
        } else if active.contains(status) {
            score += 500.0;
        }

        if best.as_ref().is_none_or(|(_, s)| score > *s) {
            best = Some((parsed, score));
        }
    }

    best.map(|(s, _)| s)
}

fn pick_best_unit_relaxed(units: &[FahUnitState]) -> Option<FahLogState> {
    let mut best: Option<(FahLogState, f64)> = None;

    for unit in units {
        let parsed = unit_to_state(unit)?;
        let score = parsed.ppd.unwrap_or(0.0) + progress_percent(unit).unwrap_or(0.0) * 10.0;
        if best.as_ref().is_none_or(|(_, s)| score > *s) {
            best = Some((parsed, score));
        }
    }

    best.map(|(s, _)| s)
}

fn parse_units_json(rows: &[String]) -> Vec<FahUnitState> {
    let mut units = Vec::new();
    for value in rows {
        let Ok(raw) = serde_json::from_str::<Value>(value) else {
            continue;
        };
        if let Some(normalized) = normalize_unit_row(&raw) {
            units.push(normalized);
        }
    }
    units
}

fn finalize_units(units: Vec<FahUnitState>, source: DbSource) -> FahDbParseResult {
    if units.is_empty() {
        return FahDbParseResult {
            state: None,
            error: Some("client.db has no units rows".into()),
            source: None,
        };
    }

    let picked = pick_best_unit(&units).or_else(|| pick_best_unit_relaxed(&units));

    if let Some(state) = picked {
        return FahDbParseResult {
            state: Some(state),
            error: None,
            source: Some(source),
        };
    }

    let hint: Vec<_> = units
        .iter()
        .map(|u| u.state.as_deref().unwrap_or("unknown"))
        .collect();
    FahDbParseResult {
        state: None,
        error: Some(format!(
            "no readable work unit in client.db ({} units, states: {}) — is fah-client folding?",
            units.len(),
            hint.join(", ")
        )),
        source: None,
    }
}

async fn load_units_via_sqlite3_cli(db_path: &Path) -> Option<Vec<FahUnitState>> {
    let output = tokio::time::timeout(
        Duration::from_secs(8),
        tokio::process::Command::new("sqlite3")
            .args(["-json", &db_path.to_string_lossy(), "SELECT value FROM units"])
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let rows: Vec<Value> = serde_json::from_str(&stdout).ok()?;
    let values: Vec<String> = rows
        .into_iter()
        .filter_map(|r| r.get("value").and_then(|v| v.as_str()).map(str::to_string))
        .collect();
    Some(parse_units_json(&values))
}

fn load_units_via_rusqlite(db_path: &Path) -> Option<Vec<FahUnitState>> {
    let conn = match rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(c) => c,
        Err(_) => {
            let conn = rusqlite::Connection::open(db_path).ok()?;
            conn.execute_batch("PRAGMA query_only = ON").ok()?;
            conn
        }
    };

    let mut stmt = conn.prepare("SELECT value FROM units").ok()?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .ok()?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();
    Some(parse_units_json(&rows))
}

async fn read_client_db(db_path: &Path) -> FahDbParseResult {
    if let Some(units) = load_units_via_sqlite3_cli(db_path).await {
        return finalize_units(units, DbSource::Sqlite3Cli);
    }

    if let Some(units) = load_units_via_rusqlite(db_path) {
        return finalize_units(units, DbSource::Rusqlite);
    }

    FahDbParseResult {
        state: None,
        error: Some(format!(
            "cannot read {} — run: apt install sqlite3 && systemctl restart foldops-agent",
            db_path.display()
        )),
        source: None,
    }
}

pub async fn parse_fah_client_db(db_path: &Path) -> FahDbParseResult {
    match tokio::fs::metadata(db_path).await {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return FahDbParseResult {
                state: None,
                error: Some(format!("{} not readable ({e})", db_path.display())),
                source: None,
            };
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return FahDbParseResult {
                state: None,
                error: Some(format!(
                    "permission denied reading {} — run: systemctl restart foldops-agent",
                    db_path.display()
                )),
                source: None,
            };
        }
        Err(e) => {
            return FahDbParseResult {
                state: None,
                error: Some(format!("{} not readable ({e})", db_path.display())),
                source: None,
            };
        }
        Ok(_) => {}
    }

    match tokio::time::timeout(DB_READ_TIMEOUT, read_client_db(db_path)).await {
        Ok(result) => result,
        Err(_) => FahDbParseResult {
            state: None,
            error: Some(format!(
                "timed out reading {} after {}ms",
                db_path.display(),
                DB_READ_TIMEOUT.as_millis()
            )),
            source: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn picks_run_unit_with_higher_score() {
        let raw = json!({
            "state": "RUN",
            "wu_progress": 0.5,
            "ppd": 100000.0,
            "assignment": { "project": 18400 },
            "wu": { "run": 0, "clone": 0, "gen": 0 }
        });
        let unit = normalize_unit_row(&raw).unwrap();
        let state = pick_best_unit(&[unit]).unwrap();
        assert_eq!(state.project.as_deref(), Some("18400"));
        assert_eq!(state.ppd, Some(100_000.0));
    }
}
