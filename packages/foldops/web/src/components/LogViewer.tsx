import { useCallback, useEffect, useState } from "react";
import { fetchMachineLogs } from "../api";
import type { LogSource, MachineLogsResponse } from "../types";

const ERROR_RE = /\b(ERROR|FATAL|Exception|failed)\b/i;

interface LogViewerProps {
  hostname: string;
  source: LogSource;
  online: boolean;
}

export function LogViewer({ hostname, source, online }: LogViewerProps) {
  const [data, setData] = useState<MachineLogsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    async (live = true) => {
      setLoading(true);
      try {
        const result = await fetchMachineLogs(hostname, source, {
          lines: 300,
          live,
        });
        setData(result);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load logs");
      } finally {
        setLoading(false);
      }
    },
    [hostname, source],
  );

  useEffect(() => {
    load(true);
  }, [load]);

  const lines = data?.lines ?? [];

  return (
    <div className="log-viewer">
      <div className="log-viewer-toolbar">
        <div className="log-viewer-meta">
          {data?.path && (
            <span className="log-viewer-path mono" title={data.path}>
              {data.path}
            </span>
          )}
          {data?.updated_at && (
            <span className="log-viewer-time">
              {data.live ? "Live" : "Snapshot"} · {formatTime(data.updated_at)}
            </span>
          )}
          {!online && (
            <span className="log-viewer-offline">Node offline</span>
          )}
        </div>
        <button
          type="button"
          className="log-viewer-refresh"
          disabled={loading}
          onClick={() => load(true)}
        >
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>

      {data?.warning && (
        <div className="log-viewer-warning">
          <p>{data.warning}</p>
          {data.live_url && (
            <p className="log-viewer-warning-detail mono">
              Tried: {data.live_url}
            </p>
          )}
        </div>
      )}
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

interface ErrorsLogViewerProps {
  hostname: string;
  recentErrors: string[];
  fahLines: string[];
  online: boolean;
}

export function ErrorsLogViewer({
  hostname,
  recentErrors,
  fahLines,
  online,
}: ErrorsLogViewerProps) {
  const fromFah = fahLines.filter((l) => ERROR_RE.test(l));
  const merged = [...new Set([...recentErrors, ...fromFah])].slice(-50);

  const [liveErrors, setLiveErrors] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const result = await fetchMachineLogs(hostname, "fah", {
        lines: 400,
        live: true,
      });
      setLiveErrors(
        result.lines.filter((l) => ERROR_RE.test(l)),
      );
    } catch {
      setLiveErrors([]);
    } finally {
      setLoading(false);
    }
  }, [hostname]);

  useEffect(() => {
    if (online) refresh();
  }, [online, refresh]);

  const shown =
    liveErrors.length > 0
      ? [...new Set([...merged, ...liveErrors])].slice(-50)
      : merged;

  return (
    <div className="log-viewer">
      <div className="log-viewer-toolbar">
        <span className="log-viewer-meta">
          {shown.length} error line{shown.length === 1 ? "" : "s"} (from FAH log)
        </span>
        <button
          type="button"
          className="log-viewer-refresh"
          disabled={loading || !online}
          onClick={refresh}
        >
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>
      {shown.length === 0 && (
        <p className="message">No recent errors reported.</p>
      )}
      {shown.length > 0 && (
        <pre className="log-viewer-pre">
          {shown.map((line, i) => (
            <div key={`${i}-${line.slice(0, 24)}`} className="log-line log-line--error">
              <span className="log-line-no">{i + 1}</span>
              <span className="log-line-text">{line}</span>
            </div>
          ))}
        </pre>
      )}
    </div>
  );
}
