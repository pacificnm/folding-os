mod acquire;
mod acquire_state;
mod activate;
mod extract;
mod manifest;
mod provision;
mod serve_https;
mod supervisor_permissions;
mod tls;
mod util;
mod verify;

use crate::paths::AppliancePaths;

pub fn acquire_json(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    acquire::foldops_acquire(paths)
}

pub fn prepare_recovery_config_permissions(paths: &AppliancePaths) -> Result<(), String> {
    supervisor_permissions::ensure_foldops_config_group_readable(paths)?;
    supervisor_permissions::ensure_recovery_state_accessible(paths)
}

pub fn run(paths: &AppliancePaths, subcommand: &str, args: &[String]) -> Result<(), String> {
    match subcommand {
        "validate-manifest" => manifest::validate_foldops_manifest_embedded(paths),
        "acquire" => {
            let _ = args;
            let data = acquire::foldops_acquire(paths)?;
            if let Some(message) = data.get("message").and_then(|value| value.as_str()) {
                println!("{message}");
            }
            Ok(())
        }
        "provision" => {
            let _ = args;
            provision::foldops_provision(paths)
        }
        "serve-https" => {
            let _ = args;
            serve_https::foldops_serve_https(paths)
        }
        other => Err(format!("unknown foldops subcommand: {other}")),
    }
}
