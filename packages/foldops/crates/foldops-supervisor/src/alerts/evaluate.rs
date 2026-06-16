use foldops_types::IngestPayload;

use crate::db::{MachineRow, SnapshotRow};

use super::stuck::detect_stuck_progress;
use super::types::{AlertCandidate, AlertConfig, AlertKind, AlertSeverity};

fn alert_id(hostname: &str, kind: AlertKind) -> String {
    format!("{}:{}", hostname, kind.as_str())
}

fn errors_fingerprint(errors: &[String]) -> String {
    errors.iter().rev().take(5).rev().cloned().collect::<Vec<_>>().join("\n")
}

fn parse_payload(row: &SnapshotRow) -> Option<IngestPayload> {
    serde_json::from_str(&row.payload).ok()
}

pub fn evaluate_machine(
    machine: &MachineRow,
    latest: Option<&SnapshotRow>,
    snapshots_since_stuck: &[SnapshotRow],
    online: bool,
    config: &AlertConfig,
    was_offline: bool,
) -> Vec<AlertCandidate> {
    let mut out = Vec::new();
    let host = &machine.hostname;

    if !online {
        out.push(AlertCandidate {
            id: alert_id(host, AlertKind::NodeOffline),
            hostname: host.clone(),
            kind: AlertKind::NodeOffline,
            severity: AlertSeverity::Critical,
            message: format!("{host} is offline (no heartbeat)"),
            details: None,
        });
        return out;
    }

    if was_offline {
        out.push(AlertCandidate {
            id: alert_id(host, AlertKind::NodeOnline),
            hostname: host.clone(),
            kind: AlertKind::NodeOnline,
            severity: AlertSeverity::Info,
            message: format!("{host} is back online"),
            details: None,
        });
    }

    let Some(latest) = latest else {
        return out;
    };

    let payload = parse_payload(latest);
    let cpu_temp = latest
        .cpu_temp
        .or_else(|| payload.as_ref().and_then(|p| p.system.cpuTemp));
    let fah_status = latest.fah_status.as_str();
    let errors = payload
        .as_ref()
        .map(|p| p.fah.recentErrors.clone())
        .unwrap_or_default();

    if cpu_temp.is_some_and(|t| t >= config.cpu_temp_alert_c) {
        out.push(AlertCandidate {
            id: alert_id(host, AlertKind::CpuTempHigh),
            hostname: host.clone(),
            kind: AlertKind::CpuTempHigh,
            severity: AlertSeverity::Warning,
            message: format!(
                "{host} CPU temperature {:.1}°C (≥ {:.0}°C)",
                cpu_temp.unwrap(),
                config.cpu_temp_alert_c
            ),
            details: None,
        });
    }

    if fah_status == "failed" {
        out.push(AlertCandidate {
            id: alert_id(host, AlertKind::FahFailed),
            hostname: host.clone(),
            kind: AlertKind::FahFailed,
            severity: AlertSeverity::Critical,
            message: format!("{host} fah-client service failed"),
            details: None,
        });
    } else if fah_status != "active" {
        out.push(AlertCandidate {
            id: alert_id(host, AlertKind::FahInactive),
            hostname: host.clone(),
            kind: AlertKind::FahInactive,
            severity: AlertSeverity::Warning,
            message: format!("{host} fah-client is {fah_status} (not folding)"),
            details: None,
        });
    } else if config.stuck_progress_hours > 0.0 {
        let stuck_ms = (config.stuck_progress_hours * 3_600_000.0) as i64;
        if let Some(stuck) = detect_stuck_progress(snapshots_since_stuck, stuck_ms) {
            out.push(AlertCandidate {
                id: alert_id(host, AlertKind::FahStuck),
                hostname: host.clone(),
                kind: AlertKind::FahStuck,
                severity: AlertSeverity::Warning,
                message: format!(
                    "{host} FAH progress stuck at {:.1}% for ~{:.1}h (threshold {:.0}h)",
                    stuck.progress, stuck.span_hours, config.stuck_progress_hours
                ),
                details: Some(format!(
                    "Range in window: {:.1}–{:.1}%",
                    stuck.min_progress, stuck.max_progress
                )),
            });
        }
    }

    if !errors.is_empty() {
        out.push(AlertCandidate {
            id: alert_id(host, AlertKind::FahErrors),
            hostname: host.clone(),
            kind: AlertKind::FahErrors,
            severity: AlertSeverity::Warning,
            message: format!("{host} reported {} recent FAH log error(s)", errors.len()),
            details: Some(errors_fingerprint(&errors)),
        });
    }

    out
}

pub fn evaluate_farm<F, G, H, I>(
    machines: &[MachineRow],
    get_latest: F,
    get_snapshots_since: G,
    is_online: H,
    was_offline: I,
    config: &AlertConfig,
) -> Vec<AlertCandidate>
where
    F: Fn(&str) -> Option<SnapshotRow>,
    G: Fn(&str) -> Vec<SnapshotRow>,
    H: Fn(&str) -> bool,
    I: Fn(&str) -> bool,
{
    let mut all = Vec::new();
    for m in machines {
        let online = is_online(&m.last_seen);
        all.extend(evaluate_machine(
            m,
            get_latest(&m.hostname).as_ref(),
            &get_snapshots_since(&m.hostname),
            online,
            config,
            was_offline(&m.hostname),
        ));
    }
    all
}
