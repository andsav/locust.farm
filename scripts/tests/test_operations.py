"""Harness assertions must not turn unavailable evidence into a pass."""

import importlib.util
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
spec = importlib.util.spec_from_file_location("check_operations", SCRIPTS / "check_operations.py")
operations = importlib.util.module_from_spec(spec)
spec.loader.exec_module(operations)


class Process:
    def __init__(self):
        self.killed = False
        self.stderr = types.SimpleNamespace(close=lambda: None)

    def send_signal(self, value):
        self.signal = value

    def kill(self):
        self.killed = True

    def wait(self, timeout):
        return -9


class OperationsTests(unittest.TestCase):
    def fixture(self, root):
        check = operations.Operations(Path("/unused"), 0.02, root / "artifacts")
        self.addCleanup(check.transcript.close)
        check.sample_rss = lambda machine, checkpoint: {"resident_bytes": 1024, "checkpoint": checkpoint}
        home = root / "home"
        (home / "blobs").mkdir(parents=True)
        process = Process()
        machine = types.SimpleNamespace(home=home, number=3, process=process,
                                        reader=types.SimpleNamespace(join=lambda timeout: None))
        return check, machine, process

    def test_workspace_smoke_removes_git_from_children_without_changing_harness_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tools = root / "tools"
            tools.mkdir()
            git = tools / "git"
            git.write_text("#!/bin/sh\nexit 0\n")
            git.chmod(0o700)
            check = operations.WorkspaceSmoke(Path("/unused"), 1, root / "artifacts", network="local")
            self.addCleanup(check.transcript.close)
            machine = types.SimpleNamespace(home=root / "home")
            with mock.patch.dict(operations.os.environ, {"PATH": str(tools)}):
                self.assertEqual(operations.shutil.which("git"), str(git))
                child = check.environment(machine)
                self.assertIsNone(operations.shutil.which("git", path=child["PATH"]))
                self.assertEqual(child["LOCUST_LOOKUP"], "local")
                self.assertEqual(child["LOCUST_RELAY"], "none")
                self.assertEqual(operations.os.environ["PATH"], str(tools))
                with self.assertRaises(FileNotFoundError):
                    operations.subprocess.run(["git", "--version"], env=child, check=True,
                                              capture_output=True, timeout=1)

    def test_conflicting_update_preserves_entire_checkout_binding(self):
        before = {"id": "checkout", "base_revision": "revision", "base_manifest": "manifest",
                  "root": "/source", "root_identity": {"device": 1, "inode": 2},
                  "session": None, "task": None, "attempt": None, "active_operation": None}
        operations.require_unchanged_checkout_binding(before, dict(before), "revision", "manifest")
        for field in before:
            with self.subTest(field=field):
                after = {**before, field: "changed"}
                with self.assertRaisesRegex(operations.CheckFailure, "changed checkout binding"):
                    operations.require_unchanged_checkout_binding(before, after, "revision", "manifest")
        for invalid in ({**before, "active_operation": "pending"}, {**before, "base_revision": "other"},
                        {**before, "base_manifest": "other"}):
            with self.assertRaisesRegex(operations.CheckFailure, "unchanged accepted base"):
                operations.require_unchanged_checkout_binding(invalid, dict(invalid), "revision", "manifest")

    def test_only_exact_recorded_workspace_event_counts_as_receipt(self):
        event = "ab" * 32
        self.assertEqual(operations.operation_event({"state": {"recorded": {"event": event}}}), event)
        for state in ("prepared", {"completed": {"target_in_lineage_at_completion": True}},
                      {"recorded": {}}, {"recorded": {"event": "prefix"}}):
            with self.subTest(state=state), self.assertRaises(operations.CheckFailure):
                operations.operation_event({"state": state})

    def test_authority_without_complete_content_cannot_qualify_peer_checkout(self):
        with tempfile.TemporaryDirectory() as directory:
            check, machine, _ = self.fixture(Path(directory))
            accepted = {"authority": "ready", "head": {"revision": "exact"},
                        "content": {"complete": {"files": 1, "bytes": 10}}}
            check.workspace = lambda *args: accepted
            self.assertEqual(check.complete_workspace(machine, "goal", "exact"), accepted)
            for changed in ({**accepted, "authority": "pending"},
                            {**accepted, "head": {"revision": "other"}},
                            {**accepted, "content": {"files_missing": {"missing": ["hash"]}}},
                            {**accepted, "content": None}):
                check.workspace = lambda *args, value=changed: value
                self.assertIsNone(check.complete_workspace(machine, "goal", "exact"))

    def test_file_preservation_observes_nested_content_and_mode(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "nested").mkdir()
            file = root / "nested" / "file.txt"
            file.write_bytes(b"before")
            file.chmod(0o600)
            original = operations.directory_files(root)
            file.chmod(0o700)
            self.assertNotEqual(operations.directory_files(root), original)
            file.chmod(0o600)
            file.write_bytes(b"after")
            self.assertNotEqual(operations.directory_files(root), original)

    def test_success_is_not_an_expected_error(self):
        with tempfile.TemporaryDirectory() as directory:
            check, machine, _ = self.fixture(Path(directory))
            check.cli = lambda *args, **kwargs: {"recorded": {"event": "unexpected"}}
            with self.assertRaisesRegex(operations.CheckFailure, "unexpectedly succeeded"):
                check.expect_error(machine, ["doc", "accept"], "conflict")

    def test_api_preserves_exact_workspace_operation_field(self):
        with tempfile.TemporaryDirectory() as directory:
            check, machine, _ = self.fixture(Path(directory))
            calls = []
            check.cli = lambda caller, args: calls.append((caller, args))
            check.api(machine, "workspace.operation.prepare", goal="goal", operation={"id": "exact"})
            self.assertEqual(calls[0][1][:2], ["call", "workspace.operation.prepare"])
            self.assertEqual(operations.json.loads(calls[0][1][2]), {"goal": "goal", "operation": {"id": "exact"}})

    def test_small_metadata_transfer_cannot_qualify_multichunk_interruption(self):
        with tempfile.TemporaryDirectory() as directory:
            check, machine, process = self.fixture(Path(directory))
            with self.assertRaisesRegex(operations.CheckFailure, "no interrupted partial transfer"):
                check.interrupt_transfer(machine,
                    lambda: (machine.home / "blobs" / ("a" * 64 + ".staged")).write_bytes(b"metadata"),
                    3 * 1024 * 1024)
            self.assertFalse(process.killed)

    def test_interruption_requires_retained_partial_multichunk_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            check, machine, process = self.fixture(Path(directory))
            check.timeout = 1
            prefix = b"x" * (1024 * 1024)
            result = check.interrupt_transfer(machine,
                lambda: (machine.home / "blobs" / ("a" * 64 + ".staged")).write_bytes(prefix),
                3 * 1024 * 1024)
            self.assertTrue(process.killed)
            self.assertEqual(process.signal, operations.signal.SIGSTOP)
            self.assertEqual(result["rss"]["checkpoint"], "paused_at_partial_transfer")
            self.assertIsNone(machine.process)
            self.assertEqual(result["durable_prefix_bytes"], len(prefix))
            self.assertEqual(result["prefix_sha256"], operations.hashlib.sha256(prefix).hexdigest())

    def test_sampling_failure_still_kills_and_reaps_paused_receiver(self):
        with tempfile.TemporaryDirectory() as directory:
            check, machine, process = self.fixture(Path(directory))
            check.timeout = 1

            def unavailable(*args):
                raise operations.CheckFailure("ps unavailable")

            check.sample_rss = unavailable
            with self.assertRaisesRegex(operations.CheckFailure, "resource sample failed"):
                check.interrupt_transfer(machine,
                    lambda: (machine.home / "blobs" / ("a" * 64 + ".staged")).write_bytes(b"x" * (1024 * 1024)),
                    3 * 1024 * 1024)
            self.assertTrue(process.killed)
            self.assertEqual(process.signal, operations.signal.SIGSTOP)
            self.assertIsNone(machine.process)

    def test_completed_stage_racing_kill_does_not_count_as_partial_resume(self):
        with tempfile.TemporaryDirectory() as directory:
            check, machine, process = self.fixture(Path(directory))
            check.timeout = 1
            staged = machine.home / "blobs" / ("a" * 64 + ".staged")
            full_size = 3 * 1024 * 1024

            def finish_before_kill():
                staged.write_bytes(b"x" * full_size)
                process.killed = True

            process.kill = finish_before_kill
            with self.assertRaisesRegex(operations.CheckFailure, "finished the object"):
                check.interrupt_transfer(machine, lambda: staged.write_bytes(b"x" * (1024 * 1024)), full_size)


if __name__ == "__main__":
    unittest.main()
