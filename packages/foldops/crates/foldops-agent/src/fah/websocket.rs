use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::state::FahLogState;

const WS_PATH: &str = "/api/websocket";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, serde::Deserialize)]
struct WsUnit {
    ppd: Option<f64>,
    state: Option<WsUnitState>,
}

#[derive(Debug, serde::Deserialize)]
struct WsUnitState {
    state: Option<String>,
    ppd: Option<f64>,
    eta: Option<String>,
    wu_progress: Option<f64>,
    progress: Option<f64>,
    assignment: Option<WsAssignment>,
    wu: Option<WsWu>,
}

#[derive(Debug, serde::Deserialize)]
struct WsAssignment {
    project: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
struct WsWu {
    run: Option<f64>,
    clone: Option<f64>,
    gen: Option<f64>,
}

fn progress_percent(unit: &WsUnitState) -> Option<f64> {
    if let Some(wp) = unit.wu_progress.filter(|&p| p > 0.0) {
        let p = if wp <= 1.0 { wp * 100.0 } else { wp };
        return Some((p * 1000.0).round() / 1000.0);
    }
    if let Some(p) = unit.progress.filter(|&p| p > 0.0) {
        let p = if p <= 1.0 { p * 100.0 } else { p };
        return Some((p * 1000.0).round() / 1000.0);
    }
    None
}

fn unit_to_state(raw: &WsUnit) -> Option<FahLogState> {
    let inner = raw.state.as_ref()?;
    let project = inner.assignment.as_ref().and_then(|a| a.project);
    let ppd = raw.ppd.or(inner.ppd).filter(|&p| p > 0.0);
    let progress = progress_percent(inner);
    let eta = inner.eta.as_deref().unwrap_or("").trim();

    if project.is_none() && ppd.is_none() && progress.is_none() && eta.is_empty() {
        return None;
    }

    Some(FahLogState {
        project: project.map(|p| format!("{p:.0}").trim_end_matches(".0").to_string()),
        run: inner.wu.as_ref().and_then(|w| w.run),
        clone: inner.wu.as_ref().and_then(|w| w.clone),
        gen: inner.wu.as_ref().and_then(|w| w.gen),
        progress,
        ppd,
        tpf: if eta.is_empty() {
            None
        } else {
            Some(eta.to_string())
        },
        folding_state: None,
        unit_state: None,
        folding_detail: None,
        recent_errors: vec![],
    })
}

fn empty_fah_log_state() -> FahLogState {
    FahLogState {
        project: None,
        run: None,
        clone: None,
        gen: None,
        progress: None,
        ppd: None,
        tpf: None,
        folding_state: None,
        unit_state: None,
        folding_detail: None,
        recent_errors: vec![],
    }
}

fn folding_state_from_unit(unit_state: &str, state: &FahLogState) -> String {
    match unit_state.trim().to_uppercase().as_str() {
        "RUN" if state.project.is_some() => "folding".into(),
        "RUN" => "waiting".into(),
        "PAUSE" => "paused".into(),
        "FINISH" => "finishing".into(),
        "DOWNLOAD" | "UPLOAD" | "READY" | "CORE" => unit_state.trim().to_lowercase(),
        "" => "idle".into(),
        other => other.to_lowercase(),
    }
}

fn format_activity_detail(state: &FahLogState, unit_state: &str) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(project) = state.project.as_deref() {
        parts.push(format!("project {project}"));
    }
    if let Some(progress) = state.progress {
        parts.push(format!("{progress:.1}%"));
    }
    if let Some(ppd) = state.ppd {
        parts.push(format!("{} PPD", format_ppd(ppd)));
    }
    if parts.is_empty() {
        if unit_state.eq_ignore_ascii_case("RUN") {
            Some("No work unit assigned".into())
        } else {
            None
        }
    } else {
        Some(parts.join(" - "))
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

fn enrich_state_with_activity(
    mut state: FahLogState,
    unit_state: String,
    detail: Option<String>,
) -> FahLogState {
    let normalized_unit_state = unit_state.trim().to_uppercase();
    state.folding_state = Some(folding_state_from_unit(&normalized_unit_state, &state));
    state.unit_state = if normalized_unit_state.is_empty() {
        None
    } else {
        Some(normalized_unit_state.clone())
    };
    state.folding_detail =
        detail.or_else(|| format_activity_detail(&state, &normalized_unit_state));
    state
}

fn unit_status(raw: &WsUnit) -> String {
    raw.state
        .as_ref()
        .and_then(|s| s.state.as_deref())
        .unwrap_or("")
        .to_string()
}

fn score_ws_unit(parsed: &FahLogState, status: &str) -> f64 {
    let mut score = parsed.progress.unwrap_or(0.0);
    if parsed.ppd.is_some() {
        score += 200.0;
    }
    if status == "RUN" {
        score += 1000.0;
    } else if status == "CORE" {
        score += 300.0;
    } else if status == "PAUSE" {
        score += 400.0;
    }
    score
}

fn pick_best_unit(units: &[WsUnit]) -> Option<(FahLogState, String)> {
    let mut best: Option<(FahLogState, String, f64)> = None;

    for raw in units {
        let parsed = unit_to_state(raw)?;
        let status = unit_status(raw);
        let score = score_ws_unit(&parsed, &status);

        if best.as_ref().is_none_or(|(_, _, s)| score > *s) {
            best = Some((parsed, status, score));
        }
    }

    best.map(|(state, status, _)| (state, status))
}

fn pick_best_unit_relaxed(units: &[WsUnit]) -> Option<(FahLogState, String)> {
    let mut best: Option<(FahLogState, String, f64)> = None;

    for raw in units {
        let status = unit_status(raw);
        if status.is_empty() {
            continue;
        }
        let parsed = unit_to_state(raw).unwrap_or_else(|| empty_fah_log_state());
        let score = score_ws_unit(&parsed, &status);

        if best.as_ref().is_none_or(|(_, _, s)| score > *s) {
            best = Some((parsed, status, score));
        }
    }

    best.map(|(state, status, _)| (state, status))
}

fn activity_from_groups(
    parsed: &serde_json::Value,
) -> Option<(FahLogState, String, Option<String>)> {
    let groups = parsed.get("groups")?.as_object()?;

    for group in groups.values() {
        let config = group.get("config")?;
        let paused = config
            .get("paused")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let finish = config
            .get("finish")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let failed = group
            .get("failed")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if paused {
            let detail = group
                .get("wait")
                .and_then(|w| w.as_str())
                .filter(|w| !w.is_empty())
                .map(|w| format!("Paused until {w}"));
            return Some((empty_fah_log_state(), "PAUSE".into(), detail));
        }
        if finish {
            return Some((
                empty_fah_log_state(),
                "FINISH".into(),
                Some("Completing current work unit".into()),
            ));
        }
        if !failed.is_empty() {
            return Some((
                FahLogState {
                    recent_errors: vec![failed],
                    ..empty_fah_log_state()
                },
                "RUN".into(),
                None,
            ));
        }

        return Some((empty_fah_log_state(), "RUN".into(), None));
    }

    None
}

fn activity_from_websocket_message(
    parsed: &serde_json::Value,
) -> Option<(FahLogState, String, Option<String>)> {
    let units = parsed.get("units")?.as_array()?;
    let typed: Vec<WsUnit> = units
        .iter()
        .filter_map(|u| serde_json::from_value(u.clone()).ok())
        .collect();

    if let Some((state, status)) = pick_best_unit(&typed).or_else(|| pick_best_unit_relaxed(&typed))
    {
        return Some((state, status, None));
    }

    if typed.is_empty() {
        return activity_from_groups(parsed);
    }

    None
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FahWsActivity {
    pub unit_state: String,
    pub project: Option<String>,
    pub progress: Option<f64>,
    pub ppd: Option<f64>,
    pub detail: Option<String>,
}

pub async fn query_fah_websocket_activity(host: &str, port: u16) -> Option<FahWsActivity> {
    parse_fah_websocket_with_unit_state(host, port)
        .await
        .map(|(state, unit_state, detail)| FahWsActivity {
            unit_state,
            project: state.project,
            progress: state.progress,
            ppd: state.ppd,
            detail,
        })
}

pub async fn parse_fah_websocket(host: &str, port: u16) -> Option<FahLogState> {
    parse_fah_websocket_with_unit_state(host, port)
        .await
        .map(|(state, unit_state, detail)| enrich_state_with_activity(state, unit_state, detail))
}

async fn parse_fah_websocket_with_unit_state(
    host: &str,
    port: u16,
) -> Option<(FahLogState, String, Option<String>)> {
    let url = format!("ws://{host}:{port}{WS_PATH}");

    let connect = tokio::time::timeout(DEFAULT_TIMEOUT, connect_async(&url));
    let Ok(Ok((mut ws, _))) = connect.await else {
        return None;
    };

    let read = tokio::time::timeout(DEFAULT_TIMEOUT, async {
        while let Some(msg) = ws.next().await {
            let Ok(msg) = msg else { break };
            let text = match msg {
                Message::Text(t) => t.to_string(),
                Message::Binary(b) => String::from_utf8_lossy(&b).to_string(),
                Message::Ping(p) => {
                    let _ = ws.send(Message::Pong(p)).await;
                    continue;
                }
                Message::Close(_) => break,
                _ => continue,
            };

            if text == "ping" {
                continue;
            }

            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if let Some(state) = activity_from_websocket_message(&parsed) {
                let _ = ws.close(None).await;
                return Some(state);
            }
        }
        None
    })
    .await;

    read.unwrap_or(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paused_with_empty_units_reports_pause_state() {
        let msg = json!({
            "groups": {
                "": {
                    "config": { "paused": true, "finish": false },
                    "wait": "2026-06-19T12:59:55Z"
                }
            },
            "units": []
        });
        let (state, unit_state, detail) = activity_from_websocket_message(&msg).unwrap();
        assert_eq!(unit_state, "PAUSE");
        assert!(state.project.is_none());
        assert_eq!(detail.as_deref(), Some("Paused until 2026-06-19T12:59:55Z"));
    }

    #[test]
    fn running_with_empty_units_reports_run_state() {
        let msg = json!({
            "groups": { "": { "config": { "paused": false, "finish": false } } },
            "units": []
        });
        let (_, unit_state, _) = activity_from_websocket_message(&msg).unwrap();
        assert_eq!(unit_state, "RUN");
    }

    #[test]
    fn run_unit_without_assignment_is_readable() {
        let msg = json!({
            "groups": { "": { "config": { "paused": false, "finish": false } } },
            "units": [{
                "state": { "state": "RUN" }
            }]
        });
        let (_, unit_state, _) = activity_from_websocket_message(&msg).unwrap();
        assert_eq!(unit_state, "RUN");
    }
}

pub type FahWsCommand = &'static str;

pub async fn send_fah_control_command(
    command: FahWsCommand,
    host: &str,
    port: u16,
) -> Result<(), String> {
    let url = format!("ws://{host}:{port}{WS_PATH}");
    let payload = serde_json::json!({ "cmd": "state", "state": command }).to_string();

    let connect = tokio::time::timeout(Duration::from_secs(8), connect_async(&url))
        .await
        .map_err(|_| "FAH WebSocket timeout".to_string())?
        .map_err(|_| "FAH WebSocket unavailable (is fah-client running?)".to_string())?;

    let (mut ws, _) = connect;

    ws.send(Message::Text(payload.into()))
        .await
        .map_err(|_| "FAH WebSocket error (is fah-client running on port 7396?)".to_string())?;

    tokio::time::sleep(Duration::from_millis(400)).await;
    let _ = ws.close(None).await;
    Ok(())
}
