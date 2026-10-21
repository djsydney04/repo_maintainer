# Changelog

## Unreleased

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
