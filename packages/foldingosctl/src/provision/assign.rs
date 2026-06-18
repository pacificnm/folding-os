use crate::assignments::apply_supervisor_local_assignments_if_needed;
use crate::automation_policy::require_supervisor_automation_mutation;
use crate::enrollment::{load_enrollment_record, load_enrollment_records_sorted, save_enrollment_record};
use crate::fs_atomic::contains_string;
use crate::paths::AppliancePaths;
use crate::registry_image::{
    is_bootstrap_assignment_label, load_foldops_registry_entry, load_registry_entry,
    load_tools_registry_entry,
};
use crate::role::require_supervisor_role;

#[derive(Debug, Default, Clone)]
pub struct AssignmentUpdate {
    pub image_version: Option<String>,
    pub foldops_manifest_release: Option<String>,
    pub tools_version: Option<String>,
}

pub fn list_enrollments(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    require_supervisor_role(paths)?;
    let records = load_enrollment_records_sorted(paths)?;
    Ok(serde_json::json!({ "enrollments": records }))
}

pub fn assign(paths: &AppliancePaths, args: &[String]) -> Result<serde_json::Value, String> {
    let parsed = parse_assign_args(args)?;
    let image_version = parsed.update.image_version.clone();
    let foldops_manifest_release = parsed.update.foldops_manifest_release.clone();
    let tools_version = parsed.update.tools_version.clone();
    let updated = assign_software_versions(paths, &parsed.scope, &parsed.node_id, parsed.update)?;
    let mut result = serde_json::json!({
        "scope": parsed.scope,
        "updated_count": updated,
    });
    if !parsed.node_id.is_empty() {
        result["node_id"] = serde_json::Value::String(parsed.node_id);
    }
    if let Some(version) = image_version.as_ref() {
        result["image_version"] = serde_json::Value::String(version.trim().to_string());
    }
    if let Some(release) = foldops_manifest_release.as_ref() {
        result["foldops_manifest_release"] = serde_json::Value::String(release.trim().to_string());
    }
    if let Some(version) = tools_version.as_ref() {
        result["tools_version"] = serde_json::Value::String(version.trim().to_string());
    }
    Ok(result)
}

struct ParsedAssign {
    scope: String,
    node_id: String,
    update: AssignmentUpdate,
}

fn parse_assign_args(args: &[String]) -> Result<ParsedAssign, String> {
    let mut node_id = String::new();
    let mut all = false;
    let mut update = AssignmentUpdate::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--node" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --node".to_string())?;
                node_id = value.clone();
                index += 2;
            }
            "--all" => {
                all = true;
                index += 1;
            }
            "--version" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --version".to_string())?;
                update.image_version = Some(value.clone());
                index += 2;
            }
            "--foldops-manifest" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --foldops-manifest".to_string())?;
                update.foldops_manifest_release = Some(value.clone());
                index += 2;
            }
            "--tools-version" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --tools-version".to_string())?;
                update.tools_version = Some(value.clone());
                index += 2;
            }
            other => return Err(format!("unknown assign option \"{other}\"")),
        }
    }
    if all && !node_id.is_empty() {
        return Err("use either --all or --node, not both".into());
    }
    if !all && node_id.is_empty() {
        return Err("assignment requires --all or --node".into());
    }
    if update.image_version.is_none()
        && update.foldops_manifest_release.is_none()
        && update.tools_version.is_none()
    {
        return Err(
            "assignment requires at least one of --version, --foldops-manifest, or --tools-version"
                .into(),
        );
    }
    Ok(ParsedAssign {
        scope: if all { "fleet".into() } else { "node".into() },
        node_id,
        update,
    })
}

pub(crate) fn assign_software_versions(
    paths: &AppliancePaths,
    scope: &str,
    node_id: &str,
    update: AssignmentUpdate,
) -> Result<i32, String> {
    require_supervisor_role(paths)?;
    require_supervisor_automation_mutation(paths, "provision", "assign")?;
    validate_assignment_update(paths, &update)?;

    let index = crate::enrollment::load_enrollment_index_internal(paths)?;
    if index.node_ids().is_empty() {
        return Err("no enrolled agents are available".into());
    }

    let targets = if scope == "node" {
        if !crate::enrollment::is_valid_node_id(node_id) {
            return Err("node id is invalid".into());
        }
        if !contains_string(index.node_ids(), node_id) {
            return Err("agent is not registered".into());
        }
        vec![node_id.to_string()]
    } else if scope == "fleet" {
        index.node_ids().to_vec()
    } else {
        return Err(format!("unsupported assignment scope \"{scope}\""));
    };

    let mut updated = 0;
    for target in targets {
        let mut record = load_enrollment_record(paths, &target)?;
        if let Some(version) = update.image_version.as_ref() {
            let version = version.trim();
            record.desired_image_version = if version.is_empty() {
                "current".into()
            } else {
                version.to_string()
            };
        }
        if let Some(release) = update.foldops_manifest_release.as_ref() {
            let release = release.trim();
            record.desired_foldops_manifest_release = if is_bootstrap_assignment_label(release) {
                String::new()
            } else {
                release.to_string()
            };
        }
        if let Some(version) = update.tools_version.as_ref() {
            let version = version.trim();
            record.desired_tools_version = if is_bootstrap_assignment_label(version) {
                String::new()
            } else {
                version.to_string()
            };
        }
        save_enrollment_record(paths, record.clone())?;
        apply_supervisor_local_assignments_if_needed(paths, scope, &target, &record)?;
        updated += 1;
    }
    Ok(updated)
}

fn validate_assignment_update(paths: &AppliancePaths, update: &AssignmentUpdate) -> Result<(), String> {
    if let Some(version) = update.image_version.as_ref() {
        let version = version.trim();
        if version.is_empty() {
            return Err("assigned image version is required".into());
        }
        if version != "current" {
            let entry = load_registry_entry(paths, version)?;
            if entry.rollout_state != "ready" {
                return Err(format!(
                    "assigned image version \"{version}\" is not ready for rollout"
                ));
            }
        }
    }
    if let Some(release) = update.foldops_manifest_release.as_ref() {
        let release = release.trim();
        if !release.is_empty() && !is_bootstrap_assignment_label(release) {
            let entry = load_foldops_registry_entry(paths, release)?;
            if entry.rollout_state != "ready" {
                return Err(format!(
                    "assigned foldops manifest \"{release}\" is not ready for rollout"
                ));
            }
        }
    }
    if let Some(version) = update.tools_version.as_ref() {
        let version = version.trim();
        if !version.is_empty() && !is_bootstrap_assignment_label(version) {
            let entry = load_tools_registry_entry(paths, version)?;
            if entry.rollout_state != "ready" {
                return Err(format!(
                    "assigned tools version \"{version}\" is not ready for rollout"
                ));
            }
        }
    }
    Ok(())
}
