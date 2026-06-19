use std::fs::{self, File};
use std::io::{Read, Write};

use sha2::{Digest, Sha256};
use crate::automation_policy::require_acquire_automation_mutation;
use crate::foldops::acquire_state::{
    clear_foldops_acquire_state, defer_foldops_acquisition_attempt, record_foldops_acquisition_failure,
};
use crate::foldops::activate::{foldops_activate, write_foldops_verified_marker};
use crate::boot_cmd::refresh_commissioning_display;
use crate::foldops::extract::{
    extract_foldops_deb_data, extract_foldops_layout_bundle, verify_foldops_artifact_file,
};
use crate::foldops::provision::{restart_foldops_runtime_services, start_foldops_provision_service, foldops_provisioned};
use crate::foldops::util::{
    embedded_bootstrap_cache_available, embedded_foldops_bundle_path, foldops_downloads_dir,
    foldops_staged_artifact_path, remove_tree, resolve_effective_foldops_manifest,
    validate_foldingos_compatibility,
};
use crate::foldops::verify::{foldops_installation_verified, normalize_install_tree, verify_foldops_package_tree_at_root};
use crate::foldops_manifest::{foldops_packages_for_role, FoldOpsPackage};
use crate::paths::AppliancePaths;
use crate::process::{command_output, run_command};

pub fn foldops_acquire(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    require_acquire_automation_mutation(paths, "foldops")?;

    let manifest = resolve_effective_foldops_manifest(paths)?;
    validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;
    let role = crate::role::read_active_installation_role(paths)?;
    let packages = foldops_packages_for_role(&manifest, &role)?;
    let package_names: Vec<String> = packages.iter().map(|pkg| pkg.name.clone()).collect();
    let manifest_release = manifest.manifest_release.clone();

    if has_verified_active_release(paths, &manifest_release, &role, &packages)? {
        clear_foldops_acquire_state(paths)?;
        let result = foldops_acquire_result(
            &manifest_release,
            &package_names,
            true,
            false,
            false,
            &format!(
                "Verified FoldOps release {manifest_release} is already active for role {role}; acquisition not required."
            ),
        );
        refresh_commissioning_display(paths);
        restart_foldops_after_acquire(paths)?;
        return Ok(result);
    }

    let state = crate::foldops::acquire_state::load_foldops_acquire_state(paths)?;
    if let (true, remaining) = defer_foldops_acquisition_attempt(&state)? {
        let next_attempt = chrono_like_rfc3339(state.next_attempt_unix);
        return Ok(foldops_acquire_result(
            &manifest_release,
            &package_names,
            false,
            false,
            true,
            &format!(
                "FoldOps acquisition deferred for {}s (next attempt at {next_attempt}).",
                remaining.as_secs()
            ),
        ));
    }

    if let Err(error) = require_foldops_acquisition_prerequisites(
        paths,
        &manifest_release,
        &manifest.artifact_format,
        &manifest.architecture,
        &packages,
    ) {
        record_foldops_acquisition_failure(paths, &error)?;
    }
    if let Err(error) = download_and_stage_foldops_packages(
        paths,
        &manifest_release,
        &manifest.artifact_format,
        &manifest.architecture,
        &packages,
    ) {
        record_foldops_acquisition_failure(paths, &error)?;
    }
    let release_dir = match extract_and_install_foldops_packages(
        paths,
        &manifest_release,
        &manifest.artifact_format,
        &packages,
        &role,
    ) {
        Ok(release_dir) => release_dir,
        Err(error) => {
            record_foldops_acquisition_failure(paths, &error)?;
            unreachable!("record_foldops_acquisition_failure always returns Err")
        }
    };

    if let Err(error) = foldops_activate(paths, &manifest_release) {
        record_foldops_acquisition_failure(paths, &error)?;
    }
    clear_foldops_acquire_state(paths)?;
    refresh_commissioning_display(paths);

    let result = foldops_acquire_result(
        &manifest_release,
        &package_names,
        true,
        true,
        false,
        &format!(
            "Installed and verified FoldOps release {manifest_release} at {}.",
            release_dir.display()
        ),
    );
    restart_foldops_after_acquire(paths)?;
    Ok(result)
}

fn restart_foldops_after_acquire(paths: &AppliancePaths) -> Result<(), String> {
    if foldops_provisioned(paths) {
        restart_foldops_runtime_services(paths)
    } else {
        start_foldops_provision_service()
    }
}

fn foldops_acquire_result(
    manifest_release: &str,
    packages: &[String],
    activated: bool,
    acquired: bool,
    deferred: bool,
    message: &str,
) -> serde_json::Value {
    serde_json::json!({
        "manifest_release": manifest_release,
        "activated": activated,
        "packages": packages,
        "acquired": acquired,
        "already_active": activated && !acquired && !deferred,
        "deferred": deferred,
        "message": message,
    })
}

fn chrono_like_rfc3339(unix: i64) -> String {
    // RFC3339 formatting without adding chrono dependency.
    let secs = unix.rem_euclid(86_400);
    let days = unix.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs / 3600;
    let minute = (secs % 3600) / 60;
    let second = secs % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
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

fn has_verified_active_release(
    paths: &AppliancePaths,
    release: &str,
    role: &str,
    packages: &[FoldOpsPackage],
) -> Result<bool, String> {
    let current_release = match crate::foldops::activate::read_foldops_current_release(paths) {
        Ok(release) => release,
        Err(_) => return Ok(false),
    };
    if current_release != release {
        return Ok(false);
    }
    foldops_installation_verified(paths, release, role, packages)
}

fn require_foldops_acquisition_prerequisites(
    paths: &AppliancePaths,
    manifest_release: &str,
    artifact_format: &str,
    architecture: &str,
    packages: &[FoldOpsPackage],
) -> Result<(), String> {
    if embedded_bootstrap_cache_available(
        paths,
        manifest_release,
        artifact_format,
        architecture,
        packages,
    ) {
        return Ok(());
    }
    if run_command("systemctl", &["is-active", "--quiet", "network-online.target"]).is_err() {
        return Err("network is not online".into());
    }
    let synchronized = ntp_synchronized_from_timedatectl()?;
    if !synchronized {
        return Err("system time is not synchronized".into());
    }
    Ok(())
}

fn ntp_synchronized_from_timedatectl() -> Result<bool, String> {
    let value = command_output("timedatectl", &["show", "-p", "NTPSynchronized", "--value"])?;
    Ok(value.trim().eq_ignore_ascii_case("yes"))
}

fn download_and_stage_foldops_packages(
    paths: &AppliancePaths,
    manifest_release: &str,
    artifact_format: &str,
    architecture: &str,
    packages: &[FoldOpsPackage],
) -> Result<(), String> {
    fs::create_dir_all(foldops_downloads_dir(paths))
        .map_err(|error| format!("create downloads directory: {error}"))?;
    for pkg in packages {
        download_and_stage_foldops_package(
            paths,
            manifest_release,
            artifact_format,
            architecture,
            pkg,
        )?;
    }
    Ok(())
}

fn download_and_stage_foldops_package(
    paths: &AppliancePaths,
    manifest_release: &str,
    artifact_format: &str,
    architecture: &str,
    pkg: &FoldOpsPackage,
) -> Result<(), String> {
    fs::create_dir_all(foldops_downloads_dir(paths))
        .map_err(|error| format!("create downloads directory: {error}"))?;
    let staged_path = foldops_staged_artifact_path(paths, artifact_format, pkg);
    let partial_path = format!("{}.partial", staged_path.display());
    let partial_path = std::path::Path::new(&partial_path);

    if partial_path.exists() {
        fs::remove_file(partial_path)
            .map_err(|error| format!("remove stale partial download: {error}"))?;
    }
    if staged_path.exists() {
        fs::remove_file(&staged_path)
            .map_err(|error| format!("remove stale staged artifact: {error}"))?;
    }

    if let Err(error) = stage_foldops_package(
        paths,
        manifest_release,
        artifact_format,
        architecture,
        pkg,
        partial_path,
    ) {
        let _ = fs::remove_file(partial_path);
        return Err(error);
    }
    if let Err(error) = verify_foldops_artifact_file(partial_path, pkg) {
        let _ = fs::remove_file(partial_path);
        return Err(error);
    }
    if let Err(error) = fs::rename(partial_path, &staged_path) {
        let _ = fs::remove_file(partial_path);
        return Err(format!("stage verified artifact: {error}"));
    }
    crate::automation::say_stdout(format!(
        "Staged verified {} {} artifact at {}.",
        pkg.name,
        pkg.version,
        staged_path.display()
    ));
    Ok(())
}

fn stage_foldops_package(
    paths: &AppliancePaths,
    manifest_release: &str,
    _artifact_format: &str,
    architecture: &str,
    pkg: &FoldOpsPackage,
    destination: &std::path::Path,
) -> Result<(), String> {
    let embedded = embedded_foldops_bundle_path(paths, manifest_release, architecture, pkg);
    if embedded.is_file() {
        verify_foldops_artifact_file(&embedded, pkg)?;
        fs::copy(&embedded, destination).map_err(|error| {
            format!(
                "copy embedded {} artifact from {}: {error}",
                pkg.name,
                embedded.display()
            )
        })?;
        crate::automation::say_stdout(format!(
            "Using embedded bootstrap {} artifact from {}.",
            pkg.name,
            embedded.display()
        ));
        return Ok(());
    }
    download_foldops_package(pkg, destination)
}

fn download_foldops_package(pkg: &FoldOpsPackage, destination: &std::path::Path) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let response = agent
        .get(&pkg.artifact_url)
        .call()
        .map_err(|error| format!("download {} artifact: {error}", pkg.name))?;
    if response.get_url() != pkg.artifact_url {
        return Err(format!(
            "{} artifact download resolved to an unexpected URL",
            pkg.name
        ));
    }
    if response.status() != 200 {
        return Err(format!(
            "{} artifact download failed with status {}",
            pkg.name,
            response.status()
        ));
    }
    let mut reader = response.into_reader();
    let mut file = File::create(destination)
        .map_err(|error| format!("open partial download: {error}"))?;
    let mut hasher = Sha256::new();
    let mut written = 0u64;
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("write partial download: {error}"))?;
        if read == 0 {
            break;
        }
        written += read as u64;
        if written > pkg.artifact_size as u64 {
            return Err(format!(
                "{} artifact download exceeded expected size {} bytes",
                pkg.name, pkg.artifact_size
            ));
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .map_err(|error| format!("write partial download: {error}"))?;
    }
    if written != pkg.artifact_size as u64 {
        return Err(format!(
            "{} artifact download size {written} does not match expected size {}",
            pkg.name, pkg.artifact_size
        ));
    }
    file.sync_all()
        .map_err(|error| format!("sync partial download: {error}"))?;
    Ok(())
}

fn extract_and_install_foldops_packages(
    paths: &AppliancePaths,
    release: &str,
    artifact_format: &str,
    packages: &[FoldOpsPackage],
    role: &str,
) -> Result<std::path::PathBuf, String> {
    if foldops_installation_verified(paths, release, role, packages)? {
        return Ok(paths.foldops_apps_root.join(release));
    }

    let staging_root = paths.foldops_apps_root.join(format!("{release}.staging"));
    let release_dir = paths.foldops_apps_root.join(release);

    remove_tree(&staging_root).ok();
    if release_dir.exists() {
        if release_dir.is_dir() {
            remove_tree(&release_dir)?;
        } else {
            return Err(format!("{} exists but is not a directory", release_dir.display()));
        }
    }

    for pkg in packages {
        if let Err(error) = extract_foldops_package(paths, &staging_root, artifact_format, pkg) {
            let _ = remove_tree(&staging_root);
            return Err(error);
        }
    }
    if let Err(error) = write_foldops_verified_marker(&staging_root, release, role, packages) {
        let _ = remove_tree(&staging_root);
        return Err(error);
    }
    for pkg in packages {
        if let Err(error) = verify_foldops_package_tree_at_root(&staging_root, pkg) {
            let _ = remove_tree(&staging_root);
            return Err(error);
        }
    }
    if let Err(error) = fs::rename(&staging_root, &release_dir) {
        let _ = remove_tree(&staging_root);
        return Err(format!("promote verified installation: {error}"));
    }
    Ok(release_dir)
}

fn extract_foldops_package(
    paths: &AppliancePaths,
    staging_root: &std::path::Path,
    artifact_format: &str,
    pkg: &FoldOpsPackage,
) -> Result<(), String> {
    match artifact_format {
        "deb" => {
            let staged_artifact = foldops_staged_artifact_path(paths, artifact_format, pkg);
            if !staged_artifact.exists() {
                return Err(format!("staged deb artifact is missing: {}", staged_artifact.display()));
            }
            let package_root = staging_root.join(&pkg.name);
            extract_foldops_deb_data(&staged_artifact, &package_root)
                .map_err(|error| format!("extract {}: {error}", pkg.name))?;
            normalize_install_tree(&package_root)
                .map_err(|error| format!("normalize {} install tree: {error}", pkg.name))?;
        }
        "layout-tar-zst" => {
            let staged_artifact = foldops_staged_artifact_path(paths, artifact_format, pkg);
            if !staged_artifact.exists() {
                return Err(format!(
                    "staged layout bundle is missing: {}",
                    staged_artifact.display()
                ));
            }
            let install_prefix = if pkg.install_prefix.trim().is_empty() {
                pkg.name.as_str()
            } else {
                pkg.install_prefix.as_str()
            };
            extract_foldops_layout_bundle(&staged_artifact, staging_root, install_prefix)
                .map_err(|error| format!("extract {}: {error}", pkg.name))?;
        }
        other => return Err(format!("unsupported artifact_format \"{other}\"")),
    }
    verify_foldops_package_tree_at_root(staging_root, pkg)
}
