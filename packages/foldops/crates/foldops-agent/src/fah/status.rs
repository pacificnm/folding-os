use super::client_db::parse_fah_client_db;
use super::log::parse_fah_log;
use super::state::FahLogState;
use super::websocket::parse_fah_websocket;
use super::work_log::parse_fah_work_log;
use std::path::Path;

pub struct FahCollectResult {
    pub state: FahLogState,
    pub db_error: Option<String>,
    pub db_source: Option<String>,
}

fn merge_field<T: Clone>(current: &Option<T>, incoming: &Option<T>) -> Option<T> {
    current.clone().or_else(|| incoming.clone())
}

fn merge_progress(current: Option<f64>, incoming: Option<f64>) -> Option<f64> {
    match (current, incoming) {
        (_, Some(i)) if i <= 0.0 => current,
        (None, Some(i)) => Some(i),
        (Some(c), Some(i)) if c <= 0.0 => Some(i),
        (Some(c), Some(i)) => Some(c.max(i)),
        (c, None) => c,
    }
}

fn merge_states(primary: Option<FahLogState>, fallbacks: &[Option<FahLogState>]) -> FahLogState {
    let mut base = primary.unwrap_or_else(FahLogState::empty);

    for fb in fallbacks {
        let Some(fb) = fb else { continue };
        base.project = merge_field(&base.project, &fb.project);
        base.run = merge_field(&base.run, &fb.run);
        base.clone = merge_field(&base.clone, &fb.clone);
        base.gen = merge_field(&base.gen, &fb.gen);
        base.progress = merge_progress(base.progress, fb.progress);
        base.ppd = merge_field(&base.ppd, &fb.ppd);
        base.tpf = merge_field(&base.tpf, &fb.tpf);
        if !fb.recent_errors.is_empty() {
            base.recent_errors = fb.recent_errors.clone();
        }
    }

    base
}

pub async fn collect_fah_status(
    log_path: &Path,
    db_path: &Path,
    work_dir: &Path,
    ws_host: &str,
    ws_port: u16,
) -> FahCollectResult {
    let db_result = parse_fah_client_db(db_path).await;

    let from_log = parse_fah_log(log_path).await;
    let from_work = parse_fah_work_log(work_dir).await;
    let from_ws = parse_fah_websocket(ws_host, ws_port).await;

    let mut state = merge_states(
        from_work,
        &[Some(from_log.clone()), from_ws, db_result.state.clone()],
    );
    state.recent_errors = from_log.recent_errors;

    FahCollectResult {
        state,
        db_error: db_result.error,
        db_source: db_result.source.map(|s| s.as_str().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_takes_max_progress() {
        let a = FahLogState {
            progress: Some(10.0),
            ..FahLogState::empty()
        };
        let b = FahLogState {
            progress: Some(42.0),
            ppd: Some(100.0),
            ..FahLogState::empty()
        };
        let merged = merge_states(Some(a), &[Some(b)]);
        assert_eq!(merged.progress, Some(42.0));
        assert_eq!(merged.ppd, Some(100.0));
    }
}
