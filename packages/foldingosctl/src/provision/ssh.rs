use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;
use crate::provision::util::{command_input, empty_human_result, run_command};

pub fn provision_ssh(paths: &AppliancePaths) -> Result<serde_json::Value, String> {
    ensure_host_key(paths)?;
    let content = match fs::read(&paths.provisioned_ssh_keys) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Ok(existing) = fs::read(&paths.active_ssh_keys) {
                validate_authorized_keys(&existing)?;
            }
            println!(
                "No SSH provisioning file present; SSH remains unavailable without persistent authorized keys."
            );
            return Ok(empty_human_result());
        }
        Err(error) => return Err(error.to_string()),
    };
    let keys = validate_authorized_keys(&content)?;
    atomic_write(&paths.active_ssh_keys, &keys, 0o644)?;
    fs::remove_file(&paths.provisioned_ssh_keys).map_err(|error| error.to_string())?;
    println!("Activated provisioned SSH administrator keys.");
    Ok(empty_human_result())
}

fn ensure_host_key(paths: &AppliancePaths) -> Result<(), String> {
    let host_key = &paths.ssh_host_key;
    if let Ok(meta) = fs::metadata(host_key) {
        if meta.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(host_key, fs::Permissions::from_mode(0o600))
                    .map_err(|error| error.to_string())?;
            }
            if let Ok(public_key) =
                command_input("", "ssh-keygen", &["-y", "-f", &host_key.to_string_lossy()])
            {
                let mut payload = public_key.trim().to_string();
                payload.push('\n');
                atomic_write(&paths.ssh_host_key_pub(), payload.as_bytes(), 0o644)?;
                return Ok(());
            }
        }
    } else if let Err(error) = fs::metadata(host_key) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error.to_string());
        }
    }

    let parent = host_key.parent().ok_or_else(|| "invalid host key path".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }

    let temp_name = parent.join(format!(
        ".ssh_host_ed25519_key.tmp-{}",
        std::process::id()
    ));
    let _ = fs::remove_file(&temp_name);
    let temp_pub_path = format!("{}.pub", temp_name.display());
    let temp_pub = Path::new(&temp_pub_path);

    run_command(
        "ssh-keygen",
        &[
            "-q",
            "-t",
            "ed25519",
            "-N",
            "",
            "-f",
            &temp_name.to_string_lossy(),
        ],
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp_name, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
        fs::set_permissions(temp_pub, fs::Permissions::from_mode(0o644))
            .map_err(|error| error.to_string())?;
    }
    fs::rename(temp_pub, paths.ssh_host_key_pub())
        .map_err(|error| error.to_string())?;
    fs::rename(&temp_name, host_key).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn validate_authorized_keys(content: &[u8]) -> Result<Vec<u8>, String> {
    let reader = BufReader::new(content);
    let mut accepted = Vec::new();
    let mut count = 0usize;
    for line in reader.lines() {
        let line = line.map_err(|error| error.to_string())?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.contains("PRIVATE KEY") {
            return Err("SSH provisioning file contains private-key material".into());
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2 {
            return Err("SSH provisioning file contains a malformed key".into());
        }
        match fields[0] {
            "ssh-ed25519" | "ecdsa-sha2-nistp256" | "ssh-rsa" => {}
            other => {
                return Err(format!(
                    "unsupported or option-prefixed SSH key type {other:?}"
                ));
            }
        }
        let output = command_input(
            &format!("{line}\n"),
            "ssh-keygen",
            &["-lf", "-"],
        )?;
        if fields[0] == "ssh-rsa" {
            let details: Vec<&str> = output.split_whitespace().collect();
            let bits = details
                .first()
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(0);
            if bits < 3072 {
                return Err("SSH RSA keys must contain at least 3072 bits".into());
            }
        }
        accepted.extend_from_slice(line.as_bytes());
        accepted.push(b'\n');
        count += 1;
    }
    if count == 0 {
        return Err("SSH provisioning file contains no supported public keys".into());
    }
    Ok(accepted)
}
