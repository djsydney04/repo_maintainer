use anyhow::{Context, Result, bail, ensure};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use url::Url;

pub type Environment = BTreeMap<String, String>;
pub fn environment() -> Environment {
    std::env::vars().collect()
}
fn text(s: &str) -> String {
    s.into()
}
fn default_provider() -> String {
    text("groq")
}
fn requests() -> u8 {
    16
}
fn version() -> u8 {
    1
}
fn name() -> String {
    text("Repository maintenance")
}
fn prefix() -> String {
    text("dylan/repo-maintenance-")
}
fn issue() -> String {
    text("Repository maintenance needs help")
}
fn commit() -> String {
    text("fix: maintain documented SDK compatibility")
}
fn workflow() -> String {
    text("ci.yml")
}
fn instructions() -> String {
    text(
        "Maintain the repository against the official documentation sources. Implement focused, compatible changes with meaningful offline tests.",
    )
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Agent {
    pub provider: String,
    pub model: Option<String>,
    pub allow_paid: bool,
    pub max_requests: u8,
}
impl Default for Agent {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: None,
            allow_paid: false,
            max_requests: requests(),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Editable {
    pub root_files: Vec<String>,
    pub markdown_dirs: Vec<String>,
    pub source_dirs: Vec<String>,
    pub extensions: Vec<String>,
}
impl Default for Editable {
    fn default() -> Self {
        Self {
            root_files: vec![text("README.md")],
            markdown_dirs: vec![text("docs")],
            source_dirs: vec![text("src"), text("crates")],
            extensions: vec![text(".rs"), text(".md"), text(".json")],
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rust {
    pub msrv: String,
    pub crates: Vec<String>,
    pub package_check: String,
}
impl Default for Rust {
    fn default() -> Self {
        Self {
            msrv: text("1.88.0"),
            crates: vec![],
            package_check: String::new(),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "version")]
    pub version: u8,
    #[serde(default = "name")]
    pub name: String,
    pub sources: Vec<String>,
    #[serde(default)]
    pub official_hosts: Vec<String>,
    #[serde(default = "instructions")]
    pub instructions: String,
    #[serde(default)]
    pub instructions_file: Option<String>,
    #[serde(default)]
    pub agent: Agent,
    #[serde(default)]
    pub editable: Editable,
    #[serde(default)]
    pub rust: Rust,
    #[serde(default = "prefix")]
    pub branch_prefix: String,
    #[serde(default = "issue")]
    pub issue_title: String,
    #[serde(default = "commit")]
    pub commit_title: String,
    #[serde(default = "workflow")]
    pub ci_workflow: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub key: String,
    pub default_model: String,
    pub models: Option<Vec<String>>,
    pub paid: bool,
    pub interval_ms: u64,
    pub max_tokens: u32,
    pub max_input_chars: usize,
}
pub fn profiles() -> BTreeMap<String, Profile> {
    serde_json::from_str(include_str!("providers.json")).expect("bundled provider profiles")
}
pub fn relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains(['\\', '\n', '\r', '\0'])
        && value
            .split('/')
            .all(|p| !p.is_empty() && p != ".." && p != ".")
}
fn strings(values: &[String], empty: bool) -> Result<()> {
    ensure!(
        (empty || !values.is_empty())
            && values.len() <= 100
            && values
                .iter()
                .all(|s| !s.is_empty() && s.chars().count() <= 2000),
        "Expected a bounded list of nonempty strings"
    );
    Ok(())
}
impl Agent {
    pub fn resolve(&mut self, env: &Environment) -> Result<Profile> {
        let old = if self.provider == "oai" {
            "openai"
        } else {
            &self.provider
        }
        .to_owned();
        if let Some(provider) = env.get("MAINTENANCE_PROVIDER").filter(|s| !s.is_empty()) {
            self.provider = provider.clone();
        }
        if self.provider == "oai" {
            self.provider = text("openai");
        }
        let profile = profiles()
            .remove(&self.provider)
            .context("agent.provider must be google, groq, openai or anthropic")?;
        let override_model = env.get("MAINTENANCE_MODEL").filter(|s| !s.is_empty());
        self.model = Some(
            override_model
                .cloned()
                .or_else(|| {
                    if old == self.provider {
                        self.model.clone()
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| profile.default_model.clone()),
        );
        let model = self.model.as_deref().unwrap();
        ensure!(
            !model.is_empty() && model.len() <= 200 && !model.chars().any(char::is_whitespace),
            "agent.model must be a model ID"
        );
        if let Some(models) = &profile.models {
            ensure!(
                models.iter().any(|m| m == model),
                "Unsupported {} model: {model}",
                self.provider
            );
        }
        if let Some(paid) = env.get("MAINTENANCE_ALLOW_PAID").filter(|s| !s.is_empty()) {
            self.allow_paid = match paid.as_str() {
                "true" => true,
                "false" => false,
                _ => bail!("MAINTENANCE_ALLOW_PAID must be true or false"),
            };
        }
        ensure!(
            !profile.paid || self.allow_paid,
            "{} requires agent.allow_paid: true; it is never a free-tier fallback",
            self.provider
        );
        ensure!(
            (1..=16).contains(&self.max_requests),
            "agent.max_requests must be between 1 and 16"
        );
        Ok(profile)
    }
}
impl Config {
    pub fn from_str(raw: &str, env: &Environment) -> Result<Self> {
        ensure!(raw.len() <= 256_000, "Configuration exceeds 256 KB");
        let options = serde_saphyr::options! {
            duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
            merge_keys: serde_saphyr::MergeKeyPolicy::Error,
            strict_booleans: true,
            reject_unsupported_tags: true,
            with_snippet: false,
            budget: serde_saphyr::budget! {
                max_documents: 1, max_depth: 16, max_aliases: 50, max_anchors: 50,
                max_nodes: 3000, max_events: 10_000, max_total_scalar_bytes: 1_000_000,
            },
            alias_limits: serde_saphyr::alias_limits! {
                max_replay_stack_depth: 16, max_total_replayed_events: 10_000,
                max_alias_expansions_per_anchor: 50,
            },
        };
        // Reject duplicate keys in the YAML parser before materializing a map.
        // An untyped intermediate preserves quoted strings as strings; the
        // second step enforces exact bool/integer/string types with Serde JSON.
        let value: serde_json::Value = serde_saphyr::from_str_with_options(raw, options)
            .context("Invalid YAML/JSON configuration")?;
        let mut config: Self =
            serde_json::from_value(value).context("Invalid configuration fields")?;
        ensure!(
            config.version == 1,
            "Unsupported maintainer configuration version"
        );
        strings(&config.sources, false)?;
        let mut hosts = vec![];
        for source in &config.sources {
            let url = Url::parse(source)?;
            ensure!(
                url.scheme() == "https"
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.port_or_known_default() == Some(443)
                    && !source.chars().any(char::is_whitespace),
                "Every documentation source must be an HTTPS URL without credentials"
            );
            hosts.push(url.host_str().unwrap().to_owned());
        }
        hosts.sort();
        hosts.dedup();
        if config.official_hosts.is_empty() {
            config.official_hosts = hosts.clone();
        }
        strings(&config.official_hosts, false)?;
        for host in &config.official_hosts {
            ensure!(
                Url::parse(&format!("https://{host}/"))?.host_str() == Some(host.as_str())
                    && !host.contains(['/', ':', '@']),
                "Invalid official host"
            );
        }
        ensure!(
            hosts.iter().all(|h| config.official_hosts.contains(h)),
            "Every source must use an official_hosts entry"
        );
        for values in [
            &config.editable.root_files,
            &config.editable.markdown_dirs,
            &config.editable.source_dirs,
        ] {
            strings(values, true)?;
            ensure!(
                values.iter().all(|v| relative_path(v)),
                "Editable paths must be repository-relative"
            );
        }
        strings(&config.editable.extensions, true)?;
        let extension = Regex::new(r"^\.[A-Za-z0-9]+$")?;
        ensure!(
            config
                .editable
                .extensions
                .iter()
                .all(|v| extension.is_match(v)),
            "Invalid editable extensions"
        );
        ensure!(
            Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+$")?.is_match(&config.rust.msrv),
            "rust.msrv must be a version such as 1.88.0"
        );
        strings(&config.rust.crates, true)?;
        let crate_name = Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_-]*$")?;
        ensure!(
            config.rust.crates.iter().all(|v| crate_name.is_match(v)),
            "Invalid Rust crate name"
        );
        ensure!(
            config.rust.package_check.is_empty() || relative_path(&config.rust.package_check),
            "Invalid rust.package_check path"
        );
        for value in [&config.name, &config.issue_title, &config.commit_title] {
            ensure!(
                !value.is_empty()
                    && value.chars().count() <= 200
                    && !value.contains(['\n', '\r', '\0']),
                "Titles must be one short line"
            );
        }
        ensure!(
            Regex::new(r"^[A-Za-z0-9_/-]+-$")?.is_match(&config.branch_prefix)
                && !config.branch_prefix.starts_with('/')
                && !config.branch_prefix.contains("//"),
            "Invalid maintenance branch prefix"
        );
        ensure!(relative_path(&config.ci_workflow), "Invalid ci_workflow");
        if let Some(path) = &config.instructions_file {
            ensure!(relative_path(path), "Invalid instructions_file");
        }
        ensure!(
            config.instructions.chars().count() <= 16_000,
            "instructions exceeds 16000 characters"
        );
        config.agent.resolve(env)?;
        Ok(config)
    }
    pub fn load(path: &Path, env: &Environment) -> Result<Self> {
        ensure!(
            fs::metadata(path)?.len() <= 256_000,
            "Configuration exceeds 256 KB"
        );
        Self::from_str(&fs::read_to_string(path)?, env)
    }
    pub fn official(&self, source: &str) -> bool {
        Url::parse(source).is_ok_and(|u| {
            u.scheme() == "https"
                && u.username().is_empty()
                && u.password().is_none()
                && u.port_or_known_default() == Some(443)
                && u.host_str()
                    .is_some_and(|h| self.official_hosts.iter().any(|v| v == h))
                && !source.chars().any(char::is_whitespace)
        })
    }
}
pub fn find(root: &Path, explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        let path = root.join(path);
        ensure!(
            path.is_file(),
            "Configuration not found: {}",
            path.display()
        );
        return Ok(path.canonicalize()?);
    }
    for name in [
        ".maintainer.yml",
        ".maintainer.yaml",
        ".github/maintainer.yml",
        ".github/maintainer.yaml",
        ".github/maintainer.json",
    ] {
        let path = root.join(name);
        if path.is_file() {
            return Ok(path.canonicalize()?);
        }
    }
    bail!("Add .maintainer.yml, or run repo-maintainer init")
}
