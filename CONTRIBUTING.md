# Contributing

Bug reports, documentation improvements and focused pull requests are welcome.
Check existing issues before opening a new one. Discuss changes to providers,
editing boundaries or workflow permissions in an issue first.

Start with the [configuration reference](docs/configuration.md) and
[architecture guide](docs/architecture.md) for the current behavior and limits.

Install Rust 1.89 or newer and Git. From a fork or clone:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked -- check --config maintainer.example.yml
```

Tests use disposable Git repositories and simulated provider responses. They
require no API keys and do not publish anything. Use `repo-maintainer smoke`
only when you intend to make live inference requests with your chosen key.

Branch from `main`, keep changes focused, and use conventional commit titles
such as `fix: reject unsafe patch paths`. Describe the behavior change and how
you validated it. Add regression tests for changed behavior and use offline
fixtures for integrations. Never commit keys, generated proposals or `.env` files.

The modules in `src/` separate configuration, provider protocols, tool boundaries,
process execution, document extraction and workflow orchestration. Keep model
tools separate from GitHub publishing credentials and preserve fresh-job patch
validation. Adding a provider requires request/response and tool-continuation
tests, plus explicit credential and budget configuration.

By contributing, you agree that your contribution is licensed under the
[MIT license](LICENSE). Follow the [code of conduct](CODE_OF_CONDUCT.md).
