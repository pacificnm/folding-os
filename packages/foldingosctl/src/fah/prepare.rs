use std::fs::{self, File, OpenOptions};
use std::io::Write;

use regex::Regex;
use std::sync::LazyLock;

use crate::config::{load_effective_config_for_domain, validate_secret_reference, DomainConfig};
use crate::paths::AppliancePaths;

use super::manifest::{load_fah_manifest, validate_foldingos_compatibility};
use super::util::{
    read_fah_current_version, require_fah_root_ownership, FAH_SERVICE_GID,
};
use super::verify_install::{fah_installation_verified, verify_fah_installed_version};

static FAH_PASSKEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-fA-F]{32}$").expect("passkey pattern compiles")
});

pub fn fah_prepare(paths: &AppliancePaths) -> Result<(), String> {
    let manifest = load_fah_manifest(paths, &paths.fah_embedded_manifest)?;
    validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;

    let active_version = read_fah_current_version(paths)
        .map_err(|error| format!("no active Folding@home installation: {error}"))?;
    if !fah_installation_verified(paths, &active_version, &manifest) {
        return Err("active Folding@home installation is not verified".into());
    }
    verify_fah_installed_version(paths, &active_version, &manifest)?;

    let (config, passkey) = load_fah_runtime_configuration(paths)?;
    let content = render_fah_config_xml(&config, &passkey);
    atomic_write_root_fah(paths, content.as_bytes())?;
    println!(
        "Rendered Folding@home runtime configuration at {}.",
        paths.fah_runtime_config.display()
    );
    Ok(())
}

fn load_fah_runtime_configuration(
    paths: &AppliancePaths,
) -> Result<(DomainConfig, String), String> {
    let merged = load_effective_config_for_domain(paths, "foldinghome")
        .map_err(|error| format!("invalid Folding@home configuration: {error}"))?;
    let secret_name = merged
        .get("identity.passkey_secret")
        .map(|value| value.text.as_str())
        .unwrap_or_default();
    let passkey = read_fah_passkey(paths, secret_name)?;
    Ok((merged, passkey))
}

fn read_fah_passkey(paths: &AppliancePaths, secret_name: &str) -> Result<String, String> {
    if secret_name.is_empty() {
        return Ok(String::new());
    }
    validate_secret_reference(paths, secret_name)?;
    let path = paths.secrets_dir().join(secret_name);
    let content = fs::read_to_string(&path).map_err(|error| format!("read passkey secret: {error}"))?;
    let passkey = content.trim_end_matches('\n');
    if !FAH_PASSKEY_PATTERN.is_match(passkey) {
        return Err("passkey secret must be exactly 32 hexadecimal characters".into());
    }
    Ok(passkey.to_string())
}

fn render_fah_config_xml(config: &DomainConfig, passkey: &str) -> String {
    let username = config
        .get("identity.username")
        .map(|value| value.text.as_str())
        .unwrap_or_default();
    let team = config
        .get("identity.team")
        .map(|value| value.ival)
        .unwrap_or_default();
    let cpus = config
        .get("resources.cpus")
        .map(|value| value.ival)
        .unwrap_or_default();

    let mut builder = String::new();
    builder.push_str("<config>\n");
    builder.push_str(&format!(
        "  <user v=\"{}\"/>\n",
        xml_escape_attribute(username)
    ));
    builder.push_str(&format!("  <team v=\"{team}\"/>\n"));
    if !passkey.is_empty() {
        builder.push_str(&format!(
            "  <passkey v=\"{}\"/>\n",
            xml_escape_attribute(passkey)
        ));
    }
    builder.push_str(&format!("  <cpus v=\"{cpus}\"/>\n"));
    builder.push_str("</config>\n");
    builder
}

fn xml_escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn atomic_write_root_fah(paths: &AppliancePaths, content: &[u8]) -> Result<(), String> {
    let path = &paths.fah_runtime_config;
    let parent = paths.fah_runtime_dir();
    fs::create_dir_all(&parent)
        .map_err(|error| error.to_string())?;
    if require_fah_root_ownership() {
        nix::unistd::chown(
            &parent,
            Some(nix::unistd::Uid::from_raw(0)),
            Some(nix::unistd::Gid::from_raw(FAH_SERVICE_GID)),
        )
        .map_err(|error| format!("set runtime directory ownership: {error}"))?;
    }

    let temp = tempfile_in(&parent, path)?;
    let temp_name = temp.path().to_path_buf();
    let mut cleanup = TempCleanup::new(temp_name.clone());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp_name, fs::Permissions::from_mode(0o640))
            .map_err(|error| error.to_string())?;
    }
    if require_fah_root_ownership() {
        nix::unistd::chown(
            &temp_name,
            Some(nix::unistd::Uid::from_raw(0)),
            Some(nix::unistd::Gid::from_raw(FAH_SERVICE_GID)),
        )
        .map_err(|error| format!("set runtime configuration ownership: {error}"))?;
    }
    {
        let mut file = temp;
        file.write_all(content).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    fs::rename(&temp_name, path).map_err(|error| error.to_string())?;
    cleanup.disarm();

    let dir = OpenOptions::new()
        .read(true)
        .open(&parent)
        .map_err(|error| error.to_string())?;
    dir.sync_all()
        .map_err(|error| format!("sync {}: {error}", parent.display()))?;
    Ok(())
}

fn tempfile_in(parent: &std::path::Path, path: &std::path::Path) -> Result<NamedTempFile, String> {
    let prefix = format!(".{}.tmp-", path.file_name().unwrap_or_default().to_string_lossy());
    NamedTempFile::create(parent, &prefix)
}

struct NamedTempFile {
    file: File,
    path: std::path::PathBuf,
}

impl NamedTempFile {
    fn create(parent: &std::path::Path, prefix: &str) -> Result<Self, String> {
        for attempt in 0..100 {
            let name = format!("{prefix}{}", std::process::id() + attempt);
            let path = parent.join(name);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => return Ok(Self { file, path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
        Err("create temp file failed".into())
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl std::ops::Deref for NamedTempFile {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

impl std::ops::DerefMut for NamedTempFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.file
    }
}

struct TempCleanup {
    path: std::path::PathBuf,
    active: bool,
}

impl TempCleanup {
    fn new(path: std::path::PathBuf) -> Self {
        Self { path, active: true }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for TempCleanup {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.path);
        }
    }
}
