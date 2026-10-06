"""False-pass, evidence privacy, and owned-cleanup checks for onboarding qualification."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, call, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_onboarding as harness


class OnboardingHarnessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.profile = harness.PrivateProfile()
        self.addCleanup(self.profile.close)
        self.args = SimpleNamespace(bootstrap=self.root / "bootstrap", bundle=self.root / "bundle",
                                    output=self.root / "evidence", timeout_ms=10, service="none")
        self.check = harness.OnboardingCheck(self.args)
        self.outside = self.root / "unrelated-home"
        self.outside.mkdir()
        (self.outside / "sentinel").write_text("unrelated user configuration")

    def prepared(self):
        self.check.profile = self.profile
        self.check.home = self.profile.root / "daemon"
        self.check.service_home = self.profile.root / "service-home"
        self.check.logs = self.profile.logs
        self.check.prefix = self.profile.root / "software"
        self.check.installed = self.check.prefix / "current/locust"
        self.check.env = self.profile.environment(self.args.bootstrap)
        return self.check

    def run_with(self, flow, *, install=None):
        self.check.install = install or Mock()
        self.check.flow = flow
        with patch.object(harness, "PrivateProfile", return_value=self.profile):
            return self.check.run()

    def retained_summary(self):
        return json.loads((self.args.output / "summary.json").read_text())

    def service_fixture(self):
        self.check.service_attempted = True
        self.check.unit = self.check.service_home / "Library/LaunchAgents/farm.locust.task.plist"
        harness.private_write(self.check.unit, "synthetic owned native service")
        harness.private_write(self.profile.fixture / "signing.secret", b"DO_NOT_RETAIN_PRIVATE_SIGNER")

    def test_failed_install_never_reports_pass_even_when_cleanup_succeeds(self):
        flow = Mock()
        installed = Mock(side_effect=harness.QualificationError("DO_NOT_RETAIN_INSTALL_FAILURE"))
        self.assertFalse(self.run_with(flow, install=installed))
        flow.assert_not_called()
        summary = self.retained_summary()
        self.assertEqual(summary["status"], "failed")
        self.assertTrue(summary["checks"]["cleanup_verified"])
        self.assertFalse(self.profile.root.exists())
        self.assertNotIn("DO_NOT_RETAIN_INSTALL_FAILURE", json.dumps(summary))
        self.assertEqual(summary["failure"]["detail_sha256"], harness.sha256(b"DO_NOT_RETAIN_INSTALL_FAILURE"))
        self.assertEqual((self.outside / "sentinel").read_text(), "unrelated user configuration")

    def test_failed_flow_never_reports_pass_and_retains_only_error_projection(self):
        flow = Mock(side_effect=harness.QualificationError("DO_NOT_RETAIN_FLOW_FAILURE"))
        self.assertFalse(self.run_with(flow))
        summary = self.retained_summary()
        self.assertEqual(summary["status"], "failed")
        self.assertNotIn("DO_NOT_RETAIN_FLOW_FAILURE", json.dumps(summary))
        self.assertFalse(summary["model_ready"])
        self.assertFalse(self.profile.root.exists())

    def test_loaded_service_cleanup_failure_retains_unit_and_private_fixture(self):
        self.args.service = "launchd"
        self.check.timeout = 0.001
        self.check.service = Mock(return_value={"state": "loaded"})
        self.assertFalse(self.run_with(self.service_fixture))
        summary = self.retained_summary()
        self.assertEqual(summary["status"], "failed")
        self.assertEqual(summary["retained_private_fixture"], str(self.profile.root))
        self.assertTrue(self.check.unit.exists())
        self.assertFalse((self.profile.fixture / "signing.secret").exists())
        self.assertIn("cleanup_failure", summary)
        self.assertNotIn("cleanup_verified", summary["checks"])
        self.assertFalse(any(row.args[0] == "remove" for row in self.check.service.call_args_list))
        self.assertTrue((self.outside / "sentinel").exists())

    def test_successful_cleanup_verifies_stop_then_removes_only_private_fixture(self):
        self.args.service = "launchd"

        def service(operation, *extra, optional=False):
            if operation in ("stop", "status"):
                return {"state": "stopped"}
            if operation == "remove-plan":
                return {"plan_sha256": "a" * 64}
            self.assertEqual(operation, "remove")
            self.assertEqual(extra, ("--expect-plan", "a" * 64))
            self.check.unit.unlink()
            return {"removed": True}

        self.check.service = Mock(side_effect=service)
        self.assertTrue(self.run_with(self.service_fixture))
        self.assertEqual(self.check.service.call_args_list, [
            call("stop", optional=True), call("status"), call("remove-plan"),
            call("remove", "--expect-plan", "a" * 64),
        ])
        summary = self.retained_summary()
        self.assertEqual(summary["status"], "passed")
        self.assertTrue(summary["checks"]["owned_service_stopped_and_removed"])
        self.assertTrue(summary["checks"]["cleanup_verified"])
        self.assertFalse(self.profile.root.exists())
        self.assertTrue((self.outside / "sentinel").exists())
        self.assertTrue(self.args.output.exists())
        self.assertEqual(self.args.output.stat().st_mode & 0o777, 0o700)
        for name in ("summary.json", "operations.jsonl"):
            self.assertEqual((self.args.output / name).stat().st_mode & 0o777, 0o600)

    def test_remove_claim_without_unit_removal_retains_fixture_and_fails(self):
        self.args.service = "launchd"
        self.check.service = Mock(side_effect=lambda operation, *extra, **kwargs:
            {"plan_sha256": "a" * 64} if operation == "remove-plan" else {"state": "stopped", "removed": True})
        self.assertFalse(self.run_with(self.service_fixture))
        summary = self.retained_summary()
        self.assertTrue(self.check.unit.exists())
        self.assertEqual(summary["status"], "failed")
        self.assertEqual(summary["retained_private_fixture"], str(self.profile.root))
        self.assertNotIn("cleanup_verified", summary["checks"])

    def test_forced_foreground_cleanup_reaps_only_owned_child_and_cannot_pass(self):
        process = Mock()
        process.poll.return_value = None
        process.wait.side_effect = [subprocess.TimeoutExpired("owned daemon", self.check.timeout), 0]
        self.check.cli = Mock(return_value={"done": None})

        def flow():
            self.check.foreground = process

        self.assertFalse(self.run_with(flow))
        process.kill.assert_called_once_with()
        self.assertEqual(process.wait.call_args_list, [call(timeout=self.check.timeout), call(timeout=self.check.timeout)])
        self.assertEqual(self.check.cli.call_args_list, [call("daemon-stop", ["daemon", "stop"], owner=True, optional=True)])
        summary = self.retained_summary()
        self.assertEqual(summary["status"], "failed")
        self.assertEqual(summary["retained_private_fixture"], str(self.profile.root))
        self.assertTrue((self.outside / "sentinel").exists())

    def test_plan_that_claims_read_only_but_writes_is_rejected_before_daemon_start(self):
        check = self.prepared()

        def plan(label, arguments, **kwargs):
            self.assertEqual(label, "up-plan")
            self.assertIn("--plan", arguments)
            self.assertTrue(kwargs["owner"])
            (self.profile.root / "unexpected-write").write_text("plan mutated state")
            return {"changed": False}

        check.cli = Mock(side_effect=plan)
        with patch.object(harness.subprocess, "Popen") as launch:
            with self.assertRaisesRegex(harness.QualificationError, "selected plan modified"):
                check.flow()
            launch.assert_not_called()
        self.assertFalse(check.service_attempted)
        self.assertNotIn("selected_plan_read_only_before_daemon", check.summary["checks"])

    def test_cli_failure_and_invalid_json_cannot_pass_despite_success_claim(self):
        check = self.prepared()
        for exit_code, output in (
            (3, b'{"ok":true,"result":{}}'),
            (0, b'{"ok":false,"error":{"code":"denied"}}'),
            (0, b"not a JSON envelope"),
        ):
            with self.subTest(exit_code=exit_code, output=output):
                result = subprocess.CompletedProcess([], exit_code, output, b"private diagnostics")
                with patch.object(harness.subprocess, "run", return_value=result):
                    with self.assertRaises(harness.QualificationError):
                        check.cli("up", ["up", "--client", "codex", "--plan"], owner=True)

    def test_bound_launcher_accepts_review_stderr_without_retaining_private_payloads(self):
        check = self.prepared()
        output = b'{"ok":true,"result":{"principal":"public"}}'
        review = b"DO_NOT_RETAIN_REVIEW_OR_PRIVATE_DATA"
        result = subprocess.CompletedProcess([], 0, output, review)
        launcher = self.profile.home / ".agents/skills/locust/locust-cli"
        with patch.object(harness.subprocess, "run", return_value=result) as command:
            self.assertEqual(check.cli("launcher", ["status"], binary=launcher, bound=True), {"principal": "public"})
        argv = command.call_args.args[0]
        self.assertEqual(argv, [str(launcher), "--json", "status"])
        for variable in ("LOCUST_HOME", "LOCUST_CREDENTIAL", "LOCUST_SESSION"):
            self.assertNotIn(variable, command.call_args.kwargs["env"])
        evidence = check.receipts.read_text()
        self.assertNotIn(review.decode(), evidence)
        self.assertNotIn("public", evidence)
        receipt = json.loads(evidence)
        self.assertEqual(receipt["stderr_sha256"], harness.sha256(review))
        self.assertEqual(receipt["stdout_sha256"], harness.sha256(output))

    def test_command_timeout_records_hashes_and_cannot_be_an_optional_success(self):
        check = self.prepared()
        expired = subprocess.TimeoutExpired("fixture", check.timeout,
                                            output=b"DO_NOT_RETAIN_OUTPUT", stderr=b"DO_NOT_RETAIN_ERROR")
        with patch.object(harness.subprocess, "run", side_effect=expired):
            with self.assertRaisesRegex(harness.QualificationError, "caller deadline expired"):
                check.cli("service-stop", ["service", "stop"], optional=True)
        evidence = check.receipts.read_text()
        self.assertNotIn("DO_NOT_RETAIN", evidence)
        self.assertTrue(json.loads(evidence)["timed_out"])


if __name__ == "__main__":
    unittest.main()
