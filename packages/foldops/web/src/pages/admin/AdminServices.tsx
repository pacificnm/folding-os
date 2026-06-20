import { useCallback, useEffect, useState } from "react";
import {
  fetchServices,
  restartAllServices,
  restartService,
} from "../../api";
import type { ManagedService, ServicesResponse } from "../../types";

function statusClass(status: string): string {
  switch (status) {
    case "active":
      return "status-active";
    case "failed":
      return "status-failed";
    case "inactive":
    case "dead":
      return "status-inactive";
    default:
      return "status-unknown";
  }
}

function formatStatus(status: string, loaded: boolean): string {
  if (!loaded) {
    return "not loaded";
  }
  return status;
}

export function AdminServices() {
  const [services, setServices] = useState<ManagedService[]>([]);
  const [installationRole, setInstallationRole] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [restartingUnit, setRestartingUnit] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response: ServicesResponse = await fetchServices();
      setServices(response.services ?? []);
      setInstallationRole(response.installation_role ?? null);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load services");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const restartOne = async (unit: string, name: string) => {
    setRestartingUnit(unit);
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const result = await restartService(unit);
      setStatus(result.message ?? `Restarted ${name}.`);
      if (result.scheduled) {
        window.setTimeout(() => {
          load();
        }, 5_000);
      } else {
        await load();
      }
    } catch (err) {
      setError(
        err instanceof Error ? err.message : `Failed to restart ${name}`,
      );
    } finally {
      setBusy(false);
      setRestartingUnit(null);
    }
  };

  const restartAll = async () => {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const result = await restartAllServices();
      setStatus(result.message ?? `Restarted ${result.count ?? 0} services.`);
      await load();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to restart all services",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <p className="admin-intro">
        Systemd services managed by FoldingOS and FoldOps on this machine.
        {installationRole ? (
          <>
            {" "}
            Installation role:{" "}
            <span className="mono">{installationRole}</span>.
          </>
        ) : null}
      </p>

      {error && <p className="message error">{error}</p>}
      {status && <p className="message admin-status">{status}</p>}

      <section className="admin-section">
        <div className="deploy-actions">
          <button
            type="button"
            className="deploy-btn deploy-btn--primary"
            disabled={busy || loading || services.length === 0}
            onClick={restartAll}
          >
            {busy && !restartingUnit ? "Restarting all…" : "Restart all services"}
          </button>
        </div>
      </section>

      <section className="admin-section">
        <h2 className="deploy-heading">Services</h2>
        {loading ? (
          <p className="admin-muted">Loading services…</p>
        ) : services.length === 0 ? (
          <p className="admin-muted">No managed services found.</p>
        ) : (
          <div className="deploy-results">
            <table className="deploy-table admin-table">
              <thead>
                <tr>
                  <th>Service name</th>
                  <th>Status</th>
                  <th className="admin-table-actions">Actions</th>
                </tr>
              </thead>
              <tbody>
                {services.map((service) => {
                  const displayStatus = formatStatus(
                    service.status,
                    service.loaded,
                  );
                  return (
                    <tr key={service.unit}>
                      <td>
                        <div>{service.name}</div>
                        <div className="admin-muted mono">{service.unit}</div>
                      </td>
                      <td>
                        <span className={statusClass(service.status)}>
                          {displayStatus}
                        </span>
                      </td>
                      <td className="admin-table-actions">
                        <button
                          type="button"
                          className="machine-controls-btn"
                          disabled={
                            busy ||
                            !service.restartable ||
                            restartingUnit !== null
                          }
                          onClick={() =>
                            restartOne(service.unit, service.name)
                          }
                        >
                          {restartingUnit === service.unit
                            ? "Restarting…"
                            : "Restart"}
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </>
  );
}
