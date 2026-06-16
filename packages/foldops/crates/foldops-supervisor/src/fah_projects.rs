use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

const FAH_PROJECT_API: &str = "https://api.foldingathome.org/project";
const CACHE_TTL: Duration = Duration::from_secs(3600);
const FETCH_TIMEOUT: Duration = Duration::from_secs(12);

static CACHE: LazyLock<Mutex<HashMap<i64, (FahProjectPublic, Instant)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FahProjectPublic {
    pub project: i64,
    pub manager: Option<String>,
    pub cause: Option<String>,
    pub institution: Option<String>,
    pub description: Option<String>,
    pub project_range: Option<String>,
    pub modified: Option<String>,
    pub stats_url: String,
}

fn html_to_plain(html: &str) -> String {
    static BR: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(?i)<br\s*/?>").unwrap());
    static P: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(?i)</p>").unwrap());
    static TAGS: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"<[^>]+>").unwrap());
    static NL3: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\n{3,}").unwrap());

    let s = BR.replace_all(html, "\n");
    let s = P.replace_all(&s, "\n\n");
    let s = TAGS.replace_all(&s, "");
    let s = s.replace("&#39;", "'").replace("&quot;", "\"").replace("&amp;", "&").replace("&nbsp;", " ");
    NL3.replace_all(&s, "\n\n").trim().to_string()
}

fn pick_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Number(n) => n.as_f64().map(|f| f.to_string()),
        serde_json::Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        _ => None,
    }
}

fn slim_fah_project_json(text: &str) -> String {
    static MTHUMB: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r#""mthumb"\s*:\s*"(?:\\.|[^"\\])*""#).unwrap());
    static THUMB: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r#""thumb"\s*:\s*"(?:\\.|[^"\\])*""#).unwrap());
    let s = MTHUMB.replace_all(text, r#""mthumb":"""#);
    THUMB.replace_all(&s, r#""thumb":"""#).into_owned()
}

fn normalize_project(raw: &serde_json::Value, project_id: i64) -> FahProjectPublic {
    let description_html = raw
        .get("description")
        .and_then(pick_string)
        .or_else(|| raw.get("mdescription").and_then(pick_string));
    FahProjectPublic {
        project: project_id,
        manager: raw.get("manager").and_then(pick_string),
        cause: raw.get("cause").and_then(pick_string),
        institution: raw.get("institution").and_then(pick_string),
        description: description_html.map(|h| html_to_plain(&h)),
        project_range: raw.get("projects").and_then(pick_string),
        modified: raw.get("modified").and_then(pick_string),
        stats_url: format!("https://stats.foldingathome.org/project/{project_id}"),
    }
}

pub async fn fetch_fah_project(project_id: i64) -> Result<Option<FahProjectPublic>, String> {
    {
        let cache = CACHE.lock().unwrap();
        if let Some((data, at)) = cache.get(&project_id) {
            if at.elapsed() < CACHE_TTL {
                return Ok(Some(data.clone()));
            }
        }
    }

    let url = format!("{FAH_PROJECT_API}/{project_id}");
    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    if status.as_u16() == 404 || status.as_u16() == 400 {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!("FAH API returned {status}"));
    }

    let text = res.text().await.map_err(|e| e.to_string())?;
    let slim = slim_fah_project_json(&text);
    let raw: serde_json::Value = serde_json::from_str(&slim).map_err(|e| e.to_string())?;
    let data = normalize_project(&raw, project_id);

    CACHE
        .lock()
        .unwrap()
        .insert(project_id, (data.clone(), Instant::now()));
    Ok(Some(data))
}
