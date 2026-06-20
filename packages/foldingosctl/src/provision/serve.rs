use std::fs;
use std::path::Path;

use crate::paths::AppliancePaths;
use crate::provision::authorize::{
    authorize_provision_install, validate_install_stream_access, ProvisionAuthorizeRequest,
};
use crate::provision::boot::boot_install_disk_for_mac;
use crate::provision::enrollment_api::{
    desired_version_for_node, handle_rollout_assign, register_agent, AgentRegistrationRequest,
    RolloutAssignRequest,
};
use crate::provision::http_server::{
    header_value, query_value, serve_forever, HttpRequest, HttpResponse,
};
use crate::provision::update::{
    authorize_agent_update, handle_update_status, validate_update_stream_access,
    UpdateAuthorizeRequest, UpdateStatusRequest,
};
use crate::provision::util::{
    empty_human_result, ensure_enrollment_token, read_provision_listen_host,
    validate_enrollment_token, INSTALL_SESSION_HEADER, UPDATE_SESSION_HEADER,
};
use crate::registry_image::RegistryEntry;
use crate::role::require_supervisor_role;

pub fn provision_serve(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    require_supervisor_role(paths)?;
    let token = ensure_enrollment_token(paths)?;
    let listen_host = read_provision_listen_host(paths)?;
    println!("Supervisor provisioning API listening on http://{listen_host}");
    println!(
        "Enrollment token is stored at {}",
        paths.enrollment_token.display()
    );
    println!(
        "Generated or loaded enrollment token prefix: {}...",
        &token[..8.min(token.len())]
    );
    serve_forever(&listen_host, |request| handle_request(paths, request))?;
    Ok(empty_human_result())
}

fn handle_request(paths: &AppliancePaths, request: &HttpRequest) -> HttpResponse {
    match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/v1/agents/register") => handle_agent_register(paths, request),
        ("GET", "/v1/agents/desired-version") => handle_desired_version(paths, request),
        ("POST", "/v1/rollouts/assign") => handle_rollout_assign_http(paths, request),
        ("POST", "/v1/agents/update/authorize") => handle_update_authorize(paths, request),
        ("POST", "/v1/agents/update/status") => handle_update_status_http(paths, request),
        ("POST", "/v1/provision/authorize") => handle_provision_authorize(paths, request),
        ("GET", "/boot/ipxe/bootstrap.ipxe") => handle_ipxe_bootstrap(request),
        ("GET", "/boot/ipxe/script.ipxe") => handle_ipxe_install_script(paths, request),
        ("GET", "/boot/vmlinuz") => handle_boot_asset(paths, "vmlinuz", request),
        ("GET", "/boot/install-initramfs.cpio.gz") => {
            handle_boot_asset(paths, "install-initramfs.cpio.gz", request)
        }
        _ if request.path.starts_with("/v1/provision/images/") => {
            handle_provision_image_stream(paths, request)
        }
        _ => HttpResponse::error(404, "not found"),
    }
}

fn handle_agent_register(paths: &AppliancePaths, request: &HttpRequest) -> HttpResponse {
    let registration: AgentRegistrationRequest = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(_) => return HttpResponse::error(400, "invalid registration payload"),
    };
    match register_agent(paths, registration) {
        Ok(record) => match serde_json::to_vec_pretty(&record) {
            Ok(body) => HttpResponse::json(200, body),
            Err(error) => HttpResponse::error(500, &error.to_string()),
        },
        Err(error) => {
            let status = if error.contains("enrollment token") {
                401
            } else {
                400
            };
            HttpResponse::error(status, &error)
        }
    }
}

fn handle_desired_version(paths: &AppliancePaths, request: &HttpRequest) -> HttpResponse {
    let node_id = query_value(request, "node_id").trim();
    if node_id.is_empty() {
        return HttpResponse::error(400, "node_id is required");
    }
    if let Err(error) =
        validate_enrollment_token(paths, header_value(request, "x-foldingos-enrollment-token"))
    {
        return HttpResponse::error(401, &error);
    }
    match desired_version_for_node(paths, node_id) {
        Ok(response) => match serde_json::to_vec_pretty(&response) {
            Ok(body) => HttpResponse::json(200, body),
            Err(error) => HttpResponse::error(500, &error.to_string()),
        },
        Err(error) => {
            let status = if error.contains("not registered") {
                403
            } else {
                400
            };
            HttpResponse::error(status, &error)
        }
    }
}

fn handle_rollout_assign_http(paths: &AppliancePaths, request: &HttpRequest) -> HttpResponse {
    let assign: RolloutAssignRequest = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(_) => return HttpResponse::error(400, "invalid rollout assignment payload"),
    };
    if assign.schema_version != 1 {
        return HttpResponse::error(400, "unsupported rollout assignment schema version");
    }
    if let Err(error) = validate_enrollment_token(paths, assign.enrollment_token.trim()) {
        return HttpResponse::error(401, &error);
    }
    match handle_rollout_assign(paths, assign) {
        Ok(body) => match serde_json::to_vec_pretty(&body) {
            Ok(encoded) => HttpResponse::json(200, encoded),
            Err(error) => HttpResponse::error(500, &error.to_string()),
        },
        Err(error) => {
            let status = if error.contains("not registered") {
                403
            } else {
                400
            };
            HttpResponse::error(status, &error)
        }
    }
}

fn handle_update_authorize(paths: &AppliancePaths, request: &HttpRequest) -> HttpResponse {
    let authorize: UpdateAuthorizeRequest = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(_) => return HttpResponse::error(400, "invalid update authorize payload"),
    };
    match authorize_agent_update(paths, authorize) {
        Ok(response) => match serde_json::to_vec_pretty(&response) {
            Ok(body) => HttpResponse::json(200, body),
            Err(error) => HttpResponse::error(500, &error.to_string()),
        },
        Err(error) => {
            let status = if error.contains("enrollment token") {
                401
            } else if error.contains("not registered") {
                404
            } else {
                400
            };
            HttpResponse::error(status, &error)
        }
    }
}

fn handle_update_status_http(paths: &AppliancePaths, request: &HttpRequest) -> HttpResponse {
    let status_request: UpdateStatusRequest = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(_) => return HttpResponse::error(400, "invalid update status payload"),
    };
    match handle_update_status(paths, status_request) {
        Ok(()) => HttpResponse::json(200, br#"{"status":"ok"}"#.to_vec()),
        Err(error) => {
            let status = if error.contains("not registered") {
                404
            } else {
                400
            };
            HttpResponse::error(status, &error)
        }
    }
}

fn handle_provision_authorize(paths: &AppliancePaths, request: &HttpRequest) -> HttpResponse {
    let authorize: ProvisionAuthorizeRequest = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(_) => return HttpResponse::error(400, "invalid authorize payload"),
    };
    match authorize_provision_install(paths, authorize) {
        Ok(response) => match serde_json::to_vec_pretty(&response) {
            Ok(body) => HttpResponse::json(200, body),
            Err(error) => HttpResponse::error(500, &error.to_string()),
        },
        Err(error) => {
            let status = if error.contains("enrollment token") {
                401
            } else {
                400
            };
            HttpResponse::error(status, &error)
        }
    }
}

fn handle_provision_image_stream(paths: &AppliancePaths, request: &HttpRequest) -> HttpResponse {
    if request.method != "GET" {
        return HttpResponse::error(405, "method not allowed");
    }
    let version = request
        .path
        .trim_start_matches("/v1/provision/images/")
        .trim_end_matches("/stream")
        .trim()
        .to_string();
    if version.is_empty() || version.contains('/') {
        return HttpResponse::error(400, "image version is required");
    }
    let session_id = header_value(request, INSTALL_SESSION_HEADER).trim();
    let update_session_id = header_value(request, UPDATE_SESSION_HEADER).trim();
    let enrollment_token = header_value(request, "x-foldingos-enrollment-token").trim();
    if session_id.is_empty() && update_session_id.is_empty() {
        return HttpResponse::error(400, "install or update session is required");
    }
    if !session_id.is_empty() && !update_session_id.is_empty() {
        return HttpResponse::error(400, "only one of install or update session may be provided");
    }

    let entry = if !update_session_id.is_empty() {
        match validate_update_stream_access(paths, update_session_id, &version, enrollment_token) {
            Ok((_, entry)) => entry,
            Err(error) => {
                let status =
                    if error.contains("enrollment token") || error.contains("update session") {
                        401
                    } else {
                        400
                    };
                return HttpResponse::error(status, &error);
            }
        }
    } else {
        match validate_install_stream_access(paths, session_id, &version, enrollment_token) {
            Ok((_, entry)) => entry,
            Err(error) => {
                let status =
                    if error.contains("enrollment token") || error.contains("install session") {
                        401
                    } else {
                        400
                    };
                return HttpResponse::error(status, &error);
            }
        }
    };
    stream_registry_image(&entry)
}

fn stream_registry_image(entry: &RegistryEntry) -> HttpResponse {
    let content = match fs::read(&entry.local_image_path) {
        Ok(content) => content,
        Err(_) => return HttpResponse::error(500, "registry image is unavailable"),
    };
    if content.len() as i64 != entry.image_size_bytes {
        return HttpResponse::error(500, "registry image size mismatch");
    }
    HttpResponse::binary(
        200,
        "application/octet-stream",
        content,
        vec![(
            "X-FoldingOS-Image-SHA256".into(),
            entry.image_sha256.clone(),
        )],
    )
}

fn handle_ipxe_bootstrap(request: &HttpRequest) -> HttpResponse {
    if request.method != "GET" {
        return HttpResponse::error(405, "method not allowed");
    }
    let host = match provision_boot_http_base(request) {
        Ok(host) => host,
        Err(error) => return HttpResponse::error(500, &error),
    };
    let script = render_ipxe_bootstrap_script(&host);
    HttpResponse::text(200, "text/plain", script.into_bytes())
}

fn handle_ipxe_install_script(paths: &AppliancePaths, request: &HttpRequest) -> HttpResponse {
    if request.method != "GET" {
        return HttpResponse::error(405, "method not allowed");
    }
    let host = match provision_boot_http_base(request) {
        Ok(host) => host,
        Err(error) => return HttpResponse::error(500, &error),
    };
    let mac = query_value(request, "mac").trim();
    let token = query_value(request, "token").trim();
    let install_disk = query_value(request, "disk").trim();
    match render_ipxe_install_script(paths, &host, mac, token, install_disk) {
        Ok(script) => HttpResponse::text(200, "text/plain", script.into_bytes()),
        Err(error) => {
            let status = if error.contains("enrollment token") {
                401
            } else {
                403
            };
            HttpResponse::error(status, &error)
        }
    }
}

fn handle_boot_asset(
    paths: &AppliancePaths,
    filename: &str,
    request: &HttpRequest,
) -> HttpResponse {
    if request.method != "GET" {
        return HttpResponse::error(405, "method not allowed");
    }
    let path = paths.provision_boot_assets_dir.join(filename);
    let content = match fs::read(&path) {
        Ok(content) => content,
        Err(_) => return HttpResponse::error(404, "boot asset is unavailable"),
    };
    let content_type = if Path::new(filename).extension().and_then(|ext| ext.to_str()) == Some("gz")
    {
        "application/gzip"
    } else {
        "application/octet-stream"
    };
    HttpResponse::text(200, content_type, content)
}

fn provision_boot_http_base(request: &HttpRequest) -> Result<String, String> {
    let host = header_value(request, "x-foldingos-boot-host").trim();
    if !host.is_empty() {
        return Ok(host.to_string());
    }
    if let Some(host_header) = request.headers.get("host") {
        return Ok(format!("http://{host_header}"));
    }
    Err("boot host is unavailable".into())
}

pub(crate) fn render_ipxe_install_script(
    paths: &AppliancePaths,
    host: &str,
    mac: &str,
    enrollment_token: &str,
    install_disk: &str,
) -> Result<String, String> {
    is_boot_client_eligible(paths, mac, enrollment_token)?;
    let mut token = crate::provision::util::read_enrollment_token(paths)?;
    if !enrollment_token.is_empty() {
        token = enrollment_token.to_string();
    }
    let mut install_disk = install_disk.trim().to_string();
    if install_disk.is_empty() {
        install_disk = boot_install_disk_for_mac(paths, mac);
    }
    if !install_disk.is_empty() {
        install_disk = crate::provision::targets::parse_provision_install_disk_path(&install_disk)?;
    }
    let base = host.trim_end_matches('/');
    let kernel_url = format!("{base}/boot/vmlinuz");
    let initrd_url = format!("{base}/boot/install-initramfs.cpio.gz");
    let mut cmdline_parts = vec![
        "foldingos.install=1".into(),
        format!("foldingos.supervisor={base}"),
        format!("foldingos.enrollment-token={token}"),
        "ip=dhcp".into(),
        "console=ttyS0,115200".into(),
        "console=tty1".into(),
    ];
    if !install_disk.is_empty() {
        cmdline_parts.push(format!("foldingos.install-disk={install_disk}"));
    }
    let cmdline = cmdline_parts.join(" ");
    Ok(format!(
        "#!ipxe\nkernel {kernel_url} {cmdline}\ninitrd {initrd_url}\nboot\n"
    ))
}

pub(crate) fn render_ipxe_bootstrap_script(host: &str) -> String {
    let base = host.trim_end_matches('/');
    let script_url = format!("{base}/boot/ipxe/script.ipxe?mac=${{net0/mac}}&arch=${{buildarch}}");
    format!("#!ipxe\ndhcp\nset foldingos-server {base}\nchain {script_url}\n")
}

pub(crate) fn is_boot_client_eligible(
    paths: &AppliancePaths,
    mac: &str,
    enrollment_token: &str,
) -> Result<(), String> {
    let mac = crate::provision::boot::normalize_mac(mac);
    if mac.is_empty() {
        return Err("client MAC address is required".into());
    }
    if !enrollment_token.is_empty() {
        return validate_enrollment_token(paths, enrollment_token);
    }
    let allowed = crate::provision::boot::read_boot_allowlist_entries(paths)?;
    if allowed.iter().any(|value| value == &mac) {
        Ok(())
    } else {
        Err("client is not enrolled for network boot".into())
    }
}
