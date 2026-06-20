mod apply;
mod effective;
mod parse;

use std::fs::{self, OpenOptions};
use std::path::PathBuf;

use nix::fcntl::{Flock, FlockArg};
use serde::Serialize;

use crate::automation_policy::{require_config_automation_mutation, require_inspectable_role};
use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;

pub use apply::apply_domain;
pub use effective::{
    effective_config, load_effective_config_for_domain, validate_secret_reference,
};
pub use parse::{parse_domain, DomainConfig, HOSTNAME_PATTERN};

#[derive(Debug, Serialize)]
struct ConfigValidationResult {
    domain: String,
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

pub fn validate_config(
    paths: &AppliancePaths,
    domain: &str,
    candidate: Option<&str>,
) -> Result<serde_json::Value, String> {
    require_inspectable_role(paths)?;
    if let Some(candidate_path) = candidate {
        validate_candidate_file(paths, domain, candidate_path)?;
        return Ok(serde_json::json!({
            "domain": domain,
            "candidate": candidate_path,
            "valid": true,
        }));
    }
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

pub fn validate_candidate_file(
    paths: &AppliancePaths,
    domain: &str,
    candidate: &str,
) -> Result<(), String> {
    if !effective::valid_domain(domain) {
        return Err(format!("unknown configuration domain {domain:?}"));
    }
    let (_resolved, content, values) = load_candidate(domain, candidate)?;
    effective::validate_candidate(paths, domain, &values)?;
    let _ = content;
    Ok(())
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

pub const FAH_PASSKEY_SECRET_NAME: &str = "fah-passkey";

pub fn set_fah_passkey(paths: &AppliancePaths, passkey: &str) -> Result<serde_json::Value, String> {
    use crate::fah::passkey::normalize_passkey_input;

    const FAH_SERVICE_GID: u32 = 200;

    require_config_automation_mutation(paths, "config", "set-passkey")?;
    let passkey = normalize_passkey_input(passkey)?;

    fs::create_dir_all(paths.secrets_dir()).map_err(|error| error.to_string())?;
    let path = paths.secrets_dir().join(FAH_PASSKEY_SECRET_NAME);
    atomic_write(&path, format!("{passkey}\n").as_bytes(), 0o640)?;
    nix::unistd::chown(
        &path,
        Some(nix::unistd::Uid::from_raw(0)),
        Some(nix::unistd::Gid::from_raw(FAH_SERVICE_GID)),
    )
    .map_err(|error| format!("set passkey secret ownership: {error}"))?;

    Ok(serde_json::json!({
        "secret": FAH_PASSKEY_SECRET_NAME,
        "written": true,
    }))
}

pub fn activate_config(
    paths: &AppliancePaths,
    domain: &str,
    candidate: &str,
) -> Result<serde_json::Value, String> {
    require_config_automation_mutation(paths, "config", "activate")?;

    if !effective::valid_domain(domain) {
        return Err(format!("unknown configuration domain {domain:?}"));
    }

    let (resolved, candidate_content, candidate_values) = load_candidate(domain, candidate)?;

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
    let _guard =
        Flock::lock(lock, FlockArg::LockExclusive).map_err(|(_, errno)| errno.to_string())?;

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
        audit_config_activation(domain, &resolved, false, &error);
        return Err(error);
    }
    if let Err(error) = effective_config(paths, domain, true) {
        let message = effective::rollback_config(
            paths,
            domain,
            &active,
            previous.ok(),
            previous_err,
            error,
            apply_domain,
        )
        .err()
        .unwrap_or_else(|| "activation failed".into());
        audit_config_activation(domain, &resolved, false, &message);
        return Err(message);
    }
    if let Err(error) = apply_domain(paths, domain) {
        let message = effective::rollback_config(
            paths,
            domain,
            &active,
            previous.ok(),
            previous_err,
            error,
            apply_domain,
        )
        .err()
        .unwrap_or_else(|| "activation failed".into());
        audit_config_activation(domain, &resolved, false, &message);
        return Err(message);
    }

    audit_config_activation(domain, &resolved, true, "activated");
    Ok(serde_json::json!({
        "domain": domain,
        "candidate": resolved,
        "activated": true,
    }))
}

fn load_candidate(
    domain: &str,
    candidate: &str,
) -> Result<(String, Vec<u8>, DomainConfig), String> {
    let resolved = resolve_data_candidate_path(candidate)?;
    let resolved_str = resolved.to_string_lossy().into_owned();
    let candidate_content = fs::read(&resolved).map_err(|error| error.to_string())?;
    let candidate_values =
        parse_domain(domain, &String::from_utf8_lossy(&candidate_content), false)?;
    Ok((resolved_str, candidate_content, candidate_values))
}

const DATA_MOUNT: &str = "/data";

fn resolve_data_candidate_path(candidate: &str) -> Result<PathBuf, String> {
    let data_root =
        fs::canonicalize(DATA_MOUNT).map_err(|error| format!("resolve {DATA_MOUNT}: {error}"))?;
    let resolved = fs::canonicalize(candidate).map_err(|error| error.to_string())?;
    if resolved == data_root {
        return Err("configuration candidate must be a regular file on /data".into());
    }
    if resolved.strip_prefix(&data_root).ok().is_none() {
        return Err("configuration candidate must be a regular file on /data".into());
    }
    let metadata = fs::metadata(&resolved).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("configuration candidate must be a regular file".into());
    }
    Ok(resolved)
}

fn audit_config_activation(domain: &str, candidate: &str, success: bool, detail: &str) {
    eprintln!(
        "foldingosctl: audit config activate domain={domain} candidate={candidate} success={success} detail={detail}"
    );
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

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn strip_prefix_accepts_nested_data_path() {
        let root =
            std::env::temp_dir().join(format!("foldingosctl-config-path-{}", std::process::id()));
        let data_root = root.join("data");
        let candidate = data_root.join("config/candidates/example.toml");
        fs::create_dir_all(candidate.parent().unwrap()).expect("create dirs");
        fs::write(&candidate, "schema_version = 1\n").expect("write candidate");

        let resolved_data = fs::canonicalize(&data_root).expect("canonical data");
        let resolved_candidate = fs::canonicalize(&candidate).expect("canonical candidate");
        assert!(resolved_candidate
            .strip_prefix(&resolved_data)
            .ok()
            .is_some());
        assert_ne!(resolved_candidate, resolved_data);

        let _ = fs::remove_dir_all(&root);
    }
}
