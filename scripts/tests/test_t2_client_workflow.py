"""False-pass regressions for scripted production-client qualification."""
import copy
import hashlib
import json
from pathlib import Path
import signal
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_t2_clients as harness
from client_qualification.observe_mcp import metadata


class T2WorkflowTests(unittest.TestCase):
    def events(self):
        calls = {}
        metadata({"jsonrpc":"2.0","id":1,"method":"initialize"}, "client_request", calls)
        init = metadata({"jsonrpc":"2.0","id":1,"result": {"protocolVersion":"2025-11-25", "serverInfo":{"name":"locust","version":"1"},"capabilities":{"tools":{}}}}, "bridge_response", calls)
        metadata({"jsonrpc":"2.0","id":2,"method":"tools/list"}, "client_request", calls)
        listed = metadata({"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"locust_goal_status","inputSchema":{"type":"object","properties":{},"additionalProperties":False}}]}}, "bridge_response", calls)
        return [init,listed]

    def test_handshake_requires_valid_envelopes_and_registry(self):
        expected={"locust_goal_status"}
        self.assertEqual(harness.handshake(self.events(),expected),(True,True))
        for key,value in (("jsonrpc",None),("has_result",False),("id",True),("error",{"code":-1})):
            events=self.events()
            for event in events:
                event[key]=value
            self.assertEqual(harness.handshake(events,expected),(False,False))
        self.assertEqual(harness.handshake(self.events(),{"locust_other"}),(True,False))
        events=self.events();events[0]["result"]["protocolVersion"]="unsupported"
        self.assertEqual(harness.handshake(events,expected),(False,True))
        events=self.events();events[1]["tool_definitions"][0]["inputSchema"]={}
        definitions=events[1]["tool_definitions"]
        events[1]["schema_sha256"]=hashlib.sha256(json.dumps({"tools":definitions},sort_keys=True,separators=(",",":")).encode()).hexdigest()
        self.assertEqual(harness.handshake(events,expected),(True,False))

    def receipt_events(self):
        calls = {}
        request = metadata({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                            "params": {"name": harness.READ, "arguments": {}}},
                           "client_request", calls)
        response = metadata({"jsonrpc": "2.0", "id": 1, "result": {"isError": False}},
                            "bridge_response", calls)
        return [request, response]

    def test_receipt_integrity_accepts_notifications_and_interrupted_pending_wait(self):
        events = self.receipt_events()
        events.append(metadata({"jsonrpc": "2.0", "method": "notifications/initialized"},
                               "client_request", {}))
        events.append(metadata({"jsonrpc": "2.0", "id": "wait", "method": "tools/call",
                                "params": {"name": harness.WAIT, "arguments": {}}},
                               "client_request", {}))
        events.append({"event": "client_output_closed"})
        self.assertTrue(harness.receipt_integrity(events))
        events = self.receipt_events()
        events[1].update(has_result=False, error={"code": -32602, "message": "invalid arguments"})
        self.assertTrue(harness.receipt_integrity(events))

    def test_receipt_integrity_rejects_parseable_protocol_and_correlation_failures(self):
        self.assertFalse(harness.receipt_integrity([]))
        self.assertFalse(harness.receipt_integrity([{"event": "invalid_json"}]))
        for index, key, value in ((0, "jsonrpc", "1.0"), (1, "jsonrpc", None),
                                  (0, "id", True), (1, "id", None), (0, "id", []),
                                  (0, "method", ""), (1, "id", 2),
                                  (1, "request_method", "tools/list"),
                                  (1, "tool", harness.WRITE), (1, "has_result", False),
                                  (1, "has_result", 1), (1, "error", {"code": -1, "message": "error"})):
            with self.subTest(index=index, key=key, value=value):
                events = self.receipt_events()
                events[index][key] = value
                self.assertFalse(harness.receipt_integrity(events))
        events = self.receipt_events()
        self.assertFalse(harness.receipt_integrity(events[1:]))
        self.assertFalse(harness.receipt_integrity([events[0], events[0], events[1]]))
        self.assertFalse(harness.receipt_integrity(events + [events[1]]))

    def test_nonzero_policy_denial_exit_allowance_is_exact_observed_droid_case(self):
        run = {"scenario": "default", "permission_profile": "default_headless",
               "interrupted": False, "exit_code": 1,
               "observed_policy_denials": [{"tool_name": "locust___locust_contribution_publish",
                                             "reason": "higher autonomy required"}]}
        self.assertFalse(harness.expected_client_exit(run, "factory-droid", "0.218.1"))
        for code in (2, 127, -signal.SIGKILL, -signal.SIGSEGV, None):
            self.assertFalse(harness.expected_client_exit(dict(run, exit_code=code), "factory-droid", "0.218.1"))
        for key, value in (("scenario", "resume"), ("permission_profile", "deliberately_permissive"),
                           ("interrupted", True), ("observed_policy_denials", []),
                           ("observed_policy_denials", [{"tool_name": harness.READ, "reason": "higher autonomy required"}])):
            self.assertFalse(harness.expected_client_exit(dict(run, **{key: value}), "factory-droid", "0.218.1"))
        self.assertFalse(harness.expected_client_exit(run, "factory-droid", "other"))
        self.assertFalse(harness.expected_client_exit(run, "claude-code", "0.218.1"))

    def test_kill_crash_and_arbitrary_failure_cannot_pass_sigint_cleanup(self):
        for code in (-signal.SIGKILL,-signal.SIGSEGV,1,127,None):
            self.assertFalse(harness.expected_interrupt_exit({"interrupted":True,"exit_code":code}))
        for code in (0,-signal.SIGINT,128+signal.SIGINT):
            self.assertTrue(harness.expected_interrupt_exit({"interrupted":True,"exit_code":code}))
        self.assertFalse(harness.expected_interrupt_exit({"interrupted":False,"exit_code":-signal.SIGINT}))

    def test_codex_documented_interrupt_exit_is_version_and_request_specific(self):
        interrupted = {"interrupted": True, "exit_code": 1}
        self.assertTrue(harness.expected_interrupt_exit(interrupted, "codex", "codex-cli 0.153.4"))
        self.assertFalse(harness.expected_interrupt_exit(interrupted, "codex", "codex-cli other"))
        self.assertFalse(harness.expected_interrupt_exit(interrupted, "claude-code", "codex-cli 0.153.4"))
        self.assertFalse(harness.expected_interrupt_exit({"interrupted": False, "exit_code": 1}, "codex", "codex-cli 0.153.4"))
        self.assertFalse(harness.expected_interrupt_exit({"interrupted": True, "exit_code": -signal.SIGKILL}, "codex", "codex-cli 0.153.4"))

    def test_claim_requires_full_independent_daemon_snapshot(self):
        claim={"goal":"goal","task":"task","attempt":"attempt","instance":"instance","generation":2}
        work={key:claim[key] for key in ("goal","task")}
        event={"jsonrpc":"2.0","has_result":True,"id":1,"direction":"bridge_response","request_method":"tools/call","tool":harness.CLAIM,"isError":False,"result":{"ok":True,"result":{"claimed":claim}}}
        self.assertTrue(harness.exact_claim([event],[claim],work,"instance"))
        self.assertFalse(harness.exact_claim([event],[],work,"instance"))
        for key,value in (("goal","other"),("task","other"),("instance","other"),("generation",0),("generation",True)):
            bad=copy.deepcopy(event);bad["result"]["result"]["claimed"][key]=value
            self.assertFalse(harness.exact_claim([bad],[bad["result"]["result"]["claimed"]],work,"instance"))

    def test_registry_comes_from_exact_installed_contract(self):
        contract={"api_version":2,"protocol_version":2,"operations":[
            {"name":"goal.status","mcp_tool":"locust_goal_status"},
            {"name":"session.show","mcp_tool":None}]}
        response=type("Output",(),{"stdout":json.dumps({"ok":True,"result":contract})})()
        evidence={}
        with patch("check_t2_clients.subprocess.run",return_value=response) as run:
            self.assertEqual(harness.expected_tools("/exact/locust",evidence),{"locust_goal_status"})
            self.assertEqual(run.call_args.args[0],["/exact/locust","--json","contract"])
        self.assertEqual(evidence["api_version"],2)
        self.assertEqual(len(evidence["contract_sha256"]),64)

    def test_invalid_installed_contract_is_not_claimed_as_discovered(self):
        response=type("Output",(),{"stdout":json.dumps({"ok":False})})()
        with patch("check_t2_clients.subprocess.run",return_value=response):
            with self.assertRaises(harness.ProductionError):
                harness.expected_tools("/exact/locust")

    def test_restart_without_contribution_verifies_known_state(self):
        class Daemon:
            goal="goal";principal="principal";endpoint="endpoint";restarted=False
            def call(self,args):
                if args[:2]==["goal","status"]:
                    return {"goal_status":{"goal":self.goal,"administrator":self.principal}}
                if args[:2]==["task","show"]:
                    return {"task":{"view":{"task":"task","attempt":"attempt","attempts":["attempt"]}}}
                if args[0]=="pending":
                    return {"pending":{"claimed":[{"attempt":"attempt","generation":2}]}}
                return {"event":{"text":"known-progress"}}
            def restart(self):
                self.restarted=True
        daemon=Daemon()
        self.assertTrue(harness.persisted_restart(daemon,{"task":"task"},["progress-event"]))
        self.assertTrue(daemon.restarted)


if __name__=="__main__":
    unittest.main()
