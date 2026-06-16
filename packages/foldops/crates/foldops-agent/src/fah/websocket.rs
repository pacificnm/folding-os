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
        recent_errors: vec![],
    })
}

fn pick_best_unit(units: &[WsUnit]) -> Option<FahLogState> {
    let mut best: Option<(FahLogState, f64)> = None;

    for raw in units {
        let parsed = unit_to_state(raw)?;
        let status = raw.state.as_ref().and_then(|s| s.state.as_deref()).unwrap_or("");
        let mut score = parsed.progress.unwrap_or(0.0);
        if parsed.ppd.is_some() {
            score += 200.0;
        }
        if status == "RUN" {
            score += 1000.0;
        } else if status == "CORE" {
            score += 300.0;
        }

        if best.as_ref().is_none_or(|(_, s)| score > *s) {
            best = Some((parsed, score));
        }
    }

    best.map(|(s, _)| s)
}

pub async fn parse_fah_websocket(host: &str, port: u16) -> Option<FahLogState> {
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
            let units = parsed.get("units").and_then(|u| u.as_array())?;
            let typed: Vec<WsUnit> = units
                .iter()
                .filter_map(|u| serde_json::from_value(u.clone()).ok())
                .collect();
            if let Some(state) = pick_best_unit(&typed) {
                let _ = ws.close(None).await;
                return Some(state);
            }
        }
        None
    })
    .await;

    read.unwrap_or(None)
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
