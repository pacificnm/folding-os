use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::alerts::types::{AlertConfig, EmailAlertConfig};

use self::types::{
    AlertSettingsResponse, AlertSettingsUpdateRequest, AlertThresholdSettingsFile,
    AlertsSettingsFile, DiscordAlertSettingsFile, DiscordAlertSettingsResponse,
    EmailAlertSettingsFile, EmailAlertSettingsResponse, FoldOpsSettingsFile,
    SETTINGS_SCHEMA_VERSION,
};

pub mod types;

pub const DEFAULT_SETTINGS_PATH: &str = "/data/config/foldops/settings.toml";
pub const DEFAULT_SUPERVISOR_ENV_PATH: &str = "/data/config/foldops/supervisor.env";

#[derive(Debug, Clone)]
pub struct SettingsStore {
    pub settings_path: PathBuf,
    pub supervisor_env_path: PathBuf,
}

impl SettingsStore {
    pub fn from_env() -> Self {
        Self {
            settings_path: std::env::var("FOLDOPS_SETTINGS_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_SETTINGS_PATH)),
            supervisor_env_path: std::env::var("FOLDOPS_SUPERVISOR_ENV_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_SUPERVISOR_ENV_PATH)),
        }
    }

    pub fn load_or_migrate(&self, env_config: &AlertConfig) -> Result<AlertConfig, String> {
        if self.settings_path.is_file() {
            let raw = fs::read_to_string(&self.settings_path)
                .map_err(|e| format!("read {}: {e}", self.settings_path.display()))?;
            let file = parse_settings(&raw)?;
            return Ok(alert_config_from_file(&file.alerts));
        }

        let file = alert_config_to_file(env_config);
        if let Some(parent) = self.settings_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(error) = self.write_settings_file(&FoldOpsSettingsFile {
            schema_version: SETTINGS_SCHEMA_VERSION,
            alerts: file,
        }) {
            tracing::warn!(
                error = %error,
                path = %self.settings_path.display(),
                "could not migrate alert settings to canonical settings file"
            );
        }
        Ok(env_config.clone())
    }

    pub fn save_alert_settings(
        &self,
        current: &AlertConfig,
        update: AlertSettingsUpdateRequest,
    ) -> Result<AlertConfig, String> {
        validate_alert_update(&update)?;

        let webhook_url = if update.discord.clear_webhook_url {
            None
        } else if let Some(url) = optional_trimmed(update.discord.webhook_url.clone()) {
            Some(url)
        } else {
            current.webhook_url.clone()
        };

        let discord = DiscordAlertSettingsFile {
            enabled: update.discord.enabled,
            webhook_url,
            username: trim_or_default(&update.discord.username, "FoldOps"),
        };

        let smtp_password = if update.email.clear_smtp_password {
            None
        } else if let Some(password) = optional_trimmed(update.email.smtp_password.clone()) {
            Some(password)
        } else {
            current.email.smtp_password.clone()
        };

        let email = EmailAlertSettingsFile {
            enabled: update.email.enabled,
            smtp_host: optional_trimmed(update.email.smtp_host.clone()),
            smtp_port: update.email.smtp_port,
            smtp_username: optional_trimmed(update.email.smtp_username.clone()),
            smtp_password,
            from_address: optional_trimmed(update.email.from_address.clone()),
            to_addresses: normalize_addresses(update.email.to_addresses.clone()),
            use_tls: update.email.use_tls,
        };

        let alerts = AlertsSettingsFile {
            discord: discord.clone(),
            email: email.clone(),
            thresholds: update.thresholds.clone(),
        };

        validate_alerts_file(&alerts)?;

        let settings = FoldOpsSettingsFile {
            schema_version: SETTINGS_SCHEMA_VERSION,
            alerts: alerts.clone(),
        };
        self.write_settings_file(&settings)?;
        sync_supervisor_env(&self.supervisor_env_path, &alerts)?;

        Ok(alert_config_from_file(&alerts))
    }

    fn write_settings_file(&self, settings: &FoldOpsSettingsFile) -> Result<(), String> {
        if settings.schema_version != SETTINGS_SCHEMA_VERSION {
            return Err(format!(
                "unsupported settings schema_version {}",
                settings.schema_version
            ));
        }

        let serialized = toml::to_string_pretty(settings)
            .map_err(|e| format!("serialize settings: {e}"))?;

        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }

        let tmp_path = self.settings_path.with_extension("toml.tmp");
        {
            let mut file = fs::File::create(&tmp_path)
                .map_err(|e| format!("create {}: {e}", tmp_path.display()))?;
            file.write_all(serialized.as_bytes())
                .map_err(|e| format!("write {}: {e}", tmp_path.display()))?;
            file.sync_all()
                .map_err(|e| format!("sync {}: {e}", tmp_path.display()))?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600)).map_err(|e| {
                format!(
                    "chmod {}: {e}",
                    tmp_path.display()
                )
            })?;
        }

        fs::rename(&tmp_path, &self.settings_path).map_err(|e| {
            format!(
                "rename {} -> {}: {e}",
                tmp_path.display(),
                self.settings_path.display()
            )
        })?;

        Ok(())
    }
}

pub fn parse_settings(raw: &str) -> Result<FoldOpsSettingsFile, String> {
    let settings: FoldOpsSettingsFile =
        toml::from_str(raw).map_err(|e| format!("invalid settings.toml: {e}"))?;
    if settings.schema_version != SETTINGS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported settings schema_version {}",
            settings.schema_version
        ));
    }
    validate_alerts_file(&settings.alerts)?;
    Ok(settings)
}

pub fn alert_config_from_file(alerts: &AlertsSettingsFile) -> AlertConfig {
    AlertConfig {
        enabled: alerts.discord.enabled || alerts.email.enabled,
        discord_enabled: alerts.discord.enabled,
        webhook_url: if alerts.discord.enabled {
            alerts.discord.webhook_url.clone()
        } else {
            None
        },
        offline_threshold_ms: alerts.thresholds.offline_threshold_ms,
        cpu_temp_alert_c: alerts.thresholds.cpu_temp_alert_c,
        stuck_progress_hours: alerts.thresholds.stuck_progress_hours.max(0.0),
        dashboard_url: optional_trimmed(alerts.thresholds.dashboard_url.clone()),
        discord_username: trim_or_default(&alerts.discord.username, "FoldOps"),
        email: EmailAlertConfig {
            enabled: alerts.email.enabled,
            smtp_host: optional_trimmed(alerts.email.smtp_host.clone()),
            smtp_port: alerts.email.smtp_port,
            smtp_username: optional_trimmed(alerts.email.smtp_username.clone()),
            smtp_password: optional_trimmed(alerts.email.smtp_password.clone()),
            from_address: optional_trimmed(alerts.email.from_address.clone()),
            to_addresses: alerts.email.to_addresses.clone(),
            use_tls: alerts.email.use_tls,
        },
    }
}

pub fn alert_config_to_file(config: &AlertConfig) -> AlertsSettingsFile {
    AlertsSettingsFile {
        discord: DiscordAlertSettingsFile {
            enabled: config.discord_enabled,
            webhook_url: config.webhook_url.clone(),
            username: config.discord_username.clone(),
        },
        email: EmailAlertSettingsFile {
            enabled: config.email.enabled,
            smtp_host: config.email.smtp_host.clone(),
            smtp_port: config.email.smtp_port,
            smtp_username: config.email.smtp_username.clone(),
            smtp_password: config.email.smtp_password.clone(),
            from_address: config.email.from_address.clone(),
            to_addresses: config.email.to_addresses.clone(),
            use_tls: config.email.use_tls,
        },
        thresholds: AlertThresholdSettingsFile {
            offline_threshold_ms: config.offline_threshold_ms,
            cpu_temp_alert_c: config.cpu_temp_alert_c,
            stuck_progress_hours: config.stuck_progress_hours,
            dashboard_url: config.dashboard_url.clone(),
        },
    }
}

pub fn alert_settings_response(config: &AlertConfig) -> AlertSettingsResponse {
    AlertSettingsResponse {
        discord: DiscordAlertSettingsResponse {
            enabled: config.discord_enabled,
            webhook_url_configured: config.webhook_url.is_some(),
            username: config.discord_username.clone(),
        },
        email: EmailAlertSettingsResponse {
            enabled: config.email.enabled,
            smtp_host: config.email.smtp_host.clone(),
            smtp_port: config.email.smtp_port,
            smtp_username: config.email.smtp_username.clone(),
            smtp_password_configured: config
                .email
                .smtp_password
                .as_ref()
                .is_some_and(|s| !s.is_empty()),
            from_address: config.email.from_address.clone(),
            to_addresses: config.email.to_addresses.clone(),
            use_tls: config.email.use_tls,
        },
        thresholds: AlertThresholdSettingsFile {
            offline_threshold_ms: config.offline_threshold_ms,
            cpu_temp_alert_c: config.cpu_temp_alert_c,
            stuck_progress_hours: config.stuck_progress_hours,
            dashboard_url: config.dashboard_url.clone(),
        },
    }
}

fn validate_alert_update(update: &AlertSettingsUpdateRequest) -> Result<(), String> {
    validate_alerts_file(&AlertsSettingsFile {
        discord: DiscordAlertSettingsFile {
            enabled: update.discord.enabled,
            webhook_url: if update.discord.clear_webhook_url {
                None
            } else {
                optional_trimmed(update.discord.webhook_url.clone())
            },
            username: trim_or_default(&update.discord.username, "FoldOps"),
        },
        email: EmailAlertSettingsFile {
            enabled: update.email.enabled,
            smtp_host: optional_trimmed(update.email.smtp_host.clone()),
            smtp_port: update.email.smtp_port,
            smtp_username: optional_trimmed(update.email.smtp_username.clone()),
            smtp_password: optional_trimmed(update.email.smtp_password.clone()),
            from_address: optional_trimmed(update.email.from_address.clone()),
            to_addresses: normalize_addresses(update.email.to_addresses.clone()),
            use_tls: update.email.use_tls,
        },
        thresholds: update.thresholds.clone(),
    })
}

fn validate_alerts_file(alerts: &AlertsSettingsFile) -> Result<(), String> {
    if alerts.thresholds.offline_threshold_ms == 0 {
        return Err("offline_threshold_ms must be greater than zero".into());
    }
    if alerts.thresholds.cpu_temp_alert_c <= 0.0 {
        return Err("cpu_temp_alert_c must be greater than zero".into());
    }
    if alerts.thresholds.stuck_progress_hours < 0.0 {
        return Err("stuck_progress_hours must be zero or greater".into());
    }

    if alerts.discord.enabled && alerts.discord.webhook_url.is_none() {
        return Err("discord.enabled requires webhook_url".into());
    }

    if let Some(url) = alerts.discord.webhook_url.as_deref() {
        if !url.starts_with("https://") {
            return Err("discord.webhook_url must use https".into());
        }
    }

    if alerts.email.enabled {
        let host = alerts
            .email
            .smtp_host
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "email.enabled requires smtp_host".to_string())?;
        if host.is_empty() {
            return Err("email.enabled requires smtp_host".into());
        }
        if alerts.email.smtp_port == 0 {
            return Err("email.smtp_port must be greater than zero".into());
        }
        if alerts
            .email
            .from_address
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Err("email.enabled requires from_address".into());
        }
        if alerts.email.to_addresses.is_empty() {
            return Err("email.enabled requires at least one to_addresses entry".into());
        }
    }

    Ok(())
}

pub fn sync_supervisor_env(env_path: &Path, alerts: &AlertsSettingsFile) -> Result<(), String> {
    let mut lines = if env_path.is_file() {
        fs::read_to_string(env_path)
            .map_err(|e| format!("read {}: {e}", env_path.display()))?
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let discord_active = alerts.discord.enabled
        && alerts
            .discord
            .webhook_url
            .as_ref()
            .is_some_and(|s| !s.is_empty());
    set_env_line(
        &mut lines,
        "ALERTS_ENABLED",
        if discord_active || alerts.email.enabled {
            "true"
        } else {
            "false"
        },
    );
    set_env_line(
        &mut lines,
        "ALERT_WEBHOOK_URL",
        alerts.discord.webhook_url.as_deref().unwrap_or(""),
    );
    set_env_line(
        &mut lines,
        "ALERT_DISCORD_USERNAME",
        alerts.discord.username.as_str(),
    );
    set_env_line(
        &mut lines,
        "ALERT_DASHBOARD_URL",
        alerts.thresholds.dashboard_url.as_deref().unwrap_or(""),
    );
    set_env_line(
        &mut lines,
        "OFFLINE_THRESHOLD_MS",
        &alerts.thresholds.offline_threshold_ms.to_string(),
    );
    set_env_line(
        &mut lines,
        "CPU_TEMP_ALERT_C",
        &alerts.thresholds.cpu_temp_alert_c.to_string(),
    );
    set_env_line(
        &mut lines,
        "ALERT_STUCK_HOURS",
        &alerts.thresholds.stuck_progress_hours.to_string(),
    );
    set_env_line(
        &mut lines,
        "EMAIL_ALERTS_ENABLED",
        if alerts.email.enabled { "true" } else { "false" },
    );
    set_env_line(
        &mut lines,
        "EMAIL_SMTP_HOST",
        alerts.email.smtp_host.as_deref().unwrap_or(""),
    );
    set_env_line(
        &mut lines,
        "EMAIL_SMTP_PORT",
        &alerts.email.smtp_port.to_string(),
    );
    set_env_line(
        &mut lines,
        "EMAIL_SMTP_USERNAME",
        alerts.email.smtp_username.as_deref().unwrap_or(""),
    );
    set_env_line(
        &mut lines,
        "EMAIL_SMTP_PASSWORD",
        alerts.email.smtp_password.as_deref().unwrap_or(""),
    );
    set_env_line(
        &mut lines,
        "EMAIL_FROM_ADDRESS",
        alerts.email.from_address.as_deref().unwrap_or(""),
    );
    set_env_line(
        &mut lines,
        "EMAIL_TO_ADDRESSES",
        &alerts.email.to_addresses.join(","),
    );
    set_env_line(
        &mut lines,
        "EMAIL_SMTP_USE_TLS",
        if alerts.email.use_tls { "true" } else { "false" },
    );

    write_env_file(env_path, &lines)
}

fn set_env_line(lines: &mut Vec<String>, key: &str, value: &str) {
    let prefix = format!("{key}=");
    if let Some(index) = lines.iter().position(|line| line.starts_with(&prefix)) {
        lines[index] = format!("{key}={}", shell_escape(value));
    } else {
        lines.push(format!("{key}={}", shell_escape(value)));
    }
}

fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == ':' || c == '/')
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

fn write_env_file(path: &Path, lines: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }

    let tmp_path = path.with_extension("env.tmp");
    let body = lines.join("\n");
    let body = if body.is_empty() {
        body
    } else {
        format!("{body}\n")
    };

    {
        let mut file = fs::File::create(&tmp_path)
            .map_err(|e| format!("create {}: {e}", tmp_path.display()))?;
        file.write_all(body.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp_path.display()))?;
        file.sync_all()
            .map_err(|e| format!("sync {}: {e}", tmp_path.display()))?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600)).map_err(|e| {
            format!("chmod {}: {e}", tmp_path.display())
        })?;
    }

    fs::rename(tmp_path, path).map_err(|e| format!("rename env file: {e}"))
}

fn optional_trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn trim_or_default(value: &str, default: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_addresses(addresses: Vec<String>) -> Vec<String> {
    addresses
        .into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn round_trip_settings_file() {
        let config = AlertConfig {
            enabled: true,
            discord_enabled: true,
            webhook_url: Some("https://discord.com/api/webhooks/test".into()),
            offline_threshold_ms: 90_000,
            cpu_temp_alert_c: 80.0,
            stuck_progress_hours: 2.0,
            dashboard_url: Some("https://supervisor.example/dashboard".into()),
            discord_username: "FoldOps".into(),
            email: EmailAlertConfig {
                enabled: true,
                smtp_host: Some("smtp.example.com".into()),
                smtp_port: 587,
                smtp_username: Some("alerts".into()),
                smtp_password: Some("secret".into()),
                from_address: Some("alerts@example.com".into()),
                to_addresses: vec!["ops@example.com".into()],
                use_tls: true,
            },
        };

        let file = alert_config_to_file(&config);
        let parsed = alert_config_from_file(&file);
        assert_eq!(parsed.webhook_url, config.webhook_url);
        assert_eq!(parsed.email.smtp_host, config.email.smtp_host);
        assert_eq!(parsed.email.to_addresses, config.email.to_addresses);
    }

    #[test]
    fn rejects_discord_enabled_without_webhook() {
        let err = validate_alerts_file(&AlertsSettingsFile {
            discord: DiscordAlertSettingsFile {
                enabled: true,
                webhook_url: None,
                ..Default::default()
            },
            ..Default::default()
        })
        .expect_err("expected validation error");
        assert!(err.contains("webhook_url"));
    }

    #[test]
    fn save_updates_env_file() {
        let temp = TempDir::new().expect("tempdir");
        let settings_path = temp.path().join("settings.toml");
        let env_path = temp.path().join("supervisor.env");
        fs::write(&env_path, "INGEST_TOKEN=abc\n").expect("seed env");

        let store = SettingsStore {
            settings_path: settings_path.clone(),
            supervisor_env_path: env_path.clone(),
        };

        let current = AlertConfig {
            enabled: false,
            discord_enabled: false,
            webhook_url: None,
            offline_threshold_ms: 120_000,
            cpu_temp_alert_c: 85.0,
            stuck_progress_hours: 4.0,
            dashboard_url: None,
            discord_username: "FoldOps".into(),
            email: EmailAlertConfig::default(),
        };

        let updated = store
            .save_alert_settings(
                &current,
                AlertSettingsUpdateRequest {
                    discord: super::types::DiscordAlertSettingsUpdate {
                        enabled: true,
                        webhook_url: Some(
                            "https://discord.com/api/webhooks/123/token".into(),
                        ),
                        username: "FoldOps".into(),
                        clear_webhook_url: false,
                    },
                    email: super::types::EmailAlertSettingsUpdate {
                        enabled: true,
                        smtp_host: Some("smtp.example.com".into()),
                        smtp_port: 587,
                        smtp_username: Some("alerts".into()),
                        smtp_password: Some("secret".into()),
                        from_address: Some("alerts@example.com".into()),
                        to_addresses: vec!["ops@example.com".into()],
                        use_tls: true,
                        clear_smtp_password: false,
                    },
                    thresholds: AlertThresholdSettingsFile::default(),
                },
            )
            .expect("save settings");

        assert!(settings_path.is_file());
        assert!(updated.enabled);
        let env = fs::read_to_string(&env_path).expect("read env");
        assert!(env.contains("ALERT_WEBHOOK_URL=https://discord.com/api/webhooks/123/token"));
        assert!(env.contains("EMAIL_SMTP_HOST=smtp.example.com"));
        assert!(env.contains("INGEST_TOKEN=abc"));
    }
}
