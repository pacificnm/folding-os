use crate::automation_policy::require_inspectable_role;
use crate::paths::AppliancePaths;

pub fn inspect_services(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    require_inspectable_role(paths)?;
    crate::services::inspect_services(paths)
}
