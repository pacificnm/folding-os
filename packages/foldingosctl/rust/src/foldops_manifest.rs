use std::collections::{HashMap, HashSet};
use std::path::Path;

use regex::Regex;
use std::sync::LazyLock;

const SCHEMA_VERSION_V1: i32 = 1;
const SCHEMA_VERSION_V2: i32 = 2;
const ARTIFACT_FORMAT_DEB: &str = "deb";
const ARTIFACT_FORMAT_LAYOUT: &str = "layout-tar-zst";
const ARCHITECTURE: &str = "x86_64";
const MINIMUM_FOLDINGOS_VERSION: &str = "0.1.0";
const APPROVED_DEB_ORIGIN: &str = "deb.folding-os.com";
const APPROVED_PACKAGES_ORIGIN: &str = "packages.folding-os.com";
const VERIFICATION_PATH_PREFIX: &str = "/data/apps/foldops/current/";

static SHA256_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").expect("sha256 pattern compiles"));

#[derive(Debug, Clone)]
pub struct FoldOpsPackage {
    pub name: String,
    pub version: String,
    pub roles: Vec<String>,
    pub install_prefix: String,
    pub artifact_url: String,
    pub artifact_size: i64,
    pub sha256: String,
    pub verification_path: String,
}

#[derive(Debug, Clone)]
pub struct FoldOpsManifest {
    pub schema_version: i32,
    pub manifest_release: String,
    pub architecture: String,
    pub artifact_format: String,
    pub minimum_foldingos_version: String,
    pub packages: Vec<FoldOpsPackage>,
}

fn required_package_roles() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        ("foldops-agent", vec!["agent", "supervisor"]),
        ("foldops-supervisor", vec!["supervisor"]),
        ("foldops-web", vec!["supervisor"]),
    ])
}

pub fn parse_foldops_manifest(content: &str) -> Result<FoldOpsManifest, String> {
    let allowed_header = [
        "schema_version",
        "manifest_release",
        "architecture",
        "artifact_format",
        "minimum_foldingos_version",
    ]
    .into_iter()
    .collect::<HashSet<_>>();
    let allowed_package = [
        "name",
        "version",
        "roles",
        "install_prefix",
        "artifact_url",
        "artifact_size",
        "sha256",
        "verification_path",
    ]
    .into_iter()
    .collect::<HashSet<_>>();

    let mut manifest = FoldOpsManifest {
        schema_version: 0,
        manifest_release: String::new(),
        architecture: String::new(),
        artifact_format: String::new(),
        minimum_foldingos_version: String::new(),
        packages: Vec::new(),
    };
    let mut header_seen = HashSet::new();
    let mut current = FoldOpsPackage {
        name: String::new(),
        version: String::new(),
        roles: Vec::new(),
        install_prefix: String::new(),
        artifact_url: String::new(),
        artifact_size: 0,
        sha256: String::new(),
        verification_path: String::new(),
    };
    let mut package_seen = HashSet::new();
    let mut in_package = false;
    let mut in_roles = false;
    let mut role_lines = Vec::new();

    let flush_package = |line_number: usize,
                             manifest: &mut FoldOpsManifest,
                             current: &mut FoldOpsPackage,
                             package_seen: &mut HashSet<&str>,
                             in_package: &mut bool,
                             in_roles: &mut bool,
                             role_lines: &mut Vec<String>|
     -> Result<(), String> {
        if !*in_package {
            return Ok(());
        }
        for key in [
            "name",
            "version",
            "roles",
            "artifact_url",
            "artifact_size",
            "sha256",
            "verification_path",
        ] {
            if !package_seen.contains(key) {
                return Err(format!(
                    "line {line_number}: package is missing required key \"{key}\""
                ));
            }
        }
        manifest.packages.push(current.clone());
        *current = FoldOpsPackage {
            name: String::new(),
            version: String::new(),
            roles: Vec::new(),
            install_prefix: String::new(),
            artifact_url: String::new(),
            artifact_size: 0,
            sha256: String::new(),
            verification_path: String::new(),
        };
        package_seen.clear();
        *in_package = false;
        *in_roles = false;
        role_lines.clear();
        Ok(())
    };

    for (number, raw) in content.lines().enumerate() {
        let line_number = number + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("[[") {
            if line != "[[packages]]" {
                return Err(format!("line {line_number}: unsupported manifest table \"{line}\""));
            }
            flush_package(
                line_number,
                &mut manifest,
                &mut current,
                &mut package_seen,
                &mut in_package,
                &mut in_roles,
                &mut role_lines,
            )?;
            in_package = true;
            continue;
        }
        if in_roles {
            role_lines.push(line.to_string());
            if line.contains(']') {
                let roles = parse_foldops_roles(&role_lines.join("\n"))?;
                if package_seen.contains("roles") {
                    return Err(format!("line {line_number}: duplicate key \"roles\""));
                }
                current.roles = roles;
                package_seen.insert("roles");
                in_roles = false;
                role_lines.clear();
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {line_number}: expected key = value"));
        };
        let key = key.trim();
        let value = value.trim();

        if in_package {
            if !allowed_package.contains(key) {
                return Err(format!("line {line_number}: unknown package key \"{key}\""));
            }
            if package_seen.contains(key) {
                return Err(format!("line {line_number}: duplicate key \"{key}\""));
            }
            if key == "roles" {
                if value.starts_with('[') {
                    if value.ends_with(']') && !value.ends_with('[') {
                        current.roles = parse_foldops_roles(value)?;
                        package_seen.insert("roles");
                        continue;
                    }
                    in_roles = true;
                    role_lines = vec![value.to_string()];
                    if value.contains(']') {
                        current.roles = parse_foldops_roles(&role_lines.join("\n"))?;
                        package_seen.insert("roles");
                        in_roles = false;
                        role_lines.clear();
                    }
                    continue;
                }
                return Err(format!("line {line_number}: roles must be a TOML array"));
            }
            package_seen.insert(key);
            match key {
                "artifact_size" => {
                    let parsed: i64 = value
                        .parse()
                        .map_err(|_| format!("line {line_number}: artifact_size must be a positive integer"))?;
                    if parsed <= 0 {
                        return Err(format!(
                            "line {line_number}: artifact_size must be a positive integer"
                        ));
                    }
                    current.artifact_size = parsed;
                }
                "name" | "version" | "artifact_url" | "sha256" | "verification_path"
                | "install_prefix" => {
                    let parsed = parse_quoted_string(value).map_err(|_| {
                        format!("line {line_number}: \"{key}\" must be a quoted string")
                    })?;
                    match key {
                        "name" => current.name = parsed,
                        "version" => current.version = parsed,
                        "artifact_url" => current.artifact_url = parsed,
                        "sha256" => current.sha256 = parsed,
                        "verification_path" => current.verification_path = parsed,
                        "install_prefix" => current.install_prefix = parsed,
                        _ => {}
                    }
                }
                _ => {}
            }
            continue;
        }

        if !allowed_header.contains(key) {
            return Err(format!("line {line_number}: unknown key \"{key}\""));
        }
        if header_seen.contains(key) {
            return Err(format!("line {line_number}: duplicate key \"{key}\""));
        }
        header_seen.insert(key);
        match key {
            "schema_version" => {
                manifest.schema_version = value.parse().map_err(|_| {
                    format!("line {line_number}: schema_version must be an integer")
                })?;
            }
            "manifest_release" | "architecture" | "artifact_format" | "minimum_foldingos_version" => {
                let parsed = parse_quoted_string(value).map_err(|_| {
                    format!("line {line_number}: \"{key}\" must be a quoted string")
                })?;
                match key {
                    "manifest_release" => manifest.manifest_release = parsed,
                    "architecture" => manifest.architecture = parsed,
                    "artifact_format" => manifest.artifact_format = parsed,
                    "minimum_foldingos_version" => manifest.minimum_foldingos_version = parsed,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    if in_roles {
        return Err("manifest roles array is not closed".into());
    }
    flush_package(
        content.lines().count(),
        &mut manifest,
        &mut current,
        &mut package_seen,
        &mut in_package,
        &mut in_roles,
        &mut role_lines,
    )?;
    for key in allowed_header {
        if !header_seen.contains(key) {
            return Err(format!("missing required key \"{key}\""));
        }
    }
    Ok(manifest)
}

fn parse_foldops_roles(array_literal: &str) -> Result<Vec<String>, String> {
    let array_literal = array_literal.trim();
    if !array_literal.starts_with('[') || !array_literal.ends_with(']') {
        return Err("roles must be a TOML array".into());
    }
    let inner = array_literal[1..array_literal.len() - 1].trim();
    if inner.is_empty() {
        return Err("roles must be a non-empty array".into());
    }
    let mut roles = Vec::new();
    for segment in split_comma_respecting_quotes(inner) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let role = parse_quoted_string(segment)
            .map_err(|_| "roles must contain only quoted strings".to_string())?;
        if role != "agent" && role != "supervisor" {
            return Err("roles must contain only agent or supervisor".into());
        }
        roles.push(role);
    }
    if roles.is_empty() {
        return Err("roles must be a non-empty array".into());
    }
    Ok(roles)
}

pub fn validate_foldops_manifest(manifest: &FoldOpsManifest) -> Result<(), String> {
    match manifest.schema_version {
        SCHEMA_VERSION_V1 if manifest.artifact_format != ARTIFACT_FORMAT_DEB => {
            return Err("manifest schema_version 1 requires artifact_format deb".into());
        }
        SCHEMA_VERSION_V2
            if manifest.artifact_format != ARTIFACT_FORMAT_LAYOUT
                && manifest.artifact_format != ARTIFACT_FORMAT_DEB =>
        {
            return Err(format!(
                "manifest schema_version 2 requires artifact_format \"{ARTIFACT_FORMAT_LAYOUT}\" or \"{ARTIFACT_FORMAT_DEB}\""
            ));
        }
        SCHEMA_VERSION_V1 | SCHEMA_VERSION_V2 => {}
        _ => return Err("manifest schema_version must be 1 or 2".into()),
    }
    if manifest.architecture != ARCHITECTURE {
        return Err("manifest architecture must be x86_64".into());
    }
    if manifest.minimum_foldingos_version != MINIMUM_FOLDINGOS_VERSION {
        return Err("manifest minimum_foldingos_version must be 0.1.0".into());
    }
    if manifest.manifest_release.trim().is_empty() {
        return Err("manifest manifest_release must be non-empty".into());
    }
    if manifest.packages.is_empty() {
        return Err("manifest packages must be non-empty".into());
    }

    let required = required_package_roles();
    let mut seen_names = HashSet::new();
    for pkg in &manifest.packages {
        validate_foldops_package(manifest, pkg)?;
        if !seen_names.insert(pkg.name.clone()) {
            return Err(format!("duplicate package name in manifest: {}", pkg.name));
        }
    }
    for name in required.keys() {
        if !seen_names.contains(*name) {
            return Err(format!("manifest is missing required package: {name}"));
        }
    }
    Ok(())
}

fn validate_foldops_package(manifest: &FoldOpsManifest, pkg: &FoldOpsPackage) -> Result<(), String> {
    let required = required_package_roles();
    let Some(expected_roles) = required.get(pkg.name.as_str()) else {
        return Err(format!("unexpected package name in manifest: {}", pkg.name));
    };
    if pkg.version.trim().is_empty() {
        return Err(format!("package {} version must be non-empty", pkg.name));
    }
    if !roles_equal(&pkg.roles, expected_roles) {
        return Err(format!(
            "package {} roles must be {:?}; found {:?}",
            pkg.name, expected_roles, pkg.roles
        ));
    }
    if !SHA256_PATTERN.is_match(&pkg.sha256) {
        return Err(format!(
            "package {} sha256 must be a 64-character lowercase hex digest",
            pkg.name
        ));
    }
    if pkg.artifact_size <= 0 {
        return Err(format!(
            "package {} artifact_size must be positive",
            pkg.name
        ));
    }

    let install_prefix = pkg.install_prefix.trim();
    if manifest.schema_version == SCHEMA_VERSION_V2
        && manifest.artifact_format == ARTIFACT_FORMAT_LAYOUT
    {
        if install_prefix.is_empty() {
            return Err(format!(
                "package {} install_prefix is required for layout-tar-zst manifests",
                pkg.name
            ));
        }
        if install_prefix != pkg.name {
            return Err(format!(
                "package {} install_prefix must match package name {:?}",
                pkg.name, pkg.name
            ));
        }
    } else if !install_prefix.is_empty() && install_prefix != pkg.name {
        return Err(format!(
            "package {} install_prefix must match package name when present",
            pkg.name
        ));
    }

    let artifact_url = url::Url::parse(&pkg.artifact_url)
        .map_err(|error| format!("package {} artifact_url is invalid: {error}", pkg.name))?;
    if artifact_url.scheme() != "https" {
        return Err(format!("package {} artifact_url must use HTTPS", pkg.name));
    }
    let expected_origin = approved_artifact_origin(&manifest.artifact_format);
    if artifact_url.host_str() != Some(expected_origin) {
        return Err(format!(
            "package {} artifact_url must use HTTPS from the approved official origin: {expected_origin}",
            pkg.name
        ));
    }
    if artifact_url.path().ends_with("/latest.deb") || artifact_url.path().ends_with("latest.deb") {
        return Err(format!(
            "package {} artifact_url must not reference an unpinned latest artifact",
            pkg.name
        ));
    }
    validate_artifact_url_path(&manifest.artifact_format, artifact_url.path(), &pkg.name)
        .map_err(|error| format!("package {}: {error}", pkg.name))?;

    let expected_prefix = format!("{VERIFICATION_PATH_PREFIX}{}/", pkg.name);
    validate_verification_path(&pkg.verification_path, &expected_prefix)
        .map_err(|error| format!("package {}: {error}", pkg.name))?;
    Ok(())
}

fn approved_artifact_origin(artifact_format: &str) -> &'static str {
    if artifact_format == ARTIFACT_FORMAT_LAYOUT {
        APPROVED_PACKAGES_ORIGIN
    } else {
        APPROVED_DEB_ORIGIN
    }
}

fn validate_artifact_url_path(
    artifact_format: &str,
    path: &str,
    package_name: &str,
) -> Result<(), String> {
    match artifact_format {
        ARTIFACT_FORMAT_DEB if !path.contains(&format!("/{package_name}/")) => Err(format!(
            "artifact_url must reference the {package_name} pool artifact"
        )),
        ARTIFACT_FORMAT_LAYOUT if !path.contains(package_name) => Err(format!(
            "artifact_url must reference the {package_name} layout bundle"
        )),
        ARTIFACT_FORMAT_DEB | ARTIFACT_FORMAT_LAYOUT => Ok(()),
        other => Err(format!("unsupported artifact_format \"{other}\"")),
    }
}

fn validate_verification_path(path: &str, expected_prefix: &str) -> Result<(), String> {
    if !path.starts_with(expected_prefix) {
        return Err(format!(
            "verification_path must remain under {expected_prefix}"
        ));
    }
    let cleaned = Path::new(path);
    if cleaned.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::CurDir
        )
    }) || path.contains("..")
    {
        return Err("verification_path must not contain path traversal".into());
    }
    if !path.starts_with(expected_prefix) {
        return Err(format!(
            "verification_path must remain under {expected_prefix}"
        ));
    }
    Ok(())
}

fn roles_equal(actual: &[String], expected: &[&str]) -> bool {
    let mut actual_sorted: Vec<_> = actual.iter().map(String::as_str).collect();
    let mut expected_sorted = expected.to_vec();
    actual_sorted.sort_unstable();
    expected_sorted.sort_unstable();
    actual_sorted == expected_sorted
}

fn split_comma_respecting_quotes(input: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    for ch in input.chars() {
        match ch {
            '"' => {
                in_string = !in_string;
                current.push(ch);
            }
            ',' if !in_string => {
                segments.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    if in_string {
        return Vec::new();
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

fn parse_quoted_string(value: &str) -> Result<String, String> {
    if !(value.starts_with('"') && value.ends_with('"') && value.len() >= 2) {
        return Err("expected non-empty quoted string".into());
    }
    let inner = &value[1..value.len() - 1];
    if inner.is_empty() {
        return Err("expected non-empty quoted string".into());
    }
    Ok(inner.to_string())
}
