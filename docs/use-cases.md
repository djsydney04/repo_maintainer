# Use cases

[← Overview](../README.md) · [Run on your infrastructure](self-hosting.md) · [Configuration](configuration.md)

Repo Maintainer schedules coding-agent work against a repository. A task defines
what to improve, which files may change, which agent to use, and which checks must
pass. The same pipeline can maintain an application, library, documentation site
or SDK. The scheduler supplies the cadence; your machines execute the work.

## Choose a task

| Use case | Example recurring task | Checks to configure |
| --- | --- | --- |
| Application upkeep | Find a small bug, add a regression test and propose the fix | Application tests, lint, build |
| Documentation upkeep | Keep examples and guides consistent with the implementation | Documentation build, links, whitespace |
| Python or Go library maintenance | Improve compatibility and regression coverage | The repository's tests and static checks |
| Dependency maintenance | Update an explicitly permitted manifest and lockfile for a focused task | Dependency installation, tests, build; allow those files in `editable` |
| Review follow-up | Address feedback and failed CI on an existing maintenance PR | The same required checks as the original proposal |
| Release preparation | Refresh a changelog or migration guide from committed changes | Documentation checks; human release review |
| SDK compatibility | Compare official API references with source and contract tests | Language-specific contract tests and packaging checks |

Give each task a narrow outcome. A weekly instruction such as “repair one
reproducible bug and add a regression test” is easier to review than a request to
rewrite the project. No useful work should produce an unchanged result.

## Starting configurations

- [JavaScript / TypeScript application](../examples/node.yml): Codex with npm checks.
- [Python repository](../examples/python.yml): Codex with unittest.
- [Go repository](../examples/go.yml): Codex with Go tests and vet.
- [Documentation](../examples/docs.yml): restricted Markdown editing.
- [Rust SDK workspace](../examples/rust-sdks.yml): the built-in agent and Cargo profile.

Copy a configuration into the target repository as `.maintainer.yml`, then adapt
its editable paths, task and commands. Install the project's runtimes on the
worker or choose a matching Docker base image. No Cargo files are needed when
using `validation.profile: custom`.

## Featured example: Prism's Rust SDKs

[Prism](https://github.com/djsydney04/prism) contains independent Rust SDKs for
OpenAI, Gemini and Groq, plus shared transport code. It motivated the original
documentation-driven maintenance profile; it is one application of the framework.

Its maintenance task compares provider changelogs and API references with the
implementation, makes a small compatibility fix, adds offline contract tests,
and updates examples. It continues an existing maintenance PR when review or CI
feedback needs attention. New blockers ask the owner for help; no-op runs stay quiet.

The [portable Prism example](../examples/rust-sdks.yml) shows the task, source
allowlist, crate paths and Rust checks. The consuming repository's full config
and scheduled caller are tracked in [Prism's integration PR](https://github.com/djsydney04/prism/pull/4);
check that PR for rollout status. This example does not imply that every provider,
agent or deployment has been exercised with a live account.

## Code-defined orchestration

Configuration and workflow files live in version control. Use the
[self-hosted caller](../examples/self-hosted-workflow.yml) for a complete
propose → validate → publish → notify pipeline on your GitHub runners, or call
the [Rust library and CLI stages](self-hosting.md#compose-the-stages-in-code) from
your own scheduler. Each maintained repository supplies its own task and checks.
