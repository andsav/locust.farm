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

    def test_prepare_recovers_saved_exports_and_bound_workspaces_after_interruption(self):
        clients = {role: "/fixture/" + role for role in runner.ROLES}
        principals = {role: "principal-" + role for role in runner.ROLES}
        self.demo.data = {"schema": 1, "clients": clients, "service": "https://farm.example",
                          "agents": {}, "phases": {}}
        binary = self.root / "fixture-binary"
        binary.write_bytes(b"not an executable client")
        bindings = {}
        calls = []
        interrupt = [True]

        def call(args, role="coordinator", **_):
            args = list(map(str, args))
            calls.append(args[:2])
            operation = tuple(args[:2])
            if args[0] == "status":
                return {"status": {"agents": [{"name": name, "agent": principal} for name, principal in principals.items()],
                                   "goals": [{"goal": "existing-goal", "title": runner.GOAL_TITLE,
                                              "member": principals["coordinator"]}]}}
            if operation == ("goal", "status"):
                return {"goal_status": {"members": [{"member": p} for p in principals.values()],
                                         "current_rules": "rules", "workspace": bindings.get(role)}}
            if operation in (("farm", "on"), ("farm", "show")):
                return {"farm_preview": {"status": {"farm_id": "farm"}}}
            if operation == ("workspace", "export"):
                bindings[role] = {"export_root": args[args.index("--root") + 1],
                                  "source_commit": args[args.index("--commit") + 1], "exported": "manifest"}
                return {"manifest": "manifest"}
            if operation == ("workspace", "materialize"):
                destination = Path(args[args.index("--destination") + 1])
                destination.mkdir(parents=True)
                bindings.setdefault(role, {}).update(destination=str(destination), integrated="manifest")
                if role == "frontend" and interrupt[0]:
                    interrupt[0] = False
                    raise RuntimeError("simulated lost materialization response")
            return {}

        with patch.object(runner.Demo, "start"), patch.object(runner.Demo, "prepare_profile"), \
                patch.object(runner.Demo, "status", return_value={"fixture": True}), patch.object(runner.Demo, "call", side_effect=call):
            with self.assertRaisesRegex(RuntimeError, "simulated"):
                self.demo.prepare(binary, clients, "https://farm.example")
            recovered = runner.Demo(self.demo.root)
            self.assertEqual(recovered.data["goal"], "existing-goal")
            self.assertEqual(recovered.data["base"], "manifest")
            # Also model a successful export whose manifest checkpoint was lost.
            # Its daemon binding must recover the ID rather than exporting again.
            recovered.data.pop("base")
            recovered.save()
            self.assertEqual(recovered.prepare(binary, clients, "https://farm.example"), {"fixture": True})
            count = len(calls)
            recovered.prepare(binary, clients, "https://farm.example")
            self.assertEqual(len(calls), count)
        self.assertEqual(calls.count(["workspace", "export"]), 1)
        self.assertEqual(calls.count(["workspace", "materialize"]), 4)
        self.assertNotIn(["goal", "create"], calls)
        self.assertTrue(runner.Demo(self.demo.root).data["prepared"])


if __name__ == "__main__":
    unittest.main()
