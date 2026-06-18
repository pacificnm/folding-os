use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use ureq::Agent;

use crate::paths::AppliancePaths;
use crate::registry_image::{
    current_import_timestamp, load_registry_entry, read_upstream_releases_url,
    registry_image_path, save_registry_entry, RegistryEntry,
};
use crate::role::require_supervisor_role;

static SHA256_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-f]{64}$").expect("sha256 pattern compiles"));

#[derive(Debug, Deserialize)]
struct UpstreamReleasesManifest {
    schema_version: i32,
    releases: Vec<UpstreamRelease>,
}

#[derive(Debug, Deserialize)]
struct UpstreamRelease {
    foldingos_version: String,
    git_revision: String,
    image_url: String,
    image_sha256: String,
    image_size_bytes: i64,
}

fn registry_http_agent() -> Agent {
    ureq::AgentBuilder::new().redirects(0).build()
}

pub fn poll(paths: &AppliancePaths) -> Result<(), String> {
    require_supervisor_role(paths)?;

    let upstream_url = read_upstream_releases_url(paths)?;
    let Some(upstream_url) = upstream_url else {
        println!("Upstream releases URL is not configured; polling skipped.");
        return Ok(());
    };

    let manifest = fetch_upstream_manifest(&upstream_url)?;
    let mut imported = 0;
    for release in manifest.releases {
        if import_upstream_release(paths, release)? {
            imported += 1;
        }
    }
    if imported == 0 {
        println!("Upstream poll completed; no new verified images were imported.");
    } else {
        println!("Upstream poll imported {imported} verified image(s).");
    }
    Ok(())
}

fn fetch_upstream_manifest(url: &str) -> Result<UpstreamReleasesManifest, String> {
    let agent = registry_http_agent();
    let response = agent
        .get(url)
        .call()
        .map_err(|error| error.to_string())?;
    if response.status() != 200 {
        return Err(format!(
            "upstream manifest request failed with status {}",
            response.status()
        ));
    }
    let mut body = Vec::new();
    response
        .into_reader()
        .take(1 << 20)
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    let manifest: UpstreamReleasesManifest = serde_json::from_slice(&body)
        .map_err(|error| format!("invalid upstream releases manifest: {error}"))?;
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported upstream manifest schema version {}",
            manifest.schema_version
        ));
    }
    Ok(manifest)
}

fn import_upstream_release(paths: &AppliancePaths, mut release: UpstreamRelease) -> Result<bool, String> {
    release.foldingos_version = release.foldingos_version.trim().to_string();
    release.git_revision = release.git_revision.trim().to_string();
    release.image_url = release.image_url.trim().to_string();
    release.image_sha256 = release.image_sha256.trim().to_lowercase();

    if release.foldingos_version.is_empty()
        || release.git_revision.is_empty()
        || release.image_url.is_empty()
    {
        return Err("upstream release entry is incomplete".into());
    }
    if !release.image_url.starts_with("https://") {
        return Err(format!(
            "upstream image URL must use HTTPS: {:?}",
            release.image_url
        ));
    }
    if !SHA256_PATTERN.is_match(&release.image_sha256) {
        return Err("upstream release image_sha256 is invalid".into());
    }
    if release.image_size_bytes <= 0 {
        return Err("upstream release image_size_bytes must be positive".into());
    }

    match load_registry_entry(paths, &release.foldingos_version) {
        Ok(existing) => {
            if existing.image_sha256 == release.image_sha256 {
                return Ok(false);
            }
            return Err(format!(
                "registry already contains version {} with a different image digest",
                release.foldingos_version
            ));
        }
        Err(error) if error.contains("No such file") || error.contains("not found") => {}
        Err(error) => return Err(error),
    }

    let image_path = registry_image_path(paths, &release.foldingos_version);
    if let Some(parent) = image_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create registry images dir: {error}"))?;
    }
    if let Err(error) = download_verified_registry_image(&release, &image_path) {
        return Err(error);
    }

    let entry = RegistryEntry {
        schema_version: 1,
        foldingos_version: release.foldingos_version.clone(),
        git_revision: release.git_revision,
        image_sha256: release.image_sha256,
        image_size_bytes: release.image_size_bytes,
        retrieval_url: release.image_url,
        verification_method: "sha256".into(),
        import_timestamp: current_import_timestamp(),
        rollout_state: "ready".into(),
        local_image_path: image_path.to_string_lossy().into_owned(),
    };
    if let Err(error) = save_registry_entry(paths, entry) {
        let _ = fs::remove_file(&image_path);
        return Err(error);
    }
    println!(
        "Imported verified upstream image for FoldingOS {}.",
        release.foldingos_version
    );
    Ok(true)
}

fn download_verified_registry_image(
    release: &UpstreamRelease,
    destination: &Path,
) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "destination path has no parent".to_string())?;
    let temp_path = parent.join(format!(".registry-download.tmp-{}", std::process::id()));
    let mut cleanup = true;
    let result = (|| -> Result<(), String> {
        let mut temp = File::create(&temp_path).map_err(|error| error.to_string())?;
        let agent = registry_http_agent();
        let response = agent
            .get(&release.image_url)
            .call()
            .map_err(|error| error.to_string())?;
        if response.status() != 200 {
            return Err(format!(
                "image download failed with status {}",
                response.status()
            ));
        }
        let mut reader = response.into_reader();
        let mut hasher = Sha256::new();
        let mut written = 0_i64;
        let mut buffer = vec![0_u8; 8192];
        while written < release.image_size_bytes {
            let remaining = (release.image_size_bytes - written) as usize;
            let chunk = remaining.min(buffer.len());
            let read = reader
                .read(&mut buffer[..chunk])
                .map_err(|error| format!("download image: {error}"))?;
            if read == 0 {
                return Err("download image: unexpected EOF".into());
            }
            temp.write_all(&buffer[..read])
                .map_err(|error| error.to_string())?;
            hasher.update(&buffer[..read]);
            written += read as i64;
        }
        if written != release.image_size_bytes {
            return Err(format!(
                "downloaded image size {written} does not match expected {}",
                release.image_size_bytes
            ));
        }
        let mut extra = [0_u8; 1];
        if reader.read(&mut extra).unwrap_or(0) > 0 {
            return Err("downloaded image exceeds declared size".into());
        }
        temp.sync_all().map_err(|error| error.to_string())?;
        drop(temp);
        let actual = format!("{:x}", hasher.finalize());
        if actual != release.image_sha256 {
            return Err("downloaded image failed SHA-256 verification".into());
        }
        fs::rename(&temp_path, destination).map_err(|error| error.to_string())?;
        cleanup = false;
        Ok(())
    })();
    if cleanup {
        let _ = fs::remove_file(&temp_path);
    }
    result
}
