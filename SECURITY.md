# Security policy

Security fixes are developed on `main`. Upgrade to the latest commit before
reporting a problem; older releases do not have a separate support window.

Use [GitHub private vulnerability reporting](https://github.com/djsydney04/repo_maintainer/security/advisories/new)
for credential exposure, source boundary escapes or publishing unvalidated changes.
Include a minimal reproduction, affected commit and impact. Do not include live
credentials or publish an exploit in a public issue.

The model can read visible source, edit configured regular files, fetch official
documentation and run fixed offline Cargo checks. Model output is untrusted.
Unknown config fields, hidden paths, traversal, symlinks and arbitrary commands
are rejected. Patches are checked again in a fresh job before publishing.

Cargo checks execute repository code and build scripts. Offline mode and a clean
child environment are safeguards, not an operating system sandbox. Maintain only
repositories you trust on dedicated infrastructure. Child processes can access
files and network resources permitted to that user; do not mount credential stores
or run unrelated privileged jobs alongside them.

Local runs mount the target read-only and operate on its committed HEAD in a
temporary clone. Only proposal artifacts are returned. GitHub publishing is an
explicit workflow command in a separate job with write permissions. The
documentation maintenance workflow does not merge PRs, force push, deploy or
release maintained repositories.

The separate, opt-in dependency runner launches the official Renovate image,
pinned by digest, with its own repository token. Preview is the default; live
updates and merging require `--apply`. The supplied central configuration keeps
major updates for review and requires passing checks before Renovate merges.
The runner mounts its config read-only and a persistent cache, without a Docker
socket, host checkout or model credentials. Renovate clones repositories and can
run package managers inside that container; maintain only trusted repositories
on infrastructure dedicated to this workload. Operator configuration controls
Renovate's execution and merge policy. See the
[self-hosted dependency setup](docs/dependency-updates.md).
