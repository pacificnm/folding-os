use std::process::{Command, Stdio};

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
