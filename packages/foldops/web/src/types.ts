export type LogSource = "fah" | "work";

export interface NodeLogs {
  fah: string[];
  work: string[];
  fahPath?: string;
  workPath?: string;
}

export interface MachineLogsResponse {
  hostname: string;
  source: LogSource;
  lines: string[];
  path: string | null;
  updated_at: string | null;
  live: boolean;
  online: boolean;
  warning?: string;
  live_error?: string;
  live_url?: string;
}

export interface MachineSummary {
  hostname: string;
  first_seen: string;
  last_seen: string;
  online: boolean;
  latest: {
    created_at: string;
    fah_status: string;
    project: string | null;
    run: number | null;
    clone: number | null;
    gen: number | null;
    progress: number | null;
    ppd: number | null;
    cpu_usage: number | null;
    memory_percent: number | null;
    disk_percent: number | null;
    cpu_temp: number | null;
    chassis_temp: number | null;
    apt_updates: number;
    reboot_required: boolean;
    payload?: {
      fah: {
        tpf: string | null;
        recentErrors: string[];
        statsDonor?: string | null;
        statsTeam?: string | null;
      };
      logs?: NodeLogs;
      system: {
        loadAvg: [number, number, number];
        uptime: number;
        cpuTemp: number | null;
        chassisTemp: number | null;
      };
    };
  } | null;
}

export interface MachinesResponse {
  machines: MachineSummary[];
  farm_ppd: number;
}

export type AlertSeverity = "info" | "warning" | "critical";

export type AlertKind =
  | "node_offline"
  | "node_online"
  | "cpu_temp_high"
  | "fah_inactive"
  | "fah_failed"
  | "fah_errors"
  | "fah_stuck";

export interface ActiveAlert {
  id: string;
  hostname: string;
  kind: AlertKind;
  severity: AlertSeverity;
  message: string;
  active: boolean;
  since: string;
  resolved_at: string | null;
}

export interface AlertsResponse {
  alerts: ActiveAlert[];
  count: number;
}

export type AlertHistoryFilter = "all" | "active" | "resolved";

export interface AlertHistoryItem {
  id: string;
  hostname: string;
  kind: AlertKind;
  severity: AlertSeverity;
  message: string;
  active: boolean;
  fired_at: string;
  resolved_at: string | null;
  duration_ms: number;
  details: string | null;
}

export interface AlertHistoryResponse {
  alerts: AlertHistoryItem[];
  count: number;
  counts: { active: number; resolved: number; total: number };
  status: AlertHistoryFilter;
}

export interface AlertsStatusResponse {
  enabled: boolean;
  webhook_configured: boolean;
  discord: boolean;
  dashboard_url: string | null;
  webhook: {
    last_error: string | null;
    last_success_at: string | null;
  };
}

export type DeployRunStatus = "running" | "completed" | "failed";

export type DeployHostStatus =
  | "pending"
  | "running"
  | "success"
  | "failed"
  | "offline"
  | "skipped";

export interface DeployHostResult {
  hostname: string;
  status: DeployHostStatus;
  exit_code: number | null;
  message: string;
  stdout: string;
  stderr: string;
  duration_ms: number | null;
  started_at: string | null;
  finished_at: string | null;
}

export interface DeployRun {
  id: string;
  created_at: string;
  status: DeployRunStatus;
  hostnames: string[];
  results: Record<string, DeployHostResult>;
}

export interface DeployRunsResponse {
  runs: DeployRun[];
}

export type ControlAction =
  | "agent.start"
  | "agent.stop"
  | "agent.restart"
  | "fah.start"
  | "fah.stop"
  | "fah.restart"
  | "fah.pause"
  | "fah.resume"
  | "fah.finish"
  | "host.reboot";

export interface ControlStatus {
  hostname?: string;
  foldops_agent: string;
  fah_client: string;
}

export interface ControlResult {
  hostname?: string;
  ok: boolean;
  action: ControlAction;
  message: string;
  stdout: string;
  stderr: string;
}

export interface FahProjectInfo {
  project: number;
  manager: string | null;
  cause: string | null;
  institution: string | null;
  description: string | null;
  projectRange: string | null;
  modified: string | null;
  statsUrl: string;
}

export interface SnapshotSummary {
  fah_status: string;
  project: string | null;
  progress: number | null;
  ppd: number | null;
  cpu_usage: number | null;
  memory_percent: number | null;
  disk_percent: number | null;
  cpu_temp: number | null;
  chassis_temp: number | null;
}

export interface SnapshotRecord {
  id: number;
  created_at: string;
  summary: SnapshotSummary;
}

export interface SnapshotsResponse {
  hostname: string;
  snapshots: SnapshotRecord[];
}

export interface HistoryPoint {
  time: string;
  label: string;
  progress: number | null;
  ppd: number | null;
  cpu: number | null;
  memory: number | null;
  disk: number | null;
  cpuTemp: number | null;
  chassisTemp: number | null;
}

export interface SoftwareUpstreamChannel {
  latest_manifest_release?: string;
  latest_tools_version?: string;
  published_at?: string | null;
}

export interface SoftwareUpstreamInfo {
  foldops: SoftwareUpstreamChannel | null;
  tools: SoftwareUpstreamChannel | null;
}

export interface SoftwareNodeVersions {
  hostname: string;
  node_id?: string;
  online?: boolean;
  active_foldops_manifest_release: string | null;
  assigned_foldops_manifest_release: string | null;
  active_tools_version: string | null;
  assigned_tools_version: string | null;
  foldops_apply_pending?: boolean;
  tools_apply_pending?: boolean;
  foldops_update_available?: boolean;
  tools_update_available?: boolean;
}

export interface SoftwareUpdatesResponse {
  checked_at: string;
  upstream: SoftwareUpstreamInfo;
  supervisor: SoftwareNodeVersions;
  agents: SoftwareNodeVersions[];
}

export interface FleetAssignRequest {
  node_id?: string;
  all?: boolean;
  version?: string;
  foldops_manifest?: string;
  tools_version?: string;
}

export interface FleetAssignResult {
  scope: string;
  updated_count: number;
  node_id?: string | null;
  image_version?: string | null;
  foldops_manifest_release?: string | null;
  tools_version?: string | null;
}

export interface FleetAssignResponse {
  ok: boolean;
  result: FleetAssignResult;
}

export interface FleetSoftwareApplyRequest {
  hostnames?: string[];
  all?: boolean;
}

export interface FleetSoftwareApplyResult {
  hostname?: string;
  component?: string;
  ok: boolean;
  skipped?: boolean;
  error?: string;
  message?: string;
  active_manifest_release?: string | null;
  active_tools_version?: string | null;
}

export interface FleetSoftwareApplyResponse {
  results: FleetSoftwareApplyResult[];
}

export interface RecoveryExportResponse {
  ok: boolean;
  path: string;
  sha256: string;
  size_bytes: number;
  hostname?: string;
  export_timestamp?: string;
  include_secrets?: boolean;
  file_count?: number;
  download_url?: string;
}
