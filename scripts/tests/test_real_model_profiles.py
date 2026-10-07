"""Real-provider preparation isolation and explicit invocation regressions."""

import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from client_qualification.real_models import (RedactingProcess, configure_real_provider, install_locust_skill,
                                              provider_model_ids)
from client_qualification.runtime import Profile


class RealModelProfileTests(unittest.TestCase):
    def setUp(self):
        self.output = tempfile.TemporaryDirectory(prefix="lh.", dir="/tmp")
        self.addCleanup(self.output.cleanup)
        mkdtemp = tempfile.mkdtemp
        def private_tempdir(*args, **kwargs):
            if kwargs.get("dir") == "/tmp":
                kwargs["prefix"] = "lh."
            return mkdtemp(*args, **kwargs)
        with patch("client_qualification.runtime.tempfile.mkdtemp", side_effect=private_tempdir):
            self.profile = Profile(self.output.name, "real-test")
        self.addCleanup(self.profile.close)
        self.ambient = {"OPENAI_API_KEY": "openai-test-secret", "ANTHROPIC_API_KEY": "anthropic-test-secret",
                        "FACTORY_API_KEY": "owner-factory-secret", "ANTHROPIC_AUTH_TOKEN": "owner-token",
                        "LOCUST_CREDENTIAL": "/owner/credential", "HOME": "/owner"}

    def configure(self, client, **kwargs):
        return configure_real_provider(client, self.profile, "/usr/bin/true", "model-from-metadata",
                                       ambient=self.ambient, **kwargs)

    def test_only_explicit_provider_key_enters_clean_environment(self):
        for client, expected in (("codex", "OPENAI_API_KEY"), ("claude-code", "ANTHROPIC_API_KEY"),
                                 ("factory-droid", "OPENAI_API_KEY"), ("pi", "OPENAI_API_KEY")):
            result = self.configure(client)
            self.assertEqual(result.environment["LOCUST_HOOKS"], "off")
            for key in self.ambient:
                if key == "HOME":
                    self.assertEqual(result.environment[key], str(self.profile.home))
                elif key == expected:
                    self.assertEqual(result.environment[key], self.ambient[key])
                else:
                    self.assertNotIn(key, result.environment)
            safe = json.dumps(result.metadata) + repr(result)
            for value in self.ambient.values():
                if "secret" in value or value == "owner-token":
                    self.assertNotIn(value, safe)

    def test_configuration_references_keys_and_never_persists_them(self):
        for client in ("codex", "factory-droid"):
            result = self.configure(client)
            for path in result.metadata["configuration_files"]:
                text = Path(path).read_text()
                self.assertIn("OPENAI_API_KEY", text)
                self.assertNotIn(self.ambient["OPENAI_API_KEY"], text)
                self.assertEqual(Path(path).stat().st_mode & 0o777, 0o600)
        settings = json.loads((self.profile.home / ".factory/settings.json").read_text())
        self.assertEqual(settings["customModels"][0]["provider"], "openai")
        self.assertEqual(settings["customModels"][0]["apiKey"], "${OPENAI_API_KEY}")
        self.assertFalse((self.profile.home / ".codex/auth.json").exists())

    def test_droid_supports_explicit_anthropic_byok_without_factory_key(self):
        result = self.configure("factory-droid", provider="anthropic")
        self.assertIn("ANTHROPIC_API_KEY", result.environment)
        self.assertNotIn("OPENAI_API_KEY", result.environment)
        settings = json.loads((self.profile.home / ".factory/settings.json").read_text())
        self.assertEqual(settings["customModels"][0]["provider"], "anthropic")
        self.assertNotIn("FACTORY_API_KEY", result.environment)

    def test_missing_key_and_incompatible_provider_do_not_fall_back_to_owner_auth(self):
        with self.assertRaisesRegex(ValueError, "OPENAI_API_KEY"):
            configure_real_provider("codex", self.profile, "/usr/bin/true", "model", ambient={})
        with self.assertRaisesRegex(ValueError, "native provider"):
            self.configure("claude-code", provider="openai")
        for model in (None, "", "model with whitespace"):
            with self.assertRaisesRegex(ValueError, "explicit provider model"):
                configure_real_provider("codex", self.profile, "/usr/bin/true", model, ambient=self.ambient)

    def test_default_permissive_and_resume_arguments_are_exact_and_separate(self):
        overlay = ["--overlay", "private"]
        for client in ("codex", "claude-code", "factory-droid", "pi"):
            result = self.configure(client)
            default = result.invocation("-literal prompt", overlay)
            permissive = result.invocation("-literal prompt", overlay, resume="native-session",
                                           policy="deliberately-permissive")
            self.assertEqual(default[-2:], ["--", "-literal prompt"])
            self.assertIn("private", default)
            self.assertIn("native-session", permissive)
            if client == "claude-code":
                self.assertEqual(default[default.index("--add-dir") + 1], str(self.profile.home))
            for bypass in ("danger-full-access", "--dangerously-skip-permissions", "--skip-permissions-unsafe"):
                self.assertNotIn(bypass, default)
            if client == "pi":
                self.assertIn("policy_limitation", result.metadata)
                self.assertEqual(Path(result.metadata["session_file"]).parent.name, "sessions")
                self.assertEqual(default[default.index("--session") + 1], result.metadata["session_file"])
            else:
                self.assertNotEqual(default, permissive)
            with self.assertRaises(ValueError):
                result.invocation("prompt", [], policy="permissive")
            self.assertEqual(overlay, ["--overlay", "private"])

    def test_skill_installation_uses_only_private_native_paths_and_does_not_claim_discovery(self):
        source = Path(__file__).resolve().parents[2] / "skills/locust/SKILL.md"
        suffixes = {"codex": ".agents/skills/locust/SKILL.md", "claude-code": ".claude/skills/locust/SKILL.md",
                    "factory-droid": ".factory/skills/locust/SKILL.md", "pi": ".pi/agent/skills/locust/SKILL.md"}
        for client, suffix in suffixes.items():
            installed = install_locust_skill(client, self.profile, source)
            self.assertEqual(Path(installed["path"]), self.profile.home / suffix)
            self.assertEqual(Path(installed["path"]).read_bytes(), source.read_bytes())
            self.assertTrue(installed["discovery"].startswith("not_run"))
            self.assertEqual(Path(installed["path"]).stat().st_mode & 0o777, 0o600)

    def test_metadata_http_error_has_no_secret_body_or_url(self):
        error = HTTPError("https://bad/secret", 401, "key=test-secret", {}, None)
        with patch("client_qualification.real_models.urlopen", side_effect=error):
            with self.assertRaisesRegex(RuntimeError, "openai model metadata HTTP 401") as caught:
                provider_model_ids("openai", 1, ambient=self.ambient)
        self.assertNotIn("secret", str(caught.exception))

    def test_capture_redacts_split_key_before_stdout_stderr_logs_and_keeps_owned_cleanup(self):
        env = self.profile.environment(sys.executable)
        env["OPENAI_API_KEY"] = "selected-private-test-key"
        code = ("import os; key=os.environ['OPENAI_API_KEY'].encode(); "
                "os.write(1,key[:8]); os.write(1,key[8:]+b'\\n'); os.write(2,key+b'\\n')")
        argv = [sys.executable, "-c", code]
        run = RedactingProcess(argv, env, self.profile.workspace, self.profile.logs, "capture", 5,
                               ["OPENAI_API_KEY"]).wait()
        self.assertEqual(run["exit_code"], 0)
        self.assertTrue(run["cleanup_verified"])
        self.assertTrue(run["natural_cleanup"])
        self.assertEqual(run["argv"], argv)
        for path in (run["stdout"], run["stderr"]):
            self.assertNotIn(env["OPENAI_API_KEY"], Path(path).read_text())
            self.assertIn("<redacted-provider-key>", Path(path).read_text())
        self.assertIn("outside this claim", run["capture_redaction"])

    def hook_group(self, command="'/tmp/lh.fixture/locust-cli' hook stop --harness claude"):
        return {"hooks": [{"type": "command", "command": command, "timeout": 300,
                           "statusMessage": "Locust"}]}

    def test_hook_opt_in_merges_existing_settings_without_duplicate_groups(self):
        paths = {"codex": ".codex/hooks.json", "claude-code": ".claude/settings.json"}
        for client, relative in paths.items():
            path = self.profile.home / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            original = {"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "existing-hook"}]}]},
                        "unrelated": {"keep": True}}
            path.write_text(json.dumps(original))
            group = self.hook_group()
            supplied = {"hooks": {"Stop": [group], "SessionStart": [self.hook_group("'/tmp/lh.fixture/locust-cli' hook start --harness claude")]}}
            result = self.configure(client, hooks=True, hook_settings=supplied)
            self.assertNotIn("LOCUST_HOOKS", result.environment)
            settings = json.loads(path.read_text())
            self.assertEqual(settings["unrelated"], original["unrelated"])
            self.assertEqual(settings["hooks"]["Stop"], original["hooks"]["Stop"] + [group])
            before = path.read_bytes()
            self.configure(client, hooks=True, hook_settings=supplied)
            self.assertEqual(path.read_bytes(), before)
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertTrue(result.metadata["hook_execution"].startswith("not_run"))

    def test_hook_opt_in_preserves_setup_settings_bytes_and_claude_loads_explicitly(self):
        path = self.profile.home / ".claude/settings.json"
        path.parent.mkdir()
        original = json.dumps({"hooks": {"Stop": [self.hook_group()]}, "unrelated": True}, indent=4) + "\n"
        path.write_text(original)
        result = self.configure("claude-code", hooks=True)
        self.assertEqual(path.read_text(), original)
        argv = result.invocation("prompt", ["--mcp-config", "private"])
        self.assertNotIn("--bare", argv)
        self.assertEqual(argv[argv.index("--settings") + 1], str(path))
        self.assertEqual(argv[argv.index("--setting-sources") + 1], "")
        self.assertIn("--include-hook-events", argv)
        default = self.configure("claude-code")
        self.assertIn("--bare", default.invocation("prompt", []))
        self.assertEqual(default.environment["LOCUST_HOOKS"], "off")

    def test_existing_codex_toml_and_hook_opt_out_are_preserved_with_provider_overrides(self):
        path = self.profile.home / ".codex/config.toml"
        path.parent.mkdir()
        original = b'# preserve comments\nmodel = "old"\n[features]\nhooks = false\n[mcp_servers.locust]\ncommand = "/private/launcher"\n'
        path.write_bytes(original)
        result = self.configure("codex")
        self.assertEqual(path.read_bytes(), original)
        argv = result.invocation("prompt", ["--overlay", "private"])
        overrides = result.metadata["provider_overrides"]
        self.assertEqual(argv[1:1+len(overrides)], overrides)
        self.assertIn('model_provider="locust_real_openai"', overrides)
        self.assertIn('model_providers.locust_real_openai.env_key="OPENAI_API_KEY"', overrides)
        self.assertNotIn("features.hooks=true", overrides)

    def test_hook_settings_conflicts_and_invalid_inputs_preserve_existing_bytes(self):
        path = self.profile.home / ".claude/settings.json"
        path.parent.mkdir()
        group = self.hook_group()
        original = json.dumps({"hooks": {"Stop": [group]}, "unrelated": True})
        path.write_text(original)
        changed = self.hook_group()
        changed["hooks"][0]["timeout"] = 5
        for supplied in ({"hooks": {"Stop": [changed]}}, {"unrelated": False},
                         {"hooks": {"Stop": "bad"}}, {"hooks": {"Stop": [self.hook_group("${HOME}/locust")]}}):
            with self.assertRaises(ValueError):
                self.configure("claude-code", hooks=True, hook_settings=supplied)
            self.assertEqual(path.read_text(), original)
        with self.assertRaisesRegex(ValueError, "explicit hook"):
            self.configure("claude-code", hook_settings={"hooks": {"Stop": [group]}})
        with self.assertRaisesRegex(ValueError, "supported native adapter"):
            self.configure("factory-droid", hooks=True)

    def test_hook_settings_require_installed_entries_and_refuse_symlink_paths(self):
        with self.assertRaisesRegex(ValueError, "requires adapter settings"):
            self.configure("claude-code", hooks=True)
        self.assertFalse((self.profile.home / ".claude/settings.json").exists())
        path = self.profile.home / ".claude/settings.json"
        path.parent.mkdir()
        sentinel = self.profile.root / "sentinel.json"
        sentinel.write_text('{"preserve":true}')
        path.symlink_to(sentinel)
        with self.assertRaisesRegex(ValueError, "symlinks"):
            self.configure("claude-code", hooks=True, hook_settings={"hooks": {"Stop": [self.hook_group()]}})
        self.assertEqual(sentinel.read_text(), '{"preserve":true}')


if __name__ == "__main__":
    unittest.main()
