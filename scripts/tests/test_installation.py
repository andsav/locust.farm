"""Isolation and false-positive checks for local installation qualification."""

import importlib.util
import json
import os
from pathlib import Path
import stat
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
spec = importlib.util.spec_from_file_location("check_installation", SCRIPTS / "check_installation.py")
installation = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installation)


class InstallationTests(unittest.TestCase):
    def bundle(self, root):
        source = root / "source"
        for relative in installation.PAYLOADS:
            path = source / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(relative.encode())
            path.chmod(0o755 if relative == "locust" else 0o644)
        return source

    def test_unsigned_copy_does_not_copy_signatures_or_unselected_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = self.bundle(root)
            (source / "manifest.sig").write_bytes(b"untrusted signature")
            (source / "private.env").write_text("never copy this")
            installation.copy_bundle(source, root / "copy")
            held = {str(p.relative_to(root / "copy")) for p in (root / "copy").rglob("*") if p.is_file()}
            self.assertEqual(held, set(installation.PAYLOADS))
            self.assertEqual(stat.S_IMODE((root / "copy/locust").stat().st_mode), 0o755)

    def test_bundle_leaf_symlink_cannot_read_outside_input(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = self.bundle(root)
            secret = root / "external"
            secret.write_bytes(b"not a payload")
            (source / "locust").unlink()
            (source / "locust").symlink_to(secret)
            with self.assertRaises(OSError):
                installation.copy_bundle(source, root / "copy")
            self.assertFalse((root / "copy/locust").exists())

    def test_bundle_directory_alias_and_hardlinks_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = self.bundle(root)
            alias = root / "alias"
            alias.symlink_to(source, target_is_directory=True)
            with self.assertRaises(installation.CheckFailure):
                installation.copy_bundle(alias, root / "copy-alias")
            os.link(source / "locust", root / "linked")
            with self.assertRaises(installation.CheckFailure):
                installation.copy_bundle(source, root / "copy-hardlink")

    def test_environment_is_constructed_without_ambient_secrets_or_home(self):
        with patch.dict(os.environ, {"OPENAI_API_KEY": "ambient", "LOCUST_CREDENTIAL": "ambient",
                                     "HOME": "/real/home", "GIT_CONFIG_GLOBAL": "/real/git"}):
            env = installation.environment(Path("/private/fixture"))
        self.assertEqual(env["HOME"], "/private/fixture/home")
        for name in ("OPENAI_API_KEY", "LOCUST_CREDENTIAL", "GIT_CONFIG_GLOBAL"):
            self.assertNotIn(name, env)
        self.assertEqual(env["LOCUST_LOOKUP"], "none")

    def test_cpu_counter_units_cover_macos_linux_and_long_processes(self):
        self.assertAlmostEqual(installation.cpu_seconds("0:00.03"), 0.03)
        self.assertEqual(installation.cpu_seconds("01:02:03"), 3723)
        self.assertEqual(installation.cpu_seconds("1-02:03:04.5"), 93784.5)
        with self.assertRaises(installation.CheckFailure):
            installation.cpu_seconds("missing")

    def test_expected_refusal_cannot_pass_on_success(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            check = installation.InstallationCheck(root / "bootstrap", root / "bundle", root / "output")
            check.bootstrap, check.installed, check.daemon_home = root / "bootstrap", root / "installed", root / "daemon"
            check.command = lambda argv: (0, json.dumps({"ok": True, "result": {"installed": True}}), "")
            with self.assertRaisesRegex(installation.CheckFailure, "expected denied"):
                check.cli(["install", "plan"], expected_error="denied")

    def test_preservation_fingerprint_includes_directories_modes_and_symlink_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            folder = root / "empty"
            folder.mkdir(mode=0o700)
            link = root / "link"
            link.symlink_to("missing-one")
            initial = installation.data_fingerprint(root)
            folder.chmod(0o755)
            self.assertNotEqual(initial, installation.data_fingerprint(root))
            folder.chmod(0o700)
            self.assertEqual(initial, installation.data_fingerprint(root))
            link.unlink()
            link.symlink_to("missing-two")
            self.assertNotEqual(initial, installation.data_fingerprint(root))
            self.assertEqual(initial["empty"][0], "directory")
            self.assertEqual(initial["link"], ("symlink", stat.S_IMODE(link.lstat().st_mode), "missing-one"))

    def test_probe_refusal_checks_specific_diagnostic(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            check = installation.InstallationCheck(root / "bootstrap", root / "bundle", root / "output")
            check.bootstrap, check.daemon_home = root / "bootstrap", root / "daemon"
            check.command = lambda argv: (8, json.dumps({"ok": False, "error": {
                "code": "unavailable", "message": "unrelated failure"}}), "")
            with self.assertRaisesRegex(installation.CheckFailure, "unexpected failure path"):
                check.cli(["install", "apply"], expected_error="unavailable",
                          expected_message="verified candidate could not start")

    def test_missing_doctor_checks_cannot_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            check = installation.InstallationCheck(root / "bootstrap", root / "bundle", root / "output")
            check.cli = lambda *args, **kwargs: {"checks": []}
            with self.assertRaisesRegex(installation.CheckFailure, "missing checks"):
                check.healthy_doctor()

    def test_registered_service_cleanup_failure_retains_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            check = installation.InstallationCheck(root / "bootstrap", root / "bundle", root / "output")
            check.prepare = lambda: None

            def flow():
                (check.root / "unit.plist").write_text("owned service fixture")
                check.service_owned = {"label": "fixture-only"}

            check.flow = flow
            check.cleanup_service = lambda: (_ for _ in ()).throw(installation.CheckFailure("manager unavailable"))
            try:
                self.assertFalse(check.run())
                self.assertTrue((check.root / "unit.plist").exists())
                self.assertEqual(check.summary["retained_fixture_for_service_cleanup"], str(check.root))
                self.assertIn("manager unavailable", check.summary["failures"][0])
            finally:
                if check.root.exists():
                    shutil.rmtree(check.root)

    def test_native_stop_rejects_other_unavailable_and_still_loaded_state(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            check = installation.InstallationCheck(root / "bootstrap", root / "bundle", root / "output", timeout=0.01)
            check.bootstrap, check.daemon_home, check.service_args = root / "bootstrap", root / "daemon", []
            check.command = lambda argv: (8, json.dumps({"ok": False, "error": {
                "code": "unavailable", "message": "manager unavailable"}}), "")
            with self.assertRaisesRegex(installation.CheckFailure, "outside the allowed"):
                check.stop_service_observed()
            check.command = lambda argv: (0, json.dumps({"ok": True, "result": {
                "state": "loaded", "daemon_api_readiness_observed": False}}), "")
            with self.assertRaisesRegex(installation.CheckFailure, "invalid service control observation"):
                check.stop_service_observed()
            check.command = lambda argv: (8, json.dumps({"ok": False, "error": {
                "code": "unavailable", "message": "service remains loaded after stop"}}), "")
            check.service_cli = lambda operation: {"state": "loaded"}
            with self.assertRaisesRegex(installation.CheckFailure, "timed out"):
                check.stop_service_observed()
            check.service_cli = lambda operation: {"state": "stopped"}
            check.stop_service_observed()

    def test_active_manifest_alone_does_not_prove_trust_policy_preservation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            check = installation.InstallationCheck(root / "bootstrap", root / "bundle", root / "output")
            check.prefix = root
            policy = root / "trust-state.json"
            policy.write_text("original retained policy")
            state = {"manifest_sha256": "a" * 64, "withdrawals_sequence": 2,
                     "trust_state_sha256": installation.digest(policy)}
            check.status = lambda: {"manifest_sha256": "a" * 64, "withdrawals_sequence": 2}
            check.unchanged_trust(state)
            policy.write_text("changed retained policy")
            with self.assertRaisesRegex(installation.CheckFailure, "retained trust policy"):
                check.unchanged_trust(state)

    def test_bootstrap_is_used_until_installed_execution_is_explicit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            check = installation.InstallationCheck(root / "bootstrap", root / "unsigned", root / "output")
            check.bootstrap, check.installed, check.daemon_home = root / "trusted-copy", root / "prefix/current/locust", root / "daemon"
            calls = []

            def command(argv):
                calls.append(argv)
                return 0, json.dumps({"ok": True, "result": {}}), ""

            check.command = command
            check.cli(["package", "verify"])
            check.cli(["install", "apply"])
            check.cli(["doctor"], installed=True)
            self.assertEqual([call[0] for call in calls], [check.bootstrap, check.bootstrap, check.installed])
            self.assertNotIn(root / "unsigned/locust", [call[0] for call in calls])


if __name__ == "__main__":
    unittest.main()
