use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    Fah,
    Work,
}

impl LogSource {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "fah" => Some(Self::Fah),
            "work" => Some(Self::Work),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fah => "fah",
            Self::Work => "work",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AgentLogResponse {
    pub path: Option<String>,
    pub lines: Option<Vec<String>>,
}

pub async fn fetch_live_agent_logs(
    hostname: &str,
    port: u16,
    token: &str,
    source: LogSource,
    lines: u32,
) -> Result<(String, Vec<String>), String> {
    let url = format!(
        "http://{hostname}:{port}/logs/{}?lines={lines}",
        source.as_str()
    );
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
    let parsed: AgentLogResponse = serde_json::from_value(body).map_err(|e| e.to_string())?;
    Ok((
        parsed.path.unwrap_or_default(),
        parsed.lines.unwrap_or_default(),
    ))
}
