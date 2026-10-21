//! Operator-configured repository checks, independent of the coding agent.
use crate::{
    boundary,
    config::{Check, Config},
    process::{self, Output},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

pub fn execute(config: &Config, check: &Check, root: &Path, remaining: Duration) -> Result<Output> {
    ensure!(!check.command.is_empty(), "Check command cannot be empty");
    let cwd = boundary::guard(root, &check.working_directory, false, false, config)?;
    let args: Vec<_> = check.command[1..].iter().map(String::as_str).collect();
    let mut environment = process::clean_environment();
    environment.insert("CI".into(), "true".into());
    if config.validation.profile == "rust" && check.name == "rustdoc" {
        environment.insert("RUSTDOCFLAGS".into(), "-D warnings".into());
    }
    process::execute(
        &check.command[0],
        &args,
        &cwd,
        &environment,
        remaining.min(Duration::from_secs(check.timeout_seconds)),
        &[],
    )
}

fn required(config: &Config, check: &Check, root: &Path) -> Result<Value> {
    let result = execute(
        config,
        check,
        root,
        Duration::from_secs(check.timeout_seconds),
    )
    .with_context(|| format!("Could not run check {}", check.name))?;
    ensure!(
        result.code == 0,
        "Check {} exited {}: {}",
        check.name,
        result.code,
        process::tail(
            &process::redact(result.text(), &crate::config::environment()),
            4000
        )
    );
    Ok(json!({"check": check.name, "passed": true}))
}

fn command(name: &str, arguments: &[&str]) -> Check {
    Check {
        name: name.into(),
        command: arguments.iter().map(|s| (*s).into()).collect(),
        working_directory: ".".into(),
        timeout_seconds: 600,
    }
}

pub fn setup(config: &Config, root: &Path) -> Result<()> {
    if config.validation.profile == "rust" {
        required(
            config,
            &command("cargo-fetch", &["cargo", "fetch", "--locked"]),
            root,
        )?;
    }
    for check in &config.validation.setup {
        required(config, check, root)?;
    }
    Ok(())
}

pub fn validate(config: &Config, root: &Path) -> Result<Vec<Value>> {
    let mut checks = vec![];
    if config.validation.profile == "rust" {
        checks.extend([
            command("format", &["cargo", "fmt", "--all", "--check"]),
            command(
                "tests",
                &["cargo", "test", "--workspace", "--all-targets", "--locked"],
            ),
            command(
                "doctests",
                &["cargo", "test", "--workspace", "--doc", "--locked"],
            ),
            command(
                "clippy",
                &[
                    "cargo",
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ],
            ),
            command(
                "rustdoc",
                &["cargo", "doc", "--workspace", "--no-deps", "--locked"],
            ),
            command(
                "msrv",
                &[
                    "cargo",
                    &format!("+{}", config.rust.msrv),
                    "check",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                ],
            ),
        ]);
        if !config.rust.package_check.is_empty() {
            checks.push(command(
                "package",
                &[&format!("./{}", config.rust.package_check)],
            ));
        }
        checks.push(command("audit", &["cargo", "audit", "--deny", "warnings"]));
    }
    checks.extend(config.validation.checks.clone());
    checks
        .iter()
        .map(|check| required(config, check, root))
        .collect()
}
