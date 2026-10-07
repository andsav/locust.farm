#!/usr/bin/env python3
"""Qualify installed hook commands in isolated profiles, with explicit proof levels.

The default run uses a real local daemon and a disposable test-signed install,
but drives native payloads itself. Native --version is discovery only. Optional
real-model runs use exactly one API key from the environment. They never copy
login files. Reviewed temporary hooks use documented one-off native approval,
and read-tool approvals are scoped; reports record each override. Personal
native trust is never read or modified. All process HOME values come from mktemp
-d /tmp/lh.XXXXXX. Output retains projections, never transcripts or credentials.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import signal
import subprocess
import sys
import tarfile
import io
import math
import time

ROOT = Path(__file__).resolve().parents[1]
CLIENTS = {"codex": {"binary": "codex", "mcp": ".codex/config.toml", "hooks": ".codex/hooks.json", "provider": "openai", "key": "OPENAI_API_KEY", "real_client": "codex", "mcp_result":"call_tool_result"},
           "claude": {"binary": "claude", "mcp": ".claude.json", "hooks": ".claude/settings.json", "provider": "anthropic", "key": "ANTHROPIC_API_KEY", "real_client": "claude-code", "mcp_result":"json_text"},
           "droid": {"binary": "droid", "mcp": ".factory/mcp.json", "hooks": ".factory/hooks.json", "provider": "openai", "key": "OPENAI_API_KEY", "real_client": "factory-droid", "mcp_result":"json_text", "tool_prefix":"locust___locust_"}}
SAFE_PATH = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

# Qualification instrumentation only: no payload text or values are persisted.
# The setup-generated executable receives identical stdin and its stdout reaches
# the harness unchanged. The caller restores owned configuration afterward.
SHAPE_WRAPPER = r'''import json,os,re,subprocess,sys
data=sys.stdin.buffer.read()
payload=json.loads(data)
response=payload.get('tool_response')
def shape(value):
 result={'type':type(value).__name__}
 if isinstance(value,dict): result['keys']=sorted(value)
 if isinstance(value,list): result['items']=[{'type':type(item).__name__,'keys':sorted(item) if isinstance(item,dict) else []} for item in value]
 return result
event=payload.get('hook_event_name')
name=payload.get('tool_name','')
row={'event':event if event in {'SessionStart','Stop','PostToolUse','PostToolUseFailure'} else 'other',
 'payload_keys':sorted(payload),'session_id_present':bool(payload.get('session_id')),
 'agent_id_present':bool(payload.get('agent_id')),
 'agent_id_matches_session':bool(payload.get('agent_id')) and payload.get('agent_id')==payload.get('session_id'),
 'tool_name':name if isinstance(name,str) and re.fullmatch(r'(?:mcp__locust__|locust___)locust_[a-z_]+',name) else 'other',
 'response':shape(response),'input':shape(payload.get('tool_input')),
 'hooks_off':os.environ.get('LOCUST_HOOKS')=='off'}
if isinstance(response,dict):
 row['isError']=response.get('isError') if type(response.get('isError')) is bool else None
 row['structuredContent']=shape(response.get('structuredContent'))
 row['content']=shape(response.get('content'))
 row['ok']=response.get('ok') if type(response.get('ok')) is bool else None
if isinstance(response,str):
 try: parsed_response=json.loads(response)
 except ValueError: parsed_response=None
 row['json_string_shape']=shape(parsed_response)
 if isinstance(parsed_response,dict):
  row['json_string_ok']=parsed_response.get('ok') if type(parsed_response.get('ok')) is bool else None
  row['json_string_result']=shape(parsed_response.get('result'))
content=response.get('content') if isinstance(response,dict) else response
if isinstance(content,list):
 envelopes=[]
 for item in content:
  if isinstance(item,dict) and item.get('type')=='text' and isinstance(item.get('text'),str):
   try: parsed=json.loads(item['text'])
   except ValueError: continue
   if isinstance(parsed,dict): envelopes.append({'keys':sorted(parsed),'ok':parsed.get('ok') if type(parsed.get('ok')) is bool else None,'result':shape(parsed.get('result'))})
 row['text_envelope_shapes']=envelopes
completed=subprocess.run(sys.argv[2:],input=data,capture_output=True)
row['exit_code']=completed.returncode
row['stderr_bytes']=len(completed.stderr)
row['empty_stdout']=not completed.stdout.strip()
try: output=json.loads(completed.stdout)
except ValueError: output=None
row['block']=isinstance(output,dict) and output.get('decision')=='block' and str(output.get('reason','')).startswith('Locust:')
row['failure_line']=isinstance(output,dict) and 'Locust context was NOT injected' in json.dumps(output)
with open(sys.argv[1],'a') as log: log.write(json.dumps(row,sort_keys=True)+'\n')
sys.stdout.buffer.write(completed.stdout)
sys.stderr.buffer.write(completed.stderr)
sys.exit(completed.returncode)
'''


class CheckError(RuntimeError):
    pass


def require(condition, message):
    if not condition:
        raise CheckError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def private_write(path, data, mode=0o600):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(data.encode() if isinstance(data, str) else data)
    path.chmod(mode)


class Profile:
    def __init__(self):
        name = subprocess.check_output(["/usr/bin/mktemp", "-d", "/tmp/lh.XXXXXX"],
                                       env={"PATH": "/usr/bin:/bin"}, text=True).strip()
        self.root = self.home = Path(name)
        require(re.fullmatch(r"/tmp/lh\.[A-Za-z0-9]{6}", name), "mktemp returned an unexpected profile")
        for name in ("workspace", "logs", "tmp", "fixture"):
            setattr(self, name, self.root / name)
            getattr(self, name).mkdir(mode=0o700)

    def environment(self, binary):
        return {"HOME": str(self.home), "CODEX_HOME": str(self.home / ".codex"),
                "CLAUDE_CONFIG_DIR": str(self.home / ".claude"),
                "XDG_CONFIG_HOME": str(self.home / "config"), "XDG_CACHE_HOME": str(self.home / "cache"),
                "XDG_DATA_HOME": str(self.home / "data"), "TMPDIR": str(self.tmp),
                "PATH": str(Path(binary).parent) + ":" + SAFE_PATH,
                "LANG": "en_US.UTF-8", "TERM": "dumb", "NO_COLOR": "1",
                "FACTORY_DROID_AUTO_UPDATE_ENABLED": "false",
                "LOCUST_RELAY": "none", "LOCUST_LOOKUP": "none", "LOCUST_BIND": "127.0.0.1:0"}

    def close(self):
        shutil.rmtree(self.root)


def run(profile, argv, timeout, *, input_value=None, env=None):
    process = subprocess.Popen(list(map(str, argv)), cwd=profile.workspace,
                               env=profile.environment(argv[0]) if env is None else env,
                               stdin=subprocess.PIPE if input_value is not None else subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    try:
        output, errors = process.communicate(None if input_value is None else json.dumps(input_value).encode(), timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        raise CheckError("qualification command timed out") from None
    return process.returncode, output, errors


def cli(profile, binary, args, timeout, *, home=None, owner=False, credential=None, session=None):
    argv = [binary, "--json"]
    if home is not None:
        argv += ["--home", home]
    if owner:
        argv.append("--owner")
    elif credential is not None:
        argv += ["--credential", credential, "--session", session]
    code, output, errors = run(profile, [*argv, *args], timeout)
    try:
        envelope = json.loads(output)
    except (ValueError, UnicodeError):
        raise CheckError("Locust CLI returned invalid JSON") from None
    accepted_codes = (0, 20, 21) if list(args[:2]) == ["call", "wait"] else (0,)
    require(code in accepted_codes and not errors and isinstance(envelope, dict) and envelope.get("ok") is True,
            "Locust CLI operation failed: " + str(envelope.get("error", {}).get("code", "invalid_response")))
    result = envelope["result"]
    if owner:
        from owner_plans import confirmation_arguments
        confirmed = confirmation_arguments(args, result)
        if confirmed is not None:
            return cli(profile, binary, confirmed, timeout, home=home, owner=owner)
    return result


def install_fixture(profile, binary, source_commit, timeout):
    code, version, errors = run(profile, [binary, "--version"], timeout)
    match = re.fullmatch(rb"locust ([^ ]+) \(([a-f0-9]{12})\) api (\d+) protocol (\d+)\n", version)
    require(code == 0 and not errors and match is not None, "qualification binary needs a 12-hex build identity without -dirty")
    require(re.fullmatch(r"[a-f0-9]{40}|[a-f0-9]{64}", source_commit) and source_commit[:12] == match[2].decode(),
            "source commit does not match binary version")
    bundle = profile.root / "bundle"
    private_write(bundle / "locust", binary.read_bytes(), 0o755)
    private_write(bundle / "skills/locust/SKILL.md", (ROOT / "skills/locust/SKILL.md").read_bytes(), 0o644)
    # A synthetic manual makes this a qualification package, never a release.
    archive = io.BytesIO()
    with tarfile.open(fileobj=archive, mode="w") as tar:
        data = b"Synthetic hook qualification manual; not a published release.\n"
        item = tarfile.TarInfo("README.md")
        item.size = len(data)
        tar.addfile(item, io.BytesIO(data))
    private_write(bundle / "manual.tar", archive.getvalue(), 0o644)
    target, machine = (("aarch64-apple-darwin", "mach-o-arm64") if platform.system() == "Darwin" and platform.machine() == "arm64"
                       else ("x86_64-unknown-linux-gnu", "elf-x86_64"))
    files = [{"path": relative, "size": (bundle / relative).stat().st_size,
              "sha256": digest((bundle / relative).read_bytes()), "mode": mode}
             for relative, mode in (("locust", 0o755), ("skills/locust/SKILL.md", 0o644), ("manual.tar", 0o644))]
    manifest = {"format": "locust-release-v2", "source_commit": source_commit, "version": match[1].decode(),
                "target": target, "machine_format": machine, "api_version": int(match[3]),
                "protocol_version": int(match[4]), "toolchain": "1.96.1", "files": files}
    private_write(bundle / "manifest.json", json.dumps(manifest))
    secret, public = profile.fixture / "signing.secret", profile.fixture / "signing.public"
    cli(profile, binary, ["package", "keygen", "--secret-key", secret, "--public-key", public], timeout)
    cli(profile, binary, ["package", "sign", "--bundle", bundle, "--secret-key", secret], timeout)
    registry = profile.fixture / "withdrawals.json"
    private_write(registry, json.dumps({"format": "locust-withdrawals-v1", "sequence": 1, "withdrawn_manifest_sha256": []}))
    cli(profile, binary, ["package", "sign-withdrawals", "--registry", registry, "--secret-key", secret], timeout)
    prefix = profile.root / "software"
    inputs = ["--prefix", prefix, "--bundle", bundle, "--trust-key", public, "--withdrawals", registry]
    plan = cli(profile, binary, ["install", "plan", *inputs], timeout)
    cli(profile, binary, ["install", "apply", *inputs, "--expect-plan", plan["plan_sha256"]], timeout)
    installed = prefix / "current/locust"
    require(digest(installed.read_bytes()) == digest(binary.read_bytes()), "installed binary differs")
    return prefix, installed


def hook_events(document, client):
    return document if client == "droid" else document.get("hooks", {})


def selected_hook(document, launcher, event, harness):
    matches = []
    for native_event, groups in hook_events(document, harness).items():
        for group in groups:
            for handler in group.get("hooks", []):
                if handler.get("statusMessage") != "Locust":
                    continue
                argv = shlex.split(handler.get("command", ""))
                if argv == [str(launcher), "hook", event, "--harness", harness]:
                    matches.append((native_event, argv))
    require(matches, "setup did not install the selected generic hook")
    return matches[0]


def invoke_hook(profile, command, native_event, timeout, *, client=None, chat="scripted-root", call=None, compacted=False, continued=False):
    payload = {"session_id": chat, "hook_event_name": native_event,
               "source": "compact" if compacted else "startup", "stop_hook_active": continued,
               "cwd": str(profile.workspace)}
    if call is not None:
        operation, arguments, result, identifier = call
        envelope = {"ok": True, "result": result}
        prefix = CLIENTS.get(client, {}).get("tool_prefix", "mcp__locust__locust_")
        payload.update(tool_name=prefix + operation.replace(".", "_"),
                       tool_use_id=identifier, tool_input=arguments,
                       tool_response={"isError": False, "structuredContent": envelope,
                                      "content": [{"type": "text", "text": json.dumps(envelope)}]})
        if client is not None and CLIENTS[client]["mcp_result"]=="json_text":
            payload["tool_response"]=json.dumps(envelope)
        if client == "droid":
            payload.pop("tool_use_id")
    code, output, errors = run(profile, command, timeout, input_value=payload)
    require(code == 0 and not errors, "hook did not fail open with a clean transport")
    if not output.strip():
        return None
    try:
        return json.loads(output)
    except ValueError:
        raise CheckError("hook emitted a malformed envelope") from None


def blocks_once(first, second):
    return (isinstance(first, dict) and first.get("decision") == "block"
            and isinstance(first.get("reason"), str) and first["reason"].startswith("Locust:")
            and second is None)


def native_version(profile, client, timeout, explicit=None):
    binary = explicit or shutil.which(CLIENTS[client]["binary"])
    if binary is None:
        return None, {"status": "missing", "reason": "native executable missing"}
    binary = str(Path(binary).absolute())
    code, output, errors = run(profile, [binary, "--version"], timeout)
    version = output.decode(errors="replace").strip()
    safe_version = version if re.fullmatch(r"[A-Za-z0-9 ._()\-]+", version) else None
    return binary, {"status": "pass" if code == 0 and safe_version else "fail",
                    "version": safe_version, "exit_code": code, "stderr_sha256": digest(errors),
                    "scope": "native version probe only; hooks not qualified"}


def mark_projection(home):
    return {str(path.relative_to(home)): {"used_locust": value.get("used_locust") is True,
            "worker": value.get("worker") is True, "shown_count": len(value.get("shown", []))}
            for path in (home / "hook-marks").glob("*/*/*.json")
            for value in [json.loads(path.read_text())]}


def native_hook_projection(stdout, expected_command):
    """Only native hook lifecycle records count; assistant JSON never does."""
    callbacks = []
    shapes = set()
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict) or event.get("type") != "system" or event.get("subtype") not in {"hook_response", "hook_started", "hook_progress"}:
            continue
        shapes.add(tuple(sorted(event)))
        if event.get("subtype") != "hook_response":
            continue
        output = event.get("stdout")
        parsed = None
        if isinstance(output, str) and output.strip():
            try:
                parsed = json.loads(output)
            except ValueError:
                pass
        native_event = event.get("hook_event") or event.get("hook_event_name")
        command = event.get("command")
        name = event.get("hook_name", "")
        callbacks.append({"event": native_event if native_event in {"SessionStart", "Stop", "PostToolUse"} else None,
                          "own_command": command == expected_command or (isinstance(name, str) and expected_command in name),
                          "block": isinstance(parsed, dict) and parsed.get("decision") == "block" and str(parsed.get("reason", "")).startswith("Locust:"),
                          "empty_stdout": isinstance(output, str) and not output.strip(),
                          "exit_code": event.get("exit_code") if type(event.get("exit_code")) is int else None})
    stops = [callback for callback in callbacks if callback["event"] == "Stop" and callback["own_command"] and callback["exit_code"] == 0]
    interpreted = any(first["block"] and any(second["empty_stdout"] for second in stops[index+1:])
                      for index, first in enumerate(stops))
    return {"hook_callback_shapes": [list(shape) for shape in sorted(shapes)],
            "hook_callbacks": callbacks, "block_then_empty_stop_callback": interpreted}


def native_tool_projection(stdout):
    declarations, states, calls, successful = set(), [], {}, 0
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict):
            continue
        if event.get("type") == "system" and event.get("subtype") == "init":
            declarations.update(name for name in event.get("tools", []) if isinstance(name,str) and re.fullmatch(r"(?:mcp__locust__|locust___)locust_[a-z_]+",name))
            states.extend({"name":"locust","status":row.get("status") if row.get("status") in {"connected","failed","pending","needs-auth"} else "other"}
                          for row in event.get("mcp_servers",[]) if isinstance(row,dict) and row.get("name")=="locust")
        if event.get("type") == "assistant":
            for item in event.get("message",{}).get("content",[]):
                if item.get("type")=="tool_use" and item.get("name")=="mcp__locust__locust_wait":
                    calls[item.get("id")] = "wait"
        if event.get("type") == "user":
            for item in event.get("message",{}).get("content",[]):
                if item.get("type")=="tool_result" and item.get("tool_use_id") in calls and item.get("is_error") is not True:
                    successful += 1
        if event.get("type") == "tool_call" and event.get("toolName") == "locust___locust_wait":
            calls[event.get("id")] = "wait"
        if event.get("type") == "tool_result" and event.get("id") in calls and event.get("isError") is False:
            try:
                result = json.loads(event.get("value", ""))
            except (ValueError, TypeError):
                result = None
            body = result.get("result") if isinstance(result, dict) else None
            successful += int(isinstance(body, dict) and result.get("ok") is True and "waited" in body)
        if event.get("type")=="item.completed":
            item=event.get("item",{})
            if item.get("type")=="mcp_tool_call" and item.get("server")=="locust" and item.get("tool")=="locust_wait":
                calls[item.get("id")]="wait"
                successful += int(item.get("status")=="completed" and item.get("error") is None and item.get("result",{}).get("isError") is not True)
    return {"locust_tool_declarations":sorted(declarations),"locust_server_states":states,
            "native_locust_wait_calls":len(calls),"native_locust_wait_completed":successful}


def real_check(profile, client, binary, home, goal, revision, timeout, model, ambient):
    spec = CLIENTS[client]
    if not ambient.get(spec["key"]):
        return {"status": "not_run", "reason": "no API key in environment"}
    if binary is None:
        return {"status": "not_run", "reason": "native executable missing"}
    if model is None:
        return {"status": "not_run", "reason": "select a small model from provider metadata with --model CLIENT=MODEL"}
    from client_qualification.real_models import configure_real_provider, provider_model_ids, RedactingProcess
    available = provider_model_ids(spec["provider"], timeout, ambient)
    require(model in available, "selected model not present in current provider metadata")
    provider = configure_real_provider(spec["real_client"], profile, binary, model, ambient, hooks=True)
    hook_path = profile.home / spec["hooks"]
    original_hooks = hook_path.read_bytes()
    wrapped_hooks = json.loads(original_hooks)
    wrapper, shapes = profile.fixture / "hook-shapes.py", profile.fixture / "hook-shapes.jsonl"
    private_write(wrapper, SHAPE_WRAPPER)
    private_write(shapes, b"")
    for groups in hook_events(wrapped_hooks, client).values():
        for group in groups:
            for handler in group.get("hooks", []):
                if handler.get("statusMessage") == "Locust":
                    handler["command"] = shlex.join([sys.executable, str(wrapper), str(shapes), *shlex.split(handler["command"])])
    if client == "droid":
        guard = profile.fixture / "read-only-guard.py"
        private_write(guard, "import json,sys\npayload=json.load(sys.stdin)\n"
                      "if payload.get('tool_name') not in {'ToolSearch','locust___locust_wait'}:\n"
                      " print(json.dumps({'hookSpecificOutput':{'hookEventName':'PreToolUse','permissionDecision':'deny','permissionDecisionReason':'Synthetic qualification permits only the wait read and tool discovery'}}))\n")
        hook_events(wrapped_hooks, client).setdefault("PreToolUse", []).append({"hooks":[{
            "type":"command", "command":shlex.join([sys.executable, str(guard)]), "timeout":60}]})
    private_write(hook_path, json.dumps(wrapped_hooks))
    prompt = (f"Use the registered Locust MCP tool locust_wait exactly once with goal={goal}, seen={revision}, timeout_ms=0. "
              "Then finish your response. This synthetic test grants reads only: do not start, take over, report, publish, acknowledge, or change any work. "
              "If a hook continues your response, make no further Locust writes and finish your response again. Do not spawn agents.")
    before = mark_projection(home)
    overlay = []
    trust_bypass = client == "codex"
    if client == "claude":
        # Explicitly reuse only setup's private scoped MCP entry. Source filters
        # in the native real-provider helper intentionally exclude normal files.
        configured = json.loads((profile.home / spec["mcp"]).read_text())
        servers = configured.get("mcpServers", {})
        for project in configured.get("projects", {}).values():
            if isinstance(project,dict) and "locust" in project.get("mcpServers",{}):
                servers = project["mcpServers"]
        require("locust" in servers,"private setup MCP entry missing")
        overlay = ["--mcp-config="+json.dumps({"mcpServers":{"locust":servers["locust"]}}),"--strict-mcp-config",
                   "--allowedTools=mcp__locust__locust_wait,ToolSearch"]
    elif client == "droid":
        overlay = ["--auto", "high", "--only-tools", "ToolSearch,MCP:locust/locust_wait"]
    invocation = provider.invocation(prompt, overlay)
    if trust_bypass:
        # Official one-off option, authorized for reviewed synthetic /tmp hooks.
        # Product setup still requires native trust review; no trust is copied.
        invocation.insert(1,"--dangerously-bypass-hook-trust")
    native = RedactingProcess(invocation, provider.environment, profile.workspace,
                             profile.logs, "real-model", timeout, [spec["key"]])
    try:
        result = native.wait()
    finally:
        private_write(hook_path, original_hooks)
    document = json.loads(original_hooks)
    own_commands = [handler["command"] for group in hook_events(document, client)["Stop"] for handler in group["hooks"] if handler.get("statusMessage") == "Locust"]
    projection = native_hook_projection(Path(result["stdout"]).read_bytes(), own_commands[0])
    tools = native_tool_projection(Path(result["stdout"]).read_bytes())
    after = mark_projection(home)
    new = [value for key, value in after.items() if key not in before]
    inputs = [json.loads(line) for line in shapes.read_text().splitlines()]
    own_stops = [row for row in inputs if row["event"] == "Stop" and row["exit_code"] == 0]
    wrapper_proof = any(first["block"] and any(second["empty_stdout"] for second in own_stops[index+1:]) for index, first in enumerate(own_stops))
    passed = (result["exit_code"] == 0 and not result["timed_out"] and wrapper_proof
              and tools["native_locust_wait_completed"] > 0
              and any(value["used_locust"] and value["worker"] and value["shown_count"] for value in new))
    return {"status": "pass" if passed else "unverified",
            "provider": spec["provider"], "model": model, "exit_code": result["exit_code"],
            "timed_out": result["timed_out"], "native_hook_marks_created": len(new),
            "native_worker_observed": any(value["worker"] for value in new),
            "native_stop_decision_observed": any(value["shown_count"] for value in new),
            "native_continuation_and_stop_interpretation": "observed" if result["exit_code"] == 0 and (projection["block_then_empty_stop_callback"] or wrapper_proof) else "unverified; marks alone do not prove native output interpretation",
            "qualification_wrapper_block_then_pass": wrapper_proof,
            "native_payload_shapes": inputs,
            "qualification_wrapper_scope": "shape-only observation; original installed hook executable and stdin/stdout preserved; original hook config restored before remove",
            **projection,
            **tools,
            "stdout_sha256": digest(Path(result["stdout"]).read_bytes()), "stderr_sha256": digest(Path(result["stderr"]).read_bytes()),
            "copied_login": False, "native_trust_bypassed": trust_bypass,
            "native_registration_overlay": client == "claude",
            "native_tool_read_approvals": ["mcp__locust__locust_wait","ToolSearch"] if client=="claude" else ["locust___locust_wait","ToolSearch"] if client=="droid" else [],
            "qualification_overrides": ("reviewed own /tmp hooks one-off trust bypass" if trust_bypass else
                "native high autonomy with exact wait/tool-search selection and a private read-only guard" if client=="droid" else
                "exact setup-owned private MCP entry explicit overlay; default native settings filtered; only wait and tool-search reads approved")}


def qualify(client, binary, source_commit, timeout, *, native_binary=None, real_model=False, model=None, ambient=None):
    profile = Profile()
    process = None
    result = {"client": client, "scripted": {"status": "not_run"},
              "real_model": {"status": "not_run", "reason": "real model not selected"},
              "native_hooks": {"status": "unverified", "reason": "scripted native payloads are not native harness delivery"}}
    phase = "install"
    try:
        prefix, installed = install_fixture(profile, binary, source_commit, timeout)
        home = profile.root / "daemon"
        home.mkdir(mode=0o700)
        phase = "daemon"
        process = subprocess.Popen([str(installed), "--home", str(home), "daemon", "run"],
                                   cwd=profile.workspace, env=profile.environment(installed),
                                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                   start_new_session=True)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            require(process.poll() is None, "private daemon exited during startup")
            if (home / "daemon.sock").exists():
                break
            time.sleep(0.02)
        require((home / "daemon.sock").exists(), "private daemon startup timed out")
        owner = lambda args: cli(profile, installed, args, timeout, home=home, owner=True)
        principal = owner(["agent", "enroll", "qualification"])["agent_enrolled"]["agent"]
        credential = home / "agents/qualification.credential"
        session = home / "sessions/qualification.secret"
        owner(["session", "create", session])
        agent = lambda args: cli(profile, installed, args, timeout, home=home, credential=credential, session=session)
        call = lambda operation, fields, own=False: (owner if own else agent)(["call", operation, json.dumps(fields)])
        formation = agent(["formation", "example", "directed"])
        goal = call("goal.create", {"agent": principal, "name": "hooks", "title": "Synthetic hook qualification", "formation_json": json.dumps(formation), "inputs": {}}, True)["goal_created"]["goal"]
        call("level.set", {"goal": goal, "agent": principal, "level": "auto"}, True)
        task = "task:" + call("task.open", {"goal": goal, "text": "Synthetic hook test work", "task_type": None, "inputs": {}, "parent": None})["recorded"]["event"]
        offer = call("work.offer", {"goal": goal, "task": task, "recipient": principal})["recorded"]["event"]
        phase = "setup"
        spec = CLIENTS[client]
        baseline_hooks = b'{"description":"Preserved fixture hook config","hooks":{"Stop":[{"hooks":[{"type":"command","command":"true"}]}]}}\n'
        if client == "droid":
            baseline_hooks = b'{"Stop":[{"hooks":[{"type":"command","command":"true"}]}]}\n'
        baseline_mcp = b"# preserve comment\nmodel = 'chosen'\n" if client == "codex" else b'{"unrelated":"preserved"}\n'
        hook_path, mcp_path = profile.home / spec["hooks"], profile.home / spec["mcp"]
        private_write(hook_path, baseline_hooks)
        private_write(mcp_path, baseline_mcp)
        setup_args = ["--prefix", prefix, "--client", client, "--profile-home", profile.home,
                      "--workspace", profile.workspace, "--daemon-home", home,
                      "--credential-file", credential, "--session-file", session]
        plan = cli(profile, installed, ["setup", "plan", *setup_args], timeout)
        applied = cli(profile, installed, ["setup", "apply", *setup_args, "--expect-plan", plan["plan_sha256"]], timeout)
        launcher = Path(applied["launcher"])
        require(launcher.is_relative_to(profile.home.resolve()), "launcher escaped private profile")
        document = json.loads(hook_path.read_text())
        hooks = {event: selected_hook(document, launcher, event, client) for event in ("start", "stop", "tool")}
        phase = "scripted_hook"
        pending = call("pending", {"goal": goal})["pending"]
        require(any(item["task"] == task and item["unattended"] for item in pending["to_start"]), "fixture task is not free authoritative work")
        wait_args = {"goal": goal, "seen": pending["revision"], "timeout_ms": 0}
        wait_result = call("wait", wait_args)
        invoke_hook(profile, hooks["tool"][1], hooks["tool"][0], timeout, client=client,call=("wait", wait_args, wait_result, "scripted-wait"))
        first = invoke_hook(profile, hooks["stop"][1], hooks["stop"][0], timeout)
        second = invoke_hook(profile, hooks["stop"][1], hooks["stop"][0], timeout, continued=True)
        require(blocks_once(first, second), "waiting work did not block exactly once then pass")
        claim_args = {"goal": goal, "task": task, "offer": offer}
        claimed = call("attempt.start", claim_args)
        invoke_hook(profile, hooks["tool"][1], hooks["tool"][0], timeout, client=client,call=("attempt.start", claim_args, claimed, "scripted-claim"))
        cancellation = call("attempt.cancel", {"goal": goal, "attempt": claimed["claimed"]["attempt"]})
        cancelled = invoke_hook(profile, hooks["stop"][1], hooks["stop"][0], timeout)
        require(isinstance(cancelled, dict) and cancelled.get("decision") == "block", "new cancellation did not reach stop callback")
        compacted = invoke_hook(profile, hooks["start"][1], hooks["start"][0], timeout, compacted=True)
        require(claimed["claimed"]["attempt"] in json.dumps(compacted), "compaction did not restore held attempt facts")
        observed = call("pending", {"goal": goal})["pending"]
        notice_args = {"goal": goal, "seen": observed["revision"], "timeout_ms": 0}
        notice_result = call("wait", notice_args)
        notice_call = ("wait", notice_args, notice_result, "scripted-cancel-notice")
        notice = invoke_hook(profile, hooks["tool"][1], hooks["tool"][0], timeout,
                             client=client, call=notice_call)
        require(isinstance(notice, dict) and "cancellation requested" in json.dumps(notice)
                and cancellation["recorded"]["event"] in json.dumps(notice),
                "tool callback did not name the cancellation already observed by stop")
        repeated = invoke_hook(profile, hooks["tool"][1], hooks["tool"][0], timeout,
                               client=client, call=notice_call)
        require(repeated is None, "repeated tool callback delivered the same notice twice")
        result["scripted"] = {"status": "pass", "waiting_task_blocks_once_then_passes": True,
                              "compaction_restores_held_attempt": True, "generated_setup_hook_command_executed": True,
                              "cancellation_reaches_stop_callback": True,
                              "cancellation_reaches_tool_callback": True,
                              "duplicate_tool_notice_suppressed": True,
                              "evidence_level": "scripted native payloads; real daemon and installed command"}
        # End the synthetic claim so a real model sees the same free task.
        ack_args = {"goal": goal, "cancel": cancellation["recorded"]["event"],
                    "generation": claimed["claimed"]["generation"], "outcome": "stopped"}
        acknowledged = call("cancel.acknowledge", ack_args)
        after_ack = invoke_hook(profile, hooks["tool"][1], hooks["tool"][0], timeout,
                                client=client, call=("cancel.acknowledge", ack_args, acknowledged, "scripted-ack"))
        require(after_ack is None, "own terminal acknowledgment produced a false claim-loss notice")
        result["scripted"]["own_terminal_ack_explains_claim_loss"] = True
        phase = "setup_remove"
        remove_plan = cli(profile, installed, ["setup", "remove-plan", *setup_args], timeout)
        cli(profile, installed, ["setup", "remove", *setup_args, "--expect-plan", remove_plan["plan_sha256"]], timeout)
        require(hook_path.read_bytes() == baseline_hooks and mcp_path.read_bytes() == baseline_mcp, "setup remove did not restore exact original bytes")
        result["setup_remove"] = {"status": "pass", "hook_bytes_restored": True, "mcp_bytes_restored": True}
        phase = "native_version"
        native_binary, result["native_version"] = native_version(profile, client, timeout, native_binary)
        if real_model:
            phase = "real_model"
            real_task = "task:" + call("task.open", {"goal": goal, "text": "Synthetic native stop qualification", "task_type": None, "inputs": {}, "parent": None})["recorded"]["event"]
            call("work.offer", {"goal": goal, "task": real_task, "recipient": principal})
            plan = cli(profile, installed, ["setup", "plan", *setup_args], timeout)
            cli(profile, installed, ["setup", "apply", *setup_args, "--expect-plan", plan["plan_sha256"]], timeout)
            current = call("pending", {"goal": goal})["pending"]
            require(any(item["task"]==real_task and item["unattended"] for item in current["to_start"]),"native scenario needs authoritative free work")
            result["real_model"] = real_check(profile, client, native_binary, home, goal, current["revision"], timeout, model, os.environ if ambient is None else ambient)
            result["native_hooks"] = {"status":result["real_model"]["status"],
                                      "evidence_level":"native real-provider run in synthetic profile with explicit qualification overrides",
                                      "proof":"successful registered wait; native hook block then empty stop callback; natural client exit" if result["real_model"]["status"]=="pass" else "native continuation remains unverified"}
            remove_plan = cli(profile, installed, ["setup", "remove-plan", *setup_args], timeout)
            cli(profile, installed, ["setup", "remove", *setup_args, "--expect-plan", remove_plan["plan_sha256"]], timeout)
            result["real_model"]["setup_removed_after_native_run"] = True
        result["status"] = "pass"
    except (CheckError, OSError, ValueError, RuntimeError) as error:
        result["status"] = "fail"
        result["failure"] = {"phase": phase, "reason": str(error) if isinstance(error, CheckError) else type(error).__name__}
    finally:
        if process is not None and process.poll() is None:
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        profile.close()
        result["private_home_removed"] = not profile.root.exists()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--client", action="append", choices=CLIENTS)
    parser.add_argument("--client-binary", action="append", default=[], metavar="CLIENT=PATH")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--timeout", type=float, default=60)
    parser.add_argument("--real-model", action="store_true")
    parser.add_argument("--model", action="append", default=[], metavar="CLIENT=MODEL")
    args = parser.parse_args()
    require(math.isfinite(args.timeout) and args.timeout > 0, "timeout must be positive and finite")
    binaries = dict(item.split("=", 1) for item in args.client_binary)
    models = dict(item.split("=", 1) for item in args.model)
    report = {"format": "locust-hook-qualification-v1", "time": datetime.now(timezone.utc).isoformat(),
              "production_release_trust": False, "source_build_claim": "supplied candidate with explicit embedded build identity; no clean-commit/release claim",
              "clients": [qualify(client, args.binary.absolute(), args.source_commit, args.timeout,
                                   native_binary=binaries.get(client), real_model=args.real_model, model=models.get(client))
                          for client in args.client or CLIENTS]}
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        private_write(args.output.absolute(), rendered)
    print(rendered, end="")
    return 0 if all(client["status"] == "pass" for client in report["clients"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
