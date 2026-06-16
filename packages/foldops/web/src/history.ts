import type { HistoryPoint, SnapshotRecord } from "./types";
import { formatChartTime } from "./utils/format";

export function snapshotsToHistory(
  snapshots: SnapshotRecord[],
): HistoryPoint[] {
  return snapshots
    .slice()
    .reverse()
    .map((s) => ({
      time: s.created_at,
      label: formatChartTime(s.created_at),
      progress: s.summary.progress,
      ppd: s.summary.ppd,
      cpu: s.summary.cpu_usage,
      memory: s.summary.memory_percent,
      disk: s.summary.disk_percent,
      cpuTemp: s.summary.cpu_temp,
      chassisTemp: s.summary.chassis_temp,
    }));
}
