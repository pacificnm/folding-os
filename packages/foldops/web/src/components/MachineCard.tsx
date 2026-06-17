import { Link } from "react-router-dom";
import { FahStatsLinks } from "./FahStatsLinks";
import type { MachineSummary } from "../types";
import {
  formatLastSeen,
  formatPpd,
  formatTemp,
  formatUptime,
} from "../utils/format";

export function MachineCard({ machine }: { machine: MachineSummary }) {
  const latest = machine.latest;
  const load = latest?.payload?.system.loadAvg;
  const uptime = latest?.payload?.system.uptime;
  const tpf = latest?.payload?.fah?.tpf;
  const errors = latest?.payload?.fah?.recentErrors ?? [];
  const statsDonor = latest?.payload?.fah?.statsDonor;
  const statsTeam = latest?.payload?.fah?.statsTeam;

  return (
    <article className={`card ${machine.online ? "online" : "offline"}`}>
      <header className="card-header">
        <h2>
          <Link
            to={`/machine/${encodeURIComponent(machine.hostname)}`}
            className="card-link"
          >
            {machine.hostname}
          </Link>
        </h2>
        <span className={`badge ${machine.online ? "badge-ok" : "badge-warn"}`}>
          {machine.online ? "online" : "offline"}
        </span>
      </header>

      <div className="card-section">
        <h3>FAH</h3>
        <dl className="stats">
          <div>
            <dt>Service</dt>
            <dd className={`status-${latest?.fah_status ?? "unknown"}`}>
              {latest?.fah_status ?? "—"}
            </dd>
          </div>
          <div>
            <dt>Project</dt>
            <dd>
              {latest?.project
                ? `${latest.project} (R${latest.run ?? "?"}/C${latest.clone ?? "?"}/G${latest.gen ?? "?"})`
                : "—"}
            </dd>
          </div>
          <div>
            <dt>Progress</dt>
            <dd>
              {latest?.progress != null ? `${latest.progress.toFixed(1)}%` : "—"}
            </dd>
          </div>
          <div>
            <dt>PPD</dt>
            <dd className="mono highlight">{formatPpd(latest?.ppd ?? null)}</dd>
          </div>
          <div>
            <dt>TPF</dt>
            <dd className="mono">{tpf ?? "—"}</dd>
          </div>
        </dl>
        {latest?.progress != null && (
          <div className="progress-bar">
            <div
              className="progress-fill"
              style={{ width: `${Math.min(latest.progress, 100)}%` }}
            />
          </div>
        )}
        <FahStatsLinks donor={statsDonor} team={statsTeam} className="card-fah-stats" />
      </div>

      <div className="card-section">
        <h3>System</h3>
        <dl className="stats">
          <div>
            <dt>CPU</dt>
            <dd>{latest?.cpu_usage != null ? `${latest.cpu_usage}%` : "—"}</dd>
          </div>
          <div>
            <dt>Load</dt>
            <dd className="mono">
              {load ? load.map((n) => n.toFixed(2)).join(" / ") : "—"}
            </dd>
          </div>
          <div>
            <dt>Memory</dt>
            <dd>
              {latest?.memory_percent != null
                ? `${latest.memory_percent}%`
                : "—"}
            </dd>
          </div>
          <div>
            <dt>Disk</dt>
            <dd>
              {latest?.disk_percent != null ? `${latest.disk_percent}%` : "—"}
            </dd>
          </div>
          <div>
            <dt>CPU temp</dt>
            <dd className="mono">
              {formatTemp(latest?.cpu_temp ?? latest?.payload?.system.cpuTemp)}
            </dd>
          </div>
          <div>
            <dt>Chassis temp</dt>
            <dd className="mono">
              {formatTemp(
                latest?.chassis_temp ?? latest?.payload?.system.chassisTemp,
              )}
            </dd>
          </div>
          <div>
            <dt>Uptime</dt>
            <dd>{uptime != null ? formatUptime(uptime) : "—"}</dd>
          </div>
        </dl>
      </div>

      <div className="card-section card-footer">
        <dl className="stats stats-inline">
          <div>
            <dt>Updates</dt>
            <dd
              className={
                (latest?.apt_updates ?? 0) > 0 ? "warn-text" : undefined
              }
            >
              {latest?.apt_updates ?? 0}
            </dd>
          </div>
          <div>
            <dt>Reboot</dt>
            <dd className={latest?.reboot_required ? "warn-text" : undefined}>
              {latest?.reboot_required ? "required" : "no"}
            </dd>
          </div>
          <div>
            <dt>Last seen</dt>
            <dd>{formatLastSeen(machine.last_seen)}</dd>
          </div>
        </dl>
        {errors.length > 0 && (
          <details className="errors">
            <summary>{errors.length} recent log errors</summary>
            <ul>
              {errors.map((e, i) => (
                <li key={i}>{e}</li>
              ))}
            </ul>
          </details>
        )}
        <Link
          to={`/machine/${encodeURIComponent(machine.hostname)}`}
          className="card-details-btn"
        >
          View history →
        </Link>
      </div>
    </article>
  );
}
