# Changelog

## Unreleased

- Add Codex CLI as a coding-agent runner with structured output, explicit account
  opt-in, sandbox selection and bounded execution.
- Add language-independent setup/check commands and optional documentation sources;
  preserve existing Rust profiles and version-1 configurations.
- Run the full PR pipeline on configurable self-hosted runner labels. Bind
  publishing to a validation receipt for the exact patch and configuration.
- Add native Codex installation, project-specific Docker base images, use cases,
  and Node/Python/Go/documentation examples alongside Prism's Rust SDK example.
- Replace the Python orchestrator and JavaScript/Pi agent with one Rust binary,
  including native provider APIs, bounded source tools and workflow commands.
- Preserve version-1 YAML/JSON configuration, paid-provider opt-in, request limits,
  official-host allowlists, patch transfer, separate validation and publishing,
  and quiet blocker notifications.
- Add native installation and update Docker and Actions to use Rust. The
  maintainer requires Rust 1.89; target repository minimums remain configurable.
- Run `rust.package_check` as a repository-relative executable. Existing checks
  must be executable and supply their own interpreter or compiled binary.
- Add MIT licensing, contributor/security/conduct guidance, issue and PR templates,
  dependency updates, and Rust package metadata.
