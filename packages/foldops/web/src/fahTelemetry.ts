import type { MachineSummary } from "./types";

type LatestSnapshot = MachineSummary["latest"];

export interface MachineFahTelemetry {
  project: string | null;
  projectLabel: string;
  ppd: number | null;
  tpf: string | null;
  cpuTemp: number | null;
  chassisTemp: number | null;
  progress: number | null;
}

export function getMachineFahTelemetry(
  latest: LatestSnapshot | null | undefined,
): MachineFahTelemetry {
  const project = latest?.project ?? null;
  const ppd = latest?.ppd ?? null;
  const tpf = latest?.payload?.fah?.tpf ?? null;
  const cpuTemp = latest?.cpu_temp ?? latest?.payload?.system.cpuTemp ?? null;
  const chassisTemp =
    latest?.chassis_temp ?? latest?.payload?.system.chassisTemp ?? null;
  const progress = latest?.progress ?? null;

  const projectLabel = project
    ? `${project} (R${latest?.run ?? "?"}/C${latest?.clone ?? "?"}/G${latest?.gen ?? "?"})`
    : "—";

  return {
    project,
    projectLabel,
    ppd,
    tpf: tpf && tpf.trim() ? tpf : null,
    cpuTemp,
    chassisTemp,
    progress,
  };
}
