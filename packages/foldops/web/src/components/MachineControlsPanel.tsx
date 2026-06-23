import { useCallback, useEffect, useState } from "react";
import {
  fetchMachineControlStatus,
  pushFoldinghomeConfig,
  runMachineControl,
} from "../api";
import {
  displayConfiguredCpus,
  displayEffectiveCpus,
  fahCpuPolicyDrift,
} from "../fahConfig";
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

const SERVICE_GROUPS: ControlGroup[] = [
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
  const [cpuMode, setCpuMode] = useState<"auto" | "manual">("auto");
  const [cpuSlots, setCpuSlots] = useState("4");
  const [cpuBusy, setCpuBusy] = useState(false);
  const [cpuMessage, setCpuMessage] = useState<string | null>(null);
  const [cpuError, setCpuError] = useState<string | null>(null);

  const online = machine?.online ?? false;
  const fah = machine?.latest?.payload?.fah;
  const cpuDrift = fahCpuPolicyDrift(fah);

  useEffect(() => {
    const configured = fah?.configCpus;
    if (configured == null) {
      return;
    }
    if (configured === 0) {
      setCpuMode("auto");
      return;
    }
    setCpuMode("manual");
    setCpuSlots(String(configured));
  }, [fah?.configCpus, hostname]);

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

  const applyCpuPolicy = async () => {
    const donor =
      fah?.configUsername?.trim() ||
      fah?.statsDonor?.trim() ||
      "Anonymous";
    const team = fah?.configTeam ?? 0;
    let cpus = 0;
    if (cpuMode === "manual") {
      const slots = Number(cpuSlots);
      if (!Number.isInteger(slots) || slots <= 0) {
        setCpuError("Enter a positive whole number of CPU slots, or choose Automatic.");
        setCpuMessage(null);
        return;
      }
      cpus = slots;
    }

    setCpuBusy(true);
    setCpuMessage(null);
    setCpuError(null);
    try {
      const result = await pushFoldinghomeConfig(hostname, {
        username: donor,
        team,
        cpus,
      });
      if (result.ok) {
        setCpuMessage(
          cpus === 0
            ? "CPU policy set to automatic."
            : `CPU policy set to ${cpus} slot${cpus === 1 ? "" : "s"}.`,
        );
      } else {
        setCpuError(result.error ?? "Failed to apply CPU policy.");
      }
    } catch (err) {
      setCpuError(err instanceof Error ? err.message : "Failed to apply CPU policy.");
    } finally {
      setCpuBusy(false);
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

      {SERVICE_GROUPS.map((group) => (
        <section key={group.title} className="machine-controls-group">
          <h3 className="machine-controls-group-title">{group.title}</h3>
          <p className="machine-controls-group-desc">{group.description}</p>
          <div className="machine-controls-buttons">
            {group.buttons.map((btn) => {
              const actionState = controlActionState(btn.action, status);
              const disabled =
                !online ||
                busy !== null ||
                cpuBusy ||
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

      <section className="machine-controls-group">
        <h3 className="machine-controls-group-title">Host</h3>
        <p className="machine-controls-group-desc">
          Folding@home CPU policy from <span className="mono">foldinghome.toml</span>{" "}
          and host power controls.
        </p>
        <dl className="machine-controls-host-cpu-status">
          <div>
            <dt>Configured CPUs</dt>
            <dd className="mono">{displayConfiguredCpus(fah)}</dd>
          </div>
          <div>
            <dt>Effective CPUs</dt>
            <dd className="mono">{displayEffectiveCpus(fah)}</dd>
          </div>
        </dl>
        {cpuDrift && (
          <p className="message error machine-controls-host-cpu-drift">
            Configured and effective CPU counts differ. Apply CPU policy below or
            wait for the next config activate to reconcile.
          </p>
        )}
        <div className="machine-controls-host-cpu-options">
          <label className="admin-radio-label">
            <input
              type="radio"
              name={`${hostname}-cpu-mode`}
              checked={cpuMode === "auto"}
              onChange={() => setCpuMode("auto")}
              disabled={!online || cpuBusy || busy !== null}
            />
            Automatic
          </label>
          <label className="admin-radio-label">
            <input
              type="radio"
              name={`${hostname}-cpu-mode`}
              checked={cpuMode === "manual"}
              onChange={() => setCpuMode("manual")}
              disabled={!online || cpuBusy || busy !== null}
            />
            Fixed slots
            <input
              className="admin-input mono machine-controls-host-cpu-input"
              type="number"
              min={1}
              step={1}
              value={cpuSlots}
              onChange={(event) => setCpuSlots(event.target.value)}
              disabled={!online || cpuBusy || busy !== null || cpuMode !== "manual"}
            />
          </label>
        </div>
        <div className="machine-controls-buttons">
          <button
            type="button"
            className="machine-controls-btn"
            disabled={!online || cpuBusy || busy !== null}
            onClick={applyCpuPolicy}
          >
            {cpuBusy ? "Applying…" : "Apply CPU policy"}
          </button>
          <button
            type="button"
            className="machine-controls-btn machine-controls-btn--danger"
            disabled={
              !online ||
              busy !== null ||
              cpuBusy ||
              statusLoading ||
              controlActionState("host.reboot", status).disabled
            }
            title={controlActionState("host.reboot", status).reason}
            onClick={() => run("host.reboot")}
          >
            {busy === "host.reboot" ? "Running…" : "Reboot server"}
          </button>
        </div>
        {cpuMessage && (
          <p className="message machine-controls-ok">{cpuMessage}</p>
        )}
        {cpuError && <p className="message error">{cpuError}</p>}
      </section>
    </div>
  );
}
