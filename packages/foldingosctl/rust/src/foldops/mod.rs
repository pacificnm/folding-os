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

pub fn run(paths: &AppliancePaths, subcommand: &str, args: &[String]) -> Result<(), String> {
    match subcommand {
        "validate-manifest" => manifest::validate_foldops_manifest_embedded(paths),
        "acquire" => {
            let _ = args;
            acquire::foldops_acquire(paths)
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
