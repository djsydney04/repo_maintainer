# Configuration reference

[← Overview](../README.md) · [Get started](getting-started.md) · [Self-hosting](self-hosting.md) · [Use cases](use-cases.md)

Store the task, agent, editable paths and checks in `.maintainer.yml`. Keys belong
in the worker environment or Actions secrets. Unknown fields, duplicate keys,
unsafe paths and excessive budgets fail validation.

## A repository task

```yaml
version: 2
name: Application maintenance
agent:
  runner: codex
  allow_paid: true
instructions: |
  Fix one reproducible bug, preserve public behavior and add a regression test.
editable:
  source_dirs: [src, tests]
  extensions: [.ts, .tsx, .js, .md, .json]
validation:
  profile: custom
  checks:
    - name: tests
      command: [npm, test]
```

This example expects an npm test script and Node/npm on the worker. Use the
[language examples](use-cases.md#starting-configurations) for a starting point;
set the editable paths to the files you want maintained.

Discovery checks `.maintainer.yml`, `.maintainer.yaml`, `.github/maintainer.yml`,
`.github/maintainer.yaml` and `.github/maintainer.json`, in that order. `--config`
overrides discovery. Version-1 JSON is supported with the same fields. The shared
workflow retains the legacy `.github/maintainer.json` default; explicitly pass
`config_path: .maintainer.yml` for YAML. Config files are limited to 256 KB and
YAML alias expansion is bounded.

## Choose a coding agent

| Field | Schema default | Meaning |
| --- | --- | --- |
| `agent.runner` | `codex` in version 2 | `codex` launches the Codex CLI; `builtin` uses the Rust agent with a model API |
| `agent.model` | Runner/provider default | Optional model ID; omitted for Codex to use its CLI default |
| `agent.allow_paid` | `false` | Explicit account-usage opt-in; required for Codex, OpenAI and Anthropic |
| `agent.timeout_seconds` | `1200` | Agent wall-clock limit, from 1 through 3600 seconds |
| `agent.max_requests` | `16` | Builtin inference budget, from 1 through 16, including compaction; does not limit Codex turns |
| `agent.codex.binary` | `codex` | Worker-owned executable on PATH or an absolute path |
| `agent.codex.sandbox` | `workspace-write` | `read-only`, `workspace-write`, or explicit `danger-full-access` for externally isolated workers |
| `agent.codex.network_access` | `true` in version 2 | Allow outbound access for workspace-write tools; explicit `false` restricts it |

`init` writes version 2. If omitted there, `agent.runner` is Codex, validation is
a generic whitespace check, and editable paths include common source/test
extensions. Account use still requires `agent.allow_paid: true`. Configure the
project's actual checks before unattended work. Version 1, including files with
no version field, retains its builtin runner, Rust validation and restricted
Codex tool networking defaults. Explicit settings always take precedence.

### Codex CLI

Install Codex on the worker, or enable the workflow's `install_codex` input.
The Docker image includes the native CLI by default. Authenticate with the
worker's Codex login or `CODEX_API_KEY`; `OPENAI_API_KEY` is accepted as a fallback
key for this runner. `CODEX_HOME` can select a worker-owned authentication home.
Other model keys and GitHub tokens are not forwarded to the Codex process.

The adapter uses non-interactive execution, a JSON output schema, an execution
deadline and the selected sandbox. User config is ignored for this invocation;
authentication is still available. Shell commands launched by Codex exclude the
API key environment variables. The agent's changed files still pass the same
patch guard and independent project checks before publishing.
[Official Codex automation documentation](https://learn.chatgpt.com/docs/non-interactive-mode)
describes CLI authentication, sandboxes and structured output.

### Model providers

```yaml
agent:
  runner: builtin
  provider: groq
  max_requests: 12
```

| `agent.provider` | Bundled default model | Key | Paid opt-in |
| --- | --- | --- | --- |
| `groq` | `openai/gpt-oss-120b` | `GROQ_API_KEY` | No |
| `google` | `gemini-3.8-flash` | `GEMINI_API_KEY` | No |
| `openai` or `oai` | `gpt-5.4` | `OPENAI_API_KEY` | Yes |
| `anthropic` | `claude-sonnet-4-6` | `ANTHROPIC_API_KEY` | Yes |

These defaults come from the bundled provider profiles. Groq also accepts
`qwen/qwen3.8-27b`; Google also accepts `gemini-3.6-flash` and
`gemini-3.5-flash-lite`. OpenAI and Anthropic accept model IDs available to your
account. The adapters use native HTTP APIs from Rust.

`MAINTENANCE_API_KEY` overrides the selected provider key.
`SDK_MAINTENANCE_API_KEY` remains a legacy alias, after the new override and before
the provider key. These overrides apply to the builtin runner. There is no
provider/key rotation or paid fallback. Actual charges and quotas depend on the
account, including billed Groq/Google accounts.

Optional environment overrides are `MAINTENANCE_RUNNER`, `MAINTENANCE_PROVIDER`,
`MAINTENANCE_MODEL` and `MAINTENANCE_ALLOW_PAID` (`true`/`false`). Changing a runner
or provider clears its previous model selection unless a model override is given.
The Codex runner uses its own authentication; provider selection is for builtin.

## Task and reference sources

| Field | Default | Meaning |
| --- | --- | --- |
| `version` | `2` in new files; absent means legacy `1` | Configuration format |
| `name` | `Repository maintenance` | Short display name |
| `instructions` | Focused maintenance task | Task text, at most 16,000 characters |
| `instructions_file` | Unset | Repository-relative file appended to the task |
| `sources` | `[]` | Optional HTTPS docs/changelogs fetched before inference |
| `official_hosts` | Hosts inferred from sources | Allowed reference and evidence hosts |

Repository-only tasks need no external URL. With configured sources, every URL
must use an allowed host and a changed result must cite reference evidence.
Without sources, a result can cite no URL. Source text, docs and PR feedback are
untrusted evidence; the trusted task/config defines the job. URLs cannot contain
credentials. Add official hosts when documentation redirects to another domain.

## Checks for any language

```yaml
validation:
  profile: custom
  setup:
    - name: dependencies
      command: [npm, ci]
      timeout_seconds: 600
  checks:
    - name: tests
      command: [npm, test]
      working_directory: .
      timeout_seconds: 300
    - name: lint
      command: [npm, run, lint]
```

`custom` requires at least one check. Names are unique simple identifiers;
commands are argument arrays, executed directly without an implicit shell.
`working_directory` defaults to `.` and must stay inside the repository without
symlink traversal. A command's timeout defaults to 300 seconds and can be from
1 through 1800. Each setup/check list has at most 20 entries.

Setup runs before inference and again in the fresh validation checkout. Checks
run independently before publication; the builtin agent can also call them by
name during repair. The model cannot invent a new command for the builtin check
tool. Child commands receive a restricted environment without model/GitHub keys,
and `CI=true`. If a shell is needed, the operator must explicitly configure a
command such as `[sh, -c, 'your command']`.

Install the project runtimes on every worker or select an appropriate
[Docker base image](self-hosting.md#docker-with-your-projects-tools). Native setup
can depend on tools such as npm, Python or Go without depending on Cargo.

Run `repo-maintainer validate --repo /path/to/project` to execute setup and
checks directly, without inference. It prints passing check names; it does not
create a publication receipt. In a pipeline, `validate --source FRESH_SOURCE
--output ARTIFACTS` validates an applied proposal and writes that receipt.

### Rust checks

`validation.profile: rust` retains Cargo dependency setup, formatting, workspace
tests/doctests, Clippy, rustdoc, the minimum Rust version, optional packaging and
audit. Additional configured checks run afterwards. Its settings remain:

```yaml
rust:
  msrv: 1.88.0
  crates: [my-library]
  package_check: scripts/verify-packages
```

`crates` defaults to `[]` and permits scoped builtin-agent checks. `package_check`
defaults to empty; when supplied it is a repository-relative executable with its
own interpreter or compiled binary. The workflow installs Rust and cargo-audit
for this profile. Custom profiles do not install or invoke Cargo for the target.

## Editable paths

```yaml
editable:
  root_files: [README.md, CHANGELOG.md]
  source_dirs: [src, tests, docs]
  markdown_dirs: [docs]
  extensions: [.ts, .tsx, .js, .py, .go, .rs, .md, .json]
```

Root files are exact names. Source directories allow the selected extensions
recursively; Markdown directories allow immediate `.md` children. Version 2
defaults cover `src/`, `tests/` and `crates/` with common language extensions;
version 1 keeps the original Rust-oriented defaults. Allow manifests/lockfiles only
when the task should change them. Hidden paths, traversal, symlinks and executable
files cannot enter a proposal. Patches stay within 25 regular files and 500 KB.

## Publishing and scheduling

| Field | Default | Meaning |
| --- | --- | --- |
| `branch_prefix` | `dylan/repo-maintenance-` | Run-specific branch prefix, ending in `-` |
| `issue_title` | `Repository maintenance needs help` | Deduplicated blocker issue title |
| `commit_title` | `fix: maintain repository` | Commit and PR title |
| `ci_workflow` | `ci.yml` | Target workflow dispatched for the maintenance branch |

A scheduler belongs in your workflow/job system, not inside this task file.
Use distinct config paths, branch prefixes and issue titles for independent
recurring jobs in the same repository. The workflow's concurrency group includes
`config_path`. Its `owner` input selects the issue/PR assignee, and
`runner_labels` chooses your self-hosted infrastructure or GitHub-hosted workers.
The target CI workflow must support `workflow_dispatch`.
