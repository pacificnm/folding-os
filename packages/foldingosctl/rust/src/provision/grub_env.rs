use std::collections::BTreeMap;
use std::fs;

use crate::fs_atomic::atomic_write;

const GRUB_ENV_BLOCK_SIZE: usize = 1024;

pub fn set_grub_env_var(path: &std::path::Path, key: &str, value: &str) -> Result<(), String> {
    let content = fs::read(path).map_err(|error| error.to_string())?;
    let mut vars = parse_grub_env_block(&content)?;
    vars.insert(key.to_string(), value.to_string());
    let updated = format_grub_env_block(&vars)?;
    atomic_write(path, &updated, 0o644)
}

pub fn clear_grub_next_entry(grub_env_path: &std::path::Path) -> Result<(), String> {
    let content = match fs::read(grub_env_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let mut vars = match parse_grub_env_block(&content) {
        Ok(vars) => vars,
        Err(_) => return Ok(()),
    };
    if vars.remove("next_entry").is_none() {
        return Ok(());
    }
    let updated = format_grub_env_block(&vars)?;
    atomic_write(grub_env_path, &updated, 0o644)
}

fn parse_grub_env_block(content: &[u8]) -> Result<BTreeMap<String, String>, String> {
    if content.len() != GRUB_ENV_BLOCK_SIZE {
        return Err(format!(
            "grub environment has invalid size {}",
            content.len()
        ));
    }
    let mut vars = BTreeMap::new();
    for line in String::from_utf8_lossy(content).lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if !key.is_empty() {
                vars.insert(key.to_string(), value.to_string());
            }
        }
    }
    Ok(vars)
}

fn format_grub_env_block(vars: &BTreeMap<String, String>) -> Result<Vec<u8>, String> {
    let mut block = vec![b'#'; GRUB_ENV_BLOCK_SIZE];
    let header = b"# GRUB Environment Block\n";
    if header.len() >= GRUB_ENV_BLOCK_SIZE {
        return Err("grub environment header is too large".into());
    }
    block[..header.len()].copy_from_slice(header);
    let mut offset = header.len();
    for (key, value) in vars {
        let line = format!("{key}={value}\n");
        if offset + line.len() > GRUB_ENV_BLOCK_SIZE {
            return Err("grub environment block overflow".into());
        }
        block[offset..offset + line.len()].copy_from_slice(line.as_bytes());
        offset += line.len();
    }
    Ok(block)
}
