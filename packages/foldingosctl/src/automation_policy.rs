use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::paths::AppliancePaths;
use crate::role::{read_active_installation_role, require_agent_role, require_supervisor_role};

static TEST_USERNAME: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub(crate) struct AutomationPolicyCommand {
    group: String,
    name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AutomationPolicy {
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
    let policy = load_merged_automation_policy(paths, "supervisor")?;
    authorize_automation_policy(&policy, "supervisor", command_group, command_name)
}

pub fn require_agent_automation_mutation(
    paths: &AppliancePaths,
    command_group: &str,
    command_name: &str,
) -> Result<(), String> {
    if !is_foldops_automation_user() {
        return Ok(());
    }
    require_agent_role(paths)?;
    let policy = load_merged_automation_policy(paths, "agent")?;
    authorize_automation_policy(&policy, "agent", command_group, command_name)
}

pub fn require_config_automation_mutation(
    paths: &AppliancePaths,
    command_group: &str,
    command_name: &str,
) -> Result<(), String> {
    if !is_foldops_automation_user() {
        return Ok(());
    }
    match read_active_installation_role(paths)?.as_str() {
        "agent" => require_agent_automation_mutation(paths, command_group, command_name),
        "supervisor" => require_supervisor_automation_mutation(paths, command_group, command_name),
        other => Err(format!(
            "operation requires agent or supervisor role, found \"{other}\""
        )),
    }
}

pub fn require_acquire_automation_mutation(
    paths: &AppliancePaths,
    command_group: &str,
) -> Result<(), String> {
    if !is_foldops_automation_user() {
        return Ok(());
    }
    let role = read_active_installation_role(paths)?;
    match role.as_str() {
        "agent" => require_agent_automation_mutation(paths, command_group, "acquire"),
        "supervisor" => require_supervisor_automation_mutation(paths, command_group, "acquire"),
        other => Err(format!(
            "operation requires agent or supervisor role, found \"{other}\""
        )),
    }
}

fn authorize_automation_policy(
    policy: &AutomationPolicy,
    expected_role: &str,
    command_group: &str,
    command_name: &str,
) -> Result<(), String> {
    if policy.service_user != "foldops" {
        return Err(format!(
            "automation policy service_user must be foldops, found \"{}\"",
            policy.service_user
        ));
    }
    if policy.installation_role != expected_role {
        return Err(format!(
            "automation policy installation_role must be {expected_role}, found \"{}\"",
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
    if command_group == "provision"
        && command_name == "assign-local"
        && policy
            .commands
            .iter()
            .any(|command| command.group == "provision" && command.name == "assign")
    {
        return Ok(());
    }
    if is_core_remote_config_command(expected_role, command_group, command_name) {
        return Ok(());
    }
    Err(format!(
        "automation policy does not authorize {command_group} {command_name} for the foldops user"
    ))
}

fn is_core_remote_config_command(
    installation_role: &str,
    command_group: &str,
    command_name: &str,
) -> bool {
    matches!(installation_role, "agent" | "supervisor")
        && command_group == "config"
        && matches!(command_name, "activate" | "set-passkey")
}

fn load_automation_policy(path: &Path) -> Result<AutomationPolicy, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("automation policy is unavailable: {error}"))?;
    parse_automation_policy(&content)
}

fn load_merged_automation_policy(
    paths: &AppliancePaths,
    expected_role: &str,
) -> Result<AutomationPolicy, String> {
    let policy_paths = automation_policy_paths(paths, expected_role);
    if policy_paths.is_empty() {
        return Err(format!(
            "automation policy is unavailable for role {expected_role}"
        ));
    }

    let mut merged_commands = Vec::new();
    let mut seen = HashSet::new();
    let mut schema_version = 1;
    let mut service_user = "foldops".to_string();
    let mut installation_role = expected_role.to_string();
    let mut loaded = 0usize;

    for policy_path in policy_paths {
        let policy = load_automation_policy(&policy_path)?;
        if policy.installation_role != expected_role {
            continue;
        }
        loaded += 1;
        schema_version = policy.schema_version;
        service_user = policy.service_user.clone();
        installation_role = policy.installation_role.clone();
        for command in policy.commands {
            let key = (command.group.clone(), command.name.clone());
            if seen.insert(key) {
                merged_commands.push(command);
            }
        }
    }

    if loaded == 0 || merged_commands.is_empty() {
        return Err(format!(
            "automation policy is unavailable for role {expected_role}"
        ));
    }

    Ok(AutomationPolicy {
        schema_version,
        service_user,
        installation_role,
        commands: merged_commands,
    })
}

fn automation_policy_paths(paths: &AppliancePaths, expected_role: &str) -> Vec<PathBuf> {
    let bundled = match expected_role {
        "agent" => paths
            .foldops_apps_root
            .join("current/foldops-agent/usr/share/foldingos/foldops-agent-automation.toml"),
        "supervisor" => paths.foldops_apps_root.join(
            "current/foldops-supervisor/usr/share/foldingos/foldops-supervisor-automation.toml",
        ),
        _ => return Vec::new(),
    };
    let system_share = match expected_role {
        "agent" => paths.agent_automation_policy.clone(),
        "supervisor" => paths.automation_policy.clone(),
        _ => return Vec::new(),
    };

    let mut policy_paths = Vec::new();
    for path in [bundled, system_share] {
        if path.is_file() && !policy_paths.iter().any(|existing| existing == &path) {
            policy_paths.push(path);
        }
    }
    policy_paths
}

#[allow(dead_code)]
pub(crate) fn resolve_supervisor_automation_policy_path(paths: &AppliancePaths) -> PathBuf {
    let bundled = paths
        .foldops_apps_root
        .join("current/foldops-supervisor/usr/share/foldingos/foldops-supervisor-automation.toml");
    if bundled.is_file() {
        return bundled;
    }
    paths.automation_policy.clone()
}

#[allow(dead_code)]
pub(crate) fn resolve_agent_automation_policy_path(paths: &AppliancePaths) -> PathBuf {
    let bundled = paths
        .foldops_apps_root
        .join("current/foldops-agent/usr/share/foldingos/foldops-agent-automation.toml");
    if bundled.is_file() {
        return bundled;
    }
    paths.agent_automation_policy.clone()
}

/// Copy bundled automation policy from the active FoldOps release into
/// `/usr/share/foldingos/` when running with root privileges.
///
/// Merges bundled and existing system-share policies so an older FoldOps bundle
/// cannot strip commands that the OS image already shipped (for example
/// `config set-passkey`).
pub fn sync_automation_policies_to_system_share(paths: &AppliancePaths) -> Result<(), String> {
    for (expected_role, destination) in [
        ("supervisor", paths.automation_policy.as_path()),
        ("agent", paths.agent_automation_policy.as_path()),
    ] {
        if automation_policy_paths(paths, expected_role).is_empty() {
            continue;
        }
        sync_merged_automation_policy_to_system_share(paths, expected_role, destination)?;
    }
    Ok(())
}

fn sync_merged_automation_policy_to_system_share(
    paths: &AppliancePaths,
    expected_role: &str,
    destination: &std::path::Path,
) -> Result<(), String> {
    let merged = load_merged_automation_policy(paths, expected_role)?;
    let content = format_automation_policy(&merged);
    if destination.is_file() {
        let existing = fs::read_to_string(destination).map_err(|error| {
            format!(
                "read installed automation policy {}: {error}",
                destination.display()
            )
        })?;
        if existing == content {
            return Ok(());
        }
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create automation policy directory {}: {error}",
                parent.display()
            )
        })?;
    }
    crate::fs_atomic::atomic_write(destination, content.as_bytes(), 0o644).map_err(|error| {
        format!(
            "install automation policy {}: {error}",
            destination.display()
        )
    })
}

fn format_automation_policy(policy: &AutomationPolicy) -> String {
    let mut content = format!(
        "schema_version = {}\nservice_user = \"{}\"\ninstallation_role = \"{}\"",
        policy.schema_version, policy.service_user, policy.installation_role
    );
    for command in &policy.commands {
        content.push_str("\n\n[[commands]]\n");
        content.push_str(&format!("group = \"{}\"\n", command.group));
        content.push_str(&format!("name = \"{}\"", command.name));
    }
    content.push('\n');
    content
}

fn parse_automation_policy(content: &str) -> Result<AutomationPolicy, String> {
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
                    return Err(format!(
                        "line {}: unknown command key \"{other}\"",
                        number + 1
                    ));
                }
            }
            continue;
        }
        match key {
            "schema_version" => {
                policy.schema_version = value.parse().map_err(|_| {
                    format!("line {}: schema_version must be an integer", number + 1)
                })?;
            }
            "service_user" => policy.service_user = value,
            "installation_role" => policy.installation_role = value,
            other => {
                return Err(format!(
                    "line {}: unknown policy key \"{other}\"",
                    number + 1
                ))
            }
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
        let root =
            std::env::temp_dir().join(format!("foldingosctl-policy-test-{}", std::process::id()));
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

    #[test]
    fn prefers_bundled_supervisor_automation_policy_path() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-bundled-policy-path-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(
            root.join("apps/foldops/current/foldops-supervisor/usr/share/foldingos"),
        )
        .unwrap();
        let bundled = root.join(
            "apps/foldops/current/foldops-supervisor/usr/share/foldingos/foldops-supervisor-automation.toml",
        );
        std::fs::write(&bundled, "schema_version = 1\n").unwrap();
        let paths = AppliancePaths {
            foldops_apps_root: root.join("apps/foldops"),
            automation_policy: root.join("system-share-policy.toml"),
            ..AppliancePaths::default()
        };
        assert_eq!(resolve_supervisor_automation_policy_path(&paths), bundled);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn authorizes_services_restart_from_bundled_supervisor_policy() {
        const BUNDLED_POLICY: &str = r#"schema_version = 1
service_user = "foldops"
installation_role = "supervisor"

[[commands]]
group = "services"
name = "restart"
"#;
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-bundled-policy-restart-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(
            root.join("apps/foldops/current/foldops-supervisor/usr/share/foldingos"),
        )
        .unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/installation-role"), "supervisor").unwrap();
        std::fs::write(
            root.join("apps/foldops/current/foldops-supervisor/usr/share/foldingos/foldops-supervisor-automation.toml"),
            BUNDLED_POLICY,
        )
        .unwrap();
        std::fs::write(root.join("automation-policy.toml"), TEST_POLICY).unwrap();
        let paths = AppliancePaths {
            active_installation_role: root.join("config/installation-role"),
            foldops_apps_root: root.join("apps/foldops"),
            automation_policy: root.join("automation-policy.toml"),
            ..AppliancePaths::default()
        };
        set_test_username(Some("foldops"));
        assert!(require_supervisor_automation_mutation(&paths, "services", "restart").is_ok());
        assert!(require_supervisor_automation_mutation(&paths, "services", "restart-all").is_err());
        set_test_username(None);
        let _ = std::fs::remove_dir_all(root);
    }

    const TEST_AGENT_POLICY: &str = r#"schema_version = 1
service_user = "foldops"
installation_role = "agent"

[[commands]]
group = "config"
name = "activate"
"#;

    #[test]
    fn authorizes_agent_config_activate() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-agent-policy-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/installation-role"), "agent").unwrap();
        std::fs::write(root.join("agent-automation-policy.toml"), TEST_AGENT_POLICY).unwrap();
        let paths = AppliancePaths {
            active_installation_role: root.join("config/installation-role"),
            foldops_apps_root: root.join("apps/foldops"),
            agent_automation_policy: root.join("agent-automation-policy.toml"),
            ..AppliancePaths::default()
        };
        set_test_username(Some("foldops"));
        assert!(require_agent_automation_mutation(&paths, "config", "activate").is_ok());
        assert!(require_agent_automation_mutation(&paths, "config", "validate").is_err());
        set_test_username(None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn merges_bundled_and_system_share_agent_policy_commands() {
        const BUNDLED_POLICY: &str = r#"schema_version = 1
service_user = "foldops"
installation_role = "agent"

[[commands]]
group = "config"
name = "activate"
"#;
        const SYSTEM_POLICY: &str = r#"schema_version = 1
service_user = "foldops"
installation_role = "agent"

[[commands]]
group = "config"
name = "set-passkey"
"#;
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-merged-agent-policy-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(
            root.join("apps/foldops/current/foldops-agent/usr/share/foldingos"),
        )
        .unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/installation-role"), "agent").unwrap();
        std::fs::write(
            root.join("apps/foldops/current/foldops-agent/usr/share/foldingos/foldops-agent-automation.toml"),
            BUNDLED_POLICY,
        )
        .unwrap();
        std::fs::write(root.join("agent-automation-policy.toml"), SYSTEM_POLICY).unwrap();
        let paths = AppliancePaths {
            active_installation_role: root.join("config/installation-role"),
            foldops_apps_root: root.join("apps/foldops"),
            agent_automation_policy: root.join("agent-automation-policy.toml"),
            ..AppliancePaths::default()
        };
        set_test_username(Some("foldops"));
        assert!(require_agent_automation_mutation(&paths, "config", "activate").is_ok());
        assert!(require_agent_automation_mutation(&paths, "config", "set-passkey").is_ok());
        set_test_username(None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sync_preserves_system_share_commands_missing_from_bundled_policy() {
        const BUNDLED_POLICY: &str = r#"schema_version = 1
service_user = "foldops"
installation_role = "agent"

[[commands]]
group = "config"
name = "activate"
"#;
        const SYSTEM_POLICY: &str = r#"schema_version = 1
service_user = "foldops"
installation_role = "agent"

[[commands]]
group = "config"
name = "set-passkey"
"#;
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-sync-policy-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(
            root.join("apps/foldops/current/foldops-agent/usr/share/foldingos"),
        )
        .unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/installation-role"), "agent").unwrap();
        std::fs::write(
            root.join("apps/foldops/current/foldops-agent/usr/share/foldingos/foldops-agent-automation.toml"),
            BUNDLED_POLICY,
        )
        .unwrap();
        std::fs::write(root.join("agent-automation-policy.toml"), SYSTEM_POLICY).unwrap();
        let paths = AppliancePaths {
            active_installation_role: root.join("config/installation-role"),
            foldops_apps_root: root.join("apps/foldops"),
            agent_automation_policy: root.join("agent-automation-policy.toml"),
            ..AppliancePaths::default()
        };
        sync_automation_policies_to_system_share(&paths).expect("sync policies");
        let synced = std::fs::read_to_string(root.join("agent-automation-policy.toml")).unwrap();
        let policy = parse_automation_policy(&synced).expect("parse synced policy");
        assert!(policy
            .commands
            .iter()
            .any(|command| { command.group == "config" && command.name == "set-passkey" }));
        assert!(policy
            .commands
            .iter()
            .any(|command| { command.group == "config" && command.name == "activate" }));
        set_test_username(Some("foldops"));
        assert!(require_agent_automation_mutation(&paths, "config", "set-passkey").is_ok());
        set_test_username(None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn authorizes_core_remote_config_without_explicit_policy_entry() {
        const MINIMAL_POLICY: &str = r#"schema_version = 1
service_user = "foldops"
installation_role = "agent"

[[commands]]
group = "foldops"
name = "acquire"
"#;
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-core-config-policy-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/installation-role"), "agent").unwrap();
        std::fs::write(root.join("agent-automation-policy.toml"), MINIMAL_POLICY).unwrap();
        let paths = AppliancePaths {
            active_installation_role: root.join("config/installation-role"),
            foldops_apps_root: root.join("apps/foldops"),
            agent_automation_policy: root.join("agent-automation-policy.toml"),
            ..AppliancePaths::default()
        };
        set_test_username(Some("foldops"));
        assert!(require_agent_automation_mutation(&paths, "config", "set-passkey").is_ok());
        assert!(require_agent_automation_mutation(&paths, "config", "validate").is_err());
        set_test_username(None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn authorizes_supervisor_config_set_passkey() {
        const SUPERVISOR_POLICY: &str = r#"schema_version = 1
service_user = "foldops"
installation_role = "supervisor"

[[commands]]
group = "config"
name = "set-passkey"
"#;
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-supervisor-config-policy-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/installation-role"), "supervisor").unwrap();
        std::fs::write(root.join("automation-policy.toml"), SUPERVISOR_POLICY).unwrap();
        let paths = AppliancePaths {
            active_installation_role: root.join("config/installation-role"),
            foldops_apps_root: root.join("apps/foldops"),
            automation_policy: root.join("automation-policy.toml"),
            ..AppliancePaths::default()
        };
        set_test_username(Some("foldops"));
        assert!(require_config_automation_mutation(&paths, "config", "set-passkey").is_ok());
        set_test_username(None);
        let _ = std::fs::remove_dir_all(root);
    }
}
