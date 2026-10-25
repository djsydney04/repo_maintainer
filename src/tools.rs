use crate::{
    boundary::guard,
    config::Config,
    documentation,
    process::{clean_environment, execute, head, tail},
};
use anyhow::{Context, Result, bail, ensure};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use walkdir::WalkDir;

#[derive(Clone)]
pub struct Definition {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}
fn definition(
    name: &'static str,
    description: &'static str,
    properties: Value,
    required: &[&str],
) -> Definition {
    Definition {
        name,
        description,
        parameters: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
    }
}
pub fn definitions(config: &Config) -> Vec<Definition> {
    let path = json!({"type":"string"});
    let mut check = json!({"check":{"type":"string","enum":["format","tests","clippy"]}});
    if !config.rust.crates.is_empty() {
        check["crate"] = json!({"type":"string","enum":config.rust.crates});
    }
    let mut definitions = vec![
        definition(
            "read",
            "Read a UTF-8 source file with one-based line offset and bounded line limit.",
            json!({"path":path,"offset":{"type":"integer","minimum":1},"limit":{"type":"integer","minimum":1,"maximum":400}}),
            &["path"],
        ),
        definition(
            "edit",
            "Replace exactly one occurrence of oldText with newText in an allowed file.",
            json!({"path":path,"oldText":{"type":"string"},"newText":{"type":"string"}}),
            &["path", "oldText", "newText"],
        ),
        definition(
            "write",
            "Write a complete UTF-8 file within the configured editable paths.",
            json!({"path":path,"content":{"type":"string"}}),
            &["path", "content"],
        ),
        definition(
            "ls",
            "List immediate visible files in a source directory.",
            json!({"path":path}),
            &[],
        ),
        definition(
            "find",
            "Find visible source files whose repository-relative names contain pattern.",
            json!({"path":path,"pattern":{"type":"string"}}),
            &["pattern"],
        ),
        definition(
            "grep",
            "Search visible UTF-8 source files with a Rust regular expression. Results contain path and line.",
            json!({"path":path,"pattern":{"type":"string"}}),
            &["pattern"],
        ),
        definition(
            "check_rust",
            "Run offline format, tests or Clippy; inspect failures and repair them.",
            check,
            &["check"],
        ),
        definition(
            "reference",
            "Read an HTTPS reference on configured official documentation hosts; use zero-based character offset to page.",
            json!({"url":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":200,"maximum":4000}}),
            &["url"],
        ),
    ];
    if config.validation.profile != "rust" {
        definitions.retain(|d| d.name != "check_rust");
    }
    if !config.validation.checks.is_empty() {
        definitions.push(definition("check", "Run one operator-configured repository check by name; inspect failures and repair them.", json!({"name":{"type":"string","enum":config.validation.checks.iter().map(|c| &c.name).collect::<Vec<_>>()}}), &["name"]));
    }
    definitions
}
pub fn check_command(config: &Config, args: &Value) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Args {
        check: String,
        #[serde(rename = "crate")]
        package: Option<String>,
    }
    let args: Args = serde_json::from_value(args.clone())?;
    if let Some(package) = &args.package {
        ensure!(
            config.rust.crates.contains(package),
            "Unknown configured crate"
        );
    }
    let scope = match args.package {
        Some(package) => vec!["-p".to_owned(), package],
        None => vec!["--workspace".to_owned()],
    };
    let result = match args.check.as_str() {
        "format" => vec!["fmt", "--all", "--check"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "tests" | "clippy" => {
            let mut command = vec![
                if args.check == "tests" {
                    "test"
                } else {
                    "clippy"
                }
                .to_owned(),
            ];
            command.extend(scope);
            command.extend(["--all-targets", "--locked", "--offline"].map(str::to_owned));
            if args.check == "clippy" {
                command.extend(["--", "-D", "warnings"].map(str::to_owned));
            }
            command
        }
        _ => bail!("Unknown Rust check"),
    };
    Ok(result)
}
pub struct Tools<'a> {
    pub root: &'a Path,
    pub config: &'a Config,
    pub smoke: bool,
    pub deadline: Instant,
    pub checks: Vec<Value>,
}
impl Tools<'_> {
    pub fn call(&mut self, name: &str, args: &Value) -> Result<String> {
        ensure!(Instant::now() < self.deadline, "Agent deadline exceeded");
        let available = definitions(self.config);
        let definition = available.iter().find(|d| d.name == name).with_context(|| {
            format!(
                "Unknown source tool; choose one of: {}",
                available
                    .iter()
                    .map(|d| d.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        let object = args
            .as_object()
            .context("Tool arguments must be an object")?;
        let properties = definition.parameters["properties"].as_object().unwrap();
        ensure!(
            object.keys().all(|k| properties.contains_key(k)),
            "Unknown tool argument"
        );
        for required in definition.parameters["required"].as_array().unwrap() {
            ensure!(
                object.contains_key(required.as_str().unwrap()),
                "Missing tool argument"
            );
        }
        for (key, value) in object {
            match properties[key]["type"].as_str() {
                Some("string") => ensure!(value.is_string(), "{key} must be text"),
                Some("integer") => {
                    let number = value.as_u64().context("Expected a positive integer")?;
                    ensure!(
                        number >= properties[key]["minimum"].as_u64().unwrap_or(0)
                            && number <= properties[key]["maximum"].as_u64().unwrap_or(u64::MAX),
                        "{key} is out of range"
                    );
                }
                _ => (),
            }
        }
        let max = if self.config.agent.provider == "groq" {
            3500
        } else {
            16_000
        };
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if name == "check" {
            let check = self
                .config
                .validation
                .checks
                .iter()
                .find(|c| args["name"] == c.name)
                .context("Unknown configured check")?;
            let result = crate::validation::execute(self.config, check, self.root, remaining)?;
            self.checks
                .push(json!({"check":check.name,"passed":result.code==0}));
            return Ok(format!(
                "Check: {}\nExit code: {}\n{}",
                check.name,
                result.code,
                tail(&result.text(), max)
            ));
        }
        if name == "check_rust" {
            let command = check_command(self.config, args)?;
            let refs: Vec<_> = command.iter().map(String::as_str).collect();
            let result = execute(
                "cargo",
                &refs,
                self.root,
                &clean_environment(),
                remaining.min(Duration::from_secs(300)),
                &[],
            )?;
            self.checks
                .push(json!({"check":args["check"],"passed":result.code==0}));
            return Ok(format!(
                "Command: cargo {}\nExit code: {}\n{}",
                command.join(" "),
                result.code,
                tail(&result.text(), max)
            ));
        }
        if name == "reference" {
            let value = documentation::fetch(
                self.config,
                args["url"].as_str().unwrap(),
                remaining.min(Duration::from_secs(45)),
            )?;
            let offset = args["offset"].as_u64().unwrap_or(0) as usize;
            let limit = args["limit"].as_u64().unwrap_or(3500) as usize;
            return Ok(value.chars().skip(offset).take(limit).collect());
        }
        let path = guard(
            self.root,
            args["path"].as_str().unwrap_or("."),
            ["edit", "write"].contains(&name),
            self.smoke,
            self.config,
        )?;
        let output = match name {
            "read" => {
                ensure!(
                    fs::metadata(&path)?.len() <= 1_000_000,
                    "Read a smaller file"
                );
                let text = fs::read_to_string(&path)?;
                let offset = args["offset"].as_u64().unwrap_or(1) as usize;
                let limit = args["limit"].as_u64().unwrap_or(100) as usize;
                text.lines()
                    .enumerate()
                    .skip(offset - 1)
                    .take(limit)
                    .map(|(i, s)| format!("{}: {s}", i + 1))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            "edit" | "write" => {
                let text = if name == "edit" {
                    ensure!(
                        fs::metadata(&path)?.len() <= 500_000,
                        "Edit exceeds file budget"
                    );
                    let text = fs::read_to_string(&path)?;
                    let old = args["oldText"].as_str().unwrap();
                    ensure!(
                        !old.is_empty() && text.matches(old).count() == 1,
                        "oldText must match exactly once"
                    );
                    text.replacen(old, args["newText"].as_str().unwrap(), 1)
                } else {
                    args["content"].as_str().unwrap().to_owned()
                };
                ensure!(text.len() <= 500_000, "Write exceeds file budget");
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, text)?;
                "File updated.".into()
            }
            "ls" | "find" | "grep" => {
                let pattern = args["pattern"].as_str().unwrap_or("");
                ensure!(pattern.len() <= 1000, "Search pattern too large");
                let regex = if name == "grep" {
                    Some(Regex::new(pattern)?)
                } else {
                    None
                };
                let root = self.root.canonicalize()?;
                let depth = if name == "ls" { 1 } else { 32 };
                let entries = WalkDir::new(&path)
                    .follow_links(false)
                    .max_depth(depth)
                    .sort_by_file_name()
                    .into_iter()
                    .filter_entry(|e| {
                        e.path() == path
                            || (!e.file_name().to_string_lossy().starts_with('.')
                                && !["node_modules", "target"]
                                    .contains(&e.file_name().to_string_lossy().as_ref()))
                    });
                let mut lines = vec![];
                let mut scanned = 0;
                let mut size = 0;
                for entry in entries {
                    ensure!(Instant::now() < self.deadline, "Agent deadline exceeded");
                    let entry = entry?;
                    scanned += 1;
                    if scanned > 10_000 || lines.len() >= 200 || size >= max {
                        break;
                    }
                    if entry.file_type().is_symlink() {
                        continue;
                    }
                    let name = entry
                        .path()
                        .strip_prefix(&root)?
                        .to_string_lossy()
                        .to_string();
                    if guard(&root, &name, false, self.smoke, self.config).is_err() {
                        continue;
                    }
                    if let Some(regex) = &regex {
                        if !entry.file_type().is_file() || entry.metadata()?.len() > 1_000_000 {
                            continue;
                        }
                        if let Ok(text) = fs::read_to_string(entry.path()) {
                            for (i, line) in text.lines().enumerate() {
                                if regex.is_match(line) {
                                    let line = format!("{name}:{}: {}", i + 1, head(line, 500));
                                    size += line.len();
                                    lines.push(line);
                                    if lines.len() >= 200 || size >= max {
                                        break;
                                    }
                                }
                            }
                        }
                    } else if entry.path() != path && (name.contains(pattern) || name.is_empty()) {
                        size += name.len();
                        lines.push(name);
                    }
                }
                lines.join("\n")
            }
            _ => bail!("Unknown source tool"),
        };
        Ok(if output.chars().count() > max {
            format!(
                "{}\n[Truncated; use a smaller range or narrower search.]",
                head(&output, max)
            )
        } else {
            output
        })
    }
}
