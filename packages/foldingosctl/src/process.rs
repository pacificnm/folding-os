use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static DEFERRED_RESTART_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn command_output(name: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(name)
        .args(args)
        .output()
        .map_err(|error| format!("{name} failed: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("{name} failed: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn run_command(name: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(name)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("{name} failed: {error}"))?;
    if !status.success() {
        return Err(format!("{name} failed: {status}"));
    }
    Ok(())
}

pub fn run_fsck_ext4(device: &str, force: bool) -> Result<(), String> {
    let mut args = vec!["-p"];
    if force {
        args.insert(0, "-f");
    }
    args.push(device);
    let status = Command::new("fsck.ext4")
        .args(&args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("fsck.ext4 failed: {error}"))?;
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        if code != 1 {
            return Err(format!("fsck.ext4 failed: {status}"));
        }
    }
    if force {
        println!("Force-checked {device} before data expansion.");
    } else {
        println!("Checked {device} before data mount.");
    }
    Ok(())
}

/// Restart a systemd unit after a short delay so the caller can finish an in-flight HTTP response.
pub fn schedule_deferred_systemd_restart(unit: &str) -> Result<(), String> {
    schedule_deferred_systemd_restart_after(unit, 2)
}

pub fn schedule_deferred_systemd_restart_after(unit: &str, delay_secs: u32) -> Result<(), String> {
    let unit_slug = unit.replace('.', "-");
    let script = format!("sleep {delay_secs}; systemctl restart {unit}");
    schedule_deferred_shell_command(&format!("restart-{unit_slug}"), &script)
}

/// Run a shell script in the background after the caller returns.
pub fn schedule_deferred_shell_command(name: &str, script: &str) -> Result<(), String> {
    let name_slug = name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    let sequence = DEFERRED_RESTART_COUNTER.fetch_add(1, Ordering::Relaxed);
    let transient_unit = format!(
        "foldingos-deferred-{}-{}-{}",
        name_slug,
        std::process::id(),
        sequence
    );
    if Command::new("systemd-run")
        .args([
            "--no-block",
            "--collect",
            &format!("--unit={transient_unit}"),
            "--",
            "sh",
            "-c",
            script,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        return Ok(());
    }

    Command::new("sh")
        .arg("-c")
        .arg(format!("({script}) >/dev/null 2>&1 &"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("schedule deferred command {name}: {error}"))?;
    Ok(())
}

pub fn write_console(message: &str) -> Result<(), String> {
    use std::io::Write;
    let mut first_err: Option<String> = None;
    for device in ["/dev/tty1", "/dev/console"] {
        match std::fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(device)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(message.as_bytes()) {
                    if first_err.is_none() {
                        first_err = Some(format!("write {device}: {error}"));
                    }
                }
            }
            Err(error) => {
                if first_err.is_none() {
                    first_err = Some(format!("open {device}: {error}"));
                }
            }
        }
    }
    first_err.map_or(Ok(()), Err)
}
