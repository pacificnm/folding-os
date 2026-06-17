use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::identity::read_installed_foldingos_version;
use crate::paths::AppliancePaths;
use crate::process::write_console;
use crate::registry_image::{
    current_import_timestamp, read_embedded_build_revision, registry_image_path,
    save_registry_entry, verify_registry_image_file, RegistryEntry, RELEASE_IMAGE_SIZE_BYTES,
};
use crate::role::require_supervisor_role;
use crate::storage::resolve_boot_disk;

const COPY_PROGRESS_INTERVAL: i64 = 256 * 1024 * 1024;
const COPY_CHUNK_SIZE: usize = 4 * 1024 * 1024;

pub fn import_bootstrap(paths: &AppliancePaths) -> Result<(), String> {
    require_supervisor_role(paths)?;

    let version = read_installed_foldingos_version()?;
    match crate::registry_image::load_registry_entry(paths, &version) {
        Ok(existing) => {
            verify_registry_image_file(
                Path::new(&existing.local_image_path),
                &existing.image_sha256,
                existing.image_size_bytes,
            )
            .map_err(|error| {
                format!("existing registry image for {version} is invalid: {error}")
            })?;
            println!("Registry already contains verified image for FoldingOS {version}.");
            return Ok(());
        }
        Err(error) if error.contains("No such file") || error.contains("not found") => {}
        Err(error) => return Err(error),
    }

    let disk = resolve_boot_disk()?;
    let image_path = registry_image_path(paths, &version);
    if let Some(parent) = image_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create registry images dir: {error}"))?;
    }

    report_registry_import_started(&version, &disk);
    let (digest, size) = match copy_boot_disk_image(&disk, &image_path, RELEASE_IMAGE_SIZE_BYTES) {
        Ok(result) => result,
        Err(error) => {
            report_registry_import_failure(&version, &error);
            return Err(error);
        }
    };
    if size != RELEASE_IMAGE_SIZE_BYTES {
        return Err(format!(
            "imported image size {size} does not match release image size {RELEASE_IMAGE_SIZE_BYTES}"
        ));
    }

    let entry = RegistryEntry {
        schema_version: 1,
        foldingos_version: version.clone(),
        git_revision: read_embedded_build_revision(paths),
        image_sha256: digest.clone(),
        image_size_bytes: size,
        retrieval_url: String::new(),
        verification_method: "sha256".into(),
        import_timestamp: current_import_timestamp(),
        rollout_state: "ready".into(),
        local_image_path: image_path.to_string_lossy().into_owned(),
    };
    save_registry_entry(paths, entry)?;
    report_registry_import_complete(&version, &digest);
    println!("Imported FoldingOS {version} into the supervisor registry.");
    println!("Image SHA-256: {digest}");
    Ok(())
}

pub fn copy_boot_disk_image(
    disk: &str,
    destination: &Path,
    size: i64,
) -> Result<(String, i64), String> {
    let mut source = File::open(disk).map_err(|error| error.to_string())?;
    let parent = destination
        .parent()
        .ok_or_else(|| "destination path has no parent".to_string())?;
    let temp_path = tempfile_in_dir(parent, ".registry-image.tmp-")?;
    let mut cleanup = true;
    let result = (|| -> Result<(String, i64), String> {
        let mut temp = File::create(&temp_path).map_err(|error| error.to_string())?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; COPY_CHUNK_SIZE];
        let mut written = 0_i64;
        let mut last_report = 0_i64;
        while written < size {
            let remaining = size - written;
            let chunk_size = (buffer.len() as i64).min(remaining) as usize;
            source
                .read_exact(&mut buffer[..chunk_size])
                .map_err(|error| format!("copy boot disk image: {error}"))?;
            temp.write_all(&buffer[..chunk_size])
                .map_err(|error| error.to_string())?;
            hasher.update(&buffer[..chunk_size]);
            written += chunk_size as i64;
            if written == size || written - last_report >= COPY_PROGRESS_INTERVAL {
                report_registry_copy_progress(written, size);
                last_report = written;
            }
        }
        temp.sync_all().map_err(|error| error.to_string())?;
        drop(temp);
        fs::rename(&temp_path, destination).map_err(|error| error.to_string())?;
        cleanup = false;
        Ok((format!("{:x}", hasher.finalize()), written))
    })();
    if cleanup {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn tempfile_in_dir(parent: &Path, prefix: &str) -> Result<std::path::PathBuf, String> {
    for attempt in 0..100 {
        let candidate = parent.join(format!("{prefix}{attempt}"));
        if candidate.exists() {
            continue;
        }
        return Ok(candidate);
    }
    Err("could not create temporary registry image file".into())
}

fn report_registry_import_started(version: &str, disk: &str) {
    emit_registry_status(&format!(
        "Registry: copying FoldingOS {version} from {disk}"
    ));
}

fn report_registry_import_complete(version: &str, digest: &str) {
    emit_registry_status(&format!(
        "Registry: imported FoldingOS {version} ({digest})"
    ));
}

fn report_registry_import_failure(version: &str, err: &str) {
    emit_registry_status(&format!(
        "Registry: failed to import FoldingOS {version} ({err})"
    ));
}

fn report_registry_copy_progress(written: i64, total: i64) {
    emit_registry_status(&format_registry_copy_progress(written, total));
}

fn format_registry_copy_progress(written: i64, total: i64) -> String {
    let percent = ((written * 100) / total) as i64;
    format!(
        "Registry: copying release image {} MiB / {} MiB ({}%)",
        written / (1024 * 1024),
        total / (1024 * 1024),
        percent
    )
}

fn emit_registry_status(message: &str) {
    println!("{message}");
    let _ = write_console(&format!("{message}\n"));
}
