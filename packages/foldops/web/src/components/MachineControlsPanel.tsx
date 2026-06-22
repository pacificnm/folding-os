import { useCallback, useEffect, useState } from "react";
import {
  fetchMachineControlStatus,
  runMachineControl,
} from "../api";
import {
  controlActionState,
  optimisticControlStatus,
} from "../machineControlUi";
import type { ControlAction, ControlStatus, MachineSummary } from "../types";
import { formatPpd } from "../utils/format";

function foldingStateLabel(state: string | undefined): string {
  switch (state) {
    case "folding":
      return "Folding";
    case "paused":
      return "Paused";
    case "waiting":
      return "Waiting for work";
    case "finishing":
      return "Finishing WU";
    case "stopped":
      return "Stopped";
    case "unreachable":
      return "Unreachable";
    case "download":
      return "Downloading WU";
    case "upload":
      return "Uploading WU";
    case "ready":
      return "Ready";
    case "core":
      return "Starting core";
    case "idle":
      return "Idle";
    default:
      return state?.length ? state : "Unknown";
  }
}

function ingestFoldingHint(machine: MachineSummary | null): string | null {
  const latest = machine?.latest;
  if (!latest) return null;
  if (latest.project) {
    const parts = [`project ${latest.project}`];
    if (latest.progress != null) parts.push(`${latest.progress.toFixed(1)}%`);
    if (latest.ppd != null) parts.push(formatPpd(latest.ppd));
    return `Last ingest: ${parts.join(" · ")}`;
  }
  if (latest.fah_status !== "active") {
    return `Last ingest: FAH service ${latest.fah_status}`;
  }
  return "Last ingest: FAH running, no work unit metrics yet";
}

interface ControlGroup {
  title: string;
  description: string;
  buttons: { action: ControlAction; label: string; variant?: "danger" }[];
}

const GROUPS: ControlGroup[] = [
  {
    title: "FoldOps agent",
    description: "foldingos-foldops-agent.service",
    buttons: [
      { action: "agent.start", label: "Start" },
      { action: "agent.stop", label: "Stop", variant: "danger" },
      { action: "agent.restart", label: "Restart" },
    ],
  },
  {
    title: "FAH client",
    description: "folding-at-home.service and folding state (WebSocket on port 7396)",
    buttons: [
      { action: "fah.start", label: "Start" },
      { action: "fah.stop", label: "Stop", variant: "danger" },
      { action: "fah.restart", label: "Restart" },
      { action: "fah.pause", label: "Pause folding" },
      { action: "fah.resume", label: "Resume folding" },
      { action: "fah.finish", label: "Finish WU" },
    ],
  },
  {
    title: "Host",
    description: "Reboots the entire machine",
    buttons: [{ action: "host.reboot", label: "Reboot server", variant: "danger" }],
  },
];

interface MachineControlsPanelProps {
  hostname: string;
  machine: MachineSummary | null;
}

export function MachineControlsPanel({
  hostname,
  machine,
}: MachineControlsPanelProps) {
  const [status, setStatus] = useState<ControlStatus | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [busy, setBusy] = useState<ControlAction | null>(null);
  const [lastResult, setLastResult] = useState<string | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);

  const online = machine?.online ?? false;

  const loadStatus = useCallback(async () => {
    if (!online) {
      setStatus(null);
      setStatusError("Node offline — controls unavailable");
      setStatusLoading(false);
      return;
    }

    setStatusLoading(true);
    try {
      const next = await fetchMachineControlStatus(hostname);
      setStatus(next);
      setStatusError(null);
    } catch (err) {
      setStatus(null);
      setStatusError(
        err instanceof Error ? err.message : "Failed to load status",
      );
    } finally {
      setStatusLoading(false);
    }
  }, [hostname, online]);

  useEffect(() => {
    loadStatus();
    if (!online) return;
    const id = setInterval(loadStatus, 15_000);
    return () => clearInterval(id);
  }, [loadStatus, online]);

  const run = async (action: ControlAction) => {
    if (action === "host.reboot") {
      const ok = window.confirm(
        `Reboot ${hostname}? This will stop folding and disconnect the node.`,
      );
      if (!ok) return;
    }

    setBusy(action);
    setLastResult(null);
    setLastError(null);

    try {
      const result = await runMachineControl(hostname, action);
      if (result.ok) {
        setLastResult(result.message);
        if (status) {
          setStatus(optimisticControlStatus(status, action));
        }
      } else {
        const message = result.stderr
          ? `${result.message}\n${result.stderr}`
          : result.message || "Command failed";
        setLastError(message);
      }
      window.setTimeout(loadStatus, 2_000);
    } catch (err) {
      setLastError(err instanceof Error ? err.message : "Control failed");
    } finally {
      setBusy(null);
    }
  };

  const ingestHint = ingestFoldingHint(machine);

  return (
    <div className="machine-controls">
      <p className="machine-controls-intro">
        Remote actions run on the node via the agent HTTP API (same as live logs).
        Requires <code>CONTROL_ENABLED=true</code> on the supervisor and{" "}
        <code>CONTROLS_ENABLED=true</code> on the agent.
      </p>

      {statusError && (
        <p className="message error">{statusError}</p>
      )}

      {online && statusLoading && !status && (
        <p className="admin-muted">Loading control status…</p>
      )}

      {status && (
        <div className="machine-controls-status-wrap">
          <div className="machine-controls-status">
            <span>
              foldops-agent:{" "}
              <strong className="mono">{status.foldops_agent}</strong>
            </span>
            <span>
              folding-at-home:{" "}
              <strong className="mono">{status.fah_client}</strong>
            </span>
            <button
              type="button"
              className="machine-controls-refresh"
              disabled={statusLoading || busy !== null}
              onClick={loadStatus}
            >
              {statusLoading ? "Refreshing…" : "Refresh status"}
            </button>
          </div>
          <div className="machine-controls-folding">
            <span className="machine-controls-folding-label">Folding activity</span>
            <span
              className={`badge machine-controls-folding-badge machine-controls-folding-badge--${status.fah_folding_state ?? "unknown"}`}
            >
              {foldingStateLabel(status.fah_folding_state)}
            </span>
            {status.fah_unit_state && (
              <span className="mono machine-controls-folding-unit">
                {status.fah_unit_state}
              </span>
            )}
            {status.fah_folding_detail && (
              <span className="machine-controls-folding-detail">
                {status.fah_folding_detail}
              </span>
            )}
          </div>
          {ingestHint && (
            <p className="machine-controls-ingest-hint">{ingestHint}</p>
          )}
        </div>
      )}

      {lastResult && (
        <p className="message machine-controls-ok">{lastResult}</p>
      )}
      {lastError && (
        <p className="message error">{lastError}</p>
      )}

      {GROUPS.map((group) => (
        <section key={group.title} className="machine-controls-group">
          <h3 className="machine-controls-group-title">{group.title}</h3>
          <p className="machine-controls-group-desc">{group.description}</p>
          <div className="machine-controls-buttons">
            {group.buttons.map((btn) => {
              const actionState = controlActionState(btn.action, status);
              const disabled =
                !online ||
                busy !== null ||
                statusLoading ||
                actionState.disabled;
              return (
                <button
                  key={btn.action}
                  type="button"
                  className={`machine-controls-btn${btn.variant === "danger" ? " machine-controls-btn--danger" : ""}`}
                  disabled={disabled}
                  title={actionState.reason}
                  onClick={() => run(btn.action)}
                >
                  {busy === btn.action ? "Running…" : btn.label}
                </button>
              );
            })}
          </div>
        </section>
      ))}
    </div>
  );
}
