import type { MachineSummary } from "./types";

function positiveMetric(value: number | null | undefined): boolean {
  return value != null && value > 0;
}

function hasActiveWorkEvidence(machine: MachineSummary | null | undefined): boolean {
  const latest = machine?.latest;
  return (
    Boolean(latest?.project) ||
    positiveMetric(latest?.progress) ||
    positiveMetric(latest?.ppd)
  );
}

export function machineFoldingActivityState(
  machine: MachineSummary | null | undefined,
): string | null {
  const latest = machine?.latest;
  const direct = latest?.payload?.fah?.foldingState?.trim().toLowerCase();

  if (direct) {
    if (
      (direct === "core" || direct === "waiting") &&
      hasActiveWorkEvidence(machine)
    ) {
      return "folding";
    }
    return direct;
  }

  if (latest?.project) return "folding";
  if (latest?.fah_status && latest.fah_status !== "active") {
    return latest.fah_status;
  }
  return "unknown";
}
