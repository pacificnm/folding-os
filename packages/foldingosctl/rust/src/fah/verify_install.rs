use std::fs::{self, File};
use std::io::{copy, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use goblin::elf::Elf;
use sha2::{Digest, Sha256};

use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;

use super::extract::extract_fah_deb_data;
use super::manifest::{
    load_fah_manifest, validate_fah_version_label, validate_foldingos_compatibility, FahManifest,
};
use super::util::{
    fah_executable_in_root, is_root_owned, parse_key_value_lines, remove_fah_path,
    require_fah_root_ownership,
};

const FAH_SHARED_LIBRARY_SEARCH_PATHS: &[&str] = &[
    "/lib/x86_64-linux-gnu",
    "/usr/lib/x86_64-linux-gnu",
    "/lib64",
    "/usr/lib64",
    "/lib",
    "/usr/lib",
];
const FAH_REQUIRED_ELF_MACHINE: u16 = goblin::elf::header::EM_X86_64;
const FAH_REQUIRED_ELF_TYPE: u16 = goblin::elf::header::ET_DYN;
const FAH_REQUIRED_INTERPRETER: &str = "/lib64/ld-linux-x86-64.so.2";

pub fn fah_verify_install(paths: &AppliancePaths, version: &str) -> Result<(), String> {
    validate_fah_version_label(version)?;
    let manifest = load_fah_manifest(paths, &paths.fah_embedded_manifest)?;
    validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;
    if version != manifest.client_version {
        return Err(format!(
            "version {version} does not match approved manifest client {}",
            manifest.client_version
        ));
    }
    verify_fah_installed_version(paths, version, &manifest)?;
    write_fah_verified_marker(paths, version, &manifest)?;
    println!(
        "Verified Folding@home {version} installation at {}.",
        paths.fah_version_dir(version).display()
    );
    Ok(())
}

pub fn extract_and_install_fah_artifact(
    paths: &AppliancePaths,
    manifest: &FahManifest,
) -> Result<PathBuf, String> {
    let version = &manifest.client_version;
    let staging_dir = paths.fah_staging_dir(version);
    let version_dir = paths.fah_version_dir(version);
    let staged_deb = paths.fah_staged_deb(version);

    if fah_installation_verified(paths, version, manifest) {
        verify_fah_installed_version(paths, version, manifest)?;
        write_fah_verified_marker(paths, version, manifest)?;
        return Ok(version_dir);
    }

    if staging_dir.exists() {
        remove_fah_path(&staging_dir)?;
    }
    match fs::metadata(&version_dir) {
        Ok(metadata) if metadata.is_dir() => remove_fah_path(&version_dir)?,
        Ok(_) => {
            return Err(format!(
                "{} exists but is not a directory",
                version_dir.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("inspect existing version directory: {error}")),
    }

    if fs::metadata(&staged_deb).is_err() {
        return Err(format!("staged deb artifact is missing: {}", staged_deb.display()));
    }
    if let Err(error) = extract_fah_deb_data(&staged_deb, &staging_dir) {
        let _ = remove_fah_path(&staging_dir);
        return Err(error);
    }
    if let Err(error) = normalize_fah_install_tree(&staging_dir) {
        let _ = remove_fah_path(&staging_dir);
        return Err(error);
    }
    if let Err(error) = verify_fah_install_tree(&staging_dir, manifest) {
        let _ = remove_fah_path(&staging_dir);
        return Err(error);
    }
    if let Err(error) = fs::rename(&staging_dir, &version_dir) {
        let _ = remove_fah_path(&staging_dir);
        return Err(format!("promote verified installation: {error}"));
    }
    if let Err(error) = verify_fah_installed_version(paths, version, manifest) {
        let _ = remove_fah_path(&version_dir);
        return Err(error);
    }
    if let Err(error) = write_fah_verified_marker(paths, version, manifest) {
        let _ = remove_fah_path(&version_dir);
        return Err(error);
    }
    Ok(version_dir)
}

pub fn verify_fah_installed_version(
    paths: &AppliancePaths,
    version: &str,
    manifest: &FahManifest,
) -> Result<(), String> {
    let version_dir = paths.fah_version_dir(version);
    let metadata = fs::metadata(&version_dir)
        .map_err(|error| format!("installed version directory is missing: {error}"))?;
    if !metadata.is_dir() {
        return Err("installed version path is not a directory".into());
    }
    verify_fah_install_tree(&version_dir, manifest)
}

pub fn fah_installation_verified(
    paths: &AppliancePaths,
    version: &str,
    manifest: &FahManifest,
) -> bool {
    let marker_path = paths.fah_verified_marker(version);
    let Ok(content) = fs::read_to_string(marker_path) else {
        return false;
    };
    let values = parse_key_value_lines(&content);
    if values.get("client_version") != Some(&manifest.client_version) {
        return false;
    }
    if values.get("artifact_sha256") != Some(&manifest.sha256) {
        return false;
    }
    let Ok(executable) = fah_executable_in_root(
        &paths.fah_version_dir(version),
        &manifest.executable_path,
    ) else {
        return false;
    };
    fs::metadata(executable)
        .map(|meta| meta.is_file())
        .unwrap_or(false)
}

fn verify_fah_install_tree(root: &Path, manifest: &FahManifest) -> Result<(), String> {
    let executable = fah_executable_in_root(root, &manifest.executable_path)?;
    verify_fah_install_layout(root, &executable)?;
    verify_fah_executable_elf(&executable)
}

fn verify_fah_install_layout(root: &Path, executable: &Path) -> Result<(), String> {
    let executable_clean = clean_path(executable);
    let root_clean = clean_path(root);
    if executable_clean != root_clean
        && !executable_clean.starts_with(&path_with_trailing_sep(&root_clean))
    {
        return Err("manifest executable is outside the installation directory".into());
    }
    if let Err(error) = fs::metadata(&executable_clean) {
        return Err(format!("required executable is missing: {error}"));
    }
    walk_install_tree(&root_clean, &root_clean)
}

fn walk_install_tree(root_clean: &Path, current: &Path) -> Result<(), String> {
    let metadata = fs::metadata(current).map_err(|error| error.to_string())?;
    let relative = current
        .strip_prefix(root_clean)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());
    if relative != "." && relative != "" {
        if metadata.file_type().is_symlink() {
            return Err(format!("symlinks are not permitted: {relative}"));
        }
        if metadata.mode() & (0o4000 | 0o2000 | 0o1000) != 0 {
            return Err(format!("special permission bits are not permitted: {relative}"));
        }
        if require_fah_root_ownership() && !is_root_owned(&metadata) {
            return Err(format!("installed file is not owned by root:root: {relative}"));
        }
        let perm = metadata.mode() & 0o777;
        if perm & 0o002 != 0 {
            return Err(format!("world-writable permissions are not permitted: {relative}"));
        }
        if metadata.is_dir() {
            if perm != 0o755 {
                return Err(format!(
                    "directory permissions are unsafe: {relative} ({perm:04o})"
                ));
            }
        } else if metadata.is_file() {
            if perm & 0o111 != 0 {
                if perm != 0o755 {
                    return Err(format!(
                        "executable permissions are unsafe: {relative} ({perm:04o})"
                    ));
                }
            } else if perm != 0o644 {
                return Err(format!("file permissions are unsafe: {relative} ({perm:04o})"));
            }
        } else {
            return Err(format!("unsupported file type: {relative}"));
        }
    }

    if metadata.is_dir() {
        for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            walk_install_tree(root_clean, &entry.path())?;
        }
    }
    Ok(())
}

fn normalize_fah_install_tree(root: &Path) -> Result<(), String> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!("symlinks are not permitted: {}", path.display()));
        }
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(format!("unsupported file type: {}", path.display()));
        }
        if require_fah_root_ownership() {
            nix::unistd::chown(&path, Some(nix::unistd::Uid::from_raw(0)), Some(nix::unistd::Gid::from_raw(0)))
                .map_err(|error| format!("normalize ownership for {}: {error}", path.display()))?;
        }
        let mut mode = metadata.mode() & !0o7000;
        mode = if metadata.is_dir() {
            0o755
        } else if mode & 0o111 != 0 {
            0o755
        } else {
            0o644
        };
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))
            .map_err(|error| format!("normalize permissions for {}: {error}", path.display()))?;
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).map_err(|error| error.to_string())? {
                stack.push(entry.map_err(|error| error.to_string())?.path());
            }
        }
    }
    Ok(())
}

fn verify_fah_executable_elf(path: &Path) -> Result<(), String> {
    let data = fs::read(path).map_err(|error| format!("read executable ELF header: {error}"))?;
    let elf = Elf::parse(&data).map_err(|error| format!("read executable ELF header: {error}"))?;
    if elf.header.e_machine != FAH_REQUIRED_ELF_MACHINE {
        return Err(format!(
            "executable architecture {} is not x86_64",
            elf.header.e_machine
        ));
    }
    if elf.header.e_type != FAH_REQUIRED_ELF_TYPE {
        return Err(format!(
            "executable type {} is not supported",
            elf.header.e_type
        ));
    }
    let interp = elf
        .interpreter
        .ok_or_else(|| "executable is missing the ELF interpreter section".to_string())?;
    let interp_value = interp.trim_end_matches('\0');
    if interp_value != FAH_REQUIRED_INTERPRETER {
        return Err(format!(
            "executable interpreter {interp_value:?} is not supported"
        ));
    }
    for library in &elf.libraries {
        if !fah_shared_library_exists(library) {
            return Err(format!("required shared library is unavailable: {library}"));
        }
    }
    Ok(())
}

fn fah_shared_library_exists(name: &str) -> bool {
    FAH_SHARED_LIBRARY_SEARCH_PATHS
        .iter()
        .any(|directory| Path::new(directory).join(name).exists())
}

fn write_fah_verified_marker(
    paths: &AppliancePaths,
    version: &str,
    manifest: &FahManifest,
) -> Result<(), String> {
    let marker_path = paths.fah_verified_marker(version);
    let content = format!(
        "client_version={}\nartifact_sha256={}\n",
        manifest.client_version, manifest.sha256
    );
    atomic_write(&marker_path, content.as_bytes(), 0o644)
}

fn clean_path(path: &Path) -> PathBuf {
    use std::path::Component;
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

pub fn verify_fah_artifact_file(path: &Path, manifest: &FahManifest) -> Result<(), String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut reader = file.take((manifest.artifact_size + 1) as u64);
    let written = copy(&mut reader, &mut hasher)
    .map_err(|error| format!("hash artifact: {error}"))?;
    if written != manifest.artifact_size as u64 {
        return Err(format!(
            "artifact size {written} does not match expected size {}",
            manifest.artifact_size
        ));
    }
    let digest = hex::encode(hasher.finalize());
    if digest != manifest.sha256 {
        return Err("artifact SHA-256 digest does not match approved manifest".into());
    }
    Ok(())
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}
