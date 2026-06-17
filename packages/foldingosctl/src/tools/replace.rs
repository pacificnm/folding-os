use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

pub fn verify_tools_executable_elf(path: &Path) -> Result<(), String> {
    let header = fs::read(path).map_err(|error| format!("read tools executable ELF header: {error}"))?;
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
        return Err(format!("tools executable architecture {machine} is not x86_64"));
    }
    Ok(())
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

    let content = fs::read(staged_path).map_err(|error| format!("read staged tools binary: {error}"))?;
    {
        let mut temp = File::create(&temp_path)
            .map_err(|error| format!("create temporary tools binary: {error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))
                .map_err(|error| format!("chmod temporary tools binary: {error}"))?;
        }
        temp.write_all(&content)
            .map_err(|error| format!("write temporary tools binary: {error}"))?;
        temp.sync_all()
            .map_err(|error| format!("sync temporary tools binary: {error}"))?;
        temp.flush()
            .map_err(|error| format!("flush temporary tools binary: {error}"))?;
    }

    fs::rename(&temp_path, destination).map_err(|error| format!("replace tools binary: {error}"))?;

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
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-tools-replace-{}",
            std::process::id()
        ));
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
