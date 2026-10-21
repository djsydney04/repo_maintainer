# Get started

[← Overview](../README.md) · [Configuration](configuration.md) · [Troubleshooting](troubleshooting.md)

Choose the native executable or Docker for a reviewable local patch, or a weekly
GitHub workflow for tested pull requests. All use the same `.maintainer.yml` configuration.
The current profile supports Rust SDK workspaces.

## Native executable

Install Rust 1.89 or newer and Git, then build the command from this repository:

```sh
git clone https://github.com/djsydney04/repo_maintainer.git
cd repo_maintainer
cargo install --locked --path .
cd /path/to/your-rust-repository
repo-maintainer init
# Edit .maintainer.yml and export the selected provider's API key.
repo-maintainer check
repo-maintainer run
```

The executable uses your shell environment. Docker Compose alone loads the
shared checkout's `.env` file. Native runs require Rust and Git at runtime for
repository checks; they use neither Python nor Node. Install rustfmt and Clippy
for the toolchain you use. The maintained repository may have its own optional
package-check executable with additional requirements.

`run` clones committed HEAD into a temporary directory and leaves your working
tree alone. Review `.maintainer-output/<run>/report.json` and `proposal.patch`.
Use `--output /path/to/new-directory` to choose an empty output directory.
Local runs generate proposals without publishing PRs or creating schedules.

## Local Docker runs

You need Git and Docker with the Compose plugin. Clone this project beside the
repository you want to maintain:

```sh
git clone https://github.com/djsydney04/repo_maintainer.git
cd /path/to/your-rust-repository
/path/to/repo_maintainer/maintainer init
```

Edit the generated `.maintainer.yml`: replace the docs sources, describe the
maintenance task, and review the editable paths. Select a provider and supply
its key in your shell environment or in the shared checkout's ignored `.env`
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
REPOSITORY_PATH=/path/to/your-rust-repository \
MAINTAINER_OUTPUT_PATH=/path/to/output \
MAINTAINER_UID=$(id -u) MAINTAINER_GID=$(id -g) \
  docker compose run --build --rm maintainer check
```

Create the output directory first. Use `run` to generate a proposal, or `smoke`
to exercise a live repair fixture with your selected provider. The wrapper is
the convenient way to handle paths and permissions automatically.

For a direct image, build `Dockerfile` and mount the target at `/repository`
read-only and a writable directory at `/output`. Pass keys at runtime; do not
add them to image layers. The default command is `check`.

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
    uses: djsydney04/repo_maintainer/.github/workflows/maintain.yml@7038a20127fd1635f9de7f82c4ef952eb8712ca9
    with:
      maintainer_ref: 7038a20127fd1635f9de7f82c4ef952eb8712ca9
      config_path: .maintainer.yml
    secrets:
      GROQ_API_KEY: ${{ secrets.GROQ_API_KEY }}
      GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
      OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
      ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
      SDK_MAINTENANCE_API_KEY: ${{ secrets.SDK_MAINTENANCE_API_KEY }}
```

This example pins a published Rust implementation commit. Keep `uses` and
`maintainer_ref` on the **same full SHA** when updating. A change to the shared
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
