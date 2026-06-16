use rusqlite::Connection;

use crate::db::{self, Db};

use super::db::{
    count_alerts_by_status, get_alert, list_active_alerts, list_alert_history, record_recovery_alert,
    resolve_alert, upsert_active_alert, AlertRowInput,
};
use super::evaluate::evaluate_farm;
use super::notify::{send_alert_notifications, send_test_notification, DiscordNotifyOptions, NotifyEvent, NotifyEventType};
use super::types::{AlertConfig, AlertKind, AlertSeverity};

pub async fn run_alert_evaluation(db: &Db, config: &AlertConfig) {
    if !config.enabled {
        return;
    }

    let (notify_events, webhook_opts) = {
        let conn = db.lock();
        let machines = db::list_machines(&conn).unwrap_or_default();

        let stuck_cutoff = if config.stuck_progress_hours > 0.0 {
            Some(
                chrono::Utc::now()
                    - chrono::Duration::milliseconds(
                        (config.stuck_progress_hours * 3_600_000.0) as i64,
                    ),
            )
        } else {
            None
        };
        let stuck_cutoff_iso = stuck_cutoff.map(|t| t.to_rfc3339());

        let candidates = evaluate_farm(
            &machines,
            |h| db::get_latest_snapshot(&conn, h).ok().flatten(),
            |h| {
                stuck_cutoff_iso
                    .as_ref()
                    .and_then(|since| db::get_snapshots_since(&conn, h, since).ok())
                    .unwrap_or_default()
            },
            |last_seen| db::is_online(last_seen, config.offline_threshold_ms),
            |h| was_offline(&conn, h),
            config,
        );

        let candidate_map: std::collections::HashMap<_, _> =
            candidates.iter().map(|c| (c.id.clone(), c)).collect();
        let active_rows = list_active_alerts(&conn).unwrap_or_default();
        let active_map: std::collections::HashMap<_, _> =
            active_rows.iter().map(|r| (r.id.clone(), r)).collect();

        let mut to_fire = Vec::new();
        let mut to_resolve = Vec::new();

        for c in &candidates {
            if c.kind == AlertKind::NodeOnline {
                continue;
            }
            if let Some(prev) = active_map.get(&c.id) {
                if c.kind == AlertKind::FahErrors
                    && c.details.as_deref() != prev.details.as_deref()
                {
                    to_fire.push(c);
                }
            } else {
                to_fire.push(c);
            }
        }

        for row in &active_rows {
            if !candidate_map.contains_key(&row.id) {
                to_resolve.push(row);
            }
        }

        let notified_at = chrono::Utc::now().to_rfc3339();
        let mut notify_events = Vec::new();

        for c in &candidates {
            if c.kind != AlertKind::NodeOnline {
                continue;
            }
            notify_events.push(NotifyEvent {
                event_type: NotifyEventType::Recovery,
                severity: AlertSeverity::Info,
                hostname: c.hostname.clone(),
                kind: c.kind,
                message: c.message.clone(),
                details: None,
            });
            let _ = record_recovery_alert(&conn, &c.id, &c.hostname, &c.message, &notified_at);
        }

        for c in to_fire {
            notify_events.push(NotifyEvent {
                event_type: NotifyEventType::Fired,
                severity: c.severity,
                hostname: c.hostname.clone(),
                kind: c.kind,
                message: c.message.clone(),
                details: c.details.clone(),
            });
            let _ = upsert_active_alert(
                &conn,
                &AlertRowInput {
                    id: c.id.clone(),
                    hostname: c.hostname.clone(),
                    kind: c.kind,
                    severity: c.severity,
                    message: c.message.clone(),
                    details: c.details.clone(),
                    notified_at: notified_at.clone(),
                },
            );
        }

        for row in to_resolve {
            notify_events.push(NotifyEvent {
                event_type: NotifyEventType::Resolved,
                severity: AlertSeverity::Info,
                hostname: row.hostname.clone(),
                kind: row.kind,
                message: format!("Resolved: {}", row.message),
                details: None,
            });
            let _ = resolve_alert(&conn, &row.id, &notified_at);
        }

        let webhook_opts = config.webhook_url.as_ref().map(|webhook_url| DiscordNotifyOptions {
            webhook_url: webhook_url.clone(),
            username: config.discord_username.clone(),
            dashboard_url: config.dashboard_url.clone(),
        });
        (notify_events, webhook_opts)
    };

    if notify_events.is_empty() {
        return;
    }

    if let Some(opts) = webhook_opts {
        match send_alert_notifications(&opts, &notify_events).await {
            Ok(()) => tracing::info!(count = notify_events.len(), "alert notifications sent"),
            Err(e) => tracing::error!(error = %e, "alert webhook failed"),
        }
    } else {
        tracing::info!(count = notify_events.len(), "alert events (no webhook configured)");
        for e in &notify_events {
            tracing::info!(message = %e.message, "alert event");
        }
    }
}

fn was_offline(conn: &Connection, hostname: &str) -> bool {
    get_alert(conn, &format!("{hostname}:node_offline"))
        .ok()
        .flatten()
        .is_some_and(|r| r.active == 1)
}

pub async fn run_test_alert(config: &AlertConfig) -> Result<(), String> {
    let webhook_url = config
        .webhook_url
        .as_ref()
        .ok_or_else(|| "ALERT_WEBHOOK_URL is not set".to_string())?;
    send_test_notification(&DiscordNotifyOptions {
        webhook_url: webhook_url.clone(),
        username: config.discord_username.clone(),
        dashboard_url: config.dashboard_url.clone(),
    })
    .await
}

pub fn list_active_alerts_json(db: &Db) -> serde_json::Value {
    let conn = db.lock();
    let alerts: Vec<_> = list_active_alerts(&conn)
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "hostname": r.hostname,
                "kind": r.kind.as_str(),
                "severity": r.severity.as_str(),
                "message": r.message,
                "active": r.active == 1,
                "since": r.fired_at,
                "resolved_at": r.resolved_at,
            })
        })
        .collect();
    let count = alerts.len();
    serde_json::json!({ "alerts": alerts, "count": count })
}

pub fn list_alert_history_json(
    db: &Db,
    limit: i64,
    status: &str,
    hostname: Option<&str>,
) -> serde_json::Value {
    let conn = db.lock();
    let alerts: Vec<_> = list_alert_history(&conn, limit, status, hostname)
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let fired = chrono::DateTime::parse_from_rfc3339(&r.fired_at)
                .map(|t| t.timestamp_millis())
                .unwrap_or(0);
            let end_ms = r
                .resolved_at
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|t| t.timestamp_millis())
                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
            serde_json::json!({
                "id": r.id,
                "hostname": r.hostname,
                "kind": r.kind.as_str(),
                "severity": r.severity.as_str(),
                "message": r.message,
                "active": r.active == 1,
                "fired_at": r.fired_at,
                "resolved_at": r.resolved_at,
                "duration_ms": (end_ms - fired).max(0),
                "details": r.details,
            })
        })
        .collect();
    let count = alerts.len();
    let (active, resolved, total) = count_alerts_by_status(&conn).unwrap_or((0, 0, 0));
    serde_json::json!({
        "alerts": alerts,
        "count": count,
        "counts": { "active": active, "resolved": resolved, "total": total },
        "status": status,
    })
}

pub fn alerts_status_json(config: &AlertConfig) -> serde_json::Value {
    let webhook_url = config.webhook_url.as_deref().unwrap_or("");
    let (last_error, last_success_at) = super::notify::get_webhook_status();
    serde_json::json!({
        "enabled": config.enabled,
        "webhook_configured": config.webhook_url.is_some(),
        "discord": regex::Regex::new(r"(?i)discord\.com/api/webhooks")
            .unwrap()
            .is_match(webhook_url),
        "dashboard_url": config.dashboard_url,
        "webhook": {
            "last_error": last_error,
            "last_success_at": last_success_at,
        },
    })
}
