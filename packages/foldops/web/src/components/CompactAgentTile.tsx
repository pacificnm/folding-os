import { Link } from "react-router-dom";
import { FahStatsLinks } from "./FahStatsLinks";
import { getMachineFahTelemetry } from "../fahTelemetry";
import type { MachineSummary } from "../types";
import { cpuTempLevel, formatPpd, formatTemp, formatUptime } from "../utils/format";

function statusLabel(machine: MachineSummary): string {
  if (!machine.online) return "offline";
  const fah = machine.latest?.fah_status ?? "unknown";
  if (fah === "active") return "folding";
  if (fah === "inactive") return "idle";
  if (fah === "failed") return "failed";
  return fah;
}

export function CompactAgentTile({ machine }: { machine: MachineSummary }) {
  const latest = machine.latest;
  const telemetry = getMachineFahTelemetry(latest);
  const load = latest?.payload?.system.loadAvg;
  const uptime = latest?.payload?.system.uptime;
  const status = statusLabel(machine);
  const statsDonor = latest?.payload?.fah?.statsDonor;
  const statsTeam = latest?.payload?.fah?.statsTeam;

  return (
    <Link
      to={`/machine/${encodeURIComponent(machine.hostname)}`}
      className={`kiosk-tile kiosk-tile-link ${machine.online ? "kiosk-tile--online" : "kiosk-tile--offline"}`}
    >
      <header className="kiosk-tile-head">
        <h2 className="kiosk-tile-name">{machine.hostname}</h2>
        <div className="kiosk-tile-head-right">
          <FahStatsLinks
            donor={statsDonor}
            team={statsTeam}
            compact
            stopPropagation
          />
          <span
            className={`kiosk-tile-status ${machine.online ? "kiosk-tile-status--ok" : "kiosk-tile-status--warn"}`}
          >
            {status}
          </span>
        </div>
      </header>

      <dl className="kiosk-stats">
        <div className="kiosk-stats-wide">
          <dt>Project</dt>
          <dd className="mono">{telemetry.projectLabel}</dd>
        </div>
        <div>
          <dt>PPD</dt>
          <dd className="mono highlight">{formatPpd(telemetry.ppd)}</dd>
        </div>
        <div>
          <dt>TPF</dt>
          <dd className="mono">{telemetry.tpf ?? "—"}</dd>
        </div>
        <div>
          <dt>CPU temp</dt>
          <dd
            className={`mono kiosk-cpu-temp kiosk-cpu-temp--${cpuTempLevel(telemetry.cpuTemp)}`}
          >
            {formatTemp(telemetry.cpuTemp)}
          </dd>
        </div>
        <div>
          <dt>Chassis</dt>
          <dd className="mono">{formatTemp(telemetry.chassisTemp)}</dd>
        </div>
        <div>
          <dt>Load</dt>
          <dd className="mono">
            {load ? load.map((n) => n.toFixed(1)).join(" / ") : "—"}
          </dd>
        </div>
        <div className="kiosk-stats-wide">
          <dt>Uptime</dt>
          <dd>{uptime != null ? formatUptime(uptime) : "—"}</dd>
        </div>
        <div>
          <dt>Progress</dt>
          <dd>
            {telemetry.progress != null
              ? `${telemetry.progress.toFixed(1)}%`
              : "—"}
          </dd>
        </div>
      </dl>

      {telemetry.progress != null && (
        <div className="progress-bar kiosk-tile-progress-bar">
          <div
            className="progress-fill"
            style={{ width: `${Math.min(telemetry.progress, 100)}%` }}
          />
        </div>
      )}
    </Link>
  );
}
