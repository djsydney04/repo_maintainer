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

Keep Rust SDKs aligned with official documentation. Repo Maintainer reads the
sources you choose, makes focused changes, runs checks, and proposes a pull
request for your review. Configure it in **one `.maintainer.yml` file**.

**[Get started](docs/getting-started.md)** · **[Configuration](docs/configuration.md)** ·
**[How it works](docs/architecture.md)** · **[Troubleshooting](docs/troubleshooting.md)** ·
**[Contribute](CONTRIBUTING.md)**

## Run it your way

| Mode | Where it runs | What you receive |
| --- | --- | --- |
| **Weekly cloud maintenance** | GitHub-hosted Linux; your computer can be off | A tested PR, or an assigned issue when help is needed |
| **Local Docker run** | The computer running Docker | A patch and report to review locally |
| **Native executable** | macOS or Linux with Git and Rust check tools | A patch and report to review locally |

### What you control

- **Your sources.** Track official API references, changelogs and documentation.
- **Your model.** Choose Groq, Gemini, OpenAI or Anthropic and supply your own key.
- **Your boundaries.** Set editable paths, instructions and the request budget.
- **Your review.** Inspect each proposal; the maintainer never merges or releases it.

The maintainer is written in **Rust**, including its agent loop, provider
adapters and workflow commands. Its validation profile supports **Rust SDK
workspaces**. The native executable and Docker image need no Python or Node
runtime.

## Quick start with the native executable

With Rust 1.89 or newer and Git installed, run from this checkout:

```sh
cargo install --locked --path .
cd /path/to/your-rust-repository
repo-maintainer init
# Edit .maintainer.yml and export the selected API key.
repo-maintainer check
repo-maintainer run
```

`check` validates configuration without inference. `run` works in a temporary
clone of committed HEAD and writes a patch/report to `.maintainer-output/<run>/`.
See [native setup](docs/getting-started.md#native-executable) for details.

## Quick start with Docker

Clone this repository beside the Rust repository you want to maintain. From the
**target repository**, run:

```sh
../repo_maintainer/maintainer init
# Edit .maintainer.yml and provide the selected API key.
../repo_maintainer/maintainer check
../repo_maintainer/maintainer run
```

Requires Docker with Compose. `init` keeps existing settings, `check` makes no
API request, and `run` reads committed HEAD through a read-only mount. Review
its report and patch in `.maintainer-output/<run>/`.

For hosted PRs, follow the **[weekly cloud setup](docs/getting-started.md#weekly-cloud-maintenance)**.
For direct image/Compose commands, see **[local Docker setup](docs/getting-started.md#local-docker-runs)**.

## One YAML file

```yaml
version: 1
agent:
  provider: groq
sources:
  - https://developers.openai.com/api/docs/changelog
instructions: |
  Keep the SDK aligned with the official docs.
  Preserve public APIs and add offline tests for behavior changes.
```

Start with [the commented template](maintainer.example.yml), then use the
[field reference](docs/configuration.md) for custom paths, checks and model settings.
Existing version-1 JSON configurations remain supported.

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

The cloud workflow separates proposing, validating and publishing. It checks the
patch boundary, formatting, tests, Clippy, rustdoc, the Rust minimum, optional
package verification and dependency audit before opening/updating a PR.
Proposals are limited to **25 regular files**, **500 KB**, and **16 model requests**.
A new blocker creates an assigned issue; unchanged blockers and successful
no-ops stay quiet. [Architecture and limits →](docs/architecture.md)

## Project information

- **Help:** [setup and troubleshooting](docs/troubleshooting.md), then [open an issue](https://github.com/djsydney04/repo_maintainer/issues/new/choose).
- **Development:** [contributing guide](CONTRIBUTING.md) and [CI runs](https://github.com/djsydney04/repo_maintainer/actions).
- **Community:** [discussions](https://github.com/djsydney04/repo_maintainer/discussions) and [code of conduct](CODE_OF_CONDUCT.md).
- **Security:** [reporting and execution boundaries](SECURITY.md).
- **License:** [MIT](LICENSE).
