mod commissioning;
mod fah;
mod foldops;
mod services;
mod system;
pub(crate) mod tools;
mod update;

pub use tools::{
    hash_file_at_path, resolve_effective_tools_assignment, save_tools_active_state,
    tools_installation_verified, validate_tools_assignment_public, ToolsActiveState,
    ToolsAssignment,
};

use crate::automation_policy::require_inspectable_role;
use crate::identity::read_node_identity;
use crate::paths::AppliancePaths;

pub fn run(
    paths: &AppliancePaths,
    subcommand: &str,
    args: &[String],
) -> Result<serde_json::Value, String> {
    if !args.is_empty() {
        return Err(format!("unknown inspect option {:?}", args[0]));
    }
    require_inspectable_role(paths)?;
    match subcommand {
        "node" => read_node_identity(paths),
        "system" => system::inspect_system(paths),
        "fah" => fah::inspect_fah(paths),
        "commissioning" => commissioning::inspect_commissioning(paths),
        "update" => update::inspect_update(paths),
        "foldops" => foldops::inspect_foldops(paths),
        "tools" => tools::inspect_tools(paths),
        "services" => services::inspect_services(paths),
        other => Err(format!("unknown inspect subcommand {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    use super::foldops::collect_inspect_foldops_data;
    use super::tools::collect_inspect_tools_data;
    use crate::paths::AppliancePaths;

    const VALID_FOLDOPS_MANIFEST: &str = r#"schema_version = 1
manifest_release = "0.1.0-1"
architecture = "x86_64"
artifact_format = "deb"
minimum_foldingos_version = "0.1.0"

[[packages]]
name = "foldops-agent"
version = "0.1.0-1"
roles = ["agent", "supervisor"]
install_prefix = "/opt/foldops"
artifact_url = "https://deb.folding-os.com/pool/main/f/foldops-agent/foldops-agent_0.1.0-1_amd64.deb"
artifact_size = 1000
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
verification_path = "/data/apps/foldops/current/foldops-agent/.foldingos-verified"
"#;

    const VALID_FOLDOPS_MANIFEST_V2: &str = r#"schema_version = 2
manifest_release = "0.2.0-1"
architecture = "x86_64"
artifact_format = "layout-tar-zst"
minimum_foldingos_version = "0.1.0"

[[packages]]
name = "foldops-agent"
version = "0.2.0-1"
roles = ["agent", "supervisor"]
install_prefix = "/opt/foldops"
artifact_url = "https://packages.folding-os.com/foldops/layouts/0.2.0-1/foldops-agent-layout.tar.zst"
artifact_size = 1000
sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
verification_path = "/data/apps/foldops/current/foldops-agent/.foldingos-verified"
"#;

    const TOOLS_BOOTSTRAP_JSON: &str = r#"{
  "schema_version": 1,
  "tools_version": "0.1.0",
  "artifact_url": "https://packages.folding-os.com/foldingos-tools/0.1.0/foldingosctl-x86_64",
  "artifact_size": 12000000,
  "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
}
"#;

    const TOOLS_ASSIGNED_JSON: &str = r#"{
  "schema_version": 1,
  "tools_version": "0.2.0",
  "artifact_url": "https://packages.folding-os.com/foldingos-tools/0.2.0/foldingosctl-x86_64",
  "artifact_size": 12000000,
  "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
}
"#;

    fn test_paths(root: &std::path::Path) -> AppliancePaths {
        AppliancePaths {
            foldops_embedded_manifest: root.join("bootstrap-foldops.toml"),
            foldops_assigned_manifest: root.join("assigned-foldops.toml"),
            foldops_apps_root: root.join("apps/foldops"),
            foldops_provisioned_marker: root.join("state/foldops/provisioned.json"),
            tools_bootstrap_manifest: root.join("bootstrap-tools.json"),
            tools_assigned_version: root.join("assigned-tools.json"),
            tools_binary: root.join("foldingosctl"),
            ..AppliancePaths::default()
        }
    }

    fn setup_runtime_paths(root: &std::path::Path) -> AppliancePaths {
        let paths = test_paths(root);
        fs::create_dir_all(paths.foldops_embedded_manifest.parent().unwrap()).unwrap();
        fs::write(&paths.foldops_embedded_manifest, VALID_FOLDOPS_MANIFEST).unwrap();
        fs::write(&paths.foldops_assigned_manifest, VALID_FOLDOPS_MANIFEST_V2).unwrap();
        fs::create_dir_all(paths.foldops_apps_root.join("0.2.0-1")).unwrap();
        symlink("0.2.0-1", paths.foldops_current_link()).unwrap();
        fs::create_dir_all(paths.foldops_provisioned_marker.parent().unwrap()).unwrap();
        fs::write(
            &paths.foldops_provisioned_marker,
            r#"{"schema_version":1,"role":"agent","manifest_release":"0.2.0-1","provisioned_at":"2026-01-01T00:00:00Z"}
"#,
        )
        .unwrap();
        fs::write(&paths.tools_bootstrap_manifest, TOOLS_BOOTSTRAP_JSON).unwrap();
        fs::write(&paths.tools_assigned_version, TOOLS_ASSIGNED_JSON).unwrap();
        fs::write(&paths.tools_binary, "foldingosctl-binary").unwrap();
        paths
    }

    fn assert_matches_golden(name: &str, actual: serde_json::Value) {
        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join(name);
        let expected: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&golden_path).unwrap()).unwrap();
        let mut actual_doc = serde_json::json!({
            "schema_version": 1,
            "ok": true,
            "command": expected["command"].clone(),
            "data": actual,
        });
        if name == "inspect_tools.json" {
            actual_doc["data"]["binary"]["path"] = "/usr/bin/foldingosctl".into();
            actual_doc["data"]["binary"]["mod_time_unix"] = 1704067200.into();
        }
        assert_eq!(expected, actual_doc);
    }

    #[test]
    fn inspect_foldops_json_matches_golden() {
        let root =
            std::env::temp_dir().join(format!("foldingosctl-foldops-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let paths = setup_runtime_paths(&root);
        let data = collect_inspect_foldops_data(&paths).unwrap();
        assert_matches_golden("inspect_foldops.json", data);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_tools_json_matches_golden() {
        let root =
            std::env::temp_dir().join(format!("foldingosctl-tools-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let paths = setup_runtime_paths(&root);
        let data = collect_inspect_tools_data(&paths).unwrap();
        assert_matches_golden("inspect_tools.json", data);
        let _ = std::fs::remove_dir_all(root);
    }
}
