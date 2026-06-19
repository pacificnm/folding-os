use std::fs::{self, OpenOptions};
use std::io::{Read, Write};

use sha2::{Digest, Sha256};

use crate::fs_atomic::atomic_write;
use crate::identity::collect_mac_addresses;
use crate::paths::AppliancePaths;
use crate::provision::authorize::ProvisionAuthorizeResponse;
use crate::provision::grub_env::clear_grub_next_entry;
use crate::provision::role_cmd::parse_installation_role_bytes;
use crate::provision::ssh::validate_authorized_keys;
use crate::provision::targets::{select_provision_install_disk, validate_provision_target_disk};
use crate::provision::util::{
    empty_human_result, format_install_bytes, http_post_json, install_logf, join_supervisor_url,
    mounted, partition_device, provision_scratch_dir, read_enrollment_token,
    read_supervisor_base_url, run_command, AGENT_INSTALLATION_ROLE, DATA_PARTITION_NUMBER,
    INSTALL_SESSION_HEADER,
};

const INSTALL_PROGRESS_INTERVAL: i64 = 128 * 1024 * 1024;

pub fn provision_install(
    paths: &AppliancePaths,
    args: &[String],
) -> Result<serde_json::Value, String> {
    let mut disk = String::new();
    let mut version = String::new();
    let mut supervisor_url = String::new();
    let mut enrollment_token = String::new();
    let mut auto_disk = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--disk" => {
                disk = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --disk".to_string())?
                    .clone();
                index += 2;
            }
            "--auto-disk" => {
                auto_disk = true;
                index += 1;
            }
            "--version" => {
                version = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --version".to_string())?
                    .clone();
                index += 2;
            }
            "--supervisor-url" => {
                supervisor_url = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --supervisor-url".to_string())?
                    .clone();
                index += 2;
            }
            "--enrollment-token" => {
                enrollment_token = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --enrollment-token".to_string())?
                    .clone();
                index += 2;
            }
            other => return Err(format!("unknown install option {other:?}")),
        }
    }

    if disk.is_empty() && !auto_disk {
        return Err("target disk is required (--disk or --auto-disk)".into());
    }
    if !disk.is_empty() && auto_disk {
        return Err("use either --disk or --auto-disk, not both".into());
    }

    if supervisor_url.is_empty() {
        supervisor_url = read_supervisor_base_url(paths)?;
    }
    if supervisor_url.is_empty() {
        return Err("supervisor URL is not configured".into());
    }
    if enrollment_token.is_empty() {
        enrollment_token = read_enrollment_token(paths)
            .map_err(|error| format!("enrollment token is not configured: {error}"))?;
    }

    if auto_disk {
        disk = select_provision_install_disk()?;
        install_logf(&format!("Selected install disk {disk}."));
    }

    let target = validate_provision_target_disk(&disk)?;
    install_logf(&format!(
        "Validated target {} (serial {}, transport {}, size {}).",
        target.path,
        target.serial,
        target.transport,
        format_install_bytes(target.size_bytes),
    ));
    let mac_addresses = collect_mac_addresses()?;

    install_logf(&format!(
        "Requesting install authorization from {supervisor_url}."
    ));
    let authorize_url = join_supervisor_url(&supervisor_url, "/v1/provision/authorize")?;
    let authorize_body = serde_json::json!({
        "schema_version": 1,
        "enrollment_token": enrollment_token,
        "mac_addresses": mac_addresses,
        "target_disk": target.path,
        "target_serial": target.serial,
        "image_version": version,
    });
    let payload = serde_json::to_string(&authorize_body).map_err(|error| error.to_string())?;
    let (status, response_body) = http_post_json(&authorize_url, &payload, &[])?;
    if status != 200 {
        return Err(format!(
            "provisioning authorization failed with status {status}: {}",
            response_body.trim()
        ));
    }
    let authorization: ProvisionAuthorizeResponse =
        serde_json::from_str(&response_body).map_err(|error| error.to_string())?;
    if authorization.installation_role != AGENT_INSTALLATION_ROLE {
        return Err(format!(
            "supervisor returned unexpected installation role {:?}",
            authorization.installation_role
        ));
    }
    if authorization.target_disk != target.path {
        return Err(format!(
            "supervisor authorized disk {:?}, expected {:?}",
            authorization.target_disk, target.path
        ));
    }
    install_logf(&format!(
        "Authorized FoldingOS {} ({}) for session {}.",
        authorization.image_version,
        format_install_bytes(authorization.image_size_bytes),
        authorization.install_session_id,
    ));

    let stream_url = join_supervisor_url(&supervisor_url, &authorization.image_stream_path)?;
    install_logf(&format!("Streaming release image from {stream_url}."));
    let (stream_status, mut reader) = crate::provision::util::http_get_stream(
        &stream_url,
        &[
            ("X-FoldingOS-Enrollment-Token", enrollment_token.as_str()),
            (
                INSTALL_SESSION_HEADER,
                authorization.install_session_id.as_str(),
            ),
        ],
    )?;
    if stream_status != 200 {
        return Err(format!("image stream failed with status {stream_status}"));
    }

    let (digest, written) =
        write_provision_image_to_disk(&target.path, &mut reader, authorization.image_size_bytes)?;
    if written != authorization.image_size_bytes {
        return Err(format!(
            "installed image size {written} does not match expected {}",
            authorization.image_size_bytes
        ));
    }
    if !digest.eq_ignore_ascii_case(&authorization.image_sha256) {
        return Err("installed image failed SHA-256 verification".into());
    }
    install_logf(&format!(
        "Verified FoldingOS {} on {} ({}).",
        authorization.image_version, target.path, digest
    ));

    relocate_provision_gpt(
        &target.path,
        target.size_bytes,
        authorization.image_size_bytes,
    )?;
    stage_provision_boot_files(
        &target.path,
        &authorization.installation_role,
        authorization.authorized_keys.as_bytes(),
        &authorization.foldops_ingest_token,
    )?;
    stage_provision_persistent_config(
        paths,
        &target.path,
        &authorization.installation_role,
        &supervisor_url,
        &enrollment_token,
        authorization.foldops_supervisor_ca_pem.as_bytes(),
    )?;

    install_logf(&format!(
        "Provisioned {} with role {}.",
        target.path, authorization.installation_role
    ));
    install_logf("Reboot the target into internal storage to complete installation.");
    Ok(empty_human_result())
}

fn write_provision_image_to_disk(
    disk: &str,
    source: &mut dyn Read,
    size: i64,
) -> Result<(String, i64), String> {
    install_logf(&format!(
        "Writing {} to {disk}.",
        format_install_bytes(size)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .open(disk)
        .map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut written = 0i64;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut last_report = 0i64;
    while written < size {
        let to_read = std::cmp::min(buffer.len() as i64, size - written) as usize;
        let read = source
            .read(&mut buffer[..to_read])
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|error| format!("write release image to {disk}: {error}"))?;
        hasher.update(&buffer[..read]);
        written += read as i64;
        if size > 0 && (written - last_report >= INSTALL_PROGRESS_INTERVAL || written == size) {
            last_report = written;
            let percent = written * 100 / size;
            install_logf(&format!(
                "FoldingOS install: wrote {} / {} ({}%)",
                format_install_bytes(written),
                format_install_bytes(size),
                percent
            ));
        }
    }
    file.sync_all().map_err(|error| error.to_string())?;
    Ok((format!("{:x}", hasher.finalize()), written))
}

fn relocate_provision_gpt(disk: &str, device_size: i64, image_size: i64) -> Result<(), String> {
    if device_size <= image_size {
        return Ok(());
    }
    install_logf(&format!("Relocating backup GPT header on {disk}."));
    run_command("sgdisk", &["-e", disk])?;
    if run_command("partprobe", &[disk]).is_ok() {
        return run_command("sync", &[]);
    }
    run_command("sync", &[])
}

fn stage_provision_boot_files(
    disk: &str,
    role: &str,
    authorized_keys: &[u8],
    foldops_ingest_token: &str,
) -> Result<(), String> {
    if role.trim().is_empty() {
        return Err("installation role is required".into());
    }
    validate_authorized_keys(authorized_keys)
        .map_err(|error| format!("authorized keys are invalid: {error}"))?;
    let efi_partition = crate::provision::util::efi_partition_path(disk);
    if mounted(&efi_partition) {
        return Err(format!("EFI partition {efi_partition} is mounted"));
    }
    let mount_point = tempfile_dir("foldingos-provision-esp-")?;
    mount_and_run(&efi_partition, &mount_point, |root| {
        let provision_dir = root.join("foldingos/provision");
        fs::create_dir_all(&provision_dir).map_err(|error| error.to_string())?;
        fs::write(provision_dir.join("installation-role"), role)
            .map_err(|error| error.to_string())?;
        fs::write(provision_dir.join("authorized_keys"), authorized_keys)
            .map_err(|error| error.to_string())?;
        if role == AGENT_INSTALLATION_ROLE {
            let token = foldops_ingest_token.trim();
            if token.is_empty() {
                return Err(
                    "supervisor ingest token is invalid: supervisor ingest token is empty".into(),
                );
            }
            fs::write(
                provision_dir.join("foldops-ingest-token"),
                format!("{token}\n"),
            )
            .map_err(|error| error.to_string())?;
        }
        clear_grub_next_entry(&root.join("EFI/BOOT/grubenv"))?;
        run_command("sync", &[])
    })?;
    install_logf(&format!(
        "Staged installation role and SSH keys on {efi_partition}."
    ));
    Ok(())
}

fn stage_provision_persistent_config(
    paths: &AppliancePaths,
    disk: &str,
    role: &str,
    supervisor_url: &str,
    enrollment_token: &str,
    foldops_supervisor_ca_pem: &[u8],
) -> Result<(), String> {
    let role = role.trim();
    if role.is_empty() {
        return Err("installation role is required".into());
    }
    parse_installation_role_bytes(role.as_bytes())
        .map_err(|error| format!("installation role is invalid: {error}"))?;
    let supervisor_url = supervisor_url.trim().trim_end_matches('/');
    if supervisor_url.is_empty() {
        return Err("supervisor URL is required for network install".into());
    }
    let _ = join_supervisor_url(supervisor_url, "/")?;
    let enrollment_token = enrollment_token.trim();
    if enrollment_token.is_empty() {
        return Err("enrollment token is required for network install".into());
    }

    let data_partition = partition_device(disk, DATA_PARTITION_NUMBER);
    if mounted(&data_partition) {
        return Err(format!("data partition {data_partition} is mounted"));
    }
    let mount_point = tempfile_dir("foldingos-provision-data-")?;
    mount_and_run_ext4(&data_partition, &mount_point, |root| {
        write_provision_persistent_files(
            paths,
            root,
            role,
            supervisor_url,
            enrollment_token,
            foldops_supervisor_ca_pem,
        )
    })?;
    install_logf(&format!(
        "Staged installation role and provisioning config on {data_partition}."
    ));
    Ok(())
}

fn write_provision_persistent_files(
    _paths: &AppliancePaths,
    root: &std::path::Path,
    role: &str,
    supervisor_url: &str,
    enrollment_token: &str,
    foldops_supervisor_ca_pem: &[u8],
) -> Result<(), String> {
    reset_agent_data_partition_state(root)?;
    fs::create_dir_all(root.join("config/provision")).map_err(|error| error.to_string())?;
    atomic_write(
        &root.join("config/installation-role"),
        role.as_bytes(),
        0o644,
    )?;
    atomic_write(
        &root.join("config/provision/supervisor.url"),
        format!("{supervisor_url}\n").as_bytes(),
        0o644,
    )?;
    atomic_write(
        &root.join("config/provision/enrollment-token"),
        format!("{enrollment_token}\n").as_bytes(),
        0o600,
    )?;
    if role == AGENT_INSTALLATION_ROLE {
        if foldops_supervisor_ca_pem
            .iter()
            .all(|b| b.is_ascii_whitespace())
        {
            return Err("supervisor TLS CA material is empty".into());
        }
        let foldops_config_dir = root.join("config/foldops");
        fs::create_dir_all(&foldops_config_dir).map_err(|error| error.to_string())?;
        atomic_write(
            &foldops_config_dir.join("supervisor-ca.pem"),
            foldops_supervisor_ca_pem,
            0o644,
        )?;
    }
    Ok(())
}

fn reset_agent_data_partition_state(root: &std::path::Path) -> Result<(), String> {
    for relative in ["config", "registry", "provision", "state"] {
        match fs::remove_dir_all(root.join(relative)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("reset inherited data at {relative}: {error}")),
        }
    }
    Ok(())
}

fn tempfile_dir(prefix: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(provision_scratch_dir())
        .join(format!("{prefix}{}", std::process::id()));
    fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

fn mount_and_run<F>(device: &str, mount_point: &std::path::Path, operation: F) -> Result<(), String>
where
    F: FnOnce(&std::path::Path) -> Result<(), String>,
{
    run_command("mount", &[device, &mount_point.to_string_lossy()])?;
    let result = operation(mount_point);
    let _ = run_command("umount", &[&mount_point.to_string_lossy()]);
    let _ = fs::remove_dir_all(mount_point);
    result
}

fn mount_and_run_ext4<F>(
    device: &str,
    mount_point: &std::path::Path,
    operation: F,
) -> Result<(), String>
where
    F: FnOnce(&std::path::Path) -> Result<(), String>,
{
    run_command(
        "mount",
        &[
            "-t",
            "ext4",
            "-o",
            "rw",
            device,
            &mount_point.to_string_lossy(),
        ],
    )?;
    let result = operation(mount_point);
    let _ = run_command("umount", &[&mount_point.to_string_lossy()]);
    let _ = fs::remove_dir_all(mount_point);
    result?;
    run_command("sync", &[])
}
