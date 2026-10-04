"""False-pass checks for enrollment in the actual native-client campaign."""
import json
import copy
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from client_qualification.onboarding import OnboardedDaemon
from client_qualification.production import ProductionError
from client_qualification.runtime import Profile, private_write


class NativeOnboardingTests(unittest.TestCase):
    def setUp(self):
        self.output = tempfile.TemporaryDirectory()
        self.addCleanup(self.output.cleanup)
        self.profile = Profile(self.output.name, "onboarding")
        self.addCleanup(self.profile.close)
        self.daemon = OnboardedDaemon(self.profile, "/usr/bin/true", 5, "claude-code")
        self.credential = self.daemon.home / "agents/enrolled.credential"
        self.session = self.daemon.home / "sessions/enrolled.secret"
        self.launcher = self.profile.home / ".claude/skills/locust/locust-cli"
        private_write(self.credential, b"c" * 32)
        private_write(self.session, b"s" * 32)
        private_write(self.launcher, "#!/bin/sh\n")
        self.launcher.chmod(0o700)
        self.row = {"client": "claude", "principal": "a" * 64, "instance": "b" * 32,
                    "credential_file": str(self.credential), "session_file": str(self.session),
                    "launcher": str(self.launcher), "changed": False}

    def test_readiness_and_client_scope_cannot_false_pass(self):
        baseline = {"configuration_ready": True, "daemon_api_ready": True,
                    "model_ready": False, "grants_added": False, "clients": [self.row]}
        mutations = [{"model_ready": True}, {"grants_added": True}, {"daemon_api_ready": False},
                     {"clients": []}, {"clients": [self.row, self.row]},
                     {"clients": [dict(self.row, client="codex")]}]
        for changed in mutations:
            response = subprocess.CompletedProcess([], 0, json.dumps({"ok": True, "result": dict(baseline, **changed)}).encode(), b"")
            with patch.object(self.daemon, "_guard", return_value=[]), patch("client_qualification.onboarding.subprocess.run", return_value=response):
                with self.assertRaises(ProductionError):
                    self.daemon.onboard()

    def test_up_uses_selected_profile_and_inferred_installed_prefix(self):
        result = {"configuration_ready": True, "daemon_api_ready": True,
                  "model_ready": False, "grants_added": False, "clients": [self.row]}
        response = subprocess.CompletedProcess([], 0, json.dumps({"ok": True, "result": result}).encode(), b"review text")
        with patch.object(self.daemon, "_guard", return_value=[]), patch("client_qualification.onboarding.subprocess.run", return_value=response) as run:
            self.assertEqual(self.daemon.onboard(), self.row)
        argv = run.call_args.args[0]
        self.assertIn("up", argv)
        self.assertEqual(argv[argv.index("--service") + 1], "none")
        self.assertEqual(argv[argv.index("--profile-home") + 1], str(self.profile.home))
        self.assertNotIn("--prefix", argv)
        self.assertNotIn("--credential", argv)
        self.assertNotIn("review text", self.daemon.events.read_text())

    def test_binding_rejects_external_secrets_and_permissions(self):
        self.assertEqual(self.daemon.binding(self.row)["principal"], "a" * 64)
        outside = self.profile.home / "ambient-secret"
        private_write(outside, b"c" * 32)
        with self.assertRaises(ProductionError):
            self.daemon.binding(dict(self.row, credential_file=str(outside)))
        self.credential.chmod(0o644)
        with self.assertRaises(ProductionError):
            self.daemon.binding(self.row)

    def test_retry_rejects_secret_replacement_extra_enrollment_and_reported_change(self):
        self.daemon.onboarding["binding"] = self.daemon.binding(self.row)
        self.daemon.principal = self.row["principal"]
        agents = {"status": {"agents": [{"agent": self.daemon.principal}]}}
        with patch.object(self.daemon, "onboard", return_value=self.row), patch.object(self.daemon, "call", return_value=agents):
            self.daemon.retry()
            private_write(self.session, b"r" * 32)
            with self.assertRaisesRegex(ProductionError, "changed enrolled identity"):
                self.daemon.retry()
            private_write(self.session, b"s" * 32)
            agents["status"]["agents"].append({"agent": "extra"})
            with self.assertRaisesRegex(ProductionError, "extra principal"):
                self.daemon.retry()
        with patch.object(self.daemon, "onboard", return_value=dict(self.row, changed=True)):
            with self.assertRaises(ProductionError):
                self.daemon.retry()

    def test_initial_authority_rejected_before_fixture_grant(self):
        with patch.object(self.daemon, "onboard", return_value=self.row), patch.object(self.daemon, "call", return_value={"status": {"agents": [{"agent": self.row["principal"], "grants": {"manage_goals": True}}]}}) as call:
            with self.assertRaisesRegex(ProductionError, "granted work authority"):
                self.daemon._enroll_identity()
            self.assertEqual(call.call_count, 1)

    def test_native_metadata_refresh_preserves_binding_then_requires_noop(self):
        self.daemon.onboarding["binding"] = self.daemon.binding(self.row)
        self.daemon.principal = self.row["principal"]
        agents = {"status": {"agents": [{"agent": self.daemon.principal}]}}
        with patch.object(self.daemon, "onboard", side_effect=[dict(self.row, changed=True), self.row]), patch.object(self.daemon, "call", return_value=agents):
            self.daemon.retry(after_native=True)
        self.assertTrue(self.daemon.onboarding["native_profile_reconciled"])
        with patch.object(self.daemon, "onboard", return_value=dict(self.row, changed=True)):
            with self.assertRaisesRegex(ProductionError, "reconciled profile"):
                self.daemon.retry(after_native=True)
        private_write(self.launcher, "#!/bin/sh\nchanged\n")
        self.launcher.chmod(0o700)
        with patch.object(self.daemon, "onboard", return_value=self.row):
            with self.assertRaisesRegex(ProductionError, "changed enrolled identity"):
                self.daemon.retry(after_native=True)

    def test_fixture_authority_uses_current_agent_grants_contract(self):
        agents = {"status": {"agents": [{"agent": self.row["principal"], "grants": {"manage_goals": False}}]}}
        with patch.object(self.daemon, "onboard", return_value=self.row), patch.object(self.daemon, "call", return_value=agents) as call:
            self.daemon._enroll_identity()
        arguments = call.call_args.args[0]
        self.assertEqual(arguments[:4], ["agent", "grant", "--agent", self.row["principal"]])
        self.assertEqual(arguments[4], "--grants")
        self.assertEqual(json.loads(arguments[5]), {"manage_goals": True})

    def test_doctor_requires_all_checks_exact_identity_and_unverified_discovery(self):
        self.daemon.principal, self.daemon.instance = self.row["principal"], self.row["instance"]
        names = ("installation", "service", "onboarding_journal", "onboarding_complete",
                 "agent_identity", "mcp_registration", "skill", "launcher", "profile_binding",
                 "profile_transaction", "profile_configuration", "hello", "status",
                 "credential_file", "daemon_lock", "socket_connection")
        result = {"checks": [{"name": name, "ok": True} for name in names], "model_ready": False,
                  "profile": {"discovery": "unverified", "identity": {
                      "principal": self.daemon.principal, "instance": self.daemon.instance}}}
        with patch.object(self.daemon, "call", return_value=result):
            self.assertFalse(self.daemon.doctor()["model_ready"])
        for field in ("failed_check", "missing_check", "wrong_identity", "model_ready", "discovery"):
            wrong = copy.deepcopy(result)
            if field == "failed_check":
                wrong["checks"][0]["ok"] = False
            elif field == "missing_check":
                wrong["checks"].pop()
            elif field == "wrong_identity":
                wrong["profile"]["identity"]["instance"] = "e" * 32
            elif field == "model_ready":
                wrong["model_ready"] = True
            else:
                wrong["profile"]["discovery"] = "verified"
            with patch.object(self.daemon, "call", return_value=wrong):
                with self.assertRaises(ProductionError):
                    self.daemon.doctor()


if __name__ == "__main__":
    unittest.main()
