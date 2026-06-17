import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { PageLayout } from "../components/PageLayout";
import {
  fetchAlertHistory,
  fetchAlertsStatus,
  fetchMachines,
  sendAlertTest,
} from "../api";
import type { AlertsStatusResponse } from "../types";
import { formatDuration, formatKindLabel } from "../utils/alerts";
import type { AlertHistoryFilter, AlertHistoryItem } from "../types";

const FILTERS: { id: AlertHistoryFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "active", label: "Active" },
  { id: "resolved", label: "Resolved" },
];

export function AlertHistory() {
  const [filter, setFilter] = useState<AlertHistoryFilter>("all");
  const [hostname, setHostname] = useState("");
  const [alerts, setAlerts] = useState<AlertHistoryItem[]>([]);
  const [counts, setCounts] = useState({ active: 0, resolved: 0, total: 0 });
  const [hosts, setHosts] = useState<string[]>([]);
  const [alertStatus, setAlertStatus] = useState<AlertsStatusResponse | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [testMsg, setTestMsg] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    try {
      const [history, machines, status] = await Promise.all([
        fetchAlertHistory({
          status: filter,
          limit: 200,
          hostname: hostname || undefined,
        }),
        fetchMachines(),
        fetchAlertsStatus(),
      ]);
      setAlerts(history.alerts);
      setCounts(history.counts);
      setAlertStatus(status);
      setHosts(
        machines.machines
          .map((m) => m.hostname)
          .sort((a, b) => a.localeCompare(b)),
      );
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load alerts");
    } finally {
      setLoading(false);
    }
  }, [filter, hostname]);

  const runTest = async () => {
    setTesting(true);
    setTestMsg(null);
    try {
      const result = await sendAlertTest();
      setTestMsg(result.message);
      await load();
    } catch (err) {
      setTestMsg(err instanceof Error ? err.message : "Test failed");
    } finally {
      setTesting(false);
    }
  };

  useEffect(() => {
    setLoading(true);
    load();
    const id = setInterval(load, 30_000);
    return () => clearInterval(id);
  }, [load]);

  return (
    <PageLayout
      backLink={{ href: "/dashboard", label: "← Farm dashboard" }}
      eyebrow="Operations"
      title="Alert history"
      footer="Auto-refresh every 30s · Discord: one embed per event"
      headerAside={
        <div className="hero-stats">
          <div className="hero-stat">
            <span className="label">Active</span>
            <span className="value">{counts.active}</span>
          </div>
          <div className="hero-stat">
            <span className="label">Resolved</span>
            <span className="value">{counts.resolved}</span>
          </div>
        </div>
      }
    >
      {alertStatus && (
        <div className="alert-discord-status">
          <p>
            Alerts {alertStatus.enabled ? "enabled" : "disabled"}
            {alertStatus.webhook_configured
              ? alertStatus.discord
                ? " · Discord webhook"
                : " · Webhook"
              : " · no webhook URL"}
            {alertStatus.webhook.last_success_at && (
              <span className="alert-discord-meta">
                {" "}
                · last sent {formatTime(alertStatus.webhook.last_success_at)}
              </span>
            )}
          </p>
          {alertStatus.webhook.last_error && (
            <p className="message error">{alertStatus.webhook.last_error}</p>
          )}
          <button
            type="button"
            className="deploy-btn"
            disabled={testing || !alertStatus.webhook_configured}
            onClick={runTest}
          >
            {testing ? "Sending…" : "Test Discord webhook"}
          </button>
          {testMsg && <p className="message machine-controls-ok">{testMsg}</p>}
        </div>
      )}

      <div className="alert-history-toolbar">
        <div className="range-buttons">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              type="button"
              className={`range-btn ${filter === f.id ? "active" : ""}`}
              onClick={() => setFilter(f.id)}
            >
              {f.label}
            </button>
          ))}
        </div>

        <label className="alert-history-host-filter">
          <span className="range-label">Host</span>
          <select
            value={hostname}
            onChange={(e) => setHostname(e.target.value)}
          >
            <option value="">All nodes</option>
            {hosts.map((h) => (
              <option key={h} value={h}>
                {h}
              </option>
            ))}
          </select>
        </label>
      </div>

      {loading && alerts.length === 0 && (
        <p className="message">Loading alert history…</p>
      )}
      {error && <p className="message error">{error}</p>}

      {!loading && alerts.length === 0 && !error && (
        <p className="message">No alerts in this view yet.</p>
      )}

      {alerts.length > 0 && (
        <div className="alert-history-table-wrap">
          <table className="alert-history-table">
            <thead>
              <tr>
                <th>Status</th>
                <th>Severity</th>
                <th>Host</th>
                <th>Type</th>
                <th>Message</th>
                <th>Fired</th>
                <th>Duration</th>
              </tr>
            </thead>
            <tbody>
              {alerts.map((a) => (
                <tr key={`${a.id}-${a.fired_at}`} className={a.active ? "alert-history-row--active" : ""}>
                  <td>
                    <span
                      className={`alert-history-status ${a.active ? "alert-history-status--active" : "alert-history-status--resolved"}`}
                    >
                      {a.active ? "active" : "resolved"}
                    </span>
                  </td>
                  <td>
                    <span className={`alert-history-severity alert-history-severity--${a.severity}`}>
                      {a.severity}
                    </span>
                  </td>
                  <td>
                    <Link to={`/machine/${encodeURIComponent(a.hostname)}`} className="mono">
                      {a.hostname}
                    </Link>
                  </td>
                  <td className="mono alert-history-kind">
                    {formatKindLabel(a.kind)}
                  </td>
                  <td className="alert-history-message">
                    {a.message}
                    {a.details && (
                      <details className="alert-history-details">
                        <summary>Details</summary>
                        <pre>{a.details}</pre>
                      </details>
                    )}
                  </td>
                  <td className="mono alert-history-time">
                    {formatTime(a.fired_at)}
                    {a.resolved_at && (
                      <span className="alert-history-resolved">
                        → {formatTime(a.resolved_at)}
                      </span>
                    )}
                  </td>
                  <td className="mono">{formatDuration(a.duration_ms)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </PageLayout>
  );
}

function formatTime(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}
