use std::fs;
use std::net::Ipv4Addr;

use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SanType};
use rustls::pki_types::CertificateDer;

use crate::config_host::read_hostname;
use crate::foldops::util::file_exists;
use crate::fs_atomic::atomic_write;
use crate::paths::AppliancePaths;

pub fn ensure_foldops_tls_material(paths: &AppliancePaths) -> Result<(), String> {
    let cert_path = paths.foldops_tls_dir.join("cert.pem");
    let key_path = paths.foldops_tls_dir.join("key.pem");
    let ca_path = paths.foldops_tls_dir.join("ca.pem");
    if file_exists(&cert_path) && file_exists(&key_path) && file_exists(&ca_path) {
        return Ok(());
    }
    let hostname = read_hostname(paths)?;
    generate_foldops_self_signed_tls(paths, &hostname)
}

fn generate_foldops_self_signed_tls(paths: &AppliancePaths, hostname: &str) -> Result<(), String> {
    fs::create_dir_all(&paths.foldops_tls_dir)
        .map_err(|error| format!("create TLS directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&paths.foldops_tls_dir, fs::Permissions::from_mode(0o750)).ok();
    }

    let mut params = CertificateParams::new(vec![hostname.to_string()])
        .map_err(|error| format!("generate TLS certificate params: {error}"))?;
    params
        .subject_alt_names
        .push(SanType::IpAddress(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, hostname);
    params.distinguished_name = distinguished_name;

    let key_pair = KeyPair::generate().map_err(|error| format!("create TLS certificate: {error}"))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|error| format!("create TLS certificate: {error}"))?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    atomic_write(&paths.foldops_tls_dir.join("cert.pem"), cert_pem.as_bytes(), 0o644)?;
    atomic_write(&paths.foldops_tls_dir.join("key.pem"), key_pem.as_bytes(), 0o600)?;
    atomic_write(&paths.foldops_tls_dir.join("ca.pem"), cert_pem.as_bytes(), 0o644)?;
    Ok(())
}

pub fn load_foldops_tls_certificate(
    paths: &AppliancePaths,
) -> Result<(String, String), String> {
    let cert_path = paths.foldops_tls_dir.join("cert.pem");
    let key_path = paths.foldops_tls_dir.join("key.pem");
    for path in [&cert_path, &key_path] {
        let metadata = fs::metadata(path).map_err(|error| {
            format!("TLS material is missing at {}: {error}", path.display())
        })?;
        if metadata.is_dir() {
            return Err(format!("TLS material path is not a file: {}", path.display()));
        }
    }
    Ok((
        cert_path.display().to_string(),
        key_path.display().to_string(),
    ))
}

pub fn validate_foldops_tls_ready(paths: &AppliancePaths) -> Result<(), String> {
    if !crate::foldops::provision::foldops_provisioned(paths) {
        return Err("FoldOps is not provisioned".into());
    }
    load_foldops_tls_certificate(paths).map(|_| ())
}

pub fn load_rustls_config(
    paths: &AppliancePaths,
) -> Result<rustls::ServerConfig, String> {
    let cert_path = paths.foldops_tls_dir.join("cert.pem");
    let key_path = paths.foldops_tls_dir.join("key.pem");
    let cert_bytes = fs::read(&cert_path).map_err(|error| error.to_string())?;
    let key_bytes = fs::read(&key_path).map_err(|error| error.to_string())?;
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_bytes.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(CertificateDer::from)
        .collect();
    let key = rustls_pemfile::private_key(&mut key_bytes.as_slice())
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "TLS private key is missing".to_string())?;
    rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|error| error.to_string())
}
