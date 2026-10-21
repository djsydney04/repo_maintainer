"""Exercise proposal transfer, protected paths, failure recovery and quiet notifications."""

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).parents[1] / "scripts" / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


maintenance = load("sdk_maintenance", "maintain.py")
checks = load("maintenance_checks", "checks.py")
maintenance.configure(Path(__file__).parents[1] / "examples/rust-sdks.json")


def envelope(status="changed", **extra):
    report = {"status": status, "summary": "Preserve the documented response field.",
              "sources": ["https://ai.google.dev/api/generate-content"], "blockers": []}
    report.update(extra)
    return json.dumps({"response": json.dumps(report)})


class ProposalTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.source.mkdir()
        self.artifacts = self.root / "artifacts"
        self.artifacts.mkdir()
        self.git("init", "-q")
        self.git("config", "user.email", "tests@example.invalid")
        self.git("config", "user.name", "Maintenance tests")
        self.git("config", "core.autocrlf", "false")
        (self.source / "README.md").write_text("Initial documentation.\n")
        self.git("add", ".")
        self.git("commit", "-qm", "initial")
        self.sha = self.git("rev-parse", "HEAD")
        maintenance.write_json(self.artifacts / "request.json", {
            "sha": self.sha, "branch": "dylan/sdk-maintenance-123-1", "pr": None,
        })
        self.raw = self.root / "model.json"
        self.raw.write_text(envelope())
        self.environment = patch.dict(os.environ, {"GITHUB_OUTPUT": ""})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def git(self, *args):
        return maintenance.run("git", *args, cwd=self.source)

    def collect(self, code=0):
        maintenance.collect(self.source, self.artifacts, self.raw, code)

    def report(self):
        return json.loads((self.artifacts / "report.json").read_text())

    def test_completed_change_survives_transfer_to_fresh_checkout(self):
        target = self.source / "crates/gemini/tests/compatibility.rs"
        target.parent.mkdir(parents=True)
        target.write_text("// New regression fixture.\n")
        self.collect()
        self.git("reset", "--hard", "HEAD")
        maintenance.apply(self.source, self.artifacts)
        self.assertEqual(target.read_text(), "// New regression fixture.\n")

    def test_noop_produces_no_patch(self):
        self.raw.write_text(envelope("unchanged"))
        self.collect()
        self.assertEqual(self.report()["status"], "unchanged")
        self.assertEqual((self.artifacts / "proposal.patch").read_bytes(), b"")

    def test_report_cannot_hide_actual_edits(self):
        (self.source / "README.md").write_text("Changed.\n")
        self.raw.write_text(envelope("unchanged"))
        with self.assertRaises(SystemExit):
            self.collect()
        self.assertEqual(self.report()["status"], "blocked")

    def test_workflow_and_dependency_edits_cannot_be_published(self):
        for filename in (".github/workflows/release.yml", "Cargo.toml", "scripts/publish.py"):
            with self.subTest(filename=filename):
                self.git("reset", "--hard", "HEAD")
                self.git("clean", "-fd")
                target = self.source / filename
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("unauthorized change\n")
                with self.assertRaises(SystemExit):
                    self.collect()
                self.assertEqual(self.report()["status"], "blocked")
                self.git("reset", "--hard", "HEAD")
                with self.assertRaises(ValueError):
                    maintenance.apply(self.source, self.artifacts)

    @unittest.skipIf(os.name == "nt", "Executable bits are not represented on Windows")
    def test_executable_file_mode_is_rejected(self):
        (self.source / "README.md").chmod(0o755)
        self.git("config", "core.filemode", "true")
        self.git("add", "--chmod=+x", "README.md")
        with self.assertRaises(ValueError):
            maintenance.check_staged(self.source)

    @unittest.skipIf(os.name == "nt", "Symlinks can require administrator privileges on Windows")
    def test_symlink_in_an_allowed_directory_is_rejected(self):
        target = self.source / "crates/gemini/tests/test.rs"
        target.parent.mkdir(parents=True)
        target.symlink_to("../../../README.md")
        with self.assertRaises(SystemExit):
            self.collect()
        self.assertEqual(self.report()["status"], "blocked")

    def test_failed_model_preserves_partial_work_without_publishing(self):
        (self.source / "README.md").write_text("Partial work.\n")
        with self.assertRaises(SystemExit):
            self.collect(code=53)
        self.assertEqual(self.report()["status"], "blocked")
        self.assertIn("Partial work", (self.artifacts / "proposal.patch").read_text())

    def test_patch_cannot_be_applied_to_a_different_revision(self):
        (self.source / "README.md").write_text("Changed.\n")
        self.collect()
        self.git("commit", "-qm", "concurrent change")
        with self.assertRaisesRegex(ValueError, "original source revision"):
            maintenance.apply(self.source, self.artifacts)

    def test_nonmaintenance_branch_is_rejected(self):
        (self.source / "README.md").write_text("Changed.\n")
        self.collect()
        self.git("reset", "--hard", "HEAD")
        request = json.loads((self.artifacts / "request.json").read_text())
        request["branch"] = "main"
        maintenance.write_json(self.artifacts / "request.json", request)
        with self.assertRaisesRegex(ValueError, "maintenance branch"):
            maintenance.apply(self.source, self.artifacts)


class ReportTests(unittest.TestCase):
    def test_diagnostics_redact_credentials_before_truncating(self):
        result = maintenance.redacted_diagnostic("prefix api-key-value suffix", {"GEMINI_API_KEY": "api-key-value"})
        self.assertNotIn("api-key-value", result)
        self.assertIn("[redacted]", result)

    def test_changes_need_official_evidence(self):
        for sources in ([], ["https://example.com/fake"], ["http://ai.google.dev/api"],
                        ["https://ai.google.dev.evil.invalid/"]):
            with self.subTest(sources=sources), self.assertRaises(ValueError):
                maintenance.parse_report(envelope(sources=sources))

    def test_malformed_or_incomplete_result_is_not_a_successful_noop(self):
        for raw in ("[]", "{}", '{"response":"[]"}', '{"response":"not json"}'):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                maintenance.parse_report(raw)

    def test_document_parser_discards_scripts_and_navigation(self):
        parser = maintenance.DocumentationText()
        parser.feed('<nav>noise</nav><main><h1>API</h1><script>secret()</script><p>New field</p></main>')
        self.assertIn("New field", parser.text())
        self.assertNotIn("noise", parser.text())
        self.assertNotIn("secret", parser.text())


class CheckToolTests(unittest.TestCase):
    def test_model_cannot_request_arbitrary_commands_or_flags(self):
        for arguments in ({"check": "publish"}, {"check": "tests", "crate": "; echo unsafe"},
                          {"check": "tests", "extra_args": ["--ignored"]}):
            with self.subTest(arguments=arguments), self.assertRaises(ValueError):
                checks.command(arguments)

    def test_check_tool_exposes_failures_to_the_agent_without_credentials(self):
        completed = subprocess.CompletedProcess([], 1, stdout="", stderr="compile error")
        with patch.dict(os.environ, {"GEMINI_API_KEY": "test-key", "GH_TOKEN": "test-token"}), \
                patch.object(checks.subprocess, "run", return_value=completed) as process:
            result = checks.dispatch("tools/call", {"name": "check_rust", "arguments": {"check": "tests"}})
        self.assertTrue(result["isError"])
        self.assertIn("compile error", result["content"][0]["text"])
        self.assertNotIn("GEMINI_API_KEY", process.call_args.kwargs["env"])
        self.assertNotIn("GH_TOKEN", process.call_args.kwargs["env"])
        self.assertIn("--offline", process.call_args.args[0])
        self.assertNotIn("shell", process.call_args.kwargs)


class NotificationTests(unittest.TestCase):
    def test_unchanged_blocker_does_not_ping_again(self):
        with tempfile.TemporaryDirectory() as temporary:
            artifacts = Path(temporary)
            environment = {"GITHUB_REPOSITORY": "djsydney04/prism", "MAINTENANCE_OWNER": "djsydney04",
                           "RUN_URL": "https://github.com/example/run/1", "PROPOSE_RESULT": "failure",
                           "VALIDATE_RESULT": "skipped", "PUBLISH_RESULT": "skipped"}
            with patch.dict(os.environ, environment), patch.object(maintenance, "gh_json", return_value=[]), \
                    patch.object(maintenance, "run"):
                maintenance.notify(artifacts)
            body = (artifacts / "issue-body.md").read_text()
            issue = {"number": 1, "title": maintenance.ISSUE_TITLE, "body": body}
            with patch.dict(os.environ, {**environment, "RUN_URL": "https://github.com/example/run/2"}), \
                    patch.object(maintenance, "gh_json", return_value=[issue]), \
                    patch.object(maintenance, "run") as mutation:
                maintenance.notify(artifacts)
                mutation.assert_not_called()

    def test_successful_noop_does_not_create_an_issue(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.dict(os.environ, {"GITHUB_REPOSITORY": "djsydney04/prism", "PROPOSE_RESULT": "success",
                                       "VALIDATE_RESULT": "skipped", "PUBLISH_RESULT": "skipped"}), \
                patch.object(maintenance, "gh_json", return_value=[]), patch.object(maintenance, "run") as mutation:
            maintenance.notify(Path(temporary))
            mutation.assert_not_called()


if __name__ == "__main__":
    unittest.main()
