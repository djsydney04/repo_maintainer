# Security policy

Security fixes are developed on `main`. Upgrade to the latest commit before
reporting a problem; older releases do not have a separate support window.

Use [GitHub private vulnerability reporting](https://github.com/djsydney04/repo_maintainer/security/advisories/new)
for credential exposure, source boundary escapes or publishing unvalidated changes.
Include a minimal reproduction, affected commit and impact. Do not include live
credentials or publish an exploit in a public issue.

The builtin agent can read visible source, edit configured regular files, fetch
allowed references and run named operator-configured checks. It cannot introduce
an arbitrary command through the check tool. The Codex runner instead exposes
the Codex CLI's coding/shell tools under the configured execution policy. Native
runs default to `workspace-write`. Version-2 Compose runs explicitly use
`danger-full-access` with Docker supplying isolation: an unprivileged user,
read-only root filesystem, temporary home/workspace storage, dropped Linux
capabilities and no privilege escalation. Explicit YAML sandbox settings take
precedence. Its capabilities differ from the builtin tool allowlist.

Model output is untrusted. Unknown config fields, hidden paths, traversal,
symlinks and executable-file changes are rejected from proposals. A fresh job
applies the patch and runs the trusted configuration's checks. Publishing
requires a passing validation receipt bound to the exact patch and config.

Project checks and dependency setup execute repository code and build scripts.
Commands are operator-defined argv arrays, without an implicit shell. A clean
child environment is a safeguard, not an operating system sandbox. Maintain only
repositories you trust on dedicated infrastructure. Child processes can access
files and network resources permitted to that user; do not mount credential stores
or run unrelated privileged jobs alongside them.

Model/GitHub credentials are removed from setup/check child environments. Codex
receives only its selected API key/auth home, without GitHub token variables;
its shell environment excludes its API-key variables. These environment rules do
not isolate credential files or other processes running as the same OS user.
Use a dedicated account/worker and do not mount unrelated credential stores.

Version-2 configurations allow the worker's outbound network by default.
`agent.codex.network_access` restricts tools when using workspace-write; Docker
network policy controls access in container isolation mode. A request for
`network_access: false` selects workspace-write unless YAML explicitly selects a
different sandbox. Proxy and certificate settings are forwarded to project checks and
Codex. Proxy credentials are therefore available to those commands. Native HTTP
clients add a configured CA bundle while retaining certificate verification.
Private Docker networks are opt-in and use no privileged mode or Docker socket.

Local Docker runs mount the target read-only and operate on its committed HEAD in a
temporary clone. Only proposal artifacts are returned. GitHub publishing is an
explicit workflow command in a separate job with write permissions. The project
does not merge PRs, force push, deploy or release maintained repositories.
