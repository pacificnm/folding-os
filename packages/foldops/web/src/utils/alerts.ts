import type { AlertKind } from "../types";

const KIND_LABELS: Record<AlertKind, string> = {
  node_offline: "Node offline",
  node_online: "Node online",
  cpu_temp_high: "CPU temp",
  fah_inactive: "FAH inactive",
  fah_failed: "FAH failed",
  fah_errors: "FAH errors",
  fah_stuck: "FAH stuck",
};

export function formatKindLabel(kind: AlertKind): string {
  return KIND_LABELS[kind] ?? kind;
}

export function formatDuration(ms: number): string {
  if (ms < 60_000) return `${Math.round(ms / 1000)}s`;
  if (ms < 3_600_000) {
    const m = Math.floor(ms / 60_000);
    const s = Math.round((ms % 60_000) / 1000);
    return s > 0 ? `${m}m ${s}s` : `${m}m`;
  }
  const h = Math.floor(ms / 3_600_000);
  const m = Math.round((ms % 3_600_000) / 60_000);
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}
