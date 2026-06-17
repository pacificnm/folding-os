use std::fs;

use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;
use crate::provision::util::empty_human_result;

pub fn provision_role(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let provisioned = fs::read(&paths.provisioned_installation_role);
    let active = fs::read(&paths.active_installation_role);

    if let Ok(provisioned_content) = provisioned {
        let role = parse_installation_role(&provisioned_content)?;
        if let Ok(active_content) = active {
            match parse_installation_role(&active_content) {
                Ok(active_role) if active_role == role => {
                    fs::remove_file(&paths.provisioned_installation_role)
                        .map_err(|error| error.to_string())?;
                    println!("Installation role {role:?} is already persisted.");
                    return Ok(empty_human_result());
                }
                Ok(active_role) => {
                    return Err(format!(
                        "provisioned installation role {role:?} conflicts with persisted role {active_role:?}"
                    ));
                }
                Err(_) => {
                    atomic_write(&paths.active_installation_role, role.as_bytes(), 0o644)?;
                    fs::remove_file(&paths.provisioned_installation_role)
                        .map_err(|error| error.to_string())?;
                    println!("Recovered installation role {role:?} from provisioned staging.");
                    return Ok(empty_human_result());
                }
            }
        }
        atomic_write(&paths.active_installation_role, role.as_bytes(), 0o644)?;
        fs::remove_file(&paths.provisioned_installation_role)
            .map_err(|error| error.to_string())?;
        println!("Activated provisioned installation role {role:?}.");
        return Ok(empty_human_result());
    } else if let Err(error) = provisioned {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error.to_string());
        }
    }

    if let Err(error) = active {
        if error.kind() == std::io::ErrorKind::NotFound {
            return Err("installation role is not provisioned".into());
        }
        return Err(error.to_string());
    }

    let active_content = active.unwrap();
    let role = parse_installation_role(&active_content)
        .map_err(|error| format!("persistent installation role is invalid: {error}"))?;
    println!("Validated installation role {role:?}.");
    Ok(empty_human_result())
}

fn parse_installation_role(content: &[u8]) -> Result<String, String> {
    let role = String::from_utf8_lossy(content).trim().to_string();
    if role.is_empty() {
        return Err("installation role is empty".into());
    }
    if role.contains('\n') {
        return Err("installation role must be a single line".into());
    }
    if role != "agent" && role != "supervisor" {
        return Err(format!("unsupported installation role {role:?}"));
    }
    Ok(role)
}

pub(crate) fn parse_installation_role_bytes(content: &[u8]) -> Result<String, String> {
    parse_installation_role(content)
}
