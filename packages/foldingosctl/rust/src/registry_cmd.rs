use crate::paths::AppliancePaths;
use crate::registry_image::{list_registry, show_registry};
use crate::role::require_supervisor_role;

pub fn run(paths: &AppliancePaths, subcommand: &str, args: &[String]) -> Result<serde_json::Value, String> {
    require_supervisor_role(paths)?;
    match subcommand {
        "list" => {
            if !args.is_empty() {
                return Err(format!("unknown registry option {:?}", args[0]));
            }
            list_registry(paths)
        }
        "show" => {
            let version = args
                .first()
                .ok_or_else(|| "registry show requires a version".to_string())?;
            if args.len() > 1 {
                return Err(format!("unknown registry option {:?}", args[1]));
            }
            show_registry(paths, version)
        }
        other => Err(format!("unknown registry subcommand {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::registry_image::RegistryEntry;
    use crate::role::require_supervisor_role;

    #[test]
    fn list_registry_returns_empty_versions() {
        let root = std::env::temp_dir().join(format!("foldingosctl-registry-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = AppliancePaths {
            active_installation_role: root.join("config/installation-role"),
            registry_index: root.join("registry/index.json"),
            registry_entries_dir: root.join("registry/entries"),
            ..AppliancePaths::default()
        };
        fs::create_dir_all(paths.active_installation_role.parent().unwrap()).unwrap();
        fs::write(&paths.active_installation_role, "supervisor").unwrap();
        require_supervisor_role(&paths).unwrap();

        let data = list_registry(&paths).unwrap();
        assert_eq!(data["versions"], serde_json::json!([]));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn show_registry_returns_entry() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-registry-show-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = AppliancePaths {
            registry_index: root.join("registry/index.json"),
            registry_entries_dir: root.join("registry/entries"),
            ..AppliancePaths::default()
        };
        fs::create_dir_all(&paths.registry_entries_dir).unwrap();
        let entry = RegistryEntry {
            schema_version: 1,
            foldingos_version: "0.2.0".into(),
            git_revision: "abc123".into(),
            image_sha256: "a".repeat(64),
            image_size_bytes: 1000,
            retrieval_url: String::new(),
            verification_method: "sha256".into(),
            import_timestamp: "2026-01-01T00:00:00Z".into(),
            rollout_state: "ready".into(),
            local_image_path: "/data/registry/images/0.2.0.squashfs".into(),
        };
        fs::write(
            paths.registry_entry_path("0.2.0"),
            serde_json::to_string_pretty(&entry).unwrap() + "\n",
        )
        .unwrap();
        fs::write(
            &paths.registry_index,
            r#"{"schema_version":1,"versions":["0.2.0"]}
"#,
        )
        .unwrap();

        let data = show_registry(&paths, "0.2.0").unwrap();
        assert_eq!(data["foldingos_version"], "0.2.0");
        assert_eq!(data["rollout_state"], "ready");

        let _ = fs::remove_dir_all(root);
    }
}
