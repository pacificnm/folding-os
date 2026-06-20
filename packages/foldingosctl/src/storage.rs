use std::collections::HashMap;
use std::path::Path;

use regex::Regex;
use std::sync::LazyLock;

use crate::paths::AppliancePaths;
use crate::process::{command_output, run_command, run_fsck_ext4};

const DATA_PARTITION_NUMBER: &str = "3";
const DATA_PARTITION_NAME: &str = "FOLDINGOS_DATA";
const DATA_PARTITION_GUID: &str = "464f4c44-494e-474f-5344-415441000001";
const DATA_PARTITION_START: u64 = 5_244_928;
const MINIMUM_DISK_SECTORS: u64 = 8_388_608;
const SECTOR_ALIGNMENT: u64 = 2048;

static LAST_USABLE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"last usable sector is ([0-9]+)").expect("pattern compiles"));
static PARTITION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*([0-9]+)\s+([0-9]+)\s+([0-9]+)\s+").expect("pattern compiles")
});

#[derive(Debug, Clone, Copy)]
struct Partition {
    start: u64,
    end: u64,
}

pub fn expand_data(_paths: &AppliancePaths) -> Result<(), String> {
    let root_source = command_output("findmnt", &["-n", "-o", "SOURCE", "/"])?;
    let root_source = root_source.trim();
    if !root_source.starts_with("/dev/") {
        return Err(format!(
            "root source is not a block device: {root_source:?}"
        ));
    }

    let parent_name = command_output("lsblk", &["-n", "-o", "PKNAME", root_source])?;
    let parent_name = parent_name.trim();
    if parent_name.is_empty() || parent_name.contains('/') {
        return Err(format!("could not resolve boot disk from {root_source:?}"));
    }
    let disk = format!("/dev/{parent_name}");

    let table = command_output("sgdisk", &["--print", &disk])?;
    let (mut last_usable, partitions) = parse_table(&table)?;
    if last_usable + 34 < MINIMUM_DISK_SECTORS {
        return Err("boot disk is smaller than the release image".into());
    }
    validate_layout(&disk, &partitions)?;

    let data = partitions
        .get(&3)
        .copied()
        .ok_or_else(|| "unexpected data partition start sector".to_string())?;
    if data.start != DATA_PARTITION_START {
        return Err("unexpected data partition start sector".into());
    }

    let data_device = partition_device(&disk, DATA_PARTITION_NUMBER);
    if mounted(&data_device) {
        return Err(format!(
            "refusing to resize mounted data filesystem {data_device}"
        ));
    }
    run_fsck_ext4(&data_device, false)?;

    let disk_size_text = command_output("lsblk", &["-b", "-d", "-n", "-o", "SIZE", &disk])?;
    let disk_bytes: u64 = disk_size_text
        .trim()
        .parse()
        .map_err(|error| format!("could not determine boot disk size: {error}"))?;
    let disk_sectors = disk_bytes / 512;
    if disk_sectors < MINIMUM_DISK_SECTORS {
        return Err("boot disk is smaller than the release image".into());
    }
    if disk_sectors > last_usable + 34 {
        run_command("sgdisk", &["--move-second-header", &disk])?;
        let table = command_output("sgdisk", &["--print", &disk])?;
        last_usable = parse_table(&table)?.0;
    }

    let target_end = aligned_end(last_usable);
    if data.end > target_end {
        return Err(format!(
            "refusing to shrink data partition from sector {} to {target_end}",
            data.end
        ));
    }
    if data.end == target_end {
        println!("Data partition already occupies available aligned capacity.");
        return Ok(());
    }

    run_fsck_ext4(&data_device, true)?;

    run_command(
        "sgdisk",
        &[
            "--delete=3",
            &format!("--new=3:{DATA_PARTITION_START}:{target_end}"),
            "--typecode=3:8300",
            &format!("--change-name=3:{DATA_PARTITION_NAME}"),
            &format!("--partition-guid=3:{DATA_PARTITION_GUID}"),
            &disk,
        ],
    )?;
    run_command("partx", &["--update", "--nr", DATA_PARTITION_NUMBER, &disk])?;
    run_command("resize2fs", &[&data_device])?;

    println!("Expanded {data_device} to sector {target_end}.");
    Ok(())
}

fn parse_table(table: &str) -> Result<(u64, HashMap<i32, Partition>), String> {
    let last_match = LAST_USABLE_PATTERN
        .captures(table)
        .and_then(|captures| captures.get(1))
        .ok_or_else(|| "could not determine GPT last usable sector".to_string())?;
    let last_usable: u64 = last_match
        .as_str()
        .parse()
        .map_err(|error| format!("parse last usable sector: {error}"))?;

    let mut partitions = HashMap::new();
    for captures in PARTITION_PATTERN.captures_iter(table) {
        let number: i32 = captures[1]
            .parse()
            .map_err(|error| format!("parse partition number: {error}"))?;
        let start: u64 = captures[2]
            .parse()
            .map_err(|error| format!("parse partition start: {error}"))?;
        let end: u64 = captures[3]
            .parse()
            .map_err(|error| format!("parse partition end: {error}"))?;
        partitions.insert(number, Partition { start, end });
    }
    Ok((last_usable, partitions))
}

fn validate_layout(disk: &str, partitions: &HashMap<i32, Partition>) -> Result<(), String> {
    if partitions.len() != 3 {
        return Err(format!(
            "expected exactly three GPT partitions, found {}",
            partitions.len()
        ));
    }
    let data = partitions
        .get(&3)
        .ok_or_else(|| "unexpected data partition start sector".to_string())?;
    if data.start != DATA_PARTITION_START {
        return Err("unexpected data partition start sector".into());
    }

    let expected_names = [
        (1, "FOLDINGOS_EFI"),
        (2, "FOLDINGOS_ROOT"),
        (3, DATA_PARTITION_NAME),
    ];
    for (number, name) in expected_names {
        let info = command_output("sgdisk", &[&format!("--info={number}"), disk])?;
        if !info.contains(name) {
            return Err(format!(
                "partition {number} name does not match approved layout"
            ));
        }
        if number == 3
            && !info
                .to_uppercase()
                .contains(&DATA_PARTITION_GUID.to_uppercase())
        {
            return Err("data partition identity does not match approved layout".into());
        }
    }
    Ok(())
}

pub fn partition_device(disk: &str, number: &str) -> String {
    let base = Path::new(disk)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if base.chars().last().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("{disk}p{number}")
    } else {
        format!("{disk}{number}")
    }
}

fn mounted(device: &str) -> bool {
    std::process::Command::new("findmnt")
        .args(["-n", device])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn aligned_end(last_usable: u64) -> u64 {
    ((last_usable + 1) / SECTOR_ALIGNMENT * SECTOR_ALIGNMENT) - 1
}

pub fn resolve_boot_disk() -> Result<String, String> {
    let root_source = command_output("findmnt", &["-n", "-o", "SOURCE", "/"])?;
    let root_source = root_source.trim();
    if !root_source.starts_with("/dev/") {
        return Err(format!(
            "root source is not a block device: {root_source:?}"
        ));
    }

    let parent_name = command_output("lsblk", &["-n", "-o", "PKNAME", root_source])?;
    let parent_name = parent_name.trim();
    if parent_name.is_empty() || parent_name.contains('/') {
        return Err(format!("could not resolve boot disk from {root_source:?}"));
    }
    Ok(format!("/dev/{parent_name}"))
}
