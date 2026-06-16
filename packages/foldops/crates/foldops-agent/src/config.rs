use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub supervisor_url: String,
    pub agent_token: String,
    pub interval_ms: u64,
    pub fah_log_path: PathBuf,
    pub fah_db_path: PathBuf,
    pub fah_work_dir: PathBuf,
    pub fah_ws_host: String,
    pub fah_ws_port: u16,
    pub fah_donor_id: Option<String>,
    pub fah_team_number: Option<String>,
    pub agent_http_port: u16,
    pub foldops_root: PathBuf,
    pub update_script: PathBuf,
    pub update_enabled: bool,
    pub controls_enabled: bool,
    pub controls_allow_reboot: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let agent_token = std::env::var("AGENT_TOKEN").map_err(|_| "AGENT_TOKEN is required".to_string())?;

        let foldops_root = PathBuf::from(
            std::env::var("FOLDOPS_ROOT").unwrap_or_else(|_| "/opt/foldops".into()),
        );
        let update_script = std::env::var("UPDATE_SCRIPT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| foldops_root.join("scripts/update-agent.sh"));

        Ok(Self {
            supervisor_url: std::env::var("SUPERVISOR_URL")
                .unwrap_or_else(|_| "http://localhost:3000".into()),
            agent_token,
            interval_ms: std::env::var("INTERVAL_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60_000),
            fah_log_path: PathBuf::from(
                std::env::var("FAH_LOG_PATH").unwrap_or_else(|_| "/var/log/fah-client/log.txt".into()),
            ),
            fah_db_path: PathBuf::from(
                std::env::var("FAH_DB_PATH").unwrap_or_else(|_| "/var/lib/fah-client/client.db".into()),
            ),
            fah_work_dir: PathBuf::from(
                std::env::var("FAH_WORK_DIR").unwrap_or_else(|_| "/var/lib/fah-client/work".into()),
            ),
            fah_ws_host: std::env::var("FAH_WS_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            fah_ws_port: std::env::var("FAH_WS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7396),
            fah_donor_id: std::env::var("FAH_DONOR_ID")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            fah_team_number: std::env::var("FAH_TEAM_NUMBER")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            agent_http_port: std::env::var("AGENT_HTTP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9100),
            foldops_root,
            update_script,
            update_enabled: env_flag("UPDATE_ENABLED"),
            controls_enabled: env_flag("CONTROLS_ENABLED"),
            controls_allow_reboot: env_flag("CONTROLS_ALLOW_REBOOT"),
        })
    }

    pub fn supervisor_base(&self) -> String {
        self.supervisor_url.trim_end_matches('/').to_string()
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}
