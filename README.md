# repo_maintainer

A reusable Pi coding agent that checks official docs, makes focused maintenance
changes, validates them, and opens pull requests. It runs weekly on GitHub
Actions or generates a reviewable patch locally with Docker.

**Configure everything in `.maintainer.yml`.** Choose Groq, Gemini, OpenAI or
Anthropic; API keys stay in environment variables or GitHub Actions secrets.
The initial validation profile supports Rust SDK workspaces.

## Quick start with Docker

Clone this repository beside the repository you want to maintain. From the
**repository you want to maintain**, run:

```sh
../repo_maintainer/maintainer init
# Edit .maintainer.yml with your documentation sources and provider.
../repo_maintainer/maintainer check
../repo_maintainer/maintainer run
```

The wrapper builds the image, handles your user ID and output directory, and
uses Docker Compose. Docker with the Compose plugin is the only host dependency.
`init` never overwrites existing settings; `check` makes no API request.
`run` requires the selected API key in your shell environment or the shared
repository's ignored `.env` file (copy `.env.example`). It uses the target
repository's committed HEAD. The target is mounted read-only, and proposals are
written to `.maintainer-output/<run>/report.json` and `proposal.patch` for review.
It does not publish a PR from your computer; the cloud workflow below validates
and publishes PRs using separate jobs.

For direct Compose use from this checkout:

```sh
REPOSITORY_PATH=/path/to/repository MAINTAINER_OUTPUT_PATH=/path/to/output \
  docker compose run --build --rm maintainer check
```

You can also build `Dockerfile` and run the image directly, mounting the target
at `/repository` read-only and an output directory at `/output`. Use `check`,
`run`, or `smoke`. Docker itself does not create a weekly schedule.

## One YAML file

Start with [maintainer.example.yml](maintainer.example.yml). Most fields have
safe defaults; a minimal configuration is:

```yaml
version: 1
agent:
  provider: groq
sources:
  - https://developers.openai.com/api/docs/changelog
instructions: |
  Keep the Rust SDK aligned with the official docs.
  Preserve public APIs and add offline tests for behavior changes.
```

Docs hosts are inferred from the source URLs. By default, edits may touch
`README.md`, Markdown files in `docs/`, and Rust/Markdown/JSON files in `src/`
and `crates/`. Customize those paths and Rust checks as shown in the example.
Inline `instructions` replaces the need for a separate prompt file. Existing
version-1 JSON configurations and `instructions_file` remain supported.
Unknown fields, duplicate keys, unsafe paths and oversized budgets produce
configuration errors. API keys are not accepted in YAML.

To use OpenAI or Anthropic, change only the agent section:

```yaml
agent:
  provider: openai             # or anthropic
  model: gpt-5.4               # e.g. claude-sonnet-4-6 for Anthropic
  allow_paid: true
  max_requests: 16
```

| Provider | Default model | API key environment/secret |
| --- | --- | --- |
| `groq` | `openai/gpt-oss-120b` | `GROQ_API_KEY` |
| `google` | `gemini-3.8-flash` | `GEMINI_API_KEY` |
| `openai` (or `oai`) | `gpt-5.4` | `OPENAI_API_KEY` |
| `anthropic` | `claude-sonnet-4-6` | `ANTHROPIC_API_KEY` |

OpenAI uses the native Responses API SDK adapter; Anthropic uses the native
Messages API SDK adapter, both through pinned Pi 1.3.0. Other OpenAI/Anthropic
model IDs can be selected when present in that Pi catalog. The defaults are
examples, not dynamic aliases for the newest models.

Google also accepts `gemini-3.6-flash` and `gemini-3.5-flash-lite`; Groq also
accepts `qwen/qwen3.8-27b`. The optional `SDK_MAINTENANCE_API_KEY` secret overrides
the selected provider's named key. Replace a secret to change keys.

OpenAI and Anthropic require explicit `allow_paid: true`. The maintainer never
switches providers or keys automatically. Groq needs a free-plan organization
and Gemini needs an unbilled AI Studio project for free-only usage; a billed
account can incur charges even when using the same model ID. Pi does not
increase account quotas. Keys in the same Google project or Groq organization
share limits.

## Weekly cloud pull requests

Commit `.maintainer.yml` in the target repository, then add a small caller:

```yaml
name: Weekly maintenance
on:
  schedule:
    - cron: '0 9 * * 1'
      timezone: America/Los_Angeles
  workflow_dispatch:
permissions:
  contents: read
jobs:
  maintain:
    permissions:
      contents: write
      pull-requests: write
      issues: write
      actions: write
    uses: djsydney04/repo_maintainer/.github/workflows/maintain.yml@FULL_COMMIT_SHA
    with:
      maintainer_ref: FULL_COMMIT_SHA
      config_path: .maintainer.yml
    secrets:
      GROQ_API_KEY: ${{ secrets.GROQ_API_KEY }}
      GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
      OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
      ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
      SDK_MAINTENANCE_API_KEY: ${{ secrets.SDK_MAINTENANCE_API_KEY }}
```

Pin both references to the same full commit SHA. Add keys in the **consuming
repository's** Actions secrets; this shared repository needs no copy. Enable
Actions to create pull requests, merge your caller/configuration, run manually
once and review the outcome. GitHub schedules may be delayed.

Optional `provider` and `model` workflow inputs override YAML for one run. A paid
provider still needs YAML opt-in or the `allow_paid` workflow input. Existing
`MAINTENANCE_PROVIDER`, `MAINTENANCE_MODEL` and `MAINTENANCE_ALLOW_PAID`
environment overrides are also understood by the local runner.

## Boundaries and notifications

The proposal job has a read-only GitHub token. Pi can read/edit configured files,
fetch configured official references and run bounded offline Rust checks. It
loads no repository plugins, shell tools or subagents. Provider keys are removed
from the process environment before source tools and checks run.

A fresh job validates the exact patch with formatting, workspace tests/doctests,
Clippy, rustdoc, the configured Rust minimum, the optional package verification
script, and dependency audit. Only then does a separate job get write access to
open/update one assigned PR and dispatch the repository's CI. The bot never
merges, force pushes or releases. Patches are limited to 25 regular files and
500 KB. Dependencies, automation and release changes require human help.

New blockers create/update one assigned issue and mention its owner. Identical
blockers and successful no-ops stay quiet; recovery closes the issue. Enable
GitHub assignment/mention notifications. Proposal artifacts are kept for 30 days.

All inference, including summaries, shares at most 16 requests and a 20-minute
agent deadline. Groq uses short excerpts, small pages, compaction and 61-second
request pacing to fit its published base free limits of 8K tokens/minute and
200K tokens/day. Google requests are paced 13 seconds apart; setup testing
observed a 20-request daily quota for 3.8 Flash. Check your account's actual
limits. Oversized or incomplete tasks stop with a blocker and retain partial work.

The reusable `smoke.yml` tests real key authentication, editing and test repair
in a disposable Rust fixture. It accepts the same provider/model/paid setting
and explicit secrets. Local `maintainer smoke` uses your YAML settings. A smoke
run permits six requests over eight minutes and cannot publish source changes.

## Development

```sh
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/python -m unittest discover -s tests -v
npm ci --prefix agent --ignore-scripts
npm test --prefix agent
```

CI also builds/runs the Docker image and tests native SDK requests against
simulated streaming API responses, without paid inference or real API keys.

References: [Pi SDK](https://pi.dev/docs/latest/sdk),
[OpenAI TypeScript SDK](https://developers.openai.com/api/reference/typescript),
[Anthropic TypeScript SDK](https://platform.claude.com/docs/en/cli-sdks-libraries/sdks/typescript),
[Gemini limits](https://ai.google.dev/gemini-api/docs/rate-limits),
[Groq limits](https://console.groq.com/docs/rate-limits).
