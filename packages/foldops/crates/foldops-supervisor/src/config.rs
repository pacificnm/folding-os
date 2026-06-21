use std::path::{Path, PathBuf};

use crate::alerts::types::{AlertConfig, EmailAlertConfig};
use crate::settings::SettingsStore;

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
    pub config_enabled: bool,
    pub web_root: PathBuf,
    pub alert_config: AlertConfig,
    pub foldingosctl_path: PathBuf,
    pub installation_role_path: PathBuf,
    pub packages_foldops_index_url: String,
    pub packages_tools_index_url: String,
    pub settings_store: SettingsStore,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let ingest_token =
            std::env::var("INGEST_TOKEN").map_err(|_| "INGEST_TOKEN is required".to_string())?;

        let settings_store = SettingsStore::from_env();
        let env_alert_config = alert_config_from_env();
        let alert_config = settings_store.load_or_migrate(&env_alert_config)?;

        let web_root = std::env::var("WEB_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/supervisor/web/dist")
            });

        let installation_role_path = crate::foldingos::default_installation_role_path();

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
            offline_threshold_ms: alert_config.offline_threshold_ms,
            agent_http_port: std::env::var("AGENT_HTTP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9100),
            deploy_enabled: env_flag("DEPLOY_ENABLED"),
            control_enabled: appliance_feature_enabled("CONTROL_ENABLED", &installation_role_path),
            config_enabled: appliance_feature_enabled("CONFIG_ENABLED", &installation_role_path),
            web_root,
            alert_config,
            foldingosctl_path: crate::foldingos::default_foldingosctl_path(),
            installation_role_path,
            packages_foldops_index_url: std::env::var("PACKAGES_FOLDOPS_INDEX_URL")
                .unwrap_or_else(|_| "https://packages.folding-os.com/foldops/index.json".into()),
            packages_tools_index_url: std::env::var("PACKAGES_TOOLS_INDEX_URL").unwrap_or_else(
                |_| "https://packages.folding-os.com/foldingos-tools/index.json".into(),
            ),
            settings_store,
        })
    }

    pub fn uses_supervisor_fleet_delegation(&self) -> bool {
        crate::foldingos::supervisor_fleet_delegation_enabled(&self.installation_role_path)
    }
}

pub fn alert_config_from_env() -> AlertConfig {
    let webhook_url = env_trimmed("ALERT_WEBHOOK_URL");
    let discord_enabled = env_flag("ALERTS_ENABLED") || webhook_url.is_some();
    let email_enabled = env_flag("EMAIL_ALERTS_ENABLED");

    AlertConfig {
        enabled: discord_enabled || email_enabled,
        discord_enabled,
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
        email: EmailAlertConfig {
            enabled: email_enabled,
            smtp_host: env_trimmed("EMAIL_SMTP_HOST"),
            smtp_port: std::env::var("EMAIL_SMTP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(587),
            smtp_username: env_trimmed("EMAIL_SMTP_USERNAME"),
            smtp_password: env_trimmed("EMAIL_SMTP_PASSWORD"),
            from_address: env_trimmed("EMAIL_FROM_ADDRESS"),
            to_addresses: std::env::var("EMAIL_TO_ADDRESSES")
                .ok()
                .map(|value| {
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|entry| !entry.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            use_tls: !matches!(
                std::env::var("EMAIL_SMTP_USE_TLS").as_deref(),
                Ok("0") | Ok("false") | Ok("FALSE")
            ),
        },
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

fn appliance_feature_enabled(name: &str, installation_role_path: &Path) -> bool {
    match std::env::var(name).as_deref() {
        Ok("1") | Ok("true") | Ok("TRUE") => return true,
        Ok("0") | Ok("false") | Ok("FALSE") => return false,
        _ => {}
    }
    installation_role_path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn appliance_feature_enabled_when_installation_role_exists() {
        let temp = TempDir::new().expect("tempdir");
        let role_path = temp.path().join("installation-role");
        assert!(!appliance_feature_enabled("CONFIG_ENABLED", &role_path));
        assert!(!appliance_feature_enabled("CONTROL_ENABLED", &role_path));
        fs::write(&role_path, "supervisor\n").expect("write role");
        assert!(appliance_feature_enabled("CONFIG_ENABLED", &role_path));
        assert!(appliance_feature_enabled("CONTROL_ENABLED", &role_path));
    }

    #[test]
    fn alert_config_from_env_reads_email_settings() {
        std::env::set_var("EMAIL_ALERTS_ENABLED", "true");
        std::env::set_var("EMAIL_SMTP_HOST", "smtp.example.com");
        std::env::set_var("EMAIL_TO_ADDRESSES", "ops@example.com, oncall@example.com");

        let config = alert_config_from_env();
        assert!(config.email.enabled);
        assert_eq!(
            config.email.smtp_host.as_deref(),
            Some("smtp.example.com")
        );
        assert_eq!(config.email.to_addresses.len(), 2);

        std::env::remove_var("EMAIL_ALERTS_ENABLED");
        std::env::remove_var("EMAIL_SMTP_HOST");
        std::env::remove_var("EMAIL_TO_ADDRESSES");
    }
}

fn env_trimmed(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
