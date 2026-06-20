import type { MachineSummary } from "../types";

export type FahPayload = NonNullable<
  NonNullable<NonNullable<MachineSummary["latest"]>["payload"]>["fah"]
>;

function nextAttemptLabel(unix?: number | null): string | null {
  if (!unix || unix <= 0) return null;
  try {
    return new Date(unix * 1000).toLocaleString();
  } catch {
    return null;
  }
}

function hasClientInspection(fah: FahPayload): boolean {
  return (
    fah.activeClientVersion != null ||
    fah.expectedClientVersion != null ||
    fah.clientInstalled != null ||
    fah.clientVerified != null ||
    fah.acquisitionFailures != null ||
    fah.acquisitionNextAttemptUnix != null ||
    fah.acquisitionLastFailureReason != null ||
    fah.logPath != null ||
    fah.logReadable != null
  );
}

export function fahClientLabel(fah?: FahPayload | null): string {
  if (!fah) return "—";
  if (!hasClientInspection(fah)) return "unknown";
  const version = fah.activeClientVersion ?? fah.expectedClientVersion ?? null;
  if (fah.clientVerified) {
    return version ? `${version} verified` : "verified";
  }
  if (fah.clientInstalled) {
    return version ? `${version} unverified` : "unverified";
  }
  if ((fah.acquisitionFailures ?? 0) > 0) {
    return "acquire failed";
  }
  return version ? `${version} pending` : "pending";
}

export function fahClientClass(fah?: FahPayload | null): string {
  if (fah && !hasClientInspection(fah)) return "status-unknown";
  if (fah?.clientVerified) return "status-active";
  if ((fah?.acquisitionFailures ?? 0) > 0) return "status-failed";
  if (fah?.clientInstalled) return "warn-text";
  return "status-unknown";
}

export function fahAcquisitionLabel(fah?: FahPayload | null): string {
  if (!fah) return "—";
  if (!hasClientInspection(fah)) return "no report";
  const failures = fah.acquisitionFailures ?? 0;
  if (failures > 0) {
    return "retry scheduled";
  }
  if (fah.clientVerified) return "complete";
  if (fah.clientInstalled) return "installed";
  return "pending";
}

export function fahAcquisitionTitle(fah?: FahPayload | null): string | undefined {
  if (!fah) return undefined;
  if (!hasClientInspection(fah)) {
    return "FAH client install status was not reported by this agent. Update FoldOps agent and foldingosctl tools, or check foldingosctl inspect fah on the node.";
  }
  const parts: string[] = [];
  if (fah.expectedClientVersion) {
    parts.push(`Expected ${fah.expectedClientVersion}`);
  }
  if (fah.activeClientVersion) {
    parts.push(`Active ${fah.activeClientVersion}`);
  }
  if (fah.acquisitionLastFailureReason) {
    parts.push(fah.acquisitionLastFailureReason);
  }
  const retry = nextAttemptLabel(fah.acquisitionNextAttemptUnix);
  if (retry) {
    parts.push(`Next retry ${retry}`);
  }
  if (fah.logPath) {
    parts.push(`Log ${fah.logPath}${fah.logReadable === false ? " is not readable yet" : ""}`);
  }
  return parts.length > 0 ? parts.join(" · ") : undefined;
}
