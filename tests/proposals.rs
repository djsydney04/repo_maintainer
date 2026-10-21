use repo_maintainer::{
    config::{Config, Environment},
    process, workflow,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    source: PathBuf,
    artifacts: PathBuf,
    raw: PathBuf,
    config: Config,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let source = root.join("source");
        let artifacts = root.join("artifacts");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&artifacts).unwrap();
        let git = |args: &[&str]| process::run("git", args, &source, false).unwrap();
        git(&["init", "-q"]);
        git(&["config", "user.email", "tests@example.invalid"]);
        git(&["config", "user.name", "Maintenance tests"]);
        git(&["config", "core.autocrlf", "false"]);
        fs::write(source.join("README.md"), "Initial documentation.\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "initial"]);
        let sha = git(&["rev-parse", "HEAD"]);
        workflow::write_json(
            &artifacts.join("request.json"),
            &json!({"sha":sha,"branch":"dylan/repo-maintenance-123-1","pr":null}),
        )
        .unwrap();
        let raw = root.join("result.json");
        let config = Config::from_str(
            "sources: [https://docs.example.org/reference]",
            &Environment::new(),
        )
        .unwrap();
        let fixture = Self {
            _temp: temp,
            root,
            source,
            artifacts,
            raw,
            config,
        };
        fixture.response("changed");
        fixture
    }
    fn git(&self, args: &[&str]) -> String {
        process::run("git", args, &self.source, false).unwrap()
    }
    fn response(&self, status: &str) {
        workflow::write_json(&self.raw,&json!({"response":json!({"status":status,"summary":"Preserve documented compatibility.","sources":["https://docs.example.org/reference"],"blockers":[]}).to_string()})).unwrap();
    }
    fn collect(&self, code: i32) -> workflow::Report {
        workflow::collect(&self.config, &self.source, &self.artifacts, &self.raw, code).unwrap()
    }
    fn report(&self) -> Value {
        workflow::read_json(&self.artifacts.join("report.json")).unwrap()
    }
    fn write(&self, name: &str, value: &str) {
        let path = self.source.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
}
#[test]
fn completed_change_survives_transfer_to_fresh_checkout() {
    let f = Fixture::new();
    f.write(
        "crates/example/tests/compatibility.rs",
        "// Regression fixture.\n",
    );
    assert_eq!(f.collect(0).status, "changed");
    let fresh = f.root.join("fresh");
    process::run(
        "git",
        &[
            "clone",
            "--quiet",
            f.source.to_str().unwrap(),
            fresh.to_str().unwrap(),
        ],
        Path::new("."),
        false,
    )
    .unwrap();
    workflow::apply(&f.config, &fresh, &f.artifacts).unwrap();
    assert_eq!(
        fs::read_to_string(fresh.join("crates/example/tests/compatibility.rs")).unwrap(),
        "// Regression fixture.\n"
    );
}
#[test]
fn unchanged_has_no_patch_and_cannot_hide_edits() {
    let f = Fixture::new();
    f.response("unchanged");
    assert_eq!(f.collect(0).status, "unchanged");
    assert!(
        fs::read(f.artifacts.join("proposal.patch"))
            .unwrap()
            .is_empty()
    );
    f.write("README.md", "Changed.\n");
    assert_eq!(f.collect(0).status, "blocked");
    assert_eq!(f.report()["status"], "blocked");
}
#[test]
fn protected_paths_symlinks_executables_and_file_count_are_rejected() {
    for name in [
        ".github/workflows/release.yml",
        "Cargo.toml",
        "Cargo.lock",
        "scripts/publish.py",
    ] {
        let f = Fixture::new();
        f.write(name, "Unauthorized change.\n");
        assert_eq!(f.collect(0).status, "blocked", "{name}");
        f.git(&["reset", "--hard", "HEAD"]);
        f.git(&["clean", "-fd"]);
        assert!(workflow::apply(&f.config, &f.source, &f.artifacts).is_err());
    }
    let f = Fixture::new();
    for i in 0..26 {
        f.write(&format!("src/file{i}.rs"), "// new\n");
    }
    assert_eq!(f.collect(0).status, "blocked");
    #[cfg(unix)]
    {
        let f = Fixture::new();
        fs::create_dir_all(f.source.join("src")).unwrap();
        std::os::unix::fs::symlink("../README.md", f.source.join("src/lib.rs")).unwrap();
        assert_eq!(f.collect(0).status, "blocked");
        let f = Fixture::new();
        f.git(&["config", "core.filemode", "true"]);
        f.git(&["add", "--chmod=+x", "README.md"]);
        assert!(workflow::staged(&f.config, &f.source).is_err());
    }
}
#[test]
fn failed_agent_preserves_partial_work_and_oversized_patch_does_not_reuse_old_artifact() {
    let f = Fixture::new();
    f.write("README.md", "Partial work.\n");
    assert_eq!(f.collect(53).status, "blocked");
    assert!(
        fs::read_to_string(f.artifacts.join("proposal.patch"))
            .unwrap()
            .contains("Partial work")
    );
    f.write("README.md", &"z".repeat(510_000));
    assert_eq!(f.collect(0).status, "blocked");
    assert!(
        fs::read(f.artifacts.join("proposal.patch"))
            .unwrap()
            .is_empty()
    );
}
#[test]
fn patch_must_match_original_revision_branch_and_clean_checkout() {
    let f = Fixture::new();
    f.write("README.md", "Changed.\n");
    f.collect(0);
    assert!(workflow::apply(&f.config, &f.source, &f.artifacts).is_err());
    f.git(&["commit", "-qm", "concurrent change"]);
    assert!(
        workflow::apply(&f.config, &f.source, &f.artifacts)
            .unwrap_err()
            .to_string()
            .contains("original source revision")
    );
    f.git(&["reset", "--hard", "HEAD~1"]);
    let mut request = workflow::read_json(&f.artifacts.join("request.json")).unwrap();
    request["branch"] = json!("main");
    workflow::write_json(&f.artifacts.join("request.json"), &request).unwrap();
    assert!(
        workflow::apply(&f.config, &f.source, &f.artifacts)
            .unwrap_err()
            .to_string()
            .contains("maintenance branch")
    );
}
#[test]
fn incomplete_reports_and_unofficial_evidence_fail_closed() {
    let f = Fixture::new();
    for raw in [
        "[]",
        "{}",
        r#"{"response":"[]"}"#,
        r#"{"response":"not json"}"#,
    ] {
        assert!(workflow::parse_report(&f.config, raw).is_err());
    }
    for sources in [
        json!([]),
        json!(["https://evil.invalid/"]),
        json!(["http://docs.example.org/"]),
    ] {
        let report = json!({"response":json!({"status":"changed","summary":"Fix.","sources":sources,"blockers":[]}).to_string()});
        assert!(workflow::parse_report(&f.config, &report.to_string()).is_err());
    }
    let report = json!({"response":json!({"status":"unchanged","summary":"Fix.","sources":[],"blockers":["still broken"]}).to_string()});
    assert!(workflow::parse_report(&f.config, &report.to_string()).is_err());
}
#[test]
fn no_op_and_repeated_blocker_notifications_are_quiet() {
    let stages = [
        ("propose", "success".into()),
        ("validate", "skipped".into()),
        ("publish", "skipped".into()),
    ];
    assert!(workflow::notification_reason(&stages, &[], false).is_none());
    let stages = [
        ("propose", "failure".into()),
        ("validate", "skipped".into()),
        ("publish", "skipped".into()),
    ];
    let first = workflow::notification_reason(&stages, &["Quota exhausted".into()], true);
    assert_eq!(
        first,
        workflow::notification_reason(&stages, &["Quota exhausted".into()], true)
    );
}
