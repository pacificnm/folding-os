import { useState } from "react";
import { createRecoveryExport, downloadRecoveryExport } from "../../api";
import type { RecoveryExportResponse } from "../../types";

export function AdminRecovery() {
  const [includeSecrets, setIncludeSecrets] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [lastExport, setLastExport] = useState<RecoveryExportResponse | null>(
    null,
  );

  const backupNow = async () => {
    setBusy(true);
    setError(null);
    setStatus("Creating backup…");
    try {
      const exportInfo = await createRecoveryExport(includeSecrets);
      setLastExport(exportInfo);
      setStatus("Downloading to your computer…");
      await downloadRecoveryExport();
      setStatus(
        `Backup complete (${formatBytes(exportInfo.size_bytes)} saved to your downloads folder).`,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Backup failed");
      setStatus(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <p className="admin-intro">
        Save your FoldOps settings and farm database to your computer. Use this
        after setup or before major changes so you can restore later.
      </p>

      {error && <p className="message error">{error}</p>}
      {status && <p className="message admin-status">{status}</p>}

      <section className="admin-section admin-backup-panel">
        <div className="deploy-actions">
          <button
            type="button"
            className="deploy-btn deploy-btn--primary deploy-btn--large"
            disabled={busy}
            onClick={backupNow}
          >
            {busy ? "Working…" : "Backup now"}
          </button>
        </div>

        <button
          type="button"
          className="admin-advanced-toggle"
          onClick={() => setShowAdvanced((open) => !open)}
          aria-expanded={showAdvanced}
        >
          {showAdvanced ? "Hide options" : "More options"}
        </button>

        {showAdvanced && (
          <div className="admin-advanced-panel">
            <label className="admin-checkbox">
              <input
                type="checkbox"
                checked={includeSecrets}
                onChange={(event) => setIncludeSecrets(event.target.checked)}
                disabled={busy}
              />
              Include TLS private keys (only if you need to restore certificates)
            </label>

            {lastExport && (
              <dl className="admin-export-meta">
                <div>
                  <dt>Created</dt>
                  <dd>
                    {lastExport.export_timestamp
                      ? new Date(lastExport.export_timestamp).toLocaleString()
                      : "—"}
                  </dd>
                </div>
                <div>
                  <dt>Size</dt>
                  <dd>{formatBytes(lastExport.size_bytes)}</dd>
                </div>
                <div>
                  <dt>Files</dt>
                  <dd>{lastExport.file_count ?? "—"}</dd>
                </div>
              </dl>
            )}
          </div>
        )}
      </section>
    </>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`;
}
