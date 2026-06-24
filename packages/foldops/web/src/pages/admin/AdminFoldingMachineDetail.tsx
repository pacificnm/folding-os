import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import {
  fahAcquisitionLabel,
  fahClientLabel,
} from "../../components/FahClientStatus";
import { FahStatsLinks } from "../../components/FahStatsLinks";
import { HistoryChart } from "../../components/HistoryChart";
import { MachineControlsPanel } from "../../components/MachineControlsPanel";
import { MachineLogsPanel } from "../../components/MachineLogsPanel";
import { ProjectInfoPanel } from "../../components/ProjectInfoPanel";
import { Tabs, type TabItem } from "../../components/Tabs";
import { fetchFahProject, fetchMachine, fetchSnapshots, fetchWorkUnitHistory } from "../../api";
import { machineFoldingActivityState } from "../../fahActivity";
import {
  displayConfiguredCpus,
  displayConfiguredDonor,
  displayConfiguredTeam,
  displayConfiguredToken,
  displayEffectiveCpus,
  fahCpuPolicyDrift,
} from "../../fahConfig";
import { snapshotsToHistory } from "../../history";
import type { FahProjectInfo, HistoryPoint, HostHardwareProfile, MachineSummary, WorkUnitHistoryResponse } from "../../types";
import {
  formatChartDate,
  formatLastSeen,
  formatPpd,
  formatTemp,
  formatUptime,
  formatWorkUnitDuration,
} from "../../utils/format";

const RANGES = [
  { label: "2 hours", limit: 120 },
  { label: "8 hours", limit: 480 },
  { label: "24 hours", limit: 500 },
] as const;

const DETAIL_TABS: TabItem[] = [
  { id: "overview", label: "Overview" },
  { id: "hardware", label: "Hardware" },
  { id: "work", label: "Work units" },
  { id: "logs", label: "Logs" },
  { id: "controls", label: "Controls" },
];

function formatBytes(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value)) return "-";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let current = value;
  let unit = 0;
  while (current >= 1024 && unit < units.length - 1) {
    current /= 1024;
    unit += 1;
  }
  const digits = unit <= 1 ? 0 : 1;
  return `${current.toFixed(digits)} ${units[unit]}`;
}

function formatPercent(value: number | null | undefined): string {
  return value == null ? "-" : `${value.toFixed(1)}%`;
}

function formatNumber(value: number | null | undefined): string {
  return value == null ? "-" : value.toLocaleString();
}

function activityLabel(state: string | null | undefined): string {
  switch (state?.trim().toLowerCase()) {
    case "folding":
      return "Folding";
    case "paused":
      return "Paused";
    case "waiting":
      return "Waiting for work";
    case "finishing":
      return "Finishing WU";
    case "download":
      return "Downloading WU";
    case "upload":
      return "Uploading WU";
    case "ready":
      return "Ready";
    case "core":
      return "Starting core";
    case "stopped":
    case "inactive":
      return "Stopped";
    case "failed":
      return "Failed";
    case "idle":
      return "Idle";
    default:
      return "Unknown";
  }
}

function activityBadgeClass(state: string | null | undefined): string {
  switch (state?.trim().toLowerCase()) {
    case "folding":
      return "badge-ok";
    case "failed":
    case "stopped":
    case "inactive":
      return "badge-danger";
    default:
      return "badge-warn";
  }
}

function projectLabel(machine: MachineSummary | null): string {
  const latest = machine?.latest;
  if (!latest?.project) return "-";
  const parts = [`Project ${latest.project}`];
  if (latest.run != null || latest.clone != null || latest.gen != null) {
    parts.push(
      `R${latest.run ?? "?"}/C${latest.clone ?? "?"}/G${latest.gen ?? "?"}`,
    );
  }
  return parts.join(" ");
}

interface InfoItem {
  label: string;
  value: string;
  mono?: boolean;
  muted?: boolean;
}

function formatWorkUnitAssignment(
  project: string,
  run: number,
  clone: number,
  gen: number,
): string {
  return `Project ${project} R${run}/C${clone}/G${gen}`;
}

function formatNamedBlock(parts: Array<string | null | undefined>): string {
  const values = parts
    .map((part) => part?.trim())
    .filter((part): part is string => Boolean(part));
  return values.length > 0 ? values.join(" / ") : "-";
}

function formatStorageSummary(storage: HostHardwareProfile["storage"]): string {
  if (!storage?.length) return "-";
  return storage
    .map((device) => {
      const model = device.model?.trim();
      const size = formatBytes(device.sizeBytes);
      const kind =
        device.rotational == null
          ? null
          : device.rotational
            ? "HDD"
            : "SSD";
      return [device.name, model, size, kind].filter(Boolean).join(" - ");
    })
    .join("; ");
}

function formatNetworkSummary(network: HostHardwareProfile["network"]): string {
  if (!network?.length) return "-";
  return network
    .map((adapter) => {
      const speed =
        adapter.speedMbps != null ? `${adapter.speedMbps} Mbps` : null;
      return [adapter.name, adapter.macAddress, speed, adapter.operstate]
        .filter(Boolean)
        .join(" / ");
    })
    .join("; ");
}

function formatMemoryModules(
  modules: HostHardwareProfile["memory"]["modules"],
): string {
  if (!modules?.length) return "-";
  return modules
    .map((module) => {
      const parts = [formatBytes(module.sizeBytes)];
      if (module.speedMts != null) parts.push(`${module.speedMts} MT/s`);
      if (module.manufacturer) parts.push(module.manufacturer);
      if (module.locator) parts.push(module.locator);
      return parts.join(" - ");
    })
    .join("; ");
}

function formatPciSummary(
  devices: HostHardwareProfile["pciDevices"],
): string {
  if (!devices?.length) return "-";
  return devices
    .slice(0, 6)
    .map((device) =>
      [device.address, device.classId, device.vendorId, device.deviceId]
        .filter(Boolean)
        .join(" "),
    )
    .join("; ");
}

function InfoCard({
  title,
  items,
}: {
  title: string;
  items: InfoItem[];
}) {
  return (
    <section className="admin-detail-card">
      <h3>{title}</h3>
      <dl className="admin-detail-list">
        {items.map((item) => (
          <div key={item.label}>
            <dt>{item.label}</dt>
            <dd
              className={`${item.mono ? "mono" : ""}${item.muted ? " admin-muted" : ""}`.trim()}
            >
              {item.value}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

export function AdminFoldingMachineDetail() {
  const { machineId: encoded } = useParams<{ machineId: string }>();
  const hostname = encoded ? decodeURIComponent(encoded) : "";

  const [tab, setTab] = useState("overview");
  const [machine, setMachine] = useState<MachineSummary | null>(null);
  const [history, setHistory] = useState<HistoryPoint[]>([]);
  const [limit, setLimit] = useState<number>(480);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [projectInfo, setProjectInfo] = useState<FahProjectInfo | null>(null);
  const [projectLoading, setProjectLoading] = useState(false);
  const [projectError, setProjectError] = useState<string | null>(null);
  const [workHistory, setWorkHistory] = useState<WorkUnitHistoryResponse | null>(null);
  const [workHistoryLoading, setWorkHistoryLoading] = useState(false);
  const [workHistoryError, setWorkHistoryError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!hostname) return;
    try {
      const [nextMachine, snapshots] = await Promise.all([
        fetchMachine(hostname),
        fetchSnapshots(hostname, limit),
      ]);
      setMachine(nextMachine);
      setHistory(snapshotsToHistory(snapshots.snapshots));
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load machine");
    } finally {
      setLoading(false);
    }
  }, [hostname, limit]);

  const loadWorkHistory = useCallback(async () => {
    if (!hostname) return;
    setWorkHistoryLoading(true);
    try {
      const history = await fetchWorkUnitHistory(hostname);
      setWorkHistory(history);
      setWorkHistoryError(null);
    } catch (err) {
      setWorkHistory(null);
      setWorkHistoryError(
        err instanceof Error ? err.message : "Failed to load work unit history",
      );
    } finally {
      setWorkHistoryLoading(false);
    }
  }, [hostname]);

  useEffect(() => {
    setLoading(true);
    load();
    const id = window.setInterval(load, 60_000);
    return () => window.clearInterval(id);
  }, [load]);

  useEffect(() => {
    if (tab !== "work") return;
    loadWorkHistory();
    const id = window.setInterval(loadWorkHistory, 60_000);
    return () => window.clearInterval(id);
  }, [tab, loadWorkHistory]);

  const latest = machine?.latest;
  const payload = latest?.payload;
  const fah = payload?.fah;
  const system = payload?.system;
  const activity = machineFoldingActivityState(machine);
  const projectId = latest?.project ?? null;
  const snapshotCount = history.length;
  const rangeLabel = useMemo(
    () => RANGES.find((range) => range.limit === limit)?.label ?? `${limit} samples`,
    [limit],
  );

  useEffect(() => {
    if (!projectId) {
      setProjectInfo(null);
      setProjectError(null);
      setProjectLoading(false);
      return;
    }

    let cancelled = false;
    setProjectLoading(true);
    setProjectError(null);

    fetchFahProject(projectId)
      .then((info) => {
        if (!cancelled) setProjectInfo(info);
      })
      .catch((err) => {
        if (!cancelled) {
          setProjectInfo(null);
          setProjectError(
            err instanceof Error ? err.message : "Failed to load project",
          );
        }
      })
      .finally(() => {
        if (!cancelled) setProjectLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [projectId]);

  const identityItems: InfoItem[] = [
    { label: "Hostname", value: machine?.hostname ?? hostname, mono: true },
    {
      label: "Node ID",
      value: machine?.node_id ?? payload?.nodeId ?? "Not reported",
      mono: true,
    },
    {
      label: "Role",
      value: machine?.installation_role ?? payload?.installationRole ?? "-",
    },
    {
      label: "FoldingOS",
      value: machine?.foldingos_version ?? payload?.foldingosVersion ?? "-",
      mono: true,
    },
    { label: "Primary IPv4", value: payload?.primaryIpv4 ?? "-", mono: true },
    {
      label: "Last seen",
      value: machine ? formatLastSeen(machine.last_seen) : "-",
    },
  ];

  const hardware = machine?.hardware_profile ?? null;

  const hardwareItems: InfoItem[] = [
    {
      label: "CPU model",
      value: hardware?.cpu.model ?? "-",
      mono: Boolean(hardware?.cpu.model),
    },
    {
      label: "CPU threads",
      value: hardware
        ? `${hardware.cpu.logicalThreads} logical / ${hardware.cpu.physicalCores} physical`
        : "-",
      mono: true,
    },
    {
      label: "Assigned FAH CPUs",
      value: displayConfiguredCpus(fah),
      mono: true,
    },
    {
      label: "Effective FAH CPUs",
      value: displayEffectiveCpus(fah),
      mono: true,
    },
    {
      label: "Memory total",
      value: formatBytes(hardware?.memory.totalBytes ?? system?.memory?.total),
      mono: true,
    },
    {
      label: "Disk total",
      value: formatBytes(system?.disk?.total),
      mono: true,
    },
    {
      label: "Board / chassis",
      value: hardware
        ? formatNamedBlock([
            hardware.board?.vendor,
            hardware.board?.product ?? hardware.system?.product,
            hardware.chassis?.vendor,
            hardware.chassis?.typeCode,
          ])
        : "-",
    },
  ];

  const hardwareDetailItems: InfoItem[] = [
    {
      label: "System product",
      value: formatNamedBlock([
        hardware?.system?.vendor,
        hardware?.system?.product,
        hardware?.system?.sku,
      ]),
    },
    {
      label: "BIOS",
      value: formatNamedBlock([
        hardware?.bios?.vendor,
        hardware?.bios?.version,
        hardware?.bios?.date,
      ]),
    },
    {
      label: "Architecture",
      value: hardware?.cpu.architecture ?? "-",
      mono: true,
    },
    {
      label: "Memory modules",
      value: formatMemoryModules(hardware?.memory.modules),
    },
    {
      label: "Storage devices",
      value: formatStorageSummary(hardware?.storage),
    },
    {
      label: "Network adapters",
      value: formatNetworkSummary(hardware?.network),
    },
    {
      label: "PCI inventory",
      value: formatPciSummary(hardware?.pciDevices),
      mono: true,
    },
  ];

  const healthItems: InfoItem[] = [
    { label: "Uptime", value: system?.uptime != null ? formatUptime(system.uptime) : "-" },
    { label: "CPU usage", value: formatPercent(latest?.cpu_usage ?? system?.cpuUsage) },
    { label: "Load average", value: system?.loadAvg.map((n) => n.toFixed(2)).join(" / ") ?? "-", mono: true },
    { label: "Memory used", value: `${formatPercent(latest?.memory_percent ?? system?.memory?.percent)} (${formatBytes(system?.memory?.used)})`, mono: true },
    { label: "Disk used", value: `${formatPercent(latest?.disk_percent ?? system?.disk?.percent)} (${formatBytes(system?.disk?.used)})`, mono: true },
    { label: "CPU temp", value: formatTemp(latest?.cpu_temp ?? system?.cpuTemp), mono: true },
    { label: "Chassis temp", value: formatTemp(latest?.chassis_temp ?? system?.chassisTemp), mono: true },
    { label: "Network RX/TX", value: `${formatBytes(system?.network?.rxBytes)} / ${formatBytes(system?.network?.txBytes)}`, mono: true },
  ];

  const foldingItems: InfoItem[] = [
    { label: "Activity", value: activityLabel(activity) },
    { label: "Unit state", value: fah?.unitState ?? "-", mono: true },
    { label: "Detail", value: fah?.foldingDetail ?? projectLabel(machine) },
    { label: "Client", value: fahClientLabel(fah), mono: true },
    { label: "Acquire", value: fahAcquisitionLabel(fah), mono: true },
    { label: "Donor", value: displayConfiguredDonor(fah) },
    { label: "Team", value: displayConfiguredTeam(fah), mono: true },
    { label: "Token", value: displayConfiguredToken(fah), mono: true },
  ];

  return (
    <div className="admin-detail">
      <div className="admin-detail-header">
        <div>
          <Link to="/admin/folding" className="admin-detail-back">
            Back to Folding@home
          </Link>
          <div className="admin-detail-title-row">
            <h2 className="deploy-heading">{hostname || "Machine"}</h2>
            {machine && (
              <span
                className={`badge ${machine.online ? "badge-ok" : "badge-warn"}`}
              >
                {machine.online ? "online" : "offline"}
              </span>
            )}
            <span className={`badge ${activityBadgeClass(activity)}`}>
              {activityLabel(activity)}
            </span>
          </div>
        </div>
        <button
          type="button"
          className="deploy-btn"
          disabled={loading}
          onClick={load}
        >
          {loading ? "Refreshing..." : "Refresh"}
        </button>
      </div>

      {error && <p className="message error">{error}</p>}
      {loading && !machine && <p className="admin-muted">Loading machine...</p>}

      {machine && (
        <>
          <div className="admin-detail-summary">
            <div className="detail-stat">
              <span className="label">Project</span>
              <span className="value mono">{projectLabel(machine)}</span>
            </div>
            <div className="detail-stat">
              <span className="label">Progress</span>
              <span className="value mono">
                {latest?.progress != null ? `${latest.progress.toFixed(1)}%` : "-"}
              </span>
            </div>
            <div className="detail-stat">
              <span className="label">PPD</span>
              <span className="value mono highlight">
                {formatPpd(latest?.ppd ?? null)}
              </span>
            </div>
            <div className="detail-stat">
              <span className="label">Assigned CPUs</span>
              <span className="value mono">{displayConfiguredCpus(fah)}</span>
            </div>
            {fahCpuPolicyDrift(fah) && (
              <div className="detail-stat">
                <span className="label">Effective CPUs</span>
                <span className="value mono">{displayEffectiveCpus(fah)}</span>
              </div>
            )}
          </div>

          <Tabs
            tabs={DETAIL_TABS}
            active={tab}
            onChange={setTab}
            className="admin-machine-tabs"
          >
            {tab === "overview" && (
              <>
                <div className="range-bar">
                  <span className="range-label">History range</span>
                  <div className="range-buttons">
                    {RANGES.map((range) => (
                      <button
                        key={range.limit}
                        type="button"
                        className={`range-btn ${limit === range.limit ? "active" : ""}`}
                        onClick={() => setLimit(range.limit)}
                      >
                        {range.label}
                      </button>
                    ))}
                  </div>
                  <span className="range-hint">
                    {snapshotCount} snapshots - {rangeLabel}
                  </span>
                </div>

                <div className="admin-detail-grid">
                  <InfoCard title="Identity" items={identityItems} />
                  <InfoCard title="Folding@home" items={foldingItems} />
                </div>

                {projectId && (
                  <ProjectInfoPanel
                    projectId={projectId}
                    run={latest?.run ?? null}
                    clone={latest?.clone ?? null}
                    gen={latest?.gen ?? null}
                    info={projectInfo}
                    loading={projectLoading}
                    error={projectError}
                  />
                )}

                {history.length > 0 ? (
                  <div className="charts-grid admin-detail-charts">
                    <HistoryChart
                      title="FAH progress"
                      data={history}
                      series={[
                        {
                          key: "progress",
                          name: "Progress",
                          color: "#3d9eff",
                          unit: "%",
                          domain: [0, 100],
                        },
                      ]}
                    />
                    <HistoryChart
                      title="Points per day"
                      data={history}
                      series={[
                        {
                          key: "ppd",
                          name: "PPD",
                          color: "#34d399",
                          unit: "ppd",
                        },
                      ]}
                    />
                  </div>
                ) : (
                  <p className="message">No snapshot history for this node yet.</p>
                )}
              </>
            )}

            {tab === "hardware" && (
              <>
                <div className="admin-detail-grid">
                  <InfoCard title="Hardware inventory" items={hardwareItems} />
                  <InfoCard title="Health and capacity" items={healthItems} />
                </div>
                {hardware ? (
                  <div className="admin-detail-grid">
                    <InfoCard title="Platform details" items={hardwareDetailItems} />
                  </div>
                ) : (
                  <section className="admin-detail-placeholder">
                    <h3>Hardware profile pending</h3>
                    <p>
                      The node has not reported a persisted hardware profile yet.
                      Inventory is collected by foldingosctl inspect hardware during
                      agent ingest on FoldingOS nodes.
                    </p>
                  </section>
                )}
              </>
            )}

            {tab === "work" && (
              <>
                <div className="admin-detail-grid">
                  <InfoCard
                    title="Current work unit"
                    items={[
                      { label: "Assignment", value: projectLabel(machine), mono: true },
                      {
                        label: "Activity",
                        value: fah?.foldingDetail ?? activityLabel(activity),
                      },
                      {
                        label: "Progress",
                        value:
                          latest?.progress != null
                            ? `${latest.progress.toFixed(1)}%`
                            : "-",
                        mono: true,
                      },
                      { label: "TPF", value: fah?.tpf ?? "-", mono: true },
                      { label: "PPD", value: formatPpd(latest?.ppd ?? null), mono: true },
                      {
                        label: "Started",
                        value: workHistory?.active
                          ? formatChartDate(workHistory.active.started_at)
                          : "Not tracked yet",
                        mono: true,
                      },
                    ]}
                  />
                  <InfoCard
                    title="Work history"
                    items={[
                      {
                        label: "Completed WUs",
                        value: formatNumber(workHistory?.total ?? 0),
                        mono: true,
                      },
                      {
                        label: "In progress",
                        value: workHistory?.active
                          ? formatWorkUnitAssignment(
                              workHistory.active.project,
                              workHistory.active.run,
                              workHistory.active.clone,
                              workHistory.active.gen,
                            )
                          : "None tracked",
                        mono: true,
                      },
                      {
                        label: "Snapshot samples",
                        value: formatNumber(snapshotCount),
                        mono: true,
                      },
                    ]}
                  />
                </div>

                {workHistoryError && (
                  <p className="message error">{workHistoryError}</p>
                )}
                {workHistoryLoading && !workHistory && (
                  <p className="admin-muted">Loading completed work units...</p>
                )}

                {workHistory && workHistory.completed.length > 0 ? (
                  <div className="alert-history-table-wrap">
                    <table className="alert-history-table">
                      <thead>
                        <tr>
                          <th>Assignment</th>
                          <th>Started</th>
                          <th>Stopped</th>
                          <th>Duration</th>
                        </tr>
                      </thead>
                      <tbody>
                        {workHistory.completed.map((record) => (
                          <tr key={record.id}>
                            <td className="mono">
                              {formatWorkUnitAssignment(
                                record.project,
                                record.run,
                                record.clone,
                                record.gen,
                              )}
                            </td>
                            <td className="mono">{formatChartDate(record.started_at)}</td>
                            <td className="mono">{formatChartDate(record.stopped_at)}</td>
                            <td className="mono">
                              {formatWorkUnitDuration(record.started_at, record.stopped_at)}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                ) : (
                  !workHistoryLoading &&
                  !workHistoryError && (
                    <p className="message">
                      No completed work units recorded for this host yet. History is
                      captured when ingest snapshots show a new assignment.
                    </p>
                  )
                )}
              </>
            )}

            {tab === "logs" && (
              <MachineLogsPanel hostname={hostname} machine={machine} />
            )}

            {tab === "controls" && (
              <MachineControlsPanel hostname={hostname} machine={machine} />
            )}
          </Tabs>

          <FahStatsLinks
            donor={fah?.statsDonor}
            team={fah?.statsTeam}
            className="admin-detail-fah-stats"
          />
        </>
      )}
    </div>
  );
}
