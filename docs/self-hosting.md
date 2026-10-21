# Run on your infrastructure

[← Overview](../README.md) · [Use cases](use-cases.md) · [Configuration](configuration.md)

Repo Maintainer is a worker you invoke from code. Your scheduler chooses when it
runs; your server or cloud VM supplies the coding agent and project tools.
The agent process and checks execute on your workers; model inference uses the
selected provider/account. Use self-hosted GitHub runners for the complete PR pipeline, or call the native
binary from cron, systemd, Kubernetes or an existing job system.

## Self-hosted GitHub runners

1. Register a dedicated GitHub Actions runner with the consuming repository or
   organization. Give it a label such as `repo-maintainer`.
2. Install Git, curl, Rust/rustup to build the maintainer, GitHub CLI for publishing,
   and the target project's runtimes. Node projects need Node/npm; Python projects
   need Python; Go projects need Go. The maintainer being written in Rust does not
   require the target repository to contain Cargo files.
3. Commit a task config as `.maintainer.yml`, starting with a [language example](use-cases.md#starting-configurations).
4. Add your agent credentials to the consuming repository's Actions secrets. For
   Codex use `CODEX_API_KEY` (or its existing login on a dedicated worker). The
   builtin runner supports its provider-specific keys and `MAINTENANCE_API_KEY`.
5. Copy [the self-hosted caller](../examples/self-hosted-workflow.yml) into
   `.github/workflows/`. Review the pinned maintainer commit before enabling it, and update both SHA
   references together when upgrading. Match `runner_labels` to your workers.

```yaml
with:
  maintainer_ref: bb53d51b6ec4e7140a5d8d7aed2a78c44899488c
  config_path: .maintainer.yml
  runner_labels: '["self-hosted", "linux", "x64", "repo-maintainer"]'
  install_codex: true
```

These labels apply to every pipeline job. All matching runners need the project
runtimes. Each job checks out source independently; model credentials are passed
only to the agent step, while GitHub writes happen in the publishing job.
`install_codex: true` installs a pinned native Codex release with checksum
verification on Linux/macOS. Set it to `false` if you manage Codex yourself.
The caller's permissions and Actions setting must allow PR creation and CI dispatch.

The included schedule runs Mondays at 9 AM Pacific. Change the schedule in the
caller, or trigger `workflow_dispatch`. A worker on a cloud VM keeps running
while your laptop is off. A worker on your laptop requires the laptop to be on.
Use `runner_labels: '["ubuntu-latest"]'` for GitHub-hosted execution instead.

## Docker with your project's tools

The default Debian image includes the maintainer, native Codex CLI, GitHub CLI,
Python and build tools. New configs select the general Codex worker by default.
Select a Debian-based runtime image for another project. For a Node repository:

```sh
export MAINTAINER_RUNTIME_IMAGE=node:24-bookworm
export CODEX_API_KEY  # Set its value in your shell or the ignored Compose .env.
/path/to/repo_maintainer/maintainer run
```

The wrapper builds the image and mounts the target read-only. The Node image
needs no Cargo toolchain at runtime. Python and Go base images work the same way;
include whatever tools your configured commands require. Debian-based images
with `apt-get` are supported by this Dockerfile. Custom images can also copy the
native executable and install tools independently.

Use `rust:1.89-slim-bookworm` for the bundled Rust validation profile. See
[Docker networking](docker-networking.md) to attach private services, configure
DNS/proxies or mount a custom CA bundle. The worker needs outbound connections;
it publishes no incoming ports.

Set `MAINTAINER_INSTALL_CODEX=false` to omit Codex for a builtin-agent-only image.
The default Codex sandbox is `workspace-write`; keep it when your runtime supports
Codex's sandbox. `agent.codex.sandbox: danger-full-access` is an explicit option
for a worker with external isolation. It grants broader command access inside
that worker; it does not replace your infrastructure's isolation.

Containers do not schedule themselves. Trigger the same image from your job
system, giving it a fresh repository checkout, agent credentials and a writable
artifact directory. A Docker/native `run` produces a patch/report; use the
workflow or the stages below when the job should also publish PRs and blockers.

## Cron and other schedulers

An artifact-only native run can be invoked by a host scheduler:

```cron
# Host timezone applies. Keep /srv/repos/my-app's committed HEAD up to date.
0 9 * * 1 /usr/local/bin/repo-maintainer run --repo /srv/repos/my-app >> /var/log/my-app-maintenance.log 2>&1
```

Configure the job's PATH, credentials and project tools explicitly. Give each run
its own output directory (the CLI default does this), retain its artifacts, and
use your scheduler's overlap/failure policy. The native binary creates no daemon
or timer, and this cron command does not publish a PR. The GitHub caller handles
publishing and assigned blocker notifications for you.

## Compose the stages in code

The public Rust library exposes configuration, runner, validation and workflow
modules. For example, a job system can generate a local proposal:

```rust,no_run
use repo_maintainer::{config::{Config, environment}, workflow};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let config = Config::load(Path::new("/srv/repos/app/.maintainer.yml"), &environment())?;
    workflow::proposal(&config, Path::new("/srv/repos/app"), Path::new("/srv/jobs/run-42"))
}
```

For a full pipeline, compose these CLI stages in separate jobs, using the same
reviewed configuration and transferring artifacts between them:

| Stage | Command | Result |
| --- | --- | --- |
| Prepare | `prepare --output ARTIFACTS` | Source revision, task brief and previous PR feedback |
| Setup | `setup --source SOURCE` | Project dependencies installed with configured commands |
| Agent | `agent --source SOURCE --prompt ARTIFACTS/brief.md` | JSON envelope to retain for `collect` |
| Collect | `collect --source SOURCE --output ARTIFACTS --raw RESULT --exit-code CODE` | Bounded patch and report, including partial work on failure |
| Apply | `apply --source FRESH_SOURCE --output ARTIFACTS` | Exact patch applied to its original revision |
| Validate | `validate --source FRESH_SOURCE --output ARTIFACTS` | Setup/checks run; `validation.json` binds passing checks to patch/config |
| Publish | `publish --source ANOTHER_FRESH_SOURCE --output ARTIFACTS` | Validated PR opened/updated and CI dispatched |
| Notify | `notify --output ARTIFACTS` | New blocker or recovery reported; no-op/repeated blocker stays quiet |

`prepare`, `publish` and `notify` use the GitHub environment described by the
[reusable workflow](../.github/workflows/maintain.yml): repository, default branch,
run ID/attempt/URL, owner and stage results. The model does not receive the
publisher's GitHub token. An unchanged proposal skips apply/validate/publish.
Keep patch, report, request and validation receipt together when transferring jobs.
