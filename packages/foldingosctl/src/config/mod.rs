mod apply;
mod effective;
mod parse;

use std::fs::{self, OpenOptions};
use std::os::unix::io::AsRawFd;

use nix::fcntl::{flock, FlockArg};
use serde::Serialize;

use crate::automation_policy::require_inspectable_role;
use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;

pub use apply::apply_domain;
pub use effective::{effective_config, load_effective_config_for_domain, validate_secret_reference};
pub use parse::{parse_domain, DomainConfig, HOSTNAME_PATTERN};

#[derive(Debug, Serialize)]
struct ConfigValidationResult {
    domain: String,
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

pub fn validate_config(paths: &AppliancePaths, domain: &str) -> Result<serde_json::Value, String> {
    require_inspectable_role(paths)?;
    if domain == "--all" {
        let results = validate_all_config_domains(paths)?;
        let valid = results.iter().all(|result| result.valid);
        return Ok(serde_json::json!({
            "valid": valid,
            "domains": results,
        }));
    }
    effective_config(paths, domain, true)?;
    Ok(serde_json::json!({
        "domain": domain,
        "valid": true,
    }))
}

pub fn print_effective_config(
    paths: &AppliancePaths,
    domain: &str,
    write_effective: bool,
) -> Result<(serde_json::Value, Option<String>), String> {
    require_inspectable_role(paths)?;
    let merged = load_effective_config_for_domain(paths, domain)?;
    let data = serde_json::json!({
        "domain": domain,
        "config": effective::domain_config_json(&merged),
    });
    if write_effective {
        let content = effective_config(paths, domain, true)?;
        Ok((data, Some(content)))
    } else {
        Ok((data, None))
    }
}

pub fn activate_config(
    paths: &AppliancePaths,
    domain: &str,
    candidate: &str,
) -> Result<(), String> {
    if !effective::valid_domain(domain) {
        return Err(format!("unknown configuration domain {domain:?}"));
    }

    let resolved = fs::canonicalize(candidate).map_err(|error| error.to_string())?;
    let resolved_str = resolved.to_string_lossy();
    if resolved_str != "/data" && !resolved_str.starts_with("/data/") {
        return Err("configuration candidate must be a regular file on /data".into());
    }
    let metadata = fs::metadata(&resolved).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("configuration candidate must be a regular file".into());
    }

    let lock_path = paths.domain_lock_path(domain);
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| error.to_string())?;
    flock(lock.as_raw_fd(), FlockArg::LockExclusive).map_err(|error| error.to_string())?;
    let unlock = || {
        let _ = flock(lock.as_raw_fd(), FlockArg::Unlock);
    };

    let candidate_content = fs::read(&resolved).map_err(|error| error.to_string())?;
    let candidate_values = parse_domain(
        domain,
        &String::from_utf8_lossy(&candidate_content),
        false,
    )?;
    effective::validate_candidate(paths, domain, &candidate_values)?;

    let active = paths.domain_active_path(domain);
    let previous = fs::read(&active);
    let previous_err = previous.as_ref().err().map(|error| error.kind());
    if let Ok(previous_content) = &previous {
        atomic_write(
            &paths.domain_last_good_path(domain),
            previous_content,
            0o644,
        )?;
    } else if !matches!(previous_err, Some(std::io::ErrorKind::NotFound)) {
        return Err(previous.unwrap_err().to_string());
    }

    if let Err(error) = atomic_write(&active, &candidate_content, 0o644) {
        unlock();
        return Err(error);
    }
    if let Err(error) = effective_config(paths, domain, true) {
        unlock();
        return effective::rollback_config(
            paths,
            domain,
            &active,
            previous.ok(),
            previous_err,
            error,
            apply_domain,
        );
    }
    if let Err(error) = apply_domain(paths, domain) {
        unlock();
        return effective::rollback_config(
            paths,
            domain,
            &active,
            previous.ok(),
            previous_err,
            error,
            apply_domain,
        );
    }
    unlock();
    Ok(())
}

fn validate_all_config_domains(
    paths: &AppliancePaths,
) -> Result<Vec<ConfigValidationResult>, String> {
    let mut results = Vec::with_capacity(effective::domains().len());
    for name in effective::domains() {
        if let Err(error) = effective_config(paths, name, true) {
            results.push(ConfigValidationResult {
                domain: (*name).to_string(),
                valid: false,
                message: Some(error),
            });
            continue;
        }
        results.push(ConfigValidationResult {
            domain: (*name).to_string(),
            valid: true,
            message: None,
        });
    }
    Ok(results)
}
