import { useCallback, useEffect, useState } from "react";
import {
  fetchMachineControlStatus,
  runMachineControl,
} from "../api";
import type { ControlAction, ControlStatus, MachineSummary } from "../types";

interface ControlGroup {
  title: string;
  description: string;
  buttons: { action: ControlAction; label: string; variant?: "danger" }[];
}

const GROUPS: ControlGroup[] = [
  {
    title: "FoldOps agent",
    description: "foldops-agent systemd service",
    buttons: [
      { action: "agent.start", label: "Start" },
      { action: "agent.stop", label: "Stop", variant: "danger" },
      { action: "agent.restart", label: "Restart" },
    ],
  },
  {
    title: "FAH client",
    description: "fah-client service and folding state (WebSocket on port 7396)",
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
  const [statusError, setStatusError] = useState<string | null>(null);
  const [busy, setBusy] = useState<ControlAction | null>(null);
  const [lastResult, setLastResult] = useState<string | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);

  const online = machine?.online ?? false;

  const loadStatus = useCallback(async () => {
    if (!online) {
      setStatus(null);
      setStatusError("Node offline — controls unavailable");
      return;
    }
    try {
      const s = await fetchMachineControlStatus(hostname);
      setStatus(s);
      setStatusError(null);
    } catch (err) {
      setStatus(null);
      setStatusError(
        err instanceof Error ? err.message : "Failed to load status",
      );
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
      } else {
        setLastError(result.message || "Command failed");
        if (result.stderr) setLastError(`${result.message}\n${result.stderr}`);
      }
      setTimeout(loadStatus, 2000);
    } catch (err) {
      setLastError(err instanceof Error ? err.message : "Control failed");
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="machine-controls">
      <p className="machine-controls-intro">
        Remote actions run on the node via the agent HTTP API (same as live logs).
        Requires <code>CONTROLS_ENABLED=true</code> on the agent.
      </p>

      {statusError && (
        <p className="message error">{statusError}</p>
      )}

      {status && (
        <div className="machine-controls-status">
          <span>
            foldops-agent:{" "}
            <strong className="mono">{status.foldops_agent}</strong>
          </span>
          <span>
            fah-client:{" "}
            <strong className="mono">{status.fah_client}</strong>
          </span>
          <button
            type="button"
            className="machine-controls-refresh"
            onClick={loadStatus}
          >
            Refresh status
          </button>
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
            {group.buttons.map((btn) => (
              <button
                key={btn.action}
                type="button"
                className={`machine-controls-btn${btn.variant === "danger" ? " machine-controls-btn--danger" : ""}`}
                disabled={!online || busy !== null}
                onClick={() => run(btn.action)}
              >
                {busy === btn.action ? "Running…" : btn.label}
              </button>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
