# repo_maintainer

Reusable weekly repository maintenance with the Pi coding agent, configurable
Gemini or Groq API keys, tested pull requests, and GitHub notifications when a
person needs to help. Runs on GitHub Actions while your computer is off.

The first validation profile supports Rust SDK workspaces. Each consuming
repository owns its documentation sources, instructions, editable paths,
minimum Rust version, package check script, schedule, keys and model choice.

## Connect a repository

1. Copy [the configuration example](examples/rust-sdks.json) to
   `.github/maintainer.json` and adapt it. Add project instructions at the
   configured `instructions_file`. The [Prism instructions](examples/prompt.md)
   show the expected task and structured result.
2. Store a free-tier key in the consuming repository's Actions secrets:
   `GEMINI_API_KEY` or `GROQ_API_KEY`. An optional `SDK_MAINTENANCE_API_KEY`
   overrides the selected provider's named secret, making key replacement easy.
   This shared repository needs no copy of those keys.
3. Add a caller workflow. Pin **both references to the same full commit SHA**
   from this repository; updating this repository alone cannot change your bot.

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
      provider: ${{ vars.SDK_MAINTENANCE_PROVIDER || 'google' }}
      model: ${{ vars.SDK_MAINTENANCE_MODEL }}
    secrets:
      GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
      GROQ_API_KEY: ${{ secrets.GROQ_API_KEY }}
      SDK_MAINTENANCE_API_KEY: ${{ secrets.SDK_MAINTENANCE_API_KEY }}
```

Allow GitHub Actions to create pull requests in the consuming repository's
Actions settings. Merge its caller/configuration, run it manually once, then
review the outcome before relying on the weekly schedule.

## Providers and limits

| Provider setting | Default model | Key secret | Free-tier considerations |
| --- | --- | --- | --- |
| `google` | `gemini-3.8-flash` | `GEMINI_API_KEY` | Setup testing observed a 20-request daily model quota. Use an unbilled AI Studio project. |
| `groq` | `openai/gpt-oss-120b` | `GROQ_API_KEY` | Published base free limits: 1,000 requests/day, 8K tokens/minute, 200K tokens/day. Use a free-plan organization. |

Google also accepts `gemini-3.6-flash` and `gemini-3.5-flash-lite`; Groq also
accepts `qwen/qwen3.8-27b`. Set `SDK_MAINTENANCE_MODEL` to select one, or leave it
unset to use the provider default. Change the provider and model together.
Check your account's actual limits before enabling a schedule.

Pi provides the agent loop and provider adapters; it does not increase provider
quotas. Google quotas apply per project, and Groq quotas per organization.
Another key in the same quota pool will not add capacity. There is no automatic
provider/key rotation or paid fallback. A billed account can incur charges even
with these model IDs; free-only usage depends on the provider account settings.

Groq runs use short documentation excerpts, small tool result pages, context
compaction, a 61-second gap between requests, and a bounded input/output budget.
Google requests are paced 13 seconds apart. All inference, including summaries,
shares a maximum of 16 requests and a 20-minute agent deadline. A task that
outgrows a free tier stops with a blocker and preserves its partial patch.

The dependency lock pins **Pi 1.3.0**. The SDK loads only explicit file tools,
official-reference fetching and fixed offline Rust checks. It loads no repository
extensions, skills, credentials or shell tools. Provider credentials are removed
from the process environment before source tools and checks run.

## Review and notifications

The proposal job has a read-only GitHub token. A fresh runner checks the patch
boundary and runs formatting, workspace tests/doctests, Clippy, rustdoc, the
configured Rust minimum, package verification and dependency audit. Only a
validated patch reaches a separate publishing job with write access.

The bot opens or updates one maintenance PR, assigns the configured owner, and
dispatches the consuming repository's CI. It never merges, force pushes or
releases. Source patches are limited to 25 regular files and 500 KB; automation,
dependencies and release changes require human help.

New failures create/update one assigned issue with an owner mention; repeated
identical blockers stay quiet. Successful no-ops stay quiet. A later successful
run closes the blocker issue. Enable GitHub assignment/mention notifications.
Completed or partial proposal artifacts remain available for 30 days.

For a live diagnostic, call `.github/workflows/smoke.yml` using the same pinned
SHA, `maintainer_ref`, provider/model and explicitly forwarded secrets. It tests
a disposable failing Rust fixture, requires the agent to edit and rerun tests,
then verifies the implementation with an independent assertion. It cannot
publish source changes. Diagnostics allow six model requests over eight minutes.

## Development

Requires Python 3.9+, Node 24, and Rust for the optional live diagnostic.

```sh
python3 -m unittest discover -s tests -v
npm ci --prefix agent --ignore-scripts
npm test --prefix agent
```

Sources: [Pi SDK](https://pi.dev/docs/latest/sdk),
[Pi providers](https://pi.dev/docs/latest/providers),
[Gemini rate limits](https://ai.google.dev/gemini-api/docs/rate-limits),
[Groq rate limits](https://console.groq.com/docs/rate-limits),
[GitHub reusable workflows](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows).
