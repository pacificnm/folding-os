use std::fs;
use std::sync::Arc;
use std::time::Duration;

use foldops_types::IngestPayload;

use crate::collector::{collect_snapshot, CollectPaths, FahStats};
use crate::config::Config;
use crate::foldingos;

pub struct IngestClient {
    http: reqwest::Client,
    config: Arc<Config>,
}

impl IngestClient {
    pub fn new(config: Arc<Config>) -> Self {
        let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(30));
        if let Some(ca_path) = &config.supervisor_tls_ca {
            let ca_pem = fs::read(ca_path).unwrap_or_else(|error| {
                panic!("read SUPERVISOR_TLS_CA at {}: {error}", ca_path.display())
            });
            let certificate = reqwest::Certificate::from_pem(&ca_pem).unwrap_or_else(|error| {
                panic!("parse SUPERVISOR_TLS_CA at {}: {error}", ca_path.display())
            });
            builder = builder
                .tls_built_in_root_certs(false)
                .add_root_certificate(certificate);
        }
        Self {
            http: builder.build().expect("reqwest client"),
            config,
        }
    }

    pub async fn probe_supervisor(&self) {
        let url = format!("{}/api/machines", self.config.supervisor_base());
        match self
            .http
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => {}
            Ok(res) => {
                tracing::warn!(status = %res.status(), "supervisor reachable but returned error status");
            }
            Err(e) => {
                tracing::error!(
                    supervisor = %self.config.supervisor_url,
                    error = %e,
                    "cannot reach supervisor — nodes will show offline"
                );
            }
        }
    }

    pub async fn post_snapshot(&self, payload: &IngestPayload) -> Result<(), String> {
        let url = format!("{}/api/ingest", self.config.supervisor_base());
        let res = self
            .http
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.agent_token),
            )
            .json(payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Ingest failed ({status}): {text}"));
        }
        Ok(())
    }

    pub async fn collect_payload(&self) -> IngestPayload {
        let fah_stats = FahStats {
            donor: self.config.fah_donor_id.clone(),
            team: self.config.fah_team_number.clone(),
        };

        if self.config.uses_foldingos_delegation() {
            tracing::debug!(
                foldingosctl = %self.config.foldingosctl_path.display(),
                "collecting ingest payload via foldingosctl inspect"
            );
            foldingos::collect_delegated_snapshot(foldingos::DelegatedCollectConfig {
                foldingosctl_path: &self.config.foldingosctl_path,
                fah_stats,
            })
            .await
        } else {
            let paths = CollectPaths {
                fah_log_path: &self.config.fah_log_path,
                fah_db_path: &self.config.fah_db_path,
                fah_work_dir: &self.config.fah_work_dir,
                fah_ws_host: &self.config.fah_ws_host,
                fah_ws_port: self.config.fah_ws_port,
                fah_stats,
            };
            collect_snapshot(paths).await
        }
    }

    pub async fn collect_and_post(&self) -> Result<(), String> {
        let payload = self.collect_payload().await;

        self.post_snapshot(&payload).await?;

        let ppd = payload
            .fah
            .ppd
            .map(|p| p.to_string())
            .unwrap_or_else(|| "n/a".into());
        let progress = payload
            .fah
            .progress
            .map(|p| format!("{p}%"))
            .unwrap_or_else(|| "n/a".into());

        tracing::info!(
            hostname = %payload.hostname,
            node_id = payload.nodeId.as_deref().unwrap_or("n/a"),
            progress = %progress,
            ppd = %ppd,
            "ingest OK"
        );

        if payload.fah.systemdStatus == foldops_types::FahSystemdStatus::Active
            && payload.fah.ppd.is_none()
            && payload.fah.progress.is_none()
            && payload.fah.project.is_none()
        {
            tracing::warn!(
                hostname = %payload.hostname,
                db = %self.config.fah_db_path.display(),
                log = %self.config.fah_log_path.display(),
                "FAH active but no metrics"
            );
        }

        Ok(())
    }
}
