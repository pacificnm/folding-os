use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

pub struct HttpResponse {
    pub status: u16,
    pub reason: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            reason: status_reason(status),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body,
        }
    }

    pub fn text(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            reason: status_reason(status),
            headers: vec![("Content-Type".into(), content_type.into())],
            body,
        }
    }

    pub fn error(status: u16, message: &str) -> Self {
        Self::text(
            status,
            "text/plain; charset=utf-8",
            message.as_bytes().to_vec(),
        )
    }

    pub fn binary(
        status: u16,
        content_type: &str,
        body: Vec<u8>,
        extra_headers: Vec<(String, String)>,
    ) -> Self {
        let mut headers = vec![("Content-Type".into(), content_type.into())];
        headers.extend(extra_headers);
        Self {
            status,
            reason: status_reason(status),
            headers,
            body,
        }
    }
}

pub fn serve_forever<F>(addr: &str, handler: F) -> Result<(), String>
where
    F: Fn(&HttpRequest) -> HttpResponse,
{
    let listener = TcpListener::bind(addr).map_err(|error| error.to_string())?;
    for stream in listener.incoming() {
        let mut stream = stream.map_err(|error| error.to_string())?;
        let request = match read_request(&mut stream) {
            Ok(request) => request,
            Err(_) => continue,
        };
        let response = handler(&request);
        let _ = write_response(&mut stream, &response);
    }
    Ok(())
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(30)))
        .ok();
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
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
    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default();
    let (path, query_string) = target.split_once('?').unwrap_or((target, ""));
    let query = parse_query(query_string);
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let mut body = buffer[header_end..].to_vec();
    if let Some(length) = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
    {
        while body.len() < length {
            let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..read]);
            if body.len() > 1 << 20 {
                return Err("request body too large".into());
            }
        }
        body.truncate(length);
    }
    Ok(HttpRequest {
        method,
        path: path.to_string(),
        query,
        headers,
        body,
    })
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        if let Some((key, value)) = pair.split_once('=') {
            values.insert(key.to_string(), value.to_string());
        } else {
            values.insert(pair.to_string(), String::new());
        }
    }
    values
}

fn write_response(stream: &mut TcpStream, response: &HttpResponse) -> Result<(), String> {
    let mut output = format!("HTTP/1.1 {} {}\r\n", response.status, response.reason);
    output.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
    for (name, value) in &response.headers {
        output.push_str(&format!("{name}: {value}\r\n"));
    }
    output.push_str("\r\n");
    stream
        .write_all(output.as_bytes())
        .map_err(|error| error.to_string())?;
    stream
        .write_all(&response.body)
        .map_err(|error| error.to_string())?;
    stream.flush().ok();
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
        500 => "Internal Server Error",
        _ => "OK",
    }
}

pub fn header_value<'a>(request: &'a HttpRequest, name: &str) -> &'a str {
    request
        .headers
        .get(&name.to_ascii_lowercase())
        .map(String::as_str)
        .unwrap_or_default()
}

pub fn query_value<'a>(request: &'a HttpRequest, name: &str) -> &'a str {
    request
        .query
        .get(name)
        .map(String::as_str)
        .unwrap_or_default()
}
