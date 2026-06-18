use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ureq::Agent;

use crate::inspect::{hash_file_at_path, ToolsAssignment};

use super::replace::verify_tools_executable_elf;

pub fn tools_http_agent() -> Agent {
    ureq::AgentBuilder::new().redirects(0).build()
}

pub fn tools_staged_artifact_path(downloads_dir: &Path, assignment: &ToolsAssignment) -> PathBuf {
    downloads_dir.join(format!("foldingosctl_{}", assignment.tools_version))
}

pub fn download_and_stage_tools_binary(
    downloads_dir: &Path,
    assignment: &ToolsAssignment,
    agent: &Agent,
) -> Result<PathBuf, String> {
    fs::create_dir_all(downloads_dir)
        .map_err(|error| format!("create tools downloads directory: {error}"))?;

    let staged_path = tools_staged_artifact_path(downloads_dir, assignment);
    let partial_path = PathBuf::from(format!("{}.partial", staged_path.display()));

    if let Err(error) = fs::remove_file(&partial_path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(format!("remove stale partial download: {error}"));
        }
    }
    if let Err(error) = fs::remove_file(&staged_path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(format!("remove stale staged artifact: {error}"));
        }
    }

    if let Err(error) = download_tools_binary(assignment, &partial_path, agent) {
        let _ = fs::remove_file(&partial_path);
        return Err(error);
    }
    if let Err(error) = verify_tools_artifact_file(&partial_path, assignment) {
        let _ = fs::remove_file(&partial_path);
        return Err(error);
    }
    if let Err(error) = fs::rename(&partial_path, &staged_path) {
        let _ = fs::remove_file(&partial_path);
        return Err(format!("stage verified tools artifact: {error}"));
    }
    Ok(staged_path)
}

pub fn download_tools_binary(
    assignment: &ToolsAssignment,
    destination: &Path,
    agent: &Agent,
) -> Result<(), String> {
    let response = agent
        .get(&assignment.artifact_url)
        .call()
        .map_err(|error| format!("download foldingosctl artifact: {error}"))?;

    if response.get_url() != assignment.artifact_url {
        return Err("foldingosctl artifact download resolved to an unexpected URL".into());
    }
    if response.status() != 200 {
        return Err(format!(
            "foldingosctl artifact download failed with status {}",
            response.status()
        ));
    }

    let mut file = File::create(destination).map_err(|error| format!("open partial download: {error}"))?;
    let mut reader = response.into_reader();
    let mut buffer = [0_u8; 8192];
    let mut written = 0_i64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("write partial download: {error}"))?;
        if read == 0 {
            break;
        }
        written += read as i64;
        if written > assignment.artifact_size {
            return Err(format!(
                "foldingosctl artifact download exceeded expected size {} bytes",
                assignment.artifact_size
            ));
        }
        file.write_all(&buffer[..read])
            .map_err(|error| format!("write partial download: {error}"))?;
    }
    if written != assignment.artifact_size {
        return Err(format!(
            "foldingosctl artifact download size {written} does not match expected size {}",
            assignment.artifact_size
        ));
    }
    file.sync_all()
        .map_err(|error| format!("sync partial download: {error}"))?;
    Ok(())
}

pub fn verify_tools_artifact_file(path: &Path, assignment: &ToolsAssignment) -> Result<(), String> {
    let digest = hash_file_at_path(path, assignment.artifact_size)?;
    if digest != assignment.sha256 {
        return Err("foldingosctl artifact SHA-256 digest does not match approved assignment".into());
    }
    verify_tools_executable_elf(path)
}

#[cfg(test)]
pub(crate) fn write_staged_tools_binary(
    downloads_dir: &Path,
    assignment: &ToolsAssignment,
    artifact: &[u8],
) -> Result<PathBuf, String> {
    fs::create_dir_all(downloads_dir)
        .map_err(|error| format!("create tools downloads directory: {error}"))?;
    let staged_path = tools_staged_artifact_path(downloads_dir, assignment);
    let partial_path = PathBuf::from(format!("{}.partial", staged_path.display()));
    let _ = fs::remove_file(&partial_path);
    let _ = fs::remove_file(&staged_path);
    fs::write(&partial_path, artifact).map_err(|error| error.to_string())?;
    verify_tools_artifact_file(&partial_path, assignment)?;
    fs::rename(&partial_path, &staged_path)
        .map_err(|error| format!("stage verified tools artifact: {error}"))?;
    Ok(staged_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn test_elf_bytes() -> Vec<u8> {
        vec![
            0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 62, 0,
        ]
    }

    fn assignment_for(artifact: &[u8]) -> ToolsAssignment {
        ToolsAssignment {
            schema_version: 1,
            tools_version: "0.2.0".into(),
            artifact_url: "https://packages.folding-os.com/foldingos-tools/0.2.0/foldingosctl-x86_64"
                .into(),
            artifact_size: artifact.len() as i64,
            sha256: format!("{:x}", Sha256::digest(artifact)),
        }
    }

    #[test]
    fn verify_tools_artifact_rejects_bad_hash() {
        let artifact = b"not-the-approved-binary";
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-tools-verify-{}",
            std::process::id()
        ));
        let path = root.join("artifact");
        fs::create_dir_all(&root).unwrap();
        fs::write(&path, artifact).unwrap();
        let assignment = ToolsAssignment {
            schema_version: 1,
            tools_version: "0.2.0".into(),
            artifact_url: "https://packages.folding-os.com/foldingos-tools/0.2.0/foldingosctl-x86_64"
                .into(),
            artifact_size: artifact.len() as i64,
            sha256: "9022c393630e11d5cec5794ac77281671c7b0d634d630c92d95ad6de22d2151a".into(),
        };
        assert!(verify_tools_artifact_file(&path, &assignment).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn download_and_stage_tools_binary_fetches_local_artifact() {
        let artifact = test_elf_bytes();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let artifact_for_server = artifact.clone();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    artifact_for_server.len()
                );
                use std::io::Write;
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&artifact_for_server);
            }
        });

        let mut assignment = assignment_for(&artifact);
        assignment.artifact_url = format!("http://{addr}/foldingosctl-x86_64");

        let root = std::env::temp_dir().join(format!(
            "foldingosctl-tools-download-{}",
            std::process::id()
        ));
        let downloads_dir = root.join(".downloads");
        let staged_path =
            download_and_stage_tools_binary(&downloads_dir, &assignment, &tools_http_agent())
                .unwrap();
        assert!(staged_path.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_staged_tools_binary_stages_verified_artifact() {
        let artifact = test_elf_bytes();
        let assignment = assignment_for(&artifact);
        let root = std::env::temp_dir().join(format!(
            "foldingosctl-tools-stage-{}",
            std::process::id()
        ));
        let downloads_dir = root.join(".downloads");
        let staged_path =
            write_staged_tools_binary(&downloads_dir, &assignment, &artifact).unwrap();
        assert!(staged_path.exists());
        let _ = fs::remove_dir_all(root);
    }
}
