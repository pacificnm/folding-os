use std::ops::Drop;

use crate::automation_policy::{
    is_foldops_automation_user, require_acquire_automation_mutation,
    require_agent_automation_mutation, require_supervisor_automation_mutation,
};
use crate::paths::AppliancePaths;

const FOLDINGOS_ADMIN_USERNAME: &str = "foldingos-admin";

/// Drop setuid-provided root before command dispatch unless a guarded command
/// re-elevates for its duration.
///
/// Uses `setresuid` so the saved set-user-ID stays root; plain `setuid(real_uid)`
/// would clear it and make later `seteuid(0)` fail with EPERM for the foldops user.
pub fn initialize() -> Result<(), String> {
    #[cfg(unix)]
    {
        use nix::unistd::{geteuid, getgid, getuid, setresgid, setresuid, Gid, Uid};
        let effective = geteuid();
        let real = getuid();
        if effective == Uid::from_raw(0) && real != Uid::from_raw(0) {
            let gid = getgid();
            setresgid(gid, gid, Gid::from_raw(0))
                .map_err(|error| format!("drop setuid group identity: {error}"))?;
            setresuid(real, real, Uid::from_raw(0))
                .map_err(|error| format!("drop setuid user identity: {error}"))?;
        }
    }
    Ok(())
}

pub fn command_requires_root_elevation(command_group: &str, command_name: &str) -> bool {
    matches!(
        (command_group, command_name),
        ("tools", "acquire")
            | ("foldops", "acquire")
            | ("provision", "allow-boot")
            | ("provision", "deny-boot")
            | ("recovery", "export")
            | ("recovery", "import")
            | ("services", "restart")
            | ("services", "restart-all")
            | ("config", "activate")
            | ("config", "set-passkey")
    )
}

pub fn guard_for_parsed_command(
    paths: &AppliancePaths,
    args: &[String],
) -> Result<PrivilegeGuard, String> {
    if args.len() < 2 {
        return Ok(PrivilegeGuard::none());
    }
    let command_group = args[0].as_str();
    let command_name = args[1].as_str();
    if !command_requires_root_elevation(command_group, command_name) {
        return Ok(PrivilegeGuard::none());
    }
    authorize_root_elevation(paths, command_group, command_name)?;
    match elevate_to_root()? {
        ElevationOutcome::AlreadyRoot => Ok(PrivilegeGuard::none()),
        ElevationOutcome::Elevated => Ok(PrivilegeGuard::elevated()),
        ElevationOutcome::Unavailable if cfg!(test) => Ok(PrivilegeGuard::none()),
        ElevationOutcome::Unavailable => Err(
            "foldingosctl must be installed setuid root (mode 4755) for this command".into(),
        ),
    }
}

pub struct PrivilegeGuard {
    elevated: bool,
}

impl PrivilegeGuard {
    fn none() -> Self {
        Self { elevated: false }
    }

    fn elevated() -> Self {
        Self { elevated: true }
    }
}

impl Drop for PrivilegeGuard {
    fn drop(&mut self) {
        if self.elevated {
            let _ = drop_to_real_identity();
        }
    }
}

fn authorize_root_elevation(
    paths: &AppliancePaths,
    command_group: &str,
    command_name: &str,
) -> Result<(), String> {
    if running_as_real_root() {
        return Ok(());
    }
    if is_operator_user() {
        return Ok(());
    }
    if !is_foldops_automation_user() {
        return Err(format!(
            "foldingosctl {command_group} {command_name} requires root privileges"
        ));
    }
    match (command_group, command_name) {
        ("tools", "acquire") => require_acquire_automation_mutation(paths, "tools"),
        ("foldops", "acquire") => require_acquire_automation_mutation(paths, "foldops"),
        ("provision", "allow-boot") => {
            require_supervisor_automation_mutation(paths, "provision", "allow-boot")
        }
        ("provision", "deny-boot") => {
            require_supervisor_automation_mutation(paths, "provision", "deny-boot")
        }
        ("recovery", "export") => {
            require_supervisor_automation_mutation(paths, "recovery", "export")
        }
        ("recovery", "import") => {
            require_supervisor_automation_mutation(paths, "recovery", "import")
        }
        ("services", "restart") | ("services", "restart-all") => {
            require_supervisor_automation_mutation(paths, "services", command_name)
        }
        ("config", "activate") => {
            require_agent_automation_mutation(paths, "config", "activate")
        }
        _ => Err(format!(
            "automation policy does not authorize {command_group} {command_name} for the foldops user"
        )),
    }
}

fn is_operator_user() -> bool {
    crate::automation_policy::current_unix_username().as_deref() == Some(FOLDINGOS_ADMIN_USERNAME)
}

fn running_as_real_root() -> bool {
    #[cfg(unix)]
    {
        use nix::unistd::{geteuid, getuid, Uid};
        getuid() == Uid::from_raw(0) && geteuid() == Uid::from_raw(0)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

enum ElevationOutcome {
    AlreadyRoot,
    Elevated,
    Unavailable,
}

fn elevate_to_root() -> Result<ElevationOutcome, String> {
    if running_as_real_root() {
        return Ok(ElevationOutcome::AlreadyRoot);
    }
    #[cfg(unix)]
    {
        use nix::unistd::{geteuid, setegid, seteuid, Gid, Uid};
        seteuid(Uid::from_raw(0)).map_err(|error| {
            format!(
                "elevate foldingosctl user identity: {error}. \
                 The foldops user requires /usr/bin/foldingosctl to be root:root mode 4755; \
                 as root run: chown root:root /usr/bin/foldingosctl && chmod 4755 /usr/bin/foldingosctl"
            )
        })?;
        setegid(Gid::from_raw(0))
            .map_err(|error| format!("elevate foldingosctl group identity: {error}"))?;
        if geteuid() == Uid::from_raw(0) {
            return Ok(ElevationOutcome::Elevated);
        }
    }
    Ok(ElevationOutcome::Unavailable)
}

fn drop_to_real_identity() -> Result<(), String> {
    #[cfg(unix)]
    {
        use nix::unistd::{getgid, getuid, setresgid, setresuid, Gid, Uid};
        let uid = getuid();
        let gid = getgid();
        setresuid(uid, uid, Uid::from_raw(0))
            .map_err(|error| format!("restore foldingosctl user identity: {error}"))?;
        setresgid(gid, gid, Gid::from_raw(0))
            .map_err(|error| format!("restore foldingosctl group identity: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_root_elevation_commands() {
        assert!(command_requires_root_elevation("tools", "acquire"));
        assert!(command_requires_root_elevation("foldops", "acquire"));
        assert!(command_requires_root_elevation("provision", "allow-boot"));
        assert!(command_requires_root_elevation("provision", "deny-boot"));
        assert!(command_requires_root_elevation("recovery", "export"));
        assert!(!command_requires_root_elevation("provision", "assign"));
        assert!(!command_requires_root_elevation("inspect", "foldops"));
    }
}
