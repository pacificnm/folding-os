use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use rustls::ServerConnection;

use crate::foldops::tls::{load_rustls_config, validate_foldops_tls_ready};
use crate::foldops::util::FOLDOPS_SUPERVISOR_LOOPBACK_PORT;
use crate::paths::AppliancePaths;

const LISTEN_ADDRESS: &str = ":3443";

pub fn foldops_serve_https(paths: &AppliancePaths) -> Result<(), String> {
    let role = crate::role::read_active_installation_role(paths)?;
    if role != "supervisor" {
        return Err("foldops serve-https is supported only on supervisor role".into());
    }
    validate_foldops_tls_ready(paths)?;
    let (_cert_path, _key_path) = crate::foldops::tls::load_foldops_tls_certificate(paths)?;
    let config = Arc::new(load_rustls_config(paths)?);
    let upstream = format!("http://127.0.0.1:{FOLDOPS_SUPERVISOR_LOOPBACK_PORT}");
    println!(
        "FoldOps HTTPS front end listening on https://0.0.0.0{LISTEN_ADDRESS} -> {upstream}"
    );
    serve_tls_reverse_proxy(LISTEN_ADDRESS, &upstream, config)
}

fn serve_tls_reverse_proxy(
    listen_addr: &str,
    upstream_base: &str,
    config: Arc<rustls::ServerConfig>,
) -> Result<(), String> {
    let listener = TcpListener::bind(listen_addr).map_err(|error| error.to_string())?;
    for stream in listener.incoming() {
        let mut stream = stream.map_err(|error| error.to_string())?;
        let config = config.clone();
        let upstream_base = upstream_base.to_string();
        if let Err(error) = handle_client(&mut stream, config, &upstream_base) {
            eprintln!("foldops serve-https: {error}");
        }
    }
    Ok(())
}

fn handle_client(
    stream: &mut TcpStream,
    config: Arc<rustls::ServerConfig>,
    upstream_base: &str,
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .ok();
    let mut tls = ServerConnection::new(config).map_err(|error| error.to_string())?;
    let mut tls_stream = rustls::Stream::new(&mut tls, stream);
    let request = read_http_request(&mut tls_stream)?;
    let response = proxy_request(&request, upstream_base)?;
    write_http_response(&mut tls_stream, &response)?;
    tls_stream.flush().ok();
    Ok(())
}

struct HttpRequest {
    method: String,
    path: String,
    version: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

struct HttpResponse {
    status: u16,
    reason: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn read_http_request(stream: &mut impl Read) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("client closed connection".into());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > 1 << 20 {
            return Err("request too large".into());
        }
    }
    let header_end = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "invalid request".to_string())?
        + 4;
    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let version = parts.next().unwrap_or_default().to_string();
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let mut body = buffer[header_end..].to_vec();
    if let Some(length) = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
    {
        while body.len() < length {
            let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..read]);
        }
        body.truncate(length);
    }
    Ok(HttpRequest {
        method,
        path,
        version,
        headers,
        body,
    })
}

fn proxy_request(request: &HttpRequest, upstream_base: &str) -> Result<HttpResponse, String> {
    let upstream_url = format!("{upstream_base}{}", request.path);
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let mut upstream_request = match request.method.as_str() {
        "GET" => agent.get(&upstream_url),
        "HEAD" => agent.head(&upstream_url),
        "POST" => agent.post(&upstream_url),
        "PUT" => agent.put(&upstream_url),
        "PATCH" => agent.request("PATCH", &upstream_url),
        "DELETE" => agent.delete(&upstream_url),
        other => return Err(format!("unsupported method {other}")),
    };
    for (name, value) in &request.headers {
        if name.eq_ignore_ascii_case("host") || name.eq_ignore_ascii_case("connection") {
            continue;
        }
        upstream_request = upstream_request.set(name, value);
    }
    let response = upstream_request
        .send_bytes(&request.body)
        .map_err(|_| "foldops upstream unavailable".to_string())?;
    let status = response.status();
    let mut headers = Vec::new();
    for name in response.headers_names() {
        if let Some(value) = response.header(&name) {
            headers.push((name, value.to_string()));
        }
    }
    let body = response.into_string().unwrap_or_default().into_bytes();
    Ok(HttpResponse {
        status,
        reason: status_reason(status),
        headers,
        body,
    })
}

fn write_http_response(stream: &mut impl Write, response: &HttpResponse) -> Result<(), String> {
    let mut output = format!("HTTP/1.1 {} {}\r\n", response.status, response.reason);
    output.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
    for (name, value) in &response.headers {
        if name.eq_ignore_ascii_case("transfer-encoding") {
            continue;
        }
        output.push_str(&format!("{name}: {value}\r\n"));
    }
    output.push_str("\r\n");
    stream
        .write_all(output.as_bytes())
        .map_err(|error| error.to_string())?;
    stream
        .write_all(&response.body)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        502 => "Bad Gateway",
        500 => "Internal Server Error",
        _ => "OK",
    }
}
