use crate::config::Config;
use crate::foldingos::{self, FleetDelegateConfig};
use crate::install_log;
use crate::software::upstream::{
    fetch_channel_index, FoldopsReleaseEntry, ToolsReleaseEntry, UpstreamError,
};

pub async fn ensure_foldops_release_imported(config: &Config, release: &str) -> Result<(), String> {
    let release = release.trim();
    if release.is_empty() {
        return Err("FoldOps manifest release is required".into());
    }

    let delegate = FleetDelegateConfig {
        foldingosctl_path: &config.foldingosctl_path,
    };

    let manifest_url = lookup_foldops_manifest_url(config, release)
        .await
        .map_err(|error| {
            let message = error.to_string();
            install_log::append_event(
                "import",
                "foldops-index",
                &config.packages_foldops_index_url,
                false,
                None,
                &message,
                "",
                "",
                Some(serde_json::json!({ "release": release })),
            );
            message
        })?;
    foldingos::registry_import_foldops_manifest_url(delegate, &manifest_url)
        .await
        .map_err(|error| {
            let message = error.to_string();
            install_log::append_event(
                "import",
                "foldops-manifest",
                &manifest_url,
                false,
                None,
                &message,
                "",
                "",
                Some(serde_json::json!({ "release": release })),
            );
            message
        })?;
    Ok(())
}

pub async fn ensure_tools_release_imported(config: &Config, version: &str) -> Result<(), String> {
    let version = version.trim();
    if version.is_empty() {
        return Err("Tools version is required".into());
    }

    let delegate = FleetDelegateConfig {
        foldingosctl_path: &config.foldingosctl_path,
    };

    let (binary_url, sha256_url) =
        lookup_tools_release_urls(config, version)
            .await
            .map_err(|error| {
                let message = error.to_string();
                install_log::append_event(
                    "import",
                    "tools-index",
                    &config.packages_tools_index_url,
                    false,
                    None,
                    &message,
                    "",
                    "",
                    Some(serde_json::json!({ "tools_version": version })),
                );
                message
            })?;
    foldingos::registry_import_tools_release_urls(delegate, version, &binary_url, &sha256_url)
        .await
        .map_err(|error| {
            let message = error.to_string();
            install_log::append_event(
                "import",
                "tools-release",
                &binary_url,
                false,
                None,
                &message,
                "",
                "",
                Some(serde_json::json!({
                    "tools_version": version,
                    "sha256_url": sha256_url,
                })),
            );
            message
        })?;
    Ok(())
}

async fn lookup_tools_release_urls(
    config: &Config,
    version: &str,
) -> Result<(String, String), String> {
    let releases: Vec<ToolsReleaseEntry> = fetch_channel_index(
        crate::software::upstream::ChannelKind::Tools,
        &config.packages_tools_index_url,
    )
    .await
    .map_err(map_upstream_error)?;

    releases
        .into_iter()
        .find(|entry| entry.tools_version == version)
        .map(|entry| (entry.binary_url, entry.sha256_url))
        .ok_or_else(|| {
            format!(
                "Tools release {version} was not found in {}",
                config.packages_tools_index_url
            )
        })
}

async fn lookup_foldops_manifest_url(config: &Config, release: &str) -> Result<String, String> {
    let releases: Vec<FoldopsReleaseEntry> = fetch_channel_index(
        crate::software::upstream::ChannelKind::Foldops,
        &config.packages_foldops_index_url,
    )
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
