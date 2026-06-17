use std::fs;
use std::sync::Mutex;

use crate::paths::AppliancePaths;
use crate::role::{read_active_installation_role, require_supervisor_role};

static TEST_USERNAME: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone)]
struct AutomationPolicyCommand {
    group: String,
    name: String,
}

#[derive(Debug, Clone)]
struct AutomationPolicy {
    schema_version: i32,
    service_user: String,
    installation_role: String,
    commands: Vec<AutomationPolicyCommand>,
}

pub fn current_unix_username() -> Option<String> {
    if let Ok(guard) = TEST_USERNAME.lock() {
        if let Some(name) = guard.as_ref() {
            return Some(name.clone());
        }
    }
    users::get_current_username().map(|name| name.to_string_lossy().into_owned())
}

pub fn is_foldops_automation_user() -> bool {
    current_unix_username().as_deref() == Some("foldops")
}

#[cfg(test)]
pub fn set_test_username(name: Option<&str>) {
    let mut guard = TEST_USERNAME.lock().expect("test username lock");
    *guard = name.map(str::to_string);
}

#[cfg(test)]
pub fn clear_policy_cache() {
    // Policy cache is process-global; tests use unique policy paths via direct parse tests.
}

pub fn require_inspectable_role(paths: &AppliancePaths) -> Result<(), String> {
    if is_foldops_automation_user() {
        return Ok(());
    }
    let role = read_active_installation_role(paths)?;
    if role == "agent" || role == "supervisor" {
        return Ok(());
    }
    Err(format!(
        "operation requires agent or supervisor role, found \"{role}\""
    ))
}

pub fn require_supervisor_automation_mutation(
    paths: &AppliancePaths,
    command_group: &str,
    command_name: &str,
) -> Result<(), String> {
    if !is_foldops_automation_user() {
        return Ok(());
    }
    require_supervisor_role(paths)?;
    let policy = load_automation_policy(paths)?;
    if policy.service_user != "foldops" {
        return Err(format!(
            "automation policy service_user must be foldops, found \"{}\"",
            policy.service_user
        ));
    }
    if policy.installation_role != "supervisor" {
        return Err(format!(
            "automation policy installation_role must be supervisor, found \"{}\"",
            policy.installation_role
        ));
    }
    let command_group = command_group.trim();
    let command_name = command_name.trim();
    if policy
        .commands
        .iter()
        .any(|command| command.group == command_group && command.name == command_name)
    {
        return Ok(());
    }
    Err(format!(
        "automation policy does not authorize {command_group} {command_name} for the foldops user"
    ))
}

fn load_automation_policy(paths: &AppliancePaths) -> Result<AutomationPolicy, String> {
    let content = fs::read_to_string(&paths.automation_policy)
        .map_err(|error| format!("automation policy is unavailable: {error}"))?;
    parse_automation_policy(&content)
}

pub fn parse_automation_policy(content: &str) -> Result<AutomationPolicy, String> {
    let mut policy = AutomationPolicy {
        schema_version: 0,
        service_user: String::new(),
        installation_role: String::new(),
        commands: Vec::new(),
    };
    let mut current = AutomationPolicyCommand {
        group: String::new(),
        name: String::new(),
    };
    let mut in_command = false;
    let mut command_seen_group = false;
    let mut command_seen_name = false;

    for (number, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("[[") {
            if line != "[[commands]]" {
                return Err(format!(
                    "line {}: unsupported automation policy table \"{line}\"",
                    number + 1
                ));
            }
            if in_command {
                if !command_seen_group || !command_seen_name {
                    return Err(format!(
                        "line {}: command entry is missing group or name",
                        number + 1
                    ));
                }
                policy.commands.push(current.clone());
                current = AutomationPolicyCommand {
                    group: String::new(),
                    name: String::new(),
                };
                command_seen_group = false;
                command_seen_name = false;
            }
            in_command = true;
            continue;
        }
        let Some((key, value_raw)) = line.split_once('=') else {
            return Err(format!("line {}: expected key = value", number + 1));
        };
        let key = key.trim();
        let value = parse_policy_scalar(value_raw.trim(), number + 1)?;
        if in_command {
            match key {
                "group" => {
                    if command_seen_group {
                        return Err(format!("line {}: duplicate key \"group\"", number + 1));
                    }
                    command_seen_group = true;
                    current.group = value;
                }
                "name" => {
                    if command_seen_name {
                        return Err(format!("line {}: duplicate key \"name\"", number + 1));
                    }
                    command_seen_name = true;
                    current.name = value;
                }
                other => {
                    return Err(format!("line {}: unknown command key \"{other}\"", number + 1));
                }
            }
            continue;
        }
        match key {
            "schema_version" => {
                policy.schema_version = value
                    .parse()
                    .map_err(|_| format!("line {}: schema_version must be an integer", number + 1))?;
            }
            "service_user" => policy.service_user = value,
            "installation_role" => policy.installation_role = value,
            other => return Err(format!("line {}: unknown policy key \"{other}\"", number + 1)),
        }
    }
    if in_command {
        if !command_seen_group || !command_seen_name {
            return Err(format!(
                "line {}: command entry is missing group or name",
                content.lines().count()
            ));
        }
        policy.commands.push(current);
    }
    if policy.schema_version != 1 {
        return Err(format!(
            "unsupported automation policy schema version {}",
            policy.schema_version
        ));
    }
    if policy.service_user.is_empty() {
        policy.service_user = "foldops".into();
    }
    if policy.installation_role.is_empty() {
        policy.installation_role = "supervisor".into();
    }
    if policy.commands.is_empty() {
        return Err("automation policy defines no commands".into());
    }
    Ok(policy)
}

fn parse_policy_scalar(raw: &str, line_number: usize) -> Result<String, String> {
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        return Ok(raw[1..raw.len() - 1].to_string());
    }
    if raw.is_empty() {
        return Err(format!("line {line_number}: expected quoted string value"));
    }
    Ok(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_POLICY: &str = r#"schema_version = 1
service_user = "foldops"
installation_role = "supervisor"

[[commands]]
group = "provision"
name = "assign"

[[commands]]
group = "provision"
name = "allow-boot"
"#;

    #[test]
    fn parses_supervisor_automation_policy() {
        let policy = parse_automation_policy(TEST_POLICY).expect("parse policy");
        assert_eq!(policy.service_user, "foldops");
        assert_eq!(policy.installation_role, "supervisor");
        assert_eq!(policy.commands.len(), 2);
    }

    #[test]
    fn operator_user_bypasses_automation_policy() {
        let paths = AppliancePaths::default();
        set_test_username(Some("foldingos-admin"));
        assert!(require_supervisor_automation_mutation(&paths, "provision", "assign").is_ok());
        set_test_username(None);
    }

    #[test]
    fn denies_unlisted_foldops_command() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-policy-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/installation-role"), "supervisor").unwrap();
        std::fs::write(root.join("automation-policy.toml"), TEST_POLICY).unwrap();
        let paths = AppliancePaths {
            active_installation_role: root.join("config/installation-role"),
            automation_policy: root.join("automation-policy.toml"),
            ..AppliancePaths::default()
        };
        set_test_username(Some("foldops"));
        assert!(require_supervisor_automation_mutation(&paths, "provision", "install").is_err());
        assert!(require_supervisor_automation_mutation(&paths, "provision", "assign").is_ok());
        set_test_username(None);
        let _ = std::fs::remove_dir_all(root);
    }
}
