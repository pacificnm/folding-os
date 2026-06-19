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
  SoftwareInstallLogResponse,
  SoftwareUpdatesResponse,
  SnapshotsResponse,
  SupervisorLogSource,
  SupervisorLogsResponse,
  AllowBootDevicesResponse,
  AllowBootResult,
  AllowBootMutationResponse,
  DenyBootResult,
  DenyBootMutationResponse,
  FoldinghomeConfigRequest,
  FoldinghomeConfigResponse,
  ServicesResponse,
  ServiceRestartResponse,
  ServicesRestartAllResponse,
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

export async function fetchSoftwareInstallLog(
  limit = 200,
): Promise<SoftwareInstallLogResponse> {
  const params = new URLSearchParams({ limit: String(limit) });
  const res = await fetch(`/api/software/install-log?${params}`);
  if (!res.ok) {
    throw new Error(await readApiError(res, "Failed to load install log"));
  }
  return res.json() as Promise<SoftwareInstallLogResponse>;
}

export async function fetchSupervisorLogs(
  source: SupervisorLogSource,
  options?: { lines?: number },
): Promise<SupervisorLogsResponse> {
  const params = new URLSearchParams({ source });
  if (options?.lines != null) {
    params.set("lines", String(options.lines));
  }
  const res = await fetch(`/api/supervisor/logs?${params}`);
  if (!res.ok) {
    throw new Error(await readApiError(res, "Failed to load supervisor logs"));
  }
  return res.json() as Promise<SupervisorLogsResponse>;
}

export async function fetchAllowBootDevices(): Promise<AllowBootDevicesResponse> {
  const res = await fetch("/api/fleet/allow-boot");
  if (!res.ok) {
    throw new Error(await readApiError(res, "Failed to load network boot allowlist"));
  }
  return res.json() as Promise<AllowBootDevicesResponse>;
}

export async function addAllowBootDevice(
  macAddress: string,
  installDisk?: string,
): Promise<AllowBootResult> {
  const res = await fetch("/api/fleet/allow-boot", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      mac_address: macAddress,
      install_disk: installDisk || undefined,
    }),
  });
  if (!res.ok) {
    throw new Error(await readApiError(res, "Failed to allow network install"));
  }
  const body = (await res.json()) as AllowBootMutationResponse;
  return body.result;
}

export async function removeAllowBootDevice(
  macAddress: string,
): Promise<DenyBootResult> {
  const res = await fetch("/api/fleet/allow-boot", {
    method: "DELETE",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ mac_address: macAddress }),
  });
  if (!res.ok) {
    throw new Error(await readApiError(res, "Failed to remove machine from allowlist"));
  }
  const body = (await res.json()) as DenyBootMutationResponse;
  return body.result;
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

export async function pushFoldinghomeConfig(
  hostname: string,
  body: FoldinghomeConfigRequest,
): Promise<FoldinghomeConfigResponse> {
  const res = await fetch(
    `/api/machines/${encodeURIComponent(hostname)}/config/foldinghome`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    },
  );
  const payload = (await res.json().catch(() => ({}))) as FoldinghomeConfigResponse & {
    error?: string;
  };
  if (!res.ok) {
    return {
      hostname,
      ok: false,
      error: payload.error ?? `Config push failed (${res.status})`,
    };
  }
  return { ...payload, hostname, ok: payload.ok ?? true };
}

export async function fetchServices(): Promise<ServicesResponse> {
  const res = await fetch("/api/services");
  if (!res.ok) {
    throw new Error(await readApiError(res, "Failed to load services"));
  }
  return res.json() as Promise<ServicesResponse>;
}

export async function restartService(
  unit: string,
): Promise<ServiceRestartResponse> {
  const dashboardRestart =
    unit === "foldingos-foldops-supervisor.service" ||
    unit === "foldingos-foldops-serve-https.service";
  const res = await fetch("/api/services/restart", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ unit }),
  }).catch((error) => {
    if (dashboardRestart) {
      return null;
    }
    throw error;
  });
  if (!res) {
    return {
      unit,
      name:
        unit === "foldingos-foldops-supervisor.service"
          ? "FoldOps supervisor (loopback)"
          : "FoldOps HTTPS (port 3443)",
      restarted: true,
      scheduled: true,
      message: "Restart started. The dashboard may reconnect briefly.",
    };
  }
  if (!res.ok) {
    throw new Error(await readApiError(res, "Service restart failed"));
  }
  return res.json() as Promise<ServiceRestartResponse>;
}

export async function restartAllServices(): Promise<ServicesRestartAllResponse> {
  const res = await fetch("/api/services/restart-all", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
  });
  if (!res.ok) {
    throw new Error(await readApiError(res, "Restart all services failed"));
  }
  return res.json() as Promise<ServicesRestartAllResponse>;
}
