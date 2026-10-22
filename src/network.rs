//! Shared TLS trust for the worker's native HTTP clients.
use anyhow::{Context, Result, ensure};
use reqwest::{Certificate, blocking::ClientBuilder};
use std::{fs, path::Path};

pub fn with_ca_bundle(builder: ClientBuilder, path: &Path) -> Result<ClientBuilder> {
    ensure!(
        fs::metadata(path)?.len() <= 4_000_000,
        "CA bundle exceeds 4 MB"
    );
    let certificates =
        Certificate::from_pem_bundle(&fs::read(path)?).context("Invalid CA certificate bundle")?;
    ensure!(
        !certificates.is_empty(),
        "CA bundle contains no certificates"
    );
    Ok(certificates
        .into_iter()
        .fold(builder, |builder, cert| builder.add_root_certificate(cert)))
}

pub fn trust(builder: ClientBuilder) -> Result<ClientBuilder> {
    if let Some(path) = std::env::var_os("SSL_CERT_FILE").filter(|s| !s.is_empty()) {
        return with_ca_bundle(builder, Path::new(&path));
    }
    Ok(builder)
}
