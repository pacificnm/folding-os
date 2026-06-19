use crate::paths::AppliancePaths;
use crate::process::{
    run_command, schedule_deferred_shell_command, schedule_deferred_systemd_restart,
    schedule_deferred_systemd_restart_after,
};
use crate::role::read_active_installation_role;

const FOLDOPS_SERVE_HTTPS_SERVICE: &str = "foldingos-foldops-serve-https.service";
const FOLDOPS_SUPERVISOR_SERVICE: &str = "foldingos-foldops-supervisor.service";
const FOLDOPS_AGENT_SERVICE: &str = "foldingos-foldops-agent.service";
const FOLDOPS_PROVISION_SERVICE: &str = "foldingos-foldops-provision.service";
const FOLDOPS_ACQUIRE_SERVICE: &str = "foldingos-foldops-acquire.service";
const PROVISION_SERVICE: &str = "foldingos-provision.service";
const PROVISION_BOOT_SERVICE: &str = "foldingos-provision-boot.service";
const REGISTRY_POLL_SERVICE: &str = "foldingos-registry-poll.service";
const FAH_SERVICE: &str = "folding-at-home.service";
const FAH_PREPARE_SERVICE: &str = "foldingos-fah-prepare.service";
const FAH_ACQUIRE_SERVICE: &str = "foldingos-fah-acquire.service";
const AGENT_REGISTER_SERVICE: &str = "foldingos-agent-register.service";
const AGENT_VERSION_CHECK_SERVICE: &str = "foldingos-agent-version-check.service";
const AGENT_APPLY_UPDATE_SERVICE: &str = "foldingos-agent-apply-update.service";

#[derive(Debug, Clone, Copy)]
struct ManagedService {
    unit: &'static str,
    name: &'static str,
    supervisor_only: bool,
    agent_only: bool,
    restartable: bool,
    /// Long-running units included in "restart all". One-shot jobs are listed but omitted from bulk restart.
    include_in_restart_all: bool,
}

const MANAGED_SERVICES: &[ManagedService] = &[
    ManagedService {
        unit: FOLDOPS_PROVISION_SERVICE,
        name: "FoldOps provision",
        supervisor_only: true,
        agent_only: false,
        restartable: true,
        include_in_restart_all: false,
    },
    ManagedService {
        unit: FOLDOPS_ACQUIRE_SERVICE,
        name: "FoldOps package acquire",
        supervisor_only: false,
        agent_only: false,
        restartable: true,
        include_in_restart_all: false,
    },
    ManagedService {
        unit: PROVISION_SERVICE,
        name: "Provisioning API",
        supervisor_only: true,
        agent_only: false,
        restartable: true,
        include_in_restart_all: true,
    },
    ManagedService {
        unit: PROVISION_BOOT_SERVICE,
        name: "Network boot assistance",
        supervisor_only: true,
        agent_only: false,
        restartable: true,
        include_in_restart_all: true,
    },
    ManagedService {
        unit: REGISTRY_POLL_SERVICE,
        name: "Registry release poll",
        supervisor_only: true,
        agent_only: false,
        restartable: true,
        include_in_restart_all: false,
    },
    ManagedService {
        unit: FOLDOPS_SERVE_HTTPS_SERVICE,
        name: "FoldOps HTTPS (port 3443)",
        supervisor_only: true,
        agent_only: false,
        restartable: true,
        include_in_restart_all: false,
    },
    ManagedService {
        unit: FOLDOPS_SUPERVISOR_SERVICE,
        name: "FoldOps supervisor (loopback)",
        supervisor_only: true,
        agent_only: false,
        restartable: true,
        include_in_restart_all: true,
    },
    ManagedService {
        unit: FOLDOPS_AGENT_SERVICE,
        name: "FoldOps agent",
        supervisor_only: false,
        agent_only: false,
        restartable: true,
        include_in_restart_all: true,
    },
    ManagedService {
        unit: AGENT_REGISTER_SERVICE,
        name: "Agent enrollment",
        supervisor_only: false,
        agent_only: true,
        restartable: true,
        include_in_restart_all: false,
    },
    ManagedService {
        unit: AGENT_VERSION_CHECK_SERVICE,
        name: "Agent version check",
        supervisor_only: false,
        agent_only: true,
        restartable: true,
        include_in_restart_all: false,
    },
    ManagedService {
        unit: AGENT_APPLY_UPDATE_SERVICE,
        name: "Apply staged OS update",
        supervisor_only: false,
        agent_only: true,
        restartable: true,
        include_in_restart_all: false,
    },
    ManagedService {
        unit: FAH_ACQUIRE_SERVICE,
        name: "FAH client acquire",
        supervisor_only: false,
        agent_only: false,
        restartable: true,
        include_in_restart_all: false,
    },
    ManagedService {
        unit: FAH_PREPARE_SERVICE,
        name: "FAH config prepare",
        supervisor_only: false,
        agent_only: false,
        restartable: true,
        include_in_restart_all: false,
    },
    ManagedService {
        unit: FAH_SERVICE,
        name: "Folding@home client",
        supervisor_only: false,
        agent_only: false,
        restartable: true,
        include_in_restart_all: true,
    },
];

const SUPERVISOR_RESTART_ALL_ORDER: &[&str] = &[
    FOLDOPS_SUPERVISOR_SERVICE,
    FOLDOPS_AGENT_SERVICE,
    PROVISION_SERVICE,
    PROVISION_BOOT_SERVICE,
    FAH_SERVICE,
];

const AGENT_RESTART_ALL_ORDER: &[&str] = &[
    FOLDOPS_AGENT_SERVICE,
    FAH_SERVICE,
];

pub fn inspect_services(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let role = crate::role::read_installation_role_for_display(paths);
    let services = managed_services_for_role(&role)
        .into_iter()
        .map(|service| service_json(service))
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "installation_role": role,
        "services": services,
    }))
}

pub fn run(paths: &AppliancePaths, subcommand: &str, args: &[String]) -> Result<serde_json::Value, String> {
    match subcommand {
        "restart" => {
            let unit = args.first().ok_or_else(|| "missing service unit name".to_string())?;
            if args.len() > 1 {
                return Err(format!("unexpected services restart argument {:?}", args[1]));
            }
            restart_service(paths, unit)
        }
        "restart-all" => {
            if args.first().map(String::as_str) == Some("--apply") {
                if args.len() > 1 {
                    return Err(format!("unexpected services restart-all argument {:?}", args[1]));
                }
                return restart_all_services(paths);
            }
            if !args.is_empty() {
                return Err(format!("unexpected services restart-all argument {:?}", args[0]));
            }
            schedule_restart_all_services(paths)
        }
        other => Err(format!("unknown services subcommand {other:?}")),
    }
}

pub fn restart_service(paths: &AppliancePaths, unit: &str) -> Result<serde_json::Value, String> {
    let role = read_active_installation_role(paths)?;
    let service = find_restartable_service(&role, unit)?;
    restart_unit(service.unit)?;
    Ok(serde_json::json!({
        "unit": service.unit,
        "name": service.name,
        "restarted": true,
        "message": format!("Restarted {}.", service.name),
    }))
}

pub fn restart_all_services(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let role = read_active_installation_role(paths)?;
    let order = match role.as_str() {
        "supervisor" => SUPERVISOR_RESTART_ALL_ORDER,
        "agent" => AGENT_RESTART_ALL_ORDER,
        other => {
            return Err(format!(
                "operation requires agent or supervisor role, found \"{other}\""
            ));
        }
    };

    let mut restarted = Vec::new();
    for unit in order {
        if !service_in_restart_all(&role, unit) {
            continue;
        }
        restart_runtime_unit(unit)?;
        restarted.push(unit.to_string());
    }

    if role == "supervisor" && unit_is_loaded(FOLDOPS_SERVE_HTTPS_SERVICE) {
        schedule_deferred_systemd_restart_after(FOLDOPS_SERVE_HTTPS_SERVICE, 3)?;
        restarted.push(FOLDOPS_SERVE_HTTPS_SERVICE.to_string());
    }

    Ok(serde_json::json!({
        "restarted": restarted,
        "count": restarted.len(),
        "message": format!(
            "Restarted {} services. HTTPS reconnects automatically after a few seconds.",
            restarted.len()
        ),
    }))
}

fn schedule_restart_all_services(_paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    let script = "sleep 1; /usr/bin/foldingosctl services restart-all --apply --format json >/dev/null 2>&1";
    schedule_deferred_shell_command("services-restart-all", script)?;
    Ok(serde_json::json!({
        "scheduled": true,
        "restarted": [],
        "count": 0,
        "message": "Service restarts scheduled. The dashboard may disconnect briefly while services restart.",
    }))
}

fn managed_services_for_role(role: &str) -> Vec<&'static ManagedService> {
    MANAGED_SERVICES
        .iter()
        .filter(|service| match role {
            "supervisor" => !service.agent_only,
            "agent" => !service.supervisor_only,
            _ => false,
        })
        .collect()
}

fn find_restartable_service(role: &str, unit: &str) -> Result<&'static ManagedService, String> {
    managed_services_for_role(role)
        .into_iter()
        .find(|service| service.unit == unit)
        .filter(|service| service.restartable)
        .ok_or_else(|| format!("service {unit:?} is not managed or cannot be restarted"))
}

fn service_json(service: &ManagedService) -> serde_json::Value {
    let (status, loaded) = unit_status(service.unit);
    serde_json::json!({
        "unit": service.unit,
        "name": service.name,
        "status": status,
        "loaded": loaded,
        "restartable": service.restartable,
    })
}

fn unit_status(unit: &str) -> (String, bool) {
    let status = crate::process::command_output("systemctl", &["is-active", unit])
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let loaded = crate::process::command_output("systemctl", &["show", "-p", "LoadState", "--value", unit])
        .map(|value| value.trim() == "loaded")
        .unwrap_or(false);
    (status, loaded)
}

fn service_in_restart_all(role: &str, unit: &str) -> bool {
    managed_services_for_role(role)
        .into_iter()
        .any(|service| service.unit == unit && service.include_in_restart_all)
}

fn unit_is_loaded(unit: &str) -> bool {
    crate::process::command_output("systemctl", &["show", "-p", "LoadState", "--value", unit])
        .map(|value| value.trim() == "loaded")
        .unwrap_or(false)
}

fn restart_runtime_unit(unit: &str) -> Result<(), String> {
    run_command("systemctl", &["restart", unit]).map_err(|error| format!("restart {unit}: {error}"))
}

fn restart_unit(unit: &str) -> Result<(), String> {
    if unit == FOLDOPS_SERVE_HTTPS_SERVICE {
        return schedule_deferred_systemd_restart(unit);
    }
    restart_runtime_unit(unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_role_excludes_supervisor_only_services() {
        let services = managed_services_for_role("agent");
        assert!(services.iter().any(|service| service.unit == FOLDOPS_AGENT_SERVICE));
        assert!(services.iter().any(|service| service.unit == AGENT_REGISTER_SERVICE));
        assert!(services.iter().all(|service| !service.supervisor_only));
        assert!(!services.iter().any(|service| service.unit == FOLDOPS_SUPERVISOR_SERVICE));
    }

    #[test]
    fn supervisor_role_includes_foldops_runtime_services() {
        let services = managed_services_for_role("supervisor");
        assert!(services.iter().any(|service| service.unit == FOLDOPS_SUPERVISOR_SERVICE));
        assert!(services.iter().any(|service| service.unit == FOLDOPS_SERVE_HTTPS_SERVICE));
        assert!(services.iter().any(|service| service.unit == FOLDOPS_PROVISION_SERVICE));
        assert!(services.iter().any(|service| service.unit == FOLDOPS_AGENT_SERVICE));
        assert!(!services.iter().any(|service| service.unit == AGENT_REGISTER_SERVICE));
    }

    #[test]
    fn restart_all_skips_one_shot_and_restarts_runtime_services_in_order() {
        let role = "supervisor";
        let units: Vec<_> = SUPERVISOR_RESTART_ALL_ORDER
            .iter()
            .filter(|unit| service_in_restart_all(role, unit))
            .copied()
            .collect();
        assert_eq!(
            units,
            vec![
                FOLDOPS_SUPERVISOR_SERVICE,
                FOLDOPS_AGENT_SERVICE,
                PROVISION_SERVICE,
                PROVISION_BOOT_SERVICE,
                FAH_SERVICE,
            ]
        );
        assert!(!service_in_restart_all(role, FOLDOPS_PROVISION_SERVICE));
        assert!(!service_in_restart_all(role, FOLDOPS_SERVE_HTTPS_SERVICE));
    }
}
