use std::fs;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

use url::Url;

use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;

pub const INSTALL_SESSION_HEADER: &str = "X-FoldingOS-Install-Session";
pub const UPDATE_SESSION_HEADER: &str = "X-FoldingOS-Update-Session";
pub const DATA_PARTITION_NUMBER: &str = "3";
pub const AGENT_INSTALLATION_ROLE: &str = "agent";

pub fn install_logf(message: &str) {
    let mut output = message.to_string();
    if !output.ends_with('\n') {
        output.push('\n');
    }
    let _ = write_console(&output);
    print!("{output}");
}

fn write_console(message: &str) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/dev/console")
        .map_err(|error| error.to_string())?;
    file.write_all(message.as_bytes())
        .map_err(|error| error.to_string())
}

pub fn format_install_bytes(size: i64) -> String {
    const GIB: i64 = 1024 * 1024 * 1024;
    const MIB: i64 = 1024 * 1024;
    if size >= GIB {
        format!("{:.1} GiB", size as f64 / GIB as f64)
    } else if size >= MIB {
        format!("{:.1} MiB", size as f64 / MIB as f64)
    } else {
        format!("{size} B")
    }
}

pub fn run_command(name: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(name)
        .args(args)
        .status()
        .map_err(|error| format!("{name} failed: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{name} exited with status {status}"))
    }
}

pub fn command_output(name: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(name)
        .args(args)
        .output()
        .map_err(|error| format!("{name} failed: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("{name} failed: {stderr}"))
    }
}

pub fn command_input(input: &str, name: &str, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new(name)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{name} failed: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .map_err(|error| format!("{name} failed: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("{name} failed: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("{name} failed: {stderr}"))
    }
}

pub fn read_supervisor_base_url(paths: &AppliancePaths) -> Result<String, String> {
    match fs::read_to_string(&paths.supervisor_url) {
        Ok(content) => {
            let raw = content.trim();
            if raw.is_empty() {
                return Ok(String::new());
            }
            let parsed =
                Url::parse(raw).map_err(|error| format!("invalid supervisor url: {error}"))?;
            if parsed.scheme() != "http" && parsed.scheme() != "https" {
                return Err(format!("supervisor url must use http or https: {raw:?}"));
            }
            if parsed.host_str().is_none() {
                return Err("supervisor url missing host".into());
            }
            Ok(raw.trim_end_matches('/').to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn read_provision_listen_host(paths: &AppliancePaths) -> Result<String, String> {
    match fs::read_to_string(&paths.provision_listen_url) {
        Ok(content) => {
            let raw = content.trim();
            if raw.is_empty() {
                return Ok("0.0.0.0:8743".into());
            }
            let parsed = Url::parse(raw)
                .map_err(|error| format!("invalid provision listen url: {error}"))?;
            if parsed.scheme() != "http" {
                return Err(format!(
                    "provision listen url must use http for Milestone 3 step 3: {raw:?}"
                ));
            }
            let host = parsed
                .host_str()
                .ok_or_else(|| "provision listen url missing host".to_string())?;
            let port = parsed.port().unwrap_or(80);
            if parsed.port().is_some() {
                Ok(format!("{host}:{port}"))
            } else {
                Ok(host.to_string())
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("0.0.0.0:8743".into()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn join_supervisor_url(base: &str, path: &str) -> Result<String, String> {
    Ok(format!("{}{}", base.trim_end_matches('/'), path))
}

pub fn read_enrollment_token(paths: &AppliancePaths) -> Result<String, String> {
    let content = fs::read_to_string(&paths.enrollment_token).map_err(|error| error.to_string())?;
    let token = content.trim();
    if token.is_empty() {
        return Err("enrollment token is empty".into());
    }
    Ok(token.to_string())
}

pub fn ensure_enrollment_token(paths: &AppliancePaths) -> Result<String, String> {
    match read_enrollment_token(paths) {
        Ok(token) => Ok(token),
        Err(_) if !paths.enrollment_token.exists() => {
            let mut bytes = [0u8; 32];
            read_random_bytes(&mut bytes)?;
            let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            let mut payload = token.clone();
            payload.push('\n');
            atomic_write(&paths.enrollment_token, payload.as_bytes(), 0o600)?;
            Ok(token)
        }
        Err(error) => Err(error),
    }
}

fn read_random_bytes(buf: &mut [u8]) -> Result<(), String> {
    fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(buf))
        .map_err(|error| error.to_string())
}

pub fn validate_enrollment_token(paths: &AppliancePaths, provided: &str) -> Result<(), String> {
    let expected = read_enrollment_token(paths)?;
    if constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err("enrollment token is invalid".into())
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

pub fn mark_agent_enrolled(paths: &AppliancePaths, node_id: &str) -> Result<(), String> {
    let content = format!("{node_id}\n");
    atomic_write(&paths.agent_enrollment_state, content.as_bytes(), 0o644)
}

pub fn agent_enrollment_node_id(paths: &AppliancePaths) -> Result<String, String> {
    let content =
        fs::read_to_string(&paths.agent_enrollment_state).map_err(|error| error.to_string())?;
    let node_id = content.trim();
    if crate::enrollment::is_valid_node_id(node_id) {
        Ok(node_id.to_string())
    } else {
        Err("local enrollment state is invalid".into())
    }
}

pub fn new_session_id() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    read_random_bytes(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn fah_service_active() -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", "folding-at-home.service"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn partition_device(disk: &str, number: &str) -> String {
    if disk.contains("nvme") {
        format!("{disk}p{number}")
    } else {
        format!("{disk}{number}")
    }
}

pub fn efi_partition_path(disk: &str) -> String {
    partition_device(disk, "1")
}

pub fn provision_scratch_dir() -> &'static str {
    for directory in ["/run", "/tmp"] {
        if fs::metadata(directory)
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
        {
            return directory;
        }
    }
    "/tmp"
}

pub fn mounted(device: &str) -> bool {
    command_output("findmnt", &["-rn", "-o", "SOURCE"])
        .map(|listing| {
            listing.lines().any(|line| {
                let line = line.trim();
                line == device || line.starts_with(device)
            })
        })
        .unwrap_or(false)
}

pub fn copy_regular_file(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), String> {
    let content = fs::read(source).map_err(|error| error.to_string())?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(destination, content).map_err(|error| error.to_string())
}

pub fn http_get_json(url: &str, headers: &[(&str, &str)]) -> Result<(u16, String), String> {
    let mut request = ureq::get(url);
    for (name, value) in headers {
        request = request.set(*name, *value);
    }
    match request.call() {
        Ok(response) => {
            let status = response.status();
            let body = response.into_string().map_err(|error| error.to_string())?;
            Ok((status, body))
        }
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string().unwrap_or_default();
            Ok((code, body))
        }
        Err(error) => Err(error.to_string()),
    }
}

pub fn http_post_json(
    url: &str,
    body: &str,
    headers: &[(&str, &str)],
) -> Result<(u16, String), String> {
    let mut request = ureq::post(url).set("Content-Type", "application/json");
    for (name, value) in headers {
        request = request.set(*name, *value);
    }
    match request.send_string(body) {
        Ok(response) => {
            let status = response.status();
            let body = response.into_string().map_err(|error| error.to_string())?;
            Ok((status, body))
        }
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string().unwrap_or_default();
            Ok((code, body))
        }
        Err(error) => Err(error.to_string()),
    }
}

pub fn http_get_stream(
    url: &str,
    headers: &[(&str, &str)],
) -> Result<(u16, Box<dyn Read + Send>), String> {
    let mut request = ureq::get(url);
    for (name, value) in headers {
        request = request.set(*name, *value);
    }
    match request.call() {
        Ok(response) => {
            let status = response.status();
            Ok((status, Box::new(response.into_reader())))
        }
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string().unwrap_or_default();
            Err(format!("HTTP {code}: {body}"))
        }
        Err(error) => Err(error.to_string()),
    }
}

pub fn rfc3339_now() -> String {
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".into())
}

pub fn empty_human_result() -> serde_json::Value {
    serde_json::json!({})
}
