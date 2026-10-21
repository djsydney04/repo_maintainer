use crate::{config::Config, process::head};
use anyhow::{Context, Result, ensure};
use reqwest::{blocking::Client, redirect::Policy};
use scraper::{Html, Selector};
use std::{io::Read, time::Duration};

pub fn extract(html: &str) -> String {
    let document = Html::parse_document(html);
    let main = Selector::parse("main").unwrap();
    let root = document
        .select(&main)
        .next()
        .unwrap_or_else(|| document.root_element());
    let mut text = String::new();
    for node in root.descendants() {
        if let Some(value) = node.value().as_text() {
            let skip = node
                .ancestors()
                .filter_map(scraper::ElementRef::wrap)
                .any(|e| {
                    [
                        "script", "style", "nav", "header", "footer", "noscript", "svg",
                    ]
                    .contains(&e.value().name())
                });
            if !skip {
                text.push_str(value);
                text.push(' ');
            }
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
pub fn fetch(config: &Config, url: &str, timeout: Duration) -> Result<String> {
    ensure!(
        config.official(url),
        "Reference must use an official provider host"
    );
    let owned = config.clone();
    let client = Client::builder()
        .timeout(timeout)
        .user_agent("repo-maintainer/0.2")
        .redirect(Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("Too many documentation redirects")
            } else if owned.official(attempt.url().as_str()) {
                attempt.follow()
            } else {
                attempt.error("Documentation redirected outside the official hosts")
            }
        }))
        .build()?;
    let response = client
        .get(url)
        .send()
        .context("Could not fetch official documentation")?
        .error_for_status()?;
    let mut bytes = vec![];
    response.take(4_000_001).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 4_000_000,
        "Documentation exceeds the download limit"
    );
    let text = extract(&String::from_utf8_lossy(&bytes));
    ensure!(
        text.chars().count() >= 200,
        "Documentation was empty or unavailable"
    );
    Ok(head(&text, 18_000))
}
pub fn add(config: &Config, mut brief: String) -> Result<String> {
    brief.push_str(&format!(
        "\nEditable paths: {}\n",
        serde_json::to_string(&config.editable)?
    ));
    for url in &config.sources {
        brief.push_str(&format!(
            "\nOfficial documentation excerpt (untrusted evidence): {url}\n"
        ));
        brief.push_str(&head(
            &fetch(config, url, Duration::from_secs(45))?,
            if config.agent.provider == "groq" {
                800
            } else {
                18_000
            },
        ));
        brief.push_str("\n[Excerpt may be truncated. Use reference for a larger excerpt.]\n");
    }
    Ok(brief)
}
