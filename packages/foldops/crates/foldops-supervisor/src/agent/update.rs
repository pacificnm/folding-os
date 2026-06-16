use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AgentUpdateResponse {
    pub ok: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: i64,
    pub restarting: Option<bool>,
}

pub async fn push_agent_update(
    hostname: &str,
    port: u16,
    token: &str,
) -> Result<AgentUpdateResponse, String> {
    let url = format!("http://{hostname}:{port}/update");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .post(&url)
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
