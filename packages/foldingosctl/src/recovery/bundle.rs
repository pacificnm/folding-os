use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::Builder;

use crate::paths::AppliancePaths;

pub const MANIFEST_SCHEMA_VERSION: i32 = 1;
pub const MAX_EXPORT_BYTES: u64 = 512 * 1024 * 1024;
pub const BACKUP_RETENTION: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleFileEntry {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryManifest {
    pub schema_version: i32,
    pub export_timestamp: String,
    pub hostname: String,
    pub foldingos_version: String,
    pub include_secrets: bool,
    pub files: Vec<BundleFileEntry>,
    pub archive_sha256: String,
}

#[derive(Debug, Clone)]
pub struct StagedBundleFile {
    pub archive_path: String,
    pub source_path: PathBuf,
}

pub fn collect_export_files(
    paths: &AppliancePaths,
    include_secrets: bool,
) -> Result<Vec<StagedBundleFile>, String> {
    let mut files = Vec::new();

    if paths.foldops_db.exists() {
        files.push(StagedBundleFile {
            archive_path: archive_path_for(paths, &paths.foldops_db)?,
            source_path: paths.foldops_db.clone(),
        });
    }

    collect_tree(
        paths,
        &paths.foldops_config_dir,
        &paths.foldops_config_dir,
        &mut files,
    )?;

    collect_tree(
        paths,
        &paths.provision_enrollments_dir,
        &paths.provision_enrollments_dir,
        &mut files,
    )?;

    for optional in [
        &paths.boot_allowlist,
        &paths.boot_install_disk_allowlist,
        &paths.foldops_assigned_manifest,
        &paths.tools_assigned_version,
        &paths.foldops_supervisor_ca_pem(),
    ] {
        if optional.is_file() {
            files.push(StagedBundleFile {
                archive_path: archive_path_for(paths, optional)?,
                source_path: optional.clone(),
            });
        }
    }

    if include_secrets && paths.foldops_tls_dir.is_dir() {
        collect_tree(
            paths,
            &paths.foldops_tls_dir,
            &paths.foldops_tls_dir,
            &mut files,
        )?;
    }

    if files.is_empty() {
        return Err("recovery export found no supervisor state files to include".into());
    }

    files.sort_by(|left, right| left.archive_path.cmp(&right.archive_path));
    files.dedup_by(|left, right| left.archive_path == right.archive_path);
    Ok(files)
}

fn collect_tree(
    paths: &AppliancePaths,
    root: &Path,
    current: &Path,
    files: &mut Vec<StagedBundleFile>,
) -> Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    if current.is_file() {
        files.push(StagedBundleFile {
            archive_path: archive_path_for(paths, current)?,
            source_path: current.to_path_buf(),
        });
        return Ok(());
    }
    for entry in fs::read_dir(current).map_err(|error| format!("read {}: {error}", current.display()))? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_tree(paths, root, &path, files)?;
            continue;
        }
        if path.is_file() {
            files.push(StagedBundleFile {
                archive_path: archive_path_for(paths, &path)?,
                source_path: path,
            });
        }
    }
    let _ = root;
    Ok(())
}

pub fn archive_path_for(paths: &AppliancePaths, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(&paths.data_root).map_err(|_| {
        format!(
            "path {} is outside data root {}",
            path.display(),
            paths.data_root.display()
        )
    })?;
    let archive = format!(
        "data/{}",
        relative
            .to_string_lossy()
            .trim_start_matches('/')
    );
    validate_archive_path(&archive)?;
    Ok(archive)
}

pub fn validate_archive_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.starts_with('/') {
        return Err(format!("invalid archive path {path:?}"));
    }
    let parsed = Path::new(path);
    for component in parsed.components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err(format!("invalid archive path {path:?}")),
        }
    }
    Ok(())
}

pub fn approved_restore_prefixes(include_secrets: bool) -> Vec<&'static str> {
    let mut prefixes = vec![
        "data/foldops/foldops.db",
        "data/config/foldops/",
        "data/config/tools/",
        "data/config/provision/",
        "data/provision/enrollments/",
    ];
    if include_secrets {
        prefixes.push("data/foldops/tls/");
    }
    prefixes
}

pub fn validate_restore_target(path: &str, include_secrets: bool) -> Result<(), String> {
    validate_archive_path(path)?;
    if path.contains("data/registry/images/") {
        return Err(format!("restore path {path:?} is not allowed"));
    }
    let allowed = approved_restore_prefixes(include_secrets)
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(prefix));
    if !allowed {
        return Err(format!("restore path {path:?} is not in the approved export scope"));
    }
    Ok(())
}

pub fn hash_file(path: &Path) -> Result<(String, u64), String> {
    let mut file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    let mut size = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok((format!("{:x}", hasher.finalize()), size))
}

pub fn write_tar_zst_archive(
    output_path: &Path,
    manifest: &RecoveryManifest,
    files: &[StagedBundleFile],
    db_backup: Option<&Path>,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temp_path = output_path.with_extension(format!("partial-{}", std::process::id()));
    {
        let output = File::create(&temp_path).map_err(|error| error.to_string())?;
        let encoder = zstd::Encoder::new(output, 3).map_err(|error| error.to_string())?;
        let mut tar = Builder::new(encoder);
        let manifest_bytes =
            serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "manifest.json", manifest_bytes.as_slice())
            .map_err(|error| error.to_string())?;

        for file in files {
            let source = resolve_file_source(file, db_backup)?;
            tar.append_path_with_name(source, &file.archive_path)
                .map_err(|error| format!("archive {}: {error}", file.archive_path))?;
        }
        let encoder = tar.into_inner().map_err(|error| error.to_string())?;
        encoder.finish().map_err(|error| error.to_string())?;
    }
    fs::rename(&temp_path, output_path).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn resolve_export_source<'a>(
    file: &'a StagedBundleFile,
    db_backup: Option<&'a Path>,
) -> Result<&'a Path, String> {
    if db_backup.is_some()
        && file.source_path.file_name().is_some_and(|name| name == "foldops.db")
    {
        return Ok(db_backup.expect("db backup path"));
    }
    Ok(&file.source_path)
}

fn resolve_file_source<'a>(file: &'a StagedBundleFile, db_backup: Option<&'a Path>) -> Result<&'a Path, String> {
    resolve_export_source(file, db_backup)
}

pub fn read_os_release_version() -> String {
    let Ok(content) = fs::read_to_string("/usr/lib/os-release") else {
        return String::new();
    };
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("VERSION_ID=") {
            return value.trim_matches('"').to_string();
        }
    }
    String::new()
}

pub fn export_timestamp_rfc3339_utc(unix: i64) -> String {
    let secs = unix.rem_euclid(86_400);
    let days = unix.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs / 3600;
    let minute = (secs % 3600) / 60;
    let second = secs % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

pub fn export_timestamp_compact(unix: i64) -> String {
    let secs = unix.rem_euclid(86_400);
    let days = unix.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs / 3600;
    let minute = (secs % 3600) / 60;
    let second = secs % 60;
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m, d)
}

pub fn sanitize_hostname_for_filename(hostname: &str) -> String {
    hostname
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_archive_path_rejects_traversal() {
        assert!(validate_archive_path("data/foldops/foldops.db").is_ok());
        assert!(validate_restore_target("../etc/passwd", false).is_err());
    }

    #[test]
    fn validate_restore_target_rejects_registry_images() {
        assert!(validate_restore_target("data/registry/images/foo.img", false).is_err());
    }
}
