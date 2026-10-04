"""False-pass and privacy regressions for installed native-client qualification."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from urllib.request import Request, urlopen
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_installed_clients as harness
from client_qualification.provider import Provider


class InstalledClientTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "SKILL.md"
        self.body = "---\nname: locust\ndescription: A unique full skill description.\n---\n\n# Private skill instructions\nDo this authored thing.\n"
        self.path.write_text(self.body)
        self.observer = harness.SkillObserver(self.path)

    def test_request_observer_retains_only_projection_and_first_request_is_independent(self):
        self.observer({"input": "No metadata"})
        self.observer({"input": self.body, "secret": "DO_NOT_RETAIN"})
        self.assertFalse(self.observer.observations[0]["description_present"])
        self.assertTrue(self.observer.observations[1]["body_present"])
        serialized = json.dumps(self.observer.observations)
        self.assertNotIn("DO_NOT_RETAIN", serialized)
        self.assertNotIn("Private skill instructions", serialized)
        self.assertNotIn("A unique full skill description", serialized)
        self.assertEqual(self.observer.observations[0]["skill_sha256"], harness.digest(self.path))

    def test_provider_callback_observes_without_retaining_request(self):
        with Provider([], request_observer=self.observer) as provider:
            body = {"model": "fixture", "messages": [{"role": "system", "content": self.body}], "tools": []}
            request = Request(provider.url + "/chat/completions", json.dumps(body).encode(), {"Content-Type": "application/json"})
            with urlopen(request) as response:
                self.assertEqual(response.status, 200)
        self.assertTrue(self.observer.observations[0]["body_present"])
        self.assertNotIn("Private skill instructions", json.dumps(provider.requests))

    def test_only_completed_native_mcp_receipts_can_pass(self):
        envelope = {"ok": True, "result": {"goal_status": {"goal": "goal", "coordinator": "principal"}}}
        result = {"content": [{"type": "text", "text": json.dumps(envelope)}]}
        event = {"type": "item.completed", "item": {"type": "mcp_tool_call", "id": "one", "server": "locust",
                 "tool": harness.workflow.READ, "status": "completed", "error": None, "result": result}}
        receipts = harness.native_receipts("codex", [event], [], self.observer)
        daemon = Mock(goal="goal", principal="principal")
        self.assertTrue(harness.read_matches(receipts, daemon))
        for changed in ({"type": "item.started"}, {"type": "assistant"}):
            bad = dict(event, **changed)
            self.assertFalse(harness.read_matches(harness.native_receipts("codex", [bad], [], self.observer), daemon))
        for key, value in (("server", "other"), ("status", "failed"), ("error", {"message": "denied"}), ("result", None)):
            bad = dict(event, item=dict(event["item"], **{key: value}))
            self.assertFalse(harness.read_matches(harness.native_receipts("codex", [bad], [], self.observer), daemon))

    def test_claude_toolsearch_response_is_not_requested_locust_execution(self):
        envelope = {"ok": True, "result": {"recorded": {"event": "abc"}}}
        event = {"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "call_fixture_1",
                                                          "content": json.dumps(envelope)}]}}
        issued = [{"requested_tool": harness.workflow.WRITE, "selected_tool": "ToolSearch", "discovery": True}]
        self.assertEqual(harness.native_receipts("claude-code", [event], issued, self.observer), [])
        issued[0].update(selected_tool="mcp__locust__locust_note_add", discovery=False)
        call = {"type":"assistant", "message":{"content":[{"type":"tool_use","id":"call_fixture_1","name":"mcp__locust__locust_note_add","input":{}}]}}
        receipts = harness.native_receipts("claude-code", [call, event], issued, self.observer)
        self.assertEqual(len(harness.successful(receipts, harness.workflow.WRITE)), 1)
        self.assertFalse(harness.successful(harness.native_receipts("claude-code", [event], issued, self.observer), harness.workflow.WRITE))

    def test_native_skill_read_retains_hash_and_match_but_not_body(self):
        numbered = "\n".join(str(index) + "→" + line for index, line in enumerate(self.body.splitlines(), 1))
        event = {"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "call_fixture_1", "content": numbered}]}}
        issued = [{"requested_tool": "Read", "selected_tool": "Read", "discovery": False}]
        call = {"type":"assistant", "message":{"content":[{"type":"tool_use","id":"call_fixture_1","name":"Read","input":{"file_path":str(self.path)}}]}}
        receipts = harness.native_receipts("claude-code", [call, event], issued, self.observer)
        self.assertTrue(receipts[0]["skill_body_match"])
        self.assertTrue(receipts[0]["skill_path_observed"])
        self.assertNotIn("Private skill instructions", json.dumps(receipts))
        self.assertNotIn("locust", receipts[0])
        event["message"]["content"][0]["is_error"] = True
        self.assertFalse(harness.native_receipts("claude-code", [event], issued, self.observer)[0]["skill_body_match"])

    def test_native_note_requires_independent_matching_durable_event(self):
        receipts = [{"success": True, "tool": harness.workflow.WRITE,
                     "locust": [{"ok": True, "result": {"recorded": {"event": "id"}}}]}]
        daemon = Mock(goal="goal", principal="principal")
        daemon.call.return_value = {"event": {"text": "expected", "view": {"author": "principal"}}}
        self.assertTrue(harness.persisted_note(receipts, daemon, "expected"))
        daemon.call.return_value["event"]["text"] = "different"
        self.assertFalse(harness.persisted_note(receipts, daemon, "expected"))
        self.assertFalse(harness.persisted_note([], daemon, "expected"))

    def test_persistent_invocations_do_not_override_registration_or_disable_skills(self):
        profile = Mock(home=Path(self.temp.name))
        for client in harness.CLIENTS:
            for permissive in (False, True):
                argv = harness.invocation(client, "/client", profile, "new-native", permissive)
                self.assertNotIn("--bare", argv)
                self.assertNotIn("--no-skills", argv)
                self.assertFalse(any(argument.startswith(("--mcp-config", "mcp_servers.", "--skill")) for argument in argv))
                self.assertNotIn(str(self.path), argv[-1])


if __name__ == "__main__":
    unittest.main()
