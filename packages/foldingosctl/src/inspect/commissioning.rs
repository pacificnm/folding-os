use std::fs;
use std::path::Path;
use std::process::Command;

use regex::Regex;
use std::sync::LazyLock;

use crate::role::read_installation_role_for_display;
use crate::paths::AppliancePaths;

static MANIFEST_RELEASE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*manifest_release\s*=\s*"([^"]+)""#).expect("manifest pattern compiles")
});

pub fn inspect_commissioning(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let role = read_installation_role_for_display(paths);
    let checks = evaluate_commissioning_checks(paths, &role);
    let all_ready = checks.iter().all(|check| check["ready"].as_bool() == Some(true));
    Ok(serde_json::json!({
        "installation_role": role,
        "all_ready": all_ready,
        "checks": checks,
    }))
}

fn evaluate_commissioning_checks(paths: &AppliancePaths, role: &str) -> Vec<serde_json::Value> {
    let mut checks = vec![
        serde_json::json!({"label": "Network online", "ready": true}),
        check_systemd_unit("SSH administrator provisioned", "foldingos-ssh-provision.service"),
        check_installation_role(role),
        check_foldops_packages(paths),
        check_foldops_provisioned(paths),
    ];
    if role == "supervisor" {
        checks.extend([
            check_systemd_unit(
                "FoldOps HTTPS (port 3443)",
                "foldingos-foldops-serve-https.service",
            ),
            check_systemd_unit(
                "FoldOps supervisor (loopback)",
                "foldingos-foldops-supervisor.service",
            ),
            check_systemd_unit("Provisioning control plane", "foldingos-provision.service"),
        ]);
    }
    checks.extend([
        check_systemd_unit("FoldOps agent", "foldingos-foldops-agent.service"),
        check_systemd_unit("Folding@home client", "folding-at-home.service"),
    ]);
    checks
}

fn check_systemd_unit(label: &str, unit: &str) -> serde_json::Value {
    serde_json::json!({
        "label": label,
        "ready": systemd_unit_is_active(unit),
    })
}

fn check_installation_role(role: &str) -> serde_json::Value {
    serde_json::json!({
        "label": "Installation role active",
        "ready": role == "agent" || role == "supervisor",
    })
}

fn check_foldops_packages(paths: &AppliancePaths) -> serde_json::Value {
    serde_json::json!({
        "label": "FoldOps packages acquired",
        "ready": paths.foldops_current_link().exists(),
    })
}

fn check_foldops_provisioned(paths: &AppliancePaths) -> serde_json::Value {
    serde_json::json!({
        "label": "FoldOps provisioned",
        "ready": paths.foldops_provisioned_marker.exists(),
    })
}

fn systemd_unit_is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn parse_manifest_release(content: &str) -> Option<String> {
    MANIFEST_RELEASE_PATTERN
        .captures(content)
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()))
}

pub fn read_current_release(apps_root: &Path) -> Result<String, String> {
    let current_path = apps_root.join("current");
    let target = fs::read_link(&current_path)
        .map_err(|error| format!("read current symlink: {error}"))?;
    let target = target.to_string_lossy();
    if target.starts_with('/') {
        return Err("current must be a relative symlink".into());
    }
    let cleaned = Path::new(target.as_ref())
        .components()
        .fold(String::new(), |mut acc, component| {
            use std::path::Component;
            match component {
                Component::Normal(part) => {
                    if !acc.is_empty() {
                        acc.push('/');
                    }
                    acc.push_str(&part.to_string_lossy());
                }
                Component::ParentDir => acc = String::new(),
                _ => {}
            }
            acc
        });
    if cleaned.is_empty() || cleaned.contains("..") || cleaned != target {
        return Err("current must not contain path traversal".into());
    }
    let release_dir = apps_root.join(&cleaned);
    let metadata = fs::metadata(&release_dir)
        .map_err(|_| "current does not reference an installed release".to_string())?;
    if !metadata.is_dir() {
        return Err("current does not reference an installed release".into());
    }
    Ok(cleaned)
}
