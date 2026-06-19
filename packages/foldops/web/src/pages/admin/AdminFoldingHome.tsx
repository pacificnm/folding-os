import { useCallback, useEffect, useState } from "react";
import { fetchMachines, pushFoldinghomeConfig } from "../../api";
import {
  fahAcquisitionLabel,
  fahAcquisitionTitle,
  fahClientClass,
  fahClientLabel,
} from "../../components/FahClientStatus";
import { formatPasskeyError, normalizePasskeyInput } from "../../fahPasskey";
import {
  displayConfiguredDonor,
  displayConfiguredTeam,
  displayConfiguredToken,
  isFahConfigured,
} from "../../fahConfig";
import type { FoldinghomeConfigResponse, MachineSummary } from "../../types";

const FOLDINGHOME_DEFAULTS_KEY = "foldops.foldinghome.defaults.v1";

interface FoldinghomeSavedDefaults {
  username?: string;
  team?: string;
}

function loadSavedDefaults(): FoldinghomeSavedDefaults {
  try {
    const raw = window.localStorage.getItem(FOLDINGHOME_DEFAULTS_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as FoldinghomeSavedDefaults;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

function saveDefaults(defaults: FoldinghomeSavedDefaults) {
  try {
    window.localStorage.setItem(
      FOLDINGHOME_DEFAULTS_KEY,
      JSON.stringify(defaults),
    );
  } catch {
    /* Ignore storage failures; config push still proceeds. */
  }
}

function mergeAppliedFahConfig(
  machines: MachineSummary[],
  hostnames: Set<string>,
  username: string,
  team: number,
  passkeyConfigured: boolean,
): MachineSummary[] {
  return machines.map((machine) => {
    if (!hostnames.has(machine.hostname) || !machine.latest?.payload) {
      return machine;
    }
    const previousFah = machine.latest.payload.fah;
    return {
      ...machine,
      latest: {
        ...machine.latest,
        payload: {
          ...machine.latest.payload,
          fah: {
            ...previousFah,
            configUsername: username,
            configTeam: team,
            configPasskeyConfigured: passkeyConfigured
              ? true
              : previousFah.configPasskeyConfigured,
          },
        },
      },
    };
  });
}

export function AdminFoldingHome() {
  const [savedDefaults] = useState(loadSavedDefaults);
  const [machines, setMachines] = useState<MachineSummary[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [username, setUsername] = useState(savedDefaults.username ?? "");
  const [team, setTeam] = useState(savedDefaults.team ?? "0");
  const [passkey, setPasskey] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [results, setResults] = useState<FoldinghomeConfigResponse[]>([]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetchMachines();
      setMachines(response.machines);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load machines");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const sorted = machines
    .slice()
    .sort((a, b) => a.hostname.localeCompare(b.hostname));

  const toggleHost = (hostname: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(hostname)) next.delete(hostname);
      else next.add(hostname);
      return next;
    });
  };

  const selectOnline = () => {
    setSelected(
      new Set(sorted.filter((machine) => machine.online).map((m) => m.hostname)),
    );
  };

  const apply = async (event: React.FormEvent) => {
    event.preventDefault();
    const donor = username.trim();
    if (!donor) {
      setError("Enter a donor name.");
      return;
    }
    if (donor.length > 128) {
      setError("Donor name must be 128 characters or fewer.");
      return;
    }

    const teamNumber = Number(team);
    if (!Number.isInteger(teamNumber) || teamNumber < 0 || teamNumber > 2_147_483_647) {
      setError("Team must be a whole number from 0 through 2147483647.");
      return;
    }

    let passkeyValue = "";
    try {
      passkeyValue = normalizePasskeyInput(passkey);
    } catch (err) {
      setError(err instanceof Error ? err.message : formatPasskeyError(0));
      return;
    }

    const targets =
      selected.size > 0
        ? sorted.filter((machine) => selected.has(machine.hostname))
        : sorted.filter((machine) => machine.online);
    if (targets.length === 0) {
      setError("Select at least one online machine.");
      return;
    }

    saveDefaults({
      username: donor,
      team: String(teamNumber),
    });

    setBusy(true);
    setError(null);
    setStatus(null);
    setResults([]);

    const nextResults: FoldinghomeConfigResponse[] = [];
    for (const machine of targets) {
      if (!machine.online) {
        nextResults.push({
          hostname: machine.hostname,
          ok: false,
          error: "Node offline",
        });
        continue;
      }

      setStatus(`Applying to ${machine.hostname}…`);
      const result = await pushFoldinghomeConfig(machine.hostname, {
        username: donor,
        team: teamNumber,
        ...(passkeyValue ? { passkey: passkeyValue } : {}),
      });
      nextResults.push(result);
      setResults([...nextResults]);
    }

    const failures = nextResults.filter((result) => !result.ok);
    const successfulHosts = new Set(
      nextResults
        .filter((result) => result.ok)
        .map((result) => result.hostname),
    );
    setMachines((current) =>
      mergeAppliedFahConfig(
        current,
        successfulHosts,
        donor,
        teamNumber,
        Boolean(passkeyValue),
      ),
    );

    if (failures.length === 0) {
      setStatus(
        `Applied Folding@home settings to ${nextResults.length} machine${nextResults.length === 1 ? "" : "s"}.`,
      );
    } else if (failures.length === nextResults.length) {
      setError(failures[0]?.error ?? "Failed to apply Folding@home settings.");
      setStatus(null);
    } else {
      setStatus(
        `Applied to ${nextResults.length - failures.length} of ${nextResults.length} machines. See results below.`,
      );
    }

    setBusy(false);
    try {
      const response = await fetchMachines();
      setMachines((current) =>
        mergeAppliedFahConfig(
          response.machines.length > 0 ? response.machines : current,
          successfulHosts,
          donor,
          teamNumber,
          Boolean(passkeyValue),
        ),
      );
      setError(null);
    } catch {
      /* Keep optimistic apply results visible if the refresh misses. */
    }
  };

  return (
    <>
      <p className="admin-intro">
        Set the Folding@home donor name, team number, and account token on farm
        nodes. Each machine's FAH v8{" "}
        <span className="mono">machine-name</span> is set automatically from its
        hostname (for example <span className="mono">folding-e1eb1a</span>).
      </p>

      {error && <p className="message error">{error}</p>}
      {status && <p className="message admin-status">{status}</p>}

      <section className="admin-section">
        <h2 className="deploy-heading">Folding@home identity</h2>
        <form onSubmit={apply}>
          <div className="admin-assign-form">
            <label>
              Donor name
              <input
                className="admin-input"
                type="text"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                placeholder="My Farm"
                autoComplete="off"
                spellCheck={false}
                disabled={busy}
                maxLength={128}
              />
            </label>
            <label>
              Team number
              <input
                className="admin-input mono"
                type="number"
                min={0}
                max={2147483647}
                value={team}
                onChange={(event) => setTeam(event.target.value)}
                disabled={busy}
              />
            </label>
            <label>
              Passkey / account token (optional)
              <input
                className="admin-input mono"
                type="text"
                value={passkey}
                onChange={(event) => setPasskey(event.target.value)}
                onBlur={(event) => {
                  try {
                    const normalized = normalizePasskeyInput(event.target.value);
                    if (normalized !== event.target.value.trim()) {
                      setPasskey(normalized);
                    }
                  } catch {
                    /* keep raw input until submit */
                  }
                }}
                placeholder="Paste passkey from config.xml or FAH email"
                autoComplete="off"
                spellCheck={false}
                disabled={busy}
              />
            </label>
          </div>
          <p className="admin-muted">
            Leave account token blank to keep the existing token unchanged. Paste
            the token from FAH v8 <span className="mono">config.xml</span> (
            <span className="mono">account-token</span>) or your Folding@home
            account email (~43 characters).
          </p>
          <div className="deploy-actions">
            <button
              type="button"
              className="deploy-btn"
              disabled={busy || loading}
              onClick={selectOnline}
            >
              Select all online
            </button>
            <button
              type="submit"
              className="deploy-btn deploy-btn--primary"
              disabled={busy || loading || !username.trim()}
            >
              {busy
                ? "Applying…"
                : selected.size > 0
                  ? `Apply to selected (${selected.size})`
                  : "Apply to all online"}
            </button>
          </div>
        </form>
      </section>

      <section className="admin-section">
        <h2 className="deploy-heading">Farm nodes</h2>
        {loading ? (
          <p className="admin-muted">Loading machines…</p>
        ) : sorted.length === 0 ? (
          <p className="admin-muted">No enrolled machines yet.</p>
        ) : (
          <div className="deploy-results">
            <table className="deploy-table admin-table">
              <thead>
                <tr>
                  <th aria-label="Select" />
                  <th>Host</th>
                  <th>Status</th>
                  <th>Client</th>
                  <th>Acquire</th>
                  <th>FAH settings</th>
                  <th>Donor</th>
                  <th>Team</th>
                  <th>Token</th>
                </tr>
              </thead>
              <tbody>
                {sorted.map((machine) => {
                  const fah = machine.latest?.payload?.fah;
                  return (
                    <tr key={machine.hostname}>
                      <td>
                        <input
                          type="checkbox"
                          checked={selected.has(machine.hostname)}
                          onChange={() => toggleHost(machine.hostname)}
                          disabled={busy || !machine.online}
                          aria-label={`Select ${machine.hostname}`}
                        />
                      </td>
                      <td className="mono">{machine.hostname}</td>
                      <td>
                        <span
                          className={`badge ${machine.online ? "badge-ok" : "badge-warn"}`}
                        >
                          {machine.online ? "online" : "offline"}
                        </span>
                      </td>
                      <td
                        className={`mono ${fahClientClass(fah)}`}
                        title={fahAcquisitionTitle(fah)}
                      >
                        {fahClientLabel(fah)}
                      </td>
                      <td
                        className={`mono ${fahClientClass(fah)}`}
                        title={fahAcquisitionTitle(fah)}
                      >
                        {fahAcquisitionLabel(fah)}
                      </td>
                      <td>
                        <span
                          className={`badge ${isFahConfigured(fah) ? "badge-ok" : "badge-warn"}`}
                        >
                          {isFahConfigured(fah) ? "configured" : "default"}
                        </span>
                      </td>
                      <td>{displayConfiguredDonor(fah)}</td>
                      <td className="mono">{displayConfiguredTeam(fah)}</td>
                      <td className="mono">{displayConfiguredToken(fah)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {results.length > 0 && (
        <section className="admin-section">
          <h2 className="deploy-heading">Apply results</h2>
          <div className="deploy-results">
            <table className="deploy-table admin-table">
              <thead>
                <tr>
                  <th>Host</th>
                  <th>Status</th>
                  <th>Message</th>
                </tr>
              </thead>
              <tbody>
                {results.map((result) => (
                  <tr key={result.hostname}>
                    <td className="mono">{result.hostname}</td>
                    <td>
                      <span
                        className={`deploy-host-status deploy-host-status--${result.ok ? "success" : "failed"}`}
                      >
                        {result.ok ? "success" : "failed"}
                      </span>
                    </td>
                    <td>
                      {result.ok
                        ? result.activated
                          ? result.ingested
                            ? "Activated and refreshed"
                            : result.ingest_error
                              ? `Activated; refresh failed: ${result.ingest_error}`
                              : "Activated"
                          : "Applied"
                        : (result.error ?? "Failed")}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </>
  );
}
