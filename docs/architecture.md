# How it works

[← Overview](../README.md) · [Configuration](configuration.md)

Repo Maintainer turns a scheduled repository task into a focused proposal.
A Rust orchestrator selects Codex CLI or the built-in agent, carries the patch
through independent project checks, and publishes a PR. The scheduler and
worker can live on your own infrastructure. References are optional; Rust SDK
maintenance is one supported task profile.

## Scheduled workflow

```mermaid
flowchart LR
    A["Task + optional references + PR feedback"] --> B["Propose: inspect, edit, repair"]
    B --> C["Fresh runner: validate exact patch"]
    C --> D["Publish: open or update a PR"]
    B --> E["New blocker: assigned issue"]
    C --> E
    D --> F["Human review and merge"]
    style B fill:#201a3d,stroke:#a78bfa,color:#f5f3ff
    style C fill:#102a2a,stroke:#54dfbb,color:#ecfdf5
    style D fill:#2b192a,stroke:#f49bcb,color:#fdf2f8
```

| Stage | Work | Access |
| --- | --- | --- |
| Propose | Read official docs and review/CI feedback; inspect, edit and repair files; export a patch/report | Selected model key and read-only GitHub token |
| Validate | Apply the patch to its original revision; run configured setup/checks; record patch/config hashes | Fresh runner; no model key or GitHub write token |
| Publish | Reapply the validated patch, commit, open/update one assigned PR, dispatch target CI | Short-lived GitHub write token; no model key |
| Notify | Report a new failure, stay quiet for an identical blocker, close the issue on recovery | Issue-write permission |

The builtin agent has file/reference tools and a named check tool. Its commands
come from trusted configuration, and the Rust profile retains fixed offline
Cargo repair checks. Codex uses its CLI tools and shell under the selected
sandbox instead; it returns a schema-validated result. Both feed the same
proposal guard. GitHub tokens are not passed to Codex; check/setup commands use
a restricted environment without model/GitHub keys. See [execution boundaries](../SECURITY.md).

Publishing requires `validation.json`: a successful check list bound to source
revision, patch hash and resolved configuration hash. A failed validation attempt
removes any previous receipt. Checks that rewrite the proposed files fail
validation. The publishing job verifies the receipt before applying the patch.

The bot never force pushes, merges, releases or deploys. A human reviews the
resulting PR. Existing maintenance PRs receive review/CI feedback on later runs.

## Limits and incomplete work

| Boundary | Limit |
| --- | --- |
| Proposed patch | 25 regular files; 500 KB |
| Builtin model requests | At most 16, including summaries; configurable downward |
| Agent deadline | 20 minutes by default; configurable from 1 second to 60 minutes |
| One builtin Rust check | 5 minutes |
| Configured setup/check command | 5 minutes by default; configurable up to 30 minutes |
| Live diagnostic | 6 requests; 8-minute agent deadline |
| Cloud proposal retention | 30 days |

Groq uses small context pages, short excerpts, compaction and 61-second request
pacing for its free token budget. Google requests are paced 13 seconds apart.
Account quotas still apply. There is no automatic model/provider/key fallback.
A task that exceeds the budget stops and retains its partial patch for review.

A new blocker creates/updates one assigned issue with an owner mention. An
identical blocker does not repeatedly mention the owner. Successful no-ops stay
quiet; a later successful run closes the blocker issue. GitHub notification
preferences control delivery.

## Local execution

The container reads the target repository's committed HEAD through a read-only
mount, creates a temporary working copy, fetches Rust dependencies, and asks the
selected coding agent for a proposal. A writable output mount receives the patch and report.
You decide whether to apply it. Local Docker runs do not publish PRs or create
a recurring schedule.

The native `repo-maintainer run` command uses the same temporary-clone and patch
workflow. It requires Git, your selected agent and project check tools in the worker environment.
Unlike Compose, it does not load a `.env` file automatically.

The Docker image contains the maintainer and native Codex CLI; its base image
supplies the project runtime. Rust is the default; Node/Python/Go images are configurable. API keys are supplied at
runtime, and the build context excludes local environment files. The wrapper
handles user IDs, repository/output paths and preserving an existing config.

## Deployment and model tests

The shared CI checks formatting, linting and Rust tests on Linux and macOS with
stable Rust and Rust 1.89, then builds/runs Rust and Node Docker images.
The Node runtime is checked without Cargo installed. Tests exercise HTTP
authentication, provider tool continuations, real offline Cargo repair, patch
transfer and notification behavior with simulated model responses. Codex tests
use a simulated CLI process to exercise structured output, credential filtering,
non-Rust patch transfer, checks, timeouts and stale-receipt rejection.
These offline tests do not demonstrate account quota or live model quality.
The optional `smoke.yml` instead verifies a real key, file editing and test repair
in a disposable Rust fixture; paid providers require explicit opt-in and keys.

## Your scheduler and infrastructure

The reusable workflow accepts `runner_labels` for GitHub-hosted or self-hosted
workers. Native/Docker commands can also be invoked by cron, systemd or a job
queue. They do not create a timer or daemon. See [self-hosting](self-hosting.md)
for the full PR pipeline and stage-by-stage integration.
