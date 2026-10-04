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
        self.output = tempfile.TemporaryDirectory()
        self.addCleanup(self.output.cleanup)
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


if __name__ == "__main__":
    unittest.main()
