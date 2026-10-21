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

The separate, opt-in dependency merge action can request merging of verified
Dependabot PRs through GitHub's required checks and reviews. It never checks out
or executes PR contents, approves reviews, or requests an administrator bypass.
The Renovate preset delegates dependency merging to Renovate. See the
[dependency setup and trust boundary](docs/dependency-updates.md).
