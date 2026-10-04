"""Isolated real-provider profiles. No account stores or normal profiles are read.

Callers must choose a model from provider metadata or a current primary catalog.
Configuration and skill installation are preparation, not model/tool evidence.
The environment contains secrets and must never be included in reports.
"""

from dataclasses import dataclass, field
import hashlib
import json
import os
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
            argv = [self.binary, *overlay]
            if permissive:
                argv += ["-a", "never", "-s", "danger-full-access"]
            argv += ["exec", "--skip-git-repo-check", "--json", "--model", self.model]
            if resume:
                argv += ["resume", resume]
            return [*argv, "--", prompt]
        if self.client == "claude-code":
            argv = [self.binary, "--bare", "-p", "--verbose", "--output-format", "stream-json",
                    "--model", self.model, "--add-dir", str(self.profile.home), *overlay]
            if permissive:
                argv += ["--dangerously-skip-permissions"]
            if resume:
                argv += ["--resume", resume]
            return [*argv, "--", prompt]
        if self.client == "factory-droid":
            argv = [self.binary, "exec", "--output-format", "stream-json",
                    "--model", "custom:Locust-Real-0", *overlay]
            if permissive:
                argv += ["--skip-permissions-unsafe"]
            if resume:
                argv += ["--session-id", resume]
            return [*argv, "--", prompt]
        argv = [self.binary, "--print", "--mode", "json", "--provider", self.provider,
                "--model", self.model, *overlay]
        session = resume or self.metadata["session_file"]
        return [*argv, "--session", session, "--", prompt]


def configure_real_provider(client, profile, binary, model, ambient=None, provider=None):
    """Use a clean Profile and exactly the selected provider's ambient API key.

    Apply config_probe's proposal independently with Profile.apply. Keys are
    referenced by name in config, not persisted. Factory account authentication
    remains unverified unless the caller observes a successful real request.
    """
    if client not in SKILL_PATHS:
        raise ValueError("Unsupported qualification client")
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
    files = []
    base = PROVIDERS[provider][1]
    if client == "codex":
        path = profile.home / ".codex/config.toml"
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
        path = profile.home / ".factory/settings.json"
        private_write(path, json.dumps({"customModels": [{"model": model,
            "displayName": "Locust Real", "baseUrl": base, "apiKey": "${" + name + "}",
            "provider": provider}]}))
        files.append(str(path))
    else:
        # Native provider catalog is used; no auth.json is copied or created.
        env["PI_TELEMETRY"] = "0"
    node = shutil.which("node", path=env["PATH"])
    metadata = {"mode": "real-provider", "provider": provider, "model": model,
                "base_url": base, "authentication": "explicit ambient API key: " + name,
                "copied_account_authentication": False, "configuration_files": files,
                "client_environment_keys": sorted(env), "node": node,
                "policy_modes": list(POLICIES),
                "network_guard": "external network allowed for real provider; fixture guard disabled"}
    if client == "factory-droid":
        metadata["factory_authentication"] = "FACTORY_API_KEY excluded; BYOK-only readiness must be observed"
    if client == "claude-code":
        metadata["bare_mode"] = "Explicit API key; no keychain/OAuth discovery; private HOME explicitly added for skills"
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
