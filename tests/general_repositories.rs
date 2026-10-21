#![cfg(unix)]
use repo_maintainer::{
    config::{Config, Environment},
    process,
    tools::{self, Tools},
    validation,
};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

fn git(root: &Path, args: &[&str]) {
    let result = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
fn script(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}
struct Fixture {
    _temp: tempfile::TempDir,
    repo: PathBuf,
    output: PathBuf,
    config: PathBuf,
    fake: PathBuf,
}
impl Fixture {
    fn new(pause: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/value.txt"), "broken\n").unwrap();
        fs::write(
            repo.join("README.md"),
            "A repository with no Cargo manifest.\n",
        )
        .unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.name", "Fixture"]);
        git(&repo, &["config", "user.email", "fixture@example.invalid"]);
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "fixture"]);
        let fake = temp.path().join("codex");
        script(
            &fake,
            &format!(
                r#"#!/bin/sh
set -eu
test "${{GH_TOKEN:-}}" = ''
test "${{GROQ_API_KEY:-}}" = ''
test "${{CODEX_API_KEY:-}}" = 'fixture-key'
output=''; schema=''; previous=''
for argument in "$@"; do
  if [ "$previous" = '--output-last-message' ]; then output="$argument"; fi
  if [ "$previous" = '--output-schema' ]; then schema="$argument"; fi
  previous="$argument"
done
test -f "$schema"
grep -q 'Fix the broken value' </dev/stdin
printf 'fixed\n' > src/value.txt
{}
printf '%s\n' '{{"status":"changed","summary":"Repaired fixture-key","sources":[],"blockers":[]}}' > "$output"
"#,
                if pause { "sleep 5" } else { ":" }
            ),
        );
        let config = temp.path().join("config.yml");
        fs::write(&config, json!({"instructions":"Fix the broken value", "agent":{"runner":"codex", "allow_paid":true,"timeout_seconds":if pause {1}else{30},"codex":{"binary":fake}},"editable":{"source_dirs":["src"],"extensions":[".txt"]},"validation":{"profile":"custom","checks":[{"name":"contract","command":["sh","-c","test \"$(cat src/value.txt)\" = fixed"]}]}}).to_string()).unwrap();
        let output = temp.path().join("proposal");
        Self {
            _temp: temp,
            repo,
            output,
            config,
            fake,
        }
    }
    fn invoke(&self, command: &str, source: Option<&Path>) -> std::process::Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_repo-maintainer"));
        cmd.args([
            command,
            "--repo",
            self.repo.to_str().unwrap(),
            "--config",
            self.config.to_str().unwrap(),
            "--output",
            self.output.to_str().unwrap(),
        ])
        .env("CODEX_API_KEY", "fixture-key")
        .env("GH_TOKEN", "must-not-reach-codex")
        .env("GROQ_API_KEY", "must-not-reach-codex")
        .env("MAINTENANCE_RUNNER", "")
        .env("MAINTENANCE_PROVIDER", "")
        .env("MAINTENANCE_MODEL", "")
        .env("MAINTENANCE_ALLOW_PAID", "");
        if let Some(source) = source {
            cmd.args(["--source", source.to_str().unwrap()]);
        }
        cmd.output().unwrap()
    }
}
#[test]
fn codex_repairs_a_non_rust_repo_then_independent_checks_validate_exact_patch() {
    let f = Fixture::new(false);
    let result = f.invoke("run", None);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(f.repo.join("src/value.txt")).unwrap(),
        "broken\n"
    );
    let report: Value =
        serde_json::from_slice(&fs::read(f.output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["status"], "changed");
    assert_eq!(report["summary"], "Repaired [redacted]");
    let fresh = f._temp.path().join("fresh");
    git(
        f._temp.path(),
        &[
            "clone",
            "--quiet",
            f.repo.to_str().unwrap(),
            fresh.to_str().unwrap(),
        ],
    );
    assert!(f.invoke("apply", Some(&fresh)).status.success());
    let result = f.invoke("validate", Some(&fresh));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let receipt: Value =
        serde_json::from_slice(&fs::read(f.output.join("validation.json")).unwrap()).unwrap();
    assert_eq!(
        receipt["checks"],
        json!([{"check":"contract","passed":true}])
    );
    let original_config = fs::read_to_string(&f.config).unwrap();
    let mut changed_config: Value = serde_json::from_str(&original_config).unwrap();
    changed_config["instructions"] = json!("A different task");
    fs::write(&f.config, changed_config.to_string()).unwrap();
    let mismatch = f.invoke("publish", Some(&fresh));
    assert!(!mismatch.status.success());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("receipt does not match"));
    fs::write(&f.config, original_config).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(f.output.join("proposal.patch"))
        .map(|mut file| {
            use std::io::Write;
            file.write_all(b"\n").unwrap();
        })
        .unwrap();
    let result = f.invoke("publish", Some(&fresh));
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("receipt does not match"));
}
#[test]
fn codex_timeout_preserves_partial_proposal_without_publishing() {
    let f = Fixture::new(true);
    let result = f.invoke("run", None);
    assert!(!result.status.success());
    let report: Value =
        serde_json::from_slice(&fs::read(f.output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["status"], "blocked");
    assert!(report["blockers"].to_string().contains("124"));
    assert!(
        fs::read_to_string(f.output.join("proposal.patch"))
            .unwrap()
            .contains("+fixed")
    );
}
#[test]
fn a_failed_validation_attempt_removes_any_earlier_receipt() {
    let f = Fixture::new(false);
    assert!(f.invoke("run", None).status.success());
    let fresh = f._temp.path().join("fresh");
    git(
        f._temp.path(),
        &[
            "clone",
            "--quiet",
            f.repo.to_str().unwrap(),
            fresh.to_str().unwrap(),
        ],
    );
    assert!(f.invoke("apply", Some(&fresh)).status.success());
    assert!(f.invoke("validate", Some(&fresh)).status.success());
    assert!(f.output.join("validation.json").exists());
    fs::write(fresh.join("src/value.txt"), "unexpected change\n").unwrap();
    assert!(!f.invoke("validate", Some(&fresh)).status.success());
    assert!(!f.output.join("validation.json").exists());
    assert!(!f.invoke("publish", Some(&fresh)).status.success());
}
#[test]
fn checks_are_named_bounded_and_independent_of_rust() {
    let f = Fixture::new(false);
    let config = Config::load(&f.config, &Environment::new()).unwrap();
    assert!(
        !tools::definitions(&config)
            .iter()
            .any(|tool| tool.name == "check_rust")
    );
    let root = f.repo.canonicalize().unwrap();
    let mut tools = Tools {
        root: &root,
        config: &config,
        smoke: false,
        deadline: Instant::now() + Duration::from_secs(10),
        checks: vec![],
    };
    assert!(
        tools
            .call("check", &json!({"name":"unconfigured-command"}))
            .is_err()
    );
    assert!(
        tools
            .call("check", &json!({"name":"contract","command":"arbitrary"}))
            .is_err()
    );
    assert!(
        tools
            .call("check", &json!({"name":"contract"}))
            .unwrap()
            .contains("Exit code: 1")
    );
    tools
        .call(
            "edit",
            &json!({"path":"src/value.txt","oldText":"broken","newText":"fixed"}),
        )
        .unwrap();
    assert!(
        tools
            .call("check", &json!({"name":"contract"}))
            .unwrap()
            .contains("Exit code: 0")
    );
    assert!(validation::validate(&config, &root).is_ok());
    assert!(f.invoke("validate", None).status.success());
    assert!(f.fake.exists());
    // The existing clean environment rules still remove wrappers and credentials.
    assert!(
        !process::clean_environment_from(Environment::from([(
            "GH_TOKEN".into(),
            "private".into()
        )]))
        .contains_key("GH_TOKEN")
    );
}
#[test]
fn invalid_runners_and_custom_checks_fail_before_execution() {
    for value in [
        json!({"agent":{"runner":"unknown"}}),
        json!({"agent":{"runner":"codex"}}),
        json!({"agent":{"timeout_seconds":0}}),
        json!({"validation":{"profile":"custom"}}),
        json!({"validation":{"profile":"custom","checks":[{"name":"bad","command":[]}]}}),
        json!({"validation":{"profile":"custom","checks":[{"name":"bad","command":["echo"],"working_directory":"../escape"}]}}),
        json!({"validation":{"profile":"custom","checks":[{"name":"bad","command":["echo"],"timeout_seconds":0}]}}),
    ] {
        assert!(
            Config::from_str(&value.to_string(), &Environment::new()).is_err(),
            "{value}"
        );
    }
}
