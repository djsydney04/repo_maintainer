# Configuration reference

[← Overview](../README.md) · [Get started](getting-started.md) · [Troubleshooting](troubleshooting.md)

Use [maintainer.example.yml](../maintainer.example.yml) as a commented starting
point. Configuration is data: store API keys separately in environment variables
or GitHub Actions secrets. Unknown fields and duplicate keys are rejected.

## Minimal configuration

```yaml
version: 1
agent:
  provider: groq
sources:
  - https://developers.openai.com/api/docs/changelog
instructions: |
  Keep the SDK aligned with the docs. Preserve compatibility.
```

Local discovery checks these names, in order: `.maintainer.yml`,
`.maintainer.yaml`, `.github/maintainer.yml`, `.github/maintainer.yaml`, and
`.github/maintainer.json`. An explicit `--config` path overrides discovery.
The cloud caller's `config_path` selects the file; set it to `.maintainer.yml`
for YAML. The reusable workflow defaults to `.maintainer.yml`.

Version-1 JSON uses the same field names and validation rules. Configuration is
limited to 256 KB; aliases are bounded. Paths must be relative to the repository.

## Model providers

| `agent.provider` | Default model | Key | Explicit paid opt-in |
| --- | --- | --- | --- |
| `groq` | `openai/gpt-oss-120b` | `GROQ_API_KEY` | No |
| `google` | `gemini-3.8-flash` | `GEMINI_API_KEY` | No |
| `openai` or `oai` | `gpt-5.4` | `OPENAI_API_KEY` | Yes |
| `anthropic` | `claude-sonnet-4-6` | `ANTHROPIC_API_KEY` | Yes |

These are defaults bundled in the Rust binary, not aliases for the newest model.
Google also accepts `gemini-3.6-flash` and `gemini-3.5-flash-lite`. Groq also
accepts `qwen/qwen3.8-27b`. OpenAI and Anthropic accept model IDs available to your
provider account. OpenAI uses the Responses API; Anthropic uses the Messages
API. All four adapters make native HTTP requests from Rust.

```yaml
agent:
  provider: anthropic
  model: claude-sonnet-4-6
  allow_paid: true
  max_requests: 12
```

| Agent field | Default | Meaning |
| --- | --- | --- |
| `provider` | `groq` in YAML | Provider to call |
| `model` | Provider default | Explicit model ID |
| `allow_paid` | `false` | Must be `true` for OpenAI/Anthropic |
| `max_requests` | `16` | Inference budget, from 1 through 16; includes summaries |

`SDK_MAINTENANCE_API_KEY`, if set, overrides the selected provider's named key.
Replace the appropriate secret to change keys; remove the generic override to
resume using provider-specific keys. The maintainer does not rotate providers
or keys automatically.

Use a Groq free-plan organization or an unbilled Google project for free-only
inference. A billed account can incur charges even for these same model IDs.
Keys within one project/organization share its quota. Provider account limits
still apply; the maintainer does not add capacity.

### Overrides for one run

Optional cloud `provider` and `model` inputs override YAML. The `allow_paid`
input explicitly enables paid usage for that run. Environment overrides are
`MAINTENANCE_PROVIDER`, `MAINTENANCE_MODEL` and `MAINTENANCE_ALLOW_PAID`
(`true` or `false`). An explicit provider override selects that provider's
default model unless a compatible model is also specified.

## Sources and instructions

| Field | Default | Meaning |
| --- | --- | --- |
| `version` | `1` | Configuration format |
| `name` | `Repository maintenance` | Short display name |
| `sources` | Required | HTTPS URLs to official docs/changelogs |
| `official_hosts` | Hosts inferred from sources | Allowed reference and evidence hosts |
| `instructions` | Focused, compatible maintenance task | Inline instructions; at most 16,000 characters |
| `instructions_file` | Unset | Optional repository-relative file appended to inline instructions |

Use `official_hosts` when a provider's documentation links to another official
host. Every source must match an allowed host. URLs cannot contain credentials.
The reference tool and final source evidence use the same host boundary.

Describe the compatibility requirements and priority areas in your task. The
runner adds its result format and basic boundaries automatically. Fetched docs,
source comments and PR feedback remain untrusted evidence.

## Editable paths

```yaml
editable:
  root_files: [README.md]
  markdown_dirs: [docs]
  source_dirs: [src, crates]
  extensions: [.rs, .md, .json]
```

Those values are the defaults. `root_files` allows the named files;
`markdown_dirs` allows Markdown files directly in those directories;
`source_dirs` allows the listed extensions recursively. The final patch guard
rejects hidden/traversal paths, symlinks, executable files, more than 25 files
or more than 500 KB. Keep manifests, dependency files, workflows and release
files outside the editing boundary; request human help for those changes.

## Rust checks

```yaml
rust:
  msrv: 1.88.0
  crates: [my-sdk-core, my-sdk-openai]
  package_check: scripts/verify-packages
```

| Field | Default | Meaning |
| --- | --- | --- |
| `msrv` | `1.88.0` | Minimum Rust toolchain for independent validation |
| `crates` | `[]` | Package names allowed for scoped agent checks; whole-workspace checks remain available |
| `package_check` | Empty | Optional executable in the target repository; it provides its own interpreter or compiled binary |

The cloud validator also runs formatting, workspace tests/doctests, Clippy,
rustdoc and dependency audit. The agent's repair tools are bounded offline
format/test/Clippy commands, with dependencies fetched before inference.

## GitHub publishing

| Field | Default | Meaning |
| --- | --- | --- |
| `branch_prefix` | `dylan/repo-maintenance-` | Prefix for run-specific proposal branches; end it with `-` |
| `issue_title` | `Repository maintenance needs help` | Title for the single blocker issue |
| `commit_title` | `fix: maintain documented SDK compatibility` | Commit and PR title |
| `ci_workflow` | `ci.yml` | Existing workflow dispatched for the proposed branch |

Set the `owner` input on the reusable workflow to choose the PR/issue assignee.
The target CI workflow must support `workflow_dispatch`, because PRs made with
`GITHUB_TOKEN` do not start ordinary PR workflows automatically.
