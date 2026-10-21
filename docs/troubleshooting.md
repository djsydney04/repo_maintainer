# Troubleshooting

[← Overview](../README.md) · [Get started](getting-started.md)

Start with `repo-maintainer check` for the native executable, or `maintainer check`
through the Docker wrapper. Both validate YAML without a model call or key.
For cloud failures, open the linked Actions run and its retained proposal.

| Symptom | What to check |
| --- | --- |
| `docker` or Compose is missing | Install/start Docker with the Compose plugin on the computer running local jobs. Cloud runs do not need local Docker. |
| Native command or Rust checks are missing | Install the maintainer with `cargo install --locked --path .` using Rust 1.89 or newer. Add Cargo's binary directory to `PATH`; install Git, rustfmt and Clippy. |
| Configuration not found | Run from the target repository, use `maintainer init`, or pass the correct `--config` path. Cloud `config_path` is relative to the consuming repo. |
| Unknown or duplicate YAML field | Compare with the [field reference](configuration.md). Settings are validated; misspelled keys are not ignored. |
| Selected key missing or rejected | Check the provider-specific environment variable/Actions secret. A generic `SDK_MAINTENANCE_API_KEY` overrides named keys. |
| OpenAI/Anthropic rejects the configuration | Set `agent.allow_paid: true` explicitly, select an available model and supply that provider's key. |
| Model is unsupported or inaccessible | Groq/Google models must be allowed by the bundled provider profile. For every provider, confirm the selected model is available to your account. |
| HTTP 429 / quota exhausted | Inspect provider account limits and wait for the relevant reset. Another key in the same quota pool does not add capacity. |
| Context or request budget exceeded | Narrow the maintenance task or sources. Groq's free token budget needs smaller work; partial proposals are retained. |
| Source fetch fails | Check that the source is an official HTTPS page, its host is allowed, and it returns readable documentation. |
| A file cannot be edited | Check `editable` paths and the [proposal boundary](architecture.md#limits-and-incomplete-work). Dependencies/automation may require human help. |
| Local output permission denied | Prefer the wrapper, which supplies your user ID and creates the output directory. For direct Compose, use a writable bind path and matching UID/GID. |
| Local changes are missing from a run | Native and Docker runs start from committed HEAD. Commit the source changes you want it to inspect. |
| Native run does not find a key in `.env` | Export the key into your shell. Only Docker Compose loads the shared checkout's `.env` automatically. |
| Local run did not open a PR | Local runs generate patches/reports. Configure the [weekly cloud workflow](getting-started.md#weekly-cloud-maintenance) for hosted PRs. |
| Cloud workflow did not start | Merge the caller to the default branch, check Actions settings and try a manual run. GitHub schedules can be delayed. |
| PR creation or CI dispatch fails | Check the caller permissions, Actions' PR-creation setting, and that `ci_workflow` supports manual dispatch. |
| Checks failed after editing | Review the logs and retained patch. A failed or incomplete proposal does not reach publishing. |
| No notification arrives | Enable GitHub assignment/mention notifications. Successful no-ops and identical blockers are intentionally quiet. |

## Sharing a useful report

Use the [bug report form](https://github.com/djsydney04/repo_maintainer/issues/new?template=bug_report.yml)
with the pinned maintainer commit, execution mode, provider/model, expected and
actual behavior, and a minimal reproduction. Include a link to a failed run if
it is public. Remove API keys, tokens and private repository content from logs
and configuration before sharing them.
