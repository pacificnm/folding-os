mod assign;
mod authorize;
mod boot;
mod enroll;
mod enrollment_api;
mod grub_env;
mod http_server;
mod install;
mod network_boot;
mod release_image;
mod role_cmd;
mod serve;
mod ssh;
mod staged_lock;
mod targets;
mod update;
pub(crate) mod util;

use crate::paths::AppliancePaths;

pub fn run(paths: &AppliancePaths, subcommand: &str, args: &[String]) -> Result<serde_json::Value, String> {
    match subcommand {
        "list-enrollments" => assign::list_enrollments(paths),
        "assign" => assign::assign(paths, args),
        "assign-local" => assign::assign_local(paths, args),
        "list-allow-boot" => boot::list_allow_boot(paths),
        "allow-boot" => boot::allow_boot(paths, args),
        "deny-boot" => boot::deny_boot(paths, args),
        "ssh" => ssh::provision_ssh(paths),
        "role" => role_cmd::provision_role(paths),
        "serve" => serve::provision_serve(paths),
        "enroll" => enroll::provision_enroll(paths),
        "check-version" => update::provision_check_version_and_stage(paths),
        "report-update-status" => update::provision_report_update_status(paths, args),
        "apply-update" => update::provision_apply_update(paths, args),
        "boot" => network_boot::provision_boot(paths),
        "install" => install::provision_install(paths, args),
        other => Err(format!("unknown provision subcommand {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::automation_policy::set_test_username;
    use crate::enrollment::{save_enrollment_record, EnrollmentRecord};
    use crate::provision::assign::assign;
    use crate::provision::boot::{allow_boot, deny_boot, list_allow_boot};

    const TEST_AGENT_NODE_ID: &str = "550e8400-e29b-41d4-a716-446655440000";

    fn provision_test_paths(root: &Path) -> AppliancePaths {
        AppliancePaths {
            active_installation_role: root.join("config/installation-role"),
            provision_enrollments_dir: root.join("enrollments"),
            provision_enrollments_index: root.join("enrollments/index.json"),
            boot_allowlist: root.join("config/provision/boot-allowlist"),
            boot_install_disk_allowlist: root.join("config/provision/boot-install-disk-allowlist"),
            automation_policy: root.join("automation-policy.toml"),
            ..AppliancePaths::default()
        }
    }

    fn write_supervisor_role(paths: &AppliancePaths) {
        fs::create_dir_all(paths.active_installation_role.parent().unwrap()).unwrap();
        fs::write(&paths.active_installation_role, "supervisor").unwrap();
    }

    fn write_sample_enrollment(paths: &AppliancePaths) {
        fs::create_dir_all(&paths.provision_enrollments_dir).unwrap();
        let record = EnrollmentRecord {
            schema_version: 1,
            node_id: TEST_AGENT_NODE_ID.into(),
            installation_role: "agent".into(),
            registered_at: "2026-01-01T00:00:00Z".into(),
            last_seen_at: "2026-01-01T00:00:00Z".into(),
            mac_addresses: vec!["52:54:00:12:34:56".into()],
            current_image_version: "0.1.0".into(),
            foldingos_version: "0.1.0".into(),
            hostname: "folding-test".into(),
            fah_active: None,
            desired_image_version: "current".into(),
            desired_foldops_manifest_release: String::new(),
            desired_tools_version: String::new(),
            last_update_status: String::new(),
            last_update_version: String::new(),
            last_update_message: String::new(),
            last_update_at: String::new(),
        };
        save_enrollment_record(paths, record).unwrap();
    }

    #[test]
    fn list_enrollments_returns_registered_agent() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-list-enrollments-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = provision_test_paths(&root);
        write_supervisor_role(&paths);
        write_sample_enrollment(&paths);

        let data = assign::list_enrollments(&paths).unwrap();
        let enrollments = data["enrollments"].as_array().unwrap();
        assert_eq!(enrollments.len(), 1);
        assert_eq!(enrollments[0]["node_id"], TEST_AGENT_NODE_ID);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn assign_updates_desired_version() {
        let root = std::env::temp_dir().join(format!("foldingosctl-assign-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = provision_test_paths(&root);
        write_supervisor_role(&paths);
        write_sample_enrollment(&paths);
        set_test_username(Some("foldingos-admin"));

        let data = assign(
            &paths,
            &[
                "--node".into(),
                TEST_AGENT_NODE_ID.into(),
                "--version".into(),
                "current".into(),
            ],
        )
        .unwrap();
        assert_eq!(data["updated_count"], 1);
        assert_eq!(data["scope"], "node");

        set_test_username(None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_allow_boot_returns_devices() {
        let root = std::env::temp_dir().join(format!("foldingosctl-boot-list-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = provision_test_paths(&root);
        write_supervisor_role(&paths);
        set_test_username(Some("foldingos-admin"));
        allow_boot(
            &paths,
            &["00:be:43:e7:59:5e".into()],
        )
        .unwrap();
        allow_boot(
            &paths,
            &[
                "--disk".into(),
                "/dev/sda".into(),
                "52:54:00:12:34:56".into(),
            ],
        )
        .unwrap();
        set_test_username(None);

        let data = list_allow_boot(&paths).unwrap();
        let devices = data["devices"].as_array().unwrap();
        assert_eq!(devices.len(), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn allow_boot_normalizes_mac_and_is_idempotent() {
        let root = std::env::temp_dir().join(format!("foldingosctl-boot-allow-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = provision_test_paths(&root);
        write_supervisor_role(&paths);
        set_test_username(Some("foldingos-admin"));

        let data = allow_boot(&paths, &["00:be:43:e7:59:5e".into()]).unwrap();
        assert_eq!(data["mac_address"], "00:be:43:e7:59:5e");
        allow_boot(&paths, &["00-BE-43-E7-59-5E".into()]).unwrap();
        let content = fs::read_to_string(&paths.boot_allowlist).unwrap();
        assert_eq!(content, "00:be:43:e7:59:5e\n");

        set_test_username(None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn allow_boot_rejects_invalid_mac() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-boot-invalid-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = provision_test_paths(&root);
        write_supervisor_role(&paths);
        set_test_username(Some("foldingos-admin"));
        assert!(allow_boot(&paths, &["not-a-mac".into()]).is_err());
        set_test_username(None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deny_boot_removes_mac_and_install_disk_mapping() {
        let root = std::env::temp_dir().join(format!("foldingosctl-boot-deny-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = provision_test_paths(&root);
        write_supervisor_role(&paths);
        set_test_username(Some("foldingos-admin"));

        allow_boot(
            &paths,
            &[
                "--disk".into(),
                "/dev/sda".into(),
                "52:54:00:12:34:56".into(),
            ],
        )
        .unwrap();
        let data = deny_boot(&paths, &["52:54:00:12:34:56".into()]).unwrap();
        assert_eq!(data["mac_address"], "52:54:00:12:34:56");
        assert_eq!(data["already_removed"], false);

        let list = list_allow_boot(&paths).unwrap();
        assert!(list["devices"].as_array().unwrap().is_empty());
        assert!(!paths.boot_install_disk_allowlist.exists()
            || fs::read_to_string(&paths.boot_install_disk_allowlist).unwrap().trim().is_empty());

        let again = deny_boot(&paths, &["52:54:00:12:34:56".into()]).unwrap();
        assert_eq!(again["already_removed"], true);

        set_test_username(None);
        let _ = fs::remove_dir_all(root);
    }
}
