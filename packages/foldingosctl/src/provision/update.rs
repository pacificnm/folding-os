use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::enrollment::{load_enrollment_record, save_enrollment_record};
use crate::fs_atomic::atomic_write;
use crate::identity::read_installed_foldingos_version;
use crate::paths::AppliancePaths;
use crate::provision::enroll::query_desired_version;
use crate::provision::grub_env::set_grub_env_var;
use crate::provision::release_image::{
    copy_staged_release_image_efi_partition, copy_staged_release_image_root_partition,
};
use crate::provision::staged_lock::with_staged_update_lock;
use crate::provision::targets::{clear_grub_next_entry_on_disk, resolve_host_boot_disk};
use crate::provision::util::{
    copy_regular_file, empty_human_result, format_install_bytes, http_post_json, install_logf,
    join_supervisor_url, new_session_id, partition_device, read_enrollment_token,
    read_supervisor_base_url, rfc3339_now, run_command, agent_enrollment_node_id, UPDATE_SESSION_HEADER,
};
use crate::registry_image::{load_registry_entry, verify_registry_image_file, RegistryEntry};
use crate::role::require_agent_role;

const APPLY_STATE_STAGED: &str = "staged";
const APPLY_STATE_BOOT_SCHEDULED: &str = "boot_scheduled";
const APPLY_STATE_APPLYING: &str = "applying";
const APPLY_STATE_FAILED: &str = "failed";
const UPDATE_GRUB_ENTRY_NAME: &str = "1";
const INSTALL_PROGRESS_INTERVAL: i64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedUpdateMetadata {
    pub schema_version: i32,
    pub node_id: String,
    pub current_version: String,
    pub desired_version: String,
    pub image_sha256: String,
    pub image_size_bytes: i64,
    pub boot_disk: String,
    pub staged_at: String,
    pub apply_state: String,
    #[serde(default)]
    pub boot_schedule_attempts: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingUpdateReport {
    schema_version: i32,
    node_id: String,
    image_version: String,
    status: String,
    #[serde(default)]
    message: String,
    recorded_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAuthorizeRequest {
    pub schema_version: i32,
    pub node_id: String,
    pub enrollment_token: String,
    pub current_image_version: String,
    pub desired_image_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAuthorizeResponse {
    pub schema_version: i32,
    pub update_session_id: String,
    pub image_version: String,
    pub image_size_bytes: i64,
    pub image_sha256: String,
    pub image_stream_path: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub schema_version: i32,
    pub node_id: String,
    pub enrollment_token: String,
    pub image_version: String,
    pub status: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSession {
    pub schema_version: i32,
    pub session_id: String,
    pub created_at: String,
    pub node_id: String,
    pub image_version: String,
    pub image_sha256: String,
    pub image_size_bytes: i64,
    pub completed: bool,
}

pub fn authorize_agent_update(
    paths: &AppliancePaths,
    request: UpdateAuthorizeRequest,
) -> Result<UpdateAuthorizeResponse, String> {
    if request.schema_version != 1 {
        return Err(format!(
            "unsupported update authorize schema version {}",
            request.schema_version
        ));
    }
    crate::provision::util::validate_enrollment_token(paths, request.enrollment_token.trim())?;
    let node_id = request.node_id.trim();
    if !crate::enrollment::is_valid_node_id(node_id) {
        return Err("node_id is invalid".into());
    }
    let current_version = request.current_image_version.trim();
    if current_version.is_empty() {
        return Err("current_image_version is required".into());
    }
    let desired_version = request.desired_image_version.trim();
    if desired_version.is_empty() || desired_version == "current" {
        return Err("desired_image_version is required".into());
    }
    if desired_version == current_version {
        return Err("desired image version matches the current image".into());
    }

    let record = load_enrollment_record(paths, node_id).map_err(|_| "agent is not registered".to_string())?;
    if record.desired_image_version != desired_version {
        return Err(format!(
            "desired image version {desired_version:?} is not assigned to node {node_id}"
        ));
    }
    let entry = load_registry_entry(paths, desired_version).map_err(|error| {
        format!("desired image version {desired_version:?} is not in registry: {error}")
    })?;
    if entry.rollout_state != "ready" {
        return Err(format!("image version {desired_version:?} is not ready for rollout"));
    }
    verify_registry_image_file(
        std::path::Path::new(&entry.local_image_path),
        &entry.image_sha256,
        entry.image_size_bytes,
    )
    .map_err(|error| format!("registry image for {} is invalid: {error}", entry.foldingos_version))?;

    let session_id = new_session_id()?;
    let session = UpdateSession {
        schema_version: 1,
        session_id: session_id.clone(),
        created_at: rfc3339_now(),
        node_id: node_id.to_string(),
        image_version: entry.foldingos_version.clone(),
        image_sha256: entry.image_sha256.clone(),
        image_size_bytes: entry.image_size_bytes,
        completed: false,
    };
    save_update_session(paths, &session)?;
    record_agent_update_status(paths, node_id, desired_version, "staging", "")?;

    Ok(UpdateAuthorizeResponse {
        schema_version: 1,
        update_session_id: session_id,
        image_version: entry.foldingos_version.clone(),
        image_size_bytes: entry.image_size_bytes,
        image_sha256: entry.image_sha256.clone(),
        image_stream_path: format!("/v1/provision/images/{}/stream", entry.foldingos_version),
    })
}

pub fn validate_update_stream_access(
    paths: &AppliancePaths,
    session_id: &str,
    version: &str,
    enrollment_token: &str,
) -> Result<(UpdateSession, RegistryEntry), String> {
    crate::provision::util::validate_enrollment_token(paths, enrollment_token.trim())?;
    let session = load_update_session(paths, session_id).map_err(|error| {
        if error.contains("No such file") || error.contains("not found") {
            "update session is invalid".to_string()
        } else {
            error
        }
    })?;
    if session.completed {
        return Err("update session is already completed".into());
    }
    let version = version.trim();
    if session.image_version != version {
        return Err(format!(
            "update session does not authorize image version {version:?}"
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

pub fn record_agent_update_status(
    paths: &AppliancePaths,
    node_id: &str,
    version: &str,
    status: &str,
    message: &str,
) -> Result<(), String> {
    let status = status.trim();
    if !matches!(status, "staging" | "staged" | "applying" | "applied" | "failed") {
        return Err(format!("unsupported update status {status:?}"));
    }
    let mut record = load_enrollment_record(paths, node_id)?;
    record.last_update_status = status.to_string();
    record.last_update_version = version.trim().to_string();
    record.last_update_message = message.trim().to_string();
    record.last_update_at = rfc3339_now();
    if status == "applied" {
        record.current_image_version = record.last_update_version.clone();
        record.foldingos_version = record.last_update_version.clone();
        if record.desired_image_version == record.last_update_version {
            record.desired_image_version = "current".into();
        }
    }
    save_enrollment_record(paths, record)
}

pub fn handle_update_status(paths: &AppliancePaths, request: UpdateStatusRequest) -> Result<(), String> {
    if request.schema_version != 1 {
        return Err("unsupported update status schema version".into());
    }
    crate::provision::util::validate_enrollment_token(paths, request.enrollment_token.trim())?;
    let node_id = request.node_id.trim();
    if !crate::enrollment::is_valid_node_id(node_id) {
        return Err("node_id is invalid".into());
    }
    record_agent_update_status(
        paths,
        node_id,
        &request.image_version,
        &request.status,
        &request.message,
    )
}

pub fn provision_check_version_and_stage(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    require_agent_role(paths)?;
    let _ = flush_pending_update_report(paths);

    let node_id = match agent_enrollment_node_id(paths) {
        Ok(node_id) => node_id,
        Err(error) if error.contains("not found") || !paths.agent_enrollment_state.exists() => {
            println!("current");
            return Ok(empty_human_result());
        }
        Err(error) => return Err(error),
    };

    let current_version = read_installed_foldingos_version()?;
    let supervisor_url = read_supervisor_base_url(paths)?;
    if supervisor_url.is_empty() {
        println!("current");
        return Ok(empty_human_result());
    }
    let token = match read_enrollment_token(paths) {
        Ok(token) => token,
        Err(_) => {
            println!("current");
            return Ok(empty_human_result());
        }
    };

    if let Err(error) =
        crate::assignments::sync_local_software_assignments_from_supervisor(paths, &supervisor_url, &node_id, &token)
    {
        install_logf(&format!("Software assignment sync failed: {error}"));
    }

    let desired = match query_desired_version(&supervisor_url, &node_id, &token) {
        Ok(version) => version,
        Err(_) => {
            println!("current");
            return Ok(empty_human_result());
        }
    };
    if desired == "current" || desired == current_version {
        clear_staged_update(paths)?;
        println!("current");
        return Ok(empty_human_result());
    }

    if let Ok(staged) = load_staged_update_metadata(paths) {
        if !staged.node_id.is_empty() && staged.node_id != node_id {
            clear_staged_update(paths)?;
        } else if is_locked_staged_update_apply_state(&staged.apply_state) {
            install_logf(&format!(
                "Update {desired} pending apply (apply_state={}); run provision apply-update.",
                effective_apply_state(&staged.apply_state)
            ));
            println!("{desired}");
            return Ok(empty_human_result());
        } else if staged.desired_version == desired && staged.current_version == current_version {
            if verify_staged_update_file(paths, &staged).is_ok() {
                install_logf(&format!(
                    "Update {desired} already staged and verified; run provision apply-update."
                ));
                println!("{desired}");
                return Ok(empty_human_result());
            }
            clear_staged_update(paths)?;
        } else {
            clear_staged_update(paths)?;
        }
    }

    if load_pending_update_report(paths).is_ok() {
        println!("{desired}");
        return Ok(empty_human_result());
    }

    install_logf(&format!(
        "Supervisor assigned image version {desired} (current {current_version}); staging update image."
    ));
    if let Err(error) = stage_agent_update(
        paths,
        &supervisor_url,
        &node_id,
        &token,
        &current_version,
        &desired,
    ) {
        let _ = report_agent_update_status(
            paths,
            &supervisor_url,
            &node_id,
            &token,
            &desired,
            "failed",
            &error,
        );
        return Err(error);
    }
    let staged = load_staged_update_metadata(paths)?;
    verify_staged_update_file(paths, &staged)?;
    install_logf(&format!(
        "Staged update {desired} verified; run provision apply-update to activate."
    ));
    println!("{desired}");
    Ok(empty_human_result())
}

pub fn provision_apply_update(paths: &AppliancePaths, args: &[String]) -> Result<serde_json::Value, String> {
    let mut offline = false;
    let mut exec_condition = false;
    for arg in args {
        match arg.as_str() {
            "--offline" => offline = true,
            "--exec-condition" => exec_condition = true,
            other => return Err(format!("unknown apply-update option {other:?}")),
        }
    }
    if exec_condition {
        if offline {
            return Err("--exec-condition cannot be combined with --offline".into());
        }
        return check_apply_update_exec_condition(paths).map(|_| empty_human_result());
    }
    if offline {
        return apply_staged_update_offline(paths);
    }
    schedule_staged_update_apply(paths)
}

pub fn provision_report_update_status(
    paths: &AppliancePaths,
    args: &[String],
) -> Result<serde_json::Value, String> {
    require_agent_role(paths)?;
    let mut status = String::new();
    let mut version = String::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--status" => {
                status = args
                    .get(index + 1)
                    .ok_or_else(|| "--status requires a value".to_string())?
                    .clone();
                index += 2;
            }
            "--version" => {
                version = args
                    .get(index + 1)
                    .ok_or_else(|| "--version requires a value".to_string())?
                    .clone();
                index += 2;
            }
            other => return Err(format!("unknown report-update-status option {other:?}")),
        }
    }
    if status.trim().is_empty() {
        return Err("--status is required".into());
    }
    if version.trim().is_empty() {
        return Err("--version is required".into());
    }
    if status != "applied" && status != "failed" {
        return Err(format!("unsupported report-update-status value {status:?}"));
    }
    let node_id = agent_enrollment_node_id(paths)?;
    let supervisor_url = read_supervisor_base_url(paths)?;
    if supervisor_url.is_empty() {
        return Err("supervisor URL is not configured".into());
    }
    let token = read_enrollment_token(paths)?;
    report_agent_update_status(
        paths,
        &supervisor_url,
        &node_id,
        &token,
        &version,
        &status,
        "",
    )?;
    println!("Reported update status {status:?} for version {version}.");
    Ok(empty_human_result())
}

fn check_apply_update_exec_condition(paths: &AppliancePaths) -> Result<(), String> {
    let metadata = load_staged_update_metadata(paths).map_err(|_| {
        "staged update is not schedulable".to_string()
    })?;
    let state = effective_apply_state(&metadata.apply_state);
    if state != APPLY_STATE_STAGED && state != APPLY_STATE_BOOT_SCHEDULED {
        return Err("staged update is not schedulable".into());
    }
    Ok(())
}

fn schedule_staged_update_apply(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    require_agent_role(paths)?;
    let metadata = match load_staged_update_metadata(paths) {
        Ok(metadata) => metadata,
        Err(error) if error.contains("not found") || !paths.staged_update_meta.exists() => {
            println!("No staged update is pending.");
            return Ok(empty_human_result());
        }
        Err(error) => return Err(error),
    };
    match effective_apply_state(&metadata.apply_state).as_str() {
        APPLY_STATE_STAGED => schedule_initial_update_apply(paths, metadata),
        APPLY_STATE_BOOT_SCHEDULED => retry_boot_scheduled_update_apply(paths, metadata),
        _ => {
            println!("No staged update is pending.");
            Ok(empty_human_result())
        }
    }
}

fn schedule_initial_update_apply(
    paths: &AppliancePaths,
    mut metadata: StagedUpdateMetadata,
) -> Result<serde_json::Value, String> {
    verify_staged_update_file(paths, &metadata)?;
    metadata.apply_state = APPLY_STATE_BOOT_SCHEDULED.into();
    metadata.boot_schedule_attempts = 1;
    save_staged_update_metadata(paths, &metadata)?;
    if let Err(error) = ensure_update_boot_assets(paths) {
        let _ = set_staged_update_apply_state(paths, APPLY_STATE_STAGED);
        return Err(error);
    }
    if let Err(error) = schedule_update_reboot(paths) {
        let _ = set_staged_update_apply_state(paths, APPLY_STATE_STAGED);
        return Err(error);
    }
    println!("Scheduled staged update apply on reboot.");
    Ok(empty_human_result())
}

fn retry_boot_scheduled_update_apply(
    paths: &AppliancePaths,
    mut metadata: StagedUpdateMetadata,
) -> Result<serde_json::Value, String> {
    metadata.boot_schedule_attempts += 1;
    if metadata.boot_schedule_attempts > 3 {
        return Err(mark_update_schedule_failed(
            paths,
            metadata,
            "update boot scheduling exceeded retry limit".into(),
        ));
    }
    save_staged_update_metadata(paths, &metadata)?;
    if let Err(error) = verify_staged_update_file(paths, &metadata) {
        return Err(mark_update_schedule_failed(paths, metadata, error));
    }
    ensure_update_boot_assets(paths)?;
    schedule_update_reboot(paths)?;
    println!("Retrying scheduled update apply on reboot.");
    Ok(empty_human_result())
}

fn mark_update_schedule_failed(
    paths: &AppliancePaths,
    mut metadata: StagedUpdateMetadata,
    cause: String,
) -> String {
    metadata.apply_state = APPLY_STATE_FAILED.into();
    let _ = save_staged_update_metadata(paths, &metadata);
    let _ = record_pending_update_outcome(
        paths,
        &metadata.node_id,
        &metadata.desired_version,
        "failed",
        &cause,
    );
    cause
}

fn schedule_update_reboot(paths: &AppliancePaths) -> Result<(), String> {
    if !paths.update_grub_env.exists() {
        return Err(format!(
            "grub environment is unavailable: {}",
            paths.update_grub_env.display()
        ));
    }
    set_grub_env_var(&paths.update_grub_env, "next_entry", UPDATE_GRUB_ENTRY_NAME)
        .map_err(|error| format!("schedule update boot entry: {error}"))?;
    run_command("sync", &[])?;
    run_command("systemctl", &["reboot"])
}

fn ensure_update_boot_assets(paths: &AppliancePaths) -> Result<(), String> {
    fs::create_dir_all(&paths.update_boot_assets_dir).map_err(|error| error.to_string())?;
    let assets = [
        (&paths.shared_update_vmlinuz, "vmlinuz"),
        (&paths.shared_update_initramfs, "install-initramfs.cpio.gz"),
    ];
    for (source, name) in assets {
        let destination = paths.update_boot_assets_dir.join(name);
        copy_regular_file(source, &destination)
            .map_err(|error| format!("stage update boot asset {name:?}: {error}"))?;
    }
    Ok(())
}

fn apply_staged_update_offline(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let metadata = load_staged_update_metadata(paths)?;
    let state = effective_apply_state(&metadata.apply_state);
    if state != APPLY_STATE_BOOT_SCHEDULED && state != APPLY_STATE_APPLYING {
        return finish_offline_apply_failure(
            paths,
            metadata,
            format!("staged update apply_state {state:?} is not ready for offline apply"),
        );
    }
    if let Err(error) = verify_staged_update_file(paths, &metadata) {
        return finish_offline_apply_failure(paths, metadata, error);
    }
    if let Err(error) = set_staged_update_apply_state(paths, APPLY_STATE_APPLYING) {
        return finish_offline_apply_failure(paths, metadata, error);
    }

    let mut boot_disk = metadata.boot_disk.trim().to_string();
    if boot_disk.is_empty() {
        boot_disk = resolve_host_boot_disk()?;
    }
    if boot_disk.is_empty() {
        return finish_offline_apply_failure(
            paths,
            metadata,
            "host boot disk is unavailable".into(),
        );
    }

    let image_path = paths.staged_update_image.to_string_lossy();
    let target_efi = partition_device(&boot_disk, "1");
    let target_root = partition_device(&boot_disk, "2");
    if let Err(error) =
        copy_staged_release_image_efi_partition(&image_path, &target_efi)
    {
        return finish_offline_apply_failure(paths, metadata, format!("copy EFI partition: {error}"));
    }
    if let Err(error) =
        copy_staged_release_image_root_partition(&image_path, &target_root)
    {
        return finish_offline_apply_failure(paths, metadata, format!("copy root partition: {error}"));
    }
    run_command("sync", &[])?;

    if let Err(error) = record_pending_update_outcome(
        paths,
        &metadata.node_id,
        &metadata.desired_version,
        "applied",
        "",
    ) {
        return finish_offline_apply_failure(paths, metadata, format!("record pending applied outcome: {error}"));
    }
    clear_staged_update(paths)?;
    println!("Applied staged FoldingOS update; rebooting.");
    run_command("sync", &[])?;
    run_command("/bin/busybox", &["reboot", "-f"])?;
    Ok(empty_human_result())
}

fn finish_offline_apply_failure(
    paths: &AppliancePaths,
    mut metadata: StagedUpdateMetadata,
    cause: String,
) -> Result<serde_json::Value, String> {
    metadata.apply_state = APPLY_STATE_FAILED.into();
    let _ = save_staged_update_metadata(paths, &metadata);
    let _ = record_pending_update_outcome(
        paths,
        &metadata.node_id,
        &metadata.desired_version,
        "failed",
        &cause,
    );
    let boot_disk = if metadata.boot_disk.trim().is_empty() {
        resolve_host_boot_disk().unwrap_or_default()
    } else {
        metadata.boot_disk.clone()
    };
    if !boot_disk.is_empty() {
        let _ = clear_grub_next_entry_on_disk(&boot_disk, &paths.update_grub_env);
    }
    eprintln!("Staged update apply failed: {cause}");
    run_command("sync", &[])?;
    run_command("/bin/busybox", &["reboot", "-f"])?;
    Err(cause)
}

fn stage_agent_update(
    paths: &AppliancePaths,
    supervisor_url: &str,
    node_id: &str,
    token: &str,
    current_version: &str,
    desired_version: &str,
) -> Result<(), String> {
    with_staged_update_lock(paths, || {
        stage_agent_update_locked(
            paths,
            supervisor_url,
            node_id,
            token,
            current_version,
            desired_version,
        )
    })
}

fn stage_agent_update_locked(
    paths: &AppliancePaths,
    supervisor_url: &str,
    node_id: &str,
    token: &str,
    current_version: &str,
    desired_version: &str,
) -> Result<(), String> {
    let authorize_url = join_supervisor_url(supervisor_url, "/v1/agents/update/authorize")?;
    let body = serde_json::json!({
        "schema_version": 1,
        "node_id": node_id,
        "enrollment_token": token,
        "current_image_version": current_version,
        "desired_image_version": desired_version,
    });
    let payload = serde_json::to_string(&body).map_err(|error| error.to_string())?;
    let (status, response_body) = http_post_json(&authorize_url, &payload, &[])?;
    if status != 200 {
        return Err(format!(
            "update authorization failed with status {status}: {}",
            response_body.trim()
        ));
    }
    let authorization: UpdateAuthorizeResponse =
        serde_json::from_str(&response_body).map_err(|error| error.to_string())?;

    let boot_disk = resolve_host_boot_disk()?;
    if boot_disk.is_empty() {
        return Err("host boot disk is unavailable".into());
    }

    if let Some(parent) = paths.staged_update_image.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let _ = fs::remove_file(&paths.staged_update_partial);

    let stream_url = join_supervisor_url(supervisor_url, &authorization.image_stream_path)?;
    install_logf(&format!(
        "Downloading assigned release {desired_version} from supervisor ({}). This may take several minutes.",
        format_install_bytes(authorization.image_size_bytes)
    ));
    let (stream_status, mut reader) = crate::provision::util::http_get_stream(
        &stream_url,
        &[
            ("X-FoldingOS-Enrollment-Token", token),
            (UPDATE_SESSION_HEADER, authorization.update_session_id.as_str()),
        ],
    )?;
    if stream_status != 200 {
        return Err(format!("update image stream failed with status {stream_status}"));
    }

    let (digest, written) = write_staged_update_image(paths, &mut reader, authorization.image_size_bytes)?;
    if written != authorization.image_size_bytes {
        return Err(format!(
            "staged update size {written} does not match expected {}",
            authorization.image_size_bytes
        ));
    }
    if !digest.eq_ignore_ascii_case(&authorization.image_sha256) {
        let _ = fs::remove_file(&paths.staged_update_image);
        return Err("staged update failed SHA-256 verification".into());
    }

    let metadata = StagedUpdateMetadata {
        schema_version: 1,
        node_id: node_id.to_string(),
        current_version: current_version.to_string(),
        desired_version: desired_version.to_string(),
        image_sha256: authorization.image_sha256,
        image_size_bytes: authorization.image_size_bytes,
        boot_disk,
        staged_at: rfc3339_now(),
        apply_state: APPLY_STATE_STAGED.into(),
        boot_schedule_attempts: 0,
    };
    save_staged_update_metadata(paths, &metadata)?;
    report_agent_update_status(paths, supervisor_url, node_id, token, desired_version, "staged", "")
}

fn write_staged_update_image(
    paths: &AppliancePaths,
    source: &mut dyn Read,
    size: i64,
) -> Result<(String, i64), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&paths.staged_update_partial)
        .map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut written = 0i64;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut last_report = 0i64;
    while written < size {
        let to_read = std::cmp::min(buffer.len() as i64, size - written) as usize;
        let read = source.read(&mut buffer[..to_read]).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|error| format!("write staged update image: {error}"))?;
        hasher.update(&buffer[..read]);
        written += read as i64;
        if size > 0 && (written - last_report >= INSTALL_PROGRESS_INTERVAL || written == size) {
            last_report = written;
            let percent = written * 100 / size;
            install_logf(&format!(
                "FoldingOS update stage: wrote {} / {} ({}%)",
                format_install_bytes(written),
                format_install_bytes(size),
                percent
            ));
        }
    }
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    fs::rename(&paths.staged_update_partial, &paths.staged_update_image)
        .map_err(|error| error.to_string())?;
    Ok((format!("{:x}", hasher.finalize()), written))
}

fn save_staged_update_metadata(paths: &AppliancePaths, metadata: &StagedUpdateMetadata) -> Result<(), String> {
    let content = serde_json::to_string_pretty(metadata).map_err(|error| error.to_string())?;
    atomic_write(&paths.staged_update_meta, format!("{content}\n").as_bytes(), 0o600)
}

fn load_staged_update_metadata(paths: &AppliancePaths) -> Result<StagedUpdateMetadata, String> {
    let content = fs::read_to_string(&paths.staged_update_meta).map_err(|error| error.to_string())?;
    let metadata: StagedUpdateMetadata =
        serde_json::from_str(&content).map_err(|error| format!("invalid staged update metadata: {error}"))?;
    if metadata.schema_version != 1 {
        return Err(format!(
            "unsupported staged update schema version {}",
            metadata.schema_version
        ));
    }
    Ok(metadata)
}

fn verify_staged_update_file(paths: &AppliancePaths, metadata: &StagedUpdateMetadata) -> Result<(), String> {
    let mut file = File::open(&paths.staged_update_image).map_err(|error| error.to_string())?;
    let file_size = file.metadata().map_err(|error| error.to_string())?.len() as i64;
    if file_size != metadata.image_size_bytes {
        return Err(format!(
            "staged update size {file_size} does not match metadata {}",
            metadata.image_size_bytes
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = format!("{:x}", hasher.finalize());
    if !digest.eq_ignore_ascii_case(&metadata.image_sha256) {
        return Err("staged update failed SHA-256 verification".into());
    }
    Ok(())
}

fn clear_staged_update(paths: &AppliancePaths) -> Result<(), String> {
    for path in [
        &paths.staged_update_image,
        &paths.staged_update_meta,
        &paths.staged_update_partial,
    ] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn effective_apply_state(state: &str) -> String {
    let state = state.trim();
    if state.is_empty() {
        APPLY_STATE_STAGED.into()
    } else {
        state.to_string()
    }
}

fn is_locked_staged_update_apply_state(state: &str) -> bool {
    matches!(
        effective_apply_state(state).as_str(),
        APPLY_STATE_BOOT_SCHEDULED | APPLY_STATE_APPLYING | APPLY_STATE_FAILED
    )
}

fn set_staged_update_apply_state(paths: &AppliancePaths, state: &str) -> Result<(), String> {
    let mut metadata = load_staged_update_metadata(paths)?;
    metadata.apply_state = state.into();
    save_staged_update_metadata(paths, &metadata)
}

fn save_update_session(paths: &AppliancePaths, session: &UpdateSession) -> Result<(), String> {
    let content = serde_json::to_string_pretty(session).map_err(|error| error.to_string())?;
    atomic_write(
        &paths.update_session_path(&session.session_id),
        format!("{content}\n").as_bytes(),
        0o600,
    )
}

fn load_update_session(paths: &AppliancePaths, session_id: &str) -> Result<UpdateSession, String> {
    let content = fs::read_to_string(paths.update_session_path(session_id))
        .map_err(|error| error.to_string())?;
    let session: UpdateSession =
        serde_json::from_str(&content).map_err(|error| format!("invalid update session: {error}"))?;
    if session.schema_version != 1 {
        return Err(format!(
            "unsupported update session schema version {}",
            session.schema_version
        ));
    }
    if session.session_id.trim().is_empty() {
        return Err("update session is missing session_id".into());
    }
    Ok(session)
}

fn save_pending_update_report(paths: &AppliancePaths, report: &PendingUpdateReport) -> Result<(), String> {
    let content = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
    atomic_write(&paths.pending_update_report, format!("{content}\n").as_bytes(), 0o600)
}

fn load_pending_update_report(paths: &AppliancePaths) -> Result<PendingUpdateReport, String> {
    let content = fs::read_to_string(&paths.pending_update_report).map_err(|error| error.to_string())?;
    let report: PendingUpdateReport =
        serde_json::from_str(&content).map_err(|error| format!("invalid pending update report: {error}"))?;
    if report.schema_version != 1 {
        return Err(format!(
            "unsupported pending update report schema version {}",
            report.schema_version
        ));
    }
    Ok(report)
}

fn clear_pending_update_report(paths: &AppliancePaths) -> Result<(), String> {
    match fs::remove_file(&paths.pending_update_report) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn record_pending_update_outcome(
    paths: &AppliancePaths,
    node_id: &str,
    version: &str,
    status: &str,
    message: &str,
) -> Result<(), String> {
    let report = PendingUpdateReport {
        schema_version: 1,
        node_id: node_id.to_string(),
        image_version: version.trim().to_string(),
        status: status.trim().to_string(),
        message: message.trim().to_string(),
        recorded_at: rfc3339_now(),
    };
    save_pending_update_report(paths, &report)?;
    let _ = flush_pending_update_report(paths);
    Ok(())
}

fn flush_pending_update_report(paths: &AppliancePaths) -> Result<(), String> {
    let report = match load_pending_update_report(paths) {
        Ok(report) => report,
        Err(error) if error.contains("not found") || !paths.pending_update_report.exists() => {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let status = report.status.trim();
    if status != "applied" && status != "failed" {
        return Err(format!("pending update status {status:?} is not deliverable"));
    }
    let supervisor_url = read_supervisor_base_url(paths)?;
    if supervisor_url.is_empty() {
        return Ok(());
    }
    let token = read_enrollment_token(paths)?;
    let node_id = if report.node_id.trim().is_empty() {
        agent_enrollment_node_id(paths)?
    } else {
        report.node_id
    };
    report_agent_update_status(
        paths,
        &supervisor_url,
        &node_id,
        &token,
        &report.image_version,
        status,
        &report.message,
    )?;
    clear_pending_update_report(paths)
}

fn report_agent_update_status(
    _paths: &AppliancePaths,
    supervisor_url: &str,
    node_id: &str,
    token: &str,
    version: &str,
    status: &str,
    message: &str,
) -> Result<(), String> {
    if supervisor_url.is_empty() {
        return Ok(());
    }
    let endpoint = join_supervisor_url(supervisor_url, "/v1/agents/update/status")?;
    let body = serde_json::json!({
        "schema_version": 1,
        "node_id": node_id,
        "enrollment_token": token,
        "image_version": version,
        "status": status,
        "message": message,
    });
    let payload = serde_json::to_string(&body).map_err(|error| error.to_string())?;
    let (code, response_body) = http_post_json(&endpoint, &payload, &[])?;
    if code != 200 {
        return Err(format!(
            "update status report failed with status {code}: {}",
            response_body.trim()
        ));
    }
    Ok(())
}
