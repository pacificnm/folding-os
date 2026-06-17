use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::fs_atomic::atomic_write;

const ACQUISITION_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
    Duration::from_secs(6 * 60 * 60),
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolsAcquireState {
    pub consecutive_failures: i32,
    pub next_attempt_unix: i64,
    pub last_failure_reason: String,
}

pub fn clear_tools_acquire_state(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("clear tools acquisition retry state: {error}")),
    }
}

pub fn load_tools_acquire_state(path: &Path) -> Result<ToolsAcquireState, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ToolsAcquireState::default());
        }
        Err(error) => return Err(format!("read tools acquisition retry state: {error}")),
    };
    parse_tools_acquire_state(&content)
        .map_err(|error| format!("parse tools acquisition retry state: {error}"))
}

pub fn parse_tools_acquire_state(content: &str) -> Result<ToolsAcquireState, String> {
    let values = parse_key_value_lines(content);
    let mut state = ToolsAcquireState::default();

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

pub fn save_tools_acquire_state(path: &Path, state: &ToolsAcquireState) -> Result<(), String> {
    let content = format!(
        "consecutive_failures={}\nnext_attempt_unix={}\nlast_failure_reason={}\n",
        state.consecutive_failures, state.next_attempt_unix, state.last_failure_reason
    );
    atomic_write(path, content.as_bytes(), 0o644)
}

pub fn tools_acquisition_retry_delay(consecutive_failures: i32) -> Duration {
    if consecutive_failures <= 0 {
        return ACQUISITION_RETRY_DELAYS[0];
    }
    let index = (consecutive_failures - 1) as usize;
    let index = index.min(ACQUISITION_RETRY_DELAYS.len() - 1);
    ACQUISITION_RETRY_DELAYS[index]
}

pub fn defer_tools_acquisition_attempt(
    state: &ToolsAcquireState,
    now_unix: i64,
) -> Result<Option<Duration>, String> {
    if state.next_attempt_unix == 0 {
        return Ok(None);
    }
    let remaining = state.next_attempt_unix - now_unix;
    if remaining > 0 {
        return Ok(Some(Duration::from_secs(remaining as u64)));
    }
    Ok(None)
}

pub fn record_tools_acquisition_failure(
    path: &Path,
    cause: &str,
    now_unix: i64,
) -> Result<(), String> {
    let mut state = load_tools_acquire_state(path)?;
    state.consecutive_failures += 1;
    let delay = tools_acquisition_retry_delay(state.consecutive_failures);
    state.next_attempt_unix = now_unix + delay.as_secs() as i64;
    state.last_failure_reason = cause.to_string();
    save_tools_acquire_state(path, &state)?;
    Err(format!(
        "tools acquisition failed; next retry in {}: {cause}",
        format_duration_rounded(delay)
    ))
}

fn parse_key_value_lines(content: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(key.trim().to_string(), value.trim().to_string());
    }
    values
}

pub fn format_duration_rounded(duration: Duration) -> String {
    let total = duration.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}h{minutes}m{seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m{seconds}s")
    } else {
        format!("{seconds}s")
    }
}

pub fn format_unix_rfc3339_utc(unix: i64) -> String {
    let mut days = unix / 86_400;
    let mut time_of_day = unix % 86_400;
    if time_of_day < 0 {
        time_of_day += 86_400;
        days -= 1;
    }
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let mut year = 1970_i64;
    loop {
        let year_days = if is_leap_year(year) { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
    }

    let month_lengths = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for month_length in month_lengths {
        if days < month_length {
            break;
        }
        days -= month_length;
        month += 1;
    }
    let day = days + 1;

    format!(
        "{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z"
    )
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tools_acquire_state_reads_fields() {
        let state = parse_tools_acquire_state(
            "consecutive_failures=2\nnext_attempt_unix=1704067200\nlast_failure_reason=network is not online\n",
        )
        .unwrap();
        assert_eq!(state.consecutive_failures, 2);
        assert_eq!(state.next_attempt_unix, 1_704_067_200);
        assert_eq!(state.last_failure_reason, "network is not online");
    }

    #[test]
    fn tools_acquisition_retry_delay_uses_backoff_table() {
        assert_eq!(
            tools_acquisition_retry_delay(1),
            Duration::from_secs(60)
        );
        assert_eq!(
            tools_acquisition_retry_delay(99),
            ACQUISITION_RETRY_DELAYS[ACQUISITION_RETRY_DELAYS.len() - 1]
        );
    }

    #[test]
    fn defer_tools_acquisition_attempt_honors_next_attempt() {
        let state = ToolsAcquireState {
            next_attempt_unix: 1_000,
            ..ToolsAcquireState::default()
        };
        let remaining = defer_tools_acquisition_attempt(&state, 900).unwrap();
        assert_eq!(remaining, Some(Duration::from_secs(100)));
        assert!(defer_tools_acquisition_attempt(&state, 1_000)
            .unwrap()
            .is_none());
    }
}
