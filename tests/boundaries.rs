use repo_maintainer::{
    boundary,
    config::{Config, Environment},
    documentation, process,
    tools::{self, Tools},
};
use serde_json::json;
use std::{
    fs,
    time::{Duration, Instant},
};

fn config() -> Config {
    Config::from_str(
        "sources: [https://docs.example.org/reference]",
        &Environment::new(),
    )
    .unwrap()
}
#[test]
fn source_and_edit_guards_reject_traversal_hidden_paths_and_symlinks() {
    let temp = tempfile::tempdir().unwrap();
    let config = config();
    for path in [
        "../README.md",
        "/etc/passwd",
        "src/../README.md",
        "src//lib.rs",
        ".env",
        ".git/config",
        "src/.hidden/x.rs",
        "target/x.rs",
        "node_modules/x.rs",
    ] {
        assert!(
            boundary::guard(temp.path(), path, false, false, &config).is_err(),
            "{path}"
        );
    }
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "scripts/publish.rs",
        ".github/workflows/ci.yml",
    ] {
        assert!(!boundary::allowed(&config, path, false));
    }
    for path in [
        "README.md",
        "src/lib.rs",
        "crates/example/src/lib.rs",
        "docs/start.md",
    ] {
        assert!(boundary::allowed(&config, path, false));
    }
    assert!(!boundary::allowed(&config, "docs/deep/start.md", false));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(temp.path().join("outside"), temp.path().join("src")).unwrap();
        assert!(boundary::guard(temp.path(), "src/new.rs", true, false, &config).is_err());
        fs::write(temp.path().join("README.md"), "original").unwrap();
        fs::hard_link(temp.path().join("README.md"), temp.path().join("other.md")).unwrap();
        assert!(boundary::guard(temp.path(), "README.md", true, false, &config).is_err());
    }
}
#[test]
fn tool_arguments_cannot_become_commands_or_flags() {
    let config = config();
    for value in [
        json!({"check":"publish"}),
        json!({"check":"tests","crate":"; echo bad"}),
        json!({"check":"tests","extra_args":["--ignored"]}),
    ] {
        assert!(tools::check_command(&config, &value).is_err());
    }
    let command = tools::check_command(&config, &json!({"check":"tests"})).unwrap();
    assert!(command.iter().any(|s| s == "--offline"));
    assert!(command.iter().any(|s| s == "--locked"));
    let env = Environment::from([
        ("PATH".into(), "/bin".into()),
        ("OPENAI_API_KEY".into(), "sample-secret".into()),
        ("GH_TOKEN".into(), "sample-token".into()),
        ("RUSTC_WRAPPER".into(), "injected".into()),
        ("NODE_OPTIONS".into(), "injected".into()),
    ]);
    let clean = process::clean_environment_from(env.clone());
    assert_eq!(clean, Environment::from([("PATH".into(), "/bin".into())]));
    assert_eq!(
        process::redact("sample-secret sample-token".into(), &env),
        "[redacted] [redacted]"
    );
}
#[test]
fn editing_and_searching_stay_within_tools_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let config = config();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::create_dir_all(temp.path().join("target")).unwrap();
    fs::write(temp.path().join("src/lib.rs"), "first\nsecond\n").unwrap();
    fs::write(temp.path().join("target/private.rs"), "needle").unwrap();
    fs::write(temp.path().join(".env"), "needle").unwrap();
    let mut tools = Tools {
        root: temp.path(),
        config: &config,
        smoke: false,
        deadline: Instant::now() + Duration::from_secs(10),
        checks: vec![],
    };
    assert_eq!(
        tools
            .call("read", &json!({"path":"src/lib.rs","offset":2,"limit":1}))
            .unwrap(),
        "2: second"
    );
    assert!(
        tools
            .call(
                "edit",
                &json!({"path":"src/lib.rs","oldText":"missing","newText":"value"})
            )
            .is_err()
    );
    tools
        .call(
            "edit",
            &json!({"path":"src/lib.rs","oldText":"second","newText":"needle"}),
        )
        .unwrap();
    let found = tools.call("grep", &json!({"pattern":"needle"})).unwrap();
    assert!(found.contains("src/lib.rs:2"));
    assert!(!found.contains("private.rs"));
    assert!(!found.contains(".env"));
    for args in [
        json!({"path":"src/lib.rs","offset":0}),
        json!({"path":"src/lib.rs","limit":9000}),
        json!({"path":"src/lib.rs","extra":true}),
    ] {
        assert!(tools.call("read", &args).is_err());
    }
    assert!(
        tools
            .call("shell", &json!({"command":"echo unsafe"}))
            .is_err()
    );
    tools.deadline = Instant::now();
    assert!(tools.call("read", &json!({"path":"src/lib.rs"})).is_err());
}
#[test]
fn official_host_checks_reject_redirect_destinations_and_credentials() {
    let config = config();
    assert!(config.official("https://docs.example.org/reference"));
    for url in [
        "http://docs.example.org",
        "https://docs.example.org.evil.invalid",
        "https://user@docs.example.org",
        "https://docs.example.org:444/",
    ] {
        assert!(!config.official(url));
    }
    assert!(documentation::fetch(&config, "https://evil.invalid", Duration::from_secs(1)).is_err());
    assert_eq!(
        documentation::extract(
            "<nav>noise</nav><main><h1>API</h1><script>secret()</script><p>New field</p></main>"
        ),
        "API New field"
    );
}
#[test]
#[cfg(unix)]
fn subprocess_deadline_kills_descendants_holding_output_pipes() {
    let start = Instant::now();
    let result = process::execute(
        "sh",
        &["-c", "sleep 30 & wait"],
        std::path::Path::new("."),
        &process::clean_environment(),
        Duration::from_millis(100),
        &[],
    )
    .unwrap();
    assert_eq!(result.code, 124);
    assert!(start.elapsed() < Duration::from_secs(5));
    let result = process::execute(
        "sh",
        &["-c", "sleep 30 & exit 0"],
        std::path::Path::new("."),
        &process::clean_environment(),
        Duration::from_secs(1),
        &[],
    )
    .unwrap();
    assert_eq!(result.code, 0);
    assert!(start.elapsed() < Duration::from_secs(5));
}
