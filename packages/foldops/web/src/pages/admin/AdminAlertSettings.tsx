import { useCallback, useEffect, useState } from "react";
import {
  fetchAlertSettings,
  saveAlertSettings,
  sendAlertTest,
} from "../../api";
import type {
  AlertSettingsResponse,
  AlertSettingsUpdateRequest,
} from "../../types";

const DEFAULT_THRESHOLDS = {
  offline_threshold_ms: 120_000,
  cpu_temp_alert_c: 85,
  stuck_progress_hours: 4,
  dashboard_url: "",
};

export function AdminAlertSettings() {
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const [discordEnabled, setDiscordEnabled] = useState(false);
  const [discordUsername, setDiscordUsername] = useState("FoldOps");
  const [webhookConfigured, setWebhookConfigured] = useState(false);
  const [webhookUrl, setWebhookUrl] = useState("");
  const [clearWebhookUrl, setClearWebhookUrl] = useState(false);

  const [emailEnabled, setEmailEnabled] = useState(false);
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState(587);
  const [smtpUsername, setSmtpUsername] = useState("");
  const [smtpPassword, setSmtpPassword] = useState("");
  const [smtpPasswordConfigured, setSmtpPasswordConfigured] = useState(false);
  const [clearSmtpPassword, setClearSmtpPassword] = useState(false);
  const [fromAddress, setFromAddress] = useState("");
  const [toAddresses, setToAddresses] = useState("");
  const [useTls, setUseTls] = useState(true);

  const [offlineThresholdMs, setOfflineThresholdMs] = useState(
    DEFAULT_THRESHOLDS.offline_threshold_ms,
  );
  const [cpuTempAlertC, setCpuTempAlertC] = useState(
    DEFAULT_THRESHOLDS.cpu_temp_alert_c,
  );
  const [stuckProgressHours, setStuckProgressHours] = useState(
    DEFAULT_THRESHOLDS.stuck_progress_hours,
  );
  const [dashboardUrl, setDashboardUrl] = useState("");

  const applySettings = useCallback((settings: AlertSettingsResponse) => {
    setDiscordEnabled(settings.discord.enabled);
    setDiscordUsername(settings.discord.username);
    setWebhookConfigured(settings.discord.webhook_url_configured);
    setWebhookUrl("");
    setClearWebhookUrl(false);

    setEmailEnabled(settings.email.enabled);
    setSmtpHost(settings.email.smtp_host ?? "");
    setSmtpPort(settings.email.smtp_port);
    setSmtpUsername(settings.email.smtp_username ?? "");
    setSmtpPassword("");
    setSmtpPasswordConfigured(settings.email.smtp_password_configured);
    setClearSmtpPassword(false);
    setFromAddress(settings.email.from_address ?? "");
    setToAddresses(settings.email.to_addresses.join(", "));
    setUseTls(settings.email.use_tls);

    setOfflineThresholdMs(settings.thresholds.offline_threshold_ms);
    setCpuTempAlertC(settings.thresholds.cpu_temp_alert_c);
    setStuckProgressHours(settings.thresholds.stuck_progress_hours);
    setDashboardUrl(settings.thresholds.dashboard_url ?? "");
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const settings = await fetchAlertSettings();
      applySettings(settings);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load alert settings");
    } finally {
      setLoading(false);
    }
  }, [applySettings]);

  useEffect(() => {
    load();
  }, [load]);

  const buildPayload = (): AlertSettingsUpdateRequest => ({
    discord: {
      enabled: discordEnabled,
      webhook_url: webhookUrl.trim() ? webhookUrl.trim() : null,
      username: discordUsername.trim() || "FoldOps",
      clear_webhook_url: clearWebhookUrl,
    },
    email: {
      enabled: emailEnabled,
      smtp_host: smtpHost.trim() ? smtpHost.trim() : null,
      smtp_port: smtpPort,
      smtp_username: smtpUsername.trim() ? smtpUsername.trim() : null,
      smtp_password: smtpPassword.trim() ? smtpPassword.trim() : null,
      from_address: fromAddress.trim() ? fromAddress.trim() : null,
      to_addresses: toAddresses
        .split(/[,\n]/)
        .map((entry) => entry.trim())
        .filter(Boolean),
      use_tls: useTls,
      clear_smtp_password: clearSmtpPassword,
    },
    thresholds: {
      offline_threshold_ms: offlineThresholdMs,
      cpu_temp_alert_c: cpuTempAlertC,
      stuck_progress_hours: stuckProgressHours,
      dashboard_url: dashboardUrl.trim() ? dashboardUrl.trim() : null,
    },
  });

  const save = async () => {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const saved = await saveAlertSettings(buildPayload());
      applySettings(saved);
      setStatus("Alert settings saved.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save alert settings");
    } finally {
      setBusy(false);
    }
  };

  const testDiscord = async () => {
    setTesting(true);
    setError(null);
    setStatus(null);
    try {
      if (busy) {
        throw new Error("Wait for the current save to finish before testing.");
      }
      const result = await sendAlertTest();
      setStatus(result.message);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Discord test failed");
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <p className="admin-intro">
        Configure Discord and email alert delivery settings. Saved values persist
        in the supervisor settings model and survive refresh and reboot. Email
        delivery is stored here for a future notification release.
      </p>

      {error && <p className="message error">{error}</p>}
      {status && <p className="message admin-status">{status}</p>}
      {loading && <p className="message">Loading alert settings…</p>}

      {!loading && (
        <>
          <section className="admin-section">
            <h2 className="admin-section-title">Discord alerts</h2>
            <label className="admin-checkbox">
              <input
                type="checkbox"
                checked={discordEnabled}
                onChange={(event) => setDiscordEnabled(event.target.checked)}
                disabled={busy}
              />
              Enable Discord notifications
            </label>

            <div className="admin-assign-form">
              <label>
                Webhook URL
                <input
                  className="admin-input"
                  type="url"
                  value={webhookUrl}
                  onChange={(event) => setWebhookUrl(event.target.value)}
                  placeholder={
                    webhookConfigured
                      ? "Leave blank to keep the saved webhook URL"
                      : "https://discord.com/api/webhooks/…"
                  }
                  disabled={busy || clearWebhookUrl}
                />
              </label>
              <label>
                Bot username
                <input
                  className="admin-input"
                  type="text"
                  value={discordUsername}
                  onChange={(event) => setDiscordUsername(event.target.value)}
                  disabled={busy}
                />
              </label>
            </div>

            {webhookConfigured && (
              <label className="admin-checkbox">
                <input
                  type="checkbox"
                  checked={clearWebhookUrl}
                  onChange={(event) => setClearWebhookUrl(event.target.checked)}
                  disabled={busy}
                />
                Remove saved webhook URL
              </label>
            )}

            <div className="deploy-actions">
              <button
                type="button"
                className="deploy-btn"
                disabled={testing || busy || !discordEnabled}
                onClick={testDiscord}
              >
                {testing ? "Sending…" : "Send Discord test"}
              </button>
            </div>
          </section>

          <section className="admin-section">
            <h2 className="admin-section-title">Email alerts</h2>
            <label className="admin-checkbox">
              <input
                type="checkbox"
                checked={emailEnabled}
                onChange={(event) => setEmailEnabled(event.target.checked)}
                disabled={busy}
              />
              Enable email alert configuration
            </label>

            <div className="admin-assign-form">
              <label>
                SMTP host
                <input
                  className="admin-input"
                  type="text"
                  value={smtpHost}
                  onChange={(event) => setSmtpHost(event.target.value)}
                  disabled={busy}
                />
              </label>
              <label>
                SMTP port
                <input
                  className="admin-input"
                  type="number"
                  min={1}
                  value={smtpPort}
                  onChange={(event) => setSmtpPort(Number(event.target.value))}
                  disabled={busy}
                />
              </label>
              <label>
                SMTP username
                <input
                  className="admin-input"
                  type="text"
                  value={smtpUsername}
                  onChange={(event) => setSmtpUsername(event.target.value)}
                  disabled={busy}
                />
              </label>
              <label>
                SMTP password
                <input
                  className="admin-input"
                  type="password"
                  value={smtpPassword}
                  onChange={(event) => setSmtpPassword(event.target.value)}
                  placeholder={
                    smtpPasswordConfigured
                      ? "Leave blank to keep the saved password"
                      : "Optional if your server allows unauthenticated relay"
                  }
                  disabled={busy || clearSmtpPassword}
                  autoComplete="new-password"
                />
              </label>
              <label>
                From address
                <input
                  className="admin-input"
                  type="email"
                  value={fromAddress}
                  onChange={(event) => setFromAddress(event.target.value)}
                  disabled={busy}
                />
              </label>
              <label>
                To addresses
                <textarea
                  className="admin-input"
                  rows={3}
                  value={toAddresses}
                  onChange={(event) => setToAddresses(event.target.value)}
                  placeholder="ops@example.com, oncall@example.com"
                  disabled={busy}
                />
              </label>
            </div>

            <label className="admin-checkbox">
              <input
                type="checkbox"
                checked={useTls}
                onChange={(event) => setUseTls(event.target.checked)}
                disabled={busy}
              />
              Use TLS for SMTP
            </label>

            {smtpPasswordConfigured && (
              <label className="admin-checkbox">
                <input
                  type="checkbox"
                  checked={clearSmtpPassword}
                  onChange={(event) => setClearSmtpPassword(event.target.checked)}
                  disabled={busy}
                />
                Remove saved SMTP password
              </label>
            )}
          </section>

          <section className="admin-section">
            <h2 className="admin-section-title">Alert thresholds</h2>
            <div className="admin-assign-form">
              <label>
                Offline threshold (ms)
                <input
                  className="admin-input"
                  type="number"
                  min={1}
                  value={offlineThresholdMs}
                  onChange={(event) =>
                    setOfflineThresholdMs(Number(event.target.value))
                  }
                  disabled={busy}
                />
              </label>
              <label>
                CPU temperature alert (°C)
                <input
                  className="admin-input"
                  type="number"
                  min={1}
                  step={0.1}
                  value={cpuTempAlertC}
                  onChange={(event) => setCpuTempAlertC(Number(event.target.value))}
                  disabled={busy}
                />
              </label>
              <label>
                Stuck progress window (hours)
                <input
                  className="admin-input"
                  type="number"
                  min={0}
                  step={0.1}
                  value={stuckProgressHours}
                  onChange={(event) =>
                    setStuckProgressHours(Number(event.target.value))
                  }
                  disabled={busy}
                />
              </label>
              <label>
                Dashboard URL for alert links
                <input
                  className="admin-input"
                  type="url"
                  value={dashboardUrl}
                  onChange={(event) => setDashboardUrl(event.target.value)}
                  placeholder="https://supervisor.example:3443/dashboard"
                  disabled={busy}
                />
              </label>
            </div>
          </section>

          <div className="deploy-actions">
            <button
              type="button"
              className="deploy-btn deploy-btn--primary"
              disabled={busy}
              onClick={save}
            >
              {busy ? "Saving…" : "Save alert settings"}
            </button>
          </div>
        </>
      )}
    </>
  );
}
