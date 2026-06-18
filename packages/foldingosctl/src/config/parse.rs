use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

pub static HOSTNAME_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$").expect("hostname pattern compiles")
});

static SECRET_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9._-]+$").expect("secret pattern compiles")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue {
    pub kind: String,
    pub text: String,
    pub ival: i64,
    pub bval: bool,
}

pub type DomainConfig = HashMap<String, ConfigValue>;

pub fn parse_domain(
    domain: &str,
    content: &str,
    require_complete: bool,
) -> Result<DomainConfig, String> {
    let allowed = allowed_keys(domain);
    let mut values = DomainConfig::new();
    let mut section = String::new();

    for (number, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            if section.is_empty() {
                return Err(format!("line {}: empty table name", number + 1));
            }
            if !valid_section(domain, &section) {
                return Err(format!("line {}: unknown table {section:?}", number + 1));
            }
            continue;
        }
        let Some((key_part, value_part)) = line.split_once('=') else {
            return Err(format!("line {}: expected key = value", number + 1));
        };
        let mut key = key_part.trim().to_string();
        if !section.is_empty() {
            key = format!("{section}.{key}");
        }
        let Some(kind) = allowed.get(key.as_str()) else {
            return Err(format!("line {}: unknown key {key:?}", number + 1));
        };
        if values.contains_key(&key) {
            return Err(format!("line {}: duplicate key {key:?}", number + 1));
        }
        let value = parse_value(kind, value_part.trim())
            .map_err(|error| format!("line {}: {error}", number + 1))?;
        values.insert(key, value);
    }

    if require_complete {
        for key in allowed.keys() {
            if !values.contains_key(*key) {
                return Err(format!("missing required key {key:?}"));
            }
        }
    }

    match values.get("schema_version") {
        Some(value) if value.ival == 1 => {}
        _ => return Err("schema_version must be present and equal 1".into()),
    }
    Ok(values)
}

fn parse_value(kind: &str, text: &str) -> Result<ConfigValue, String> {
    let mut value = ConfigValue {
        kind: kind.to_string(),
        text: String::new(),
        ival: 0,
        bval: false,
    };
    match kind {
        "string" => {
            let parsed =
                parse_quoted_string(text).ok_or_else(|| "expected quoted string".to_string())?;
            value.text = parsed;
        }
        "int" => {
            value.ival = text
                .parse::<i64>()
                .map_err(|_| "expected integer".to_string())?;
        }
        "bool" => {
            value.bval = text
                .parse::<bool>()
                .map_err(|_| "expected boolean".to_string())?;
        }
        other => return Err(format!("unsupported value kind {other:?}")),
    }
    Ok(value)
}

fn parse_quoted_string(text: &str) -> Option<String> {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        return Some(text[1..text.len() - 1].to_string());
    }
    None
}

pub fn validate_domain(domain: &str, values: &DomainConfig) -> Result<(), String> {
    match values.get("schema_version") {
        Some(value) if value.ival == 1 => {}
        _ => return Err("schema_version must be 1".into()),
    }
    match domain {
        "system" => {
            let hostname = values
                .get("identity.hostname")
                .map(|value| value.text.as_str())
                .unwrap_or_default();
            if !hostname.is_empty() && !HOSTNAME_PATTERN.is_match(hostname) {
                return Err("identity.hostname is not a valid RFC 1123 host label".into());
            }
        }
        "network" => {
            let dhcp = values
                .get("ethernet.dhcp")
                .map(|value| value.bval)
                .unwrap_or(false);
            let required = values
                .get("ethernet.required_for_online")
                .map(|value| value.bval)
                .unwrap_or(false);
            if !dhcp || !required {
                return Err("v0.1.0 requires DHCP Ethernet for network-online".into());
            }
        }
        "foldinghome" => {
            let username = values
                .get("identity.username")
                .map(|value| value.text.as_str())
                .unwrap_or_default();
            if username.is_empty()
                || std::str::from_utf8(username.as_bytes()).is_err()
                || username.as_bytes().len() > 128
            {
                return Err("identity.username must contain 1 through 128 UTF-8 bytes".into());
            }
            let team = values
                .get("identity.team")
                .map(|value| value.ival)
                .unwrap_or(0);
            if team < 0 || team > 2_147_483_647 {
                return Err("identity.team is outside the supported range".into());
            }
            let secret = values
                .get("identity.passkey_secret")
                .map(|value| value.text.as_str())
                .unwrap_or_default();
            if !secret.is_empty()
                && (!SECRET_PATTERN.is_match(secret) || secret == "." || secret == "..")
            {
                return Err("identity.passkey_secret must be a safe basename".into());
            }
            let cpus = values
                .get("resources.cpus")
                .map(|value| value.ival)
                .unwrap_or(0);
            if cpus < 0 {
                return Err("resources.cpus must be zero or positive".into());
            }
            if values
                .get("resources.gpus")
                .map(|value| value.bval)
                .unwrap_or(false)
            {
                return Err("resources.gpus must be false in v0.1.0".into());
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn render_domain(_domain: &str, values: &DomainConfig) -> String {
    let mut builder = String::new();
    if let Some(schema) = values.get("schema_version") {
        builder.push_str("schema_version = ");
        builder.push_str(&schema.ival.to_string());
        builder.push('\n');
    }

    let mut keys: Vec<&String> = values
        .keys()
        .filter(|key| key.as_str() != "schema_version")
        .collect();
    keys.sort();

    let mut current_section = String::new();
    for key in keys {
        let (section, name) = split_key(key);
        if section != current_section {
            builder.push('\n');
            builder.push('[');
            builder.push_str(&section);
            builder.push_str("]\n");
            current_section = section;
        }
        let value = &values[key.as_str()];
        builder.push_str(&name);
        builder.push_str(" = ");
        match value.kind.as_str() {
            "string" => {
                builder.push('"');
                builder.push_str(&value.text);
                builder.push('"');
            }
            "int" => builder.push_str(&value.ival.to_string()),
            "bool" => builder.push_str(&value.bval.to_string()),
            _ => {}
        }
        builder.push('\n');
    }
    builder
}

fn split_key(key: &str) -> (String, String) {
    match key.rfind('.') {
        Some(index) => (key[..index].to_string(), key[index + 1..].to_string()),
        None => (String::new(), key.to_string()),
    }
}

pub fn domain_config_to_map(config: &DomainConfig) -> serde_json::Map<String, serde_json::Value> {
    let mut result = serde_json::Map::new();
    for (key, value) in config {
        let json_value = match value.kind.as_str() {
            "int" => serde_json::Value::Number(value.ival.into()),
            "bool" => serde_json::Value::Bool(value.bval),
            _ => serde_json::Value::String(value.text.clone()),
        };
        result.insert(key.clone(), json_value);
    }
    result
}

fn allowed_keys(domain: &str) -> HashMap<&'static str, &'static str> {
    let mut common = HashMap::from([("schema_version", "int")]);
    match domain {
        "system" => {
            common.insert("identity.hostname", "string");
        }
        "network" => {
            common.insert("ethernet.dhcp", "bool");
            common.insert("ethernet.required_for_online", "bool");
        }
        "foldinghome" => {
            common.insert("identity.username", "string");
            common.insert("identity.team", "int");
            common.insert("identity.passkey_secret", "string");
            common.insert("resources.cpus", "int");
            common.insert("resources.gpus", "bool");
        }
        _ => {}
    }
    common
}

pub fn valid_section(domain: &str, section: &str) -> bool {
    matches!(
        (domain, section),
        ("system", "identity")
            | ("network", "ethernet")
            | ("foldinghome", "identity" | "resources")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_render_system_config() {
        let config = "schema_version = 1\n\n[identity]\nhostname = \"folding-node\"\n";
        let values = parse_domain("system", config, true).expect("parse system config");
        assert_eq!(render_domain("system", &values), config);
    }

    #[test]
    fn reject_unknown_config_key() {
        let config = "schema_version = 1\n\n[ethernet]\ndhcp = true\nrequired_for_online = true\naddress = \"192.0.2.1\"\n";
        assert!(parse_domain("network", config, true).is_err());
    }

    #[test]
    fn reject_unsupported_foldinghome_config() {
        let mut config = DomainConfig::new();
        config.insert(
            "schema_version".into(),
            ConfigValue {
                kind: "int".into(),
                text: String::new(),
                ival: 1,
                bval: false,
            },
        );
        config.insert(
            "identity.username".into(),
            ConfigValue {
                kind: "string".into(),
                text: "Anonymous".into(),
                ival: 0,
                bval: false,
            },
        );
        config.insert(
            "identity.team".into(),
            ConfigValue {
                kind: "int".into(),
                text: String::new(),
                ival: 0,
                bval: false,
            },
        );
        config.insert(
            "identity.passkey_secret".into(),
            ConfigValue {
                kind: "string".into(),
                text: "../secret".into(),
                ival: 0,
                bval: false,
            },
        );
        config.insert(
            "resources.cpus".into(),
            ConfigValue {
                kind: "int".into(),
                text: String::new(),
                ival: 0,
                bval: false,
            },
        );
        config.insert(
            "resources.gpus".into(),
            ConfigValue {
                kind: "bool".into(),
                text: String::new(),
                ival: 0,
                bval: false,
            },
        );
        assert!(validate_domain("foldinghome", &config).is_err());
    }

    #[test]
    fn reject_enabled_gpus() {
        let mut config = DomainConfig::new();
        config.insert(
            "schema_version".into(),
            ConfigValue {
                kind: "int".into(),
                text: String::new(),
                ival: 1,
                bval: false,
            },
        );
        config.insert(
            "identity.username".into(),
            ConfigValue {
                kind: "string".into(),
                text: "Anonymous".into(),
                ival: 0,
                bval: false,
            },
        );
        config.insert(
            "identity.team".into(),
            ConfigValue {
                kind: "int".into(),
                text: String::new(),
                ival: 0,
                bval: false,
            },
        );
        config.insert(
            "identity.passkey_secret".into(),
            ConfigValue {
                kind: "string".into(),
                text: String::new(),
                ival: 0,
                bval: false,
            },
        );
        config.insert(
            "resources.cpus".into(),
            ConfigValue {
                kind: "int".into(),
                text: String::new(),
                ival: 0,
                bval: false,
            },
        );
        config.insert(
            "resources.gpus".into(),
            ConfigValue {
                kind: "bool".into(),
                text: String::new(),
                ival: 0,
                bval: true,
            },
        );
        assert!(validate_domain("foldinghome", &config).is_err());
    }

    #[test]
    fn reject_malformed_schema_version() {
        assert!(parse_domain(
            "system",
            "schema_version = nope\n\n[identity]\nhostname = \"\"\n",
            true
        )
        .is_err());
    }

    #[test]
    fn rejects_invalid_hostname_label() {
        let mut config = DomainConfig::new();
        config.insert(
            "schema_version".into(),
            ConfigValue {
                kind: "int".into(),
                text: String::new(),
                ival: 1,
                bval: false,
            },
        );
        config.insert(
            "identity.hostname".into(),
            ConfigValue {
                kind: "string".into(),
                text: "INVALID HOST".into(),
                ival: 0,
                bval: false,
            },
        );
        assert!(validate_domain("system", &config).is_err());
    }

    #[test]
    fn accepts_valid_hostname_label() {
        let mut config = DomainConfig::new();
        config.insert(
            "schema_version".into(),
            ConfigValue {
                kind: "int".into(),
                text: String::new(),
                ival: 1,
                bval: false,
            },
        );
        config.insert(
            "identity.hostname".into(),
            ConfigValue {
                kind: "string".into(),
                text: "folding-node".into(),
                ival: 0,
                bval: false,
            },
        );
        assert!(validate_domain("system", &config).is_ok());
    }
}
