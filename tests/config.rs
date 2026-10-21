use repo_maintainer::config::{self, Config, Environment};
use serde_json::json;
use std::{fs, process::Command};

const MINIMAL: &str = "sources: [https://docs.example.org/reference]\n";
#[test]
fn yaml_json_defaults_and_legacy_configuration() {
    let config = Config::from_str(MINIMAL, &Environment::new()).unwrap();
    assert_eq!(config.official_hosts, ["docs.example.org"]);
    assert_eq!(config.agent.provider, "groq");
    assert_eq!(config.agent.model.as_deref(), Some("openai/gpt-oss-120b"));
    let json = Config::from_str(
        &json!({"sources":["https://docs.example.org/reference"]}).to_string(),
        &Environment::new(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(config).unwrap(),
        serde_json::to_value(json).unwrap()
    );
    for text in [
        include_str!("../maintainer.example.yml"),
        include_str!("../examples/rust-sdks.json"),
    ] {
        assert!(Config::from_str(text, &Environment::new()).is_ok());
    }
}
#[test]
fn unsafe_configuration_is_rejected() {
    for extra in [
        "agent:\n  api_key: secret\n",
        "agent:\n  provider: groq\n  provider: google\n",
        "agent:\n  max_requests: 999\n",
        "agent:\n  max_requests: true\n",
        "agent:\n  max_requests: '16'\n",
        "agent:\n  allow_paid: 'false'\n",
        "instructions_file: ../secret\n",
        "editable:\n  source_dirs: [/etc]\n",
        "rust:\n  crates: ['--unsafe']\n",
        "version: 2\n",
        "version: true\n",
        "sources: [123]\n",
        "ci_workflow: |\n  bad\n  path\n",
    ] {
        assert!(
            Config::from_str(&format!("{MINIMAL}{extra}"), &Environment::new()).is_err(),
            "{extra}"
        );
    }
    for url in [
        "http://docs.example.org",
        "https://user:pass@docs.example.org",
        "https://docs.example.org:444/",
        "https://docs.example.org/ bad",
    ] {
        assert!(
            Config::from_str(&json!({"sources":[url]}).to_string(), &Environment::new()).is_err()
        );
    }
    assert!(Config::from_str(r#"{"sources":["https://docs.example.org"],"agent":{"provider":"groq","provider":"google"}}"#,&Environment::new()).is_err());
}
#[test]
fn yaml_alias_budgets_and_unsupported_tags_fail_closed() {
    let yaml = format!(
        "sources:\n  - &source https://docs.example.org/reference\n{}",
        "  - *source\n".repeat(51)
    );
    assert!(Config::from_str(&yaml, &Environment::new()).is_err());
    for suffix in [
        "instructions: !include /etc/passwd\n",
        "agent: { <<: {provider: groq} }\n",
    ] {
        assert!(Config::from_str(&format!("{MINIMAL}{suffix}"), &Environment::new()).is_err());
    }
}
#[test]
fn provider_overrides_preserve_billing_opt_in() {
    for (provider, model) in [("openai", "gpt-5.4"), ("anthropic", "claude-sonnet-4-6")] {
        assert!(
            Config::from_str(
                &format!("{MINIMAL}agent:\n  provider: {provider}\n"),
                &Environment::new()
            )
            .is_err()
        );
        let config = Config::from_str(
            &format!("{MINIMAL}agent:\n  provider: {provider}\n  allow_paid: true\n"),
            &Environment::new(),
        )
        .unwrap();
        assert_eq!(config.agent.model.as_deref(), Some(model));
    }
    let env = Environment::from([
        ("MAINTENANCE_PROVIDER".into(), "anthropic".into()),
        ("MAINTENANCE_ALLOW_PAID".into(), "true".into()),
    ]);
    let config = Config::from_str(
        &format!("{MINIMAL}agent:\n  provider: groq\n  model: openai/gpt-oss-120b\n"),
        &env,
    )
    .unwrap();
    assert_eq!(config.agent.model.as_deref(), Some("claude-sonnet-4-6"));
    assert!(
        Config::from_str(
            MINIMAL,
            &Environment::from([("MAINTENANCE_ALLOW_PAID".into(), "yes".into())])
        )
        .is_err()
    );
    let alias = Config::from_str(
        &format!("{MINIMAL}agent:\n  provider: oai\n  allow_paid: true\n"),
        &Environment::new(),
    )
    .unwrap();
    assert_eq!(alias.agent.provider, "openai");
}
#[test]
fn binary_init_and_check_need_no_api_key_and_keep_existing_config() {
    let temp = tempfile::tempdir().unwrap();
    let invoke = |command| {
        Command::new(env!("CARGO_BIN_EXE_repo-maintainer"))
            .args([command, "--repo", temp.path().to_str().unwrap()])
            .env_remove("MAINTENANCE_CONFIG")
            .env("MAINTENANCE_PROVIDER", "")
            .env("MAINTENANCE_MODEL", "")
            .env("MAINTENANCE_ALLOW_PAID", "")
            .output()
            .unwrap()
    };
    assert!(invoke("init").status.success());
    fs::write(temp.path().join(".maintainer.yml"), MINIMAL).unwrap();
    assert!(invoke("init").status.success());
    assert_eq!(
        fs::read_to_string(temp.path().join(".maintainer.yml")).unwrap(),
        MINIMAL
    );
    assert_eq!(
        config::find(temp.path(), None).unwrap(),
        temp.path().join(".maintainer.yml").canonicalize().unwrap()
    );
    let result = invoke("check");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("No API request made"));
}
