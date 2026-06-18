use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

const CACHE_TTL: Duration = Duration::from_secs(60);
const REFRESH_MIN_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    Foldops,
    Tools,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct FoldopsReleaseEntry {
    pub manifest_release: String,
    pub published_at: String,
    pub manifest_url: String,
    pub minimum_foldingos_version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ToolsReleaseEntry {
    pub tools_version: String,
    pub published_at: String,
    pub binary_url: String,
    pub sha256_url: String,
    pub minimum_foldingos_version: String,
}

#[derive(Debug, Clone)]
pub struct FoldopsLatest {
    pub manifest_release: String,
    pub published_at: String,
}

#[derive(Debug, Clone)]
pub struct ToolsLatest {
    pub tools_version: String,
    pub published_at: String,
}

#[derive(Debug, Clone)]
pub struct UpstreamLatest {
    pub foldops: Option<FoldopsLatest>,
    pub tools: Option<ToolsLatest>,
}

#[derive(Debug, Clone)]
pub struct UpstreamIndexes {
    pub foldops: Vec<FoldopsReleaseEntry>,
    pub tools: Vec<ToolsReleaseEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    #[error("unsupported index schema_version: {0}")]
    UnsupportedSchema(i64),
    #[error("index channel mismatch: expected {expected}, got {actual}")]
    ChannelMismatch { expected: String, actual: String },
    #[error("failed to fetch {channel} index: {message}")]
    FetchFailed { channel: &'static str, message: String },
    #[error("failed to parse {channel} index JSON: {message}")]
    InvalidJson { channel: &'static str, message: String },
}

#[derive(Default)]
pub struct UpstreamCache {
    foldops: Option<CachedIndex<FoldopsReleaseEntry>>,
    tools: Option<CachedIndex<ToolsReleaseEntry>>,
    last_refresh_at: Option<Instant>,
}

struct CachedIndex<T> {
    fetched_at: Instant,
    releases: Vec<T>,
}

impl UpstreamCache {
    pub async fn load_indexes(
        &mut self,
        foldops_url: &str,
        tools_url: &str,
        refresh: bool,
    ) -> Result<UpstreamIndexes, UpstreamError> {
        let now = Instant::now();
        let allow_refresh = refresh
            && self
                .last_refresh_at
                .map(|at| now.duration_since(at) >= REFRESH_MIN_INTERVAL)
                .unwrap_or(true);

        if allow_refresh {
            self.last_refresh_at = Some(now);
        }

        let foldops = Self::load_channel(
            ChannelKind::Foldops,
            foldops_url,
            allow_refresh || self.foldops.is_none(),
            &mut self.foldops,
        )
        .await?;
        let tools = Self::load_channel(
            ChannelKind::Tools,
            tools_url,
            allow_refresh || self.tools.is_none(),
            &mut self.tools,
        )
        .await?;

        Ok(UpstreamIndexes { foldops, tools })
    }

    async fn load_channel<T>(
        channel: ChannelKind,
        url: &str,
        fetch: bool,
        slot: &mut Option<CachedIndex<T>>,
    ) -> Result<Vec<T>, UpstreamError>
    where
        T: for<'de> Deserialize<'de> + Clone,
    {
        if fetch || slot.is_none() {
            let releases = fetch_channel_index(channel, url).await?;
            *slot = Some(CachedIndex {
                fetched_at: Instant::now(),
                releases: releases.clone(),
            });
            return Ok(releases);
        }

        if let Some(cached) = slot {
            if cached.fetched_at.elapsed() <= CACHE_TTL {
                return Ok(cached.releases.clone());
            }
        }

        let releases = fetch_channel_index(channel, url).await?;
        *slot = Some(CachedIndex {
            fetched_at: Instant::now(),
            releases: releases.clone(),
        });
        Ok(releases)
    }
}

pub async fn fetch_channel_index<T>(
    channel: ChannelKind,
    url: &str,
) -> Result<Vec<T>, UpstreamError>
where
    T: for<'de> Deserialize<'de>,
{
    let channel_name = channel_name(channel);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| UpstreamError::FetchFailed {
            channel: channel_name,
            message: error.to_string(),
        })?;

    let response = client.get(url).send().await.map_err(|error| UpstreamError::FetchFailed {
        channel: channel_name,
        message: error.to_string(),
    })?;

    if !response.status().is_success() {
        return Err(UpstreamError::FetchFailed {
            channel: channel_name,
            message: format!("HTTP {}", response.status()),
        });
    }

    let body: Value = response.json().await.map_err(|error| UpstreamError::InvalidJson {
        channel: channel_name,
        message: error.to_string(),
    })?;

    parse_channel_index(channel, body)
}

fn parse_channel_index<T>(channel: ChannelKind, body: Value) -> Result<Vec<T>, UpstreamError>
where
    T: for<'de> Deserialize<'de>,
{
    let channel_name = channel_name(channel);
    let schema_version = body
        .get("schema_version")
        .and_then(|value| value.as_i64())
        .unwrap_or(-1);
    if schema_version != 1 {
        return Err(UpstreamError::UnsupportedSchema(schema_version));
    }

    let expected = match channel {
        ChannelKind::Foldops => "foldops",
        ChannelKind::Tools => "foldingos-tools",
    };
    let actual = body
        .get("channel")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if actual != expected {
        return Err(UpstreamError::ChannelMismatch {
            expected: expected.into(),
            actual: actual.into(),
        });
    }

    body.get("releases")
        .cloned()
        .unwrap_or(Value::Array(vec![]))
        .as_array()
        .ok_or_else(|| UpstreamError::InvalidJson {
            channel: channel_name,
            message: "releases must be an array".into(),
        })
        .and_then(|entries| {
            serde_json::from_value(Value::Array(entries.clone())).map_err(|error| {
                UpstreamError::InvalidJson {
                    channel: channel_name,
                    message: error.to_string(),
                }
            })
        })
}

pub fn select_foldops_latest(
    releases: &[FoldopsReleaseEntry],
    foldingos_version: &str,
) -> Option<FoldopsLatest> {
    releases
        .iter()
        .filter(|entry| {
            foldingos_version_satisfies(foldingos_version, &entry.minimum_foldingos_version)
        })
        .max_by(|left, right| {
            version_cmp(&left.manifest_release, &right.manifest_release)
                .then_with(|| left.published_at.cmp(&right.published_at))
        })
        .map(|entry| FoldopsLatest {
            manifest_release: entry.manifest_release.clone(),
            published_at: entry.published_at.clone(),
        })
}

pub fn select_tools_latest(
    releases: &[ToolsReleaseEntry],
    foldingos_version: &str,
) -> Option<ToolsLatest> {
    releases
        .iter()
        .filter(|entry| {
            foldingos_version_satisfies(foldingos_version, &entry.minimum_foldingos_version)
        })
        .max_by(|left, right| {
            version_cmp(&left.tools_version, &right.tools_version)
                .then_with(|| left.published_at.cmp(&right.published_at))
        })
        .map(|entry| ToolsLatest {
            tools_version: entry.tools_version.clone(),
            published_at: entry.published_at.clone(),
        })
}

pub fn version_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    match (parse_version_label(left), parse_version_label(right)) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => left.cmp(right),
    }
}

pub fn version_gt(left: &str, right: &str) -> bool {
    version_cmp(left, right) == std::cmp::Ordering::Greater
}

pub fn foldingos_version_satisfies(node_version: &str, minimum: &str) -> bool {
    match (
        parse_version_label(node_version),
        parse_version_label(minimum),
    ) {
        (Some(node), Some(minimum)) => node >= minimum,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => true,
    }
}

pub fn update_available(latest: &str, active: &str, assigned: &str) -> bool {
    if !latest.is_empty() && !active.is_empty() && version_gt(latest, active) {
        return true;
    }
    !assigned.is_empty() && !active.is_empty() && assigned != active
}

pub fn apply_pending(assigned: &str, active: &str) -> bool {
    !assigned.is_empty() && !active.is_empty() && assigned != active
}

fn parse_version_label(value: &str) -> Option<Vec<u32>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let (base, suffix) = match value.split_once('-') {
        Some((base, suffix)) => (base, suffix),
        None => (value, ""),
    };

    let mut parts: Vec<u32> = base
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;

    if parts.len() < 3 {
        return None;
    }
    parts.truncate(3);

    let suffix_num = if suffix.is_empty() {
        0
    } else {
        suffix.parse().ok()?
    };
    parts.push(suffix_num);
    Some(parts)
}

fn channel_name(channel: ChannelKind) -> &'static str {
    match channel {
        ChannelKind::Foldops => "foldops",
        ChannelKind::Tools => "foldingos-tools",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_cmp_orders_manifest_releases() {
        assert_eq!(
            version_cmp("0.1.0-2", "0.1.0-1"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            version_cmp("0.1.1", "0.1.0"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn select_latest_respects_minimum_foldingos_version() {
        let releases = vec![
            FoldopsReleaseEntry {
                manifest_release: "0.2.0-1".into(),
                published_at: "2026-06-18T12:00:00Z".into(),
                manifest_url: "https://example/manifest.toml".into(),
                minimum_foldingos_version: "0.2.0".into(),
            },
            FoldopsReleaseEntry {
                manifest_release: "0.1.0-2".into(),
                published_at: "2026-06-18T11:00:00Z".into(),
                manifest_url: "https://example/manifest-2.toml".into(),
                minimum_foldingos_version: "0.1.0".into(),
            },
        ];

        let latest = select_foldops_latest(&releases, "0.1.0").expect("latest");
        assert_eq!(latest.manifest_release, "0.1.0-2");
    }

    #[test]
    fn update_available_when_upstream_newer_or_assignment_pending() {
        assert!(update_available("0.1.0-2", "0.1.0-1", "0.1.0-1"));
        assert!(update_available("0.1.0-2", "0.1.0-1", "0.1.0-2"));
        assert!(!update_available("0.1.0-1", "0.1.0-1", "0.1.0-1"));
    }

    #[test]
    fn parse_channel_index_validates_schema() {
        let body = serde_json::json!({
            "schema_version": 1,
            "channel": "foldops",
            "releases": [{
                "manifest_release": "0.1.0-2",
                "published_at": "2026-06-18T12:00:00Z",
                "manifest_url": "https://example/manifest.toml",
                "minimum_foldingos_version": "0.1.0"
            }]
        });
        let releases: Vec<FoldopsReleaseEntry> =
            parse_channel_index(ChannelKind::Foldops, body).expect("parse");
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].manifest_release, "0.1.0-2");
    }
}
