"""Real-pipe contracts for the transparent production-MCP observer."""
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

OBSERVER = Path(__file__).resolve().parents[1] / "client_qualification/observe_mcp.py"
spec = importlib.util.spec_from_file_location("observe_mcp", OBSERVER)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ObserverTests(unittest.TestCase):
    def start(self, code):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        receipt = root / "receipt.jsonl"
        receipt.write_text("")
        receipt.chmod(0o600)
        script = root / "bridge.py"
        script.write_text(code)
        process = subprocess.Popen([sys.executable, str(OBSERVER), "--events-file", str(receipt),
                                    "--executable", sys.executable, "--argument", str(script)],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        def cleanup():
            if process.poll() is None:
                process.kill()
                process.wait()
            for pipe in (process.stdin, process.stdout, process.stderr):
                if not pipe.closed:
                    pipe.close()
        self.addCleanup(cleanup)
        return process, receipt

    def test_passthrough_preserves_bytes_and_eof_naturally_exits(self):
        process, receipt = self.start("import sys\nfor line in sys.stdin.buffer:\n sys.stdout.buffer.write(line);sys.stdout.buffer.flush()\n")
        raw = b'{ "jsonrpc":"2.0", "id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}\ninvalid\xffbytes\n'
        output, error = process.communicate(raw, timeout=5)
        self.assertEqual(process.returncode, 0, error)
        self.assertEqual(output, raw)
        records = [json.loads(line) for line in receipt.read_text().splitlines()]
        self.assertEqual(records[0]["event"], "bridge_started")
        self.assertTrue(any(record["event"] == "client_input_eof" for record in records))
        self.assertTrue(any(record["event"] == "invalid_json" for record in records))
        self.assertEqual(records[-1]["cleanup"], "natural_wait_no_signals")
        self.assertEqual(records[-1]["exit_code"], 0)

    def test_output_reader_closure_with_open_stdin_ends_waiting_bridge(self):
        process, receipt = self.start("import sys\nfor line in sys.stdin.buffer:\n pass\n")
        process.stdin.write(b'{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"locust_wait","arguments":{"goal":"synthetic","seen":1,"timeout_ms":10000}}}\n')
        process.stdin.flush()
        process.stdout.close()
        self.assertEqual(process.wait(timeout=5), 0, process.stderr.read())
        self.assertFalse(process.stdin.closed)
        records = [json.loads(line) for line in receipt.read_text().splitlines()]
        self.assertTrue(any(record["event"] == "client_output_closed" for record in records))
        self.assertEqual(records[-1]["event"], "bridge_exited")
        self.assertEqual(records[-1]["cleanup"], "natural_wait_no_signals")

    def test_cancellation_and_results_forward_exactly_with_redacted_receipts(self):
        code = '''import json,sys
for line in sys.stdin.buffer:
 request=json.loads(line)
 if request.get("method")=="tools/call":
  answer={"jsonrpc":"2.0","id":request["id"],"result":{"isError":False,"structuredContent":{"ok":True,"result":{"invited":{"ticket":"secret-ticket"}}}}}
  sys.stdout.buffer.write((json.dumps(answer)+"\\n").encode());sys.stdout.buffer.flush()
'''
        process, receipt = self.start(code)
        request = {"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "locust_context_read", "arguments": {"goal": "synthetic-goal"}}}
        cancellation = {"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 4}}
        raw = (json.dumps(request) + "\n" + json.dumps(cancellation) + "\n").encode()
        output, error = process.communicate(raw, timeout=5)
        self.assertEqual(process.returncode, 0, error)
        self.assertEqual(json.loads(output)["result"]["structuredContent"]["result"]["invited"]["ticket"], "secret-ticket")
        text = receipt.read_text()
        self.assertNotIn("secret-ticket", text)
        records = [json.loads(line) for line in text.splitlines()]
        self.assertTrue(any(record.get("method") == "notifications/cancelled" and record["requestId"] == 4 for record in records))
        response = next(record for record in records if record.get("direction") == "bridge_response")
        self.assertEqual(response["tool"], "locust_context_read")
        self.assertEqual(response["result"]["result"]["invited"]["ticket"], "<redacted>")

    def test_tools_list_has_deterministic_schema_digest(self):
        calls = {}
        module.metadata({"id": 2, "method": "tools/list"}, "client_request", calls)
        definitions = [{"name": "locust_context_read", "inputSchema": {"type": "object", "properties": {"goal": {"type": "string"}}}}]
        result = module.metadata({"id": 2, "result": {"tools": definitions}}, "bridge_response", calls)
        self.assertEqual(result["tools"], ["locust_context_read"])
        self.assertEqual(result["tool_definitions"], definitions)
        canonical = json.dumps({"tools": result["tool_definitions"]}, sort_keys=True, separators=(",", ":")).encode()
        self.assertEqual(result["schema_sha256"], hashlib.sha256(canonical).hexdigest())

    def test_symlink_receipt_is_refused_without_launching_child(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "target"
            target.write_text(""); target.chmod(0o600)
            link = Path(root) / "link"
            link.symlink_to(target)
            run = subprocess.run([sys.executable, str(OBSERVER), "--events-file", str(link), "--executable", sys.executable], capture_output=True)
            self.assertNotEqual(run.returncode, 0)
            self.assertEqual(target.read_text(), "")


if __name__ == "__main__":
    unittest.main()
