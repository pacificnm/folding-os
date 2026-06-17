//! Control actions proxied from supervisor to agent.
//! Mirrors [`packages/shared/src/control.ts`](../../../packages/shared/src/control.ts).

/// All supported remote control actions.
pub const CONTROL_ACTIONS: &[&str] = &[
    "agent.start",
    "agent.stop",
    "agent.restart",
    "fah.start",
    "fah.stop",
    "fah.restart",
    "fah.pause",
    "fah.resume",
    "fah.finish",
    "host.reboot",
];

/// Parsed control action (same strings as the TypeScript union).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlAction {
    AgentStart,
    AgentStop,
    AgentRestart,
    FahStart,
    FahStop,
    FahRestart,
    FahPause,
    FahResume,
    FahFinish,
    HostReboot,
}

impl ControlAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentStart => "agent.start",
            Self::AgentStop => "agent.stop",
            Self::AgentRestart => "agent.restart",
            Self::FahStart => "fah.start",
            Self::FahStop => "fah.stop",
            Self::FahRestart => "fah.restart",
            Self::FahPause => "fah.pause",
            Self::FahResume => "fah.resume",
            Self::FahFinish => "fah.finish",
            Self::HostReboot => "host.reboot",
        }
    }
}

impl TryFrom<&str> for ControlAction {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "agent.start" => Ok(Self::AgentStart),
            "agent.stop" => Ok(Self::AgentStop),
            "agent.restart" => Ok(Self::AgentRestart),
            "fah.start" => Ok(Self::FahStart),
            "fah.stop" => Ok(Self::FahStop),
            "fah.restart" => Ok(Self::FahRestart),
            "fah.pause" => Ok(Self::FahPause),
            "fah.resume" => Ok(Self::FahResume),
            "fah.finish" => Ok(Self::FahFinish),
            "host.reboot" => Ok(Self::HostReboot),
            _ => Err(()),
        }
    }
}

/// Returns true when `value` is a known control action string.
pub fn is_control_action(value: &str) -> bool {
    ControlAction::try_from(value).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_actions_round_trip() {
        for action in CONTROL_ACTIONS {
            assert!(is_control_action(action));
            let parsed = ControlAction::try_from(*action).unwrap();
            assert_eq!(parsed.as_str(), *action);
        }
    }

    #[test]
    fn rejects_unknown_action() {
        assert!(!is_control_action("fah.explode"));
    }
}
