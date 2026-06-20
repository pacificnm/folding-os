use std::thread;
use std::time::{Duration, Instant};

use crate::identity::ensure_identity;
use crate::paths::AppliancePaths;
use crate::process::run_command;

const FAH_SERVICE: &str = "folding-at-home.service";

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
            let was_active = systemd_unit_is_active(FAH_SERVICE);
            if was_active {
                run_command("systemctl", &["stop", FAH_SERVICE])?;
            }
            match crate::fah::fah_prepare_quiet(paths) {
                Ok(()) => {
                    run_command("systemctl", &["start", FAH_SERVICE])?;
                }
                Err(error) => {
                    eprintln!(
                        "foldingosctl: foldinghome configuration activated but runtime prepare failed: {error}"
                    );
                    if was_active {
                        let _ = run_command("systemctl", &["start", FAH_SERVICE]);
                    }
                }
            }
            Ok(())
        }
        other => Err(format!("unknown configuration domain {other:?}")),
    }
}

fn systemd_unit_is_active(unit: &str) -> bool {
    std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
