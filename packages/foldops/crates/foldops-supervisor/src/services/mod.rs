use serde_json::Value;

use crate::config::Config;
use crate::foldingos::{self, FleetCommandError, FleetDelegateConfig};

pub async fn list_services(config: &Config) -> Result<Value, String> {
    let delegate = FleetDelegateConfig {
        foldingosctl_path: &config.foldingosctl_path,
    };
    foldingos::inspect_services(delegate)
        .await
        .map_err(fleet_command_message)
}

pub async fn restart_service(config: &Config, unit: &str) -> Result<Value, String> {
    let delegate = FleetDelegateConfig {
        foldingosctl_path: &config.foldingosctl_path,
    };
    foldingos::restart_service(delegate, unit)
        .await
        .map_err(fleet_command_message)
}

pub async fn restart_all_services(config: &Config) -> Result<Value, String> {
    let delegate = FleetDelegateConfig {
        foldingosctl_path: &config.foldingosctl_path,
    };
    foldingos::restart_all_services(delegate)
        .await
        .map_err(fleet_command_message)
}

fn fleet_command_message(error: FleetCommandError) -> String {
    error.to_string()
}
