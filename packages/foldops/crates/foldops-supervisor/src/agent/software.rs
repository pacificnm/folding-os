use serde_json::Value;

pub async fn push_foldops_acquire(hostname: &str, port: u16, token: &str) -> Result<Value, String> {
    push_software_acquire(hostname, port, token, "foldops-acquire").await
}

pub async fn push_tools_acquire(hostname: &str, port: u16, token: &str) -> Result<Value, String> {
    push_software_acquire(hostname, port, token, "tools-acquire").await
}

async fn push_software_acquire(
    hostname: &str,
    port: u16,
    token: &str,
    endpoint: &str,
) -> Result<Value, String> {
    let url = format!("http://{hostname}:{port}/software/{endpoint}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(&url)
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
            .unwrap_or("Agent software apply error")
            .to_string());
    }
    Ok(body)
}
