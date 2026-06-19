use std::thread;
use std::time::{Duration, Instant};

use crate::identity::ensure_identity;
use crate::paths::AppliancePaths;
use crate::process::run_command;

pub fn apply_domain(paths: &AppliancePaths, domain: &str) -> Result<(), String> {
    match domain {
        "system" => ensure_identity(paths),
        "network" => {
            run_command("systemctl", &["try-restart", "systemd-networkd.service"])?;
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if std::process::Command::new("systemctl")
                    .args(["is-active", "--quiet", "systemd-networkd.service"])
                    .status()
                    .map(|status| status.success())
                    .unwrap_or(false)
                {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err("systemd-networkd did not become active within 30 seconds".into());
                }
                thread::sleep(Duration::from_secs(1));
            }
        }
        "foldinghome" => {
            match crate::fah::fah_prepare_quiet(paths) {
                Ok(()) => {
                    run_command("systemctl", &["try-restart", "folding-at-home.service"])?;
                }
                Err(error) => {
                    eprintln!(
                        "foldingosctl: foldinghome configuration activated but runtime prepare failed: {error}"
                    );
                }
            }
            Ok(())
        }
        other => Err(format!("unknown configuration domain {other:?}")),
    }
}
