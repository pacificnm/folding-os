import { useCallback, useEffect, useState } from "react";
import { PageLayout } from "../components/PageLayout";
import {
  fetchDeployRun,
  fetchDeployRuns,
  fetchMachines,
  startAgentDeploy,
} from "../api";
import type { DeployHostResult, DeployRun, MachineSummary } from "../types";

export function Deploy() {
  const [machines, setMachines] = useState<MachineSummary[]>([]);
  const [runs, setRuns] = useState<DeployRun[]>([]);
  const [activeRun, setActiveRun] = useState<DeployRun | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const load = useCallback(async () => {
    try {
      const [m, r] = await Promise.all([
        fetchMachines(),
        fetchDeployRuns(),
      ]);
      setMachines(m.machines);
      setRuns(r.runs);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load");
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    if (!activeRun || activeRun.status !== "running") return;
    const id = setInterval(async () => {
      try {
        const run = await fetchDeployRun(activeRun.id);
        setActiveRun(run);
        if (run.status !== "running") {
          load();
        }
      } catch {
        /* ignore poll errors */
      }
    }, 2000);
    return () => clearInterval(id);
  }, [activeRun, load]);

  const toggleHost = (hostname: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(hostname)) next.delete(hostname);
      else next.add(hostname);
      return next;
    });
  };

  const selectAll = () => {
    setSelected(new Set(machines.map((m) => m.hostname)));
  };

  const deploy = async (hostnames?: string[]) => {
    setStarting(true);
    setError(null);
    try {
      const { run_id } = await startAgentDeploy(hostnames);
      const run = await fetchDeployRun(run_id);
      setActiveRun(run);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Deploy failed to start");
    } finally {
      setStarting(false);
    }
  };

  const sorted = machines
    .slice()
    .sort((a, b) => a.hostname.localeCompare(b.hostname));

  return (
    <PageLayout
      backLink={{ href: "/dashboard", label: "← Farm dashboard" }}
      eyebrow="Operations"
      title="Deploy agents"
      footer="Runs git pull, npm install, build:agent, then restarts foldops-agent on each node"
    >
      <p className="deploy-intro">
        Push application updates from the supervisor to farm agents. Requires{" "}
        <code>UPDATE_ENABLED=true</code> on each node and a git checkout at{" "}
        <code>/opt/foldops</code>.
      </p>

      {error && <p className="message error">{error}</p>}

      <section className="deploy-section">
        <h2 className="deploy-heading">Targets</h2>
        <div className="deploy-actions">
          <button type="button" className="deploy-btn" onClick={selectAll}>
            Select all
          </button>
          <button
            type="button"
            className="deploy-btn deploy-btn--primary"
            disabled={starting}
            onClick={() => deploy()}
          >
            {starting ? "Starting…" : "Update all agents"}
          </button>
          <button
            type="button"
            className="deploy-btn deploy-btn--primary"
            disabled={starting || selected.size === 0}
            onClick={() => deploy([...selected])}
          >
            Update selected ({selected.size})
          </button>
        </div>

        <ul className="deploy-host-list">
          {sorted.map((m) => (
            <li key={m.hostname}>
              <label className="deploy-host-label">
                <input
                  type="checkbox"
                  checked={selected.has(m.hostname)}
                  onChange={() => toggleHost(m.hostname)}
                />
                <span className="mono">{m.hostname}</span>
                <span
                  className={`badge ${m.online ? "badge-ok" : "badge-warn"}`}
                >
                  {m.online ? "online" : "offline"}
                </span>
              </label>
            </li>
          ))}
        </ul>
      </section>

      {activeRun && (
        <section className="deploy-section">
          <h2 className="deploy-heading">
            Current run{" "}
            <span className={`deploy-run-status deploy-run-status--${activeRun.status}`}>
              {activeRun.status}
            </span>
          </h2>
          <DeployResultsTable results={activeRun.results} />
        </section>
      )}

      {runs.length > 0 && (
        <section className="deploy-section">
          <h2 className="deploy-heading">Recent runs</h2>
          <ul className="deploy-history">
            {runs.map((run) => (
              <li key={run.id}>
                <button
                  type="button"
                  className="deploy-history-btn"
                  onClick={() => setActiveRun(run)}
                >
                  <span>{new Date(run.created_at).toLocaleString()}</span>
                  <span className={`deploy-run-status deploy-run-status--${run.status}`}>
                    {run.status}
                  </span>
                  <span className="deploy-history-hosts">
                    {run.hostnames.join(", ")}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </PageLayout>
  );
}

function DeployResultsTable({
  results,
}: {
  results: Record<string, DeployHostResult>;
}) {
  const rows = Object.values(results).sort((a, b) =>
    a.hostname.localeCompare(b.hostname),
  );

  return (
    <div className="deploy-results">
      <table className="deploy-table">
        <thead>
          <tr>
            <th>Host</th>
            <th>Status</th>
            <th>Message</th>
            <th>Duration</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.hostname}>
              <td className="mono">{r.hostname}</td>
              <td>
                <span className={`deploy-host-status deploy-host-status--${r.status}`}>
                  {r.status}
                </span>
              </td>
              <td>{r.message}</td>
              <td className="mono">
                {r.duration_ms != null
                  ? `${(r.duration_ms / 1000).toFixed(1)}s`
                  : "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.some((r) => r.stdout || r.stderr) && (
        <div className="deploy-logs">
          {rows.map(
            (r) =>
              (r.stdout || r.stderr) && (
                <details key={r.hostname} className="deploy-log-details">
                  <summary className="mono">{r.hostname} output</summary>
                  {r.stdout && <pre className="deploy-log-pre">{r.stdout}</pre>}
                  {r.stderr && (
                    <pre className="deploy-log-pre deploy-log-pre--err">
                      {r.stderr}
                    </pre>
                  )}
                </details>
              ),
          )}
        </div>
      )}
    </div>
  );
}
