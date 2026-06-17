use crate::role::read_active_installation_role;
use crate::paths::AppliancePaths;

pub fn current_unix_username() -> Option<String> {
    users::get_current_username().map(|name| name.to_string_lossy().into_owned())
}

pub fn is_foldops_automation_user() -> bool {
    current_unix_username().as_deref() == Some("foldops")
}

pub fn require_inspectable_role(paths: &AppliancePaths) -> Result<(), String> {
    if is_foldops_automation_user() {
        return Ok(());
    }
    let role = read_active_installation_role(paths)?;
    if role == "agent" || role == "supervisor" {
        return Ok(());
    }
    Err(format!(
        "operation requires agent or supervisor role, found \"{role}\""
    ))
}
