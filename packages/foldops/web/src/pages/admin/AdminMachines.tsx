import { useCallback, useEffect, useState } from "react";
import {
  addAllowBootDevice,
  fetchAllowBootDevices,
  removeAllowBootDevice,
} from "../../api";
import type { AllowBootDevice, NetworkInstallStatus } from "../../types";

const MAC_RE = /^([0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}$/;

function normalizeMac(value: string): string {
  const hex = value.replace(/[^0-9A-Fa-f]/g, "").toLowerCase();
  if (hex.length !== 12) {
    return value.trim();
  }
  return hex.match(/.{1,2}/g)!.join(":");
}

function networkStatusLabel(status: NetworkInstallStatus | undefined): string {
  switch (status) {
    case "awaiting_install":
      return "Awaiting install";
    case "online":
      return "Online";
    case "offline":
      return "Offline";
    case "installed":
      return "Installed";
    default:
      return "Unknown";
  }
}

function networkStatusClass(status: NetworkInstallStatus | undefined): string {
  switch (status) {
    case "online":
      return "status-active";
    case "offline":
      return "status-inactive";
    case "awaiting_install":
      return "status-unknown";
    case "installed":
      return "status-active";
    default:
      return "status-unknown";
  }
}

export function AdminMachines() {
  const [devices, setDevices] = useState<AllowBootDevice[]>([]);
  const [macAddress, setMacAddress] = useState("");
  const [installDisk, setInstallDisk] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [removingMac, setRemovingMac] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const response = await fetchAllowBootDevices();
      setDevices(response.devices ?? []);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load allowlist");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
    const id = setInterval(load, 15_000);
    return () => clearInterval(id);
  }, [load]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    const mac = normalizeMac(macAddress);
    if (!MAC_RE.test(mac)) {
      setError("Enter a valid MAC address (e.g. 00:11:22:33:44:55).");
      return;
    }

    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const result = await addAllowBootDevice(
        mac,
        installDisk.trim() || undefined,
      );
      setStatus(
        result.already_allowed
          ? `${result.mac_address} was already on the network boot allowlist.`
          : `Added ${result.mac_address} to the network boot allowlist.`,
      );
      setMacAddress("");
      setInstallDisk("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to add machine");
    } finally {
      setBusy(false);
    }
  };

  const removeDevice = async (mac: string) => {
    setRemovingMac(mac);
    setError(null);
    setStatus(null);
    try {
      const result = await removeAllowBootDevice(mac);
      setStatus(
        result.already_removed
          ? `${result.mac_address} was not on the allowlist.`
          : `Removed ${result.mac_address} from the network boot allowlist.`,
      );
      await load();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to remove machine",
      );
    } finally {
      setRemovingMac(null);
    }
  };

  return (
    <>
      <p className="admin-intro">
        Allow a machine to network-install FoldingOS over PXE/iPXE. Add its MAC
        address here, then boot the machine from the network. After install and
        agent registration, hostname, IP address, and online status appear
        automatically. The table refreshes every 15 seconds.
      </p>

      {error && <p className="message error">{error}</p>}
      {status && <p className="message admin-status">{status}</p>}

      <section className="admin-section">
        <h2 className="deploy-heading">Add machine</h2>
        <form onSubmit={submit}>
          <div className="admin-assign-form">
            <label>
              MAC address
              <input
                className="admin-input mono"
                type="text"
                value={macAddress}
                onChange={(event) => setMacAddress(event.target.value)}
                placeholder="00:11:22:33:44:55"
                autoComplete="off"
                spellCheck={false}
                disabled={busy}
              />
            </label>
            <label>
              Install disk (optional)
              <input
                className="admin-input mono"
                type="text"
                value={installDisk}
                onChange={(event) => setInstallDisk(event.target.value)}
                placeholder="/dev/sda"
                autoComplete="off"
                spellCheck={false}
                disabled={busy}
              />
            </label>
          </div>
          <div className="deploy-actions">
            <button
              type="submit"
              className="deploy-btn deploy-btn--primary"
              disabled={busy || !macAddress.trim()}
            >
              {busy ? "Adding…" : "Allow network install"}
            </button>
          </div>
        </form>
      </section>

      <section className="admin-section">
        <h2 className="deploy-heading">Allowed machines</h2>
        {loading ? (
          <p className="admin-muted">Loading allowlist…</p>
        ) : devices.length === 0 ? (
          <p className="admin-muted">No machines are on the allowlist yet.</p>
        ) : (
          <div className="deploy-results">
            <table className="deploy-table admin-table">
              <thead>
                <tr>
                  <th>MAC address</th>
                  <th>Hostname</th>
                  <th>IP address</th>
                  <th>Network status</th>
                  <th>Install disk</th>
                  <th className="admin-table-actions">Actions</th>
                </tr>
              </thead>
              <tbody>
                {devices.map((device) => (
                  <tr key={device.mac_address}>
                    <td className="mono">{device.mac_address}</td>
                    <td className="mono">{device.hostname ?? "—"}</td>
                    <td className="mono">{device.primary_ipv4 ?? "—"}</td>
                    <td>
                      <span
                        className={networkStatusClass(device.network_status)}
                      >
                        {networkStatusLabel(device.network_status)}
                      </span>
                    </td>
                    <td className="mono">{device.install_disk ?? "—"}</td>
                    <td className="admin-table-actions">
                      <button
                        type="button"
                        className="machine-controls-btn machine-controls-btn--danger"
                        disabled={busy || removingMac !== null}
                        onClick={() => removeDevice(device.mac_address)}
                      >
                        {removingMac === device.mac_address
                          ? "Removing…"
                          : "Remove"}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </>
  );
}
