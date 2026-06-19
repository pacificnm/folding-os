use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::Path;

#[cfg(unix)]
const FOLDINGOSCTL_INSTALL_MODE: u32 = 0o4755;

pub fn verify_tools_executable_elf(path: &Path) -> Result<(), String> {
    let header =
        fs::read(path).map_err(|error| format!("read tools executable ELF header: {error}"))?;
    if header.len() < 20 {
        return Err("read tools executable ELF header: file too short".into());
    }
    if header[0..4] != [0x7f, b'E', b'L', b'F'] {
        return Err("read tools executable ELF header: invalid magic".into());
    }
    let elf_type = u16::from_le_bytes([header[16], header[17]]);
    if elf_type != 2 && elf_type != 3 {
        return Err(format!("tools executable type {elf_type} is not supported"));
    }
    let machine = u16::from_le_bytes([header[18], header[19]]);
    if machine != 62 {
        return Err(format!(
            "tools executable architecture {machine} is not x86_64"
        ));
    }
    Ok(())
}

/// Ensure `/usr/bin/foldingosctl` keeps root ownership and the setuid bit required
/// for the foldops automation user to re-elevate during acquire.
pub fn restore_setuid_install_mode(destination: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use nix::sys::stat::{fchmod, Mode};
        use nix::unistd::{chown, geteuid, Gid, Uid};

        if geteuid() != Uid::from_raw(0) {
            return Ok(());
        }

        chown(destination, Some(Uid::from_raw(0)), Some(Gid::from_raw(0)))
            .map_err(|error| format!("restore tools binary ownership: {error}"))?;

        let file = OpenOptions::new()
            .read(true)
            .open(destination)
            .map_err(|error| format!("open tools binary for chmod: {error}"))?;
        fchmod(
            file.as_raw_fd(),
            Mode::from_bits_truncate(FOLDINGOSCTL_INSTALL_MODE),
        )
        .map_err(|error| format!("restore setuid tools binary mode: {error}"))?;

        if !setuid_bit_set(destination)? {
            let path = destination
                .to_str()
                .ok_or_else(|| "tools binary path is not valid UTF-8".to_string())?;
            crate::process::command_output("chmod", &["4755", path])
                .map_err(|error| format!("chmod tools binary: {error}"))?;
        }

        if !setuid_bit_set(destination)? {
            return Err(format!(
                "tools binary at {} is missing the setuid bit after install",
                destination.display()
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn setuid_bit_set(path: &Path) -> Result<bool, String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .map_err(|error| format!("inspect tools binary mode: {error}"))?
        .permissions()
        .mode();
    Ok(mode & 0o4000 != 0)
}

pub fn atomic_replace_tools_binary(staged_path: &Path, destination: &Path) -> Result<(), String> {
    verify_tools_executable_elf(staged_path)?;

    let metadata = fs::metadata(staged_path)
        .map_err(|error| format!("inspect staged tools binary: {error}"))?;
    if !metadata.is_file() {
        return Err("staged tools artifact is not a regular file".into());
    }

    let parent = destination
        .parent()
        .ok_or_else(|| "tools binary destination has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create tools binary directory: {error}"))?;

    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("foldingosctl");
    let temp_path = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let _ = fs::remove_file(&temp_path);

    let content =
        fs::read(staged_path).map_err(|error| format!("read staged tools binary: {error}"))?;
    {
        let mut temp = File::create(&temp_path)
            .map_err(|error| format!("create temporary tools binary: {error}"))?;
        temp.write_all(&content)
            .map_err(|error| format!("write temporary tools binary: {error}"))?;
        temp.sync_all()
            .map_err(|error| format!("sync temporary tools binary: {error}"))?;
        temp.flush()
            .map_err(|error| format!("flush temporary tools binary: {error}"))?;
    }

    fs::rename(&temp_path, destination)
        .map_err(|error| format!("replace tools binary: {error}"))?;
    restore_setuid_install_mode(destination)?;

    let dir = OpenOptions::new()
        .read(true)
        .open(parent)
        .map_err(|error| error.to_string())?;
    dir.sync_all()
        .map_err(|error| format!("sync {}: {error}", parent.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_replace_tools_binary_replaces_destination() {
        let root =
            std::env::temp_dir().join(format!("foldingosctl-tools-replace-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let staged_path = root.join("staged");
        let destination = root.join("bin").join("foldingosctl");
        let artifact = [
            0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 62, 0,
        ];
        fs::write(&staged_path, artifact).unwrap();
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(&destination, b"old-binary").unwrap();

        atomic_replace_tools_binary(&staged_path, &destination).unwrap();
        let content = fs::read(&destination).unwrap();
        assert_eq!(content, artifact);

        let _ = fs::remove_dir_all(root);
    }
}
