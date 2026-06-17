use std::fs;
use std::path::Path;

use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;

use crate::provision::util::{command_output, efi_partition_path};
use crate::registry_image::RELEASE_IMAGE_SIZE_BYTES;

static DISK_PATH_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^/dev/(sd[a-z]+|vd[a-z]+|nvme[0-9]+n[0-9]+)$").expect("disk path pattern compiles")
});

#[derive(Debug, Clone)]
pub struct ProvisionTargetDisk {
    pub path: String,
    pub size_bytes: i64,
    pub serial: String,
    pub transport: String,
    pub removable: bool,
    pub device_type: String,
}

pub fn validate_provision_target_disk(path: &str) -> Result<ProvisionTargetDisk, String> {
    let disk = inspect_provision_target_disk(path)?;
    if disk.device_type != "disk" {
        return Err(format!("target {path:?} is not a whole disk"));
    }
    if disk.removable {
        return Err(format!(
            "target {path:?} is removable and is not eligible for network provisioning"
        ));
    }
    if !is_eligible_provision_transport(&disk) {
        return Err(format!(
            "target {:?} uses transport {:?}; only internal SATA or NVMe targets are eligible",
            disk.path, disk.transport
        ));
    }
    if disk.serial.trim().is_empty() {
        return Err("target disk serial number is required".into());
    }
    if disk.size_bytes < RELEASE_IMAGE_SIZE_BYTES {
        return Err(format!(
            "target {:?} is too small ({} bytes); release image requires {} bytes",
            disk.path, disk.size_bytes, RELEASE_IMAGE_SIZE_BYTES
        ));
    }
    if let Ok(boot_disk) = resolve_host_boot_disk() {
        if !boot_disk.is_empty() && boot_disk == disk.path {
            return Err(format!("refusing to provision the host boot disk {path:?}"));
        }
    }
    let mounted = list_mounted_block_devices()?;
    for device in mounted {
        if device == disk.path || device.starts_with(&disk.path) {
            return Err(format!("target {path:?} has mounted filesystems"));
        }
    }
    Ok(disk)
}

pub fn parse_provision_install_disk_path(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("install disk path is empty".into());
    }
    if !DISK_PATH_PATTERN.is_match(path) {
        return Err(format!(
            "install disk must be a whole-disk device path such as /dev/sda or /dev/nvme0n1: {path:?}"
        ));
    }
    Ok(path.to_string())
}

pub fn select_provision_install_disk() -> Result<String, String> {
    let listing = command_output(
        "lsblk",
        &["-J", "-b", "-d", "-o", "NAME,TYPE,TRAN,SIZE,SERIAL,RM,PATH"],
    )?;
    let parsed: LsblkOutput = serde_json::from_str(&listing)
        .map_err(|error| format!("parse lsblk output: {error}"))?;
    for device in parsed.blockdevices {
        let path = device.resolved_path();
        if validate_provision_target_disk(&path).is_ok() {
            return Ok(path);
        }
    }
    Err("no eligible internal target disk was found".into())
}

pub fn resolve_host_boot_disk() -> Result<String, String> {
    let root_source = command_output("findmnt", &["-n", "-o", "SOURCE", "/"]).unwrap_or_default();
    let root_source = root_source.trim();
    if !root_source.starts_with("/dev/") {
        return Ok(String::new());
    }
    let parent_name = command_output("lsblk", &["-n", "-o", "PKNAME", root_source])?;
    let parent_name = parent_name.trim();
    if parent_name.is_empty() || parent_name.contains('/') {
        return Ok(String::new());
    }
    Ok(format!("/dev/{parent_name}"))
}

fn list_mounted_block_devices() -> Result<Vec<String>, String> {
    let listing = command_output("findmnt", &["-rn", "-o", "SOURCE"])?;
    Ok(listing
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && line.starts_with("/dev/"))
        .map(str::to_string)
        .collect())
}

fn inspect_provision_target_disk(path: &str) -> Result<ProvisionTargetDisk, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("target disk path is required".into());
    }
    if !path.starts_with("/dev/") {
        return Err(format!("target disk must be a block device path: {path:?}"));
    }
    if Path::new(path).file_name().and_then(|name| name.to_str()).unwrap_or("").contains('/') {
        return Err(format!(
            "target disk must be a whole-disk device, not a partition: {path:?}"
        ));
    }
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if !metadata.file_type().is_block_device() {
            return Err(format!("{path:?} is not a block device"));
        }
    }
    #[cfg(not(unix))]
    let _ = metadata;

    let listing = command_output(
        "lsblk",
        &["-J", "-b", "-d", "-o", "NAME,TYPE,TRAN,SIZE,SERIAL,RM,PATH"],
    )?;
    let parsed: LsblkOutput = serde_json::from_str(&listing)
        .map_err(|error| format!("parse lsblk output: {error}"))?;
    for device in parsed.blockdevices {
        let device_path = device.resolved_path();
        if device_path != path {
            continue;
        }
        let mut serial = device.serial.trim().to_string();
        if serial.is_empty() {
            serial = read_provision_target_disk_serial_from_sysfs(&device_path);
        }
        return Ok(ProvisionTargetDisk {
            path: device_path,
            size_bytes: device.size,
            serial,
            transport: device.tran.trim().to_string(),
            removable: device.rm,
            device_type: device.kind.trim().to_string(),
        });
    }
    Err(format!("target disk {path:?} was not found"))
}

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Deserialize)]
struct LsblkDevice {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    tran: String,
    size: i64,
    serial: String,
    rm: bool,
    path: String,
}

impl LsblkDevice {
    fn resolved_path(&self) -> String {
        if self.path.trim().is_empty() {
            format!("/dev/{}", self.name)
        } else {
            self.path.clone()
        }
    }
}

fn read_provision_target_disk_serial_from_sysfs(device_path: &str) -> String {
    let name = Path::new(device_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .trim();
    if name.is_empty() {
        return String::new();
    }
    let mut candidates = vec![format!("/sys/block/{name}/device/serial")];
    if name.starts_with("nvme") {
        let controller = name.rfind('n').filter(|idx| *idx > "nvme".len()).map(|idx| &name[..idx]);
        if let Some(controller) = controller {
            candidates.push(format!("/sys/class/nvme/{controller}/serial"));
        }
    }
    for candidate in candidates {
        if let Ok(content) = fs::read_to_string(&candidate) {
            let serial = content.trim();
            if !serial.is_empty() {
                return serial.to_string();
            }
        }
    }
    String::new()
}

fn is_eligible_provision_transport(disk: &ProvisionTargetDisk) -> bool {
    let transport = disk.transport.to_ascii_lowercase();
    let name = Path::new(&disk.path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if transport == "usb" {
        return false;
    }
    if transport == "sata" || transport == "ata" || transport == "nvme" {
        return true;
    }
    if name.contains("nvme") {
        return true;
    }
    if name.starts_with("sd") && name.len() >= 3 {
        return transport.is_empty() || transport == "sata" || transport == "ata";
    }
    if name.starts_with("vd") && name.len() >= 3 {
        return transport.is_empty() || transport == "sata" || transport == "ata";
    }
    false
}

pub fn clear_grub_next_entry_on_disk(disk: &str, update_grub_env: &Path) -> Result<(), String> {
    if update_grub_env.exists() {
        return crate::provision::grub_env::clear_grub_next_entry(update_grub_env);
    }
    let efi_partition = efi_partition_path(disk);
    if crate::provision::util::mounted(&efi_partition) {
        return Err(format!("EFI partition {efi_partition} is mounted"));
    }
    let mount_point = tempfile_mount_dir("foldingos-grubenv-")?;
    let _ = run_mount_umount(&efi_partition, &mount_point, |mount| {
        crate::provision::grub_env::clear_grub_next_entry(
            &mount.join("EFI/BOOT/grubenv"),
        )
    });
    crate::provision::util::run_command("sync", &[])
}

fn tempfile_mount_dir(prefix: &str) -> Result<std::path::PathBuf, String> {
    let base = crate::provision::util::provision_scratch_dir();
    let path = std::path::Path::new(base).join(format!("{prefix}{}", std::process::id()));
    fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

fn run_mount_umount<F>(device: &str, mount_point: &Path, operation: F) -> Result<(), String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    crate::provision::util::run_command("mount", &[device, &mount_point.to_string_lossy()])?;
    let result = operation(mount_point);
    let _ = crate::provision::util::run_command("umount", &[&mount_point.to_string_lossy()]);
    result
}
