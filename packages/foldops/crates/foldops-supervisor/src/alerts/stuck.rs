use crate::db::SnapshotRow;

const MIN_SNAPSHOTS_IN_WINDOW: usize = 10;
const SPAN_FRACTION: f64 = 0.85;

pub struct StuckProgressResult {
    pub progress: f64,
    pub span_hours: f64,
    pub min_progress: f64,
    pub max_progress: f64,
}

pub fn detect_stuck_progress(
    snapshots_asc: &[SnapshotRow],
    stuck_ms: i64,
) -> Option<StuckProgressResult> {
    detect_stuck_progress_opts(snapshots_asc, stuck_ms, 0.5, 0.15)
}

pub fn detect_stuck_progress_opts(
    snapshots_asc: &[SnapshotRow],
    stuck_ms: i64,
    min_progress: f64,
    epsilon: f64,
) -> Option<StuckProgressResult> {
    if stuck_ms <= 0 || snapshots_asc.is_empty() {
        return None;
    }

    let latest = snapshots_asc.last()?;
    let latest_progress = latest.progress?;
    if latest.fah_status != "active" || latest_progress < min_progress {
        return None;
    }

    let latest_ms = chrono::DateTime::parse_from_rfc3339(&latest.created_at)
        .ok()?
        .timestamp_millis();
    let cutoff_ms = latest_ms - stuck_ms;

    let in_window: Vec<_> = snapshots_asc
        .iter()
        .filter(|s| {
            chrono::DateTime::parse_from_rfc3339(&s.created_at)
                .map(|t| t.timestamp_millis() >= cutoff_ms)
                .unwrap_or(false)
                && s.fah_status == "active"
                && s.progress.is_some_and(|p| p >= min_progress)
        })
        .collect();

    if in_window.len() < MIN_SNAPSHOTS_IN_WINDOW {
        return None;
    }

    let oldest = in_window.first()?;
    let span_ms = chrono::DateTime::parse_from_rfc3339(&latest.created_at)
        .ok()?
        .timestamp_millis()
        - chrono::DateTime::parse_from_rfc3339(&oldest.created_at)
            .ok()?
            .timestamp_millis();
    if (span_ms as f64) < stuck_ms as f64 * SPAN_FRACTION {
        return None;
    }

    let values: Vec<f64> = in_window.iter().filter_map(|s| s.progress).collect();
    let min_p = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_p = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if max_p - min_p > epsilon {
        return None;
    }

    Some(StuckProgressResult {
        progress: latest_progress,
        span_hours: ((span_ms as f64 / 3_600_000.0) * 10.0).round() / 10.0,
        min_progress: min_p,
        max_progress: max_p,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SnapshotRow;

    fn snap(at: &str, progress: f64) -> SnapshotRow {
        SnapshotRow {
            id: 1,
            hostname: "fah-01".into(),
            created_at: at.into(),
            payload: "{}".into(),
            fah_status: "active".into(),
            project: Some("1".into()),
            run: None,
            clone: None,
            gen: None,
            progress: Some(progress),
            ppd: Some(100.0),
            cpu_usage: None,
            memory_percent: None,
            disk_percent: None,
            cpu_temp: None,
            chassis_temp: None,
            apt_updates: 0,
            reboot_required: 0,
        }
    }

    #[test]
    fn detects_flat_progress() {
        let snaps: Vec<_> = (0..20)
            .map(|i| {
                let mins = i * 15;
                let h = 10 + mins / 60;
                let m = mins % 60;
                snap(&format!("2026-06-11T{h:02}:{m:02}:00Z"), 42.0)
            })
            .collect();
        assert!(detect_stuck_progress(&snaps, 4 * 3_600_000).is_some());
    }
}
