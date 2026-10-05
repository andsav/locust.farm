"""Evidence predicates for real model/native tool workflow qualification."""

import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_t2_models as harness
from client_qualification.runtime import Profile, private_write


class ModelWorkflowTests(unittest.TestCase):
    def log(self, events):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        path = Path(temp.name) / "events.jsonl"
        path.write_text("".join(json.dumps(event) + "\n" for event in events))
        return path

    def test_generated_prose_and_echoed_prompt_are_not_native_calls(self):
        for client in ("codex", "claude-code", "factory-droid", "pi"):
            path = self.log([{"type": "user", "message": {"content": "Read skill and run locust workspace publish"}},
                             {"type": "assistant", "message": {"content": [{"type": "text", "text": "All tools succeeded"}]}}])
            self.assertEqual(harness.native_calls(client, path), [])

    def test_native_calls_require_completed_matching_result(self):
        for client, start, end in (
            ("claude-code", {"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "call", "name": "Read", "input": {"file_path": "/private/SKILL.md"}}]}},
             {"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "call", "content": "# Locust collaboration"}]}}),
            ("factory-droid", {"type": "tool_call", "id": "call", "toolId": "Read", "parameters": {"file_path": "/private/SKILL.md"}},
             {"type": "tool_result", "id": "call", "isError": False, "value": "# Locust collaboration"}),
            ("pi", {"type": "tool_execution_start", "toolCallId": "call", "toolName": "read", "args": {"path": "/private/SKILL.md"}},
             {"type": "tool_execution_end", "toolCallId": "call", "isError": False, "result": {"content": [{"type": "text", "text": "# Locust collaboration"}]}})):
            self.assertEqual(harness.native_calls(client, self.log([start])), [])
            calls = harness.native_calls(client, self.log([start, end]))
            self.assertEqual(len(calls), 1)
            self.assertTrue(harness.skill_read(calls, "/private/SKILL.md"))
            self.assertFalse(harness.skill_read(calls, "/other/SKILL.md"))
            self.assertFalse(harness.skill_read([{**calls[0], "success": False}], "/private/SKILL.md"))
            self.assertFalse(harness.skill_read([{**calls[0], "output": "denied"}], "/private/SKILL.md"))

    def test_model_completion_needs_structured_success_and_usage(self):
        cases = (("codex", {"type": "turn.completed", "usage": {"output_tokens": 5}}),
                 ("claude-code", {"type": "result", "subtype": "success", "is_error": False, "usage": {"output_tokens": 5}}),
                 ("factory-droid", {"type": "completion", "usage": {"output_tokens": 5}}),
                 ("pi", {"type": "message_end", "message": {"role": "assistant", "stopReason": "stop", "usage": {"output": 5}}}))
        for client, event in cases:
            self.assertTrue(harness.model_completion(client, self.log([event])))
            self.assertFalse(harness.model_completion(client, self.log([{"type": "assistant", "content": "All done"}])))
        self.assertFalse(harness.model_completion("claude-code", self.log([{"type": "result", "subtype": "success", "is_error": True, "usage": {"output_tokens": 5}}])))

    def test_codex_completed_exit_and_output_are_required_for_shell_skill_read(self):
        item = {"type": "command_execution", "command": "/bin/zsh -c 'cat /private/SKILL.md'", "exit_code": 0,
                "aggregated_output": "# Locust collaboration", "status": "completed"}
        self.assertEqual(harness.native_calls("codex", self.log([{"type": "item.started", "item": item}])), [])
        calls = harness.native_calls("codex", self.log([{"type": "item.completed", "item": item}]))
        self.assertTrue(harness.skill_read(calls, "/private/SKILL.md"))
        self.assertFalse(harness.skill_read([{**calls[0], "output": "unrelated output"}], "/private/SKILL.md"))

    def test_cli_evidence_requires_actual_native_shell_and_success(self):
        call = {"tool": "Bash", "arguments": {"command": "/pinned/locust --json workspace publish --goal g"}, "success": True}
        self.assertTrue(harness.native_operation([call], "workspace publish", "/pinned/locust"))
        self.assertFalse(harness.native_operation([{**call, "success": False}], "workspace publish", "/pinned/locust"))
        self.assertFalse(harness.native_operation([{**call, "tool": "mcp__locust__locust_contribution_publish"}], "workspace publish", "/pinned/locust"))
        self.assertFalse(harness.native_operation([call], "workspace update", "/pinned/locust"))

    def test_original_boundary_preserves_tests_unrelated_work_and_plain_directory(self):
        with tempfile.TemporaryDirectory() as output:
            profile = Profile(output, "model-test")
            self.addCleanup(profile.close)
            source = profile.workspace / "source"
            source.mkdir()
            for name, content in (("calculator.py", harness.BASE_CODE), ("test_calculator.py", harness.TEST_CODE),
                                  ("unrelated.txt", "preserve unrelated local work\n")):
                private_write(source / name, content)
            work = {"source": str(source)}
            self.assertTrue(harness.original_preserved(profile, work))
            self.assertNotEqual(harness.check_tests(profile, source)["exit_code"], 0)
            private_write(source / "calculator.py", harness.FIXED_CODE)
            self.assertFalse(harness.original_preserved(profile, work))
            self.assertTrue(harness.original_preserved(profile, work, harness.FIXED_CODE))
            self.assertEqual(harness.check_tests(profile, source)["exit_code"], 0)
            private_write(source / "unrelated.txt", "lost work\n")
            self.assertFalse(harness.original_preserved(profile, work, harness.FIXED_CODE))


if __name__ == "__main__":
    unittest.main()
