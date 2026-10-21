# Get started

[← Overview](../README.md) · [Configuration](configuration.md) · [Troubleshooting](troubleshooting.md)

Choose the native executable or Docker for a reviewable local patch, or a weekly
GitHub workflow for tested pull requests. All use the same `.maintainer.yml` configuration.
Use [self-hosted runners](self-hosting.md) for the complete pipeline on your own
infrastructure. Choose a [task/language example](use-cases.md#starting-configurations).

## Native executable

Install Rust 1.89 or newer and Git, then build the command from this repository:

```sh
git clone https://github.com/djsydney04/repo_maintainer.git
cd repo_maintainer
cargo install --locked --path .
cd /path/to/your-repository
repo-maintainer init
# Configure the task, project checks and agent account.
repo-maintainer check
repo-maintainer run
```

The executable uses your shell environment. Docker Compose alone loads the
shared checkout's `.env` file. Install Git, the chosen coding agent, and the project tools used by your checks.
Codex uses its worker login or `CODEX_API_KEY`; the builtin agent uses its selected
model provider key. Rust target checks need rustfmt/Clippy; custom profiles can
use Node, Python, Go or other project tools without Cargo files.

`run` clones committed HEAD into a temporary directory and leaves your working
tree alone. Review `.maintainer-output/<run>/report.json` and `proposal.patch`.
Use `--output /path/to/new-directory` to choose an empty output directory.
Local runs generate proposals without publishing PRs or creating schedules.

## Local Docker runs

You need Git and Docker with the Compose plugin. Clone this project beside the
repository you want to maintain:

```sh
git clone https://github.com/djsydney04/repo_maintainer.git
cd /path/to/your-repository
/path/to/repo_maintainer/maintainer init
```

Edit the generated `.maintainer.yml`: describe the maintenance task, select the
agent, set project checks, and review the editable paths. External documentation
sources are optional. Supply the selected agent's
key in your shell environment or in the shared checkout's ignored `.env`
file, using [`.env.example`](../.env.example) as the starting point. Keep real
keys out of your repository and Dockerfile.

```sh
/path/to/repo_maintainer/maintainer check
/path/to/repo_maintainer/maintainer run
```

`check` validates configuration without inference or an API key. `run` works
from the target repository's **committed HEAD**: commit the source changes you
want it to consider first. The target mount is read-only; the wrapper handles
user IDs and creates `.maintainer-output/<run>/` for the report and patch.

Review `report.json` and `proposal.patch`. To apply a changed proposal yourself,
run from the target repository:

```sh
git apply --check .maintainer-output/<run>/proposal.patch
git apply .maintainer-output/<run>/proposal.patch
```

Add `.maintainer-output/` to the target repository's `.gitignore`. A local run
generates a proposal; it does not open a GitHub PR or install a schedule.

### Direct Compose or Docker

From the shared checkout, provide explicit paths:

```sh
REPOSITORY_PATH=/path/to/your-repository \
MAINTAINER_OUTPUT_PATH=/path/to/output \
MAINTAINER_UID=$(id -u) MAINTAINER_GID=$(id -g) \
  docker compose run --build --rm maintainer check
```

Create the output directory first. Use `run` to generate a proposal, or `smoke`
to exercise a live repair fixture with your selected provider. The wrapper is
the convenient way to handle paths and permissions automatically.

For a direct image, build `Dockerfile` and mount the target at `/repository`
read-only and a writable directory at `/output`. Pass keys at runtime; do not
add them to image layers. The default command is `check`. The default image includes native Codex; set
`MAINTAINER_RUNTIME_IMAGE` to a Debian-based Node/Python/Go image when needed.
See [container configuration](self-hosting.md#docker-with-your-projects-tools).

## Weekly cloud maintenance

The workflow runs on **GitHub-hosted Linux**. Your Mac or other development
computer can be off. The consuming repository owns its schedule and keys.

1. Commit `.maintainer.yml` in the repository to maintain.
2. Add the selected API key in that repository's **Settings → Secrets and
   variables → Actions → Secrets**. See [provider/key names](configuration.md#model-providers).
3. Allow GitHub Actions to create pull requests in the consuming repository's
   Actions settings. The bot creates PRs; it never approves or merges them.
4. Add a caller such as `.github/workflows/maintenance.yml`:

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
      install_codex: true
    secrets:
      CODEX_API_KEY: ${{ secrets.CODEX_API_KEY }}
      MAINTENANCE_API_KEY: ${{ secrets.MAINTENANCE_API_KEY }}
      GROQ_API_KEY: ${{ secrets.GROQ_API_KEY }}
      GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
      OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
      ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
      SDK_MAINTENANCE_API_KEY: ${{ secrets.SDK_MAINTENANCE_API_KEY }}
```

Replace both `FULL_COMMIT_SHA` placeholders with the **same reviewed full SHA**
that includes the selected runner and validation features. A change to the shared
repository only affects your bot after you update that pin.

The example runs Mondays at 9 AM Pacific, including DST. Change the cron and
timezone to suit your repository. GitHub can delay scheduled runs.

Merge the configuration and caller, manually start **Weekly maintenance** once,
and review its result. The bot runs only from the repository's default branch.
Enable GitHub assignment/mention notifications to receive new blocker issues.
Its `owner` workflow input defaults to the repository owner and can be changed.

The CI workflow named in configuration must exist in the target repository.
When configured, `rust.package_check` must be an executable there, with its own
interpreter or compiled binary. For unpublished workspace crates, use your
repository's existing packaging verifier. See [Rust settings](configuration.md#rust-checks).
