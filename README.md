<p align="center">
  <img src="docs/assets/banner.png" alt="Repo Maintainer hand-drawn wordmark" width="720">
</p>

<p align="center">
  <a href="https://github.com/djsydney04/repo_maintainer/actions/workflows/ci.yml"><img alt="Build and tests" src="https://github.com/djsydney04/repo_maintainer/actions/workflows/ci.yml/badge.svg?branch=main"></a>
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-a78bfa?style=flat-square&amp;labelColor=111827"></a>
  <a href="maintainer.example.yml"><img alt="Configured with YAML" src="https://img.shields.io/badge/config-YAML-56c8ff?style=flat-square&amp;labelColor=111827"></a>
  <a href="Dockerfile"><img alt="Docker ready" src="https://img.shields.io/badge/run-Docker-54dfbb?style=flat-square&amp;labelColor=111827"></a>
  <a href="Cargo.toml"><img alt="Rust 1.89 or newer" src="https://img.shields.io/badge/Rust-1.89%2B-f49bcb?style=flat-square&amp;labelColor=111827"></a>
</p>

Schedule coding agents on infrastructure you control. Repo Maintainer turns
repository tasks into focused changes, runs the checks you define, and proposes
pull requests for your review. Use it for applications, libraries, documentation
and SDKs. Define the work in **one `.maintainer.yml` file**.

**[Get started](docs/getting-started.md)** · **[Self-hosting](docs/self-hosting.md)** ·
**[Use cases](docs/use-cases.md)** · **[Configuration](docs/configuration.md)** ·
**[How it works](docs/architecture.md)** · **[Troubleshooting](docs/troubleshooting.md)** ·
**[Contribute](CONTRIBUTING.md)**

## Run it your way

| Mode | Where it runs | What you receive |
| --- | --- | --- |
| **Scheduled work on your infrastructure** | Self-hosted GitHub runners on your server or cloud VM | A tested PR, or an assigned issue when help is needed |
| **GitHub-hosted maintenance** | GitHub-hosted Linux | The same PR and notification pipeline |
| **Local Docker run** | The computer running Docker | A patch and report to review locally |
| **Native executable / your scheduler** | macOS or Linux with Git and your project tools | A patch and report; compose the stages for your own pipeline |

### What you control

- **Your task and schedule.** Define recurring work in YAML and trigger it from a workflow, cron or your own code. Documentation sources are optional.
- **Your coding agent.** Run Codex CLI, or use the built-in agent with Groq, Gemini, OpenAI or Anthropic.
- **Your checks.** Supply test, lint and build commands for any language, or use the bundled Rust profile.
- **Your boundaries.** Set editable paths, instructions and execution budgets.
- **Your review.** Inspect each proposal; the maintainer never merges or releases it.

The framework is written in **Rust**. The native executable works with the
project tools on your worker; the Docker image can use a language-specific base
image. The built-in agent and the Codex runner share patch boundaries, independent
validation and publishing. Existing Rust configurations remain supported.

## See it in use

Use it for [application upkeep, documentation, dependency maintenance and more](docs/use-cases.md).
[Prism](https://github.com/djsydney04/prism), the independent Rust SDKs for OpenAI,
Gemini and Groq, is the original compatibility-maintenance example. See its
[task and integration details](docs/use-cases.md#featured-example-prisms-rust-sdks).

## Quick start with the native executable

To build the maintainer, install Rust 1.89 or newer and Git. For the Codex runner,
install Codex CLI and configure its account on the worker. From this checkout:

```sh
cargo install --locked --path .
cd /path/to/your-repository
repo-maintainer init
# Edit the task, allowed paths and checks; configure the selected agent account.
repo-maintainer check
repo-maintainer run
```

`check` validates configuration without inference. `run` works in a temporary
clone of committed HEAD and writes a patch/report to `.maintainer-output/<run>/`.
See [native setup](docs/getting-started.md#native-executable) for details.

## Quick start with Docker

Clone this repository beside the repository you want to maintain. From the
**target repository**, run:

```sh
../repo_maintainer/maintainer init
# Edit .maintainer.yml and provide the selected API key.
../repo_maintainer/maintainer check
../repo_maintainer/maintainer run
```

Requires Docker with Compose. The default image includes the native Codex CLI.
Choose the project runtime with `MAINTAINER_RUNTIME_IMAGE`, such as
`node:24-bookworm`. `init` keeps existing settings, `check` makes no
API request, and `run` reads committed HEAD through a read-only mount. Review
its report and patch in `.maintainer-output/<run>/`.

For hosted PRs, follow the **[weekly cloud setup](docs/getting-started.md#weekly-cloud-maintenance)**.
For direct image/Compose commands, see **[local Docker setup](docs/getting-started.md#local-docker-runs)**.

## One YAML file

```yaml
version: 1
agent:
  runner: codex
  allow_paid: true
instructions: |
  Fix one reproducible bug, preserve public behavior, and add a regression test.
editable:
  source_dirs: [src, tests]
  extensions: [.ts, .tsx, .js, .md, .json]
validation:
  profile: custom
  setup:
    - name: dependencies
      command: [npm, ci]
  checks:
    - name: tests
      command: [npm, test]
```

Start with [the commented template](maintainer.example.yml), then use the
[field reference](docs/configuration.md) for custom paths, checks and model settings.
Adapt the commands to your project. [Node, Python, Go, documentation and Rust examples](docs/use-cases.md#starting-configurations) are included. Existing version-1 JSON configurations remain supported.

Codex uses its CLI authentication or `CODEX_API_KEY`. `allow_paid: true` explicitly
permits use of that account. The following provider settings apply to the
**built-in agent**, selected with `agent.runner: builtin`:

| Provider | Default model | Key |
| --- | --- | --- |
| Groq | `openai/gpt-oss-120b` | `GROQ_API_KEY` |
| Google Gemini | `gemini-3.8-flash` | `GEMINI_API_KEY` |
| OpenAI | `gpt-5.4` | `OPENAI_API_KEY` |
| Anthropic | `claude-sonnet-4-6` | `ANTHROPIC_API_KEY` |

OpenAI and Anthropic require `agent.allow_paid: true`. Keys belong in environment
variables or Actions secrets. Free-tier usage depends on your provider account;
there is no automatic provider/key rotation or paid fallback.
[Provider details →](docs/configuration.md#model-providers)

## Checks before publishing

The workflow separates proposing, validating and publishing. A fresh checkout
applies the exact patch and runs your configured checks. Publishing requires a
validation record bound to that patch and configuration. The optional Rust
profile retains its Cargo tests, linting, documentation, minimum-version and
dependency checks. Proposals are limited to **25 regular files** and **500 KB**.
The built-in agent has a **16-request** ceiling; Codex is bounded by its timeout.
A new blocker creates an assigned issue; unchanged blockers and successful
no-ops stay quiet. [Architecture and limits →](docs/architecture.md)

## Project information

- **Help:** [setup and troubleshooting](docs/troubleshooting.md), then [open an issue](https://github.com/djsydney04/repo_maintainer/issues/new/choose).
- **Development:** [contributing guide](CONTRIBUTING.md) and [CI runs](https://github.com/djsydney04/repo_maintainer/actions).
- **Community:** [discussions](https://github.com/djsydney04/repo_maintainer/discussions) and [code of conduct](CODE_OF_CONDUCT.md).
- **Security:** [reporting and execution boundaries](SECURITY.md).
- **License:** [MIT](LICENSE).
