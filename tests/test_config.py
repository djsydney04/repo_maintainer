import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1] / "scripts"))
from config_loader import agent_settings, find_config, load_config


class ConfigTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.path = self.root / ".maintainer.yml"
        self.path.write_text("sources:\n  - https://docs.example.org/reference\n")
        self.environment = patch.dict(os.environ, {"MAINTENANCE_PROVIDER": "", "MAINTENANCE_MODEL": "", "MAINTENANCE_ALLOW_PAID": ""})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def test_minimal_yaml_infers_hosts_and_safe_defaults(self):
        config = load_config(self.path)
        self.assertEqual(config["official_hosts"], ["docs.example.org"])
        self.assertEqual(config["agent"]["provider"], "groq")
        self.assertFalse(config["agent"]["allow_paid"])
        self.assertEqual(find_config(self.root), self.path.resolve())

    def test_paid_providers_need_explicit_opt_in_and_select_their_defaults(self):
        for provider, model in (("openai", "gpt-5.4"), ("anthropic", "claude-sonnet-4-6")):
            with self.subTest(provider=provider):
                with self.assertRaisesRegex(ValueError, "allow_paid"):
                    agent_settings({"provider": provider}, {})
                config = agent_settings({"provider": provider, "allow_paid": True}, {})
                self.assertEqual(config["model"], model)

    def test_environment_provider_override_does_not_reuse_another_providers_model(self):
        result = agent_settings({"provider": "groq", "model": "openai/gpt-oss-120b", "allow_paid": True}, {"MAINTENANCE_PROVIDER": "anthropic"})
        self.assertEqual(result["model"], "claude-sonnet-4-6")
        self.assertEqual(agent_settings({"provider": "oai", "allow_paid": True}, {})["provider"], "openai")

    def test_yaml_and_json_have_identical_semantics(self):
        other = self.root / "config.json"
        other.write_text(json.dumps({"sources": ["https://docs.example.org/reference"]}))
        self.assertEqual(load_config(other), load_config(self.path))

    def test_invalid_secrets_duplicates_paths_and_budgets_are_rejected(self):
        cases = (
            "agent:\n  api_key: do-not-store-keys\n",
            "agent:\n  provider: groq\n  provider: google\n",
            "agent:\n  max_requests: 999\n",
            "instructions_file: ../private.txt\n",
            "editable:\n  source_dirs: [/etc]\n",
            "agent:\n  allow_paid: 'false'\n",
            "rust:\n  crates: ['--unsafe']\n",
        )
        for case in cases:
            with self.subTest(case=case):
                self.path.write_text("sources: [https://docs.example.org/reference]\n" + case)
                with self.assertRaises(ValueError):
                    load_config(self.path)

    def test_cli_checks_yaml_without_a_provider_key(self):
        result = subprocess.run([sys.executable, str(Path(__file__).parents[1] / "scripts/cli.py"), "check", "--repo", str(self.root)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("No API request made", result.stdout)


if __name__ == "__main__":
    unittest.main()
