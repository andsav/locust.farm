"""Proof-boundary and isolation regressions for hook qualification."""
import json
import os
from pathlib import Path
import sys
import subprocess
import shutil
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_hooks as harness


class HookQualificationTests(unittest.TestCase):
    def setUp(self):
        self.profile = harness.Profile()
        self.addCleanup(self.profile.close)

    def test_all_process_settings_are_derived_from_exact_mktemp_home(self):
        self.assertRegex(str(self.profile.home), r"^/tmp/lh\.[A-Za-z0-9]{6}$")
        with patch.dict(os.environ, {"HOME":"/forbidden/home", "OPENAI_API_KEY":"DO_NOT_RETAIN", "LOCUST_CREDENTIAL":"secret", "CODEX_HOME":"/forbidden/codex"}):
            env = self.profile.environment("/fixture/locust")
        self.assertEqual(env["HOME"], str(self.profile.home))
        self.assertEqual(env["CODEX_HOME"], str(self.profile.home / ".codex"))
        self.assertEqual(env["FACTORY_DROID_AUTO_UPDATE_ENABLED"], "false")
        self.assertNotIn("OPENAI_API_KEY", env)
        self.assertNotIn("LOCUST_CREDENTIAL", env)
        self.assertNotIn("DO_NOT_RETAIN", json.dumps(env))

    def test_only_exact_setup_owned_generic_command_is_selected(self):
        launcher = self.profile.root / "launcher"
        document = {"hooks":{"Stop":[{"hooks":[{"command":"touch /forbidden"}]},
                    {"hooks":[{"statusMessage":"Locust", "command":f"'{launcher}' hook stop --harness codex"}]}]}}
        native, argv = harness.selected_hook(document, launcher, "stop", "codex")
        self.assertEqual(native, "Stop")
        self.assertEqual(argv, [str(launcher), "hook", "stop", "--harness", "codex"])
        for changed in ("/unrelated hook stop --harness codex", f"'{launcher}' hook stop --harness codex; touch /forbidden"):
            document["hooks"]["Stop"][1]["hooks"][0]["command"] = changed
            with self.assertRaises(harness.CheckError):
                harness.selected_hook(document, launcher, "stop", "codex")

    def test_failure_line_or_repeated_block_cannot_pass_stop_proof(self):
        block = {"decision":"block", "reason":"Locust: tasks 1; locust_pending"}
        self.assertTrue(harness.blocks_once(block, None))
        for first, second in ((None,None), ({"systemMessage":"Locust context was NOT injected"},None), (block,block), (block,{}), ({"decision":"block","reason":"opaque"},None)):
            self.assertFalse(harness.blocks_once(first, second))

    def test_synthetic_payload_preserves_exact_typed_mcp_result(self):
        result = {"claimed":{"goal":"g","task":"t","attempt":"a","instance":"i","generation":1}}
        with patch.object(harness, "run", return_value=(0,b"",b"")) as run:
            self.assertIsNone(harness.invoke_hook(self.profile,["/fixture"],"PostToolUse",1,
                call=("attempt.start",{"goal":"g"},result,"native-call")))
        payload = run.call_args.kwargs["input_value"]
        self.assertEqual(payload["tool_name"], "mcp__locust__locust_attempt_start")
        self.assertEqual(payload["tool_use_id"], "native-call")
        self.assertIs(payload["tool_response"]["isError"], False)
        self.assertEqual(payload["tool_response"]["structuredContent"]["result"], result)

    def test_claude_scripted_payload_matches_native_json_text_transport(self):
        with patch.object(harness,"run",return_value=(0,b"",b"")) as run:
            harness.invoke_hook(self.profile,["/fixture"],"PostToolUse",1,client="claude",
                                call=("wait",{"goal":"g","seen":0,"timeout_ms":0},{"waited":"no_event"},"call"))
        response = run.call_args.kwargs["input_value"]["tool_response"]
        self.assertIsInstance(response,str)
        self.assertEqual(json.loads(response),{"ok":True,"result":{"waited":"no_event"}})

    def test_droid_replay_uses_native_name_string_envelope_and_no_invented_id(self):
        with patch.object(harness,"run",return_value=(0,b"",b"")) as run:
            harness.invoke_hook(self.profile,["/fixture"],"PostToolUse",1,client="droid",
                                call=("wait",{"goal":"g","seen":0,"timeout_ms":0},{"waited":"no_event"},"synthetic-id"))
        payload = run.call_args.kwargs["input_value"]
        self.assertEqual(payload["tool_name"],"locust___locust_wait")
        self.assertNotIn("tool_use_id",payload)
        self.assertEqual(json.loads(payload["tool_response"]),{"ok":True,"result":{"waited":"no_event"}})
        launcher = self.profile.root / "launcher"
        document = {"Stop":[{"hooks":[{"statusMessage":"Locust","command":f"'{launcher}' hook stop --harness droid"}]}]}
        self.assertEqual(harness.selected_hook(document,launcher,"stop","droid")[0],"Stop")

    def test_droid_proof_requires_a_matching_successful_typed_native_result(self):
        call = {"type":"tool_call","id":"call-1","toolId":"native-tool","toolName":"locust___locust_wait"}
        result = {"type":"tool_result","id":"call-1","toolId":"native-tool","isError":False,
                  "value":json.dumps({"ok":True,"result":{"waited":"no_event"}})}
        project = lambda rows: harness.native_tool_projection("\n".join(json.dumps(row) for row in rows))
        self.assertEqual(project([call,result])["native_locust_wait_completed"],1)
        for bad in (dict(result,id="another"),dict(result,isError=True),dict(result,value="assistant says success"),
                    dict(result,value=json.dumps({"ok":False})),dict(result,value=json.dumps({"ok":True,"result":None})),dict(result,type="message")):
            self.assertEqual(project([call,bad])["native_locust_wait_completed"],0)

    def test_no_key_and_missing_client_have_distinct_real_model_reasons(self):
        args = (self.profile,"codex",None,self.profile.root,"goal",0,1,None)
        no_key = harness.real_check(*args, {})
        self.assertIn("no API key", no_key["reason"])
        missing = harness.real_check(*args, {"OPENAI_API_KEY":"DO_NOT_RETAIN"})
        self.assertIn("executable missing", missing["reason"])
        self.assertNotIn("DO_NOT_RETAIN", json.dumps(missing))

    def test_native_version_probe_is_not_hook_execution_proof(self):
        with patch.object(harness.shutil, "which", return_value="/native/codex"), patch.object(harness,"run",return_value=(0,b"codex-cli 0.153.4\n",b"")) as run:
            binary, result = harness.native_version(self.profile,"codex",1)
        self.assertEqual(binary,"/native/codex")
        self.assertEqual(run.call_args.args[1], ["/native/codex", "--version"])
        self.assertEqual(result["status"], "pass")
        self.assertIn("hooks not qualified", result["scope"])

    def test_only_native_owned_stop_callbacks_prove_block_then_pass(self):
        command = "/fixture hook stop --harness claude"
        callback = {"type":"system","subtype":"hook_response","hook_event":"Stop", "command":command, "exit_code":0}
        blocked = dict(callback, stdout=json.dumps({"decision":"block","reason":"Locust: work 1"}))
        passed = dict(callback, stdout="")
        text = "\n".join(json.dumps(row) for row in (blocked,passed))
        self.assertTrue(harness.native_hook_projection(text,command)["block_then_empty_stop_callback"])
        for rows in ((blocked,), (dict(blocked,type="assistant"),passed), (blocked,dict(passed,command="other")), (blocked,dict(passed,exit_code=2))):
            self.assertFalse(harness.native_hook_projection("\n".join(json.dumps(row) for row in rows),command)["block_then_empty_stop_callback"])

    def test_native_wrapper_preserves_transport_but_retains_no_payload_values(self):
        wrapper, log = self.profile.fixture / "wrapper.py", self.profile.fixture / "shapes.jsonl"
        harness.private_write(wrapper,harness.SHAPE_WRAPPER)
        payload = {"session_id":"private-native-id","hook_event_name":"PostToolUse","tool_name":"mcp__locust__locust_wait",
                   "tool_input":{"goal":"private-goal-id"},"tool_response":{"content":[{"type":"text","text":"DO_NOT_RETAIN"}],"isError":False},
                   "last_assistant_message":"private free text"}
        raw = json.dumps(payload).encode()
        result = subprocess.run([sys.executable,str(wrapper),str(log),"/bin/cat"], input=raw,capture_output=True,
                                cwd=self.profile.root,env=self.profile.environment(sys.executable),check=True)
        self.assertEqual(result.stdout,raw)
        saved = log.read_text()
        for value in ("private-native-id","private-goal-id","DO_NOT_RETAIN","private free text"):
            self.assertNotIn(value,saved)
        row = json.loads(saved)
        self.assertEqual(row["tool_name"],"mcp__locust__locust_wait")
        self.assertEqual(row["response"]["keys"],["content","isError"])

    def test_pi_replay_routes_nested_native_mcp_result_to_the_extension_driver(self):
        projected={"envelope":None,"through_installed_extension":True,"tool_result_preserved":True}
        with patch.object(harness,"run",return_value=(0,json.dumps(projected).encode(),b"")) as run:
            self.assertIsNone(harness.invoke_hook(self.profile,["/node","/driver","/installed.ts"],"PostToolUse",1,
                client="pi",call=("wait",{"goal":"g","seen":0,"timeout_ms":0},{"waited":"no_event"},"native-id")))
        frame=run.call_args.kwargs["input_value"]
        self.assertEqual(frame["session_id"],"scripted-root")
        event=frame["event"]
        self.assertEqual(event["type"],"tool_result")
        self.assertEqual(event["toolName"],"mcp__locust__locust_wait")
        self.assertEqual(event["toolCallId"],"native-id")
        self.assertEqual(event["details"],{"server":"locust","tool":"locust_wait"})
        self.assertEqual(event["structuredContent"]["structuredContent"],{"ok":True,"result":{"waited":"no_event"}})
        self.assertEqual(event["structuredContent"]["content"],event["content"])
        self.assertFalse(event["structuredContent"]["isError"])

    def test_pi_projection_refuses_unpreserved_native_tool_result(self):
        with patch.object(harness,"run",return_value=(0,b'{"envelope":null,"through_installed_extension":true,"tool_result_preserved":false}',b"")):
            with self.assertRaises(harness.CheckError):
                harness.invoke_hook(self.profile,["/node"],"PostToolUse",1,client="pi")

    def test_missing_pi_is_reported_without_model_or_provider_calls(self):
        result=harness.real_check(self.profile,"pi",None,self.profile.root,"goal",0,1,"model",{"OPENAI_API_KEY":"DO_NOT_RETAIN"})
        self.assertEqual(result["status"],"not_run")
        self.assertIn("Pi executable missing",result["reason"])
        self.assertNotIn("DO_NOT_RETAIN",json.dumps(result))

    def generated_pi_extension(self, line, keep_going=False):
        launcher=self.profile.fixture / "fake-launcher"
        script=("#!"+sys.executable+"\nimport json,sys\npayload=json.load(sys.stdin)\n"
                +"print(json.dumps("+repr({"line":line,"keep_going":keep_going})+"))\n")
        harness.private_write(launcher,script,0o700)
        template=(Path(__file__).resolve().parents[2] / "crates/locust-adapter/src/hooks/pi-shim.ts").read_text()
        source=template.replace("__LOCUST_HOOK_TIMEOUT_MS__","300000").replace("__LOCUST_FAILURE_LINE_JSON__",json.dumps("Locust context was NOT injected")).replace("__LOCUST_LAUNCHER_JSON__",json.dumps(str(launcher)))
        extension=self.profile.home / ".pi/agent/extensions/locust.ts"
        harness.private_write(extension,source)
        return extension

    def pi_driver(self, extension, event):
        node="/opt/homebrew/bin/node" if Path("/opt/homebrew/bin/node").is_file() else shutil.which("node")
        if node is None:
            self.skipTest("Node runtime missing")
        driver=Path(__file__).resolve().parents[1] / "check_pi_hook_driver.mjs"
        return harness.run(self.profile,[node,"--disable-warning=ExperimentalWarning",str(driver),str(extension)],5,
                           input_value={"session_id":"fake-session","event":event})

    def test_pi_driver_loads_actual_source_and_projects_its_native_continuation(self):
        extension=self.generated_pi_extension("Locust: canned fixture feedback",True)
        code,stdout,stderr=self.pi_driver(extension,{"type":"agent_before_settle","outcome":"completed","entries":[],"continue":False})
        self.assertEqual(code,0,stderr)
        self.assertFalse(stderr)
        projection=json.loads(stdout)
        self.assertEqual(projection["envelope"],{"decision":"block","reason":"Locust: canned fixture feedback"})
        self.assertTrue(projection["through_installed_extension"])
        self.assertTrue(projection["tool_result_preserved"])

    def test_pi_driver_preserves_nested_tool_result_when_source_appends_context(self):
        extension=self.generated_pi_extension("Locust: canned tool feedback")
        raw={"isError":False,"content":[{"type":"text","text":json.dumps({"ok":True,"result":{"waited":"no_event"}})}],
             "structuredContent":{"ok":True,"result":{"waited":"no_event"}}}
        event={"type":"tool_result","toolName":"mcp__locust__locust_wait","toolCallId":"native-id",
               "input":{"goal":"g","seen":0,"timeout_ms":0},"content":raw["content"],
               "structuredContent":raw,"details":{"server":"locust","tool":"locust_wait"},"isError":False,
               "usage":{"inputTokens":1,"outputTokens":2}}
        code,stdout,stderr=self.pi_driver(extension,event)
        self.assertEqual(code,0,stderr)
        projection=json.loads(stdout)
        self.assertTrue(projection["tool_result_preserved"])
        self.assertEqual(projection["envelope"]["hookSpecificOutput"]["additionalContext"],"Locust: canned tool feedback")
        self.assertNotIn("native-id",stdout.decode())
        self.assertNotIn("waited",stdout.decode())

    def test_pi_driver_projects_session_compaction_custom_message(self):
        extension=self.generated_pi_extension("Locust: canned restored context")
        code,stdout,stderr=self.pi_driver(extension,{"type":"session_compact"})
        self.assertEqual(code,0,stderr)
        self.assertEqual(json.loads(stdout)["envelope"]["hookSpecificOutput"]["additionalContext"],"Locust: canned restored context")

    def test_pi_driver_refuses_source_outside_its_isolated_home(self):
        other=harness.Profile()
        self.addCleanup(other.close)
        marker=self.profile.fixture / "unexpected-execution"
        extension=other.home / "outside.ts"
        harness.private_write(extension,"import { writeFileSync } from 'node:fs';\nwriteFileSync("+json.dumps(str(marker))+",'unexpected');\nexport default function () {}\n")
        code,stdout,stderr=self.pi_driver(extension,{"type":"session_start","reason":"startup"})
        self.assertNotEqual(code,0)
        self.assertFalse(stdout)
        self.assertEqual(stderr,b"Fake Pi replay failed\n")
        self.assertFalse(marker.exists())

    def test_pi_driver_cannot_pass_a_changed_native_structured_result(self):
        extension=self.profile.home / ".pi/agent/extensions/corrupt.ts"
        source="""export default function (pi) {
          pi.on('tool_result', async (event) => ({
            content: [...event.content, {type:'text',text:'Locust: opaque fixture feedback'}],
            details: event.details,
            structuredContent: {changed: true},
            isError: event.isError,
            usage: event.usage,
          }));
        }
        """
        harness.private_write(extension,source)
        event={"type":"tool_result","content":[{"type":"text","text":"DO_NOT_RETAIN"}],
               "structuredContent":{"preserve":True},"details":{"server":"locust","tool":"locust_wait"},
               "isError":False,"usage":{}}
        code,stdout,stderr=self.pi_driver(extension,event)
        self.assertNotEqual(code,0)
        self.assertFalse(stdout)
        self.assertEqual(stderr,b"Fake Pi replay failed\n")
        self.assertNotIn(b"DO_NOT_RETAIN",stderr)


if __name__ == "__main__":
    unittest.main()
