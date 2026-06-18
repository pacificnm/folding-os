use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn build_foldinghome_candidate_toml(
    username: &str,
    team: i64,
    passkey_secret: &str,
) -> String {
    format!(
        "schema_version = 1\n\n[identity]\nusername = {}\nteam = {team}\npasskey_secret = {}\n\n[resources]\ncpus = 0\ngpus = false\n",
        toml_string(username),
        toml_string(passkey_secret),
    )
}

fn toml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[derive(Debug, Deserialize)]
pub struct FoldinghomeConfigRequest {
    pub username: String,
    pub team: i64,
    #[serde(default)]
    pub passkey_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FoldinghomeConfigResult {
    pub ok: bool,
    pub domain: String,
    pub candidate: String,
    pub activated: bool,
}

pub async fn push_foldinghome_config(
    hostname: &str,
    port: u16,
    token: &str,
    config_toml: &str,
) -> Result<FoldinghomeConfigResult, String> {
    let url = format!("http://{hostname}:{port}/config/foldinghome");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "config": config_toml }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let body: Value = res.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Agent error")
            .to_string());
    }
    serde_json::from_value(body).map_err(|e| e.to_string())
}

pub async fn validate_foldinghome_candidate(
    foldingosctl_path: &Path,
    candidate_path: &Path,
) -> Result<(), String> {
    let candidate = candidate_path
        .to_str()
        .ok_or_else(|| "candidate path is not valid UTF-8".to_string())?;
    let output = tokio::process::Command::new(foldingosctl_path)
        .args(["config", "validate", "foldinghome", candidate, "--format", "json"])
        .output()
        .await
        .map_err(|error| error.to_string())?;

    let stdout = String::from_utf8(output.stdout).map_err(|_| "invalid UTF-8 from foldingosctl".to_string())?;
    let envelope: Value = serde_json::from_str(stdout.trim())
        .map_err(|error| format!("invalid JSON from foldingosctl: {error}"))?;

    if output.status.success() && envelope.get("ok").and_then(|value| value.as_bool()) == Some(true) {
        return Ok(());
    }

    if let Some(error) = envelope.get("error") {
        let code = error
            .get("code")
            .and_then(|value| value.as_str())
            .unwrap_or("validation_failed");
        let message = error
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("candidate validation failed");
        return Err(format!("[{code}] {message}"));
    }

    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

pub fn write_supervisor_candidate(content: &str) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!(
        "foldinghome-candidate-{}.toml",
        std::process::id()
    ));
    std::fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_FOLDINGHOME_CANDIDATE: &str = r#"schema_version = 1

[identity]
username = "Test User"
team = 0
passkey_secret = ""

[resources]
cpus = 0
gpus = false
"#;

    #[test]
    fn build_foldinghome_candidate_toml_quotes_strings() {
        let toml = build_foldinghome_candidate_toml("Test User", 123, "");
        assert!(toml.contains("username = \"Test User\""));
        assert!(toml.contains("team = 123"));
        assert!(toml.contains("passkey_secret = \"\""));
        assert!(toml.contains("gpus = false"));
    }

    #[tokio::test]
    async fn validate_foldinghome_candidate_accepts_valid_fixture() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let candidate = temp.path().join("candidate.toml");
        std::fs::write(&candidate, VALID_FOLDINGHOME_CANDIDATE).expect("write candidate");

        let foldingosctl = write_mock_foldingosctl(&temp, VALIDATE_AND_ACTIVATE_MOCK);
        validate_foldinghome_candidate(&foldingosctl, &candidate)
            .await
            .expect("validate candidate");
    }

    fn write_mock_foldingosctl(dir: &tempfile::TempDir, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.path().join("foldingosctl");
        std::fs::write(&path, script).expect("write mock foldingosctl");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod mock foldingosctl");
        path
    }

    const VALIDATE_AND_ACTIVATE_MOCK: &str = r#"#!/bin/sh
if [ "$1" = "config" ] && [ "$2" = "validate" ] && [ "$3" = "foldinghome" ]; then
  printf '%s' '{"schema_version":1,"ok":true,"command":"config validate foldinghome","data":{"domain":"foldinghome","candidate":"'"$4"'","valid":true}}'
  exit 0
fi
exit 1
"#;
}
