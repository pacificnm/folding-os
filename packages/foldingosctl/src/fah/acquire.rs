use std::fs::{self, File};
use std::io::{Read, Write};
use std::sync::LazyLock;

use ureq::Agent;

use crate::paths::AppliancePaths;
use crate::process::{command_output, run_command};

use super::acquire_state::{
    clear_fah_acquire_state, defer_fah_acquisition_attempt, load_fah_acquire_state,
    record_fah_acquisition_failure,
};
use super::activate::fah_activate;
use super::manifest::{load_fah_manifest, validate_foldingos_compatibility, FahManifest};
use super::util::format_go_duration;
use super::verify_install::{
    extract_and_install_fah_artifact, fah_installation_verified, verify_fah_artifact_file,
};

static FAH_HTTP_AGENT: LazyLock<Agent> =
    LazyLock::new(|| ureq::AgentBuilder::new().redirects(0).build());

pub fn fah_acquire(paths: &AppliancePaths) -> Result<(), String> {
    let manifest = load_fah_manifest(paths, &paths.fah_embedded_manifest)?;
    validate_foldingos_compatibility(&manifest.minimum_foldingos_version)?;
    if has_verified_active_client(paths, &manifest) {
        clear_fah_acquire_state(paths)?;
        println!(
            "Verified Folding@home client {} is already active; acquisition not required.",
            manifest.client_version
        );
        return Ok(());
    }

    let state = load_fah_acquire_state(paths)?;
    let (deferred, remaining) = defer_fah_acquisition_attempt(&state)?;
    if deferred {
        let next_attempt = chrono_like_rfc3339(state.next_attempt_unix);
        println!(
            "Folding@home acquisition deferred for {} (next attempt at {next_attempt}).",
            format_go_duration(remaining)
        );
        return Ok(());
    }

    if let Err(error) = require_fah_acquisition_prerequisites() {
        return record_fah_acquisition_failure(paths, &error);
    }
    let staged_path = match download_and_stage_fah_artifact(paths, &manifest) {
        Ok(path) => path,
        Err(error) => return record_fah_acquisition_failure(paths, &error),
    };
    println!(
        "Staged verified Folding@home {} artifact at {}.",
        manifest.client_version,
        staged_path.display()
    );

    let version_dir = match extract_and_install_fah_artifact(paths, &manifest) {
        Ok(path) => path,
        Err(error) => return record_fah_acquisition_failure(paths, &error),
    };
    println!(
        "Installed and verified Folding@home {} at {}.",
        manifest.client_version,
        version_dir.display()
    );

    if let Err(error) = fah_activate(paths, &manifest.client_version) {
        return record_fah_acquisition_failure(paths, &error);
    }
    clear_fah_acquire_state(paths)
}

fn chrono_like_rfc3339(unix: i64) -> String {
    let secs = unix.max(0) as u64;
    let days = secs / 86_400;
    let time_of_day = secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as i64, d as i64)
}

pub fn require_fah_acquisition_prerequisites() -> Result<(), String> {
    if run_command(
        "systemctl",
        &["is-active", "--quiet", "network-online.target"],
    )
    .is_err()
    {
        return Err("network is not online".into());
    }
    let synchronized = fah_ntp_synchronized()?;
    if !synchronized {
        return Err("system time is not synchronized".into());
    }
    Ok(())
}

fn fah_ntp_synchronized() -> Result<bool, String> {
    let value = command_output("timedatectl", &["show", "-p", "NTPSynchronized", "--value"])?;
    Ok(value.trim() == "yes")
}

pub fn has_verified_active_client(paths: &AppliancePaths, manifest: &FahManifest) -> bool {
    let Ok(version) = super::util::read_fah_current_version(paths) else {
        return false;
    };
    fah_installation_verified(paths, &version, manifest)
}

pub fn download_and_stage_fah_artifact(
    paths: &AppliancePaths,
    manifest: &FahManifest,
) -> Result<std::path::PathBuf, String> {
    let downloads_dir = paths.fah_downloads_dir();
    fs::create_dir_all(&downloads_dir)
        .map_err(|error| format!("create downloads directory: {error}"))?;

    let partial_path = paths.fah_partial_deb(&manifest.client_version);
    let staged_path = paths.fah_staged_deb(&manifest.client_version);

    if partial_path.exists() {
        fs::remove_file(&partial_path)
            .map_err(|error| format!("remove stale partial download: {error}"))?;
    }
    if staged_path.exists() {
        fs::remove_file(&staged_path)
            .map_err(|error| format!("remove stale staged artifact: {error}"))?;
    }

    if let Err(error) = download_fah_artifact(manifest, &partial_path) {
        let _ = fs::remove_file(&partial_path);
        return Err(error);
    }
    if let Err(error) = verify_fah_artifact_file(&partial_path, manifest) {
        let _ = fs::remove_file(&partial_path);
        return Err(error);
    }
    if let Err(error) = fs::rename(&partial_path, &staged_path) {
        let _ = fs::remove_file(&partial_path);
        return Err(format!("stage verified artifact: {error}"));
    }
    Ok(staged_path)
}

fn download_fah_artifact(
    manifest: &FahManifest,
    destination: &std::path::Path,
) -> Result<(), String> {
    let response = FAH_HTTP_AGENT
        .get(&manifest.artifact_url)
        .call()
        .map_err(|error| format!("download artifact: {error}"))?;

    if response.get_url() != manifest.artifact_url {
        return Err("artifact download resolved to an unexpected URL".into());
    }
    if response.status() != 200 {
        return Err(format!(
            "artifact download failed with status {}",
            response.status()
        ));
    }

    let mut file =
        File::create(destination).map_err(|error| format!("open partial download: {error}"))?;
    let mut reader = response.into_reader();
    let mut written = 0_i64;
    let mut buffer = vec![0_u8; 8192];
    while written < manifest.artifact_size {
        let remaining = (manifest.artifact_size - written) as usize;
        let chunk = remaining.min(buffer.len());
        let read = reader
            .read(&mut buffer[..chunk])
            .map_err(|error| format!("write partial download: {error}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|error| format!("write partial download: {error}"))?;
        written += read as i64;
    }
    file.sync_all()
        .map_err(|error| format!("sync partial download: {error}"))?;

    if written > manifest.artifact_size {
        return Err(format!(
            "artifact download exceeded expected size {} bytes",
            manifest.artifact_size
        ));
    }
    if written != manifest.artifact_size {
        return Err(format!(
            "artifact download size {written} does not match expected size {}",
            manifest.artifact_size
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use crate::fah::manifest::parse_fah_manifest;

    fn test_manifest(artifact: &[u8]) -> FahManifest {
        let mut manifest = parse_fah_manifest(&test_manifest_content()).expect("manifest");
        manifest.artifact_size = artifact.len() as i64;
        let digest = Sha256::digest(artifact);
        manifest.sha256 = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        manifest
    }

    fn test_manifest_content() -> String {
        r#"schema_version = 1
client_version = "8.5.5"
architecture = "x86_64"
artifact_url = "https://download.foldingathome.org/approved.deb"
artifact_size = 1
sha256 = "4f9c8bed9b2893752afb87e2796512ca0ca300ffc3d6035c518d56360370886c"
artifact_format = "deb"
minimum_foldingos_version = "0.1.0"
terms_url = "https://foldingathome.org/faq/opensource/"
executable_path = "/data/apps/fah/current/usr/bin/fah-client"
arguments = ["--config=/run/foldingos/fah/config.xml"]
"#
        .to_string()
    }

    fn spawn_http_server(
        handler: impl Fn(&str) -> (u16, Vec<u8>) + Send + 'static,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            listener.set_nonblocking(true).expect("nonblocking");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept failed: {error}"),
                }
            };
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).expect("read request");
            let request = String::from_utf8_lossy(&request[..read]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");
            let (status, body) = handler(path);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write headers");
            stream.write_all(&body).expect("write body");
            stream.shutdown(std::net::Shutdown::Write).ok();
        });
        (format!("http://{addr}"), handle)
    }

    fn test_paths(root: &std::path::Path) -> AppliancePaths {
        let mut paths = AppliancePaths::default();
        paths.fah_apps_root = root.to_path_buf();
        paths
    }

    #[test]
    fn download_and_stage_fah_artifact_success() {
        let artifact = b"foldingos-test-artifact";
        let mut manifest = test_manifest(artifact);
        let (base_url, _handle) = spawn_http_server(|path| {
            assert_eq!(path, "/approved.deb");
            (200, artifact.to_vec())
        });
        manifest.artifact_url = format!("{base_url}/approved.deb");

        let root = std::env::temp_dir().join(format!("fah-download-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");
        let paths = test_paths(&root);

        let staged_path = download_and_stage_fah_artifact(&paths, &manifest).expect("stage");
        assert_eq!(staged_path, paths.fah_downloads_dir().join("8.5.5.deb"));
        assert!(staged_path.exists());
    }

    #[test]
    fn reject_oversized_fah_download() {
        let artifact = b"foldingos-test-artifact-too-large";
        let mut manifest = test_manifest(artifact);
        manifest.artifact_size = artifact.len() as i64 - 1;
        let (base_url, _handle) = spawn_http_server(|_| (200, artifact.to_vec()));
        manifest.artifact_url = format!("{base_url}/approved.deb");

        let root =
            std::env::temp_dir().join(format!("fah-download-oversize-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");
        let paths = test_paths(&root);

        assert!(download_and_stage_fah_artifact(&paths, &manifest).is_err());
        assert!(!paths.fah_partial_deb("8.5.5").exists());
    }

    #[test]
    fn reject_wrong_fah_artifact_hash() {
        let artifact = b"foldingos-test-artifact";
        let mut manifest = test_manifest(artifact);
        manifest.sha256 = "a".repeat(64);
        let (base_url, _handle) = spawn_http_server(|_| (200, artifact.to_vec()));
        manifest.artifact_url = format!("{base_url}/approved.deb");

        let root = std::env::temp_dir().join(format!("fah-download-hash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("mkdir");
        let paths = test_paths(&root);

        assert!(download_and_stage_fah_artifact(&paths, &manifest).is_err());
    }

    #[test]
    fn fah_acquire_requires_synchronized_time() {
        assert!(
            require_fah_acquisition_prerequisites().is_err()
                || fah_ntp_synchronized().unwrap_or(false)
        );
    }

    #[test]
    fn has_verified_active_client_detects_installation() {
        let root = std::env::temp_dir().join(format!("fah-active-client-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let version_dir = root.join("8.5.5");
        fs::create_dir_all(version_dir.join("usr/bin")).expect("mkdir");
        fs::write(version_dir.join("usr/bin/fah-client"), b"binary").expect("write exe");
        std::os::unix::fs::symlink("8.5.5", root.join("current")).expect("symlink");
        fs::write(
            version_dir.join(crate::paths::FAH_VERIFIED_MARKER),
            "client_version=8.5.5\nartifact_sha256=4f9c8bed9b2893752afb87e2796512ca0ca300ffc3d6035c518d56360370886c\n",
        )
        .expect("marker");

        let manifest = parse_fah_manifest(&test_manifest_content()).expect("manifest");
        let paths = test_paths(&root);
        assert!(has_verified_active_client(&paths, &manifest));
    }
}
