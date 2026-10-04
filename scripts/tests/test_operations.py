"""Harness assertions must not turn unavailable evidence into a pass."""

import importlib.util
from pathlib import Path
import sys
import tempfile
import types
import unittest

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

    def test_success_is_not_an_expected_error(self):
        with tempfile.TemporaryDirectory() as directory:
            check, machine, _ = self.fixture(Path(directory))
            check.cli = lambda *args, **kwargs: {"recorded": {"event": "unexpected"}}
            with self.assertRaisesRegex(operations.CheckFailure, "unexpectedly succeeded"):
                check.expect_error(machine, ["doc", "accept"], "conflict")

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
