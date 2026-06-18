use std::fs::{self, File};
use std::io::{copy, Read};
use std::path::{Component, Path, PathBuf};

use crate::foldops::util::{clean_path, path_with_trailing_sep};
use tar::Archive;
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

const AR_MAGIC: &[u8] = b"!<arch>\n";

pub fn extract_foldops_deb_data(deb_path: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("create extraction directory: {error}"))?;
    let mut file = File::open(deb_path).map_err(|error| format!("open deb artifact: {error}"))?;
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)
        .map_err(|error| format!("read deb archive header: {error}"))?;
    if magic != AR_MAGIC {
        return Err("deb artifact is not a valid ar archive".into());
    }

    loop {
        let header = match read_ar_member_header(&mut file) {
            Ok(header) => header,
            Err(error) if error == "eof" => break,
            Err(error) => return Err(error),
        };
        let member_name = normalize_ar_member_name(&header.name);
        let mut limited = (&mut file).take(header.size);
        match member_name.as_str() {
            "data.tar.xz" => extract_foldops_data_tar_xz(&mut limited, destination)?,
            "data.tar.zst" => extract_foldops_data_tar_zst(&mut limited, destination)?,
            _ => {}
        }
        copy(&mut limited, &mut std::io::sink())
            .map_err(|error| format!("consume deb member \"{member_name}\": {error}"))?;
        if header.size % 2 == 1 {
            let mut padding = [0u8; 1];
            file.read_exact(&mut padding)
                .map_err(|error| format!("read deb member padding: {error}"))?;
        }
    }
    Ok(())
}

pub fn extract_foldops_layout_bundle(
    bundle_path: &Path,
    staging_root: &Path,
    install_prefix: &str,
) -> Result<(), String> {
    let install_prefix = install_prefix.trim();
    if install_prefix.is_empty()
        || install_prefix.contains("..")
        || install_prefix.contains('/')
        || install_prefix.contains('\\')
    {
        return Err("install_prefix is invalid".into());
    }
    let prefix = format!("{install_prefix}/");
    let file = File::open(bundle_path).map_err(|error| format!("open layout bundle: {error}"))?;
    let decoder = ZstdDecoder::new(file)
        .map_err(|error| format!("open layout bundle zstd stream: {error}"))?;
    let mut archive = Archive::new(decoder);
    for entry in archive
        .entries()
        .map_err(|error| format!("read layout bundle entry: {error}"))?
    {
        let mut entry = entry.map_err(|error| format!("read layout bundle entry: {error}"))?;
        let header = entry.header().clone();
        let path = header.path().map_err(|error| error.to_string())?;
        let name = path.to_string_lossy();
        let name = name.strip_prefix("./").unwrap_or(&name);
        if name.is_empty() || name == install_prefix {
            continue;
        }
        if name != install_prefix && !name.starts_with(&prefix) {
            return Err(format!(
                "layout bundle entry \"{}\" is outside install_prefix \"{install_prefix}\"",
                header.path().map_err(|error| error.to_string())?.display()
            ));
        }
        extract_tar_entry(staging_root, &header, &mut entry)?;
    }
    Ok(())
}

fn extract_foldops_data_tar_xz(reader: &mut dyn Read, destination: &Path) -> Result<(), String> {
    let xz_reader = XzDecoder::new(reader);
    extract_foldops_tar_archive(xz_reader, destination)
}

fn extract_foldops_data_tar_zst(reader: &mut dyn Read, destination: &Path) -> Result<(), String> {
    let zstd_reader = ZstdDecoder::new(reader)
        .map_err(|error| format!("open data.tar.zst stream: {error}"))?;
    extract_foldops_tar_archive(zstd_reader, destination)
}

fn extract_foldops_tar_archive<R: Read>(reader: R, destination: &Path) -> Result<(), String> {
    let mut archive = Archive::new(reader);
    for entry in archive
        .entries()
        .map_err(|error| format!("read data archive entry: {error}"))?
    {
        let mut entry = entry.map_err(|error| format!("read data archive entry: {error}"))?;
        let header = entry.header().clone();
        extract_tar_entry(destination, &header, &mut entry)?;
    }
    Ok(())
}

fn extract_tar_entry(
    destination: &Path,
    header: &tar::Header,
    reader: &mut tar::Entry<'_, impl Read>,
) -> Result<(), String> {
    let relative = sanitize_tar_path(&header.path().map_err(|error| error.to_string())?)?;
    if relative.as_os_str().is_empty() {
        return Ok(());
    }
    let target = destination.join(&relative);
    let destination_root = clean_path(destination);
    let cleaned = clean_path(&target);
    if cleaned != destination_root
        && !cleaned.starts_with(&path_with_trailing_sep(&destination_root))
    {
        return Err(format!(
            "archive entry escapes staging directory: {:?}",
            header.path()
        ));
    }

    match header.entry_type() {
        tar::EntryType::Directory => {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        }
        tar::EntryType::Regular | tar::EntryType::GNUSparse => {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let mode = header.mode().unwrap_or(0o644) & 0o777;
            let mode = if mode == 0 { 0o644 } else { mode };
            let mut file = File::create(&target).map_err(|error| error.to_string())?;
            copy(reader, &mut file)
                .map_err(|error| format!("write archive file {:?}: {error}", relative))?;
            file.sync_all().ok();
            fs::set_permissions(&target, fs::Permissions::from_mode(mode)).ok();
        }
        other => {
            return Err(format!(
                "unsupported archive entry type {:?} for {:?}",
                other,
                header.path()
            ));
        }
    }
    Ok(())
}

fn sanitize_tar_path(name: &Path) -> Result<PathBuf, String> {
    let name = name.to_string_lossy().trim().to_string();
    if name.is_empty() || name == "." || name == "./" {
        return Ok(PathBuf::new());
    }
    let name = name.strip_prefix("./").unwrap_or(&name);
    if name.starts_with('/') {
        return Err(format!("archive entry uses an absolute path: {name:?}"));
    }
    let path = Path::new(name);
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(format!("archive entry contains path traversal: {name:?}"));
            }
            Component::Normal(part) if part == ".." => {
                return Err(format!("archive entry contains path traversal: {name:?}"));
            }
            _ => {}
        }
    }
    let cleaned = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part),
            _ => None,
        })
        .collect::<PathBuf>();
    if cleaned.as_os_str().is_empty() {
        return Ok(PathBuf::new());
    }
    Ok(cleaned)
}

use sha2::{Digest, Sha256};

struct ArMemberHeader {
    name: String,
    size: u64,
}

fn read_ar_member_header(reader: &mut impl Read) -> Result<ArMemberHeader, String> {
    let mut raw = [0u8; 60];
    match reader.read_exact(&mut raw) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err("eof".into());
        }
        Err(error) => return Err(format!("read deb member header: {error}")),
    }
    if raw[58..60] != [b'`', b'\n'] {
        return Err("deb archive member header is invalid".into());
    }
    let size = std::str::from_utf8(&raw[48..58])
        .map_err(|error| error.to_string())?
        .trim()
        .parse::<u64>()
        .map_err(|_| "deb archive member size is invalid".to_string())?;
    Ok(ArMemberHeader {
        name: String::from_utf8_lossy(&raw[0..16]).trim().to_string(),
        size,
    })
}

fn normalize_ar_member_name(name: &str) -> String {
    let name = name.trim().trim_end_matches('\0');
    name.split('/').next().unwrap_or(name).to_string()
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_tar_path_accepts_relative_prefix() {
        let path = sanitize_tar_path(Path::new("./etc/ssl/certs")).expect("path");
        assert_eq!(path, Path::new("etc/ssl/certs"));
    }

    #[test]
    fn reject_tar_path_traversal() {
        assert!(sanitize_tar_path(Path::new("./../escape.txt")).is_err());
        assert!(sanitize_tar_path(Path::new("/etc/passwd")).is_err());
    }
}

pub fn verify_foldops_artifact_file(path: &Path, pkg: &crate::foldops_manifest::FoldOpsPackage) -> Result<(), String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let written = copy(&mut file.take((pkg.artifact_size + 1) as u64), &mut hasher)
        .map_err(|error| format!("hash artifact: {error}"))?;
    if written != pkg.artifact_size as u64 {
        return Err(format!(
            "{} artifact size {written} does not match expected size {}",
            pkg.name, pkg.artifact_size
        ));
    }
    let digest = format!("{:x}", hasher.finalize());
    if digest != pkg.sha256 {
        return Err(format!(
            "{} artifact SHA-256 digest does not match approved manifest",
            pkg.name
        ));
    }
    Ok(())
}
