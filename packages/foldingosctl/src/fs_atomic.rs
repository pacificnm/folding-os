use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

pub fn atomic_write(path: &Path, content: &[u8], mode: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let temp_path = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut file = File::create(&temp_path)
            .map_err(|error| format!("create temp file {}: {error}", temp_path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(mode))
                .map_err(|error| format!("chmod temp file: {error}"))?;
        }
        file.write_all(content)
            .map_err(|error| format!("write temp file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync temp file: {error}"))?;
    }
    fs::rename(&temp_path, path).map_err(|error| format!("rename into place: {error}"))?;
    if let Some(parent) = path.parent() {
        let dir = OpenOptions::new()
            .read(true)
            .open(parent)
            .map_err(|error| format!("open parent dir: {error}"))?;
        dir.sync_all()
            .map_err(|error| format!("sync parent dir: {error}"))?;
    }
    Ok(())
}

pub fn contains_string(values: &[String], target: &str) -> bool {
    values.iter().any(|value| value == target)
}
