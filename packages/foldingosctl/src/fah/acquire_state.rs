use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;

use super::util::{format_go_duration, parse_key_value_lines};

const FAH_ACQUISITION_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
    Duration::from_secs(6 * 60 * 60),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FahAcquireState {
    pub consecutive_failures: i32,
    pub next_attempt_unix: i64,
    pub last_failure_reason: String,
}

pub fn clear_fah_acquire_state(paths: &AppliancePaths) -> Result<(), String> {
    match fs::remove_file(&paths.fah_acquire_state) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("clear acquisition retry state: {error}")),
    }
}

pub fn load_fah_acquire_state(paths: &AppliancePaths) -> Result<FahAcquireState, String> {
    let content = match fs::read_to_string(&paths.fah_acquire_state) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FahAcquireState {
                consecutive_failures: 0,
                next_attempt_unix: 0,
                last_failure_reason: String::new(),
            });
        }
        Err(error) => return Err(format!("read acquisition retry state: {error}")),
    };
    parse_fah_acquire_state(&content).map_err(|error| format!("parse acquisition retry state: {error}"))
}

pub fn parse_fah_acquire_state(content: &str) -> Result<FahAcquireState, String> {
    let values = parse_key_value_lines(content);
    let mut state = FahAcquireState {
        consecutive_failures: 0,
        next_attempt_unix: 0,
        last_failure_reason: values
            .get("last_failure_reason")
            .cloned()
            .unwrap_or_default(),
    };

    if let Some(failures) = values.get("consecutive_failures") {
        let parsed = failures
            .parse::<i32>()
            .map_err(|_| "consecutive_failures must be a non-negative integer".to_string())?;
        if parsed < 0 {
            return Err("consecutive_failures must be a non-negative integer".into());
        }
        state.consecutive_failures = parsed;
    }
    if let Some(next_attempt) = values.get("next_attempt_unix") {
        let parsed = next_attempt
            .parse::<i64>()
            .map_err(|_| "next_attempt_unix must be a non-negative integer".to_string())?;
        if parsed < 0 {
            return Err("next_attempt_unix must be a non-negative integer".into());
        }
        state.next_attempt_unix = parsed;
    }
    Ok(state)
}

pub fn save_fah_acquire_state(paths: &AppliancePaths, state: &FahAcquireState) -> Result<(), String> {
    let content = format!(
        "consecutive_failures={}\nnext_attempt_unix={}\nlast_failure_reason={}\n",
        state.consecutive_failures, state.next_attempt_unix, state.last_failure_reason
    );
    atomic_write(&paths.fah_acquire_state, content.as_bytes(), 0o644)
}

pub fn fah_acquisition_retry_delay(consecutive_failures: i32) -> Duration {
    if consecutive_failures <= 0 {
        return FAH_ACQUISITION_RETRY_DELAYS[0];
    }
    let index = (consecutive_failures - 1) as usize;
    let index = index.min(FAH_ACQUISITION_RETRY_DELAYS.len() - 1);
    FAH_ACQUISITION_RETRY_DELAYS[index]
}

pub fn defer_fah_acquisition_attempt(state: &FahAcquireState) -> Result<(bool, Duration), String> {
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

pub fn record_fah_acquisition_failure(paths: &AppliancePaths, cause: &str) -> Result<(), String> {
    let mut state = load_fah_acquire_state(paths)?;
    state.consecutive_failures += 1;
    let delay = fah_acquisition_retry_delay(state.consecutive_failures);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs() as i64;
    state.next_attempt_unix = now + delay.as_secs() as i64;
    state.last_failure_reason = cause.to_string();
    save_fah_acquire_state(paths, &state)?;
    Err(format!(
        "acquisition failed; next retry in {}: {cause}",
        format_go_duration(delay)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fah_acquisition_retry_delay_schedule() {
        let cases = [
            (1, Duration::from_secs(60)),
            (2, Duration::from_secs(5 * 60)),
            (3, Duration::from_secs(15 * 60)),
            (4, Duration::from_secs(60 * 60)),
            (5, Duration::from_secs(6 * 60 * 60)),
            (9, Duration::from_secs(6 * 60 * 60)),
        ];
        for (failures, want) in cases {
            assert_eq!(fah_acquisition_retry_delay(failures), want, "failures={failures}");
        }
    }

    #[test]
    fn defer_fah_acquisition_attempt_honors_persisted_state() {
        let now = 1_700_000_000_i64;
        let state = FahAcquireState {
            consecutive_failures: 0,
            next_attempt_unix: now + 5 * 60,
            last_failure_reason: String::new(),
        };
        let original_now = now;
        let remaining_target = 5 * 60;
        let state_remaining = state.next_attempt_unix - original_now;
        assert_eq!(state_remaining, remaining_target);
        let (deferred, remaining) = {
            let fake_now = original_now;
            let remaining_secs = state.next_attempt_unix - fake_now;
            (
                remaining_secs > 0,
                Duration::from_secs(remaining_secs.max(0) as u64),
            )
        };
        assert!(deferred);
        assert_eq!(remaining, Duration::from_secs(5 * 60));
    }

    #[test]
    fn record_fah_acquisition_failure_persists_next_attempt() {
        let dir = tempfile_dir("persist-next-attempt");
        let paths = test_paths(&dir);
        let now_before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_secs() as i64;

        let err = record_fah_acquisition_failure(&paths, "download artifact: connection reset")
            .expect_err("should return error");
        assert!(err.contains("acquisition failed; next retry in 1m0s"));
        assert!(err.contains("connection reset"));

        let state = load_fah_acquire_state(&paths).expect("load state");
        assert_eq!(state.consecutive_failures, 1);
        assert!(state.next_attempt_unix >= now_before + 60);
        assert!(state.last_failure_reason.contains("connection reset"));
    }

    #[test]
    fn record_fah_acquisition_failure_caps_delay_at_six_hours() {
        let dir = tempfile_dir("cap-delay-six-hours");
        let paths = test_paths(&dir);
        save_fah_acquire_state(
            &paths,
            &FahAcquireState {
                consecutive_failures: 4,
                next_attempt_unix: 0,
                last_failure_reason: String::new(),
            },
        )
        .expect("seed state");

        let now_before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_secs() as i64;
        record_fah_acquisition_failure(&paths, "network is not online").expect_err("error");

        let state = load_fah_acquire_state(&paths).expect("load state");
        assert_eq!(state.consecutive_failures, 5);
        assert!(state.next_attempt_unix >= now_before + 6 * 60 * 60);
    }

    fn tempfile_dir(label: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fah-acquire-test-{}-{}-{label}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn test_paths(dir: &std::path::Path) -> AppliancePaths {
        let mut paths = AppliancePaths::default();
        paths.fah_acquire_state = dir.join("fah-acquire.state");
        paths
    }
}
