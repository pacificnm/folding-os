use std::fs;

use serde::Deserialize;

use crate::identity::{read_installed_foldingos_version, read_node_id};
use crate::paths::AppliancePaths;

#[derive(Debug, Deserialize)]
struct StagedUpdateMetadata {
    schema_version: i32,
    current_version: String,
    desired_version: String,
    image_sha256: String,
    image_size_bytes: i64,
    staged_at: String,
    apply_state: String,
}

#[derive(Debug, Deserialize)]
struct PendingUpdateReport {
    schema_version: i32,
    image_version: String,
    status: String,
    #[serde(default)]
    message: String,
    recorded_at: String,
}

#[derive(Debug, Deserialize)]
struct DesiredVersionResponse {
    desired_version: String,
}

pub fn inspect_update(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let current_image_version = read_installed_foldingos_version()?;
    let mut data = serde_json::json!({
        "current_image_version": current_image_version,
        "reboot_required": paths.reboot_required.exists(),
    });

    match query_desired_image_version(paths) {
        Ok(desired) => data["desired_image_version"] = serde_json::Value::String(desired),
        Err(error) => data["desired_query_error"] = serde_json::Value::String(error),
    }

    match load_staged_update_metadata(&paths.staged_update_meta) {
        Ok(metadata) => {
            data["staged_update"] = serde_json::json!({
                "current_version": metadata.current_version,
                "desired_version": metadata.desired_version,
                "image_sha256": metadata.image_sha256,
                "image_size_bytes": metadata.image_size_bytes,
                "staged_at": metadata.staged_at,
                "apply_state": metadata.apply_state,
            });
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("read staged update metadata: {error}")),
    }

    match load_pending_update_report(&paths.pending_update_report) {
        Ok(report) => {
            let mut last_report = serde_json::json!({
                "image_version": report.image_version,
                "status": report.status,
                "recorded_at": report.recorded_at,
            });
            if !report.message.is_empty() {
                last_report["message"] = serde_json::Value::String(report.message);
            }
            data["last_update_report"] = last_report;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("read pending update report: {error}")),
    }

    Ok(data)
}

fn load_staged_update_metadata(path: &std::path::Path) -> Result<StagedUpdateMetadata, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let metadata: StagedUpdateMetadata = serde_json::from_str(&content)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if metadata.schema_version != 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unsupported staged update schema version",
        ));
    }
    Ok(metadata)
}

fn load_pending_update_report(path: &std::path::Path) -> Result<PendingUpdateReport, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let report: PendingUpdateReport = serde_json::from_str(&content)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if report.schema_version != 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unsupported pending update report schema version",
        ));
    }
    Ok(report)
}

fn query_desired_image_version(paths: &AppliancePaths) -> Result<String, String> {
    let supervisor_url = read_supervisor_base_url(paths)?;
    if supervisor_url.is_empty() {
        return Err("supervisor URL is not configured".into());
    }
    let node_id = read_node_id(paths)?;
    let token = read_enrollment_token(paths)?;
    let endpoint = join_supervisor_url(&supervisor_url, &format!("/v1/agents/desired-version?node_id={node_id}"))?;
    let response = ureq::get(&endpoint)
        .set("X-FoldingOS-Enrollment-Token", &token)
        .call()
        .map_err(|error| format!("desired-version query failed: {error}"))?;
    if response.status() != 200 {
        return Err(format!(
            "desired-version query failed with status {}: {}",
            response.status(),
            response.into_string().unwrap_or_default().trim()
        ));
    }
    let body: DesiredVersionResponse = response
        .into_json()
        .map_err(|error| format!("parse desired-version response: {error}"))?;
    Ok(body.desired_version.trim().to_string())
}

fn read_supervisor_base_url(paths: &AppliancePaths) -> Result<String, String> {
    let content = match fs::read_to_string(&paths.supervisor_url) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(format!("read supervisor url: {error}")),
    };
    let raw = content.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }
    let parsed = url::Url::parse(raw).map_err(|error| format!("invalid supervisor url: {error}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(format!("supervisor url must use http or https: \"{raw}\""));
    }
    if parsed.host_str().is_none() {
        return Err("supervisor url missing host".into());
    }
    Ok(raw.trim_end_matches('/').to_string())
}

fn read_enrollment_token(paths: &AppliancePaths) -> Result<String, String> {
    let content = fs::read_to_string(&paths.enrollment_token)
        .map_err(|error| format!("read enrollment token: {error}"))?;
    let token = content.trim();
    if token.is_empty() {
        return Err("enrollment token is empty".into());
    }
    Ok(token.to_string())
}

fn join_supervisor_url(base: &str, path: &str) -> Result<String, String> {
    let parsed = url::Url::parse(base).map_err(|error| format!("invalid supervisor url: {error}"))?;
    let joined = parsed
        .join(path)
        .map_err(|error| format!("join supervisor url: {error}"))?;
    Ok(joined.to_string())
}
