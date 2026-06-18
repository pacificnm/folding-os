use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;

use super::parse::{domain_config_to_map, parse_domain, render_domain, validate_domain, DomainConfig};

pub fn load_effective_config_for_domain(
    paths: &AppliancePaths,
    domain: &str,
) -> Result<DomainConfig, String> {
    if !valid_domain(domain) {
        return Err(format!("unknown configuration domain {domain:?}"));
    }
    let mut merged = load_effective(
        paths,
        domain,
        Some(paths.domain_active_path(domain)),
        true,
    );
    if merged.is_err() {
        merged = load_effective(
            paths,
            domain,
            Some(paths.domain_last_good_path(domain)),
            false,
        );
    }
    if merged.is_err() {
        merged = load_effective(paths, domain, None, false);
    }
    merged
}

pub fn effective_config(
    paths: &AppliancePaths,
    domain: &str,
    write: bool,
) -> Result<String, String> {
    if !valid_domain(domain) {
        return Err(format!("unknown configuration domain {domain:?}"));
    }

    let mut merged = load_effective(
        paths,
        domain,
        Some(paths.domain_active_path(domain)),
        true,
    );
    if let Err(error) = &merged {
        eprintln!(
            "foldingosctl: invalid active {domain} configuration, trying last-known-good: {error}"
        );
        merged = load_effective(
            paths,
            domain,
            Some(paths.domain_last_good_path(domain)),
            false,
        );
    }
    if let Err(error) = &merged {
        eprintln!(
            "foldingosctl: invalid last-known-good {domain} configuration, using image defaults: {error}"
        );
        merged = load_effective(paths, domain, None, false);
    }
    let merged = merged?;
    let content = render_domain(domain, &merged);
    if write {
        atomic_write(
            &paths.domain_effective_path(domain),
            content.as_bytes(),
            0o644,
        )?;
    }
    Ok(content)
}

pub fn load_effective(
    paths: &AppliancePaths,
    domain: &str,
    active_path: Option<std::path::PathBuf>,
    include_override: bool,
) -> Result<DomainConfig, String> {
    let mut merged = DomainConfig::new();
    let mut load_paths = vec![paths.domain_defaults_path(domain)];
    if let Some(active_path) = active_path {
        load_paths.push(active_path);
    }
    if include_override {
        load_paths.push(paths.domain_overrides_path(domain));
    }

    for (index, path) in load_paths.into_iter().enumerate() {
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && index > 0 => continue,
            Err(error) => return Err(format!("read {}: {error}", path.display())),
        };
        let values = parse_domain(domain, &content, index == 0)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        for (key, value) in values {
            merged.insert(key, value);
        }
    }

    validate_domain(domain, &merged)?;
    if domain == "foldinghome" {
        validate_secret_reference(
            paths,
            &merged
                .get("identity.passkey_secret")
                .map(|value| value.text.as_str())
                .unwrap_or_default(),
        )?;
    }
    Ok(merged)
}

pub fn validate_secret_reference(paths: &AppliancePaths, name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Ok(());
    }
    let path = paths.secrets_dir().join(name);
    let metadata = fs::metadata(&path)
        .map_err(|error| format!("passkey secret {name:?} is unavailable: {error}"))?;
    if !metadata.is_file() || metadata.mode() & 0o777 != 0o640 {
        return Err(format!(
            "passkey secret {name:?} must be a regular file with mode 0640"
        ));
    }
    if metadata.uid() != 0 || metadata.gid() != 200 {
        return Err(format!("passkey secret {name:?} must be owned by root:fah"));
    }
    Ok(())
}

pub fn validate_candidate(
    paths: &AppliancePaths,
    domain: &str,
    candidate: &DomainConfig,
) -> Result<(), String> {
    let default_content = fs::read_to_string(paths.domain_defaults_path(domain))
        .map_err(|error| error.to_string())?;
    let defaults = parse_domain(domain, &default_content, true)?;
    let mut merged = defaults;
    for (key, value) in candidate {
        merged.insert(key.clone(), value.clone());
    }

    let override_path = paths.domain_overrides_path(domain);
    if let Ok(override_content) = fs::read_to_string(&override_path) {
        let overrides = parse_domain(domain, &override_content, false)?;
        for (key, value) in overrides {
            merged.insert(key, value);
        }
    } else if let Err(error) = fs::read_to_string(&override_path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error.to_string());
        }
    }

    validate_domain(domain, &merged)?;
    if domain == "foldinghome" {
        return validate_secret_reference(
            paths,
            &merged
                .get("identity.passkey_secret")
                .map(|value| value.text.as_str())
                .unwrap_or_default(),
        );
    }
    Ok(())
}

pub fn domain_config_json(config: &DomainConfig) -> serde_json::Map<String, serde_json::Value> {
    domain_config_to_map(config)
}

const DOMAINS: &[&str] = &["system", "network", "foldinghome"];

pub fn valid_domain(domain: &str) -> bool {
    DOMAINS.contains(&domain)
}

pub fn domains() -> &'static [&'static str] {
    DOMAINS
}

pub fn rollback_config(
    paths: &AppliancePaths,
    domain: &str,
    active: &Path,
    previous: Option<Vec<u8>>,
    previous_err: Option<std::io::ErrorKind>,
    cause: String,
    apply_domain: fn(&AppliancePaths, &str) -> Result<(), String>,
) -> Result<(), String> {
    if let Some(previous_content) = previous {
        if let Err(error) = atomic_write(active, &previous_content, 0o644) {
            return Err(format!("{cause}; rollback failed: {error}"));
        }
    } else if matches!(previous_err, Some(std::io::ErrorKind::NotFound)) {
        if let Err(error) = fs::remove_file(active) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(format!("{cause}; rollback failed: {error}"));
            }
        }
    }
    let _ = effective_config(paths, domain, true);
    let _ = apply_domain(paths, domain);
    Err(format!(
        "configuration activation failed and was rolled back: {cause}"
    ))
}
