use std::path::PathBuf;

use crate::alerts::types::AlertConfig;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub db_path: PathBuf,
    pub ingest_token: String,
    pub offline_threshold_ms: u64,
    pub agent_http_port: u16,
    pub deploy_enabled: bool,
    pub control_enabled: bool,
    pub web_root: PathBuf,
    pub alert_config: AlertConfig,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let ingest_token =
            std::env::var("INGEST_TOKEN").map_err(|_| "INGEST_TOKEN is required".to_string())?;

        let webhook_url = env_trimmed("ALERT_WEBHOOK_URL");
        let alerts_enabled = env_flag("ALERTS_ENABLED") || webhook_url.is_some();

        let web_root = std::env::var("WEB_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/supervisor/web/dist")
            });

        Ok(Self {
            port: std::env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            db_path: PathBuf::from(
                std::env::var("DB_PATH").unwrap_or_else(|_| "./data/foldops.db".into()),
            ),
            ingest_token,
            offline_threshold_ms: std::env::var("OFFLINE_THRESHOLD_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(120_000),
            agent_http_port: std::env::var("AGENT_HTTP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9100),
            deploy_enabled: env_flag("DEPLOY_ENABLED"),
            control_enabled: env_flag("CONTROL_ENABLED"),
            web_root,
            alert_config: AlertConfig {
                enabled: alerts_enabled,
                webhook_url,
                offline_threshold_ms: std::env::var("OFFLINE_THRESHOLD_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(120_000),
                cpu_temp_alert_c: std::env::var("CPU_TEMP_ALERT_C")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(85.0),
                stuck_progress_hours: std::env::var("ALERT_STUCK_HOURS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .map(|h: f64| h.max(0.0))
                    .unwrap_or(4.0),
                dashboard_url: env_trimmed("ALERT_DASHBOARD_URL"),
                discord_username: std::env::var("ALERT_DISCORD_USERNAME")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "FoldOps".into()),
            },
        })
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

fn env_trimmed(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
