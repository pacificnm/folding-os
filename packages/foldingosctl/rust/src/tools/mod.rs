mod acquire_state;
mod download;
mod replace;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::inspect::{
    hash_file_at_path, resolve_effective_tools_assignment, save_tools_active_state,
    tools_installation_verified, ToolsActiveState,
};
use crate::paths::AppliancePaths;

use self::acquire_state::{
    clear_tools_acquire_state, defer_tools_acquisition_attempt, format_duration_rounded,
    format_unix_rfc3339_utc, load_tools_acquire_state, record_tools_acquisition_failure,
};
use self::download::{download_and_stage_tools_binary, tools_http_agent, write_staged_tools_binary};
use self::replace::atomic_replace_tools_binary;

const TOOLS_DEPENDENT_SYSTEMD_UNITS: &[&str] = &[
    "foldingos-provision.service",
    "foldingos-provision-boot.service",
    "foldingos-foldops-serve-https.service",
];

pub fn run(paths: &AppliancePaths, subcommand: &str, args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err(format!("unknown tools option {:?}", args[0]));
    }
    match subcommand {
        "acquire" => tools_acquire(paths, &ToolsAcquireHooks::default()),
        other => Err(format!("unknown tools subcommand {other:?}")),
    }
}

struct ToolsAcquireHooks {
    check_prerequisites: Box<dyn Fn() -> Result<(), String>>,
    restart_dependent_units: Box<dyn Fn() -> Result<(), String>>,
    now_unix: Box<dyn Fn() -> i64>,
    download_and_stage:
        Box<dyn Fn(&Path, &crate::inspect::ToolsAssignment) -> Result<PathBuf, String>>,
}

impl Default for ToolsAcquireHooks {
    fn default() -> Self {
        Self {
            check_prerequisites: Box::new(require_tools_acquisition_prerequisites),
            restart_dependent_units: Box::new(restart_tools_dependent_units),
            now_unix: Box::new(current_unix_timestamp),
            download_and_stage: Box::new(|downloads_dir, assignment| {
                download_and_stage_tools_binary(downloads_dir, assignment, &tools_http_agent())
            }),
        }
    }
}

fn tools_acquire(paths: &AppliancePaths, hooks: &ToolsAcquireHooks) -> Result<(), String> {
    let assignment = match resolve_effective_tools_assignment(paths)? {
        Some(assignment) => assignment,
        None => {
            println!(
                "No supervisor-assigned or bootstrap tools version is configured; image bootstrap foldingosctl remains active."
            );
            return Ok(());
        }
    };

    if tools_installation_verified(paths, &assignment) {
        clear_tools_acquire_state(&tools_acquire_state_path(paths))?;
        println!(
            "Verified foldingosctl tools release {} is already active; acquisition not required.",
            assignment.tools_version
        );
        return Ok(());
    }

    let acquire_state_path = tools_acquire_state_path(paths);
    let state = load_tools_acquire_state(&acquire_state_path)?;
    let now_unix = (hooks.now_unix)();
    if let Some(remaining) = defer_tools_acquisition_attempt(&state, now_unix)? {
        println!(
            "Tools acquisition deferred for {} (next attempt at {}).",
            format_duration_rounded(remaining),
            format_unix_rfc3339_utc(state.next_attempt_unix),
        );
        return Ok(());
    }

    if let Err(error) = (hooks.check_prerequisites)() {
        return record_tools_acquisition_failure(&acquire_state_path, &error, now_unix);
    }

    let downloads_dir = tools_downloads_dir(paths);
    let staged_path = match (hooks.download_and_stage)(&downloads_dir, &assignment) {
        Ok(path) => path,
        Err(error) => {
            return record_tools_acquisition_failure(&acquire_state_path, &error, now_unix);
        }
    };
    println!(
        "Staged verified foldingosctl {} artifact at {}.",
        assignment.tools_version,
        staged_path.display()
    );

    if let Err(error) = atomic_replace_tools_binary(&staged_path, &paths.tools_binary) {
        return record_tools_acquisition_failure(&acquire_state_path, &error, now_unix);
    }

    let digest = match hash_file_at_path(&paths.tools_binary, assignment.artifact_size) {
        Ok(digest) => digest,
        Err(error) => {
            return record_tools_acquisition_failure(&acquire_state_path, &error, now_unix);
        }
    };
    if digest != assignment.sha256 {
        return record_tools_acquisition_failure(
            &acquire_state_path,
            "installed tools binary SHA-256 digest does not match approved assignment",
            now_unix,
        );
    }

    let active_state = ToolsActiveState {
        schema_version: 1,
        tools_version: assignment.tools_version.clone(),
        sha256: assignment.sha256.clone(),
        installed_at_unix: now_unix,
    };
    if let Err(error) = save_tools_active_state(&paths.tools_active_state, active_state) {
        return record_tools_acquisition_failure(&acquire_state_path, &error, now_unix);
    }
    if let Err(error) = (hooks.restart_dependent_units)() {
        return record_tools_acquisition_failure(&acquire_state_path, &error, now_unix);
    }
    clear_tools_acquire_state(&acquire_state_path)?;
    println!(
        "Installed and verified foldingosctl tools release {} at {}.",
        assignment.tools_version,
        paths.tools_binary.display()
    );
    Ok(())
}

fn tools_acquire_state_path(paths: &AppliancePaths) -> PathBuf {
    tools_state_dir(paths).join("acquire.state")
}

fn tools_downloads_dir(paths: &AppliancePaths) -> PathBuf {
    tools_state_dir(paths).join(".downloads")
}

fn tools_state_dir(paths: &AppliancePaths) -> &Path {
    paths
        .tools_active_state
        .parent()
        .expect("tools active state path has a parent")
}

fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn require_tools_acquisition_prerequisites() -> Result<(), String> {
    let status = Command::new("systemctl")
        .args(["is-active", "--quiet", "network-online.target"])
        .status()
        .map_err(|error| format!("systemctl failed: {error}"))?;
    if !status.success() {
        return Err("network is not online".into());
    }

    let value = crate::process::command_output(
        "timedatectl",
        &["show", "-p", "NTPSynchronized", "--value"],
    )
    .map_err(|error| format!("check time synchronization: {error}"))?;
    if value.trim() != "yes" {
        return Err("system time is not synchronized".into());
    }
    Ok(())
}

fn restart_tools_dependent_units() -> Result<(), String> {
    for unit in TOOLS_DEPENDENT_SYSTEMD_UNITS {
        let status = Command::new("systemctl")
            .args(["try-restart", unit])
            .status()
            .map_err(|error| format!("restart {unit}: {error}"))?;
        if !status.success() {
            return Err(format!("restart {unit}: {status}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Mutex};

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::inspect::{
        parse_tools_assignment, save_tools_active_state, validate_tools_assignment_public,
        ToolsActiveState,
    };

    const VALID_TOOLS_ASSIGNMENT_JSON: &str = r#"{
  "schema_version": 1,
  "tools_version": "0.2.0",
  "artifact_url": "https://packages.folding-os.com/foldingos-tools/0.2.0/foldingosctl-x86_64",
  "artifact_size": 12345,
  "sha256": "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a"
}"#;

    fn test_elf_bytes() -> Vec<u8> {
        vec![
            0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 62, 0,
        ]
    }

    fn test_paths(root: &Path) -> AppliancePaths {
        AppliancePaths {
            tools_bootstrap_manifest: root.join("bootstrap.json"),
            tools_assigned_version: root.join("assigned.json"),
            tools_active_state: root.join("state/tools/active.json"),
            tools_binary: root.join("foldingosctl"),
            ..AppliancePaths::default()
        }
    }

    #[test]
    fn parse_tools_assignment_accepts_valid_manifest() {
        let assignment = parse_tools_assignment(VALID_TOOLS_ASSIGNMENT_JSON.as_bytes()).unwrap();
        validate_tools_assignment_public(&assignment).unwrap();
        assert_eq!(assignment.tools_version, "0.2.0");
    }

    #[test]
    fn reject_invalid_tools_assignment_origin() {
        let content = r#"{
  "schema_version": 1,
  "tools_version": "0.2.0",
  "artifact_url": "https://evil.example/foldingos-tools/0.2.0/foldingosctl-x86_64",
  "artifact_size": 12345,
  "sha256": "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a"
}"#;
        let assignment = parse_tools_assignment(content.as_bytes()).unwrap();
        assert!(validate_tools_assignment_public(&assignment).is_err());
    }

    #[test]
    fn resolve_effective_tools_assignment_prefers_assigned() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-tools-resolve-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            &root.join("bootstrap.json"),
            r#"{
  "schema_version": 1,
  "tools_version": "0.1.0",
  "artifact_url": "https://packages.folding-os.com/foldingos-tools/0.1.0/foldingosctl-x86_64",
  "artifact_size": 1000,
  "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
}"#,
        )
        .unwrap();
        fs::write(&root.join("assigned.json"), VALID_TOOLS_ASSIGNMENT_JSON).unwrap();
        let paths = test_paths(&root);
        let assignment = resolve_effective_tools_assignment(&paths).unwrap().unwrap();
        assert_eq!(assignment.tools_version, "0.2.0");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tools_acquire_without_assignment_succeeds() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-tools-no-assignment-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = test_paths(&root);
        tools_acquire(&paths, &ToolsAcquireHooks::default()).unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tools_installation_verified_requires_active_state_and_matching_binary() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-tools-verified-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = test_paths(&root);
        let payload = test_elf_bytes();
        let mut assignment = parse_tools_assignment(VALID_TOOLS_ASSIGNMENT_JSON.as_bytes()).unwrap();
        assignment.artifact_size = payload.len() as i64;
        assignment.sha256 = format!("{:x}", Sha256::digest(&payload));
        fs::write(&paths.tools_binary, &payload).unwrap();

        assert!(!tools_installation_verified(&paths, &assignment));

        fs::create_dir_all(paths.tools_active_state.parent().unwrap()).unwrap();
        save_tools_active_state(
            &paths.tools_active_state,
            ToolsActiveState {
                schema_version: 1,
                tools_version: assignment.tools_version.clone(),
                sha256: assignment.sha256.clone(),
                installed_at_unix: 0,
            },
        )
        .unwrap();
        assert!(tools_installation_verified(&paths, &assignment));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tools_acquire_installs_verified_binary() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-tools-acquire-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let artifact = test_elf_bytes();
        let mut assignment =
            parse_tools_assignment(VALID_TOOLS_ASSIGNMENT_JSON.as_bytes()).unwrap();
        assignment.artifact_size = artifact.len() as i64;
        assignment.sha256 = format!("{:x}", Sha256::digest(&artifact));

        let paths = test_paths(&root);
        fs::create_dir_all(paths.tools_assigned_version.parent().unwrap()).unwrap();
        fs::write(
            &paths.tools_assigned_version,
            serde_json::to_string(&assignment).unwrap(),
        )
        .unwrap();
        fs::write(&paths.tools_binary, b"bootstrap-binary").unwrap();

        let restarted = Arc::new(Mutex::new(false));
        let restarted_flag = Arc::clone(&restarted);
        let staged_artifact = artifact.clone();
        let hooks = ToolsAcquireHooks {
            check_prerequisites: Box::new(|| Ok(())),
            restart_dependent_units: Box::new(move || {
                *restarted_flag.lock().unwrap() = true;
                Ok(())
            }),
            now_unix: Box::new(|| 1_704_067_200),
            download_and_stage: Box::new(move |downloads_dir, assignment| {
                write_staged_tools_binary(downloads_dir, assignment, &staged_artifact)
            }),
        };

        tools_acquire(&paths, &hooks).unwrap();
        assert!(*restarted.lock().unwrap());
        let content = fs::read(&paths.tools_binary).unwrap();
        assert_eq!(content, artifact);

        let restarted_flag = Arc::new(Mutex::new(false));
        let artifact_for_second = artifact.clone();
        let hooks = ToolsAcquireHooks {
            check_prerequisites: Box::new(|| Ok(())),
            restart_dependent_units: Box::new({
                let restarted_flag = Arc::clone(&restarted_flag);
                move || {
                    *restarted_flag.lock().unwrap() = true;
                    Ok(())
                }
            }),
            now_unix: Box::new(|| 1_704_067_200),
            download_and_stage: Box::new(move |downloads_dir, assignment| {
                write_staged_tools_binary(downloads_dir, assignment, &artifact_for_second)
            }),
        };
        tools_acquire(&paths, &hooks).unwrap();
        assert!(!*restarted_flag.lock().unwrap());

        let _ = fs::remove_dir_all(root);
    }
}
