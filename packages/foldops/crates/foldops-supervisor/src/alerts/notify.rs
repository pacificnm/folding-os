use std::sync::Mutex;

use super::types::{AlertKind, AlertSeverity};

static WEBHOOK_STATUS: Mutex<WebhookStatus> = Mutex::new(WebhookStatus {
    last_error: None,
    last_success_at: None,
});

#[derive(Debug, Clone, Default)]
struct WebhookStatus {
    last_error: Option<String>,
    last_success_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NotifyEvent {
    pub event_type: NotifyEventType,
    pub severity: AlertSeverity,
    pub hostname: String,
    pub kind: AlertKind,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyEventType {
    Fired,
    Resolved,
    Recovery,
}

pub struct DiscordNotifyOptions {
    pub webhook_url: String,
    pub username: String,
    pub dashboard_url: Option<String>,
}

pub fn get_webhook_status() -> (Option<String>, Option<String>) {
    let guard = WEBHOOK_STATUS.lock().unwrap_or_else(|e| e.into_inner());
    (guard.last_error.clone(), guard.last_success_at.clone())
}

fn is_discord_webhook(url: &str) -> bool {
    regex::Regex::new(r"(?i)discord\.com/api/webhooks")
        .unwrap()
        .is_match(url)
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        format!("{}…", &text[..max.saturating_sub(1)])
    }
}

fn machine_url(dashboard_url: Option<&str>, hostname: &str) -> Option<String> {
    dashboard_url.map(|base| {
        format!(
            "{}/machine/{}",
            base.trim_end_matches('/'),
            urlencoding_path(hostname)
        )
    })
}

fn urlencoding_path(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                c.to_string()
            } else {
                format!("%{:02X}", c as u8)
            }
        })
        .collect()
}

fn kind_label(kind: AlertKind) -> &'static str {
    match kind {
        AlertKind::NodeOffline => "Node offline",
        AlertKind::NodeOnline => "Node online",
        AlertKind::CpuTempHigh => "CPU temperature",
        AlertKind::FahInactive => "FAH inactive",
        AlertKind::FahFailed => "FAH failed",
        AlertKind::FahErrors => "FAH log errors",
        AlertKind::FahStuck => "FAH progress stuck",
    }
}

fn severity_color(severity: AlertSeverity) -> u32 {
    match severity {
        AlertSeverity::Critical => 0xed4245,
        AlertSeverity::Warning => 0xfee75c,
        AlertSeverity::Info => 0x57f287,
    }
}

fn build_embed(event: &NotifyEvent, dashboard_url: Option<&str>) -> serde_json::Value {
    let now = chrono::Utc::now().to_rfc3339();
    let url = machine_url(dashboard_url, &event.hostname);

    match event.event_type {
        NotifyEventType::Resolved => {
            return serde_json::json!({
                "title": "✅ Resolved",
                "description": event.message,
                "color": 0x57f287,
                "fields": [
                    {"name": "Host", "value": event.hostname, "inline": true},
                    {"name": "Type", "value": kind_label(event.kind), "inline": true},
                ],
                "timestamp": now,
                "url": url,
            });
        }
        NotifyEventType::Recovery => {
            return serde_json::json!({
                "title": "🟢 Node back online",
                "description": event.message,
                "color": 0x5865f2,
                "fields": [{"name": "Host", "value": event.hostname, "inline": true}],
                "timestamp": now,
                "url": url,
            });
        }
        NotifyEventType::Fired => {}
    }

    let icon = match event.severity {
        AlertSeverity::Critical => "🔴",
        AlertSeverity::Warning => "🟡",
        AlertSeverity::Info => "ℹ️",
    };

    let mut fields = vec![
        serde_json::json!({"name": "Host", "value": event.hostname, "inline": true}),
        serde_json::json!({"name": "Severity", "value": event.severity.as_str(), "inline": true}),
    ];
    if let Some(details) = &event.details {
        fields.push(serde_json::json!({
            "name": "Details",
            "value": truncate(details, 1000),
        }));
    }

    serde_json::json!({
        "title": format!("{} {}", icon, kind_label(event.kind)),
        "description": event.message,
        "color": severity_color(event.severity),
        "fields": fields,
        "timestamp": now,
        "url": url,
    })
}

async fn post_webhook(webhook_url: &str, body: serde_json::Value) -> Result<(), String> {
    let client = reqwest::Client::new();
    let res = client
        .post(webhook_url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Webhook {status}: {}", truncate(&text, 300)));
    }
    Ok(())
}

pub async fn send_alert_notifications(
    opts: &DiscordNotifyOptions,
    events: &[NotifyEvent],
) -> Result<(), String> {
    if events.is_empty() {
        return Ok(());
    }

    let result = if is_discord_webhook(&opts.webhook_url) {
        for event in events {
            let embed = build_embed(event, opts.dashboard_url.as_deref());
            post_webhook(
                &opts.webhook_url,
                serde_json::json!({
                    "username": opts.username,
                    "embeds": [embed],
                }),
            )
            .await?;
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        }
        Ok(())
    } else {
        let lines: Vec<String> = events
            .iter()
            .map(|e| {
                let prefix = match e.event_type {
                    NotifyEventType::Resolved => "✅",
                    NotifyEventType::Recovery => "🟢",
                    NotifyEventType::Fired => match e.severity {
                        AlertSeverity::Critical => "🔴",
                        AlertSeverity::Warning => "🟡",
                        AlertSeverity::Info => "ℹ️",
                    },
                };
                format!("{prefix} **FoldOps** — {}", e.message)
            })
            .collect();
        let mut content = lines.join("\n");
        if content.len() > 1900 {
            content = truncate(&content, 1900);
        }
        let body = if regex::Regex::new(r"(?i)hooks\.slack\.com")
            .unwrap()
            .is_match(&opts.webhook_url)
        {
            serde_json::json!({ "text": content })
        } else {
            serde_json::json!({ "content": content })
        };
        post_webhook(&opts.webhook_url, body).await
    };

    match result {
        Ok(()) => {
            let mut guard = WEBHOOK_STATUS.lock().unwrap_or_else(|e| e.into_inner());
            guard.last_error = None;
            guard.last_success_at = Some(chrono::Utc::now().to_rfc3339());
            Ok(())
        }
        Err(e) => {
            let mut guard = WEBHOOK_STATUS.lock().unwrap_or_else(|e| e.into_inner());
            guard.last_error = Some(e.clone());
            Err(e)
        }
    }
}

pub async fn send_test_notification(opts: &DiscordNotifyOptions) -> Result<(), String> {
    send_alert_notifications(
        opts,
        &[NotifyEvent {
            event_type: NotifyEventType::Fired,
            severity: AlertSeverity::Warning,
            hostname: "fah-test".into(),
            kind: AlertKind::CpuTempHigh,
            message: "FoldOps test alert — webhook is working".into(),
            details: None,
        }],
    )
    .await
}
