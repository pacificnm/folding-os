import {
  hasProjectDetails,
  normalizeFahProject,
} from "./fahProject";
import type {
  AlertHistoryFilter,
  AlertHistoryResponse,
  AlertsResponse,
  AlertsStatusResponse,
  ControlAction,
  ControlResult,
  ControlStatus,
  DeployRun,
  DeployRunsResponse,
  FahProjectInfo,
  FleetAssignRequest,
  FleetAssignResponse,
  FleetSoftwareApplyRequest,
  FleetSoftwareApplyResponse,
  LogSource,
  MachineLogsResponse,
  MachineSummary,
  MachinesResponse,
  RecoveryExportResponse,
  SoftwareUpdatesResponse,
  SnapshotsResponse,
} from "./types";

const FAH_PROJECT_API = "https://api.foldingathome.org/project";

export async function fetchMachineControlStatus(
  hostname: string,
): Promise<ControlStatus> {
  const res = await fetch(
    `/api/machines/${encodeURIComponent(hostname)}/control/status`,
  );
  if (!res.ok) {
    const body = (await res.json().catch(() => ({}))) as { error?: string };
    throw new Error(body.error ?? `Failed to load control status (${res.status})`);
  }
  return res.json() as Promise<ControlStatus>;
}

export async function runMachineControl(
  hostname: string,
  action: ControlAction,
): Promise<ControlResult> {
  const res = await fetch(
    `/api/machines/${encodeURIComponent(hostname)}/control`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ action }),
    },
  );
  const body = (await res.json().catch(() => ({}))) as ControlResult & {
    error?: string;
  };
  if (!res.ok) {
    throw new Error(body.error ?? `Control failed (${res.status})`);
  }
  return body;
}

export async function fetchDeployRuns(): Promise<DeployRunsResponse> {
  const res = await fetch("/api/deploy/runs");
  if (!res.ok) {
    throw new Error(`Failed to load deploy history (${res.status})`);
  }
  return res.json() as Promise<DeployRunsResponse>;
}

export async function fetchDeployRun(id: string): Promise<DeployRun> {
  const res = await fetch(`/api/deploy/runs/${encodeURIComponent(id)}`);
  if (!res.ok) {
    throw new Error(`Failed to load deploy run (${res.status})`);
  }
  return res.json() as Promise<DeployRun>;
}

export async function startAgentDeploy(
  hostnames?: string[],
): Promise<{ run_id: string; status: string }> {
  const res = await fetch("/api/deploy/agents", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(hostnames?.length ? { hostnames } : {}),
  });
  const body = (await res.json().catch(() => ({}))) as {
    error?: string;
    run_id?: string;
    status?: string;
  };
  if (!res.ok) {
    throw new Error(body.error ?? `Deploy failed (${res.status})`);
  }
  if (!body.run_id) {
    throw new Error("Deploy started but no run id returned");
  }
  return { run_id: body.run_id, status: body.status ?? "running" };
}

export async function fetchAlerts(): Promise<AlertsResponse> {
  const res = await fetch("/api/alerts");
  if (!res.ok) {
    throw new Error(`Failed to load alerts (${res.status})`);
  }
  return res.json() as Promise<AlertsResponse>;
}

export async function fetchAlertsStatus(): Promise<AlertsStatusResponse> {
  const res = await fetch("/api/alerts/status");
  if (!res.ok) {
    throw new Error(`Failed to load alert status (${res.status})`);
  }
  return res.json() as Promise<AlertsStatusResponse>;
}

export async function sendAlertTest(): Promise<{
  ok: boolean;
  message: string;
}> {
  const res = await fetch("/api/alerts/test", { method: "POST" });
  const body = (await res.json().catch(() => ({}))) as {
    ok?: boolean;
    message?: string;
    error?: string;
  };
  if (!res.ok) {
    throw new Error(body.error ?? `Test failed (${res.status})`);
  }
  return { ok: true, message: body.message ?? "Sent" };
}

export async function fetchAlertHistory(opts?: {
  status?: AlertHistoryFilter;
  limit?: number;
  hostname?: string;
}): Promise<AlertHistoryResponse> {
  const params = new URLSearchParams();
  if (opts?.status) params.set("status", opts.status);
  if (opts?.limit) params.set("limit", String(opts.limit));
  if (opts?.hostname) params.set("hostname", opts.hostname);

  const qs = params.toString();
  const res = await fetch(`/api/alerts/history${qs ? `?${qs}` : ""}`);
  if (!res.ok) {
    throw new Error(`Failed to load alert history (${res.status})`);
  }
  return res.json() as Promise<AlertHistoryResponse>;
}

export async function fetchMachines(): Promise<MachinesResponse> {
  const res = await fetch("/api/machines");
  if (!res.ok) {
    throw new Error(`Failed to load machines (${res.status})`);
  }
  return res.json() as Promise<MachinesResponse>;
}

export async function fetchMachineLogs(
  hostname: string,
  source: LogSource,
  opts?: { lines?: number; live?: boolean },
): Promise<MachineLogsResponse> {
  const params = new URLSearchParams({
    source,
    lines: String(opts?.lines ?? 200),
  });
  if (opts?.live === false) params.set("live", "0");

  const res = await fetch(
    `/api/machines/${encodeURIComponent(hostname)}/logs?${params}`,
  );
  if (!res.ok) {
    throw new Error(
      res.status === 404 ? "Machine not found" : `Failed to load logs (${res.status})`,
    );
  }
  return res.json() as Promise<MachineLogsResponse>;
}

export async function fetchMachine(hostname: string): Promise<MachineSummary> {
  const res = await fetch(`/api/machines/${encodeURIComponent(hostname)}`);
  if (!res.ok) {
    throw new Error(
      res.status === 404 ? "Machine not found" : `Failed to load machine (${res.status})`,
    );
  }
  return res.json() as Promise<MachineSummary>;
}

export async function fetchSnapshots(
  hostname: string,
  limit = 500,
): Promise<SnapshotsResponse> {
  const res = await fetch(
    `/api/snapshots/${encodeURIComponent(hostname)}?limit=${limit}`,
  );
  if (!res.ok) {
    throw new Error(`Failed to load history (${res.status})`);
  }
  return res.json() as Promise<SnapshotsResponse>;
}

async function fetchFahProjectDirect(
  projectId: string,
): Promise<FahProjectInfo | null> {
  const res = await fetch(
    `${FAH_PROJECT_API}/${encodeURIComponent(projectId)}`,
  );
  if (res.status === 404 || res.status === 400) return null;
  if (!res.ok) {
    throw new Error(`Folding@home API returned ${res.status}`);
  }
  const raw = (await res.json()) as Record<string, unknown>;
  const info = normalizeFahProject(raw, Number(projectId));
  return hasProjectDetails(info) ? info : null;
}

export async function fetchFahProject(
  projectId: string | number,
): Promise<FahProjectInfo | null> {
  const id = String(projectId).trim();
  if (!/^\d+$/.test(id)) {
    throw new Error(`Invalid project id: ${id}`);
  }

  const res = await fetch(`/api/projects/${encodeURIComponent(id)}`);
  const contentType = res.headers.get("content-type") ?? "";

  if (res.status === 404 && !contentType.includes("application/json")) {
    try {
      return await fetchFahProjectDirect(id);
    } catch {
      throw new Error(
        "Project API unavailable — rebuild and restart foldops-supervisor, then try again",
      );
    }
  }

  if (res.status === 404) {
    try {
      return await fetchFahProjectDirect(id);
    } catch {
      return null;
    }
  }

  if (!res.ok) {
    throw new Error(`Failed to load project (${res.status})`);
  }

  const info = (await res.json()) as FahProjectInfo;
  return hasProjectDetails(info) ? info : null;
}

async function readApiError(res: Response, fallback: string): Promise<string> {
  const body = (await res.json().catch(() => ({}))) as { error?: string };
  return body.error ?? `${fallback} (${res.status})`;
}

export async function fetchSoftwareUpdates(
  refresh = false,
): Promise<SoftwareUpdatesResponse> {
  const params = refresh ? "?refresh=true" : "";
  const res = await fetch(`/api/software/updates${params}`);
  if (!res.ok) {
    throw new Error(await readApiError(res, "Failed to check for updates"));
  }
  return res.json() as Promise<SoftwareUpdatesResponse>;
}

export async function fleetAssign(
  body: FleetAssignRequest,
): Promise<FleetAssignResponse> {
  const res = await fetch("/api/fleet/assign", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    throw new Error(await readApiError(res, "Assignment failed"));
  }
  return res.json() as Promise<FleetAssignResponse>;
}

export async function assignLocalSoftware(
  body: Pick<FleetAssignRequest, "foldops_manifest" | "tools_version">,
): Promise<FleetAssignResponse> {
  const res = await fetch("/api/software/assign-local", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    throw new Error(await readApiError(res, "Supervisor assignment failed"));
  }
  return res.json() as Promise<FleetAssignResponse>;
}

export async function fleetApplyFoldops(
  body: FleetSoftwareApplyRequest,
): Promise<FleetSoftwareApplyResponse> {
  const res = await fetch("/api/fleet/software/apply-foldops", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    throw new Error(await readApiError(res, "FoldOps apply failed"));
  }
  return res.json() as Promise<FleetSoftwareApplyResponse>;
}

export async function fleetApplyTools(
  body: FleetSoftwareApplyRequest,
): Promise<FleetSoftwareApplyResponse> {
  const res = await fetch("/api/fleet/software/apply-tools", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    throw new Error(await readApiError(res, "Tools apply failed"));
  }
  return res.json() as Promise<FleetSoftwareApplyResponse>;
}

export async function applyLocalSoftware(body: {
  foldops?: boolean;
  tools?: boolean;
  force?: boolean;
}): Promise<FleetSoftwareApplyResponse> {
  const res = await fetch("/api/software/apply-local", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    throw new Error(await readApiError(res, "Supervisor apply failed"));
  }
  return res.json() as Promise<FleetSoftwareApplyResponse>;
}

export async function createRecoveryExport(
  includeSecrets: boolean,
): Promise<RecoveryExportResponse> {
  const res = await fetch("/api/recovery/export", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ include_secrets: includeSecrets }),
  });
  if (!res.ok) {
    throw new Error(await readApiError(res, "Recovery export failed"));
  }
  return res.json() as Promise<RecoveryExportResponse>;
}

export async function downloadRecoveryExport(): Promise<void> {
  const res = await fetch("/api/recovery/export/latest");
  if (!res.ok) {
    throw new Error(await readApiError(res, "Recovery download failed"));
  }
  const blob = await res.blob();
  const disposition = res.headers.get("content-disposition") ?? "";
  const match = disposition.match(/filename="([^"]+)"/);
  const filename = match?.[1] ?? "foldingos-supervisor-backup.tar.zst";
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}
