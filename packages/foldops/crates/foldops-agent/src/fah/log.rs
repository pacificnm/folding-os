use std::sync::LazyLock;

use regex::Regex;

use super::state::FahLogState;

static PROJECT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Project:\s*(\d+)\s*\(\s*Run\s*(\d+)\s*,\s*Clone\s*(\d+)\s*,\s*Gen\s*(\d+)\s*\)")
        .unwrap()
});
static PROGRESS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)Progress:\s*([\d.]+)\s*%").unwrap());
static STEPS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Completed\s+(\d+)\s+out\s+of\s+(\d+)\s+steps\s+\(([\d.]+)%\)").unwrap()
});
static PPD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)PPD[:\s]+([\d,.]+)").unwrap());
static TPF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)TPF[:\s]+([\d:]+(?:\.\d+)?)").unwrap());
static ERROR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(ERROR|FATAL|Exception|failed)\b").unwrap());

pub async fn parse_fah_log(log_path: &std::path::Path) -> FahLogState {
    let mut state = FahLogState::empty();

    let content = match tokio::fs::read_to_string(log_path).await {
        Ok(c) => c,
        Err(_) => return state,
    };

    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    let tail = if lines.len() > 500 {
        &lines[lines.len() - 500..]
    } else {
        &lines[..]
    };

    for line in tail {
        if let Some(caps) = PROJECT_RE.captures(line) {
            state.project = caps.get(1).map(|m| m.as_str().to_string());
            state.run = caps.get(2).and_then(|m| m.as_str().parse().ok());
            state.clone = caps.get(3).and_then(|m| m.as_str().parse().ok());
            state.gen = caps.get(4).and_then(|m| m.as_str().parse().ok());
        }
        if let Some(caps) = PROGRESS_RE.captures(line) {
            state.progress = caps.get(1).and_then(|m| m.as_str().parse().ok());
        }
        if let Some(caps) = STEPS_RE.captures(line) {
            state.progress = caps.get(3).and_then(|m| m.as_str().parse().ok());
        }
        if let Some(caps) = PPD_RE.captures(line) {
            let raw = caps.get(1).map(|m| m.as_str().replace(',', "")).unwrap_or_default();
            state.ppd = raw.parse().ok();
        }
        if let Some(caps) = TPF_RE.captures(line) {
            state.tpf = caps.get(1).map(|m| m.as_str().to_string());
        }
        if ERROR_RE.is_match(line) {
            state.recent_errors.push(line.trim().to_string());
        }
    }

    if state.recent_errors.len() > 10 {
        let start = state.recent_errors.len() - 10;
        state.recent_errors = state.recent_errors[start..].to_vec();
    }

    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_project_and_ppd() {
        let content = "Project: 18400 (Run 0, Clone 1, Gen 2)\nPPD: 250,000\nProgress: 42.5%\n";
        let state = parse_content(content);
        assert_eq!(state.project.as_deref(), Some("18400"));
        assert_eq!(state.run, Some(0.0));
        assert_eq!(state.ppd, Some(250_000.0));
        assert_eq!(state.progress, Some(42.5));
    }

    fn parse_content(content: &str) -> FahLogState {
        let mut state = FahLogState::empty();
        for line in content.lines().filter(|l| !l.is_empty()) {
            if let Some(caps) = PROJECT_RE.captures(line) {
                state.project = caps.get(1).map(|m| m.as_str().to_string());
                state.run = caps.get(2).and_then(|m| m.as_str().parse().ok());
            }
            if let Some(caps) = PPD_RE.captures(line) {
                let raw = caps.get(1).map(|m| m.as_str().replace(',', "")).unwrap_or_default();
                state.ppd = raw.parse().ok();
            }
            if let Some(caps) = PROGRESS_RE.captures(line) {
                state.progress = caps.get(1).and_then(|m| m.as_str().parse().ok());
            }
        }
        state
    }
}
