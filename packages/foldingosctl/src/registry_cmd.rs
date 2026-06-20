use crate::paths::AppliancePaths;
use crate::registry_foldops_tools::{
    list_foldops_manifest_registry, list_tools_version_registry, registry_import_foldops_manifest,
    registry_import_tools_release,
};
use crate::registry_image::{list_registry, show_registry};
use crate::registry_import::import_bootstrap;
use crate::registry_poll::poll;
use crate::role::require_supervisor_role;

pub fn run(
    paths: &AppliancePaths,
    subcommand: &str,
    args: &[String],
) -> Result<RegistryOutput, String> {
    match subcommand {
        "list" => {
            require_supervisor_role(paths)?;
            if !args.is_empty() {
                return Err(format!("unknown registry option {:?}", args[0]));
            }
            Ok(RegistryOutput::Json(list_registry(paths)?))
        }
        "show" => {
            require_supervisor_role(paths)?;
            let version = args
                .first()
                .ok_or_else(|| "registry show requires a version".to_string())?;
            if args.len() > 1 {
                return Err(format!("unknown registry option {:?}", args[1]));
            }
            Ok(RegistryOutput::Json(show_registry(paths, version)?))
        }
        "import-bootstrap" => {
            import_bootstrap(paths)?;
            Ok(RegistryOutput::Silent)
        }
        "poll" => {
            poll(paths)?;
            Ok(RegistryOutput::Silent)
        }
        "list-foldops-manifests" => {
            list_foldops_manifest_registry(paths)?;
            Ok(RegistryOutput::Silent)
        }
        "list-tools-versions" => {
            list_tools_version_registry(paths)?;
            Ok(RegistryOutput::Silent)
        }
        "import-foldops-manifest" => {
            registry_import_foldops_manifest(paths, args)?;
            Ok(RegistryOutput::Silent)
        }
        "import-tools-release" => {
            registry_import_tools_release(paths, args)?;
            Ok(RegistryOutput::Silent)
        }
        other => Err(format!("unknown registry subcommand {other:?}")),
    }
}

#[derive(Debug)]
pub enum RegistryOutput {
    Json(serde_json::Value),
    Silent,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::registry_image::RegistryEntry;
    use crate::role::require_supervisor_role;

    #[test]
    fn list_registry_returns_empty_versions() {
        let root =
            std::env::temp_dir().join(format!("foldingosctl-registry-{}", std::process::id()));
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

        let RegistryOutput::Json(data) = run(&paths, "list", &[]).unwrap() else {
            panic!("expected json output");
        };
        assert_eq!(data["versions"], serde_json::json!([]));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn show_registry_returns_entry() {
        let root =
            std::env::temp_dir().join(format!("foldingosctl-registry-show-{}", std::process::id()));
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

        let RegistryOutput::Json(data) = run(&paths, "show", &[String::from("0.2.0")]).unwrap()
        else {
            panic!("expected json output");
        };
        assert_eq!(data["foldingos_version"], "0.2.0");
        assert_eq!(data["rollout_state"], "ready");

        let _ = fs::remove_dir_all(root);
    }
}
