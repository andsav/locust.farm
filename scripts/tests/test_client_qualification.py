"""Truth, profile isolation, provider protocol and process cleanup regressions."""

import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.request import Request, urlopen
from urllib.error import HTTPError

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import check_clients as harness
from client_qualification.provider import Provider
from client_qualification.runtime import Process, Profile, SocketFixture, records


class QualificationTests(unittest.TestCase):
    def profile(self, output):
        profile = Profile(output, "test")
        self.addCleanup(profile.close)
        return profile

    def test_missing_binary_is_not_a_pass(self):
        result = harness.qualify("pi", None, None)
        self.assertTrue(all(item["status"] == "not_run" for item in result["assertions"].values()))
        self.assertIsNone(result["version"])

    def test_success_requires_both_mcp_and_authenticated_socket_receipts(self):
        event = {"event": "tools/call", "tool": harness.READ, "phase": "complete", "success": True}
        receipt = {"operation": "read", "phase": "complete", "authenticated_fixture": True}
        self.assertFalse(harness.tool_pass([event], [], harness.READ, "read"))
        self.assertFalse(harness.tool_pass([], [receipt], harness.READ, "read"))
        self.assertTrue(harness.tool_pass([event], [receipt], harness.READ, "read"))
        self.assertFalse(harness.tool_pass([dict(event, success=False)], [receipt], harness.READ, "read"))
        self.assertFalse(harness.tool_pass([event], [dict(receipt, authenticated_fixture=False)], harness.READ, "read"))

    def test_private_profile_excludes_ambient_auth_and_bridge_environment(self):
        with tempfile.TemporaryDirectory() as output:
            profile = self.profile(output)
            env = profile.environment("/usr/bin/true")
            for key in ("OPENAI_API_KEY", "ANTHROPIC_AUTH_TOKEN", "LOCUST_HOME", "LOCUST_SESSION", "LOCUST_CREDENTIAL"):
                self.assertNotIn(key, env)
            for path in (profile.root, profile.home, profile.config, profile.tmp, profile.workspace, profile.fixture):
                self.assertEqual(path.stat().st_mode & 0o777, 0o700)
            for path in (profile.session, profile.credential):
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)
                self.assertEqual(len(path.read_bytes()), 32)
            with self.assertRaises(ValueError):
                profile.apply({"environment": {"LOCUST_SESSION": "/bad"}, "files": []})
            with self.assertRaises(ValueError):
                profile.apply({"environment": {}, "files": [{"relative_path": "../outside", "content": "bad"}]})

    def test_default_and_permissive_policy_are_separate(self):
        for name in ("codex", "claude-code", "factory-droid"):
            default = harness.invocation(name, "/usr/bin/client", [])
            permissive = harness.invocation(name, "/usr/bin/client", [], permissive=True)
            self.assertNotEqual(default, permissive)
            self.assertNotIn("--dangerously-skip-permissions", default)
            self.assertNotIn("--skip-permissions-unsafe", default)
            self.assertNotIn("danger-full-access", default)

    def test_fake_ready_prose_does_not_make_session_or_tool_evidence(self):
        with tempfile.TemporaryDirectory() as output:
            profile = self.profile(output)
            text = profile.logs / "client.stdout"
            text.write_text('I am ready and initialized, all tools successful.\n')
            self.assertIsNone(harness.session_identifier("codex", {"stdout": str(text)}, profile))
            self.assertFalse(harness.tool_pass(records(text), [], harness.READ, "read"))

    def test_malformed_completed_receipts_are_rejected(self):
        with tempfile.TemporaryDirectory() as output:
            path = Path(output) / "bad.jsonl"
            path.write_text('{"event":"initialize"}\nnot json\n')
            with self.assertRaises(ValueError):
                records(path, strict=True)

    def test_pi_native_id_is_not_the_session_file_path(self):
        with tempfile.TemporaryDirectory() as output:
            profile = self.profile(output)
            path = profile.home / ".pi/agent/qualification-session.jsonl"
            path.parent.mkdir(parents=True)
            path.write_text('{"type":"session","id":"original"}\n')
            run = {"stdout": str(profile.logs / "missing.stdout")}
            original = harness.session_identifier("pi", run, profile)
            path.write_text('{"type":"session","id":"replacement"}\n')
            self.assertEqual(original, "original")
            self.assertNotEqual(harness.session_identifier("pi", run, profile), original)
            profile.close()

    def test_declared_codex_namespace_preserved_in_response(self):
        with Provider([harness.READ]) as provider:
            request = Request(provider.url + "/v1/responses", json.dumps({"model": "fixture", "tools": [{
                "type": "namespace", "name": "mcp__locust", "tools": [{"type": "function", "name": harness.READ}]}]}).encode(),
                {"Content-Type": "application/json"})
            with urlopen(request) as response:
                body = json.load(response)
            self.assertEqual(body["output"][0]["namespace"], "mcp__locust")
            self.assertEqual(body["output"][0]["name"], harness.READ)

    def test_droid_discovery_does_not_consume_requested_fixture_step(self):
        with Provider([harness.READ]) as provider:
            request = Request(provider.url + "/v1/chat/completions", json.dumps({"model": "fixture", "tools": [{
                "type": "function", "function": {"name": "ToolSearch"}}]}).encode(),
                {"Content-Type": "application/json"})
            with urlopen(request) as response:
                body = json.load(response)
            self.assertEqual(body["choices"][0]["message"]["tool_calls"][0]["function"]["name"], "ToolSearch")
            self.assertEqual(provider.index, 0)
            self.assertEqual(provider.errors, [])

    def test_provider_only_selects_declared_tool_and_never_forwards(self):
        with Provider([harness.READ]) as provider:
            request = Request(provider.url + "/v1/chat/completions", json.dumps({
                "model": "fixture", "tools": [{"type": "function", "function": {"name": "unknown"}}]}).encode(),
                {"Content-Type": "application/json"})
            with urlopen(request) as response:
                body = json.load(response)
            self.assertNotIn("tool_calls", body["choices"][0]["message"])
            self.assertEqual(provider.index, 0)
            request = Request(provider.url + "/v1/chat/completions", json.dumps({
                "model": "fixture", "tools": [{"type": "function", "function": {"name": "mcp__locust__" + harness.READ}}]}).encode(),
                {"Content-Type": "application/json"})
            with urlopen(request) as response:
                body = json.load(response)
            self.assertEqual(body["choices"][0]["message"]["tool_calls"][0]["function"]["name"], "mcp__locust__" + harness.READ)

    def test_factory_native_session_lookup_is_scripted_redacted_404(self):
        with Provider([harness.READ]) as provider:
            with self.assertRaises(HTTPError) as response:
                urlopen(provider.url + "/api/sessions/private-session-id")
            self.assertEqual(response.exception.code, 404)
            response.exception.close()
            self.assertEqual(provider.requests, [])
            self.assertEqual(provider.index, 0)
            self.assertEqual(provider.backend_requests, [{"method": "GET", "path_template": "/api/sessions/<id>",
                                                        "status": 404, "purpose": "scripted local-session fallback"}])
            self.assertNotIn("private-session-id", json.dumps(provider.backend_requests))

    def test_droid_model_and_backend_urls_are_all_loopback_fixture(self):
        with tempfile.TemporaryDirectory() as output:
            profile = self.profile(output)
            env = harness.provider_settings("factory-droid", profile, "http://127.0.0.1:9999")
            self.assertEqual(env["FACTORY_API_BASE_URL"], "http://127.0.0.1:9999")
            self.assertEqual(env["FACTORY_API_BASE_URL_EU"], "http://127.0.0.1:9999")
            settings = json.loads((profile.home / ".factory/settings.json").read_text())
            self.assertEqual(settings["customModels"][0]["baseUrl"], "http://127.0.0.1:9999/v1")
            self.assertEqual(env["FACTORY_API_KEY"], "fk-locust-dummy-key")

    def test_responses_and_anthropic_streams_complete_with_tool_identity(self):
        for endpoint in ("responses", "messages"):
            with self.subTest(endpoint=endpoint), Provider([harness.READ]) as provider:
                request = Request(provider.url + "/v1/" + endpoint, json.dumps({
                    "model": "fixture", "stream": True, "tools": [{"name": "mcp__locust__" + harness.READ}]}).encode(),
                    {"Content-Type": "application/json"})
                with urlopen(request) as response:
                    text = response.read().decode()
                self.assertIn("mcp__locust__" + harness.READ, text)
                self.assertIn("response.completed" if endpoint == "responses" else "message_stop", text)

    def test_socket_fixture_authentication_write_and_held_wait(self):
        with tempfile.TemporaryDirectory() as output:
            profile = self.profile(output)
            with SocketFixture(profile) as fixture:
                def connect(operation, valid=True):
                    client = socket.socket(socket.AF_UNIX)
                    client.settimeout(1)
                    client.connect(str(profile.fixture / "daemon.sock"))
                    body = {"fixture": "locust-probe-v1", "operation": operation,
                            "session": fixture.session, "credential": fixture.credential if valid else "invalid"}
                    client.sendall((json.dumps(body) + "\n").encode())
                    return client
                with connect("write") as client:
                    self.assertEqual(json.loads(client.recv(4096))["sequence"], 1)
                with connect("read", False) as client:
                    self.assertEqual(client.recv(4096), b"")
                with connect("wait") as client:
                    self.assertTrue(fixture.wait_started.wait(1))
                    fixture.release.set()
                    self.assertEqual(json.loads(client.recv(4096))["sequence"], 1)
                for receipt in fixture.receipts:
                    self.assertNotIn("credential", receipt)
                    self.assertNotIn("session", receipt)

    @unittest.skipUnless(os.name == "posix", "Owned process groups require Unix")
    def test_timeout_cleans_owned_descendants_even_after_leader_exit(self):
        with tempfile.TemporaryDirectory() as output:
            profile = self.profile(output)
            code = 'import subprocess; subprocess.Popen(["/bin/sleep", "60"]); print("leader exited", flush=True)'
            process = Process([sys.executable, "-c", code], profile.environment(sys.executable),
                              profile.workspace, profile.logs, "descendant", 0.2, guarded=False)
            process.wait()
            # The group may temporarily hold an unreaped zombie, but no live
            # sleep descendant may remain. ps status distinguishes that case.
            listing = subprocess.run(["ps", "-axo", "pgid=,stat=,comm="], capture_output=True, text=True, check=True).stdout
            live = [line for line in listing.splitlines() if line.split()[0] == str(process.pgid) and not line.split()[1].startswith("Z")]
            self.assertEqual(live, [])

    @unittest.skipUnless(os.name == "posix", "Owned process groups require Unix")
    def test_timeout_marks_failure_and_reaps_leader(self):
        with tempfile.TemporaryDirectory() as output:
            profile = self.profile(output)
            process = Process(["/bin/sleep", "60"], profile.environment("/bin/sleep"),
                              profile.workspace, profile.logs, "timeout", 0.05, guarded=False)
            run = process.wait()
            self.assertTrue(run["timed_out"])
            self.assertIsNotNone(process.process.returncode)

    def test_relative_binary_path_rejected(self):
        with self.assertRaises(ValueError):
            harness.checked_binary("./codex")

    def test_failure_preserves_observed_version_and_removes_private_profile(self):
        from types import SimpleNamespace
        with tempfile.TemporaryDirectory() as output:
            def fail(_name, _binary, _args, result, profile):
                result["version"] = "observed version"
                result["profiles"] = [{"root": str(profile.root)}]
                raise ValueError("fixture error")
            with patch.object(harness, "_qualify", fail):
                result = harness.qualify("codex", "/usr/bin/true", SimpleNamespace(output=output))
            self.assertEqual(result["version"], "observed version")
            self.assertEqual(result["assertions"]["client_execution"]["status"], "fail")
            self.assertTrue(result["private_runtime_profile_removed"])
            self.assertFalse(Path(result["profiles"][0]["root"]).exists())


if __name__ == "__main__":
    unittest.main()
