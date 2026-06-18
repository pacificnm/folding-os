use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::Config;
use crate::foldingos::{self, AssignLocalRequest, FleetDelegateConfig};
use crate::software::upstream::{fetch_channel_index, FoldopsReleaseEntry, UpstreamError};

#[derive(Debug, Deserialize)]
pub struct AssignLocalBody {
    pub foldops_manifest: Option<String>,
    pub tools_version: Option<String>,
}

pub async fn assign_local(
    config: &Config,
    body: AssignLocalBody,
) -> Result<Value, String> {
    if !config.uses_supervisor_fleet_delegation() {
        return Err(
            "FoldingOS fleet delegation is unavailable on this host (requires supervisor role with foldingosctl)"
                .into(),
        );
    }

    let foldops_manifest = trim_optional(body.foldops_manifest);
    let tools_version = trim_optional(body.tools_version);
    if foldops_manifest.is_none() && tools_version.is_none() {
        return Err(
            "Local assignment requires foldops_manifest and/or tools_version".into(),
        );
    }

    let delegate = FleetDelegateConfig {
        foldingosctl_path: &config.foldingosctl_path,
    };

    if let Some(release) = foldops_manifest.as_deref() {
        ensure_foldops_release_imported(config, release).await?;
    }

    let result = foldingos::provision_assign_local(
        delegate,
        AssignLocalRequest {
            foldops_manifest_release: foldops_manifest,
            tools_version,
        },
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(json!({ "ok": true, "result": result }))
}

pub async fn ensure_foldops_release_imported(
    config: &Config,
    release: &str,
) -> Result<(), String> {
    let release = release.trim();
    if release.is_empty() {
        return Err("FoldOps manifest release is required".into());
    }

    let delegate = FleetDelegateConfig {
        foldingosctl_path: &config.foldingosctl_path,
    };

    let manifest_url = lookup_foldops_manifest_url(config, release).await?;
    foldingos::registry_import_foldops_manifest_url(delegate, &manifest_url)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn lookup_foldops_manifest_url(
    config: &Config,
    release: &str,
) -> Result<String, String> {
    let releases: Vec<FoldopsReleaseEntry> =
        fetch_channel_index(crate::software::upstream::ChannelKind::Foldops, &config.packages_foldops_index_url)
            .await
            .map_err(map_upstream_error)?;

    releases
        .into_iter()
        .find(|entry| entry.manifest_release == release)
        .map(|entry| entry.manifest_url)
        .ok_or_else(|| {
            format!(
                "FoldOps release {release} was not found in {}",
                config.packages_foldops_index_url
            )
        })
}

fn map_upstream_error(error: UpstreamError) -> String {
    error.to_string()
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
