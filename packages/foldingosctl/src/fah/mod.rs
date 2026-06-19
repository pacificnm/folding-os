mod acquire;
mod acquire_state;
mod activate;
mod extract;
mod manifest;
pub mod passkey;
mod prepare;
mod run_cmd;
mod util;
mod verify_install;

pub(crate) use acquire_state::load_fah_acquire_state;
pub(crate) use prepare::fah_prepare_quiet;

use crate::paths::AppliancePaths;

pub fn run(paths: &AppliancePaths, subcommand: &str, args: &[String]) -> Result<(), String> {
    match subcommand {
        "validate-manifest" => manifest::validate_fah_manifest_embedded(paths),
        "acquire" => acquire::fah_acquire(paths),
        "verify-install" => {
            let version = args
                .first()
                .ok_or_else(|| "usage: foldingosctl fah verify-install <version>".to_string())?;
            verify_install::fah_verify_install(paths, version)
        }
        "activate" => {
            let version = args
                .first()
                .ok_or_else(|| "usage: foldingosctl fah activate <version>".to_string())?;
            activate::fah_activate(paths, version)
        }
        "prepare" => prepare::fah_prepare(paths),
        "run" => run_cmd::fah_run(paths),
        other => Err(format!("unknown fah subcommand: {other}")),
    }
}
