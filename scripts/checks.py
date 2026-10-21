#!/usr/bin/env python3
"""Small stdio MCP server: let the agent test and repair without arbitrary shell."""

import json
import os
import subprocess
import sys


CRATES = tuple(json.loads(open(os.environ["MAINTENANCE_CONFIG"]).read())["rust"]["crates"]) if os.environ.get("MAINTENANCE_CONFIG") else ("prism-ai-core",)
TOOL = {
    "name": "check_rust",
    "description": "Run a bounded offline Rust check, then inspect the output and repair failures.",
    "inputSchema": {
        "type": "object",
        "properties": {
            "check": {"type": "string", "enum": ["format", "tests", "clippy"]},
            "crate": {"type": "string", "enum": list(CRATES)},
        },
        "required": ["check"],
        "additionalProperties": False,
    },
}


def command(arguments):
    if set(arguments) - {"check", "crate"}:
        raise ValueError("Unknown check arguments")
    crate = arguments.get("crate")
    if crate is not None and crate not in CRATES:
        raise ValueError("Unknown configured crate")
    scope = ["-p", crate] if crate else ["--workspace"]
    check = arguments.get("check")
    if check == "format":
        return ["cargo", "fmt", "--all", "--check"]
    if check == "tests":
        return ["cargo", "test", *scope, "--all-targets", "--locked", "--offline"]
    if check == "clippy":
        return ["cargo", "clippy", *scope, "--all-targets", "--locked", "--offline", "--", "-D", "warnings"]
    raise ValueError("Unknown Rust check")


def dispatch(method, params):
    if method == "initialize":
        return {"protocolVersion": "2024-11-05", "capabilities": {"tools": {}},
                "serverInfo": {"name": "repo-maintainer-checks", "version": "1.0.0"}}
    if method == "ping":
        return {}
    if method == "tools/list":
        return {"tools": [TOOL]}
    if method != "tools/call" or params.get("name") != TOOL["name"]:
        raise ValueError("Unknown check tool")
    args = command(params.get("arguments", {}))
    environment = {key: value for key, value in os.environ.items()
                   if not any(secret in key.upper() for secret in ("KEY", "TOKEN", "SECRET", "PASSWORD"))}
    try:
        result = subprocess.run(args, capture_output=True, text=True, timeout=300,
                                env=environment, check=False)
        text = f"Command: {' '.join(args)}\nExit code: {result.returncode}\n"
        text += (result.stdout + result.stderr)[-18_000:]
        return {"content": [{"type": "text", "text": text}], "isError": result.returncode != 0}
    except subprocess.TimeoutExpired:
        return {"content": [{"type": "text", "text": "Check exceeded five minutes; finish the proposal and let full CI validate it."}],
                "isError": True}


def main():
    for line in sys.stdin:
        request = json.loads(line)
        if "id" not in request:
            continue
        response = {"jsonrpc": "2.0", "id": request["id"]}
        try:
            response["result"] = dispatch(request["method"], request.get("params", {}))
        except (ValueError, TypeError, OSError) as error:
            response["error"] = {"code": -32602, "message": str(error)}
        print(json.dumps(response), flush=True)


if __name__ == "__main__":
    main()
