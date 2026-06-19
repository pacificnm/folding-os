use std::fs;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::fs_atomic::{atomic_write, contains_string};
use crate::paths::AppliancePaths;

static UUID_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
        .expect("uuid pattern compiles")
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentRecord {
    pub schema_version: i32,
    pub node_id: String,
    pub installation_role: String,
    pub registered_at: String,
    pub last_seen_at: String,
    pub mac_addresses: Vec<String>,
    pub current_image_version: String,
    pub foldingos_version: String,
    pub hostname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fah_active: Option<bool>,
    pub desired_image_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub desired_foldops_manifest_release: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub desired_tools_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_update_status: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_update_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_update_message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_update_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct EnrollmentIndex {
    schema_version: i32,
    node_ids: Vec<String>,
}

impl EnrollmentIndex {
    pub(crate) fn node_ids(&self) -> &[String] {
        &self.node_ids
    }
}

pub fn load_enrollment_records_sorted(
    paths: &AppliancePaths,
) -> Result<Vec<EnrollmentRecord>, String> {
    let index = load_enrollment_index(paths)?;
    let mut node_ids = index.node_ids().to_vec();
    node_ids.sort();
    let mut records = Vec::with_capacity(node_ids.len());
    for node_id in node_ids {
        records.push(load_enrollment_record(paths, &node_id)?);
    }
    Ok(records)
}

pub fn load_enrollment_record(
    paths: &AppliancePaths,
    node_id: &str,
) -> Result<EnrollmentRecord, String> {
    let content = fs::read_to_string(paths.enrollment_record_path(node_id))
        .map_err(|error| format!("read enrollment record for {node_id}: {error}"))?;
    let record: EnrollmentRecord = serde_json::from_str(&content)
        .map_err(|error| format!("invalid enrollment record for {node_id}: {error}"))?;
    validate_enrollment_record(record)
}

pub fn save_enrollment_record(
    paths: &AppliancePaths,
    record: EnrollmentRecord,
) -> Result<(), String> {
    let validated = validate_enrollment_record(record)?;
    let content = serde_json::to_string_pretty(&validated)
        .map_err(|error| format!("serialize enrollment record: {error}"))?;
    let mut content = content;
    content.push('\n');
    atomic_write(
        &paths.enrollment_record_path(&validated.node_id),
        content.as_bytes(),
        0o644,
    )?;
    let mut index = load_enrollment_index(paths)?;
    if !contains_string(index.node_ids(), &validated.node_id) {
        index.node_ids.push(validated.node_id.clone());
    }
    save_enrollment_index(paths, &index)
}

fn load_enrollment_index(paths: &AppliancePaths) -> Result<EnrollmentIndex, String> {
    match fs::read_to_string(&paths.provision_enrollments_index) {
        Ok(content) => {
            let index: EnrollmentIndex = serde_json::from_str(&content)
                .map_err(|error| format!("invalid enrollment index: {error}"))?;
            if index.schema_version != 1 {
                return Err(format!(
                    "unsupported enrollment index schema version {}",
                    index.schema_version
                ));
            }
            Ok(index)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(EnrollmentIndex {
            schema_version: 1,
            node_ids: Vec::new(),
        }),
        Err(error) => Err(format!("read enrollment index: {error}")),
    }
}

fn save_enrollment_index(paths: &AppliancePaths, index: &EnrollmentIndex) -> Result<(), String> {
    let mut index = index.clone();
    index.schema_version = 1;
    index.node_ids.sort();
    let content = serde_json::to_string_pretty(&index)
        .map_err(|error| format!("serialize enrollment index: {error}"))?;
    let mut content = content;
    content.push('\n');
    atomic_write(
        &paths.provision_enrollments_index,
        content.as_bytes(),
        0o644,
    )
}

fn validate_enrollment_record(mut record: EnrollmentRecord) -> Result<EnrollmentRecord, String> {
    if record.schema_version != 1 {
        return Err(format!(
            "unsupported enrollment schema version {}",
            record.schema_version
        ));
    }
    record.node_id = record.node_id.trim().to_string();
    if !UUID_PATTERN.is_match(&record.node_id) {
        return Err("enrollment record node_id is invalid".into());
    }
    record.installation_role = record.installation_role.trim().to_string();
    if record.installation_role != "agent" {
        return Err(format!(
            "enrollment record role must be agent, found \"{}\"",
            record.installation_role
        ));
    }
    if record.current_image_version.trim().is_empty() {
        return Err("enrollment record missing current_image_version".into());
    }
    if record.foldingos_version.trim().is_empty() {
        return Err("enrollment record missing foldingos_version".into());
    }
    if record.hostname.trim().is_empty() {
        return Err("enrollment record missing hostname".into());
    }
    if record.mac_addresses.is_empty() {
        return Err("enrollment record missing mac_addresses".into());
    }
    record.desired_image_version = record.desired_image_version.trim().to_string();
    if record.desired_image_version.is_empty() {
        record.desired_image_version = "current".into();
    }
    Ok(record)
}

pub fn is_valid_node_id(node_id: &str) -> bool {
    UUID_PATTERN.is_match(node_id.trim())
}

pub(crate) fn load_enrollment_index_internal(
    paths: &AppliancePaths,
) -> Result<EnrollmentIndex, String> {
    load_enrollment_index(paths)
}
