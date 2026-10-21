//! Self-hosted Renovate execution. This path never loads an agent or model key.
use crate::{config::Environment, process};
use anyhow::{Context, Result, ensure};
use std::{fs, path::Path, process::Command};

pub const IMAGE: &str = include_str!("../renovate-image.txt");
pub const TEMPLATE: &[u8] = include_bytes!("../renovate.example.json");
const CONTAINER_CONFIG: &str = "/config/renovate.json";
const FORWARDED: &[&str] = &[
    "RENOVATE_TOKEN",
    "RENOVATE_GITHUB_COM_TOKEN",
    "RENOVATE_HOST_RULES",
    "RENOVATE_GIT_PRIVATE_KEY",
    "LOG_LEVEL",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
];

pub fn init(path: &Path) -> Result<()> {
    use std::io::Write;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(TEMPLATE)?;
            println!(
                "Created {}. Add your repositories before running dependencies.",
                path.display()
            );
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            println!("{} already exists; keeping your settings.", path.display());
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

pub fn command(path: &Path, env: &Environment, validate: bool, apply: bool) -> Result<Command> {
    ensure!(
        !(validate && apply),
        "--validate-only and --apply cannot be combined"
    );
    let path = path
        .canonicalize()
        .context("Add renovate.config.json, or run dependencies-init")?;
    ensure!(
        path.is_file(),
        "Renovate configuration must be a regular file"
    );
    let source = path
        .to_str()
        .context("Renovate config path must be UTF-8")?;
    // Docker's --mount syntax treats commas as field separators.
    ensure!(
        !source.contains([',', '\n', '\r']),
        "Renovate config path cannot contain commas or newlines"
    );
    let image = env
        .get("MAINTENANCE_RENOVATE_IMAGE")
        .map(String::as_str)
        .unwrap_or(IMAGE.trim());
    let (reference, digest) = image
        .rsplit_once("@sha256:")
        .context("Renovate image must be pinned with @sha256:DIGEST")?;
    ensure!(
        !reference.is_empty()
            && digest.len() == 64
            && digest.bytes().all(|c| c.is_ascii_hexdigit())
            && !image.starts_with('-')
            && !image.chars().any(char::is_whitespace),
        "Invalid pinned Renovate image"
    );
    let mut child_env = process::clean_environment_from(env.clone());
    for (key, value) in env {
        if key.starts_with("DOCKER_") || FORWARDED.contains(&key.as_str()) {
            child_env.insert(key.clone(), value.clone());
        }
    }
    let mut command = Command::new("docker");
    command.env_clear().envs(child_env).args([
        "run",
        "--rm",
        "--init",
        "--mount",
        &format!("type=bind,source={source},target={CONTAINER_CONFIG},readonly"),
        "--volume",
        "repo-maintainer-renovate-cache:/tmp/renovate",
        "--env",
        "RENOVATE_CONFIG_FILE=/config/renovate.json",
        "--env",
        "RENOVATE_BASE_DIR=/tmp/renovate",
    ]);
    for name in FORWARDED {
        command.args(["--env", name]);
    }
    command.arg(image);
    if validate {
        command.args(["renovate-config-validator", "--strict", CONTAINER_CONFIG]);
    } else {
        command.args(["renovate", "--autodiscover=false"]);
        if !apply {
            command.arg("--dry-run=full");
        }
    }
    Ok(command)
}

pub fn run(path: &Path, env: &Environment, validate_only: bool, apply: bool) -> Result<()> {
    // Validate with Renovate itself before any preview or live run.
    let mut validator = command(path, env, true, false)?;
    let mut runner = command(path, env, validate_only, apply)?;
    let status = validator
        .status()
        .context("Docker is required to run self-hosted Renovate")?;
    ensure!(status.success(), "Renovate configuration validation failed");
    if validate_only {
        return Ok(());
    }
    println!(
        "Starting self-hosted Renovate ({}). No model calls.",
        if apply { "live" } else { "preview" }
    );
    // Let Docker receive signals directly and preserve Renovate's exit status.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(runner.exec()).context("Could not start Docker")
    }
    #[cfg(not(unix))]
    {
        ensure!(runner.status()?.success(), "Renovate run failed");
        Ok(())
    }
}
