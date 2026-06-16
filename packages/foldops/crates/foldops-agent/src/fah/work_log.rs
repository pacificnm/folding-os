use std::path::{Path, PathBuf};
use std::time::SystemTime;

use regex::Regex;
use std::sync::LazyLock;

use super::state::FahLogState;

static PROJECT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Project:\s*(\d+)\s*\(\s*Run\s*(\d+)\s*,\s*Clone\s*(\d+)\s*,\s*Gen\s*(\d+)\s*\)")
        .unwrap()
});
static STEPS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Completed\s+(\d+)\s+out\s+of\s+(\d+)\s+steps\s+\(([\d.]+)%\)").unwrap()
});

pub async fn get_newest_work_log_path(work_dir: &Path) -> Option<PathBuf> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;

    let mut units = match tokio::fs::read_dir(work_dir).await {
        Ok(u) => u,
        Err(_) => return None,
    };

    while let Ok(Some(entry)) = units.next_entry().await {
        let unit_path = entry.path();
        let mut files = match tokio::fs::read_dir(&unit_path).await {
            Ok(f) => f,
            Err(_) => continue,
        };

        while let Ok(Some(file_entry)) = files.next_entry().await {
            let name = file_entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("logfile_") || !name.ends_with(".txt") {
                continue;
            }
            let path = file_entry.path();
            let meta = match tokio::fs::metadata(&path).await {
                Ok(m) if m.is_file() => m,
                _ => continue,
            };
            let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if newest.as_ref().is_none_or(|(_, t)| mtime > *t) {
                newest = Some((path, mtime));
            }
        }
    }

    newest.map(|(p, _)| p)
}

pub async fn parse_fah_work_log(work_dir: &Path) -> Option<FahLogState> {
    let log_path = get_newest_work_log_path(work_dir).await?;
    let content = tokio::fs::read_to_string(&log_path).await.ok()?;

    let mut state = FahLogState::empty();
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    let tail = if lines.len() > 200 {
        &lines[lines.len() - 200..]
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
        if let Some(caps) = STEPS_RE.captures(line) {
            state.progress = caps.get(3).and_then(|m| m.as_str().parse().ok());
        }
    }

    if state.project.is_none() && state.progress.is_none() {
        return None;
    }
    Some(state)
}
