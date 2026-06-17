use std::fs;
use std::path::Path;

use crate::foldops::activate::foldops_verification_target_at_root;
use crate::foldops::util::{FOLDOPS_VERIFIED_MARKER, parse_key_value_lines};
use crate::foldops_manifest::FoldOpsPackage;
use crate::paths::AppliancePaths;

const SHARED_LIBRARY_SEARCH_PATHS: &[&str] = &[
    "/lib/x86_64-linux-gnu",
    "/usr/lib/x86_64-linux-gnu",
    "/lib64",
    "/usr/lib64",
    "/lib",
    "/usr/lib",
];
const REQUIRED_INTERPRETER: &str = "/lib64/ld-linux-x86-64.so.2";

pub fn foldops_installation_verified(
    paths: &AppliancePaths,
    release: &str,
    role: &str,
    packages: &[FoldOpsPackage],
) -> Result<bool, String> {
    let marker_path = paths.foldops_apps_root.join(release).join(FOLDOPS_VERIFIED_MARKER);
    let content = match fs::read_to_string(&marker_path) {
        Ok(content) => content,
        Err(_) => return Ok(false),
    };
    let values = parse_key_value_lines(&content);
    if values.get("manifest_release").map(String::as_str) != Some(release) {
        return Ok(false);
    }
    if values.get("installation_role").map(String::as_str) != Some(role) {
        return Ok(false);
    }
    for pkg in packages {
        let key = format!("package_{}_sha256", pkg.name);
        if values.get(&key).map(String::as_str) != Some(pkg.sha256.as_str()) {
            return Ok(false);
        }
    }
    let release_root = paths.foldops_apps_root.join(release);
    for pkg in packages {
        if verify_foldops_package_tree_at_root(&release_root, pkg).is_err() {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn verify_foldops_package_tree_at_root(
    release_root: &Path,
    pkg: &FoldOpsPackage,
) -> Result<(), String> {
    let target = foldops_verification_target_at_root(release_root, &pkg.verification_path)?;
    let metadata = fs::metadata(&target)
        .map_err(|error| format!("{} verification path is missing: {error}", pkg.name))?;
    if metadata.is_dir() {
        return Err(format!("{} verification path must be a file", pkg.name));
    }
    if target.to_string_lossy().ends_with(".html")
        || target.to_string_lossy().contains("/web/")
    {
        return Ok(());
    }
    verify_executable_elf(&target)
}

fn verify_executable_elf(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| format!("read executable ELF header: {error}"))?;
    let elf = goblin::elf::Elf::parse(&bytes)
        .map_err(|error| format!("read executable ELF header: {error}"))?;
    if elf.header.e_machine != goblin::elf::header::EM_X86_64 {
        return Err(format!(
            "executable architecture {:?} is not x86_64",
            elf.header.e_machine
        ));
    }
    if elf.header.e_type != goblin::elf::header::ET_DYN {
        return Err(format!(
            "executable type {:?} is not supported",
            elf.header.e_type
        ));
    }
    let interp = elf
        .section_headers
        .iter()
        .find(|section| {
            elf.shdr_strtab
                .get_at(section.sh_name)
                .map(|name| name == ".interp")
                .unwrap_or(false)
        })
        .and_then(|section| {
            let start = section.sh_offset as usize;
            let end = start + section.sh_size as usize;
            bytes.get(start..end)
        })
        .ok_or_else(|| "executable is missing the ELF interpreter section".to_string())?;
    let interp_value = String::from_utf8_lossy(interp)
        .trim_end_matches('\0')
        .to_string();
    if interp_value != REQUIRED_INTERPRETER {
        return Err(format!(
            "executable interpreter \"{interp_value}\" is not supported"
        ));
    }
    for library in elf.libraries {
        if !shared_library_exists(library) {
            return Err(format!("required shared library is unavailable: {library}"));
        }
    }
    Ok(())
}

fn shared_library_exists(name: &str) -> bool {
    SHARED_LIBRARY_SEARCH_PATHS
        .iter()
        .any(|directory| Path::new(directory).join(name).exists())
}

pub fn normalize_install_tree(root: &Path) -> Result<(), String> {
    for entry in walkdir_paths(root)? {
        let metadata = fs::symlink_metadata(&entry).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!("symlinks are not permitted: {}", entry.display()));
        }
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(format!("unsupported file type: {}", entry.display()));
        }
        #[cfg(unix)]
        if nix::unistd::geteuid().is_root() {
            nix::unistd::chown(&entry, Some(nix::unistd::Uid::from_raw(0)), Some(nix::unistd::Gid::from_raw(0)))
                .map_err(|error| format!("normalize ownership for {}: {error}", entry.display()))?;
        }
        let mode = if metadata.is_dir() {
            0o755
        } else if metadata.permissions().mode() & 0o111 != 0 {
            0o755
        } else {
            0o644
        };
        fs::set_permissions(&entry, fs::Permissions::from_mode(mode))
            .map_err(|error| format!("normalize permissions for {}: {error}", entry.display()))?;
    }
    Ok(())
}

fn walkdir_paths(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(path) = pending.pop() {
        paths.push(path.clone());
        if path.is_dir() {
            for entry in fs::read_dir(&path).map_err(|error| error.to_string())? {
                let entry = entry.map_err(|error| error.to_string())?;
                pending.push(entry.path());
            }
        }
    }
    Ok(paths)
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
