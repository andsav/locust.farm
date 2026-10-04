"""Production fixture safety and real persistent-runtime qualification checks."""

import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from client_qualification.production import ProductionDaemon, ProductionError, NETWORK, redact
from client_qualification.runtime import Profile, private_write

BINARY = Path(__file__).resolve().parents[2] / "target/product-root/debug/locust"


class ProductionTests(unittest.TestCase):
    def fixture(self, output):
        profile = Profile(output, "production-test")
        self.addCleanup(profile.close)
        return ProductionDaemon(profile, "/usr/bin/true", 3)

    def test_explicit_enrolled_auth_and_isolated_environment(self):
        with tempfile.TemporaryDirectory() as output:
            daemon = self.fixture(output)
            command = daemon.command(["status"])
            self.assertIn(str(daemon.credential), command)
            self.assertIn(str(daemon.session), command)
            self.assertNotIn("--owner", command)
            self.assertIn("--owner", daemon.command(["status"], owner=True))
            with patch.dict(os.environ, {"LOCUST_HOME": "/real", "LOCUST_CREDENTIAL": "secret", "OPENAI_API_KEY": "secret"}):
                env = daemon.environment()
            for key in ("LOCUST_HOME", "LOCUST_CREDENTIAL", "LOCUST_SESSION", "OPENAI_API_KEY"):
                self.assertNotIn(key, env)
            self.assertEqual({key: env[key] for key in NETWORK}, NETWORK)

    def test_secret_and_capability_redaction(self):
        secret = bytes(range(32))
        data = {"credential": list(secret), "nested": {"ticket": "locust-invite-abc"},
                "text": secret.hex().upper(), "array": list(secret), "public": "a" * 64}
        clean = redact(data, [secret])
        self.assertEqual(clean["credential"], "<redacted>")
        self.assertEqual(clean["array"], "<redacted>")
        self.assertEqual(clean["text"], "<redacted>")
        self.assertEqual(clean["public"], "a" * 64)
        self.assertNotIn("locust-invite-abc", json.dumps(clean))

    def test_guard_absence_fails_closed_before_launch(self):
        with tempfile.TemporaryDirectory() as output:
            daemon = self.fixture(output)
            with patch("client_qualification.production.platform.system", return_value="Linux"), patch("client_qualification.production.subprocess.Popen") as launch:
                with self.assertRaisesRegex(ProductionError, "guard unavailable"):
                    daemon.__enter__()
                launch.assert_not_called()

    def test_invalid_cli_output_and_error_code_do_not_escape(self):
        with tempfile.TemporaryDirectory() as output:
            daemon = self.fixture(output)
            for stdout in ("PRIVATE-DIAGNOSTIC", '{"ok":false,"error":{"code":"PRIVATE-DIAGNOSTIC"}}'):
                response = subprocess.CompletedProcess([], 1, stdout, "")
                with patch.object(daemon, "_guard", return_value=[]), patch("client_qualification.production.subprocess.run", return_value=response):
                    with self.assertRaises(ProductionError) as error:
                        daemon._invoke(["note", "add", "PRIVATE-PAYLOAD"])
                self.assertNotIn("PRIVATE", str(error.exception))
            self.assertNotIn("PRIVATE", daemon.events.read_text())

    def test_cli_result_and_evidence_are_redacted(self):
        with tempfile.TemporaryDirectory() as output:
            daemon = self.fixture(output)
            secret = b"S" * 32
            private_write(daemon.credential, secret)
            response = subprocess.CompletedProcess([], 0, json.dumps({"ok": True, "result": {"text": secret.hex(), "ticket": "locust-invite-abc"}}), "")
            with patch.object(daemon, "_guard", return_value=[]), patch("client_qualification.production.subprocess.run", return_value=response):
                result = daemon._invoke(["status"])
            self.assertEqual(result, {"text": "<redacted>", "ticket": "<redacted>"})
            self.assertNotIn(secret.hex(), daemon.events.read_text())
            self.assertNotIn("locust-invite-abc", daemon.events.read_text())

    def test_binary_change_rejected_on_restart(self):
        with tempfile.TemporaryDirectory() as output:
            daemon = self.fixture(output)
            candidate = Path(daemon.profile.root) / "binary"
            private_write(candidate, b"first")
            candidate.chmod(0o700)
            daemon.binary = candidate
            daemon._verify_binary()
            candidate.write_bytes(b"changed")
            with self.assertRaisesRegex(ProductionError, "binary changed"):
                daemon._verify_binary()

    @unittest.skipUnless(platform.system() == "Darwin" and Path("/usr/bin/sandbox-exec").exists() and BINARY.is_file(), "compiled production binary and macOS guard required")
    def test_actual_binary_persistent_sqlite_and_identity(self):
        with tempfile.TemporaryDirectory() as output:
            profile = Profile(output, "production-smoke")
            self.addCleanup(profile.close)
            daemon = ProductionDaemon(profile, BINARY, 15)
            with daemon:
                self.assertTrue((daemon.home / "locust.db").is_file())
                original = (daemon.principal, daemon.goal, daemon.endpoint, daemon.instance)
                daemon.call(["note", "add", "--goal", daemon.goal, "Persistent private qualification note"])
                daemon.restart()
                self.assertEqual((daemon.principal, daemon.goal, daemon.endpoint, daemon.instance), original)
                notes = daemon.call(["notes", "--goal", daemon.goal])
                self.assertIn("Persistent private qualification note", json.dumps(notes))
                status = daemon.call(["goal", "status", "--goal", daemon.goal])["goal_status"]
                self.assertEqual(status["coordinator"], daemon.principal)
                self.assertFalse(status["grants"]["execute"])
                secrets = daemon._secrets()
            self.assertIsNone(daemon._process)
            evidence = daemon.events.read_text()
            for secret in secrets:
                self.assertNotIn(secret.hex(), evidence)
                self.assertNotIn(json.dumps(list(secret)), evidence)
            exits = [json.loads(line) for line in evidence.splitlines() if json.loads(line)["event"] == "daemon_exit"]
            self.assertEqual(len(exits), 2)
            self.assertTrue(all(item["exit_code"] == 0 and item["socket_removed"] and not item["forced_cleanup"] for item in exits))


if __name__ == "__main__":
    unittest.main()
