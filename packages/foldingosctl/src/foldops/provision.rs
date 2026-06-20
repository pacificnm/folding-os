use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::boot_cmd::refresh_commissioning_display;
use crate::foldops::supervisor_permissions::{
    ensure_agent_software_assignment_permissions, ensure_foldops_config_group_readable,
    ensure_supervisor_database_writable, ensure_supervisor_fleet_automation_permissions,
};
use crate::foldops::tls::ensure_foldops_tls_material;
use crate::foldops::util::{
    foldops_web_root, FOLDOPS_AGENT_ENV, FOLDOPS_AGENT_SERVICE, FOLDOPS_HTTPS_PORT,
    FOLDOPS_PROVISION_SERVICE, FOLDOPS_SERVE_HTTPS_SERVICE, FOLDOPS_SUPERVISOR_ENV,
    FOLDOPS_SUPERVISOR_LOOPBACK_PORT, FOLDOPS_SUPERVISOR_SERVICE, PROVISIONED_FOLDOPS_INGEST_TOKEN,
};
use crate::foldops::verify::foldops_installation_verified;
use crate::foldops_manifest::foldops_packages_for_role;
use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;
use crate::process::{command_output, run_command};

static INGEST_TOKEN_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").expect("ingest token pattern compiles"));

const PROVISIONED_SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct FoldOpsProvisionedMarker {
    schema_version: i32,
    role: String,
    manifest_release: String,
    provisioned_at_unix: i64,
}

pub fn foldops_provision(paths: &AppliancePaths) -> Result<(), String> {
    let role = crate::role::read_active_installation_role(paths)?;
    if let Some(provisioned) = load_foldops_provisioned_marker(paths)? {
        if provisioned.role == "supervisor" {
            ensure_supervisor_fleet_automation_permissions(paths)?;
            refresh_supervisor_colocated_env(paths)?;
        }
        println!(
            "FoldOps is already provisioned for role {}.",
            provisioned.role
        );
        return restart_foldops_runtime_services(paths);
    }

    let manifest = crate::foldops::util::resolve_effective_foldops_manifest(paths)?;
    let packages = foldops_packages_for_role(&manifest, &role)?;
    if !has_verified_active_release(paths, &manifest.manifest_release, &role, &packages)? {
        return Err("FoldOps packages must be acquired before provision".into());
    }

    let (token, imported_from_efi) = import_foldops_ingest_token(paths)?;
    match role.as_str() {
        "supervisor" => provision_foldops_supervisor(paths, &manifest.manifest_release, &token)?,
        "agent" => provision_foldops_agent(paths, &manifest.manifest_release, &token)?,
        other => return Err(format!("unsupported installation role \"{other}\"")),
    }

    if imported_from_efi {
        match fs::remove_file(PROVISIONED_FOLDOPS_INGEST_TOKEN) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("remove EFI ingest token staging file: {error}"));
            }
        }
    }

    println!("FoldOps provision completed for role {role}.");
    start_foldops_runtime_services(paths)
}

fn has_verified_active_release(
    paths: &AppliancePaths,
    release: &str,
    role: &str,
    packages: &[crate::foldops_manifest::FoldOpsPackage],
) -> Result<bool, String> {
    let current_release = match crate::foldops::activate::read_foldops_current_release(paths) {
        Ok(release) => release,
        Err(_) => return Ok(false),
    };
    if current_release != release {
        return Ok(false);
    }
    foldops_installation_verified(paths, release, role, packages)
}

pub fn start_foldops_provision_service() -> Result<(), String> {
    start_systemd_unit_if_loaded(FOLDOPS_PROVISION_SERVICE, true)
}

pub fn restart_foldops_runtime_services(paths: &AppliancePaths) -> Result<(), String> {
    refresh_supervisor_colocated_env(paths)?;
    if unit_is_loaded(FOLDOPS_AGENT_SERVICE) {
        crate::process::schedule_deferred_systemd_restart_after(FOLDOPS_AGENT_SERVICE, 1)?;
    }
    if unit_is_loaded(FOLDOPS_SUPERVISOR_SERVICE) {
        crate::process::schedule_deferred_systemd_restart_after(FOLDOPS_SUPERVISOR_SERVICE, 2)?;
    }
    if unit_is_loaded(FOLDOPS_SERVE_HTTPS_SERVICE) {
        crate::process::schedule_deferred_systemd_restart_after(FOLDOPS_SERVE_HTTPS_SERVICE, 3)?;
    }
    refresh_commissioning_display(paths);
    Ok(())
}

fn unit_is_loaded(unit: &str) -> bool {
    command_output("systemctl", &["show", "-p", "LoadState", "--value", unit])
        .map(|value| value.trim() == "loaded")
        .unwrap_or(false)
}

pub fn start_foldops_runtime_services(paths: &AppliancePaths) -> Result<(), String> {
    for unit in [
        FOLDOPS_SUPERVISOR_SERVICE,
        FOLDOPS_SERVE_HTTPS_SERVICE,
        FOLDOPS_AGENT_SERVICE,
    ] {
        start_systemd_unit_if_loaded(unit, true)?;
    }
    refresh_commissioning_display(paths);
    Ok(())
}

fn start_systemd_unit_if_loaded(unit: &str, no_block: bool) -> Result<(), String> {
    let state = command_output("systemctl", &["show", "-p", "LoadState", "--value", unit])?;
    if state.trim() != "loaded" {
        return Ok(());
    }
    let mut args = vec!["start"];
    if no_block {
        args.push("--no-block");
    }
    args.push(unit);
    run_command("systemctl", &args).map_err(|error| format!("start {unit}: {error}"))
}

fn import_foldops_ingest_token(paths: &AppliancePaths) -> Result<(String, bool), String> {
    if let Ok(content) = fs::read_to_string(&paths.foldops_ingest_token) {
        let token = parse_foldops_ingest_token(&content)
            .map_err(|error| format!("persistent ingest token is invalid: {error}"))?;
        return Ok((token, false));
    }

    let content = fs::read_to_string(PROVISIONED_FOLDOPS_INGEST_TOKEN).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "foldops ingest token is not staged on EFI".to_string()
        } else {
            format!("read EFI ingest token: {error}")
        }
    })?;
    let token = parse_foldops_ingest_token(&content)
        .map_err(|error| format!("EFI ingest token is invalid: {error}"))?;
    let mut payload = token.clone();
    payload.push('\n');
    atomic_write(&paths.foldops_ingest_token, payload.as_bytes(), 0o640)?;
    Ok((token, true))
}

fn parse_foldops_ingest_token(content: &str) -> Result<String, String> {
    let token = content.trim();
    if token.is_empty() {
        return Err("ingest token is empty".into());
    }
    if token.contains('\n') {
        return Err("ingest token must be a single line".into());
    }
    if !INGEST_TOKEN_PATTERN.is_match(token) {
        return Err("ingest token must be 64 lowercase hex characters".into());
    }
    Ok(token.to_string())
}

fn provision_foldops_supervisor(
    paths: &AppliancePaths,
    manifest_release: &str,
    token: &str,
) -> Result<(), String> {
    ensure_supervisor_fleet_automation_permissions(paths)?;
    ensure_foldops_tls_material(paths)?;
    let ca_bytes = fs::read(paths.foldops_tls_dir.join("ca.pem"))
        .map_err(|error| format!("read TLS CA material: {error}"))?;
    atomic_write(&paths.foldops_supervisor_ca_pem(), &ca_bytes, 0o644)?;

    let supervisor_env = BTreeMap::from([
        ("HOST".to_string(), "127.0.0.1".to_string()),
        (
            "PORT".to_string(),
            FOLDOPS_SUPERVISOR_LOOPBACK_PORT.to_string(),
        ),
        ("INGEST_TOKEN".to_string(), token.to_string()),
        (
            "DB_PATH".to_string(),
            "/data/foldops/foldops.db".to_string(),
        ),
        (
            "WEB_ROOT".to_string(),
            foldops_web_root(paths).display().to_string(),
        ),
        ("CONFIG_ENABLED".to_string(), "true".to_string()),
        ("CONTROL_ENABLED".to_string(), "true".to_string()),
    ]);
    write_foldops_env_file(Path::new(FOLDOPS_SUPERVISOR_ENV), &supervisor_env, 0o640)?;

    write_supervisor_colocated_agent_env(paths, token)?;
    ensure_foldops_config_group_readable(paths)?;
    write_foldops_provisioned_marker(paths, "supervisor", manifest_release)
}

fn write_supervisor_colocated_agent_env(
    _paths: &AppliancePaths,
    token: &str,
) -> Result<(), String> {
    let agent_env = BTreeMap::from([
        (
            "SUPERVISOR_URL".to_string(),
            format!("http://127.0.0.1:{FOLDOPS_SUPERVISOR_LOOPBACK_PORT}"),
        ),
        ("AGENT_TOKEN".to_string(), token.to_string()),
        ("FAH_LOG_PATH".to_string(), "/data/fah/log.txt".to_string()),
        ("FAH_DB_PATH".to_string(), "/data/fah/client.db".to_string()),
        ("FAH_WORK_DIR".to_string(), "/data/fah/work".to_string()),
        ("CONFIG_ENABLED".to_string(), "true".to_string()),
        ("CONTROLS_ENABLED".to_string(), "true".to_string()),
    ]);
    write_foldops_env_file(Path::new(FOLDOPS_AGENT_ENV), &agent_env, 0o640)
}

pub fn refresh_supervisor_colocated_env(paths: &AppliancePaths) -> Result<(), String> {
    let role = crate::role::read_active_installation_role(paths)?;
    if role != "supervisor" || !foldops_provisioned(paths) {
        return Ok(());
    }

    let token = fs::read_to_string(&paths.foldops_ingest_token)
        .map_err(|error| format!("read ingest token: {error}"))?;
    let token = parse_foldops_ingest_token(&token)?;

    let supervisor_env = BTreeMap::from([
        ("HOST".to_string(), "127.0.0.1".to_string()),
        (
            "PORT".to_string(),
            FOLDOPS_SUPERVISOR_LOOPBACK_PORT.to_string(),
        ),
        ("INGEST_TOKEN".to_string(), token.clone()),
        (
            "DB_PATH".to_string(),
            paths.foldops_db.display().to_string(),
        ),
        (
            "WEB_ROOT".to_string(),
            foldops_web_root(paths).display().to_string(),
        ),
        ("CONFIG_ENABLED".to_string(), "true".to_string()),
        ("CONTROL_ENABLED".to_string(), "true".to_string()),
    ]);
    write_foldops_env_file(Path::new(FOLDOPS_SUPERVISOR_ENV), &supervisor_env, 0o640)?;
    write_supervisor_colocated_agent_env(paths, &token)?;
    ensure_supervisor_database_writable(paths)?;
    ensure_foldops_config_group_readable(paths)?;
    Ok(())
}

fn provision_foldops_agent(
    paths: &AppliancePaths,
    manifest_release: &str,
    token: &str,
) -> Result<(), String> {
    let supervisor_url_bytes = fs::read_to_string(&paths.supervisor_url)
        .map_err(|error| format!("read supervisor URL: {error}"))?;
    let host = crate::foldops::util::foldops_supervisor_host_from_url(&supervisor_url_bytes)?;
    if !paths.foldops_supervisor_ca_pem().exists() {
        return Err(format!(
            "supervisor CA trust anchor is missing: {}",
            paths.foldops_supervisor_ca_pem().display()
        ));
    }

    let agent_env = BTreeMap::from([
        (
            "SUPERVISOR_URL".to_string(),
            format!("https://{host}:{FOLDOPS_HTTPS_PORT}"),
        ),
        (
            "SUPERVISOR_TLS_CA".to_string(),
            paths.foldops_supervisor_ca_pem().display().to_string(),
        ),
        ("AGENT_TOKEN".to_string(), token.to_string()),
        ("FAH_LOG_PATH".to_string(), "/data/fah/log.txt".to_string()),
        ("FAH_DB_PATH".to_string(), "/data/fah/client.db".to_string()),
        ("FAH_WORK_DIR".to_string(), "/data/fah/work".to_string()),
        ("CONFIG_ENABLED".to_string(), "true".to_string()),
        ("CONTROLS_ENABLED".to_string(), "true".to_string()),
    ]);
    write_foldops_env_file(Path::new(FOLDOPS_AGENT_ENV), &agent_env, 0o640)?;
    ensure_foldops_config_group_readable(paths)?;
    ensure_agent_software_assignment_permissions(paths)?;
    write_foldops_provisioned_marker(paths, "agent", manifest_release)
}

fn write_foldops_env_file(
    path: &Path,
    values: &BTreeMap<String, String>,
    mode: u32,
) -> Result<(), String> {
    let mut lines = Vec::new();
    for (key, value) in values {
        if value.contains('\n') || value.contains('\r') {
            return Err(format!("env value for {key} must not contain newlines"));
        }
        lines.push(format!("{key}={value}"));
    }
    let mut content = lines.join("\n");
    content.push('\n');
    atomic_write(path, content.as_bytes(), mode)
}

fn write_foldops_provisioned_marker(
    paths: &AppliancePaths,
    role: &str,
    manifest_release: &str,
) -> Result<(), String> {
    let marker = FoldOpsProvisionedMarker {
        schema_version: PROVISIONED_SCHEMA_VERSION,
        role: role.to_string(),
        manifest_release: manifest_release.to_string(),
        provisioned_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs() as i64,
    };
    let content = serde_json::to_string(&marker)
        .map_err(|error| format!("encode provisioned marker: {error}"))?;
    let mut payload = content;
    payload.push('\n');
    atomic_write(&paths.foldops_provisioned_marker, payload.as_bytes(), 0o644)
}

fn load_foldops_provisioned_marker(
    paths: &AppliancePaths,
) -> Result<Option<FoldOpsProvisionedMarker>, String> {
    let content = match fs::read_to_string(&paths.foldops_provisioned_marker) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read provisioned marker: {error}")),
    };
    let marker: FoldOpsProvisionedMarker = serde_json::from_str(content.trim())
        .map_err(|error| format!("parse provisioned marker: {error}"))?;
    if marker.schema_version != PROVISIONED_SCHEMA_VERSION {
        return Err("provisioned marker schema_version is unsupported".into());
    }
    if marker.role != "agent" && marker.role != "supervisor" {
        return Err("provisioned marker role is invalid".into());
    }
    if marker.manifest_release.trim().is_empty() {
        return Err("provisioned marker manifest_release is empty".into());
    }
    Ok(Some(marker))
}

pub fn foldops_provisioned(paths: &AppliancePaths) -> bool {
    load_foldops_provisioned_marker(paths)
        .ok()
        .flatten()
        .is_some()
}
