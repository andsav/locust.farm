from pathlib import Path
import errno
import json
import os
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_t1 import (CheckFailure, Machine, Qualification, identity, local_multicast, mdns_query,
                      process_stop, redact, route_fact)


class FakeMulticastSocket:
    """A UDP socket whose send may fail and whose reads return scripted datagrams."""

    def __init__(self, send_error=None, replies=()):
        self.send_error, self.replies, self.sent = send_error, list(replies), None

    def __call__(self, *_args):
        return self

    def __enter__(self):
        return self

    def __exit__(self, *_exc):
        return False

    def setsockopt(self, *_args):
        pass

    def bind(self, _address):
        pass

    def settimeout(self, _seconds):
        pass

    def sendto(self, data, _address):
        if self.send_error is not None:
            raise self.send_error
        self.sent = data

    def recvfrom(self, _size):
        if not self.replies:
            raise TimeoutError
        return self.replies.pop(0)(self.sent), ("192.0.2.1", 5353)


class T1HarnessTests(unittest.TestCase):
    def test_redaction_removes_tickets_and_secret_fields_but_keeps_public_paths(self):
        ticket = "locust-invite-" + "ab" * 150
        secret = list(range(32))
        value = {
            "result": {"invited": {"ticket": ticket}},
            "command": ["goal", "invite", "--goal", "ab" * 32],
            "credential": secret, "session_secret": secret,
            "credential_path": "/tmp/m2/agents/worker.credential",
            "session_path": "/tmp/m2/sessions/worker.secret",
            "diagnostic": f"unexpected {ticket}; secret_key={secret}",
        }
        output = redact(value)
        encoded = json.dumps(output)
        self.assertNotIn(ticket, encoded)
        self.assertNotIn(str(secret), encoded)
        self.assertEqual(output["credential"], "<redacted>")
        self.assertEqual(output["credential_path"], value["credential_path"])
        self.assertEqual(output["session_path"], value["session_path"])

    def test_routes_capture_only_public_snapshot_fields(self):
        peer = "ab" * 32
        parsed = route_fact(f"locust: peer {peer} paths [PathSnapshot {{ kind: Direct, selected: true, rtt: 1.2ms }}, PathSnapshot {{ kind: Relay, selected: false, rtt: 4ms }}]")
        self.assertEqual(parsed, {"peer_endpoint": peer, "paths": [
            {"kind": "direct", "selected": True, "rtt": "1.2ms"},
            {"kind": "relay", "selected": False, "rtt": "4ms"},
        ]})
        self.assertIsNone(route_fact("a raw arbitrary line"))
        self.assertIsNone(route_fact("locust: secret=not-public"))

    def test_identifiers_must_be_full_canonical_public_ids(self):
        self.assertEqual(identity("ab" * 32, "event"), "ab" * 32)
        for value in ("ab" * 31, "AB" * 32, "g" * 64, None):
            with self.subTest(value=value), self.assertRaises(CheckFailure):
                identity(value, "event")

    def suite(self, root):
        return Qualification(Path(sys.executable), 0.05, root / "artifacts", network="local")

    def test_cli_transcript_redacts_invitation_response(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = self.suite(root)
            machine = Machine(1, root / "m1")
            ticket = "locust-invite-" + "ab" * 150
            body = {"ok": True, "result": {"invited": {"ticket": ticket}}}
            completed = subprocess.CompletedProcess([], 0, json.dumps(body), "")
            with patch("check_t1.subprocess.run", return_value=completed):
                self.assertEqual(suite.cli(machine, ["goal", "invite", "--goal", "ab" * 32], owner=True), body["result"])
            suite.transcript.close()
            report = (suite.artifact_dir / "transcript.jsonl").read_text()
            self.assertNotIn(ticket, report)
            self.assertIn("<redacted>", report)

    def test_invalid_child_output_is_not_copied_to_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = self.suite(root)
            machine = Machine(1, root / "m1")
            raw = "locust-invite-" + "ab" * 150
            completed = subprocess.CompletedProcess([], 1, raw, "")
            with patch("check_t1.subprocess.run", return_value=completed), self.assertRaises(CheckFailure):
                suite.cli(machine, ["status"], owner=True)
            suite.transcript.close()
            self.assertNotIn(raw, (suite.artifact_dir / "transcript.jsonl").read_text())

    def test_explicit_wait_deadline_fails_when_progress_never_arrives(self):
        with tempfile.TemporaryDirectory() as directory:
            suite = self.suite(Path(directory))
            with self.assertRaises(CheckFailure):
                suite.wait("fixture does not converge", lambda: False)
            self.assertIsNone(suite.deadline)
            suite.transcript.close()

    def test_environment_never_inherits_operator_credentials_or_bind(self):
        with tempfile.TemporaryDirectory() as directory:
            suite = self.suite(Path(directory))
            machine = Machine(2, Path(directory) / "m2")
            with patch.dict(os.environ, {
                "LOCUST_HOME": "/operator", "LOCUST_CREDENTIAL": "/operator/owner",
                "LOCUST_SESSION": "/operator/session", "LOCUST_BIND": "192.0.2.1:9",
                "LOCUST_RELAY": "https://operator.invalid", "LOCUST_LOOKUP": "all",
            }):
                env = suite.environment(machine)
            self.assertEqual(env["LOCUST_HOME"], str(machine.home))
            self.assertEqual(env["LOCUST_RELAY"], "none")
            self.assertEqual(env["LOCUST_LOOKUP"], "local")
            self.assertNotIn("LOCUST_CREDENTIAL", env)
            self.assertNotIn("LOCUST_SESSION", env)
            self.assertNotIn("LOCUST_BIND", env)
            suite.transcript.close()

    def test_process_cleanup_kills_and_reaps_only_launched_process(self):
        process = subprocess.Popen([
            sys.executable, "-c",
            "import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); "
            "print('ready',flush=True); time.sleep(60)",
        ], stdout=subprocess.PIPE, text=True)
        try:
            self.assertEqual(process.stdout.readline().strip(), "ready")
            self.assertEqual(process_stop(process, 0.05), -signal.SIGKILL)
            self.assertIsNotNone(process.poll())
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            process.stdout.close()

    def test_daemon_cleanup_runs_when_the_qualification_flow_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            suite = self.suite(Path(directory))
            with patch("check_t1.binary_facts", return_value={"sha256": "fixture"}), \
                    patch("check_t1.local_multicast", return_value=None), \
                    patch.object(suite, "flow", side_effect=CheckFailure("fixture flow failure")), \
                    patch.object(suite, "cleanup") as cleanup:
                self.assertFalse(suite.run())
                cleanup.assert_called_once()
            summary = json.loads((suite.artifact_dir / "summary.json").read_text())
            self.assertEqual(summary["status"], "failed")
            self.assertIn("not published-build or three-Mac", summary["qualification"])

    def test_the_multicast_probe_asks_one_standard_mdns_question(self):
        query = mdns_query(0x1234)
        self.assertEqual(query[:12], bytes.fromhex("123400000001000000000000"))
        self.assertEqual(query[12:], b"\x07_irohv1\x04_udp\x05local\x00\x00\x0c\x00\x01")

    def test_the_multicast_probe_needs_its_own_query_back_over_loopback(self):
        heard = FakeMulticastSocket(replies=[lambda sent: b"another responder", lambda sent: sent])
        with patch("check_t1.socket.socket", heard):
            self.assertIsNone(local_multicast(0.5))
        dropped = FakeMulticastSocket(replies=[lambda sent: b"another responder"])
        with patch("check_t1.socket.socket", dropped):
            self.assertIn("did not come back", local_multicast(0.5))

    def test_the_multicast_probe_names_a_denied_send(self):
        # macOS Local Network privacy refuses with EHOSTUNREACH; a sandbox with EPERM.
        for code in (errno.EHOSTUNREACH, errno.EPERM):
            denied = FakeMulticastSocket(send_error=OSError(code, os.strerror(code)))
            with self.subTest(code=code), patch("check_t1.socket.socket", denied):
                self.assertTrue(local_multicast(0.5).startswith(errno.errorcode[code] + " ("))

    def test_a_local_run_without_multicast_fails_as_the_environment_before_any_daemon(self):
        with tempfile.TemporaryDirectory() as directory:
            suite = self.suite(Path(directory))
            reason = "EHOSTUNREACH (No route to host)"
            with patch("check_t1.binary_facts", return_value={"sha256": "fixture"}), \
                    patch("check_t1.local_multicast", return_value=reason), \
                    patch.object(suite, "flow") as flow:
                self.assertFalse(suite.run())
                flow.assert_not_called()
            self.assertEqual(suite.machines, [])
            summary = json.loads((suite.artifact_dir / "summary.json").read_text())
            self.assertEqual(summary["transport"]["local_multicast"], reason)
            self.assertTrue(summary["failures"][0].startswith(
                "environment: local-network multicast is unavailable to this run: " + reason))

    def test_a_default_network_run_does_not_depend_on_multicast(self):
        with tempfile.TemporaryDirectory() as directory:
            suite = Qualification(Path(sys.executable), 0.05, Path(directory) / "artifacts")
            with patch("check_t1.binary_facts", return_value={"sha256": "fixture"}), \
                    patch("check_t1.local_multicast", side_effect=AssertionError("probed")), \
                    patch.object(suite, "flow", side_effect=CheckFailure("fixture flow failure")), \
                    patch.object(suite, "cleanup"):
                self.assertFalse(suite.run())
            summary = json.loads((suite.artifact_dir / "summary.json").read_text())
            self.assertEqual(summary["failures"], ["fixture flow failure"])
            self.assertNotIn("local_multicast", summary["transport"])


if __name__ == "__main__":
    unittest.main()
