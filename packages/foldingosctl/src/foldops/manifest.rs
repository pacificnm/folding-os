use std::fs;

use crate::foldops::util::{
    load_foldops_manifest_from_allowed_path, validate_foldingos_compatibility,
};
use crate::paths::AppliancePaths;

pub fn validate_foldops_manifest_embedded(paths: &AppliancePaths) -> Result<(), String> {
    let manifest = load_foldops_manifest_from_allowed_path(paths, &paths.foldops_embedded_manifest)?;
    validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;
    println!(
        "Approved FoldOps bootstrap manifest {} is valid for FoldingOS {}.",
        manifest.manifest_release, manifest.minimum_foldingos_version
    );

    if paths.foldops_assigned_manifest.exists() {
        let assigned = load_foldops_manifest_from_allowed_path(
            paths,
            &paths.foldops_assigned_manifest,
        )
        .map_err(|error| format!("assigned manifest: {error}"))?;
        validate_foldingos_compatibility(&assigned.minimum_foldingos_version)
            .map_err(|error| format!("assigned manifest: {error}"))?;
        println!(
            "Supervisor-assigned FoldOps manifest {} is valid for FoldingOS {}.",
            assigned.manifest_release, assigned.minimum_foldingos_version
        );
    } else if let Err(error) = fs::metadata(&paths.foldops_assigned_manifest) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error.to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::foldops::util::{
        load_foldops_manifest_from_allowed_path, resolve_effective_foldops_manifest,
        FOLDOPS_MANIFEST_PLACEHOLDER,
    };
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
artifact_url = "https://deb.folding-os.com/pool/main/f/foldops-agent/foldops-agent_0.1.0-1_amd64.deb"
artifact_size = 3127044
sha256 = "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a"
verification_path = "/data/apps/foldops/current/foldops-agent/usr/bin/foldops-agent"

[[packages]]
name = "foldops-supervisor"
version = "0.1.0-1"
roles = ["supervisor"]
artifact_url = "https://deb.folding-os.com/pool/main/f/foldops-supervisor/foldops-supervisor_0.1.0-1_amd64.deb"
artifact_size = 3111920
sha256 = "a8b91ec03803259ade0bc3595218d74408390f6ac4e0f077cc47ba85edaaa8d5"
verification_path = "/data/apps/foldops/current/foldops-supervisor/usr/bin/foldops-supervisor"

[[packages]]
name = "foldops-web"
version = "0.1.0"
roles = ["supervisor"]
artifact_url = "https://deb.folding-os.com/pool/main/f/foldops-web/foldops-web_0.1.0_all.deb"
artifact_size = 174466
sha256 = "e560956f0aa6f77677af9bbac464a71ebcf0ff1da19877070f6f8dc05f738ecf"
verification_path = "/data/apps/foldops/current/foldops-web/usr/share/foldops/web/index.html"
"#;

    const VALID_FOLDOPS_MANIFEST_V2: &str = r#"schema_version = 2
manifest_release = "0.2.0-1"
architecture = "x86_64"
artifact_format = "layout-tar-zst"
minimum_foldingos_version = "0.1.0"

[[packages]]
name = "foldops-agent"
version = "0.1.0"
roles = ["agent", "supervisor"]
install_prefix = "foldops-agent"
artifact_url = "https://packages.folding-os.com/foldops/0.2.0-1/foldops-agent-x86_64.tar.zst"
artifact_size = 3740000
sha256 = "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a"
verification_path = "/data/apps/foldops/current/foldops-agent/usr/bin/foldops-agent"

[[packages]]
name = "foldops-supervisor"
version = "0.1.0"
roles = ["supervisor"]
install_prefix = "foldops-supervisor"
artifact_url = "https://packages.folding-os.com/foldops/0.2.0-1/foldops-supervisor-x86_64.tar.zst"
artifact_size = 3720000
sha256 = "a8b91ec03803259ade0bc3595218d74408390f6ac4e0f077cc47ba85edaaa8d5"
verification_path = "/data/apps/foldops/current/foldops-supervisor/usr/bin/foldops-supervisor"

[[packages]]
name = "foldops-web"
version = "0.1.0"
roles = ["supervisor"]
install_prefix = "foldops-web"
artifact_url = "https://packages.folding-os.com/foldops/0.2.0-1/foldops-web-x86_64.tar.zst"
artifact_size = 174000
sha256 = "e560956f0aa6f77677af9bbac464a71ebcf0ff1da19877070f6f8dc05f738ecf"
verification_path = "/data/apps/foldops/current/foldops-web/usr/share/foldops/web/index.html"
"#;

    fn test_paths(root: &std::path::Path) -> AppliancePaths {
        AppliancePaths {
            foldops_embedded_manifest: root.join("bootstrap.toml"),
            foldops_assigned_manifest: root.join("assigned.toml"),
            ..AppliancePaths::default()
        }
    }

    #[test]
    fn reject_external_foldops_manifest_path() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-foldops-manifest-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = test_paths(&root);
        assert!(load_foldops_manifest_from_allowed_path(&paths, &root.join("external.toml")).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reject_unresolved_foldops_manifest_placeholder() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-foldops-placeholder-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = test_paths(&root);
        let content = VALID_FOLDOPS_MANIFEST.replace(
            "minimum_foldingos_version = \"0.1.0\"",
            &format!("minimum_foldingos_version = \"{FOLDOPS_MANIFEST_PLACEHOLDER}\""),
        );
        fs::write(&paths.foldops_embedded_manifest, content).unwrap();
        assert!(load_foldops_manifest_from_allowed_path(&paths, &paths.foldops_embedded_manifest).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_effective_foldops_manifest_prefers_assigned() {
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-foldops-effective-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = test_paths(&root);
        fs::write(&paths.foldops_embedded_manifest, VALID_FOLDOPS_MANIFEST).unwrap();
        fs::write(&paths.foldops_assigned_manifest, VALID_FOLDOPS_MANIFEST_V2).unwrap();
        let manifest = resolve_effective_foldops_manifest(&paths).unwrap();
        assert_eq!(manifest.manifest_release, "0.2.0-1");
        assert_eq!(manifest.artifact_format, "layout-tar-zst");
        let _ = fs::remove_dir_all(root);
    }
}
