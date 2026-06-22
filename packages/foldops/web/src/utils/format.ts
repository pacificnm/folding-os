export function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function formatLastSeen(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return new Date(iso).toLocaleString();
}

export function formatTemp(celsius: number | null | undefined): string {
  if (celsius == null) return "—";
  return `${celsius.toFixed(1)}°C`;
}

/** CPU temperature band for kiosk / at-a-glance styling */
export function cpuTempLevel(
  celsius: number | null | undefined,
): "unknown" | "ok" | "warn" | "hot" {
  if (celsius == null) return "unknown";
  if (celsius >= 85) return "hot";
  if (celsius >= 70) return "warn";
  return "ok";
}

export function formatPpd(ppd: number | null): string {
  if (ppd == null) return "—";
  if (ppd >= 1_000_000) return `${(ppd / 1_000_000).toFixed(2)}M`;
  if (ppd >= 1_000) return `${(ppd / 1_000).toFixed(1)}k`;
  return ppd.toFixed(0);
}

export function formatChartTime(iso: string): string {
  return new Date(iso).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatChartDate(iso: string): string {
  return new Date(iso).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatDurationMs(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "—";
  const totalMinutes = Math.floor(ms / 60_000);
  const days = Math.floor(totalMinutes / (60 * 24));
  const hours = Math.floor((totalMinutes % (60 * 24)) / 60);
  const minutes = totalMinutes % 60;
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m`;
  return "<1m";
}

export function formatWorkUnitDuration(startedAt: string, stoppedAt: string): string {
  const start = Date.parse(startedAt);
  const stop = Date.parse(stoppedAt);
  if (Number.isNaN(start) || Number.isNaN(stop)) return "—";
  return formatDurationMs(stop - start);
}
