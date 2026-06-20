use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::foldops::util::foldops_acquire_state_path;
use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;

const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
    Duration::from_secs(6 * 60 * 60),
];

#[derive(Debug, Clone, Default)]
pub struct FoldOpsAcquireState {
    pub consecutive_failures: i32,
    pub next_attempt_unix: i64,
    pub last_failure_reason: String,
}

pub fn clear_foldops_acquire_state(paths: &AppliancePaths) -> Result<(), String> {
    let path = foldops_acquire_state_path(paths);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("clear acquisition retry state: {error}")),
    }
}

pub fn load_foldops_acquire_state(paths: &AppliancePaths) -> Result<FoldOpsAcquireState, String> {
    let path = foldops_acquire_state_path(paths);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FoldOpsAcquireState::default())
        }
        Err(error) => return Err(format!("read acquisition retry state: {error}")),
    };
    parse_foldops_acquire_state(&content)
        .map_err(|error| format!("parse acquisition retry state: {error}"))
}

pub fn parse_foldops_acquire_state(content: &str) -> Result<FoldOpsAcquireState, String> {
    let values = crate::foldops::util::parse_key_value_lines(content);
    let mut state = FoldOpsAcquireState::default();

    if let Some(failures) = values.get("consecutive_failures") {
        let parsed: i32 = failures
            .parse()
            .map_err(|_| "consecutive_failures must be a non-negative integer".to_string())?;
        if parsed < 0 {
            return Err("consecutive_failures must be a non-negative integer".into());
        }
        state.consecutive_failures = parsed;
    }
    if let Some(next_attempt) = values.get("next_attempt_unix") {
        let parsed: i64 = next_attempt
            .parse()
            .map_err(|_| "next_attempt_unix must be a non-negative integer".to_string())?;
        if parsed < 0 {
            return Err("next_attempt_unix must be a non-negative integer".into());
        }
        state.next_attempt_unix = parsed;
    }
    state.last_failure_reason = values
        .get("last_failure_reason")
        .cloned()
        .unwrap_or_default();
    Ok(state)
}

pub fn save_foldops_acquire_state(
    paths: &AppliancePaths,
    state: &FoldOpsAcquireState,
) -> Result<(), String> {
    let content = format!(
        "consecutive_failures={}\nnext_attempt_unix={}\nlast_failure_reason={}\n",
        state.consecutive_failures, state.next_attempt_unix, state.last_failure_reason
    );
    atomic_write(
        &foldops_acquire_state_path(paths),
        content.as_bytes(),
        0o644,
    )
}

fn acquisition_retry_delay(consecutive_failures: i32) -> Duration {
    if consecutive_failures <= 0 {
        return RETRY_DELAYS[0];
    }
    let index = (consecutive_failures - 1) as usize;
    RETRY_DELAYS
        .get(index)
        .copied()
        .unwrap_or(*RETRY_DELAYS.last().unwrap())
}

pub fn defer_foldops_acquisition_attempt(
    state: &FoldOpsAcquireState,
) -> Result<(bool, Duration), String> {
    if state.next_attempt_unix == 0 {
        return Ok((false, Duration::ZERO));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs() as i64;
    let remaining_secs = state.next_attempt_unix - now;
    if remaining_secs > 0 {
        return Ok((true, Duration::from_secs(remaining_secs as u64)));
    }
    Ok((false, Duration::ZERO))
}

pub fn record_foldops_acquisition_failure(
    paths: &AppliancePaths,
    cause: &str,
) -> Result<(), String> {
    let mut state = load_foldops_acquire_state(paths)?;
    state.consecutive_failures += 1;
    let delay = acquisition_retry_delay(state.consecutive_failures);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs() as i64;
    state.next_attempt_unix = now + delay.as_secs() as i64;
    state.last_failure_reason = cause.to_string();
    save_foldops_acquire_state(paths, &state)?;
    Err(format!(
        "acquisition failed; next retry in {}s: {cause}",
        delay.as_secs()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_acquire_state() {
        let state = parse_foldops_acquire_state(
            "consecutive_failures=2\nnext_attempt_unix=1700000000\nlast_failure_reason=network is not online\n",
        )
        .expect("parse state");
        assert_eq!(state.consecutive_failures, 2);
        assert_eq!(state.next_attempt_unix, 1_700_000_000);
        assert_eq!(state.last_failure_reason, "network is not online");
    }
}
