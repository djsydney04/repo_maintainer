#![cfg(unix)]
use repo_maintainer::workflow;
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
#[test]
fn native_binary_creates_one_blocker_stays_quiet_on_repeat_and_closes_on_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    fs::write(bin.join("gh"),"#!/bin/sh\nif [ \"$1\" = issue ] && [ \"$2\" = list ]; then\n  /bin/cat \"$FIXTURE_ISSUES\"\nelse\n  printf '%s\\n' \"$*\" >> \"$FIXTURE_LOG\"\nfi\n").unwrap();
    fs::set_permissions(bin.join("gh"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        root.join(".maintainer.yml"),
        "sources: [https://docs.example.org/reference]\n",
    )
    .unwrap();
    let artifacts = root.join("artifacts");
    fs::create_dir(&artifacts).unwrap();
    workflow::write_json(&artifacts.join("report.json"),&json!({"status":"blocked","summary":"Incomplete","sources":[],"blockers":["Quota exhausted"]})).unwrap();
    let issues = root.join("issues.json");
    fs::write(&issues, "[]").unwrap();
    let log = root.join("calls.log");
    let invoke = |propose: &str, run: &str| {
        Command::new(env!("CARGO_BIN_EXE_repo-maintainer"))
            .args([
                "notify",
                "--repo",
                root.to_str().unwrap(),
                "--artifacts",
                artifacts.to_str().unwrap(),
            ])
            .env_clear()
            .env("PATH", &bin)
            .env("FIXTURE_ISSUES", &issues)
            .env("FIXTURE_LOG", &log)
            .env("GITHUB_REPOSITORY", "fixture/repository")
            .env("MAINTENANCE_OWNER", "fixture-owner")
            .env("RUN_URL", run)
            .env("PROPOSE_RESULT", propose)
            .env("VALIDATE_RESULT", "skipped")
            .env("PUBLISH_RESULT", "skipped")
            .output()
            .unwrap()
    };
    let result = invoke("failure", "https://github.invalid/run/1");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        fs::read_to_string(&log)
            .unwrap()
            .starts_with("issue create")
    );
    let body = fs::read_to_string(artifacts.join("issue-body.md")).unwrap();
    assert!(body.contains("@fixture-owner"));
    assert!(body.contains("Quota exhausted"));
    workflow::write_json(
        &issues,
        &json!([{"number":1,"title":"Repository maintenance needs help","body":body}]),
    )
    .unwrap();
    fs::write(&log, "").unwrap();
    assert!(
        invoke("failure", "https://github.invalid/run/2")
            .status
            .success()
    );
    assert!(fs::read_to_string(&log).unwrap().is_empty());
    assert!(
        invoke("skipped", "https://github.invalid/run/3")
            .status
            .success()
    );
    assert!(fs::read_to_string(&log).unwrap().is_empty());
    assert!(
        invoke("success", "https://github.invalid/run/4")
            .status
            .success()
    );
    assert!(
        fs::read_to_string(&log)
            .unwrap()
            .starts_with("issue close 1")
    );
    fs::write(&issues, "[]").unwrap();
    fs::write(&log, "").unwrap();
    assert!(
        invoke("success", "https://github.invalid/run/5")
            .status
            .success()
    );
    assert!(fs::read_to_string(&log).unwrap().is_empty());
}
