"""One safe YAML/JSON configuration format for Actions, the CLI and Docker."""

import argparse
import json
import os
from pathlib import Path, PurePosixPath
import re
from urllib.parse import urlparse

import yaml

ROOT = Path(__file__).resolve().parents[1]
PROVIDERS = json.loads((ROOT / "agent/providers.json").read_text())
FILENAMES = (".maintainer.yml", ".maintainer.yaml", ".github/maintainer.yml", ".github/maintainer.yaml", ".github/maintainer.json")


class UniqueLoader(yaml.SafeLoader):
    """Reject duplicate keys instead of silently choosing the last value."""


def mapping(loader, node, deep=False):
    result = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        if not isinstance(key, str):
            raise ValueError("Configuration keys must be strings")
        if key in result:
            raise ValueError(f"Duplicate configuration key: {key}")
        result[key] = loader.construct_object(value_node, deep=deep)
    return result


UniqueLoader.add_constructor(yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, mapping)


def object_fields(value, allowed, name):
    if not isinstance(value, dict):
        raise ValueError(f"{name} must be a mapping")
    extra = set(value) - set(allowed)
    if extra:
        raise ValueError(f"Unknown {name} fields: {', '.join(sorted(extra))}. Keep API keys in environment variables.")
    return value


def strings(value, name, *, empty=False):
    if (not isinstance(value, list) or len(value) > 100 or (not value and not empty)
            or any(not isinstance(item, str) or not item or len(item) > 2000 for item in value)):
        raise ValueError(f"{name} must be a list of nonempty strings")
    return value


def relative_path(value, name, *, empty=False):
    if empty and value == "":
        return value
    if (not isinstance(value, str) or not value or PurePosixPath(value).is_absolute()
            or ".." in PurePosixPath(value).parts or "\\" in value or "\n" in value):
        raise ValueError(f"{name} must be a repository-relative path")
    return value


def agent_settings(raw, env=None):
    env = os.environ if env is None else env
    raw = object_fields(raw or {}, ("provider", "model", "allow_paid", "max_requests"), "agent")
    provider = env.get("MAINTENANCE_PROVIDER") or raw.get("provider", "groq")
    if provider == "oai":
        provider = "openai"
    if provider not in PROVIDERS:
        raise ValueError("agent.provider must be google, groq, openai or anthropic")
    profile = PROVIDERS[provider]
    # An explicit environment provider override selects its own default model.
    model = env.get("MAINTENANCE_MODEL") or (raw.get("model") if not env.get("MAINTENANCE_PROVIDER") or provider == raw.get("provider") else None) or profile["defaultModel"]
    if not isinstance(model, str) or not model or len(model) > 200 or any(char.isspace() for char in model):
        raise ValueError("agent.model must be a model ID")
    if profile["models"] is not None and model not in profile["models"]:
        raise ValueError(f"Unsupported {provider} model: {model}")
    allow_paid = raw.get("allow_paid", False)
    if env.get("MAINTENANCE_ALLOW_PAID"):
        if env["MAINTENANCE_ALLOW_PAID"] not in ("true", "false"):
            raise ValueError("MAINTENANCE_ALLOW_PAID must be true or false")
        allow_paid = env["MAINTENANCE_ALLOW_PAID"] == "true"
    if not isinstance(allow_paid, bool):
        raise ValueError("agent.allow_paid must be true or false")
    if profile["paid"] and not allow_paid:
        raise ValueError(f"{provider} requires agent.allow_paid: true; it is never a free-tier fallback")
    count = raw.get("max_requests", 16)
    if type(count) is not int or not 1 <= count <= 16:
        raise ValueError("agent.max_requests must be between 1 and 16")
    return {"provider": provider, "model": model, "allow_paid": allow_paid, "max_requests": count}


def find_config(root, explicit=None):
    if explicit:
        path = Path(explicit)
        path = path if path.is_absolute() else Path(root) / path
        if not path.is_file():
            raise ValueError(f"Configuration not found: {path}")
        return path.resolve()
    for name in FILENAMES:
        if (Path(root) / name).is_file():
            return (Path(root) / name).resolve()
    raise ValueError("Add .maintainer.yml, or run the maintainer init command")


def load_config(path, env=None):
    path = Path(path)
    if path.stat().st_size > 256_000:
        raise ValueError("Configuration exceeds 256 KB")
    text = path.read_text()
    if sum(isinstance(token, yaml.tokens.AliasToken) for token in yaml.scan(text)) > 50:
        raise ValueError("Too many YAML aliases")
    raw = yaml.load(text, Loader=UniqueLoader)
    object_fields(raw, ("version", "name", "sources", "official_hosts", "instructions", "instructions_file",
                       "agent", "editable", "rust", "branch_prefix", "issue_title", "commit_title", "ci_workflow"), "configuration")
    if raw.get("version", 1) != 1:
        raise ValueError("Unsupported maintainer configuration version")
    sources = strings(raw.get("sources"), "sources")
    derived = set()
    for url in sources:
        parsed = urlparse(url)
        if (parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password
                or parsed.port not in (None, 443) or any(char.isspace() for char in url)):
            raise ValueError("Every documentation source must be an HTTPS URL without credentials")
        derived.add(parsed.hostname)
    hosts = strings(raw.get("official_hosts", sorted(derived)), "official_hosts")
    if not derived.issubset(hosts):
        raise ValueError("Every source must use an official_hosts entry")
    editable = {"root_files": ["README.md"], "markdown_dirs": ["docs"],
                "source_dirs": ["src", "crates"], "extensions": [".rs", ".md", ".json"]}
    edit_raw = object_fields(raw.get("editable", {}), editable, "editable")
    editable.update(edit_raw)
    for name, values in editable.items():
        strings(values, f"editable.{name}", empty=True)
        if name == "extensions":
            if any(not re.fullmatch(r"\.[A-Za-z0-9]+", value) for value in values):
                raise ValueError("editable.extensions must contain file extensions")
        else:
            for value in values:
                relative_path(value, f"editable.{name}")
    rust = {"msrv": "1.88.0", "crates": [], "package_check": ""}
    rust.update(object_fields(raw.get("rust", {}), rust, "rust"))
    if not isinstance(rust["msrv"], str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", rust["msrv"]):
        raise ValueError("rust.msrv must be a version such as 1.88.0")
    strings(rust["crates"], "rust.crates", empty=True)
    if any(not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", crate) for crate in rust["crates"]):
        raise ValueError("Invalid Rust crate name")
    relative_path(rust["package_check"], "rust.package_check", empty=True)
    config = {"version": 1, "name": "Repository maintenance", "branch_prefix": "dylan/repo-maintenance-",
              "issue_title": "Repository maintenance needs help", "commit_title": "fix: maintain documented SDK compatibility",
              "ci_workflow": "ci.yml", "instructions": "Maintain the repository against the official documentation sources. Implement focused, compatible changes with meaningful offline tests."}
    config.update(raw)
    config.update({"sources": sources, "official_hosts": hosts, "editable": editable, "rust": rust,
                   "agent": agent_settings(raw.get("agent"), env)})
    for name in ("name", "issue_title", "commit_title"):
        if not isinstance(config[name], str) or not config[name] or len(config[name]) > 200 or "\n" in config[name]:
            raise ValueError(f"{name} must be one short line")
    if not isinstance(config["branch_prefix"], str) or not re.fullmatch(r"[A-Za-z0-9_/-]+-", config["branch_prefix"]):
        raise ValueError("Invalid maintenance branch prefix")
    relative_path(config["ci_workflow"], "ci_workflow")
    if "instructions_file" in config:
        relative_path(config["instructions_file"], "instructions_file")
    if not isinstance(config["instructions"], str) or len(config["instructions"]) > 16_000:
        raise ValueError("instructions must be text of at most 16000 characters")
    return config


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path")
    args = parser.parse_args()
    try:
        print(json.dumps(load_config(args.path)))
    except (ValueError, OSError, yaml.YAMLError) as error:
        parser.exit(2, f"Configuration error: {error}\n")
