use std::path::Path;

const DEFAULT_MAX_BYTES: usize = 256 * 1024;

pub struct LogTail {
    pub lines: Vec<String>,
    pub path: String,
}

pub async fn read_log_tail(
    log_path: &Path,
    max_lines: usize,
    max_bytes: usize,
) -> Option<LogTail> {
    let mut content = tokio::fs::read_to_string(log_path).await.ok()?;

    if content.len() > max_bytes {
        content = content[content.len() - max_bytes..].to_string();
        if let Some(first_nl) = content.find('\n') {
            content = content[first_nl + 1..].to_string();
        }
    }

    let lines: Vec<String> = content
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();

    let start = lines.len().saturating_sub(max_lines);
    Some(LogTail {
        path: log_path.to_string_lossy().into_owned(),
        lines: lines[start..].to_vec(),
    })
}

pub async fn read_log_tail_default(log_path: &Path, max_lines: usize) -> Option<LogTail> {
    read_log_tail(log_path, max_lines, DEFAULT_MAX_BYTES).await
}
