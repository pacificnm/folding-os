use std::fs;
use crate::paths::AppliancePaths;

pub fn read_hostname(paths: &AppliancePaths) -> Result<String, String> {
    if let Ok(content) = fs::read_to_string(paths.effective_system_path()) {
        if let Some(hostname) = parse_hostname_from_system_toml(&content) {
            return Ok(hostname);
        }
    }

    let mut hostname = None;
    for path in [
        paths.system_defaults_path(),
        paths.system_config_path(),
        paths.system_overrides_path(),
    ] {
        if let Ok(content) = fs::read_to_string(path) {
            if let Some(value) = parse_hostname_from_system_toml(&content) {
                hostname = Some(value);
            }
        }
    }

    hostname.ok_or_else(|| "hostname is unavailable".into())
}

fn parse_hostname_from_system_toml(content: &str) -> Option<String> {
    let mut in_identity = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[identity]" {
            in_identity = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_identity = false;
            continue;
        }
        if !in_identity {
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("hostname = ") {
            return parse_toml_string_value(value);
        }
    }
    None
}

fn parse_toml_string_value(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        return Some(raw[1..raw.len() - 1].to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_identity_hostname() {
        let content = "schema_version = 1\n\n[identity]\nhostname = \"folding-test\"\n";
        assert_eq!(
            parse_hostname_from_system_toml(content),
            Some("folding-test".into())
        );
    }
}
