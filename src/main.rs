use anyhow::{Context, Result, ensure};
use clap::{Parser, ValueEnum};
use repo_maintainer::{
    config::{self, Config},
    documentation, process, runner, validation, workflow,
};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum Command {
    Init,
    #[default]
    Check,
    Run,
    Setup,
    Validate,
    Smoke,
    Prepare,
    Agent,
    Collect,
    Apply,
    Publish,
    Notify,
    Reference,
}
#[derive(Parser)]
#[command(
    version,
    about = "Run coding agents on your infrastructure with configured repository checks"
)]
struct Cli {
    #[arg(value_enum, default_value = "check")]
    command: Command,
    #[arg(long, env = "MAINTENANCE_REPOSITORY_ROOT", default_value = ".")]
    repo: PathBuf,
    #[arg(long, env = "MAINTENANCE_CONFIG")]
    config: Option<PathBuf>,
    #[arg(long, alias = "artifacts")]
    output: Option<PathBuf>,
    #[arg(long)]
    source: Option<PathBuf>,
    #[arg(long)]
    prompt: Option<PathBuf>,
    #[arg(long)]
    raw: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    exit_code: i32,
    #[arg(long)]
    url: Option<String>,
}
fn main() {
    let cli = Cli::parse();
    if let Err(error) = execute(cli) {
        eprintln!(
            "{}",
            process::redact(format!("{error:#}"), &config::environment())
        );
        std::process::exit(1);
    }
}
fn execute(cli: Cli) -> Result<()> {
    if matches!(cli.command, Command::Init) {
        let path = cli.repo.join(".maintainer.yml");
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write;
                file.write_all(include_bytes!("../maintainer.example.yml"))?;
                println!(
                    "Created {}. Configure the task, agent and project checks, then run repo-maintainer check.",
                    path.display()
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                println!(".maintainer.yml already exists; keeping your settings.")
            }
            Err(e) => return Err(e.into()),
        }
        return Ok(());
    }
    let environment = config::environment();
    let path = config::find(&cli.repo, cli.config.as_deref());
    let config = if matches!(cli.command, Command::Smoke) && cli.config.is_none() && path.is_err() {
        Config::from_str(
            "sources: [https://developers.openai.com/api/docs/changelog]",
            &environment,
        )?
    } else {
        Config::load(&path?, &environment)?
    };
    let artifacts = cli.output.unwrap_or_else(|| {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        cli.repo
            .join(".maintainer-output")
            .join(format!("{seconds}-{}", std::process::id()))
    });
    let source = || cli.source.as_deref().context("--source is required");
    match cli.command {
        Command::Init => unreachable!(),
        Command::Check => println!(
            "Configuration valid: {}\nRunner: {}\nProvider: {} / {}\nValidation: {}\nDocumentation sources: {}\nNo API request made.",
            config.name,
            config.agent.runner,
            config.agent.provider,
            config.agent.model.as_deref().unwrap_or("CLI default"),
            config.validation.profile,
            config.sources.len()
        ),
        Command::Run => workflow::proposal(&config, &cli.repo, &artifacts)?,
        Command::Setup => validation::setup(&config, cli.source.as_deref().unwrap_or(&cli.repo))?,
        Command::Validate => {
            if let Some(source) = &cli.source {
                workflow::validate(&config, source, &artifacts)?;
            } else {
                validation::setup(&config, &cli.repo)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&validation::validate(&config, &cli.repo)?)?
                );
            }
        }
        Command::Smoke => workflow::smoke(&config, &artifacts)?,
        Command::Prepare => {
            fs::create_dir_all(&artifacts)?;
            workflow::prepare(&config, &artifacts, &cli.repo)?;
        }
        Command::Agent => {
            let prompt_path = cli.prompt.context("--prompt is required")?;
            ensure!(
                fs::metadata(&prompt_path)?.len() <= 256_000,
                "Prompt exceeds 256 KB"
            );
            let prompt = fs::read_to_string(prompt_path)?;
            match runner::run(&config, source()?, &prompt, &environment, false) {
                Ok(value) => println!("{}", serde_json::to_string(&value)?),
                Err(error) => {
                    println!(
                        "{}",
                        serde_json::json!({"error":process::redact(error.to_string(),&environment)})
                    );
                    return Err(error);
                }
            }
        }
        Command::Collect => {
            fs::create_dir_all(&artifacts)?;
            let report = workflow::collect(
                &config,
                source()?,
                &artifacts,
                cli.raw.as_deref().context("--raw is required")?,
                cli.exit_code,
            )?;
            ensure!(
                report.status != "blocked",
                "Maintenance blocked; partial work retained"
            );
        }
        Command::Apply => {
            workflow::apply(&config, source()?, &artifacts)?;
        }
        Command::Publish => workflow::publish(&config, source()?, &artifacts)?,
        Command::Notify => {
            fs::create_dir_all(&artifacts)?;
            workflow::notify(&config, &artifacts)?;
        }
        Command::Reference => println!(
            "{}",
            documentation::fetch(
                &config,
                cli.url.as_deref().context("--url is required")?,
                Duration::from_secs(45)
            )?
        ),
    }
    Ok(())
}
