use repo_maintainer::{config::Environment, network, process};

#[test]
fn child_network_settings_survive_without_model_or_publisher_credentials() {
    let environment = Environment::from([
        (
            "HTTPS_PROXY".into(),
            "http://user:secret@proxy.internal:8080".into(),
        ),
        (
            "https_proxy".into(),
            "http://user:secret@proxy.internal:8080".into(),
        ),
        ("NO_PROXY".into(), "database,localhost".into()),
        ("SSL_CERT_FILE".into(), "/etc/worker/ca-bundle.pem".into()),
        (
            "NODE_EXTRA_CA_CERTS".into(),
            "/etc/worker/ca-bundle.pem".into(),
        ),
        ("GIT_SSL_CAINFO".into(), "/etc/worker/ca-bundle.pem".into()),
        ("CODEX_API_KEY".into(), "model-secret".into()),
        ("GH_TOKEN".into(), "publisher-secret".into()),
        ("RUSTC_WRAPPER".into(), "/untrusted/wrapper".into()),
    ]);
    let child = process::clean_environment_from(environment.clone());
    for name in [
        "HTTPS_PROXY",
        "https_proxy",
        "NO_PROXY",
        "SSL_CERT_FILE",
        "NODE_EXTRA_CA_CERTS",
        "GIT_SSL_CAINFO",
    ] {
        assert_eq!(child.get(name), environment.get(name));
    }
    for name in ["CODEX_API_KEY", "GH_TOKEN", "RUSTC_WRAPPER"] {
        assert!(!child.contains_key(name));
    }
    let text = process::redact(
        "Failed via http://user:secret@proxy.internal:8080".into(),
        &environment,
    );
    assert!(!text.contains("secret@"));
}

#[test]
fn invalid_or_oversized_ca_bundles_fail_without_disabling_tls_verification() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "not a CA certificate").unwrap();
    assert!(network::with_ca_bundle(reqwest::blocking::Client::builder(), temp.path()).is_err());
    temp.as_file().set_len(4_000_001).unwrap();
    assert!(network::with_ca_bundle(reqwest::blocking::Client::builder(), temp.path()).is_err());
}
