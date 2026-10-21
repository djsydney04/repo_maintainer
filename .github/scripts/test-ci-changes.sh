#!/usr/bin/env bash
set -euo pipefail

selector="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/ci-changes.sh"
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
cd "$fixture"
git init --quiet
git config user.email ci@example.invalid
git config user.name 'CI fixture'
mkdir -p docs src tests examples .github/workflows .github/scripts
for path in README.md docs/banner.png src/main.rs src/template.md Cargo.lock tests/config.rs examples/rust-sdks.json maintainer.example.yml Dockerfile compose.yml .github/workflows/ci.yml .github/workflows/maintain.yml .github/scripts/ci-changes.sh; do
  printf 'fixture\n' > "$path"
done
git add .
git commit --quiet -m 'Initial fixture'
base_commit=$(git rev-parse HEAD)

expect() {
  local label="$1" expected="$2" base="${3-$base_commit}" actual
  actual=$(bash "$selector" "$base")
  if [[ "$actual" != "$expected" ]]; then
    printf 'Wrong checks for %s:\n%s\n' "$label" "$actual" >&2
    exit 1
  fi
}

none=$'rust=false\ndocker=false\nworkflows=false'
rust_only=$'rust=true\ndocker=false\nworkflows=false'
docker_only=$'rust=false\ndocker=true\nworkflows=false'
builds=$'rust=true\ndocker=true\nworkflows=false'
all=$'rust=true\ndocker=true\nworkflows=true'

expect 'unchanged tree' "$none"
expect 'manual run' "$all" ''
expect 'missing base' "$all" 0000000000000000000000000000000000000000
expect 'invalid base' "$all" --invalid

for path in README.md docs/banner.png src/main.rs src/template.md Cargo.lock tests/config.rs examples/rust-sdks.json maintainer.example.yml Dockerfile compose.yml .github/workflows/ci.yml .github/workflows/maintain.yml .github/scripts/ci-changes.sh unknown-input.txt; do
  git reset --hard --quiet "$base_commit"
  git clean -fdq
  printf 'changed\n' > "$path"
  git add .
  git commit --quiet -m 'Change fixture'
  case "$path" in
    README.md|docs/*) expected="$none" ;;
    tests/*|examples/*) expected="$rust_only" ;;
    Dockerfile|compose.yml) expected="$docker_only" ;;
    .github/workflows/ci.yml|.github/scripts/*) expected="$all" ;;
    .github/workflows/maintain.yml) expected=$'rust=false\ndocker=false\nworkflows=true' ;;
    *) expected="$builds" ;;
  esac
  expect "$path" "$expected"
done

git reset --hard --quiet "$base_commit"
git clean -fdq
git rm --quiet src/main.rs
git commit --quiet -m 'Delete source'
expect 'source deletion' "$builds"

git reset --hard --quiet "$base_commit"
git mv src/main.rs docs/moved.md
git commit --quiet -m 'Rename source into docs'
expect 'source renamed into docs' "$builds"

git reset --hard --quiet "$base_commit"
printf 'changed\n' > README.md
git add .
git commit --quiet -m 'Docs first'
printf 'changed\n' > src/main.rs
git add .
git commit --quiet -m 'Source second'
expect 'complete multi-commit PR' "$builds"

printf 'CI change-selection fixtures passed.\n'
