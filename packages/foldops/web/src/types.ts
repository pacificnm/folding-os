export type LogSource = "fah" | "work";

export type SupervisorLogSource = "foldops" | "foldingosctl";

export interface SupervisorLogsResponse {
  source: SupervisorLogSource;
  lines: string[];
  path: string | null;
  updated_at: string | null;
  live: boolean;
}

export type NetworkInstallStatus =
  | "awaiting_install"
  | "online"
  | "offline"
  | "installed";

export interface AllowBootDevice {
  mac_address: string;
  install_disk?: string | null;
  install_status?: "pending" | "installed";
  network_status?: NetworkInstallStatus;
  hostname?: string | null;
  node_id?: string | null;
  primary_ipv4?: string | null;
  online?: boolean | null;
  registered_at?: string | null;
  last_seen_at?: string | null;
}

export interface AllowBootDevicesResponse {
  devices: AllowBootDevice[];
}

export interface AllowBootResult {
  mac_address: string;
  already_allowed: boolean;
  install_disk?: string | null;
}

export interface DenyBootResult {
  mac_address: string;
  already_removed: boolean;
}

export interface DenyBootMutationResponse {
  ok: boolean;
  result: DenyBootResult;
}

export interface AllowBootMutationResponse {
  ok: boolean;
  result: AllowBootResult;
}

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
  node_id?: string | null;
  installation_role?: string | null;
  foldingos_version?: string | null;
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
      hostname?: string;
      nodeId?: string | null;
      installationRole?: string | null;
      foldingosVersion?: string | null;
      primaryIpv4?: string | null;
      fah: {
        activeClientVersion?: string | null;
        expectedClientVersion?: string | null;
        clientInstalled?: boolean | null;
        clientVerified?: boolean | null;
        acquisitionFailures?: number | null;
        acquisitionNextAttemptUnix?: number | null;
        acquisitionLastFailureReason?: string | null;
        logPath?: string | null;
        logReadable?: boolean | null;
        tpf: string | null;
        foldingState?: string | null;
        unitState?: string | null;
        foldingDetail?: string | null;
        recentErrors: string[];
        statsDonor?: string | null;
        statsTeam?: string | null;
        configUsername?: string | null;
        configTeam?: number | null;
        configPasskeyConfigured?: boolean | null;
        configCpus?: number | null;
      };
      logs?: NodeLogs;
      system: {
        loadAvg: [number, number, number];
        uptime: number;
        cpuUsage?: number | null;
        cpuTemp: number | null;
        chassisTemp: number | null;
        memory?: {
          total: number;
          used: number;
          free: number;
          percent: number;
        };
        disk?: {
          total: number;
          used: number;
          free: number;
          percent: number;
        };
        network?: {
          rxBytes: number;
          txBytes: number;
          rxSec?: number | null;
          txSec?: number | null;
        };
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
  fah_folding_state?: string;
  fah_unit_state?: string | null;
  fah_folding_detail?: string | null;
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
  local?: boolean;
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

export interface SoftwareInstallLogEntry {
  timestamp: string;
  phase: string;
  operation: string;
  command: string;
  ok: boolean;
  exit_code?: number | null;
  message: string;
  stdout?: string;
  stderr?: string;
  detail?: unknown;
}

export interface SoftwareInstallLogResponse {
  path: string;
  updated_at: string;
  entries: SoftwareInstallLogEntry[];
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

export interface FoldinghomeConfigRequest {
  username: string;
  team: number;
  passkey?: string;
  passkey_secret?: string;
}

export interface FoldinghomeConfigResponse {
  hostname: string;
  ok: boolean;
  domain?: string;
  candidate?: string;
  activated?: boolean;
  ingested?: boolean;
  ingest_error?: string | null;
  error?: string;
}

export interface ManagedService {
  unit: string;
  name: string;
  status: string;
  loaded: boolean;
  restartable: boolean;
}

export interface ServicesResponse {
  installation_role?: string;
  services: ManagedService[];
}

export interface ServiceRestartResponse {
  unit: string;
  name: string;
  restarted: boolean;
  scheduled?: boolean;
  message?: string;
}

export interface ServicesRestartAllResponse {
  restarted: string[];
  count: number;
  message?: string;
}
