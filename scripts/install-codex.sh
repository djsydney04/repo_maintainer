#!/usr/bin/env bash
set -euo pipefail
# Native Rust CLI, pinned to an official release and its published asset digests.
destination="${1:-/usr/local/bin}"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) platform=x86_64-unknown-linux-musl; digest=b48ca1b2d6b1bf42b944e02c3d937c898e24651916684cdc35fdedf31b291bcb ;;
  Linux-aarch64|Linux-arm64) platform=aarch64-unknown-linux-musl; digest=cd5f307b3fcd6080773e684b86c3114a67d4f1c61dc447be09876b552eb4bea7 ;;
  Darwin-arm64) platform=aarch64-apple-darwin; digest=51de50a39ea592b5b0a64ae0474548c282181cb6b96d9bd08965a682ae12774c ;;
  Darwin-x86_64) platform=x86_64-apple-darwin; digest=cbaea8206d3189b8a7cd7c1b476e14a541ab22d469c37253f758ff71d4ed24b7 ;;
  *) echo 'Unsupported Codex platform; install a compatible CLI yourself.' >&2; exit 1 ;;
esac
temporary="$(mktemp -d)"
trap 'rm -rf -- "$temporary"' EXIT
archive="$temporary/codex.tar.gz"
curl --fail --silent --show-error --location --retry 2 --proto '=https' --tlsv1.2 \
  "https://github.com/openai/codex/releases/download/rust-v0.159.3/codex-$platform.tar.gz" -o "$archive"
if command -v sha256sum >/dev/null; then
  actual="$(sha256sum "$archive")"
else
  actual="$(shasum -a 256 "$archive")"
fi
if [ "${actual%% *}" != "$digest" ]; then echo 'Codex archive checksum mismatch' >&2; exit 1; fi
tar -xzf "$archive" -C "$temporary"
mkdir -p -- "$destination"
install -m 755 "$temporary/codex-$platform" "$destination/codex"
"$destination/codex" --version
