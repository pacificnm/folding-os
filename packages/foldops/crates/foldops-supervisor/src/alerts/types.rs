use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    NodeOffline,
    NodeOnline,
    CpuTempHigh,
    FahInactive,
    FahFailed,
    FahErrors,
    FahStuck,
}

impl AlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NodeOffline => "node_offline",
            Self::NodeOnline => "node_online",
            Self::CpuTempHigh => "cpu_temp_high",
            Self::FahInactive => "fah_inactive",
            Self::FahFailed => "fah_failed",
            Self::FahErrors => "fah_errors",
            Self::FahStuck => "fah_stuck",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "node_offline" => Some(Self::NodeOffline),
            "node_online" => Some(Self::NodeOnline),
            "cpu_temp_high" => Some(Self::CpuTempHigh),
            "fah_inactive" => Some(Self::FahInactive),
            "fah_failed" => Some(Self::FahFailed),
            "fah_errors" => Some(Self::FahErrors),
            "fah_stuck" => Some(Self::FahStuck),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlertCandidate {
    pub id: String,
    pub hostname: String,
    pub kind: AlertKind,
    pub severity: AlertSeverity,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AlertConfig {
    pub enabled: bool,
    pub webhook_url: Option<String>,
    pub offline_threshold_ms: u64,
    pub cpu_temp_alert_c: f64,
    pub stuck_progress_hours: f64,
    pub dashboard_url: Option<String>,
    pub discord_username: String,
}
