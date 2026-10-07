"""Isolated real-provider profiles. No account stores or normal profiles are read.

Callers must choose a model from provider metadata or a current primary catalog.
Configuration and skill installation are preparation, not model/tool evidence.
The environment contains secrets and must never be included in reports.
"""

from dataclasses import dataclass, field
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import sys
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from .runtime import Process, private_write


PROVIDERS = {
    "openai": ("OPENAI_API_KEY", "https://api.openai.com/v1"),
    "anthropic": ("ANTHROPIC_API_KEY", "https://api.anthropic.com"),
}
SKILL_PATHS = {
    "codex": ".agents/skills/locust/SKILL.md",
    "claude-code": ".claude/skills/locust/SKILL.md",
    "factory-droid": ".factory/skills/locust/SKILL.md",
    "pi": ".pi/agent/skills/locust/SKILL.md",
}
POLICIES = ("default", "deliberately-permissive")
HOOK_SETTINGS = {"codex": ".codex/hooks.json", "claude-code": ".claude/settings.json",
                 "factory-droid": ".factory/hooks.json"}


def _profile_path(profile, relative):
    """Read and write only the caller's isolated, plain native profile paths."""
    home = Path(profile.home)
    if home.is_symlink() or not home.is_relative_to(Path(profile.root)):
        raise ValueError("Native settings must stay inside the isolated profile")
    path = home / relative
    for component in (path, *path.parents):
        if component.is_symlink():
            raise ValueError("Native settings cannot follow profile symlinks")
        if component == home:
            break
    if path.exists() and not path.is_file():
        raise ValueError("Native settings must be regular files")
    return path


def _merge_settings(current, supplied, hooks=False):
    """Merge adapter JSON without replacing existing hook groups or preferences."""
    if not isinstance(current, dict) or not isinstance(supplied, dict):
        raise ValueError("Native hook settings must be JSON objects")
    result = json.loads(json.dumps(current))
    for key, value in supplied.items():
        if (hooks or key == "hooks") and isinstance(value, list):
            existing = result.setdefault(key, [])
            if not isinstance(existing, list) or any(not isinstance(group, dict) for group in value):
                raise ValueError("Native hook events must contain group arrays")
            for group in value:
                handlers = group.get("hooks")
                if not isinstance(handlers, list):
                    raise ValueError("Native hook groups must contain handlers")
                commands = []
                for handler in handlers:
                    command = handler.get("command") if isinstance(handler, dict) else None
                    if not isinstance(command, str) or "${" in command or any(ord(char) < 32 for char in command):
                        raise ValueError("Native hook commands must be literal adapter commands")
                    commands.append(command)
                if group in existing:
                    continue
                if any(handler.get("command") in commands for old in existing if isinstance(old, dict)
                       for handler in old.get("hooks", []) if isinstance(handler, dict)):
                    raise ValueError("Native hook command is already configured differently")
                existing.append(group)
        elif isinstance(value, dict):
            result[key] = _merge_settings(result.get(key, {}), value, hooks=hooks or key == "hooks")
        elif key in result and result[key] != value:
            raise ValueError("Native settings conflict; preserving existing configuration")
        else:
            result[key] = value
    return result


def _hook_settings(client, profile, supplied):
    path = _profile_path(profile, HOOK_SETTINGS[client])
    current = json.loads(path.read_text()) if path.exists() else {}
    unwrapped = client == "factory-droid"
    if unwrapped and ("hooks" in current or isinstance(supplied, dict) and "hooks" in supplied):
        raise ValueError("Droid hooks.json must use the unwrapped event map")
    merged = _merge_settings(current, {} if supplied is None else supplied, hooks=unwrapped)
    events = merged if unwrapped else merged.get("hooks")
    if isinstance(events, dict) and any(not isinstance(groups, list) or
                                       any(not isinstance(group, dict) for group in groups)
                                       for groups in events.values()):
        raise ValueError("Native hook events must contain group arrays")
    if not isinstance(events, dict) or not any(isinstance(groups, list) and groups for groups in events.values()):
        raise ValueError("Hook qualification requires adapter settings or setup-installed hooks")
    if not path.exists() or current != merged:
        private_write(path, json.dumps(merged))
    return path


def _droid_model_settings(profile, model, provider, name, base):
    """Append or locate the qualification model without changing other BYOKs."""
    path = _profile_path(profile, ".factory/settings.json")
    current = json.loads(path.read_text()) if path.exists() else {}
    if not isinstance(current, dict):
        raise ValueError("Droid settings must be a JSON object")
    rows = current.get("customModels", [])
    if not isinstance(rows, list) or any(not isinstance(row, dict) for row in rows):
        raise ValueError("Droid customModels must be an array of objects")
    desired = {"model": model, "baseUrl": base, "apiKey": "${" + name + "}", "provider": provider}
    existing = next(((index, row) for index, row in enumerate(rows)
                     if isinstance(row.get("displayName"), str)
                     and re.fullmatch(r"Locust Real(?: [1-9][0-9]*)?", row["displayName"])
                     and all(row.get(key) == value for key, value in desired.items())), None)
    if existing is not None:
        index, row = existing
    else:
        labels = {row.get("displayName") for row in rows if isinstance(row.get("displayName"), str)}
        display, suffix = "Locust Real", 2
        while display in labels:
            display = "Locust Real " + str(suffix)
            suffix += 1
        row = dict(desired, displayName=display)
        index = len(rows)
        current["customModels"] = [*rows, row]
        private_write(path, json.dumps(current))
    return path, "custom:" + row["displayName"].replace(" ", "-") + "-" + str(index)


CAPTURE = '''import os,selectors,subprocess,sys
names=sys.argv[1].split(',')
secrets=[os.environ[name].encode() for name in names if os.environ.get(name)]
child=subprocess.Popen(sys.argv[2:],stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
selector=selectors.DefaultSelector()
buffers={}
for stream,destination in [(child.stdout,1),(child.stderr,2)]:
    selector.register(stream,selectors.EVENT_READ,destination)
    buffers[stream]=bytearray()
def emit(data,destination):
    for secret in secrets: data=data.replace(secret,b'<redacted-provider-key>')
    while data:
        count=os.write(destination,data)
        data=data[count:]
while selector.get_map():
    for key,_ in selector.select():
        data=os.read(key.fileobj.fileno(),65536)
        buffer=buffers[key.fileobj]
        if not data:
            emit(bytes(buffer),key.data)
            selector.unregister(key.fileobj)
            key.fileobj.close()
            continue
        buffer.extend(data)
        while b'\\n' in buffer:
            line,_,remaining=buffer.partition(b'\\n')
            emit(bytes(line)+b'\\n',key.data)
            buffer[:]=remaining
sys.exit(child.wait())
'''


class RedactingProcess(Process):
    """Redact named provider keys BEFORE retained stdout/stderr evidence writes.

    This does not redact client internal state or native-tool results before
    they reach the provider. Process ownership is the wrapper's private session;
    interruption of a client leader is not qualified by this helper.
    """

    def __init__(self, argv, env, cwd, logs, name, timeout, secret_keys):
        keys = list(secret_keys)
        if not keys or any(key not in {item[0] for item in PROVIDERS.values()} for key in keys):
            raise ValueError("Only explicitly named provider API keys may be redacted")
        super().__init__([sys.executable, "-c", CAPTURE, ",".join(keys), *argv], env, cwd,
                         logs, name, timeout, guarded=False)
        self.argv = list(argv)

    def wait(self, condition=None, observe=None):
        if condition is not None:
            raise ValueError("Redacting process does not qualify client-leader interruption")
        result = super().wait(observe=observe)
        result["capture_redaction"] = "Named provider keys redacted before retained stdout/stderr; client internal/model-visible results remain outside this claim"
        result["owned_process_session_leader"] = "redaction-wrapper with actual client child"
        return result


def _key(provider, ambient):
    if provider not in PROVIDERS:
        raise ValueError("Unsupported real provider")
    name, _ = PROVIDERS[provider]
    value = ambient.get(name)
    if not isinstance(value, str) or not value.strip():
        raise ValueError("Missing explicitly named " + name)
    return name, value


def provider_model_ids(provider, timeout, ambient=None):
    """Read the first metadata page using one named key; no raw auth/errors."""
    name, key = _key(provider, os.environ if ambient is None else ambient)
    base = PROVIDERS[provider][1]
    headers = ({"Authorization": "Bearer " + key} if name == "OPENAI_API_KEY" else
               {"x-api-key": key, "anthropic-version": "2023-06-01"})
    request = Request(base + "/v1/models" if provider == "anthropic" else base + "/models",
                      headers=headers)
    try:
        with urlopen(request, timeout=timeout) as response:
            data = json.load(response)
    except HTTPError as error:
        status = error.code
        error.close()
        raise RuntimeError(provider + " model metadata HTTP " + str(status)) from None
    except (URLError, OSError, ValueError):
        raise RuntimeError(provider + " model metadata unavailable") from None
    rows = data.get("data") if isinstance(data, dict) else None
    if not isinstance(rows, list):
        raise RuntimeError(provider + " model metadata malformed")
    return sorted({row["id"] for row in rows if isinstance(row, dict) and
                   isinstance(row.get("id"), str) and row["id"]})


@dataclass(repr=False)
class RealProvider:
    client: str
    binary: str
    profile: object = field(repr=False)
    model: str
    provider: str
    environment: dict = field(repr=False)
    metadata: dict

    def __repr__(self):
        return "RealProvider(" + repr(self.metadata) + ")"

    def invocation(self, prompt, overlay, resume=None, policy="default"):
        if policy not in POLICIES:
            raise ValueError("Unknown qualification policy")
        permissive = policy == "deliberately-permissive"
        if self.client == "codex":
            argv = [self.binary, *self.metadata.get("provider_overrides", []), *overlay]
            if permissive:
                argv += ["-a", "never", "-s", "danger-full-access"]
            argv += ["exec", "--skip-git-repo-check", "--json", "--model", self.model]
            if resume:
                argv += ["resume", resume]
            return [*argv, "--", prompt]
        if self.client == "claude-code":
            native = (["--setting-sources", "", "--settings", self.metadata["hook_settings_file"],
                       "--include-hook-events"] if self.metadata["hooks_enabled"] else ["--bare"])
            argv = [self.binary, *native, "-p", "--verbose", "--output-format", "stream-json",
                    "--model", self.model, "--add-dir", str(self.profile.home), *overlay]
            if permissive:
                argv += ["--dangerously-skip-permissions"]
            if resume:
                argv += ["--resume", resume]
            return [*argv, "--", prompt]
        if self.client == "factory-droid":
            argv = [self.binary, "exec", "--output-format", "stream-json",
                    "--model", self.metadata["custom_model_id"], *overlay]
            if permissive:
                argv += ["--skip-permissions-unsafe"]
            if resume:
                argv += ["--session-id", resume]
            return [*argv, "--", prompt]
        argv = [self.binary, "--print", "--mode", "json", "--provider", self.provider,
                "--model", self.model, *overlay]
        session = resume or self.metadata["session_file"]
        return [*argv, "--session", session, "--", prompt]


def configure_real_provider(client, profile, binary, model, ambient=None, provider=None, *,
                            hooks=False, hook_settings=None):
    """Use a clean Profile and exactly the selected provider's ambient API key.

    Apply config_probe's proposal independently with Profile.apply. Keys are
    referenced by name in config, not persisted. Factory account authentication
    remains unverified unless the caller observes a successful real request.
    Hooks stay off except in an explicit hook qualification. Adapter JSON is
    merged into the selected private native file; setup-installed entries can
    be used as-is. Claude loads that file explicitly without bare mode.
    """
    if client not in SKILL_PATHS:
        raise ValueError("Unsupported qualification client")
    if not isinstance(hooks, bool) or (hooks and client not in HOOK_SETTINGS):
        raise ValueError("Hook qualification requires a supported native adapter")
    if hook_settings is not None and not hooks:
        raise ValueError("Native hook settings require explicit hook qualification")
    if not isinstance(model, str) or not model or any(char.isspace() for char in model):
        raise ValueError("An explicit provider model identifier is required")
    if not Path(binary).is_absolute():
        raise ValueError("Client executable must be absolute")
    provider = provider or ("anthropic" if client == "claude-code" else "openai")
    if (client == "codex" and provider != "openai") or (client == "claude-code" and provider != "anthropic"):
        raise ValueError("Client requires its native provider")
    name, key = _key(provider, os.environ if ambient is None else ambient)
    env = profile.environment(binary)
    env[name] = key
    if not hooks:
        env["LOCUST_HOOKS"] = "off"
    files = []
    hook_file = _hook_settings(client, profile, hook_settings) if hooks else None
    if hook_file:
        files.append(str(hook_file))
    provider_overrides = []
    custom_model_id = None
    base = PROVIDERS[provider][1]
    if client == "codex":
        path = _profile_path(profile, ".codex/config.toml")
        if path.exists():
            # CLI overrides preserve all existing TOML bytes, including hook
            # feature opt-outs, trust settings, comments and MCP registration.
            for key, value in {
                "model_provider": "locust_real_openai",
                "model_providers.locust_real_openai.name": "OpenAI direct qualification",
                "model_providers.locust_real_openai.base_url": "https://api.openai.com/v1",
                "model_providers.locust_real_openai.wire_api": "responses",
                "model_providers.locust_real_openai.env_key": "OPENAI_API_KEY",
                "model_providers.locust_real_openai.requires_openai_auth": False,
            }.items():
                provider_overrides += ["-c", key + "=" + json.dumps(value)]
        else:
            private_write(path, '\n'.join([
                'model = ' + json.dumps(model), 'model_provider = "locust_real_openai"',
                '[model_providers.locust_real_openai]', 'name = "OpenAI direct qualification"',
                'base_url = "https://api.openai.com/v1"', 'wire_api = "responses"',
                'env_key = "OPENAI_API_KEY"', 'requires_openai_auth = false', '']))
        files.append(str(path))
        env["CODEX_DISABLE_UPDATE_CHECK"] = "1"
    elif client == "claude-code":
        env.update({"DISABLE_TELEMETRY": "1", "DISABLE_ERROR_REPORTING": "1",
                    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1"})
    elif client == "factory-droid":
        path, custom_model_id = _droid_model_settings(profile, model, provider, name, base)
        files.append(str(path))
        env["FACTORY_DROID_AUTO_UPDATE_ENABLED"] = "false"
    else:
        # Native provider catalog is used; no auth.json is copied or created.
        env["PI_TELEMETRY"] = "0"
    node = shutil.which("node", path=env["PATH"])
    metadata = {"mode": "real-provider", "provider": provider, "model": model,
                "base_url": base, "authentication": "explicit ambient API key: " + name,
                "copied_account_authentication": False, "configuration_files": files,
                "client_environment_keys": sorted(env), "node": node,
                "policy_modes": list(POLICIES),
                "hooks_enabled": hooks, "hook_settings_file": str(hook_file) if hook_file else None,
                "hook_execution": "not_run; native delivery and trust require observation",
                "provider_overrides": provider_overrides,
                "network_guard": "external network allowed for real provider; fixture guard disabled"}
    if client == "factory-droid":
        metadata["custom_model_id"] = custom_model_id
        metadata["factory_authentication"] = "FACTORY_API_KEY excluded; BYOK-only readiness must be observed"
    if client == "claude-code":
        metadata["bare_mode"] = ("Disabled for explicit private hook settings; explicit API key, no copied account authentication"
                                 if hooks else "Explicit API key; no keychain/OAuth discovery; private HOME explicitly added for skills")
    if client == "pi":
        # Pi migrates agent-root *.jsonl files on startup. A nested native path
        # survives the first explicit resume instead of creating a new session.
        metadata["session_file"] = str(profile.home / ".pi/agent/sessions/qualification-session.jsonl")
        metadata["policy_limitation"] = "Both modes use Pi default; no permission extension installed"
        if not node:
            raise RuntimeError("Node is not resolvable in the isolated PATH")
    return RealProvider(client, str(binary), profile, model, provider, env, metadata)


def install_locust_skill(client, profile, source):
    """Install only the packaged manifest into the private native discovery path."""
    if client not in SKILL_PATHS:
        raise ValueError("Unsupported qualification client")
    source = Path(source)
    if source.is_dir():
        source = source / "SKILL.md"
    if source.is_symlink() or not source.is_file():
        raise ValueError("Locust skill source must be a regular manifest")
    content = source.read_bytes()
    if not content.startswith(b"---\n") or b"name: locust\n" not in content:
        raise ValueError("Locust skill manifest is invalid")
    destination = profile.home / SKILL_PATHS[client]
    private_write(destination, content)
    return {"source": str(source.resolve()), "path": str(destination),
            "sha256": hashlib.sha256(content).hexdigest(), "installation": "private-profile-only",
            "discovery": "not_run; require client skill invocation/read evidence"}
