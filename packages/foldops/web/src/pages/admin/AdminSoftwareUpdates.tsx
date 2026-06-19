import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyLocalSoftware,
  fetchSoftwareInstallLog,
  fetchSoftwareUpdates,
  fleetApplyFoldops,
  fleetApplyTools,
  fleetAssign,
} from "../../api";
import type {
  FleetSoftwareApplyResult,
  SoftwareInstallLogEntry,
  SoftwareNodeVersions,
  SoftwareUpdatesResponse,
} from "../../types";

function versionCell(active: string | null, assigned: string | null, pending?: boolean) {
  const activeText = active ?? "—";
  const assignedText = assigned ?? "—";
  if (pending) {
    return (
      <span>
        <span className="mono">{activeText}</span>
        {" → "}
        <span className="mono admin-pending">{assignedText}</span>
      </span>
    );
  }
  return <span className="mono">{activeText}</span>;
}

export function AdminSoftwareUpdates() {
  const [data, setData] = useState<SoftwareUpdatesResponse | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [assignFoldops, setAssignFoldops] = useState("");
  const [assignTools, setAssignTools] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [applyResults, setApplyResults] = useState<FleetSoftwareApplyResult[]>([]);
  const [installLogRefresh, setInstallLogRefresh] = useState(0);

  const bumpInstallLog = () => setInstallLogRefresh((value) => value + 1);

  const load = useCallback(async (refresh = false) => {
    setLoading(true);
    try {
      const response = await fetchSoftwareUpdates(refresh);
      setData(response);
      const latestFoldops =
        response.upstream.foldops?.latest_manifest_release ?? "";
      const latestTools = response.upstream.tools?.latest_tools_version ?? "";
      if (refresh) {
        setAssignFoldops(latestFoldops);
        setAssignTools(latestTools);
      } else {
        setAssignFoldops((prev) => prev || latestFoldops);
        setAssignTools((prev) => prev || latestTools);
      }
      setError(null);
      return response;
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load updates");
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load(false);
  }, [load]);

  const agents = useMemo(
    () =>
      (data?.agents ?? [])
        .slice()
        .sort((a, b) => a.hostname.localeCompare(b.hostname)),
    [data],
  );

  const toggleHost = (hostname: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(hostname)) next.delete(hostname);
      else next.add(hostname);
      return next;
    });
  };

  const selectAllAgents = () => {
    setSelected(new Set(agents.map((agent) => agent.hostname)));
  };

  const selectedAgents = agents.filter((agent) => selected.has(agent.hostname));

  const runAssign = async (targets: SoftwareNodeVersions[], all = false) => {
    const foldops = assignFoldops.trim();
    const tools = assignTools.trim();
    if (!foldops && !tools) {
      setError("Enter a FoldOps manifest release and/or tools version to assign.");
      return;
    }

    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      if (all) {
        const response = await fleetAssign({
          all: true,
          foldops_manifest: foldops || undefined,
          tools_version: tools || undefined,
        });
        setStatus(`Assigned updates to ${response.result.updated_count} node(s).`);
      } else {
        let updated = 0;
        for (const target of targets) {
          if (!target.node_id) continue;
          const response = await fleetAssign({
            node_id: target.node_id,
            foldops_manifest: foldops || undefined,
            tools_version: tools || undefined,
          });
          updated += response.result.updated_count;
        }
        setStatus(`Assigned updates to ${updated} selected node(s).`);
      }
      await load(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Assignment failed");
    } finally {
      setBusy(false);
    }
  };

  const runUpdateSupervisorLatest = async () => {
    const foldops =
      assignFoldops.trim() ||
      data?.upstream.foldops?.latest_manifest_release?.trim() ||
      "";
    const tools =
      assignTools.trim() ||
      data?.upstream.tools?.latest_tools_version?.trim() ||
      "";
    if (!foldops && !tools) {
      setError("Check for updates first — no upstream releases are available.");
      return;
    }

    setBusy(true);
    setError(null);
    setStatus(null);
    setApplyResults([]);
    let applyFoldops = false;
    let applyTools = false;
    try {
      await fleetAssign({
        local: true,
        foldops_manifest: foldops || undefined,
        tools_version: tools || undefined,
      });
      const refreshed = await load(true);
      const supervisor = refreshed?.supervisor;
      applyFoldops = !!supervisor?.foldops_apply_pending;
      applyTools = !!supervisor?.tools_apply_pending;
      if (!applyFoldops && !applyTools) {
        setStatus("Supervisor is already on the latest assigned releases.");
        return;
      }
      const response = await applyLocalSoftware({
        foldops: applyFoldops,
        tools: applyTools,
      });
      setApplyResults(response.results);
      const summary = summarizeApplyResults(response.results);
      if (summary.error) {
        setError(summary.error);
      } else {
        setStatus(summary.status);
      }
      await load(true);
      bumpInstallLog();
    } catch (err) {
      if (applyFoldops || applyTools) {
        await recoverLocalApplyAfterConnectionDrop(
          err,
          applyFoldops,
          applyTools,
          load,
          setError,
          setStatus,
          bumpInstallLog,
        );
      } else {
        setError(err instanceof Error ? err.message : "Supervisor update failed");
      }
    } finally {
      setBusy(false);
    }
  };

  const supervisorApplyPending =
    data?.supervisor.foldops_apply_pending || data?.supervisor.tools_apply_pending;

  const supervisorUpdateAvailable =
    data?.supervisor.foldops_update_available ||
    data?.supervisor.tools_update_available ||
    supervisorApplyPending;

  const runAssignSupervisor = async () => {
    const foldops = assignFoldops.trim();
    const tools = assignTools.trim();
    if (!foldops && !tools) {
      setError("Enter a FoldOps manifest release and/or tools version to assign.");
      return;
    }

    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const response = await fleetAssign({
        local: true,
        foldops_manifest: foldops || undefined,
        tools_version: tools || undefined,
      });
      setStatus(
        `Assigned to supervisor (updated ${response.result.updated_count} enrollment record).`,
      );
      await load(true);
      bumpInstallLog();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Supervisor assignment failed");
    } finally {
      setBusy(false);
    }
  };

  const runApply = async (
    kind: "foldops" | "tools",
    scope: "local" | "selected" | "all",
  ) => {
    setBusy(true);
    setError(null);
    setStatus(null);
    setApplyResults([]);
    try {
      let response;
      if (scope === "local") {
        response = await applyLocalSoftware({
          foldops: kind === "foldops",
          tools: kind === "tools",
        });
      } else if (scope === "all") {
        response =
          kind === "foldops"
            ? await fleetApplyFoldops({ all: true })
            : await fleetApplyTools({ all: true });
      } else {
        const hostnames = selectedAgents.map((agent) => agent.hostname);
        if (hostnames.length === 0) {
          throw new Error("Select at least one agent.");
        }
        response =
          kind === "foldops"
            ? await fleetApplyFoldops({ hostnames })
            : await fleetApplyTools({ hostnames });
      }
      setApplyResults(response.results);
      const summary = summarizeApplyResults(response.results);
      if (summary.error) {
        setError(summary.error);
      } else if (scope === "local") {
        setStatus(summary.status ?? "Supervisor apply completed.");
      } else {
        setStatus(`${kind} apply completed for ${response.results.length} node(s).`);
      }
      await load(true);
      bumpInstallLog();
    } catch (err) {
      if (scope === "local") {
        await recoverLocalApplyAfterConnectionDrop(
          err,
          kind === "foldops",
          kind === "tools",
          load,
          setError,
          setStatus,
          bumpInstallLog,
        );
      } else {
        setError(err instanceof Error ? err.message : "Apply failed");
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <p className="admin-intro">
        Click Check for updates to refresh the packages channel, then Install
        latest on supervisor to assign and apply FoldOps and tools on this node
        without reimaging.
      </p>

      <div className="admin-toolbar">
        <button
          type="button"
          className="deploy-btn"
          disabled={busy || loading}
          onClick={() => load(true)}
        >
          {loading ? "Checking…" : "Check for updates"}
        </button>
        <button
          type="button"
          className="deploy-btn deploy-btn--primary"
          disabled={busy || loading || !data}
          onClick={runUpdateSupervisorLatest}
        >
          {busy ? "Updating…" : "Install latest on supervisor"}
        </button>
        {data?.checked_at && (
          <span className="admin-muted">
            Last checked {new Date(data.checked_at).toLocaleString()}
          </span>
        )}
      </div>

      {supervisorUpdateAvailable && data && (
        <p className="message admin-status">
          {supervisorApplyPending
            ? `Apply pending — ${formatPendingUpdates(data.supervisor)}. Click Install latest on supervisor.`
            : `Update available on packages — ${formatUpstreamUpdates(data)}. Click Check for updates, then Install latest on supervisor.`}
        </p>
      )}

      {error && <p className="message error">{error}</p>}
      {status && <p className="message admin-status">{status}</p>}

      {data && (
        <section className="admin-section">
          <h2 className="deploy-heading">Upstream packages channel</h2>
          <div className="admin-upstream-grid">
            <div className="admin-upstream-card">
              <h3>FoldOps</h3>
              <p className="mono">
                {data.upstream.foldops?.latest_manifest_release ?? "—"}
              </p>
              {data.upstream.foldops?.published_at && (
                <p className="admin-muted">
                  Published{" "}
                  {new Date(data.upstream.foldops.published_at).toLocaleString()}
                </p>
              )}
            </div>
            <div className="admin-upstream-card">
              <h3>foldingosctl tools</h3>
              <p className="mono">
                {data.upstream.tools?.latest_tools_version ?? "—"}
              </p>
              {data.upstream.tools?.published_at && (
                <p className="admin-muted">
                  Published{" "}
                  {new Date(data.upstream.tools.published_at).toLocaleString()}
                </p>
              )}
            </div>
          </div>
        </section>
      )}

      {data && (
        <section className="admin-section">
          <h2 className="deploy-heading">Supervisor</h2>
          <AdminNodeTable
            rows={[data.supervisor]}
            showCheckbox={false}
            selected={selected}
            onToggle={() => {}}
            versionCell={versionCell}
          />
          <div className="deploy-actions">
            <button
              type="button"
              className="deploy-btn"
              disabled={busy}
              onClick={runAssignSupervisor}
            >
              Assign to supervisor
            </button>
            <button
              type="button"
              className="deploy-btn deploy-btn--primary"
              disabled={busy || !data.supervisor.foldops_apply_pending}
              onClick={() => runApply("foldops", "local")}
            >
              Apply FoldOps on supervisor
            </button>
            <button
              type="button"
              className="deploy-btn deploy-btn--primary"
              disabled={busy || !data.supervisor.tools_apply_pending}
              onClick={() => runApply("tools", "local")}
            >
              Apply tools on supervisor
            </button>
          </div>
        </section>
      )}

      <section className="admin-section">
        <h2 className="deploy-heading">Assign versions</h2>
        <div className="admin-assign-form">
          <label>
            FoldOps manifest release
            <input
              className="admin-input mono"
              value={assignFoldops}
              onChange={(event) => setAssignFoldops(event.target.value)}
              placeholder="0.1.0-2"
            />
          </label>
          <label>
            Tools version
            <input
              className="admin-input mono"
              value={assignTools}
              onChange={(event) => setAssignTools(event.target.value)}
              placeholder="0.1.1"
            />
          </label>
        </div>
        <div className="deploy-actions">
          <button type="button" className="deploy-btn" onClick={selectAllAgents}>
            Select all agents
          </button>
          <button
            type="button"
            className="deploy-btn deploy-btn--primary"
            disabled={busy || selectedAgents.length === 0}
            onClick={() => runAssign(selectedAgents)}
          >
            Assign to selected ({selectedAgents.length})
          </button>
          <button
            type="button"
            className="deploy-btn deploy-btn--primary"
            disabled={busy}
            onClick={() => runAssign([], true)}
          >
            Assign to all enrolled
          </button>
        </div>
      </section>

      {data && (
        <section className="admin-section">
          <h2 className="deploy-heading">Fleet agents</h2>
          <div className="deploy-actions">
            <button
              type="button"
              className="deploy-btn deploy-btn--primary"
              disabled={busy || selectedAgents.length === 0}
              onClick={() => runApply("foldops", "selected")}
            >
              Apply FoldOps to selected
            </button>
            <button
              type="button"
              className="deploy-btn deploy-btn--primary"
              disabled={busy || selectedAgents.length === 0}
              onClick={() => runApply("tools", "selected")}
            >
              Apply tools to selected
            </button>
            <button
              type="button"
              className="deploy-btn"
              disabled={busy}
              onClick={() => runApply("foldops", "all")}
            >
              Apply FoldOps to all online
            </button>
            <button
              type="button"
              className="deploy-btn"
              disabled={busy}
              onClick={() => runApply("tools", "all")}
            >
              Apply tools to all online
            </button>
          </div>
          <AdminNodeTable
            rows={agents}
            showCheckbox
            selected={selected}
            onToggle={toggleHost}
            versionCell={versionCell}
          />
        </section>
      )}

      {applyResults.length > 0 && (
        <section className="admin-section">
          <h2 className="deploy-heading">Apply results</h2>
          <ApplyResultsTable results={applyResults} />
        </section>
      )}

      <InstallLogSection refreshToken={installLogRefresh} />
    </>
  );
}

function AdminNodeTable({
  rows,
  showCheckbox,
  selected,
  onToggle,
  versionCell: renderVersion = versionCell,
}: {
  rows: SoftwareNodeVersions[];
  showCheckbox: boolean;
  selected: Set<string>;
  onToggle: (hostname: string) => void;
  versionCell?: typeof versionCell;
}) {
  return (
    <div className="deploy-results">
      <table className="deploy-table admin-table">
        <thead>
          <tr>
            {showCheckbox && <th />}
            <th>Host</th>
            <th>Status</th>
            <th>FoldOps active → assigned</th>
            <th>Tools active → assigned</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.hostname}>
              {showCheckbox && (
                <td>
                  <input
                    type="checkbox"
                    checked={selected.has(row.hostname)}
                    onChange={() => onToggle(row.hostname)}
                    aria-label={`Select ${row.hostname}`}
                  />
                </td>
              )}
              <td className="mono">{row.hostname}</td>
              <td>
                {row.online === undefined ? (
                  "supervisor"
                ) : (
                  <span
                    className={`badge ${row.online ? "badge-ok" : "badge-warn"}`}
                  >
                    {row.online ? "online" : "offline"}
                  </span>
                )}
              </td>
              <td>
                {renderVersion(
                  row.active_foldops_manifest_release,
                  row.assigned_foldops_manifest_release,
                  row.foldops_apply_pending,
                )}
              </td>
              <td>
                {renderVersion(
                  row.active_tools_version,
                  row.assigned_tools_version,
                  row.tools_apply_pending,
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function isLikelyApplyConnectionDrop(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }
  const message = error.message.toLowerCase();
  return message.includes("failed to fetch") || message.includes("networkerror");
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function installLogMatchesComponent(
  entry: SoftwareInstallLogEntry,
  component: "foldops" | "tools",
): boolean {
  if (entry.phase !== "apply" || entry.operation !== "acquire") {
    return false;
  }
  const command = entry.command.toLowerCase();
  return component === "foldops"
    ? command.startsWith("foldops acquire")
    : command.startsWith("tools acquire");
}

async function recoverLocalApplyAfterConnectionDrop(
  error: unknown,
  needFoldops: boolean,
  needTools: boolean,
  load: (refresh?: boolean) => Promise<SoftwareUpdatesResponse | null>,
  setError: (value: string | null) => void,
  setStatus: (value: string | null) => void,
  bumpInstallLog: () => void,
): Promise<void> {
  if (!isLikelyApplyConnectionDrop(error)) {
    setError(error instanceof Error ? error.message : "Apply failed");
    return;
  }

  setError(null);
  setStatus("Connection dropped while applying — checking install status…");
  await sleep(2500);

  for (let attempt = 0; attempt < 15; attempt += 1) {
    try {
      const log = await fetchSoftwareInstallLog(40);
      const recentFailures = log.entries.filter(
        (entry) =>
          !entry.ok &&
          ((needFoldops && installLogMatchesComponent(entry, "foldops")) ||
            (needTools && installLogMatchesComponent(entry, "tools"))),
      );
      if (recentFailures.length > 0) {
        setStatus(null);
        setError(recentFailures.at(-1)?.message ?? "Supervisor apply failed");
        bumpInstallLog();
        return;
      }

      const foldopsOk =
        !needFoldops ||
        log.entries.some(
          (entry) => entry.ok && installLogMatchesComponent(entry, "foldops"),
        );
      const toolsOk =
        !needTools ||
        log.entries.some(
          (entry) => entry.ok && installLogMatchesComponent(entry, "tools"),
        );

      const refreshed = await load(true);
      const pendingCleared =
        refreshed &&
        (!needFoldops || !refreshed.supervisor.foldops_apply_pending) &&
        (!needTools || !refreshed.supervisor.tools_apply_pending);

      if (pendingCleared && foldopsOk && toolsOk) {
        setStatus("Supervisor updated to the latest assigned releases.");
        bumpInstallLog();
        return;
      }
    } catch {
      // HTTPS proxy may still be restarting.
    }
    await sleep(1000);
  }

  setStatus(null);
  setError(
    "Connection lost during update. Refresh the page to confirm whether it completed.",
  );
}

function summarizeApplyResults(results: FleetSoftwareApplyResult[]): {
  status: string | null;
  error: string | null;
} {
  const failures = results.filter((result) => !result.ok);
  if (failures.length > 0) {
    return {
      status: null,
      error: `${failures.length} component(s) failed to apply — see results below.`,
    };
  }

  const applied = results.filter((result) => result.ok && !result.skipped);
  const skipped = results.filter((result) => result.ok && result.skipped);
  if (applied.length === 0 && skipped.length > 0) {
    return {
      status: null,
      error:
        skipped.map((result) => result.message).filter(Boolean).join(" ") ||
        "Nothing to apply — assigned versions already match active releases.",
    };
  }

  const messages = applied
    .map((result) => result.message)
    .filter(Boolean);
  return {
    status:
      messages.join(" ") ||
      "Supervisor updated to the latest assigned releases.",
    error: null,
  };
}

function formatPendingUpdates(supervisor: SoftwareNodeVersions): string {
  const parts: string[] = [];
  if (supervisor.foldops_apply_pending) {
    parts.push(
      `FoldOps ${supervisor.active_foldops_manifest_release ?? "—"} → ${supervisor.assigned_foldops_manifest_release ?? "—"}`,
    );
  }
  if (supervisor.tools_apply_pending) {
    parts.push(
      `tools ${supervisor.active_tools_version ?? "—"} → ${supervisor.assigned_tools_version ?? "—"}`,
    );
  }
  return parts.join(", ");
}

function formatUpstreamUpdates(data: SoftwareUpdatesResponse): string {
  const parts: string[] = [];
  if (data.supervisor.foldops_update_available) {
    parts.push(
      `FoldOps ${data.upstream.foldops?.latest_manifest_release ?? "—"}`,
    );
  }
  if (data.supervisor.tools_update_available) {
    parts.push(`tools ${data.upstream.tools?.latest_tools_version ?? "—"}`);
  }
  return parts.join(", ");
}

function InstallLogSection({ refreshToken }: { refreshToken: number }) {
  const [entries, setEntries] = useState<SoftwareInstallLogEntry[]>([]);
  const [path, setPath] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetchSoftwareInstallLog(200);
      setEntries(response.entries.slice().reverse());
      setPath(response.path);
      setUpdatedAt(response.updated_at);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load install log");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load, refreshToken]);

  return (
    <section className="admin-section">
      <h2 className="deploy-heading">Install log</h2>
      <p className="admin-intro">
        Assign, import, and apply steps run by the supervisor during software
        updates. Use this log to see exact foldingosctl output and errors.
      </p>
      <div className="log-viewer">
        <div className="log-viewer-toolbar">
          <div className="log-viewer-meta">
            {path && (
              <span className="log-viewer-path mono" title={path}>
                {path}
              </span>
            )}
            {updatedAt && (
              <span className="log-viewer-time">
                Refreshed {new Date(updatedAt).toLocaleString()}
              </span>
            )}
          </div>
          <button
            type="button"
            className="log-viewer-refresh"
            disabled={loading}
            onClick={load}
          >
            {loading ? "Loading…" : "Refresh log"}
          </button>
        </div>
        {error && <p className="message error">{error}</p>}
        <pre className="log-viewer-pre install-log-pre">
          {entries.length === 0 && !loading
            ? "No install log entries yet. Run Check for updates, Assign, or Install latest."
            : entries.map(formatInstallLogEntry).join("\n\n")}
        </pre>
      </div>
    </section>
  );
}

function formatInstallLogEntry(entry: SoftwareInstallLogEntry): string {
  const status = entry.ok ? "OK" : "FAILED";
  const lines = [
    `[${entry.timestamp}] ${entry.phase}/${entry.operation} ${status}${entry.exit_code != null ? ` (exit ${entry.exit_code})` : ""}`,
  ];
  if (entry.command) {
    lines.push(`command: ${entry.command}`);
  }
  if (entry.message) {
    lines.push(`message: ${entry.message}`);
  }
  if (entry.stderr) {
    lines.push(`stderr:\n${entry.stderr}`);
  }
  if (entry.stdout) {
    lines.push(`stdout:\n${entry.stdout}`);
  }
  if (entry.detail) {
    lines.push(`detail:\n${JSON.stringify(entry.detail, null, 2)}`);
  }
  return lines.join("\n");
}

function ApplyResultsTable({
  results,
}: {
  results: FleetSoftwareApplyResult[];
}) {
  const rows = results.slice().sort((a, b) => {
    const left = a.hostname ?? a.component ?? "";
    const right = b.hostname ?? b.component ?? "";
    return left.localeCompare(right);
  });

  return (
    <div className="deploy-results">
      <table className="deploy-table">
        <thead>
          <tr>
            <th>Target</th>
            <th>Status</th>
            <th>Message</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
            const label = row.hostname ?? row.component ?? "—";
            const status = row.skipped
              ? "skipped"
              : row.ok
                ? "success"
                : "failed";
            return (
              <tr key={label}>
                <td className="mono">{label}</td>
                <td>
                  <span className={`deploy-host-status deploy-host-status--${status}`}>
                    {status}
                  </span>
                </td>
                <td>{row.message ?? row.error ?? "—"}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
