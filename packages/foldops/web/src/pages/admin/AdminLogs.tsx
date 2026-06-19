import { useCallback, useEffect, useState } from "react";
import { Tabs, type TabItem } from "../../components/Tabs";
import { fetchSupervisorLogs } from "../../api";
import type { SupervisorLogSource } from "../../types";

const ERROR_RE = /\b(ERROR|FATAL|Exception|failed|error)\b/i;

const LOG_TABS: TabItem[] = [
  { id: "foldops", label: "FoldOps supervisor" },
  { id: "foldingosctl", label: "foldingosctl" },
];

export function AdminLogs() {
  const [logTab, setLogTab] = useState<SupervisorLogSource>("foldops");

  return (
    <>
      <p className="admin-intro">
        Recent journal output from this supervisor node. Use these logs when
        provisioning, HTTPS, fleet delegation, or software updates fail.
      </p>

      <section className="admin-section">
        <Tabs
          tabs={LOG_TABS}
          active={logTab}
          onChange={(id) => setLogTab(id as SupervisorLogSource)}
          className="machine-logs-tabs"
        >
          <SupervisorLogViewer source={logTab} />
        </Tabs>
      </section>
    </>
  );
}

function SupervisorLogViewer({ source }: { source: SupervisorLogSource }) {
  const [lines, setLines] = useState<string[]>([]);
  const [path, setPath] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const result = await fetchSupervisorLogs(source, { lines: 300 });
      setLines(result.lines);
      setPath(result.path);
      setUpdatedAt(result.updated_at);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load logs");
    } finally {
      setLoading(false);
    }
  }, [source]);

  useEffect(() => {
    load();
  }, [load]);

  return (
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
              Live · {formatTime(updatedAt)}
            </span>
          )}
        </div>
        <button
          type="button"
          className="log-viewer-refresh"
          disabled={loading}
          onClick={load}
        >
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>

      {error && <p className="message error">{error}</p>}

      {!loading && lines.length === 0 && !error && (
        <p className="message">No log lines available for this source yet.</p>
      )}

      {lines.length > 0 && (
        <pre className="log-viewer-pre">
          {lines.map((line, i) => (
            <div
              key={`${i}-${line.slice(0, 24)}`}
              className={
                ERROR_RE.test(line) ? "log-line log-line--error" : "log-line"
              }
            >
              <span className="log-line-no">{i + 1}</span>
              <span className="log-line-text">{line}</span>
            </div>
          ))}
        </pre>
      )}
    </div>
  );
}

function formatTime(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}
