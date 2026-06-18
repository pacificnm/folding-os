use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::foldops_manifest::{parse_foldops_manifest, validate_foldops_manifest};
use crate::fs_atomic::{atomic_write, contains_string};
use crate::inspect::{hash_file_at_path, validate_tools_assignment_public, ToolsAssignment};
use crate::paths::AppliancePaths;
use crate::registry_image::{current_import_timestamp, validate_rollout_state};
use crate::role::require_supervisor_role;

const TOOLS_APPROVED_ORIGIN: &str = "packages.folding-os.com";
const TOOLS_ARTIFACT_BASENAME: &str = "foldingosctl-x86_64";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldOpsManifestRegistryEntry {
    pub schema_version: i32,
    pub manifest_release: String,
    pub manifest_toml: String,
    pub rollout_state: String,
    pub import_timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FoldOpsManifestRegistryIndex {
    schema_version: i32,
    releases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsVersionRegistryEntry {
    pub schema_version: i32,
    pub tools_version: String,
    pub assignment: ToolsAssignment,
    pub rollout_state: String,
    pub import_timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolsVersionRegistryIndex {
    schema_version: i32,
    versions: Vec<String>,
}

pub fn list_foldops_manifest_registry(paths: &AppliancePaths) -> Result<(), String> {
    require_supervisor_role(paths)?;
    let index = load_foldops_manifest_registry_index(paths)?;
    if index.releases.is_empty() {
        println!("No FoldOps manifest releases in registry.");
        return Ok(());
    }
    let mut releases = index.releases;
    releases.sort();
    for release in releases {
        let entry = load_foldops_manifest_registry_entry(paths, &release)?;
        println!(
            "{}\trollout={}",
            entry.manifest_release, entry.rollout_state
        );
    }
    Ok(())
}

pub fn registry_import_foldops_manifest(
    paths: &AppliancePaths,
    args: &[String],
) -> Result<(), String> {
    require_supervisor_role(paths)?;
    let (manifest_path, cleanup) = resolve_foldops_manifest_import_source(args)?;
    let result = import_foldops_manifest_file(paths, &manifest_path);
    if cleanup {
        let _ = fs::remove_file(&manifest_path);
    }
    result
}

fn resolve_foldops_manifest_import_source(args: &[String]) -> Result<(PathBuf, bool), String> {
    let mut manifest_path = None;
    let mut manifest_url = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --manifest".to_string())?;
                manifest_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--url" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --url".to_string())?;
                manifest_url = Some(value.clone());
                index += 2;
            }
            other => return Err(format!("unknown import-foldops-manifest option {other:?}")),
        }
    }
    if manifest_path.is_some() && manifest_url.is_some() {
        return Err("import-foldops-manifest requires either --manifest or --url, not both".into());
    }
    if let Some(path) = manifest_path {
        return Ok((path, false));
    }
    let url = manifest_url.ok_or_else(|| "import-foldops-manifest requires --manifest or --url".to_string())?;
    let content = fetch_https_text(&url, "FoldOps manifest")?;
    let temp_path = std::env::temp_dir().join(format!(
        "foldingos-foldops-manifest-import-{}.toml",
        std::process::id()
    ));
    fs::write(&temp_path, content).map_err(|error| format!("write temporary manifest: {error}"))?;
    Ok((temp_path, true))
}

fn import_foldops_manifest_file(paths: &AppliancePaths, manifest_path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(manifest_path)
        .map_err(|error| format!("read manifest: {error}"))?;
    let manifest = parse_foldops_manifest(&content)?;
    validate_foldops_manifest(&manifest)?;
    let entry = FoldOpsManifestRegistryEntry {
        schema_version: 1,
        manifest_release: manifest.manifest_release,
        manifest_toml: content.trim().to_string(),
        rollout_state: "ready".into(),
        import_timestamp: current_import_timestamp(),
    };
    save_foldops_manifest_registry_entry(paths, entry.clone())?;
    println!(
        "Imported FoldOps manifest release {:?} into the supervisor registry.",
        entry.manifest_release
    );
    Ok(())
}

fn fetch_https_text(url: &str, label: &str) -> Result<String, String> {
    use std::io::Read;

    let url = url.trim();
    if !url.starts_with("https://") {
        return Err(format!("{label} URL must use HTTPS"));
    }
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let response = agent
        .get(url)
        .call()
        .map_err(|error| format!("download {label}: {error}"))?;
    if response.status() != 200 {
        return Err(format!(
            "{label} download failed with status {}",
            response.status()
        ));
    }
    let mut body = String::new();
    response
        .into_reader()
        .take(1 << 20)
        .read_to_string(&mut body)
        .map_err(|error| format!("read {label}: {error}"))?;
    Ok(body)
}

pub fn list_tools_version_registry(paths: &AppliancePaths) -> Result<(), String> {
    require_supervisor_role(paths)?;
    let index = load_tools_version_registry_index(paths)?;
    if index.versions.is_empty() {
        println!("No foldingosctl tools releases in registry.");
        return Ok(());
    }
    let mut versions = index.versions;
    versions.sort();
    for version in versions {
        let entry = load_tools_version_registry_entry(paths, &version)?;
        println!(
            "{}\trollout={}\tsize={}",
            entry.tools_version, entry.rollout_state, entry.assignment.artifact_size
        );
    }
    Ok(())
}

pub fn registry_import_tools_release(
    paths: &AppliancePaths,
    args: &[String],
) -> Result<(), String> {
    require_supervisor_role(paths)?;
    let (release_dir, version) = parse_tools_release_args(args)?;
    validate_tools_version_label(&version)?;

    let binary_path = release_dir.join(TOOLS_ARTIFACT_BASENAME);
    let metadata = fs::metadata(&binary_path)
        .map_err(|error| format!("tools release binary is missing: {error}"))?;
    if !metadata.is_file() {
        return Err("tools release binary is not a regular file".into());
    }
    let digest = hash_file_at_path(&binary_path, metadata.len() as i64)?;
    verify_tools_artifact_digest_matches_checksums(&release_dir, &digest)?;

    let assignment = ToolsAssignment {
        schema_version: 1,
        tools_version: version.clone(),
        artifact_url: format!(
            "https://{TOOLS_APPROVED_ORIGIN}/foldingos-tools/{version}/{TOOLS_ARTIFACT_BASENAME}"
        ),
        artifact_size: metadata.len() as i64,
        sha256: digest,
    };
    let entry = ToolsVersionRegistryEntry {
        schema_version: 1,
        tools_version: version.clone(),
        assignment,
        rollout_state: "ready".into(),
        import_timestamp: current_import_timestamp(),
    };
    save_tools_version_registry_entry(paths, entry)?;
    println!("Imported foldingosctl tools release {version:?} into the supervisor registry.");
    Ok(())
}

fn parse_tools_release_args(args: &[String]) -> Result<(PathBuf, String), String> {
    let mut release_dir = None;
    let mut version = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dir" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --dir".to_string())?;
                release_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--version" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --version".to_string())?;
                version = Some(value.clone());
                index += 2;
            }
            other => return Err(format!("unknown import-tools-release option {other:?}")),
        }
    }
    let release_dir = release_dir.ok_or_else(|| "import-tools-release requires --dir".to_string())?;
    let version = version.unwrap_or_else(|| {
        release_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string()
    });
    Ok((release_dir, version))
}

fn load_foldops_manifest_registry_index(
    paths: &AppliancePaths,
) -> Result<FoldOpsManifestRegistryIndex, String> {
    match fs::read_to_string(&paths.foldops_registry_index) {
        Ok(content) => {
            let index: FoldOpsManifestRegistryIndex = serde_json::from_str(&content)
                .map_err(|error| format!("invalid foldops manifest registry index: {error}"))?;
            if index.schema_version != 1 {
                return Err(format!(
                    "unsupported foldops manifest registry index schema version {}",
                    index.schema_version
                ));
            }
            Ok(index)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(FoldOpsManifestRegistryIndex {
                schema_version: 1,
                releases: Vec::new(),
            })
        }
        Err(error) => Err(error.to_string()),
    }
}

fn save_foldops_manifest_registry_index(
    paths: &AppliancePaths,
    index: &FoldOpsManifestRegistryIndex,
) -> Result<(), String> {
    let mut index = index.clone();
    index.schema_version = 1;
    index.releases.sort();
    let content = serde_json::to_string_pretty(&index)
        .map_err(|error| error.to_string())?;
    atomic_write(
        &paths.foldops_registry_index,
        format!("{content}\n").as_bytes(),
        0o644,
    )
}

fn load_foldops_manifest_registry_entry(
    paths: &AppliancePaths,
    release: &str,
) -> Result<FoldOpsManifestRegistryEntry, String> {
    validate_release_label(release)?;
    let content = fs::read_to_string(paths.foldops_registry_entry_path(release))
        .map_err(|error| error.to_string())?;
    let entry: FoldOpsManifestRegistryEntry = serde_json::from_str(&content)
        .map_err(|error| format!("invalid foldops manifest registry entry: {error}"))?;
    validate_foldops_manifest_registry_entry(entry)
}

fn validate_foldops_manifest_registry_entry(
    mut entry: FoldOpsManifestRegistryEntry,
) -> Result<FoldOpsManifestRegistryEntry, String> {
    if entry.schema_version != 1 {
        return Err(format!(
            "unsupported foldops manifest registry schema version {}",
            entry.schema_version
        ));
    }
    entry.manifest_release = entry.manifest_release.trim().to_string();
    validate_release_label(&entry.manifest_release)?;
    entry.manifest_toml = entry.manifest_toml.trim().to_string();
    if entry.manifest_toml.is_empty() {
        return Err("foldops manifest registry entry is missing manifest_toml".into());
    }
    let manifest = parse_foldops_manifest(&entry.manifest_toml)
        .map_err(|error| format!("foldops manifest registry entry is invalid: {error}"))?;
    validate_foldops_manifest(&manifest)
        .map_err(|error| format!("foldops manifest registry entry is invalid: {error}"))?;
    if manifest.manifest_release != entry.manifest_release {
        return Err(format!(
            "foldops manifest release {:?} does not match registry entry {:?}",
            manifest.manifest_release, entry.manifest_release
        ));
    }
    entry.rollout_state = entry.rollout_state.trim().to_string();
    if entry.rollout_state.is_empty() {
        entry.rollout_state = "ready".into();
    }
    validate_rollout_state(&entry.rollout_state)?;
    Ok(entry)
}

fn save_foldops_manifest_registry_entry(
    paths: &AppliancePaths,
    entry: FoldOpsManifestRegistryEntry,
) -> Result<(), String> {
    let validated = validate_foldops_manifest_registry_entry(entry)?;
    let content = serde_json::to_string_pretty(&validated)
        .map_err(|error| error.to_string())?;
    let entry_path = paths.foldops_registry_entry_path(&validated.manifest_release);
    if let Some(parent) = entry_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    atomic_write(&entry_path, format!("{content}\n").as_bytes(), 0o644)?;
    let mut index = load_foldops_manifest_registry_index(paths)?;
    if !contains_string(&index.releases, &validated.manifest_release) {
        index.releases.push(validated.manifest_release);
    }
    save_foldops_manifest_registry_index(paths, &index)
}

fn load_tools_version_registry_index(
    paths: &AppliancePaths,
) -> Result<ToolsVersionRegistryIndex, String> {
    match fs::read_to_string(&paths.tools_registry_index) {
        Ok(content) => {
            let index: ToolsVersionRegistryIndex = serde_json::from_str(&content)
                .map_err(|error| format!("invalid tools version registry index: {error}"))?;
            if index.schema_version != 1 {
                return Err(format!(
                    "unsupported tools version registry index schema version {}",
                    index.schema_version
                ));
            }
            Ok(index)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ToolsVersionRegistryIndex {
            schema_version: 1,
            versions: Vec::new(),
        }),
        Err(error) => Err(error.to_string()),
    }
}

fn save_tools_version_registry_index(
    paths: &AppliancePaths,
    index: &ToolsVersionRegistryIndex,
) -> Result<(), String> {
    let mut index = index.clone();
    index.schema_version = 1;
    index.versions.sort();
    let content = serde_json::to_string_pretty(&index)
        .map_err(|error| error.to_string())?;
    atomic_write(
        &paths.tools_registry_index,
        format!("{content}\n").as_bytes(),
        0o644,
    )
}

fn load_tools_version_registry_entry(
    paths: &AppliancePaths,
    version: &str,
) -> Result<ToolsVersionRegistryEntry, String> {
    validate_tools_version_label(version)?;
    let content = fs::read_to_string(paths.tools_registry_entry_path(version))
        .map_err(|error| error.to_string())?;
    let entry: ToolsVersionRegistryEntry = serde_json::from_str(&content)
        .map_err(|error| format!("invalid tools version registry entry: {error}"))?;
    validate_tools_version_registry_entry(entry)
}

fn validate_tools_version_registry_entry(
    mut entry: ToolsVersionRegistryEntry,
) -> Result<ToolsVersionRegistryEntry, String> {
    if entry.schema_version != 1 {
        return Err(format!(
            "unsupported tools version registry schema version {}",
            entry.schema_version
        ));
    }
    entry.tools_version = entry.tools_version.trim().to_string();
    validate_tools_version_label(&entry.tools_version)?;
    validate_tools_assignment_public(&entry.assignment)?;
    if entry.assignment.tools_version != entry.tools_version {
        return Err(format!(
            "tools assignment version {:?} does not match registry entry {:?}",
            entry.assignment.tools_version, entry.tools_version
        ));
    }
    entry.rollout_state = entry.rollout_state.trim().to_string();
    if entry.rollout_state.is_empty() {
        entry.rollout_state = "ready".into();
    }
    validate_rollout_state(&entry.rollout_state)?;
    Ok(entry)
}

fn save_tools_version_registry_entry(
    paths: &AppliancePaths,
    entry: ToolsVersionRegistryEntry,
) -> Result<(), String> {
    let validated = validate_tools_version_registry_entry(entry)?;
    let content = serde_json::to_string_pretty(&validated)
        .map_err(|error| error.to_string())?;
    let entry_path = paths.tools_registry_entry_path(&validated.tools_version);
    if let Some(parent) = entry_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    atomic_write(&entry_path, format!("{content}\n").as_bytes(), 0o644)?;
    let mut index = load_tools_version_registry_index(paths)?;
    if !contains_string(&index.versions, &validated.tools_version) {
        index.versions.push(validated.tools_version);
    }
    save_tools_version_registry_index(paths, &index)
}

fn verify_tools_artifact_digest_matches_checksums(
    release_dir: &Path,
    digest: &str,
) -> Result<(), String> {
    let checksums_path = release_dir.join("SHA256SUMS");
    let content = fs::read_to_string(&checksums_path)
        .map_err(|error| format!("read SHA256SUMS: {error}"))?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 2 {
            continue;
        }
        if fields[1] != TOOLS_ARTIFACT_BASENAME {
            continue;
        }
        if fields[0] != digest {
            return Err("tools release binary SHA-256 does not match SHA256SUMS".into());
        }
        return Ok(());
    }
    Err(format!(
        "SHA256SUMS does not contain {:?}",
        TOOLS_ARTIFACT_BASENAME
    ))
}

fn validate_release_label(release: &str) -> Result<(), String> {
    if release.is_empty() || release.contains('/') || release.contains('\\') || release.contains("..")
    {
        return Err(
            "release must be non-empty and must not contain path separators or traversal".into(),
        );
    }
    Ok(())
}

fn validate_tools_version_label(version: &str) -> Result<(), String> {
    if version.is_empty() || version.contains('/') || version.contains('\\') || version.contains("..")
    {
        return Err(
            "tools version must be non-empty and must not contain path separators or traversal"
                .into(),
        );
    }
    Ok(())
}
