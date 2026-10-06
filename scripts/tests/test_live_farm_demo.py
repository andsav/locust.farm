"""Runner regressions without starting a daemon or invoking native model clients."""

from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import sys
import tempfile
import threading
import tomllib
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import live_farm_demo as runner


class LiveDemoTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.demo = runner.Demo(self.root / "demo")
        self.demo.data = {"phases": {}, "goal": "goal", "agents": {}}

    def test_concurrent_phase_saves_preserve_both_updates_and_snapshots(self):
        self.demo.save()
        barrier = threading.Barrier(2)

        def writer(phase):
            demo = runner.Demo(self.demo.root)
            demo.active_phase = phase
            row = {"finished": False}
            demo.data["phases"][phase] = row
            barrier.wait()
            for number in range(15):
                row["progress"] = number
                demo.save()
            row["finished"] = True
            demo.save()

        with ThreadPoolExecutor(max_workers=2) as pool:
            list(pool.map(writer, ("frontend", "backend")))
        stored = json.loads(self.demo.manifest.read_text())
        for phase in ("frontend", "backend"):
            self.assertEqual(stored["phases"][phase], {"finished": True, "progress": 14})
            snapshot = self.demo.root / "phases" / (phase + ".json")
            self.assertEqual(json.loads(snapshot.read_text()), stored["phases"][phase])
            self.assertEqual(snapshot.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.demo.manifest.stat().st_mode & 0o777, 0o600)
        self.assertFalse((self.demo.root / "demo.next.json").exists())

    def test_stale_startup_save_preserves_new_phase_outcomes(self):
        self.demo.save()
        stale = runner.Demo(self.demo.root)
        phase = runner.Demo(self.demo.root)
        phase.active_phase = "backend"
        phase.data["phases"]["backend"] = {"finished": True}
        phase.save()
        stale.data["daemon_pid"] = 123
        stale.save()
        stored = json.loads(stale.manifest.read_text())
        self.assertEqual(stored["phases"], {"backend": {"finished": True}})
        self.assertEqual(stored["daemon_pid"], 123)

    def test_initial_private_file_publication_is_atomic_and_never_replaces_existing_files(self):
        target = self.root / "profile/config"
        payloads = [b"first complete configuration", b"second complete configuration"]
        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(lambda content: runner.private_create(target, content), payloads))
        self.assertEqual(sorted(results), [False, True])
        self.assertIn(target.read_bytes(), payloads)
        self.assertEqual(target.stat().st_mode & 0o777, 0o600)
        self.assertEqual(list(target.parent.iterdir()), [target])
        missing = target.parent / "not-published"
        with patch.object(runner.os, "link", side_effect=OSError("publication failed")):
            with self.assertRaisesRegex(OSError, "publication failed"):
                runner.private_create(missing, b"configuration")
        self.assertFalse(missing.exists())
        self.assertEqual(list(target.parent.iterdir()), [target])

    def test_native_success_requires_consistent_explicit_integer_exit_codes(self):
        valid = {"ok": True, "result": {"exit_code": 0}}
        self.assertTrue(runner.native_succeeded(valid, 0))
        for code in (1, 20, 21, -9, None, False, 0.0):
            self.assertFalse(runner.native_succeeded(valid, code), code)
        for value in (1, -9, None, False, "0", 0.0):
            self.assertFalse(runner.native_succeeded({"ok": True, "result": {"exit_code": value}}, 0), value)
        for result in (None, [], {}, {"ok": False, "result": {"exit_code": 0}},
                       {"ok": 1, "result": {"exit_code": 0}}, {"ok": True},
                       {"ok": True, "result": None}, {"ok": True, "result": {}},
                       {"ok": True, "result": []}):
            self.assertFalse(runner.native_succeeded(result, 0), result)

    def test_launch_preserves_unfinished_phase_on_native_failure_or_invalid_output(self):
        for phase, output in (("native-failed", json.dumps({"ok": True, "result": {"exit_code": 1}})),
                              ("invalid-output", "not JSON")):
            with self.subTest(phase=phase):
                demo = runner.Demo(self.root / phase)
                demo.data = {"phases": {}, "goal": "goal", "url": "https://farm.example/farm/test",
                             "clients": {"coordinator": "/fixture/codex"}}
                demo.workspace("coordinator").mkdir(parents=True)

                def popen(_, stdout, **__):
                    stdout.write(output)
                    stdout.flush()
                    return Mock(pid=123, wait=Mock(return_value=0))

                with patch.object(demo, "start"), patch.object(runner.subprocess, "check_output", return_value="fixture-version"), \
                        patch.object(runner.subprocess, "Popen", side_effect=popen), patch("builtins.print"):
                    with self.assertRaisesRegex(RuntimeError, "Native phase"):
                        demo.launch("coordinator", phase, "fixture prompt")
                stored = runner.Demo(demo.root).data["phases"][phase]
                self.assertFalse(stored["finished"])
                self.assertEqual(stored["exit_code"], 0)
                self.assertIn("finished_at", stored)
                if phase == "native-failed":
                    self.assertEqual(stored["managed_result"]["result"]["exit_code"], 1)

    def test_codex_profile_uses_ambient_api_key_and_preserves_local_choices(self):
        owner = self.root / "owner"
        runner.private_write(owner / ".codex/auth.json", "owner-auth-canary")
        with patch.object(runner.Path, "home", return_value=owner), patch.dict(runner.os.environ, {"OPENAI_API_KEY": "key-canary"}):
            self.demo.prepare_profile("coordinator", "codex")
            profile = self.demo.root / "profiles/coordinator"
            config = profile / ".codex/config.toml"
            parsed = tomllib.loads(config.read_text())
            self.assertEqual(parsed["model"], "gpt-6.1-sol")
            self.assertEqual(parsed["model_provider"], "demo_openai")
            provider = parsed["model_providers"]["demo_openai"]
            self.assertEqual(provider["env_key"], "OPENAI_API_KEY")
            self.assertEqual(provider["wire_api"], "responses")
            self.assertFalse(provider["requires_openai_auth"])
            self.assertEqual(parsed["projects"][str(self.demo.workspace("coordinator"))]["trust_level"], "trusted")
            self.assertNotIn("key-canary", config.read_text())
            self.assertFalse((profile / ".codex/auth.json").exists())
            self.assertEqual(config.stat().st_mode & 0o777, 0o600)
            runner.private_write(config, 'model = "participant-choice"\n')
            skill = profile / ".agents/skills/locust/SKILL.md"
            runner.private_write(skill, "participant skill choice")
            self.demo.prepare_profile("coordinator", "codex")
            self.assertEqual(config.read_text(), 'model = "participant-choice"\n')
            self.assertEqual(skill.read_text(), "participant skill choice")

    def test_missing_codex_api_key_does_not_copy_owner_login(self):
        with patch.dict(runner.os.environ, {}, clear=True):
            with self.assertRaisesRegex(RuntimeError, "OPENAI_API_KEY"):
                self.demo.prepare_profile("coordinator", "codex")
        self.assertFalse((self.demo.root / "profiles/coordinator/.codex/auth.json").exists())

    def test_kimi_profile_repeat_preserves_tokens_and_does_not_duplicate_permission_rule(self):
        owner = self.root / "owner"
        runner.private_write(owner / ".kimi-code/config.toml", 'default_model = "native-model"\n')
        runner.private_write(owner / ".kimi-code/credentials/kimi-code.json", "initial-token")
        runner.private_write(owner / ".kimi-code/oauth/kimi-code", b"")
        with patch.object(runner.Path, "home", return_value=owner):
            self.demo.prepare_profile("backend", "kimi-code")
            profile = self.demo.root / "profiles/backend/.kimi-code"
            config = profile / "config.toml"
            original = config.read_bytes()
            self.assertEqual(tomllib.loads(config.read_text())["permission"]["rules"],
                             [{"decision": "allow", "pattern": "mcp__locust__*"}])
            token = profile / "credentials/kimi-code.json"
            runner.private_write(token, "refreshed-profile-token")
            runner.private_write(owner / ".kimi-code/config.toml", 'default_model = "owner-changed-model"\n')
            runner.private_write(owner / ".kimi-code/credentials/kimi-code.json", "different-owner-token")
            self.demo.prepare_profile("backend", "kimi-code")
            self.assertEqual(config.read_bytes(), original)
            self.assertEqual(token.read_text(), "refreshed-profile-token")
            self.assertEqual(token.stat().st_mode & 0o777, 0o600)

    def test_prepare_recovers_frozen_seed_and_registered_checkouts_after_interruption(self):
        clients = {role: "/fixture/" + role for role in runner.ROLES}
        principals = {role: "principal-" + role for role in runner.ROLES}
        self.demo.data = {"schema": 2, "clients": clients, "service": "https://farm.example",
                          "agents": {}, "phases": {}}
        binary = self.root / "fixture-binary"
        binary.write_bytes(b"not an executable client")
        bindings = {}
        calls = []
        interrupt = [True]

        def call(args, role="coordinator", **_):
            args = list(map(str, args))
            if args[:1] == ["--idempotency-key"]:
                args = args[2:]
            if args[:1] == ["--agent"]:
                role, args = args[1], args[2:]
            calls.append(args[:2])
            operation = tuple(args[:2])
            if args[0] == "status":
                return {"status": {"agents": [{"name": name, "agent": principal} for name, principal in principals.items()],
                                   "goals": [{"goal": "existing-goal", "title": runner.GOAL_TITLE,
                                              "member": principals["coordinator"]}]}}
            if operation == ("goal", "status"):
                return {"goal_status": {"members": [{"member": p} for p in principals.values()],
                                         "current_rules": "rules", "workspace": bindings.get(role),
                                         "roles": {name: [principals["coordinator"]] for name in principals}}}
            if operation in (("farm", "on"), ("farm", "show")):
                return {"farm_preview": {"status": {"farm_id": "farm"}}}
            if operation == ("workspace", "init"):
                return {"operation": {"id": "seed-operation"}}
            if operation == ("workspace", "publish"):
                return {"workspace_operation": {"state": {"recorded": {"event": "seed-proposal"}}}}
            if operation == ("workspace", "integrate"):
                return {"workspace_operation": {"state": {"recorded": {"event": "seed-revision"}}}}
            if args[0] == "checkouts":
                return {"checkouts": [bindings[role]] if role in bindings else []}
            if operation == ("workspace", "connect"):
                destination = Path(args[args.index("--folder") + 1])
                destination.mkdir(parents=True)
                bindings[role] = {"id": "checkout-" + role, "root": str(destination), "base_revision": "seed-revision"}
                if role == "frontend" and interrupt[0]:
                    interrupt[0] = False
                    raise RuntimeError("simulated lost checkout registration response")
                return {"checkout": bindings[role]}
            return {}

        with patch.object(runner.Demo, "start"), patch.object(runner.Demo, "prepare_profile"), \
                patch.object(runner.Demo, "status", return_value={"fixture": True}), patch.object(runner.Demo, "call", side_effect=call):
            with self.assertRaisesRegex(RuntimeError, "simulated"):
                self.demo.prepare(binary, clients, "https://farm.example")
            recovered = runner.Demo(self.demo.root)
            self.assertEqual(recovered.data["goal"], "existing-goal")
            self.assertEqual(recovered.data["seed_operation"], "seed-operation")
            self.assertEqual(recovered.data["seed_revision"], "seed-revision")
            self.assertEqual(recovered.prepare(binary, clients, "https://farm.example"), {"fixture": True})
            count = len(calls)
            recovered.prepare(binary, clients, "https://farm.example")
            self.assertEqual(len(calls), count)
        self.assertEqual(calls.count(["workspace", "init"]), 1)
        self.assertEqual(calls.count(["workspace", "connect"]), 4)
        self.assertNotIn(["goal", "create"], calls)
        self.assertTrue(runner.Demo(self.demo.root).data["prepared"])

    def test_review_integration_and_update_keep_exact_proposal_and_checkout_boundaries(self):
        self.demo.data.update(checkouts={"backend": "backend-checkout"})
        calls = []
        def call(args, **kwargs):
            calls.append((list(map(str,args)), kwargs))
            if args[2:4] == ["workspace", "integrate"]:
                return {"workspace_operation":{"state":{"recorded":{"event":"accepted-revision"}}}}
            return {"fixture":True}
        with patch.object(self.demo, "call", side_effect=call):
            self.demo.proposal("proposal", "reviewer", self.root / "fresh-review")
            self.demo.integrate("proposal")
            self.demo.update("backend", "accepted-revision")
        self.assertEqual(calls[0][0], ["workspace","review","--goal","goal","--proposal","proposal","--destination",str(self.root / "fresh-review")])
        self.assertEqual(calls[0][1], {"role":"reviewer"})
        self.assertEqual(calls[1][0][0], "--idempotency-key")
        self.assertEqual(calls[1][0][2:], ["workspace","integrate","--goal","goal","--proposal","proposal"])
        self.assertEqual(calls[2][0], ["workspace","update","--goal","goal","--checkout","backend-checkout","--revision","accepted-revision"])
        self.assertEqual(calls[2][1], {"role":"backend"})
        self.assertEqual(self.demo.data["head_revision"], "accepted-revision")

    def test_native_stage_requires_task_report_with_exact_workspace_proposal(self):
        for sources, successful in ((["proposal"], True), (["untyped-note"], False)):
            with self.subTest(sources=sources):
                demo = runner.Demo(self.root / ("valid" if successful else "invalid"))
                demo.data = {"phases":{}, "goal":"goal", "url":"https://farm.example/farm/test",
                             "clients":{"coordinator":"/fixture/codex"}, "agents":{"coordinator":"coordinator"},
                             "checkouts":{"coordinator":"checkout"}, "seed_revision":"seed"}
                demo.workspace("coordinator").mkdir(parents=True)
                claim = {"task":"task", "attempt":"attempt", "generation":1}
                report = {"contribution":"report", "attempt":"attempt", "sources":sources}
                def popen(_, stdout, **__):
                    stdout.write(json.dumps({"ok":True,"result":{"exit_code":0}}))
                    stdout.flush()
                    return Mock(pid=123, wait=Mock(return_value=0))
                replies = [{"claimed":claim}, {"workspace_proposals":[{"proposal":"proposal","result_manifest":"manifest"}]}]
                with patch.object(demo,"start"), patch.object(demo,"task",return_value={"task":"task"}), \
                        patch.object(demo,"call",side_effect=replies), patch.object(demo,"contributions",return_value=[report]), \
                        patch.object(runner.subprocess,"check_output",return_value="fixture-version"), \
                        patch.object(runner.subprocess,"Popen",side_effect=popen), patch("builtins.print"):
                    if successful:
                        row=demo.launch("coordinator","implementation","fixture prompt",stage="contract")
                        self.assertTrue(row["finished"])
                        self.assertEqual(row["proposal"],"proposal")
                        self.assertEqual(row["result_manifest"],"manifest")
                    else:
                        with self.assertRaisesRegex(RuntimeError,"exact workspace proposal"):
                            demo.launch("coordinator","implementation","fixture prompt",stage="contract")
                        self.assertFalse(runner.Demo(demo.root).data["phases"]["implementation"]["finished"])
                prompt=(demo.root / "prompts/implementation.txt").read_text()
                self.assertIn("workspace propose",prompt)
                self.assertIn("workspace publish",prompt)
                self.assertIn("workspace review",prompt)
                self.assertIn("workspace update",prompt)
                self.assertIn("--sources",prompt)
                self.assertNotIn("patch create",prompt)
                self.assertNotIn("patch submit",prompt)


if __name__ == "__main__":
    unittest.main()
