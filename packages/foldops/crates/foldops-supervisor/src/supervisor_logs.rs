#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorLogSource {
    Foldops,
    Foldingosctl,
}

impl SupervisorLogSource {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "foldops" => Some(Self::Foldops),
            "foldingosctl" => Some(Self::Foldingosctl),
            _ => None,
        }
    }

    fn journal_descriptor(self) -> &'static str {
        match self {
            Self::Foldops => "journal:foldingos-foldops-supervisor.service",
            Self::Foldingosctl => "journal:_COMM=foldingosctl",
        }
    }
}

pub async fn fetch_supervisor_logs(
    source: SupervisorLogSource,
    lines: u32,
) -> Result<(String, Vec<String>), String> {
    let mut cmd = tokio::process::Command::new("/usr/bin/journalctl");
    cmd.args([
        "-q",
        "--no-pager",
        "-o",
        "short-iso",
        "-n",
        &lines.to_string(),
    ]);

    match source {
        SupervisorLogSource::Foldops => {
            cmd.args(["-u", "foldingos-foldops-supervisor.service"]);
        }
        SupervisorLogSource::Foldingosctl => {
            cmd.arg("_COMM=foldingosctl");
        }
    }

    let output = tokio::time::timeout(std::time::Duration::from_secs(10), cmd.output())
        .await
        .map_err(|_| "journalctl timed out".to_string())?
        .map_err(|error| format!("failed to run journalctl: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("journalctl failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "journalctl output was not valid UTF-8".to_string())?;

    let log_lines: Vec<String> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect();

    Ok((source.journal_descriptor().to_string(), log_lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supervisor_log_sources() {
        assert_eq!(
            SupervisorLogSource::parse("foldops"),
            Some(SupervisorLogSource::Foldops)
        );
        assert_eq!(
            SupervisorLogSource::parse("foldingosctl"),
            Some(SupervisorLogSource::Foldingosctl)
        );
        assert_eq!(SupervisorLogSource::parse("fah"), None);
    }
}
