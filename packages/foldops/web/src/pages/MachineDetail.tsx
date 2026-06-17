import { useCallback, useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import { HistoryChart } from "../components/HistoryChart";
import { MachineControlsPanel } from "../components/MachineControlsPanel";
import { MachineLogsPanel } from "../components/MachineLogsPanel";
import { PageLayout } from "../components/PageLayout";
import { FahStatsLinks } from "../components/FahStatsLinks";
import { ProjectInfoPanel } from "../components/ProjectInfoPanel";
import { Tabs, type TabItem } from "../components/Tabs";
import { fetchFahProject, fetchMachine, fetchSnapshots } from "../api";
import { snapshotsToHistory } from "../history";
import type { FahProjectInfo, HistoryPoint, MachineSummary } from "../types";
import {
  formatLastSeen,
  formatPpd,
  formatTemp,
} from "../utils/format";

const RANGES = [
  { label: "2 hours", limit: 120 },
  { label: "8 hours", limit: 480 },
  { label: "24 hours", limit: 500 },
] as const;

const PAGE_TABS: TabItem[] = [
  { id: "overview", label: "Overview" },
  { id: "logs", label: "Logs" },
  { id: "control", label: "Control" },
];

export function MachineDetail() {
  const { hostname: encoded } = useParams<{ hostname: string }>();
  const hostname = encoded ? decodeURIComponent(encoded) : "";

  const [pageTab, setPageTab] = useState("overview");
  const [machine, setMachine] = useState<MachineSummary | null>(null);
  const [history, setHistory] = useState<HistoryPoint[]>([]);
  const [limit, setLimit] = useState<number>(480);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [projectInfo, setProjectInfo] = useState<FahProjectInfo | null>(null);
  const [projectLoading, setProjectLoading] = useState(false);
  const [projectError, setProjectError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!hostname) return;
    try {
      const [m, snaps] = await Promise.all([
        fetchMachine(hostname),
        fetchSnapshots(hostname, limit),
      ]);
      setMachine(m);
      setHistory(snapshotsToHistory(snaps.snapshots));
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  }, [hostname, limit]);

  useEffect(() => {
    setLoading(true);
    load();
    const id = setInterval(load, 60_000);
    return () => clearInterval(id);
  }, [load]);

  const latest = machine?.latest;
  const projectId = latest?.project ?? null;

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

  const snapshotCount = history.length;

  const rangeLabel = useMemo(
    () => RANGES.find((r) => r.limit === limit)?.label ?? `${limit} samples`,
    [limit],
  );

  return (
    <PageLayout
      backLink={{ href: "/dashboard", label: "← Farm dashboard" }}
      eyebrow="Node history"
      title={hostname || "Unknown"}
      badge={
        machine ? (
          <span
            className={`badge ${machine.online ? "badge-ok" : "badge-warn"}`}
          >
            {machine.online ? "online" : "offline"}
          </span>
        ) : undefined
      }
      footer={
        pageTab === "overview"
          ? "History refreshes every 60s"
          : "Logs refresh on demand · snapshot cache updates every 60s"
      }
      headerAside={
        <div className="header-meta">
          {latest?.project && (
            <span className="mono">
              Project {latest.project}
              {latest.run != null &&
                ` · R${latest.run}/C${latest.clone}/G${latest.gen}`}
            </span>
          )}
          {machine && (
            <span>Last seen {formatLastSeen(machine.last_seen)}</span>
          )}
          <FahStatsLinks
            donor={latest?.payload?.fah?.statsDonor}
            team={latest?.payload?.fah?.statsTeam}
          />
        </div>
      }
    >
      <Tabs
        tabs={PAGE_TABS}
        active={pageTab}
        onChange={setPageTab}
        className="machine-page-tabs"
      >
        {pageTab === "overview" && (
          <>
            <div className="range-bar">
              <span className="range-label">Time range</span>
              <div className="range-buttons">
                {RANGES.map((r) => (
                  <button
                    key={r.limit}
                    type="button"
                    className={`range-btn ${limit === r.limit ? "active" : ""}`}
                    onClick={() => setLimit(r.limit)}
                  >
                    {r.label}
                  </button>
                ))}
              </div>
              <span className="range-hint">
                {snapshotCount} snapshots · ~1/min · {rangeLabel}
              </span>
            </div>

            {loading && !history.length && (
              <p className="message">Loading history…</p>
            )}
            {error && <p className="message error">{error}</p>}

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

            {!loading && history.length > 0 && (
              <>
                <div className="detail-stats">
                  <div className="detail-stat">
                    <span className="label">Current PPD</span>
                    <span className="value mono highlight">
                      {formatPpd(latest?.ppd ?? null)}
                    </span>
                  </div>
                  <div className="detail-stat">
                    <span className="label">Progress</span>
                    <span className="value mono">
                      {latest?.progress != null
                        ? `${latest.progress.toFixed(1)}%`
                        : "—"}
                    </span>
                  </div>
                  <div className="detail-stat">
                    <span className="label">CPU</span>
                    <span className="value mono">
                      {latest?.cpu_usage != null ? `${latest.cpu_usage}%` : "—"}
                    </span>
                  </div>
                  <div className="detail-stat">
                    <span className="label">Temps</span>
                    <span className="value mono">
                      {formatTemp(latest?.cpu_temp)} /{" "}
                      {formatTemp(latest?.chassis_temp)}
                    </span>
                  </div>
                </div>

                <div className="charts-grid">
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
                  <HistoryChart
                    title="CPU & memory"
                    data={history}
                    series={[
                      {
                        key: "cpu",
                        name: "CPU",
                        color: "#fbbf24",
                        unit: "%",
                        domain: [0, 100],
                      },
                      {
                        key: "memory",
                        name: "Memory",
                        color: "#a78bfa",
                        unit: "%",
                        domain: [0, 100],
                      },
                    ]}
                    height={220}
                  />
                  <HistoryChart
                    title="Disk usage"
                    data={history}
                    series={[
                      {
                        key: "disk",
                        name: "Disk",
                        color: "#fb923c",
                        unit: "%",
                        domain: [0, 100],
                      },
                    ]}
                  />
                  <HistoryChart
                    title="Temperatures"
                    data={history}
                    series={[
                      {
                        key: "cpuTemp",
                        name: "CPU",
                        color: "#f87171",
                        unit: "°C",
                      },
                      {
                        key: "chassisTemp",
                        name: "Chassis",
                        color: "#38bdf8",
                        unit: "°C",
                      },
                    ]}
                    height={220}
                  />
                </div>
              </>
            )}

            {!loading && !error && history.length === 0 && (
              <p className="message">No snapshot history for this node yet.</p>
            )}
          </>
        )}

        {pageTab === "logs" && (
          <MachineLogsPanel hostname={hostname} machine={machine} />
        )}

        {pageTab === "control" && (
          <MachineControlsPanel hostname={hostname} machine={machine} />
        )}
      </Tabs>
    </PageLayout>
  );
}
