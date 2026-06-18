use serde_json::Value;

pub async fn fetch_agent_inspect_foldops(
    hostname: &str,
    port: u16,
    token: &str,
) -> Result<Value, String> {
    fetch_agent_inspect(hostname, port, token, "foldops").await
}

pub async fn fetch_agent_inspect_tools(
    hostname: &str,
    port: u16,
    token: &str,
) -> Result<Value, String> {
    fetch_agent_inspect(hostname, port, token, "tools").await
}

async fn fetch_agent_inspect(
    hostname: &str,
    port: u16,
    token: &str,
    subcommand: &str,
) -> Result<Value, String> {
    let url = format!("http://{hostname}:{port}/inspect/{subcommand}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    let body: Value = response.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(body
            .get("error")
            .and_then(|value| value.as_str())
            .unwrap_or("Agent inspect error")
            .to_string());
    }

    body.get("data")
        .cloned()
        .ok_or_else(|| "Agent inspect response missing data".to_string())
}
