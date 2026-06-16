use chrono::DateTime;

use crate::ingest::{IngestPayload, NodeLogs};

/// Validation errors for ingest payloads (mirrors Zod constraints in shared schema).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IngestValidationError {
    #[error("hostname must not be empty")]
    EmptyHostname,
    #[error("timestamp is not a valid ISO 8601 datetime: {0}")]
    InvalidTimestamp(String),
    #[error("logs.fah exceeds maximum length of {max} (got {len})")]
    LogsFahTooLong { len: usize, max: usize },
    #[error("logs.work exceeds maximum length of {max} (got {len})")]
    LogsWorkTooLong { len: usize, max: usize },
}

const MAX_LOG_LINES: usize = 200;

/// Validate semantic constraints not enforced by serde alone.
pub fn validate_ingest_payload(payload: &IngestPayload) -> Result<(), IngestValidationError> {
    if payload.hostname.trim().is_empty() {
        return Err(IngestValidationError::EmptyHostname);
    }

    DateTime::parse_from_rfc3339(&payload.timestamp)
        .map_err(|_| IngestValidationError::InvalidTimestamp(payload.timestamp.clone()))?;

    if let Some(logs) = &payload.logs {
        validate_node_logs(logs)?;
    }

    Ok(())
}

fn validate_node_logs(logs: &NodeLogs) -> Result<(), IngestValidationError> {
    if logs.fah.len() > MAX_LOG_LINES {
        return Err(IngestValidationError::LogsFahTooLong {
            len: logs.fah.len(),
            max: MAX_LOG_LINES,
        });
    }
    if logs.work.len() > MAX_LOG_LINES {
        return Err(IngestValidationError::LogsWorkTooLong {
            len: logs.work.len(),
            max: MAX_LOG_LINES,
        });
    }
    Ok(())
}

/// Parse JSON and validate an ingest payload.
pub fn parse_ingest_json(json: &str) -> Result<IngestPayload, IngestJsonError> {
    let payload: IngestPayload = serde_json::from_str(json)?;
    validate_ingest_payload(&payload)?;
    Ok(payload)
}

#[derive(Debug, thiserror::Error)]
pub enum IngestJsonError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Validation(#[from] IngestValidationError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{
        Disk, Fah, FahSystemdStatus, Maintenance, Memory, Network, System,
    };

    fn sample_payload() -> IngestPayload {
        IngestPayload {
            hostname: "fah-01".into(),
            timestamp: "2026-06-11T12:00:00Z".into(),
            system: System {
                uptime: 86400.0,
                loadAvg: [0.5, 0.4, 0.3],
                cpuUsage: 12.5,
                memory: Memory {
                    total: 16_000_000_000.0,
                    used: 8_000_000_000.0,
                    free: 8_000_000_000.0,
                    percent: 50.0,
                },
                disk: Disk {
                    total: 500_000_000_000.0,
                    used: 100_000_000_000.0,
                    free: 400_000_000_000.0,
                    percent: 20.0,
                },
                network: Network {
                    rxBytes: 1_000_000,
                    txBytes: 500_000,
                    rxSec: Some(1024.0),
                    txSec: Some(512.0),
                },
                cpuTemp: Some(55.0),
                chassisTemp: None,
            },
            fah: Fah {
                systemdStatus: FahSystemdStatus::Active,
                project: Some("18400".into()),
                run: Some(0.0),
                clone: Some(0.0),
                gen: Some(0.0),
                progress: Some(42.5),
                ppd: Some(250_000.0),
                tpf: Some("00:01:30".into()),
                recentErrors: vec![],
                statsDonor: None,
                statsTeam: None,
            },
            maintenance: Maintenance {
                aptUpdatesAvailable: 0,
                rebootRequired: false,
            },
            logs: None,
        }
    }

    #[test]
    fn valid_sample_passes() {
        validate_ingest_payload(&sample_payload()).unwrap();
    }

    #[test]
    fn rejects_empty_hostname() {
        let mut payload = sample_payload();
        payload.hostname = "  ".into();
        assert_eq!(
            validate_ingest_payload(&payload).unwrap_err(),
            IngestValidationError::EmptyHostname
        );
    }

    #[test]
    fn rejects_invalid_timestamp() {
        let mut payload = sample_payload();
        payload.timestamp = "not-a-date".into();
        assert!(matches!(
            validate_ingest_payload(&payload).unwrap_err(),
            IngestValidationError::InvalidTimestamp(_)
        ));
    }
}
