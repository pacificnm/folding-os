use std::fs::{self, File};
use std::io::{copy, Read};
use std::path::{Component, Path, PathBuf};

use tar::Archive;
use xz2::read::XzDecoder;

pub const AR_MAGIC: &[u8] = b"!<arch>\n";

pub fn extract_fah_deb_data(deb_path: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("create extraction directory: {error}"))?;

    let mut file = File::open(deb_path).map_err(|error| format!("open deb artifact: {error}"))?;
    let mut magic = [0_u8; AR_MAGIC.len()];
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
        if member_name == "data.tar.xz" {
            extract_fah_data_tar_xz(&mut limited, destination)?;
        }
        copy(&mut limited, &mut std::io::sink())
            .map_err(|error| format!("consume deb member {member_name:?}: {error}"))?;
        if header.size % 2 == 1 {
            let mut padding = [0_u8; 1];
            file.read_exact(&mut padding)
                .map_err(|error| format!("read deb member padding: {error}"))?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ArMemberHeader {
    name: String,
    size: u64,
}

fn read_ar_member_header(reader: &mut impl Read) -> Result<ArMemberHeader, String> {
    let mut raw = [0_u8; 60];
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
    let size_text = String::from_utf8_lossy(&raw[48..58]);
    let size = size_text
        .trim()
        .parse::<u64>()
        .map_err(|_| "deb archive member size is invalid".to_string())?;
    let name = String::from_utf8_lossy(&raw[0..16]).trim().to_string();
    Ok(ArMemberHeader { name, size })
}

fn normalize_ar_member_name(name: &str) -> String {
    let name = name.trim().trim_end_matches('\0');
    name.split('/').next().unwrap_or(name).to_string()
}

fn extract_fah_data_tar_xz(reader: impl Read, destination: &Path) -> Result<(), String> {
    let xz_reader = XzDecoder::new(reader);
    let mut archive = Archive::new(xz_reader);
    for entry in archive
        .entries()
        .map_err(|error| format!("read data archive entry: {error}"))?
    {
        let mut entry = entry.map_err(|error| format!("read data archive entry: {error}"))?;
        let header = entry.header().clone();
        let path = header
            .path()
            .map_err(|error| format!("read data archive entry: {error}"))?
            .into_owned();
        extract_fah_tar_entry(destination, &path, &header, &mut entry)?;
    }
    Ok(())
}

fn extract_fah_tar_entry(
    destination: &Path,
    path: &Path,
    header: &tar::Header,
    reader: &mut impl Read,
) -> Result<(), String> {
    let relative = sanitize_fah_tar_path(path)?;
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
            path.display()
        ));
    }

    match header.entry_type() {
        tar::EntryType::Directory => {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
            Ok(())
        }
        tar::EntryType::Regular => {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!("create parent directory for {:?}: {error}", relative)
                })?;
            }
            let mut mode = header.mode().unwrap_or(0) & 0o777;
            if mode == 0 {
                mode = 0o644;
            }
            let mut file = File::create(&target)
                .map_err(|error| format!("create archive file {:?}: {error}", relative))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&target, fs::Permissions::from_mode(mode))
                    .map_err(|error| format!("create archive file {:?}: {error}", relative))?;
            }
            let size = header.size().unwrap_or(0);
            let written = copy(&mut reader.take(size + 1), &mut file)
                .map_err(|error| format!("write archive file {:?}: {error}", relative))?;
            file.sync_all()
                .map_err(|error| format!("close archive file {:?}: {error}", relative))?;
            if written != size {
                return Err(format!(
                    "archive file {:?} size {written} does not match header size {size}",
                    relative
                ));
            }
            Ok(())
        }
        other => Err(format!(
            "unsupported archive entry type {:?} for {:?}",
            other,
            path.display()
        )),
    }
}

fn sanitize_fah_tar_path(name: &Path) -> Result<PathBuf, String> {
    use std::path::PathBuf;
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

fn clean_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

fn path_with_trailing_sep(path: &Path) -> PathBuf {
    let mut out = path.to_path_buf();
    if !out.as_os_str().is_empty() {
        out.push("");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_ar_member_header_from_bytes() {
        let mut raw = vec![0_u8; 60];
        raw[..16].copy_from_slice(b"debian-binary   ");
        raw[48..58].copy_from_slice(b"4         ");
        raw[58] = b'`';
        raw[59] = b'\n';
        let header = read_ar_member_header(&mut Cursor::new(raw)).expect("header");
        assert_eq!(header.name, "debian-binary");
        assert_eq!(header.size, 4);
    }

    #[test]
    fn reject_invalid_ar_member_header() {
        let raw = vec![0_u8; 60];
        assert!(read_ar_member_header(&mut Cursor::new(raw)).is_err());
    }

    #[test]
    fn reject_fah_tar_path_traversal() {
        assert!(sanitize_fah_tar_path(std::path::Path::new("./../escape.txt")).is_err());
        assert!(sanitize_fah_tar_path(std::path::Path::new("../escape.txt")).is_err());
        assert!(sanitize_fah_tar_path(std::path::Path::new("/etc/passwd")).is_err());
    }
}
