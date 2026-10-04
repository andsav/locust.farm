"""Managed lifecycle proof predicates reject prose, stale receipts and failures."""
import json
from pathlib import Path
import signal
import sys
import tempfile
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import check_managed_clients as harness
from client_qualification.runtime import private_write


def view(detail, **record):
    return {"instance":"instance", "record":{"state":"ready","client_session":"native", "detail":list(json.dumps(detail).encode()),**record}}


class ManagedWorkflowTests(unittest.TestCase):
    def test_each_native_wait_id_is_cleared_only_by_matching_completion(self):
        examples=[
            ({"type":"item.started","item":{"id":"call","type":"mcp_tool_call","tool":"locust_wait"}},
             {"type":"item.completed","item":{"id":"call","type":"mcp_tool_call"}}),
            ({"type":"tool_call","id":"call","toolId":"locust___locust_wait"},
             {"type":"tool_result","id":"call"}),
            ({"type":"assistant","message":{"content":[{"type":"tool_use","id":"call","name":"mcp__locust__locust_wait"}]}},
             {"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"call"}]}}),
            ({"type":"tool_execution_start","toolCallId":"call","toolName":"codemode","args":{"code":"text(await tools.mcp__locust__locust_wait({}));"}},
             {"type":"tool_execution_end","toolCallId":"call"}),
        ]
        with tempfile.TemporaryDirectory() as output:
            path=Path(output)/"native.jsonl"
            for started,completed in examples:
                private_write(path,json.dumps(started)+"\n")
                self.assertEqual(harness.native_wait_ids(path),{"call"})
                wrong=json.loads(json.dumps(completed).replace('"call"','"other"'))
                private_write(path,json.dumps(started)+"\n"+json.dumps(wrong)+"\n")
                self.assertEqual(harness.native_wait_ids(path),{"call"})
                private_write(path,json.dumps(started)+"\n"+json.dumps(completed)+"\n")
                self.assertEqual(harness.native_wait_ids(path),set())
            private_write(path,json.dumps({"type":"text","text":"locust_wait started"})+"\n")
            self.assertFalse(harness.native_wait_started(path))

    def test_ready_requires_independent_exact_native_and_protected_scoped_receipt(self):
        with tempfile.TemporaryDirectory() as output:
            receipt=Path(output)/"ready.jsonl";native=Path(output)/"native.jsonl"
            detail={"initialized":True,"tools_ready":True,"launch_id":"launch","lifecycle_receipt":str(receipt)}
            data=[{"schema":1,"event":"launch_receipt","launch_id":"launch"},
                  {"schema":1,"event":"tools_ready","instance":"instance","pid":123}]
            private_write(receipt,"\n".join(map(json.dumps,data))+"\n")
            private_write(native,json.dumps({"type":"thread.started","thread_id":"native"})+"\n")
            self.assertTrue(harness.ready_evidence(view(detail),native))
            self.assertFalse(harness.ready_evidence(view(detail,client_session="different"),native))
            data[1]["instance"]="other"
            private_write(receipt,"\n".join(map(json.dumps,data))+"\n")
            self.assertFalse(harness.ready_evidence(view(detail),native))
            data[1]["instance"]="instance";data[0]["launch_id"]="old"
            private_write(receipt,"\n".join(map(json.dumps,data))+"\n")
            self.assertFalse(harness.ready_evidence(view(detail),native))
            data[0]["launch_id"]="launch"
            private_write(receipt,"\n".join(map(json.dumps,data))+"\n")
            receipt.chmod(0o644)
            self.assertFalse(harness.ready_evidence(view(detail),native))

    def test_clean_managed_parent_does_not_hide_failed_or_killed_native_child(self):
        clean={"exit_code":0,"natural_cleanup":True,"cleanup_verified":True,"forced_cleanup":False,"timed_out":False}
        self.assertTrue(harness.clean_run(clean))
        for changes in ({"exit_code":1},{"exit_code":-signal.SIGKILL},{"forced_cleanup":True},{"timed_out":True}):
            self.assertFalse(harness.clean_run(dict(clean,**changes)))
        for code,sig,interrupted,expected in ((0,None,False,True),(None,signal.SIGINT,True,True),
                                             (None,signal.SIGKILL,True,False),(1,None,False,False),
                                             (None,None,True,False),(None,signal.SIGINT,False,False)):
            process={"exited":True,"exit_code":code,"termination_signal":sig}
            self.assertEqual(harness.expected_native_exit(view({"process":process}),interrupted),expected)


if __name__=="__main__":
    unittest.main()

class CancellationEvidenceTests(unittest.TestCase):
    def test_requires_exact_effective_acknowledgment_not_empty_pending(self):
        import copy
        cancel = {"view": {"event": "cancel", "standing": "effective"},
                  "body": {"cancel_requested": {"attempt": "attempt"}}, "task": "task"}
        ack = {"view": {"event": "ack", "kind": "cancel_acknowledged", "standing": "effective", "author": "principal"},
               "body": {"cancel_acknowledged": {"cancel": "cancel", "outcome": "uncertain"}}, "task": "task"}
        kwargs = dict(cancel="cancel", attempt="attempt", task="task", principal="principal")
        pending = {"to_acknowledge": []}
        self.assertTrue(harness.cancellation_acknowledged(cancel, ack, pending, **kwargs))
        self.assertFalse(harness.cancellation_acknowledged(cancel, {}, pending, **kwargs))
        for path, value in [(('view','author'),'other'), (('view','standing'),'excluded'), (('task',),'other'),
                            (('body','cancel_acknowledged','cancel'),'other'), (('body','cancel_acknowledged','outcome'),'stopped')]:
            wrong = copy.deepcopy(ack); target = wrong
            for key in path[:-1]: target = target[key]
            target[path[-1]] = value
            self.assertFalse(harness.cancellation_acknowledged(cancel, wrong, pending, **kwargs))
        wrong_cancel = copy.deepcopy(cancel); wrong_cancel['body']['cancel_requested']['attempt'] = 'other'
        self.assertFalse(harness.cancellation_acknowledged(wrong_cancel, ack, pending, **kwargs))
        self.assertFalse(harness.cancellation_acknowledged(cancel, ack, {'to_acknowledge':[{'cancel':'cancel'}]}, **kwargs))
