use serde::{Deserialize, Serialize};

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldOpsSettingsFile {
    pub schema_version: u32,
    #[serde(default)]
    pub alerts: AlertsSettingsFile,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AlertsSettingsFile {
    #[serde(default)]
    pub discord: DiscordAlertSettingsFile,
    #[serde(default)]
    pub email: EmailAlertSettingsFile,
    #[serde(default)]
    pub thresholds: AlertThresholdSettingsFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordAlertSettingsFile {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default = "default_discord_username")]
    pub username: String,
}

fn default_discord_username() -> String {
    "FoldOps".into()
}

impl Default for DiscordAlertSettingsFile {
    fn default() -> Self {
        Self {
            enabled: false,
            webhook_url: None,
            username: default_discord_username(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAlertSettingsFile {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub smtp_host: Option<String>,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_username: Option<String>,
    #[serde(default)]
    pub smtp_password: Option<String>,
    #[serde(default)]
    pub from_address: Option<String>,
    #[serde(default)]
    pub to_addresses: Vec<String>,
    #[serde(default = "default_true")]
    pub use_tls: bool,
}

fn default_smtp_port() -> u16 {
    587
}

fn default_true() -> bool {
    true
}

impl Default for EmailAlertSettingsFile {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp_host: None,
            smtp_port: default_smtp_port(),
            smtp_username: None,
            smtp_password: None,
            from_address: None,
            to_addresses: Vec::new(),
            use_tls: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholdSettingsFile {
    #[serde(default = "default_offline_threshold_ms")]
    pub offline_threshold_ms: u64,
    #[serde(default = "default_cpu_temp_alert_c")]
    pub cpu_temp_alert_c: f64,
    #[serde(default = "default_stuck_progress_hours")]
    pub stuck_progress_hours: f64,
    #[serde(default)]
    pub dashboard_url: Option<String>,
}

fn default_offline_threshold_ms() -> u64 {
    120_000
}

fn default_cpu_temp_alert_c() -> f64 {
    85.0
}

fn default_stuck_progress_hours() -> f64 {
    4.0
}

impl Default for AlertThresholdSettingsFile {
    fn default() -> Self {
        Self {
            offline_threshold_ms: default_offline_threshold_ms(),
            cpu_temp_alert_c: default_cpu_temp_alert_c(),
            stuck_progress_hours: default_stuck_progress_hours(),
            dashboard_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertSettingsUpdateRequest {
    pub discord: DiscordAlertSettingsUpdate,
    pub email: EmailAlertSettingsUpdate,
    pub thresholds: AlertThresholdSettingsFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordAlertSettingsUpdate {
    pub enabled: bool,
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default = "default_discord_username")]
    pub username: String,
    #[serde(default)]
    pub clear_webhook_url: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAlertSettingsUpdate {
    pub enabled: bool,
    #[serde(default)]
    pub smtp_host: Option<String>,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_username: Option<String>,
    #[serde(default)]
    pub smtp_password: Option<String>,
    #[serde(default)]
    pub from_address: Option<String>,
    #[serde(default)]
    pub to_addresses: Vec<String>,
    #[serde(default = "default_true")]
    pub use_tls: bool,
    #[serde(default)]
    pub clear_smtp_password: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertSettingsResponse {
    pub discord: DiscordAlertSettingsResponse,
    pub email: EmailAlertSettingsResponse,
    pub thresholds: AlertThresholdSettingsFile,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscordAlertSettingsResponse {
    pub enabled: bool,
    pub webhook_url_configured: bool,
    pub username: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailAlertSettingsResponse {
    pub enabled: bool,
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password_configured: bool,
    pub from_address: Option<String>,
    pub to_addresses: Vec<String>,
    pub use_tls: bool,
}
