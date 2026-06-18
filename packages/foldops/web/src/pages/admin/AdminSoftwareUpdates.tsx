import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyLocalSoftware,
  assignLocalSoftware,
  fetchSoftwareUpdates,
  fleetApplyFoldops,
  fleetApplyTools,
  fleetAssign,
} from "../../api";
import type {
  FleetSoftwareApplyResult,
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

  const load = useCallback(async (refresh = false) => {
    setLoading(true);
    try {
      const response = await fetchSoftwareUpdates(refresh);
      setData(response);
      setAssignFoldops((prev) =>
        prev ||
        response.upstream.foldops?.latest_manifest_release ||
        "",
      );
      setAssignTools((prev) =>
        prev || response.upstream.tools?.latest_tools_version || "",
      );
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load updates");
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
      const response = await assignLocalSoftware({
        foldops_manifest: foldops || undefined,
        tools_version: tools || undefined,
      });
      setStatus(
        `Assigned to supervisor (FoldOps: ${response.result.foldops_manifest_release ?? "unchanged"}, tools: ${response.result.tools_version ?? "unchanged"}).`,
      );
      await load(true);
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
      const failures = response.results.filter((result) => !result.ok);
      const skipped = response.results.filter((result) => result.ok && result.skipped);
      if (failures.length === 0) {
        if (scope === "local" && skipped.length > 0) {
          const message =
            skipped[0]?.message ??
            "Assigned version already matches active release — assign a newer release first.";
          setStatus(`Supervisor apply skipped: ${message}`);
        } else if (scope === "local") {
          const message =
            response.results[0]?.message ?? "Supervisor apply completed.";
          setStatus(message);
        } else {
          setStatus(`${kind} apply completed for ${response.results.length} node(s).`);
        }
      } else {
        setError(
          `${failures.length} node(s) failed ${kind} apply — see results below.`,
        );
      }
      await load(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Apply failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <p className="admin-intro">
        Check packages-channel indexes, assign desired FoldOps manifest and tools
        versions, then apply updates to the supervisor and enrolled agents without
        reimaging.
      </p>

      <div className="admin-toolbar">
        <button
          type="button"
          className="deploy-btn deploy-btn--primary"
          disabled={busy || loading}
          onClick={() => load(true)}
        >
          {loading ? "Checking…" : "Check for updates"}
        </button>
        {data?.checked_at && (
          <span className="admin-muted">
            Last checked {new Date(data.checked_at).toLocaleString()}
          </span>
        )}
      </div>

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
          />
          {data.supervisor.foldops_apply_pending && (
            <p className="admin-muted">
              Update pending: assigned{" "}
              <span className="mono">
                {data.supervisor.assigned_foldops_manifest_release}
              </span>{" "}
              — assign first if needed, then apply.
            </p>
          )}
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
              disabled={busy}
              onClick={() => runApply("foldops", "local")}
            >
              Apply FoldOps on supervisor
            </button>
            <button
              type="button"
              className="deploy-btn deploy-btn--primary"
              disabled={busy}
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
