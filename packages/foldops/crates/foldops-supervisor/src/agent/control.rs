use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AgentControlStatus {
    pub foldops_agent: String,
    pub fah_client: String,
}

#[derive(Debug, Deserialize)]
pub struct AgentControlResult {
    pub ok: bool,
    pub action: String,
    pub message: String,
    pub stdout: String,
    pub stderr: String,
}

pub async fn fetch_agent_control_status(
    hostname: &str,
    port: u16,
    token: &str,
) -> Result<AgentControlStatus, String> {
    let url = format!("http://{hostname}:{port}/control/status");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let body: serde_json::Value = res.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Agent error")
            .to_string());
    }
    serde_json::from_value(body).map_err(|e| e.to_string())
}

pub async fn push_agent_control(
    hostname: &str,
    port: u16,
    token: &str,
    action: &str,
) -> Result<AgentControlResult, String> {
    let url = format!("http://{hostname}:{port}/control");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "action": action }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let body: serde_json::Value = res.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Agent error")
            .to_string());
    }
    serde_json::from_value(body).map_err(|e| e.to_string())
}
