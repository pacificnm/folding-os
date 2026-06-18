use std::fs;

use serde::{Deserialize, Serialize};

use crate::fs_atomic::atomic_write;
use crate::identity::read_installed_foldingos_version;
use crate::paths::AppliancePaths;
use crate::provision::ssh::validate_authorized_keys;
use crate::provision::util::{
    new_session_id, rfc3339_now, validate_enrollment_token, AGENT_INSTALLATION_ROLE,
};
use crate::registry_image::{
    load_registry_entry, load_registry_index, verify_registry_image_file, RegistryEntry,
};

#[derive(Debug, Deserialize)]
pub struct ProvisionAuthorizeRequest {
    pub schema_version: i32,
    pub enrollment_token: String,
    pub mac_addresses: Vec<String>,
    pub target_disk: String,
    pub target_serial: String,
    #[serde(default)]
    pub image_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProvisionAuthorizeResponse {
    pub schema_version: i32,
    pub install_session_id: String,
    pub image_version: String,
    pub image_size_bytes: i64,
    pub image_sha256: String,
    pub image_stream_path: String,
    pub installation_role: String,
    pub authorized_keys: String,
    pub foldops_ingest_token: String,
    pub foldops_supervisor_ca_pem: String,
    pub reboot_required: bool,
    pub target_disk: String,
    pub target_serial: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSession {
    pub schema_version: i32,
    pub session_id: String,
    pub created_at: String,
    pub mac_addresses: Vec<String>,
    pub target_disk: String,
    pub target_serial: String,
    pub image_version: String,
    pub image_sha256: String,
    pub image_size_bytes: i64,
    pub authorized_keys: String,
    pub completed: bool,
}

pub fn authorize_provision_install(
    paths: &AppliancePaths,
    request: ProvisionAuthorizeRequest,
) -> Result<ProvisionAuthorizeResponse, String> {
    if request.schema_version != 1 {
        return Err(format!(
            "unsupported authorize schema version {}",
            request.schema_version
        ));
    }
    validate_enrollment_token(paths, request.enrollment_token.trim())?;
    let mac_addresses = normalize_mac_addresses(&request.mac_addresses);
    if mac_addresses.is_empty() {
        return Err("at least one MAC address is required".into());
    }
    let target_disk = request.target_disk.trim();
    if target_disk.is_empty() {
        return Err("target_disk is required".into());
    }
    if !target_disk.starts_with("/dev/") {
        return Err(format!("target_disk must be a block device path: {target_disk:?}"));
    }
    let target_serial = request.target_serial.trim();
    if target_serial.is_empty() {
        return Err("target_serial is required".into());
    }

    let entry = resolve_provision_image_version(paths, request.image_version.trim())?;
    if entry.rollout_state != "ready" {
        return Err(format!(
            "image version {:?} is not ready for provisioning",
            entry.foldingos_version
        ));
    }
    verify_registry_image_file(
        std::path::Path::new(&entry.local_image_path),
        &entry.image_sha256,
        entry.image_size_bytes,
    )
    .map_err(|error| {
        format!(
            "registry image for {} is invalid: {error}",
            entry.foldingos_version
        )
    })?;

    let authorized_keys = read_supervisor_authorized_keys(paths)?;
    let (foldops_ingest_token, foldops_supervisor_ca_pem) =
        load_supervisor_foldops_install_materials(paths)?;

    let session_id = new_session_id()?;
    let session = InstallSession {
        schema_version: 1,
        session_id: session_id.clone(),
        created_at: rfc3339_now(),
        mac_addresses,
        target_disk: target_disk.to_string(),
        target_serial: target_serial.to_string(),
        image_version: entry.foldingos_version.clone(),
        image_sha256: entry.image_sha256.clone(),
        image_size_bytes: entry.image_size_bytes,
        authorized_keys: authorized_keys.clone(),
        completed: false,
    };
    save_install_session(paths, &session)?;

    Ok(ProvisionAuthorizeResponse {
        schema_version: 1,
        install_session_id: session_id,
        image_version: entry.foldingos_version.clone(),
        image_size_bytes: entry.image_size_bytes,
        image_sha256: entry.image_sha256,
        image_stream_path: format!("/v1/provision/images/{}/stream", entry.foldingos_version),
        installation_role: AGENT_INSTALLATION_ROLE.into(),
        authorized_keys,
        foldops_ingest_token,
        foldops_supervisor_ca_pem: String::from_utf8_lossy(&foldops_supervisor_ca_pem).into_owned(),
        reboot_required: true,
        target_disk: target_disk.to_string(),
        target_serial: target_serial.to_string(),
    })
}

pub fn validate_install_stream_access(
    paths: &AppliancePaths,
    session_id: &str,
    version: &str,
    enrollment_token: &str,
) -> Result<(InstallSession, RegistryEntry), String> {
    validate_enrollment_token(paths, enrollment_token.trim())?;
    let session = load_install_session(paths, session_id).map_err(|error| {
        if error.contains("No such file") || error.contains("not found") {
            "install session is invalid".to_string()
        } else {
            error
        }
    })?;
    if session.completed {
        return Err("install session is already completed".into());
    }
    let version = version.trim();
    if session.image_version != version {
        return Err(format!(
            "install session does not authorize image version {version:?}"
        ));
    }
    let entry = load_registry_entry(paths, version)?;
    verify_registry_image_file(
        std::path::Path::new(&entry.local_image_path),
        &entry.image_sha256,
        entry.image_size_bytes,
    )
    .map_err(|error| format!("registry image for {version} is invalid: {error}"))?;
    Ok((session, entry))
}

pub fn save_install_session(paths: &AppliancePaths, session: &InstallSession) -> Result<(), String> {
    let content = serde_json::to_string_pretty(session).map_err(|error| error.to_string())?;
    atomic_write(
        &paths.install_session_path(&session.session_id),
        format!("{content}\n").as_bytes(),
        0o600,
    )
}

pub fn load_install_session(paths: &AppliancePaths, session_id: &str) -> Result<InstallSession, String> {
    let content = fs::read_to_string(paths.install_session_path(session_id))
        .map_err(|error| error.to_string())?;
    let session: InstallSession =
        serde_json::from_str(&content).map_err(|error| format!("invalid install session: {error}"))?;
    if session.schema_version != 1 {
        return Err(format!(
            "unsupported install session schema version {}",
            session.schema_version
        ));
    }
    if session.session_id.trim().is_empty() {
        return Err("install session is missing session_id".into());
    }
    Ok(session)
}

fn resolve_provision_image_version(paths: &AppliancePaths, version: &str) -> Result<RegistryEntry, String> {
    if !version.is_empty() {
        return load_registry_entry(paths, version);
    }
    let index = load_registry_index(paths)?;
    if index.versions.is_empty() {
        return Err("supervisor registry has no release images".into());
    }
    if let Ok(installed) = read_installed_foldingos_version() {
        if let Ok(entry) = load_registry_entry(paths, &installed) {
            if entry.rollout_state == "ready" {
                return Ok(entry);
            }
        }
    }
    let mut latest_ready = None;
    for candidate in &index.versions {
        let entry = load_registry_entry(paths, candidate)?;
        if entry.rollout_state == "ready" {
            latest_ready = Some(entry);
        }
    }
    latest_ready.ok_or_else(|| "supervisor registry has no ready release images".into())
}

fn read_supervisor_authorized_keys(paths: &AppliancePaths) -> Result<String, String> {
    let content = match fs::read(&paths.active_ssh_keys) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("supervisor administrator authorized keys are not configured".into());
        }
        Err(error) => return Err(error.to_string()),
    };
    let keys = validate_authorized_keys(&content)
        .map_err(|error| format!("supervisor administrator authorized keys are invalid: {error}"))?;
    Ok(String::from_utf8_lossy(&keys).into_owned())
}

fn load_supervisor_foldops_install_materials(
    paths: &AppliancePaths,
) -> Result<(String, Vec<u8>), String> {
    let content = match fs::read_to_string(&paths.foldops_ingest_token) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("supervisor FoldOps provisioning must complete before network agent install".into());
        }
        Err(error) => return Err(format!("read supervisor ingest token: {error}")),
    };
    let token = parse_foldops_ingest_token(&content)?;
    let ca_bytes = match fs::read(paths.foldops_supervisor_ca_pem()) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("supervisor FoldOps TLS material is unavailable for network agent install".into());
        }
        Err(error) => return Err(format!("read supervisor TLS CA: {error}")),
    };
    if ca_bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err("supervisor TLS CA material is empty".into());
    }
    Ok((token, ca_bytes))
}

fn parse_foldops_ingest_token(content: &str) -> Result<String, String> {
    let token = content.trim();
    if token.is_empty() {
        return Err("supervisor ingest token is empty".into());
    }
    Ok(token.to_string())
}

fn normalize_mac_addresses(addresses: &[String]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut normalized = Vec::new();
    for address in addresses {
        let address = address.trim().to_ascii_lowercase();
        if address.is_empty() || !seen.insert(address.clone()) {
            continue;
        }
        normalized.push(address);
    }
    normalized.sort();
    normalized
}
