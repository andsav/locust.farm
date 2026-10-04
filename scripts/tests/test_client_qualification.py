"""Truth, profile isolation, provider protocol and process cleanup regressions."""

import json
from http.client import HTTPConnection
import os
from pathlib import Path
import signal
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
from client_qualification.provider import Provider, planned_call
from client_qualification.runtime import Process, Profile, SocketFixture, records


class QualificationTests(unittest.TestCase):
    def test_native_execute_probe_rejects_parent_success_with_failed_child(self):
        with tempfile.TemporaryDirectory() as output:
            path = Path(output) / "native.stdout"
            call = {"type": "tool_call", "toolId": "Execute", "id": "c",
                    "timestamp": 10, "parameters": {"command": "/bin/echo marker"}}
            reply = {"type": "tool_result", "toolId": "Execute", "id": "c", "timestamp": 70,
                     "isError": True, "error": {"message": "Command terminated by signal: SIGKILL"}}
            run = {"stdout": str(path), "exit_code": 0, "timed_out": False, "forced_cleanup": False}
            path.write_text(json.dumps(call) + "\n" + json.dumps(reply) + "\n")
            failed = harness.execute_probe_outcome(run, "/bin/echo marker", "marker")
            self.assertFalse(failed["executed"])
            self.assertEqual(failed["tool_elapsed_ms"], 60)
            reply.update(isError=False, value="marker\n[Process exited with code 0]")
            reply.pop("error")
            path.write_text(json.dumps(call) + "\n" + json.dumps(reply) + "\n")
            self.assertTrue(harness.execute_probe_outcome(run, "/bin/echo marker", "marker")["executed"])
            self.assertFalse(harness.execute_probe_outcome(run, "/bin/echo other", "marker")["executed"])
            for flag in ("timed_out", "forced_cleanup"):
                self.assertFalse(harness.execute_probe_outcome(dict(run, **{flag: True}), "/bin/echo marker", "marker")["executed"])

    def test_codex_default_approval_denial_requires_structured_tool_failure(self):
        with tempfile.TemporaryDirectory() as output:
            path = Path(output) / "events.jsonl"
            denial = {"type": "item.completed", "item": {"type": "mcp_tool_call", "status": "failed",
                "tool": "locust_contribution_publish", "error": {"message": "MCP tool call requires approval, but approval policy is never"}}}
            path.write_text(json.dumps(denial) + "\n")
            self.assertEqual(harness.permission_denials({"stdout": str(path)})[0]["tool_name"], "locust_contribution_publish")
            denial["item"]["type"] = "agent_message"
            path.write_text(json.dumps(denial) + "\n")
            self.assertEqual(harness.permission_denials({"stdout": str(path)}), [])

    def test_production_arguments_are_resolved_for_each_call_and_copied(self):
        state = {"goal": "g", "generation": 1}
        step = {"tool": "locust_attempt_report", "arguments": lambda: state}
        name, first = planned_call(step)
        state["generation"] = 2
        self.assertEqual(name, "locust_attempt_report")
        self.assertEqual(first["generation"], 1)
        self.assertEqual(planned_call(step)[1]["generation"], 2)
        for invalid in ({"tool": "x"}, {"tool": "x", "arguments": []}, {"tool": "", "arguments": {}}):
            with self.assertRaises(ValueError):
                planned_call(invalid)

    def test_production_arguments_survive_pi_codemode(self):
        arguments = {"goal": "a" * 64, "text": 'A "quoted" note', "task": None}
        with Provider([{"tool": "locust_contribution_publish", "arguments": arguments}]) as provider:
            request = Request(provider.url + "/v1/chat/completions", json.dumps({"model": "fixture", "tools": [{
                "type": "function", "function": {"name": "codemode"}}]}).encode(),
                {"Content-Type": "application/json"})
            with urlopen(request) as response:
                body = json.load(response)
            call = body["choices"][0]["message"]["tool_calls"][0]["function"]
            self.assertEqual(call["name"], "codemode")
            self.assertEqual(json.loads(call["arguments"])["code"],
                             "text(await tools.mcp__locust__locust_contribution_publish(" + json.dumps(arguments) + "));" )

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
            path = profile.home / ".pi/agent/sessions/qualification-session.jsonl"
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

    def test_provider_records_redacted_backend_requests_and_rejections(self):
        with Provider([]) as provider:
            connection = HTTPConnection("127.0.0.1", provider.server.server_port)
            for method, path, body, status in (
                    ("GET", "/private-value?token=secret", None, 404),
                    ("POST", "/private-value", "secret", 403),
                    ("CONNECT", "private-value:443", None, 403),
                    ("DELETE", "/private-value", None, 403),
                    ("TRACE", "/private-value", None, 403),
                    ("CUSTOM", "/private-value", None, 403),
                    ("GET", "/private-value/models", None, 200),
                    ("POST", "/private-value/count_tokens", "{}", 200),
                    ("POST", "/v1/messages", "secret", 400),
                    ("POST", "/private-value/messages", json.dumps({"messages": [{"role": "user", "content": "prompt-secret"}]}), 200)):
                connection.request(method, path, body, {"Authorization": "Bearer provider-secret"})
                response = connection.getresponse()
                self.assertEqual(response.status, status)
                response.read()
            connection.close()
            self.assertEqual(len(provider.backend_requests), 9)
            self.assertEqual([r["status"] for r in provider.backend_requests], [404, 403, 403, 403, 403, 403, 200, 200, 400])
            self.assertEqual(provider.requests[0]["path"], "/messages")
            retained = json.dumps(provider.backend_requests + provider.errors + provider.requests)
            self.assertNotIn("private-value", retained)
            self.assertNotIn("secret", retained)

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

    @unittest.skipUnless(os.name == "posix", "Owned process groups require Unix")
    def test_interruption_requires_natural_descendant_cleanup(self):
        for reap_child, new_group in ((False, False), (False, True), (True, True)):
            with self.subTest(reap_child=reap_child, new_group=new_group), tempfile.TemporaryDirectory() as output:
                profile = self.profile(output)
                ready = profile.workspace / "ready"
                code = "\n".join([
                    "import os, pathlib, signal, subprocess, sys, time",
                    "child = subprocess.Popen(['/bin/sleep', '60'], preexec_fn=os.setpgrp)" if new_group else
                    "child = subprocess.Popen(['/bin/sleep', '60'])",
                    "def stop(*args):",
                    "    child.terminate(); child.wait()" if reap_child else "    pass",
                    "    sys.exit(0)",
                    "signal.signal(signal.SIGINT, stop)",
                    "pathlib.Path(sys.argv[1]).write_text(str(child.pid))",
                    "while True: time.sleep(0.01)",
                ])
                process = Process([sys.executable, "-c", code, str(ready)], profile.environment(sys.executable),
                                  profile.workspace, profile.logs, "interrupt", 1, guarded=False)
                run = process.wait(condition=ready.exists)
                self.assertTrue(run["interrupted"])
                self.assertEqual(run["signal_scope"], "client_leader")
                self.assertTrue(run["observed_exit_before_cleanup"])
                self.assertEqual(run["natural_cleanup"], reap_child)
                self.assertEqual(run["forced_cleanup"], not reap_child)
                self.assertTrue(run["cleanup_verified"])
                self.assertEqual(run["cleanup_scope"], "owned_session_and_receipt_identified_bridges")
                self.assertIn("Fully detached", run["cleanup_limitation"])
                child_status = subprocess.run(["/bin/ps", "-p", ready.read_text(), "-o", "stat="],
                                              capture_output=True, check=False).stdout.decode("ascii").strip()
                self.assertTrue(not child_status or child_status.startswith("Z"), child_status)

    @unittest.skipUnless(os.name == "posix", "Owned process sessions require Unix")
    def test_stale_membership_does_not_authorize_signaling_another_session(self):
        with tempfile.TemporaryDirectory() as output:
            profile = self.profile(output)
            unrelated = subprocess.Popen(["/bin/sleep", "60"], start_new_session=True)
            process = Process(["/bin/sleep", "60"], profile.environment("/bin/sleep"),
                              profile.workspace, profile.logs, "session-identity", 0.2, guarded=False)
            try:
                with patch.object(process, "live_owned_members", return_value=[unrelated.pid]):
                    process.signal_owned(signal.SIGKILL)
                self.assertIsNone(unrelated.poll())
            finally:
                process.close()
                unrelated.terminate()
                unrelated.wait()

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
