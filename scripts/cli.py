#!/usr/bin/env python3
"""Check configuration or generate a reviewed patch from a local repository."""

import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import yaml

from config_loader import find_config, load_config, PROVIDERS

ROOT = Path(__file__).resolve().parents[1]


def clean_environment():
    return {key: value for key, value in os.environ.items()
            if not any(word in key.upper() for word in ("KEY", "TOKEN", "SECRET", "PASSWORD"))}


def proposal(repository, config_path, config, output):
    spec = importlib.util.spec_from_file_location("maintenance", ROOT / "scripts/maintain.py")
    maintenance = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(maintenance)
    maintenance.configure(config_path)
    key_name = PROVIDERS[config["agent"]["provider"]]["key"]
    if not (os.environ.get("SDK_MAINTENANCE_API_KEY") or os.environ.get(key_name)):
        raise ValueError(f"Set {key_name} in your environment before running inference")
    environment = dict(os.environ, MAINTENANCE_CONFIG=str(config_path), MAINTENANCE_REPOSITORY_ROOT=str(repository))
    environment.pop("GH_TOKEN", None)
    environment.pop("GITHUB_TOKEN", None)
    check_env = clean_environment()
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        raise ValueError("Output directory already contains a proposal; choose a new --output directory")
    with tempfile.TemporaryDirectory(prefix="repo-maintainer-") as temporary:
        source = Path(temporary) / "source"
        sha = subprocess.check_output(["git", "-c", f"safe.directory={repository}", "-C", str(repository), "rev-parse", "HEAD"], text=True, env=check_env).strip()
        subprocess.run(["git", "-c", f"safe.directory={repository}", "clone", "--quiet", "--no-hardlinks", "--no-checkout", "--", str(repository), str(source)], env=check_env, check=True)
        subprocess.run(["git", "checkout", "--quiet", "--detach", sha], cwd=source, env=check_env, check=True)
        subprocess.run(["cargo", "fetch", "--locked"], cwd=source, env=check_env, check=True, timeout=600)
        maintenance.write_json(output / "request.json", {"sha": sha, "branch": f"{config['branch_prefix']}{int(time.time())}-1", "pr": None})
        brief = output / "brief.md"
        brief.write_text(maintenance.add_documentation(maintenance.task_instructions(repository)))
        try:
            result = subprocess.run(["node", str(ROOT / "agent/agent.mjs"), str(brief)], cwd=source,
                                    env=environment, input="", capture_output=True, text=True, timeout=1200, check=False)
        except subprocess.TimeoutExpired:
            result = subprocess.CompletedProcess([], 124, stdout=json.dumps({"error": "The agent exceeded its 20-minute deadline"}), stderr="")
        raw = output / "pi-result.json"
        raw.write_text(result.stdout)
        if result.returncode:
            print(maintenance.redacted_diagnostic(result.stdout + result.stderr, environment), file=sys.stderr)
        maintenance.collect(source, output, raw, result.returncode)
        report = json.loads((output / "report.json").read_text())
        print(report["summary"])
        print(f"Review {output / 'report.json'} and {output / 'proposal.patch'}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "run", "smoke"), nargs="?", default="check")
    parser.add_argument("--repo", type=Path, default=Path(os.environ.get("MAINTENANCE_REPOSITORY_ROOT", "/repository")))
    parser.add_argument("--config", default=os.environ.get("MAINTENANCE_CONFIG"))
    parser.add_argument("--output", type=Path, default=Path("/output") / time.strftime("%Y%m%d-%H%M%S"))
    args = parser.parse_args()
    path = find_config(args.repo, args.config)
    config = load_config(path)
    if args.command == "check":
        agent = config["agent"]
        print(f"Configuration valid: {config['name']}\nProvider: {agent['provider']} / {agent['model']}\nDocumentation sources: {len(config['sources'])}\nNo API request made.")
    elif args.command == "smoke":
        env = dict(os.environ, MAINTENANCE_PROVIDER=config["agent"]["provider"],
                   MAINTENANCE_MODEL=config["agent"]["model"], MAINTENANCE_ALLOW_PAID=str(config["agent"]["allow_paid"]).lower())
        subprocess.run([sys.executable, str(ROOT / "scripts/maintain.py"), "smoke", "--artifacts", str(args.output)], env=env, check=True)
    else:
        proposal(args.repo.resolve(), path, config, args.output.resolve())


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError, yaml.YAMLError) as error:
        raise SystemExit(str(error)) from None
