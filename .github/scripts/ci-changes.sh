#!/usr/bin/env bash
set -euo pipefail

base_ref="${1:-}"
rust=false
docker=false
workflows=false

# Manual runs, new branches and unavailable history must run all checks.
if [[ -z "$base_ref" ]] || ! base_commit=$(git rev-parse --verify --end-of-options "$base_ref^{commit}" 2>/dev/null); then
  rust=true
  docker=true
  workflows=true
else
  changed_paths=$(mktemp)
  trap 'rm -f -- "$changed_paths"' EXIT
  # Show a rename as a deletion plus addition so its old build inputs count too.
  git diff --name-only --no-renames -z "$base_commit" HEAD -- > "$changed_paths"
  while IFS= read -r -d '' path; do
    case "$path" in
      .github/workflows/ci.yml|.github/scripts/*)
        rust=true; docker=true; workflows=true ;;
      .github/workflows/*)
        workflows=true ;;
      Cargo.toml|Cargo.lock|src/*|maintainer.example.yml)
        rust=true; docker=true ;;
      tests/*|examples/*|rustfmt.toml|rust-toolchain|rust-toolchain.toml|.cargo/*)
        rust=true ;;
      Dockerfile|.dockerignore|compose.yml|compose.yaml|maintainer|.env.example)
        docker=true ;;
      *.md|docs/*|LICENSE|.github/ISSUE_TEMPLATE/*|.github/dependabot.yml)
        ;;
      *)
        # Unknown inputs are deliberately conservative, rather than silently skipped.
        rust=true; docker=true ;;
    esac
  done < "$changed_paths"
fi

printf 'rust=%s\ndocker=%s\nworkflows=%s\n' "$rust" "$docker" "$workflows"
