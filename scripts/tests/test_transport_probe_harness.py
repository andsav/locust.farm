from pathlib import Path
import os
import signal
import sys
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_transport_probe import (
    CheckFailure,
    Children,
    check_success,
    endpoint_id,
    parse_records,
    prefilled_pipe,
)


class TransportProbeHarnessTests(unittest.TestCase):
    def wait_for_output(self, child):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if child.records():
                return
            if child.process.poll() is not None:
                self.fail("fixture exited before writing output")
            time.sleep(0.01)
        self.fail("fixture did not write output")

    def test_records_preserve_repeated_contacts_and_equals_in_values(self):
        records = parse_records(
            "record=contact endpoint_id=abc direct_addr=127.0.0.1:1234\n"
            "record=contact endpoint_id=abc relay_url=https://relay.invalid/?x=y\n"
        )
        self.assertEqual(len(records), 2)
        self.assertEqual(records[1]["relay_url"], "https://relay.invalid/?x=y")

    def test_malformed_records_do_not_leak_contact_values(self):
        for line in (
            "record=contact direct_addr=192.0.2.1:1234 direct_addr=192.0.2.2:1",
            "record=contact 192.0.2.1:1234",
            "direct_addr=192.0.2.1:1234",
        ):
            with self.subTest(line=line), self.assertRaises(CheckFailure) as failure:
                parse_records(line)
            self.assertNotIn("192.0.2", str(failure.exception))

    def test_endpoint_identity_is_canonical_hex(self):
        identity = "ab" * 32
        self.assertEqual(endpoint_id({"endpoint_id": identity}), identity)
        for value in ("ab" * 31, "gg" * 32, identity.upper()):
            with self.subTest(value=value), self.assertRaises(CheckFailure):
                endpoint_id({"endpoint_id": value})

    def test_success_requires_authenticated_peer_and_selected_final_direct_route(self):
        identity = "ab" * 32
        records = parse_records(
            f"record=peer peer_id={identity}\n"
            "record=path phase=before kind=relay selected=true\n"
            "record=path phase=after kind=direct selected=true\n"
        )
        check_success(records, identity)
        with self.assertRaises(CheckFailure):
            check_success(records, "cd" * 32)
        with self.assertRaises(CheckFailure):
            check_success(records[:2], identity)
        records[-1]["kind"] = "relay"
        with self.assertRaises(CheckFailure):
            check_success(records, identity)

    def test_children_are_reaped_when_a_check_raises(self):
        children = []
        with self.assertRaises(CheckFailure):
            with Children(1) as group:
                for _ in range(2):
                    children.append(group.launch([sys.executable, "-c", "import time; time.sleep(60)"]))
                raise CheckFailure("fixture failure")
        for child in children:
            self.assertIsNotNone(child.process.returncode)
            self.assertFalse(child.stdout_path.exists())

    def test_cleanup_kills_and_reaps_a_child_that_ignores_termination(self):
        with Children(0.1) as group:
            child = group.launch([
                sys.executable, "-c",
                "import signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                "print('record=fixture status=ready', flush=True); time.sleep(60)",
            ])
            self.wait_for_output(child)
        self.assertEqual(child.process.returncode, -signal.SIGKILL)
        self.assertFalse(child.stdout_path.exists())

    def test_harness_wait_timeout_still_reaps_the_process(self):
        with Children(1) as group:
            child = group.launch([sys.executable, "-c", "import time; time.sleep(60)"])
            with self.assertRaises(CheckFailure):
                child.finish(0.01)
        self.assertIsNotNone(child.process.returncode)
        self.assertFalse(child.stdout_path.exists())

    def test_prefilled_pipe_is_blocking_and_has_no_capacity_for_one_byte(self):
        with prefilled_pipe() as output:
            self.assertTrue(os.get_blocking(output.fileno()))
            os.set_blocking(output.fileno(), False)
            with self.assertRaises(BlockingIOError):
                os.write(output.fileno(), b"x")
            os.set_blocking(output.fileno(), True)
        self.assertTrue(output.closed)

    def test_child_blocked_on_output_is_reaped_without_draining_the_pipe(self):
        with prefilled_pipe() as output, Children(1) as group:
            child = group.launch([
                sys.executable, "-c",
                "import os, sys; print('fixture_started', file=sys.stderr, flush=True); "
                "os.write(1, b'record=fixture status=blocked\\n')",
            ], stdout_fd=output.fileno())
            output.close()
            # The fixture marks its write attempt on separate temporary stderr;
            # no readiness guess or stdout read can release the blocked writer.
            deadline = time.monotonic() + 5
            while "fixture_started" not in child.stderr_path.read_text():
                if child.process.poll() is not None or time.monotonic() >= deadline:
                    self.fail("fixture did not attempt its blocked write")
                time.sleep(0.01)
            with self.assertRaises(CheckFailure):
                child.finish(0.01)
        self.assertIsNotNone(child.process.returncode)
        self.assertTrue(output.closed)
        self.assertFalse(child.stderr_path.exists())

    def test_cleanup_continues_after_one_child_cleanup_error(self):
        with Children(1) as group:
            first = group.launch([sys.executable, "-c", "import time; time.sleep(60)"])
            second = group.launch([sys.executable, "-c", "import time; time.sleep(60)"])
            original_close = first.close

            def fail_after_reaping():
                original_close()
                raise OSError("fixture cleanup error")

            with patch.object(first, "close", side_effect=fail_after_reaping):
                with self.assertRaises(OSError):
                    group.__exit__(None, None, None)
            self.assertIsNotNone(second.process.returncode)
        self.assertIsNotNone(first.process.returncode)

    @unittest.skipUnless(hasattr(signal, "SIGUSR1"), "requires POSIX signals")
    def test_output_reads_do_not_move_the_child_write_position(self):
        with Children(1) as group:
            child = group.launch([
                sys.executable, "-c",
                "import signal, time, sys\n"
                "def finish(signum, frame):\n"
                "    print(' sequence=2', flush=True)\n"
                "    sys.exit(0)\n"
                "signal.signal(signal.SIGUSR1, finish)\n"
                "sys.stdout.write('record=fixture sequence=1\\nrecord=fixture'); sys.stdout.flush()\n"
                "while True: time.sleep(1)\n",
            ])
            self.wait_for_output(child)
            self.assertEqual(child.records()[0]["sequence"], "1")
            child.process.send_signal(signal.SIGUSR1)
            code, records = child.finish(5)
            self.assertEqual(code, 0)
            self.assertEqual([record["sequence"] for record in records], ["1", "2"])


if __name__ == "__main__":
    unittest.main()
