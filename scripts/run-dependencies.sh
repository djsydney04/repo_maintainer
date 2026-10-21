#!/usr/bin/env bash
set -euo pipefail
dependency_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
dependency_command=run
dependency_config="$PWD/renovate.config.json"
dependency_apply=false
if [[ $# -gt 0 && "$1" != --* ]]; then dependency_command="$1"; shift; fi
while [[ $# -gt 0 ]]; do
  case "$1" in
    --config)
      [[ $# -ge 2 ]] || { echo '--config requires a path' >&2; exit 2; }
      dependency_config="$2"; shift 2 ;;
    --apply) dependency_apply=true; shift ;;
    *) echo "Unknown dependency argument: $1" >&2; exit 2 ;;
  esac
done
case "$dependency_command" in
  init|check|run) ;;
  *) echo 'Usage: maintainer dependencies [init|check|run] [--config PATH] [--apply]' >&2; exit 2 ;;
esac
if [[ "$dependency_apply" == true && "$dependency_command" != run ]]; then
  echo '--apply is only valid for dependencies run' >&2; exit 2
fi
if [[ "$dependency_command" == init ]]; then
  if [[ -e "$dependency_config" ]]; then
    echo "$dependency_config already exists; keeping your settings."
  else
    (set -o noclobber; cat "$dependency_root/renovate.example.json" > "$dependency_config")
    echo "Created $dependency_config. Add your repositories before running dependencies."
  fi
  exit 0
fi
[[ -f "$dependency_config" ]] || { echo 'Add renovate.config.json, or run maintainer dependencies init' >&2; exit 2; }
export RENOVATE_CONFIG_PATH="$(cd -- "$(dirname -- "$dependency_config")" && pwd)/$(basename -- "$dependency_config")"
export MAINTENANCE_RENOVATE_IMAGE="${MAINTENANCE_RENOVATE_IMAGE:-$(cat "$dependency_root/renovate-image.txt")}"
if [[ ! "$MAINTENANCE_RENOVATE_IMAGE" =~ ^[^[:space:]-][^[:space:]]*@sha256:[a-fA-F0-9]{64}$ ]]; then
  echo 'Renovate image must be pinned with @sha256:DIGEST' >&2; exit 2
fi
dependency_compose=(docker compose --project-directory "$dependency_root" -f "$dependency_root/compose.dependencies.yml")
"${dependency_compose[@]}" run --rm renovate renovate-config-validator --strict /config/renovate.json
if [[ "$dependency_command" == check ]]; then exit 0; fi
dependency_arguments=(renovate --autodiscover=false)
if [[ "$dependency_apply" != true ]]; then dependency_arguments+=(--dry-run=full); fi
exec "${dependency_compose[@]}" run --rm renovate "${dependency_arguments[@]}"
