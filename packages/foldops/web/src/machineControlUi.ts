import type { ControlAction, ControlStatus } from "./types";

const SYSTEMD_ACTIVE = new Set(["active", "activating", "reloading"]);
const SYSTEMD_INACTIVE = new Set(["inactive", "dead", "failed"]);

const FOLDING_WORK_STATES = new Set([
  "folding",
  "waiting",
  "download",
  "upload",
  "ready",
  "core",
  "finishing",
]);

export function isSystemdActive(state: string | undefined): boolean {
  return SYSTEMD_ACTIVE.has((state ?? "").trim().toLowerCase());
}

export function isSystemdInactive(state: string | undefined): boolean {
  const normalized = (state ?? "").trim().toLowerCase();
  return SYSTEMD_INACTIVE.has(normalized);
}

export function controlActionState(
  action: ControlAction,
  status: ControlStatus | null,
): { disabled: boolean; reason?: string } {
  if (!status) {
    return { disabled: false };
  }

  const agent = status.foldops_agent;
  const fah = status.fah_client;
  const folding = (status.fah_folding_state ?? "").trim().toLowerCase();

  switch (action) {
    case "agent.start":
      if (isSystemdActive(agent)) {
        return { disabled: true, reason: "Agent is already running" };
      }
      return { disabled: false };
    case "agent.stop":
      if (isSystemdInactive(agent)) {
        return { disabled: true, reason: "Agent is not running" };
      }
      return { disabled: false };
    case "fah.start":
      if (isSystemdActive(fah)) {
        return { disabled: true, reason: "FAH client is already running" };
      }
      return { disabled: false };
    case "fah.stop":
      if (isSystemdInactive(fah)) {
        return { disabled: true, reason: "FAH client is not running" };
      }
      return { disabled: false };
    case "fah.pause":
      if (isSystemdInactive(fah)) {
        return { disabled: true, reason: "FAH client is not running" };
      }
      if (folding === "paused") {
        return { disabled: true, reason: "Folding is already paused" };
      }
      if (folding === "stopped" || folding === "unreachable") {
        return { disabled: true, reason: "Folding is not active" };
      }
      return { disabled: false };
    case "fah.resume":
      if (isSystemdInactive(fah)) {
        return { disabled: true, reason: "FAH client is not running" };
      }
      if (folding !== "paused") {
        return { disabled: true, reason: "Folding is not paused" };
      }
      return { disabled: false };
    case "fah.finish":
      if (isSystemdInactive(fah)) {
        return { disabled: true, reason: "FAH client is not running" };
      }
      if (!FOLDING_WORK_STATES.has(folding)) {
        return { disabled: true, reason: "No active work unit to finish" };
      }
      return { disabled: false };
    default:
      return { disabled: false };
  }
}

export function optimisticControlStatus(
  status: ControlStatus,
  action: ControlAction,
): ControlStatus {
  const next: ControlStatus = { ...status };

  switch (action) {
    case "agent.start":
      next.foldops_agent = "active";
      break;
    case "agent.stop":
      next.foldops_agent = "inactive";
      break;
    case "agent.restart":
      next.foldops_agent = "activating";
      break;
    case "fah.start":
      next.fah_client = "active";
      next.fah_folding_state = "idle";
      break;
    case "fah.stop":
      next.fah_client = "inactive";
      next.fah_folding_state = "stopped";
      next.fah_unit_state = null;
      next.fah_folding_detail = "FAH service is not running";
      break;
    case "fah.restart":
      next.fah_client = "activating";
      break;
    case "fah.pause":
      next.fah_folding_state = "paused";
      break;
    case "fah.resume":
      next.fah_folding_state = "waiting";
      break;
    case "fah.finish":
      next.fah_folding_state = "finishing";
      break;
    default:
      break;
  }

  return next;
}
