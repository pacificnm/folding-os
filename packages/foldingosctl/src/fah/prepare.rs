use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::time::Duration;

use crate::config::{load_effective_config_for_domain, validate_secret_reference, DomainConfig};
use crate::config_host::read_hostname;
use crate::paths::AppliancePaths;

use rusqlite::OptionalExtension;

use super::manifest::{load_fah_manifest, validate_foldingos_compatibility};
use super::passkey::is_valid_fah_passkey;
use super::util::{read_fah_current_version, require_fah_root_ownership, FAH_SERVICE_GID};
use super::verify_install::{fah_installation_verified, verify_fah_installed_version};

pub fn fah_prepare(paths: &AppliancePaths) -> Result<(), String> {
    fah_prepare_with_output(paths, true)
}

pub fn fah_prepare_quiet(paths: &AppliancePaths) -> Result<(), String> {
    fah_prepare_with_output(paths, false)
}

fn fah_prepare_with_output(paths: &AppliancePaths, emit_status: bool) -> Result<(), String> {
    let manifest = load_fah_manifest(paths, &paths.fah_embedded_manifest)?;
    validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;

    let active_version = read_fah_current_version(paths)
        .map_err(|error| format!("no active Folding@home installation: {error}"))?;
    if !fah_installation_verified(paths, &active_version, &manifest) {
        return Err("active Folding@home installation is not verified".into());
    }
    verify_fah_installed_version(paths, &active_version, &manifest)?;

    let (config, passkey) = load_fah_runtime_configuration(paths)?;
    let runtime = build_fah_runtime_config(paths, &config, &passkey);
    let content = render_fah_config_xml(&runtime);
    atomic_write_root_fah(paths, content.as_bytes())?;
    let reconciled_group = reconcile_fah_default_group(paths, runtime.cpus)?;
    if emit_status {
        println!(
            "Rendered Folding@home runtime configuration at {}.",
            paths.fah_runtime_config.display()
        );
        if reconciled_group {
            println!("Updated persisted Folding@home default resource group.");
        }
    }
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
    let content =
        fs::read_to_string(&path).map_err(|error| format!("read passkey secret: {error}"))?;
    let passkey = content.trim_end_matches('\n');
    if !is_valid_fah_passkey(passkey) {
        return Err("passkey secret has an invalid length or character set".into());
    }
    Ok(passkey.to_string())
}

struct FahRuntimeConfig {
    machine_name: String,
    username: String,
    team: i64,
    passkey: String,
    cpus: i64,
}

fn build_fah_runtime_config(
    paths: &AppliancePaths,
    config: &DomainConfig,
    passkey: &str,
) -> FahRuntimeConfig {
    let machine_name = read_hostname(paths).unwrap_or_else(|_| String::from("foldingos"));
    let team = config
        .get("identity.team")
        .map(|value| value.ival)
        .unwrap_or_default();
    let username = config
        .get("identity.username")
        .map(|value| value.text.as_str())
        .unwrap_or("Anonymous");
    let configured_cpus = config
        .get("resources.cpus")
        .map(|value| value.ival)
        .unwrap_or_default();
    let cpus = if configured_cpus > 0 {
        configured_cpus
    } else {
        automatic_fah_cpus()
    };

    FahRuntimeConfig {
        machine_name,
        username: username.to_string(),
        team,
        passkey: passkey.to_string(),
        cpus,
    }
}

fn render_fah_config_xml(runtime: &FahRuntimeConfig) -> String {
    let mut builder = String::new();
    builder.push_str("<config>\n");
    if !runtime.passkey.is_empty() {
        builder.push_str(&format!(
            "  <account-token v=\"{}\"/>\n",
            xml_escape_attribute(&runtime.passkey)
        ));
    }
    builder.push_str(&format!(
        "  <machine-name v=\"{}\"/>\n",
        xml_escape_attribute(&runtime.machine_name)
    ));
    if !runtime.username.is_empty() {
        builder.push_str(&format!(
            "  <user v=\"{}\"/>\n",
            xml_escape_attribute(&runtime.username)
        ));
    }
    if runtime.team != 0 {
        builder.push_str(&format!("  <team v=\"{}\"/>\n", runtime.team));
    }
    if runtime.cpus != 0 {
        builder.push_str(&format!("  <cpus v=\"{}\"/>\n", runtime.cpus));
    }
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

fn automatic_fah_cpus() -> i64 {
    let threads = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    normalize_auto_cpus(threads)
}

fn normalize_auto_cpus(threads: usize) -> i64 {
    let mut cpus = threads.saturating_sub(1).max(1);
    if cpus > 1 && cpus % 2 == 1 {
        cpus -= 1;
    }
    cpus.min(4) as i64
}

fn reconcile_fah_default_group(paths: &AppliancePaths, cpus: i64) -> Result<bool, String> {
    if cpus <= 0 {
        return Ok(false);
    }

    let db_path = paths
        .fah_log
        .parent()
        .map(|parent| parent.join("client.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("/data/fah/client.db"));
    if !db_path.exists() {
        return Ok(false);
    }

    let mut connection = rusqlite::Connection::open(&db_path)
        .map_err(|error| format!("open FAH client state {}: {error}", db_path.display()))?;
    connection
        .busy_timeout(Duration::from_secs(10))
        .map_err(|error| format!("set FAH client state busy timeout: {error}"))?;

    let table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'groups')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("inspect FAH client groups table: {error}"))?;
    if !table_exists {
        return Ok(false);
    }

    let transaction = connection
        .transaction()
        .map_err(|error| format!("begin FAH client state transaction: {error}"))?;
    let Some(raw_group) = transaction
        .query_row("SELECT value FROM groups WHERE name = ''", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(|error| format!("read FAH default resource group: {error}"))?
    else {
        transaction
            .commit()
            .map_err(|error| format!("commit FAH client state transaction: {error}"))?;
        return Ok(false);
    };

    let mut group: serde_json::Value = serde_json::from_str(&raw_group)
        .map_err(|error| format!("parse FAH default resource group: {error}"))?;
    let Some(group_object) = group.as_object_mut() else {
        return Err("FAH default resource group is not a JSON object".into());
    };
    if group_object.get("cpus").and_then(|value| value.as_i64()) == Some(cpus) {
        transaction
            .commit()
            .map_err(|error| format!("commit FAH client state transaction: {error}"))?;
        return Ok(false);
    }

    group_object.insert("cpus".into(), serde_json::json!(cpus));
    let updated_group = serde_json::to_string_pretty(&group)
        .map_err(|error| format!("serialize FAH default resource group: {error}"))?;
    transaction
        .execute(
            "UPDATE groups SET value = ?1 WHERE name = ''",
            [&updated_group],
        )
        .map_err(|error| format!("update FAH default resource group: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("commit FAH client state transaction: {error}"))?;
    Ok(true)
}

fn atomic_write_root_fah(paths: &AppliancePaths, content: &[u8]) -> Result<(), String> {
    let path = &paths.fah_runtime_config;
    let parent = paths.fah_runtime_dir();
    fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
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
    let prefix = format!(
        ".{}.tmp-",
        path.file_name().unwrap_or_default().to_string_lossy()
    );
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
            match OpenOptions::new().write(true).create_new(true).open(&path) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_domain;

    #[test]
    fn render_fah_config_xml_includes_configured_donor() {
        let config = parse_domain(
            "foldinghome",
            r#"
schema_version = 1

[identity]
username = "FoldingOS"
team = 1068254
passkey_secret = ""

[resources]
cpus = 0
gpus = false
"#,
            true,
        )
        .expect("parse config");

        let runtime = build_fah_runtime_config(&AppliancePaths::default(), &config, "");
        let xml = render_fah_config_xml(&runtime);

        assert!(xml.contains(r#"<user v="FoldingOS"/>"#));
        assert!(xml.contains(r#"<team v="1068254"/>"#));
    }

    #[test]
    fn normalize_auto_cpus_avoids_odd_slots() {
        assert_eq!(normalize_auto_cpus(1), 1);
        assert_eq!(normalize_auto_cpus(2), 1);
        assert_eq!(normalize_auto_cpus(4), 2);
        assert_eq!(normalize_auto_cpus(12), 4);
        assert_eq!(normalize_auto_cpus(16), 4);
    }

    #[test]
    fn reconcile_fah_default_group_updates_persisted_cpu_count() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut paths = AppliancePaths::default();
        paths.fah_log = dir.path().join("log.txt");
        let db_path = dir.path().join("client.db");
        let connection = rusqlite::Connection::open(&db_path).expect("open sqlite");
        connection
            .execute("CREATE TABLE groups (name TEXT PRIMARY KEY, value)", [])
            .expect("create groups");
        connection
            .execute(
                "INSERT INTO groups (name, value) VALUES ('', ?1)",
                [r#"{"cpus":11,"gpus":{},"paused":false}"#],
            )
            .expect("insert group");
        drop(connection);

        assert!(reconcile_fah_default_group(&paths, 4).expect("reconcile"));

        let connection = rusqlite::Connection::open(&db_path).expect("reopen sqlite");
        let value: String = connection
            .query_row("SELECT value FROM groups WHERE name = ''", [], |row| {
                row.get(0)
            })
            .expect("read group");
        let group: serde_json::Value = serde_json::from_str(&value).expect("parse group");
        assert_eq!(group.get("cpus").and_then(|value| value.as_i64()), Some(4));
        assert_eq!(
            group.get("paused").and_then(|value| value.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn reconcile_fah_default_group_ignores_missing_database() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut paths = AppliancePaths::default();
        paths.fah_log = dir.path().join("log.txt");

        assert!(!reconcile_fah_default_group(&paths, 4).expect("reconcile"));
    }
}
