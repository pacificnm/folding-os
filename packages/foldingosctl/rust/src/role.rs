use std::fs;
use std::path::Path;

use crate::paths::AppliancePaths;

pub fn read_active_installation_role(paths: &AppliancePaths) -> Result<String, String> {
    parse_installation_role_file(&paths.active_installation_role)
}

pub fn read_installation_role_for_display(paths: &AppliancePaths) -> String {
    read_active_installation_role(paths).unwrap_or_else(|_| "unknown".into())
}

fn parse_installation_role_file(path: &Path) -> Result<String, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("read installation role: {error}"))?;
    let role = content.trim();
    if role.is_empty() {
        return Err("installation role is empty".into());
    }
    if role.contains('\n') {
        return Err("installation role must be a single line".into());
    }
    if role != "agent" && role != "supervisor" {
        return Err(format!("unsupported installation role \"{role}\""));
    }
    Ok(role.to_string())
}
