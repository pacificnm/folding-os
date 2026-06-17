use std::collections::HashSet;
use std::fs;
use std::sync::LazyLock;

use regex::Regex;
use url::Url;

use crate::paths::AppliancePaths;

use super::util::os_release_value;

pub const FAH_MANIFEST_PLACEHOLDER: &str = "REQUIRED_BEFORE_RELEASE";
const FAH_APPROVED_ARTIFACT_ORIGIN: &str = "download.foldingathome.org";
const FAH_MANIFEST_SCHEMA_VERSION: i32 = 1;
const FAH_MANIFEST_ARCHITECTURE: &str = "x86_64";
const FAH_MANIFEST_ARTIFACT_FORMAT: &str = "deb";
const FAH_MANIFEST_MINIMUM_VERSION: &str = "0.1.0";

static FAH_SHA256_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").expect("sha256 pattern compiles"));
static FAH_CLIENT_VERSION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^8\.5\.[0-9]+$").expect("fah client version pattern compiles")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FahManifest {
    pub schema_version: i32,
    pub client_version: String,
    pub architecture: String,
    pub artifact_url: String,
    pub artifact_size: i64,
    pub sha256: String,
    pub artifact_format: String,
    pub minimum_foldingos_version: String,
    pub terms_url: String,
    pub executable_path: String,
    pub arguments: Vec<String>,
}

pub fn validate_fah_manifest_embedded(paths: &AppliancePaths) -> Result<(), String> {
    let manifest = load_fah_manifest(paths, &paths.fah_embedded_manifest)?;
    validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;
    println!(
        "Approved Folding@home manifest {} is valid for FoldingOS {}.",
        manifest.client_version, manifest.minimum_foldingos_version
    );
    Ok(())
}

pub fn load_fah_manifest(paths: &AppliancePaths, path: &std::path::Path) -> Result<FahManifest, String> {
    if path != paths.fah_embedded_manifest {
        return Err("v0.1.0 accepts only the embedded approved manifest".into());
    }
    let content = fs::read_to_string(path).map_err(|error| format!("read manifest: {error}"))?;
    if content.contains(FAH_MANIFEST_PLACEHOLDER) {
        return Err(format!(
            "manifest contains unresolved placeholder {FAH_MANIFEST_PLACEHOLDER:?}"
        ));
    }
    let manifest = parse_fah_manifest(&content)?;
    validate_fah_manifest(&manifest)?;
    Ok(manifest)
}

pub fn parse_fah_manifest(content: &str) -> Result<FahManifest, String> {
    let allowed_keys = [
        "schema_version",
        "client_version",
        "architecture",
        "artifact_url",
        "artifact_size",
        "sha256",
        "artifact_format",
        "minimum_foldingos_version",
        "terms_url",
        "executable_path",
        "arguments",
    ]
    .into_iter()
    .collect::<HashSet<_>>();

    let mut manifest = FahManifest {
        schema_version: 0,
        client_version: String::new(),
        architecture: String::new(),
        artifact_url: String::new(),
        artifact_size: 0,
        sha256: String::new(),
        artifact_format: String::new(),
        minimum_foldingos_version: String::new(),
        terms_url: String::new(),
        executable_path: String::new(),
        arguments: Vec::new(),
    };
    let mut seen = HashSet::new();
    let mut in_arguments = false;
    let mut argument_lines = Vec::new();

    for (number, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            return Err(format!(
                "line {}: manifest tables are not supported",
                number + 1
            ));
        }
        if in_arguments {
            argument_lines.push(line.to_string());
            if line.contains(']') {
                let joined = argument_lines.join("\n");
                let arguments = parse_fah_manifest_arguments(&joined)
                    .map_err(|error| format!("line {}: {error}", number + 1))?;
                if seen.contains("arguments") {
                    return Err(format!(
                        "line {}: duplicate key {:?}",
                        number + 1,
                        "arguments"
                    ));
                }
                manifest.arguments = arguments;
                seen.insert("arguments");
                in_arguments = false;
                argument_lines.clear();
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("arguments = [") {
            if seen.contains("arguments") {
                return Err(format!(
                    "line {}: duplicate key {:?}",
                    number + 1,
                    "arguments"
                ));
            }
            let array_literal = format!("[{rest}");
            if rest.ends_with(']') && rest != "[" {
                let arguments = parse_fah_manifest_arguments(&array_literal)
                    .map_err(|error| format!("line {}: {error}", number + 1))?;
                manifest.arguments = arguments;
                seen.insert("arguments");
                continue;
            }
            in_arguments = true;
            argument_lines.push(array_literal);
            if rest.contains(']') {
                let joined = argument_lines.join("\n");
                let arguments = parse_fah_manifest_arguments(&joined)
                    .map_err(|error| format!("line {}: {error}", number + 1))?;
                manifest.arguments = arguments;
                seen.insert("arguments");
                in_arguments = false;
                argument_lines.clear();
            }
            continue;
        }

        let Some((key_part, value_part)) = line.split_once('=') else {
            return Err(format!("line {}: expected key = value", number + 1));
        };
        let key = key_part.trim();
        if !allowed_keys.contains(key) {
            return Err(format!("line {}: unknown key {key:?}", number + 1));
        }
        if seen.contains(key) {
            return Err(format!("line {}: duplicate key {key:?}", number + 1));
        }
        seen.insert(key);
        let value = value_part.trim();

        match key {
            "schema_version" => {
                let parsed = value
                    .parse::<i32>()
                    .map_err(|_| format!("line {}: schema_version must be an integer", number + 1))?;
                manifest.schema_version = parsed;
            }
            "artifact_size" => {
                let parsed = value
                    .parse::<i64>()
                    .map_err(|_| {
                        format!(
                            "line {}: artifact_size must be a positive integer",
                            number + 1
                        )
                    })?;
                if parsed <= 0 {
                    return Err(format!(
                        "line {}: artifact_size must be a positive integer",
                        number + 1
                    ));
                }
                manifest.artifact_size = parsed;
            }
            "client_version" => {
                manifest.client_version = parse_quoted_string(value).map_err(|_| {
                    format!("line {}: client_version must be a quoted string", number + 1)
                })?;
            }
            "architecture" => {
                manifest.architecture = parse_quoted_string(value).map_err(|_| {
                    format!("line {}: architecture must be a quoted string", number + 1)
                })?;
            }
            "artifact_url" => {
                manifest.artifact_url = parse_quoted_string(value).map_err(|_| {
                    format!("line {}: artifact_url must be a quoted string", number + 1)
                })?;
            }
            "sha256" => {
                manifest.sha256 = parse_quoted_string(value).map_err(|_| {
                    format!("line {}: sha256 must be a quoted string", number + 1)
                })?;
            }
            "artifact_format" => {
                manifest.artifact_format = parse_quoted_string(value).map_err(|_| {
                    format!("line {}: artifact_format must be a quoted string", number + 1)
                })?;
            }
            "minimum_foldingos_version" => {
                manifest.minimum_foldingos_version = parse_quoted_string(value).map_err(|_| {
                    format!(
                        "line {}: minimum_foldingos_version must be a quoted string",
                        number + 1
                    )
                })?;
            }
            "terms_url" => {
                manifest.terms_url = parse_quoted_string(value).map_err(|_| {
                    format!("line {}: terms_url must be a quoted string", number + 1)
                })?;
            }
            "executable_path" => {
                manifest.executable_path = parse_quoted_string(value).map_err(|_| {
                    format!("line {}: executable_path must be a quoted string", number + 1)
                })?;
            }
            _ => return Err(format!("line {}: unknown key {key:?}", number + 1)),
        }
    }

    if in_arguments {
        return Err("manifest arguments array is not closed".into());
    }
    for key in allowed_keys {
        if !seen.contains(key) {
            return Err(format!("missing required key {key:?}"));
        }
    }
    Ok(manifest)
}

fn parse_fah_manifest_arguments(array_literal: &str) -> Result<Vec<String>, String> {
    let array_literal = array_literal.trim();
    if !array_literal.starts_with('[') || !array_literal.ends_with(']') {
        return Err("arguments must be a TOML array".into());
    }
    let inner = array_literal[1..array_literal.len() - 1].trim();
    if inner.is_empty() {
        return Err("arguments must be a non-empty array".into());
    }

    let mut arguments = Vec::new();
    for segment in split_comma_respecting_quotes(inner) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let argument = parse_quoted_string(segment)
            .map_err(|_| "arguments must contain only quoted strings".to_string())?;
        arguments.push(argument);
    }
    if arguments.is_empty() {
        return Err("arguments must be a non-empty array".into());
    }
    Ok(arguments)
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

pub fn validate_fah_manifest(manifest: &FahManifest) -> Result<(), String> {
    if manifest.schema_version != FAH_MANIFEST_SCHEMA_VERSION {
        return Err("manifest schema_version must be 1".into());
    }
    if manifest.architecture != FAH_MANIFEST_ARCHITECTURE {
        return Err("manifest architecture must be x86_64".into());
    }
    if manifest.artifact_format != FAH_MANIFEST_ARTIFACT_FORMAT {
        return Err("manifest artifact_format must be deb".into());
    }
    if manifest.minimum_foldingos_version != FAH_MANIFEST_MINIMUM_VERSION {
        return Err("manifest minimum_foldingos_version must be 0.1.0".into());
    }
    if !FAH_CLIENT_VERSION_PATTERN.is_match(&manifest.client_version) {
        return Err("manifest client_version must be a Folding@home 8.5 release".into());
    }
    if !FAH_SHA256_PATTERN.is_match(&manifest.sha256) {
        return Err("manifest sha256 must be a 64-character lowercase hex digest".into());
    }

    let artifact_url = Url::parse(&manifest.artifact_url).map_err(|_| {
        format!(
            "manifest artifact_url must use HTTPS from the approved official origin: {FAH_APPROVED_ARTIFACT_ORIGIN}"
        )
    })?;
    if artifact_url.scheme() != "https" || artifact_url.host_str() != Some(FAH_APPROVED_ARTIFACT_ORIGIN)
    {
        return Err(format!(
            "manifest artifact_url must use HTTPS from the approved official origin: {FAH_APPROVED_ARTIFACT_ORIGIN}"
        ));
    }
    let path = artifact_url.path();
    if path.ends_with("/latest.deb") || path.ends_with("latest.deb") {
        return Err("manifest artifact_url must not reference an unpinned latest artifact".into());
    }

    let terms_url = Url::parse(&manifest.terms_url)
        .map_err(|_| "manifest terms_url must use HTTPS on foldingathome.org".to_string())?;
    if terms_url.scheme() != "https" {
        return Err("manifest terms_url must use HTTPS on foldingathome.org".into());
    }
    let host = terms_url.host_str().unwrap_or_default();
    if !host.ends_with("foldingathome.org") {
        return Err("manifest terms_url must use HTTPS on foldingathome.org".into());
    }

    validate_fah_executable_path(&manifest.executable_path)?;
    for argument in &manifest.arguments {
        if argument.trim().is_empty() {
            return Err("manifest arguments must contain only non-empty strings".into());
        }
    }
    Ok(())
}

pub fn validate_fah_executable_path(path: &str) -> Result<(), String> {
    use super::util::FAH_EXECUTABLE_PATH_PREFIX;
    if !path.starts_with(FAH_EXECUTABLE_PATH_PREFIX) {
        return Err("manifest executable_path must remain under /data/apps/fah/current".into());
    }
    let cleaned = std::path::Path::new(path)
        .components()
        .fold(String::new(), |mut acc, component| {
            use std::path::Component;
            match component {
                Component::RootDir => acc.push('/'),
                Component::Normal(part) => {
                    if !acc.is_empty() && !acc.ends_with('/') {
                        acc.push('/');
                    }
                    acc.push_str(&part.to_string_lossy());
                }
                Component::ParentDir => {
                    if let Some(pos) = acc.rfind('/') {
                        acc.truncate(pos);
                    } else {
                        acc.clear();
                    }
                }
                Component::CurDir => {}
                Component::Prefix(_) => {}
            }
            acc
        });
    if cleaned != path || path.contains("..") {
        return Err("manifest executable_path must not contain path traversal".into());
    }
    if !cleaned.starts_with(FAH_EXECUTABLE_PATH_PREFIX) {
        return Err("manifest executable_path must remain under /data/apps/fah/current".into());
    }
    Ok(())
}

pub fn validate_foldingos_compatibility(minimum_version: &str) -> Result<(), String> {
    let current_version = os_release_value("VERSION_ID")?;
    if current_version != minimum_version {
        return Err(format!(
            "manifest requires FoldingOS {minimum_version} but image reports {current_version}"
        ));
    }
    Ok(())
}

pub fn validate_fah_version_label(version: &str) -> Result<(), String> {
    if !FAH_CLIENT_VERSION_PATTERN.is_match(version) {
        return Err("version must be a Folding@home 8.5 release".into());
    }
    if version.contains("..") || version.contains('/') || version.contains('\\') {
        return Err("version must not contain path separators or traversal".into());
    }
    let cleaned = std::path::Path::new(version);
    if cleaned.components().count() != 1 {
        return Err("version must not contain path separators or traversal".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_FAH_MANIFEST: &str = r#"schema_version = 1
client_version = "8.5.6"
architecture = "x86_64"
artifact_url = "https://download.foldingathome.org/releases/beta/fah-client/debian-10-64bit/release/fah-client_8.5.6_amd64.deb"
artifact_size = 3205180
sha256 = "643de04033a1cb972a81e3a193d710e919a4f34634a987f11adc4cee61fdaefe"
artifact_format = "deb"
minimum_foldingos_version = "0.1.0"
terms_url = "https://foldingathome.org/faq/opensource/"
executable_path = "/data/apps/fah/current/usr/bin/fah-client"
arguments = [
  "--config=/run/foldingos/fah/config.xml",
  "--log=/data/fah/log.txt",
  "--log-rotate-dir=/data/fah/log/",
]
"#;

    #[test]
    fn parse_approved_fah_manifest() {
        let manifest = parse_fah_manifest(VALID_FAH_MANIFEST).expect("parse manifest");
        validate_fah_manifest(&manifest).expect("validate manifest");
        assert_eq!(manifest.client_version, "8.5.6");
        assert_eq!(manifest.arguments.len(), 3);
    }

    #[test]
    fn reject_unknown_fah_manifest_key() {
        let content = VALID_FAH_MANIFEST.replace(
            r#"artifact_format = "deb""#,
            r#"artifact_format = "deb"
latest = true"#,
        );
        assert!(parse_fah_manifest(&content).is_err());
    }

    #[test]
    fn reject_unpinned_latest_artifact_url() {
        let content = VALID_FAH_MANIFEST.replace(
            r#"fah-client_8.5.6_amd64.deb""#,
            r#"latest.deb""#,
        );
        let manifest = parse_fah_manifest(&content).expect("parse manifest");
        assert!(validate_fah_manifest(&manifest).is_err());
    }

    #[test]
    fn reject_invalid_fah_origin() {
        let content = VALID_FAH_MANIFEST.replace(
            "https://download.foldingathome.org/",
            "https://evil.example/",
        );
        let manifest = parse_fah_manifest(&content).expect("parse manifest");
        assert!(validate_fah_manifest(&manifest).is_err());
    }

    #[test]
    fn reject_executable_path_outside_current() {
        let content = VALID_FAH_MANIFEST.replace(
            r#"executable_path = "/data/apps/fah/current/usr/bin/fah-client""#,
            r#"executable_path = "/usr/bin/fah-client""#,
        );
        let manifest = parse_fah_manifest(&content).expect("parse manifest");
        assert!(validate_fah_manifest(&manifest).is_err());
    }

    #[test]
    fn reject_external_fah_manifest_path() {
        let paths = AppliancePaths::default();
        assert!(load_fah_manifest(&paths, std::path::Path::new("/tmp/fah.toml")).is_err());
    }

    #[test]
    fn reject_uppercase_sha256() {
        let content = VALID_FAH_MANIFEST.replace(
            "643de04033a1cb972a81e3a193d710e919a4f34634a987f11adc4cee61fdaefe",
            "643DE04033A1CB972A81E3A193D710E919A4F34634A987F11ADC4CEE61FDAEFE",
        );
        let manifest = parse_fah_manifest(&content).expect("parse manifest");
        assert!(validate_fah_manifest(&manifest).is_err());
    }

    #[test]
    fn reject_invalid_fah_architecture() {
        let content =
            VALID_FAH_MANIFEST.replace(r#"architecture = "x86_64""#, r#"architecture = "aarch64""#);
        let manifest = parse_fah_manifest(&content).expect("parse manifest");
        assert!(validate_fah_manifest(&manifest).is_err());
    }

    #[test]
    fn reject_http_artifact_url() {
        let content = VALID_FAH_MANIFEST.replace(
            r#"artifact_url = "https://download.foldingathome.org/"#,
            r#"artifact_url = "http://download.foldingathome.org/"#,
        );
        let manifest = parse_fah_manifest(&content).expect("parse manifest");
        assert!(validate_fah_manifest(&manifest).is_err());
    }

    #[test]
    fn reject_incompatible_foldingos_version() {
        let content = VALID_FAH_MANIFEST.replace(
            r#"minimum_foldingos_version = "0.1.0""#,
            r#"minimum_foldingos_version = "9.9.9""#,
        );
        let manifest = parse_fah_manifest(&content).expect("parse manifest");
        assert!(validate_fah_manifest(&manifest).is_err());
    }
}
