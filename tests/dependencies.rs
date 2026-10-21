#![cfg(unix)]
use repo_maintainer::{config::Environment, dependencies};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn args(command: &Command) -> Vec<String> {
    command
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn preview_is_model_free_and_credentials_are_passed_by_name() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config with spaces.json");
    fs::write(&path, dependencies::TEMPLATE).unwrap();
    let env = Environment::from([
        ("RENOVATE_TOKEN".into(), "private-token".into()),
        ("OPENAI_API_KEY".into(), "model-key".into()),
        ("RENOVATE_CONFIG".into(), "unwanted override".into()),
        ("DOCKER_CONTEXT".into(), "my-server".into()),
    ]);
    let command = dependencies::command(&path, &env, false, false).unwrap();
    let arguments = args(&command);
    assert!(arguments.contains(&"--dry-run=full".into()));
    assert!(arguments.contains(&"--autodiscover=false".into()));
    assert!(arguments.contains(&dependencies::IMAGE.trim().to_owned()));
    assert!(
        arguments
            .iter()
            .any(|s| s.contains("config with spaces.json") && s.ends_with(",readonly"))
    );
    assert!(!arguments.join(" ").contains("private-token"));
    assert!(!arguments.join(" ").contains("model-key"));
    assert!(!arguments.join(" ").contains("docker.sock"));
    let environment: std::collections::BTreeMap<_, _> = command.get_envs().collect();
    assert!(environment.contains_key(std::ffi::OsStr::new("RENOVATE_TOKEN")));
    assert!(environment.contains_key(std::ffi::OsStr::new("DOCKER_CONTEXT")));
    assert!(!environment.contains_key(std::ffi::OsStr::new("OPENAI_API_KEY")));
    assert!(!environment.contains_key(std::ffi::OsStr::new("RENOVATE_CONFIG")));
}

#[test]
fn live_and_validation_modes_are_explicit() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    let env = Environment::new();
    let live = args(&dependencies::command(temp.path(), &env, false, true).unwrap());
    assert!(live.contains(&"renovate".into()));
    assert!(!live.contains(&"--dry-run=full".into()));
    let check = args(&dependencies::command(temp.path(), &env, true, false).unwrap());
    assert!(check.contains(&"renovate-config-validator".into()));
    assert!(!check.contains(&"renovate".into()));
    assert!(dependencies::command(temp.path(), &env, true, true).is_err());
}

#[test]
fn missing_config_unsafe_mount_paths_and_unpinned_images_fail_before_docker() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        dependencies::command(
            &root.path().join("missing"),
            &Environment::new(),
            false,
            false
        )
        .is_err()
    );
    let path = root.path().join("comma,config.json");
    fs::write(&path, b"{}").unwrap();
    assert!(dependencies::command(&path, &Environment::new(), false, false).is_err());
    let path = root.path().join("valid.json");
    fs::write(&path, b"{}").unwrap();
    for image in [
        "renovate/renovate:latest",
        "--privileged",
        "renovate@sha256:abcd",
    ] {
        let env = Environment::from([("MAINTENANCE_RENOVATE_IMAGE".into(), image.into())]);
        assert!(dependencies::command(&path, &env, false, false).is_err());
    }
}

struct Fixture {
    root: tempfile::TempDir,
    config: std::path::PathBuf,
    log: std::path::PathBuf,
}
impl Fixture {
    fn new(validator_code: i32, runner_code: i32) -> Self {
        let root = tempfile::tempdir().unwrap();
        let log = root.path().join("docker.log");
        let bin = root.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let quote = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "'\\''"));
        fs::write(bin.join("docker"), format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\ncase \" $* \" in *' renovate-config-validator '*) exit {validator_code};; esac\nexit {runner_code}\n", quote(&log)
        )).unwrap();
        fs::set_permissions(bin.join("docker"), fs::Permissions::from_mode(0o755)).unwrap();
        let config = root.path().join("renovate.config.json");
        fs::write(&config, dependencies::TEMPLATE).unwrap();
        Self { root, config, log }
    }
    fn invoke(&self, native: bool, arguments: &[&str]) -> std::process::Output {
        let mut command = if native {
            let mut c = Command::new(env!("CARGO_BIN_EXE_repo-maintainer"));
            c.arg("dependencies");
            c
        } else {
            let mut c = Command::new("/bin/bash");
            c.arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("maintainer"));
            c.arg("dependencies");
            c
        };
        command
            .current_dir(self.root.path())
            .env_clear()
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.path().join("bin").display()),
            )
            .env("RENOVATE_TOKEN", "test-secret")
            .env("MAINTENANCE_PROVIDER", "not-a-model-provider")
            .args(arguments)
            .arg("--config")
            .arg(&self.config)
            .output()
            .unwrap()
    }
    fn calls(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

#[test]
fn both_launchers_validate_then_preview_without_an_agent_config() {
    for native in [true, false] {
        let f = Fixture::new(0, 0);
        let result = f.invoke(native, &[]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let calls = f.calls();
        assert_eq!(calls.lines().count(), 2);
        assert!(
            calls
                .lines()
                .next()
                .unwrap()
                .contains("renovate-config-validator")
        );
        assert!(calls.lines().last().unwrap().contains("--dry-run=full"));
        assert!(!calls.contains("test-secret"));
        assert!(!calls.contains("--build"));
    }
}

#[test]
fn both_launchers_allow_writes_only_with_apply_and_preserve_run_failures() {
    for native in [true, false] {
        let f = Fixture::new(0, 9);
        let result = f.invoke(native, &["--apply"]);
        assert_eq!(result.status.code(), Some(9));
        assert_eq!(f.calls().lines().count(), 2);
        assert!(!f.calls().contains("--dry-run"));
    }
}

#[test]
fn failed_validation_prevents_both_launchers_from_running_updates() {
    for native in [true, false] {
        let f = Fixture::new(3, 0);
        assert!(!f.invoke(native, &["--apply"]).status.success());
        assert_eq!(f.calls().lines().count(), 1);
    }
}

#[test]
fn both_validation_modes_stop_without_updating_repositories() {
    for native in [true, false] {
        let f = Fixture::new(0, 0);
        let result = f.invoke(
            native,
            if native {
                &["--validate-only"]
            } else {
                &["check"]
            },
        );
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(f.calls().lines().count(), 1);
    }
}

#[test]
fn native_init_preserves_operator_configuration() {
    let f = Fixture::new(0, 0);
    fs::remove_file(&f.config).unwrap();
    for iteration in 0..2 {
        let result = Command::new(env!("CARGO_BIN_EXE_repo-maintainer"))
            .arg("dependencies-init")
            .arg("--config")
            .arg(&f.config)
            .output()
            .unwrap();
        assert!(result.status.success());
        if iteration == 0 {
            fs::write(&f.config, b"operator settings").unwrap();
        }
    }
    assert_eq!(fs::read_to_string(&f.config).unwrap(), "operator settings");
    assert!(f.calls().is_empty());
}

#[test]
fn wrapper_init_preserves_operator_configuration_and_rejects_invalid_flags() {
    let f = Fixture::new(0, 0);
    fs::write(&f.config, b"operator settings").unwrap();
    assert!(f.invoke(false, &["init"]).status.success());
    assert_eq!(fs::read_to_string(&f.config).unwrap(), "operator settings");
    assert!(!f.invoke(false, &["check", "--apply"]).status.success());
    assert!(!f.invoke(false, &["--unknown"]).status.success());
    assert!(f.calls().is_empty());
}
