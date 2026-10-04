#!/usr/bin/env python3
"""Qualify actual clients against the production daemon/MCP using scripted models.

The original fixture harness remains available as check_clients.py. This run
uses real scoped credentials, SQLite, typed claims and trusted workspace CLI
operations, but no real provider account/model, remote peer or installed release.
All source content is synthetic and all client profiles are temporary.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import shlex
import subprocess
import sys
import time

import check_clients as fixture
from client_qualification.production import ProductionDaemon, ProductionError
from client_qualification.provider import Provider
from client_qualification.runtime import Process, Profile, private_write, records


ROOT = Path(__file__).resolve().parents[1]
READ, WRITE, CLAIM, PROGRESS, WAIT = (
    "locust_goal_status", "locust_note_add", "locust_task_claim", "locust_task_progress", "locust_wait")
ASSERTIONS = (
    "configuration", "network_isolation", "initialize", "tools_list", "default_read", "default_write",
    "scoped_daemon_authentication", "claim", "progress", "workspace_native_tool", "contribution_flow",
    "accepted_before_integrated", "dirty_work_preserved", "held_wait", "interruption", "explicit_resume",
    "bridge_restart", "daemon_restart", "receipt_integrity", "client_execution", "default_interactive_approval",
    "real_model", "skill_discovery", "independent_accounts", "packaged_install")


def step(tool, arguments):
    return {"tool": tool, "arguments": arguments}


def envelope(event):
    value = event.get("result")
    if isinstance(value, list):
        value = value[0] if len(value) == 1 else None
    return value if isinstance(value, dict) else {}


def successful(events, tool):
    return [item for item in events if item.get("direction") == "bridge_response" and
            valid_response(item, "tools/call") and item.get("tool") == tool and
            item.get("isError") is False and envelope(item).get("ok") is True]


def record_result(event):
    return envelope(event).get("result", {})


def outstanding_wait(events):
    requests = [item for item in events if item.get("direction") == "client_request" and
                item.get("method") == "tools/call" and item.get("tool") == WAIT]
    responses = {json.dumps(item.get("id"), sort_keys=True) for item in events
                 if item.get("direction") == "bridge_response"}
    return any(json.dumps(item.get("id"), sort_keys=True) not in responses for item in requests)


def expected_tools(version, evidence=None):
    """Read registry from the binary's recorded Git revision, never moving HEAD."""
    match = re.search(r"\(([0-9a-f]{12,40})(-dirty)?\)", version)
    if match is None:
        raise ProductionError("Pinned production daemon revision is unavailable")
    source = subprocess.run(["git", "show", match[1] + ":crates/locust-proto/src/api.rs"],
                            cwd=ROOT, capture_output=True, text=True, check=True).stdout
    current = (ROOT / "crates/locust-proto/src/api.rs").read_bytes()
    base = source.encode()
    unchanged = current == base
    if match[2] and not unchanged:
        raise ProductionError("Dirty production revision has changed API registry source; independent build provenance required")
    if evidence is not None:
        evidence.update(revision=match[1], binary_reports_dirty=bool(match[2]),
                        registry_source_sha256=hashlib.sha256(base).hexdigest(),
                        working_registry_sha256=hashlib.sha256(current).hexdigest(),
                        working_registry_equals_revision=unchanged)
    names = re.findall(r'^\s*\w+(?:\s*\{[^}]*\})?\s*=>\s*\("([^"]+)"[^\n]*,\s*TOOL,', source, re.MULTILINE)
    if not names:
        raise ProductionError("Pinned production tool registry is unavailable")
    return {"locust_" + name.replace(".", "_") for name in names}


def valid_response(event, method):
    return (event.get("direction") == "bridge_response" and event.get("request_method") == method
            and event.get("jsonrpc") == "2.0" and event.get("has_result") is True
            and "error" not in event and isinstance(event.get("id"), (str, int))
            and not isinstance(event.get("id"), bool))


def receipt_integrity(events):
    """Validate retained protocol envelopes and correlation, allowing held calls."""
    pending = {}
    observed = False
    controls = {"bridge_started", "bridge_exited", "client_input_eof", "client_output_closed"}
    for event in events:
        if event.get("event") in controls:
            continue
        if event.get("event") != "jsonrpc" or event.get("jsonrpc") != "2.0":
            return False
        observed = True
        direction = event.get("direction")
        if direction not in {"client_request", "bridge_response"}:
            return False
        if "id" in event and (type(event["id"]) not in (str, int)):
            return False
        if "method" in event:
            method = event["method"]
            if not isinstance(method, str) or not method:
                return False
            if "id" not in event:
                if not method.startswith("notifications/"):
                    return False
                continue
            # Locust issues responses/notifications, not client RPC requests.
            if direction != "client_request":
                return False
            key = json.dumps(event["id"])
            if key in pending:
                return False
            pending[key] = event
        else:
            if direction != "bridge_response" or "id" not in event:
                return False
            request = pending.pop(json.dumps(event["id"]), None)
            if request is None or event.get("request_method") != request["method"]:
                return False
            if event.get("tool") != request.get("tool"):
                return False
            has_result = event.get("has_result")
            if type(has_result) is not bool or has_result == ("error" in event):
                return False
            if "error" in event:
                error = event["error"]
                if (not isinstance(error, dict) or type(error.get("code")) is not int
                        or not isinstance(error.get("message"), str)):
                    return False
    # An interrupted wait intentionally has no response. An empty stream
    # cannot establish protocol integrity.
    return observed


def handshake(events, expected):
    initialized = False
    listed = False
    for event in events:
        if valid_response(event, "initialize"):
            result = event.get("result")
            initialized |= (isinstance(result, dict) and result.get("protocolVersion") in
                            {"2025-03-26", "2025-06-18", "2025-11-25"}
                            and isinstance(result.get("serverInfo"), dict)
                            and result["serverInfo"].get("name") == "locust"
                            and isinstance(result["serverInfo"].get("version"), str)
                            and bool(result["serverInfo"].get("version"))
                            and isinstance(result.get("capabilities"), dict)
                            and isinstance(result["capabilities"].get("tools"), dict))
        if valid_response(event, "tools/list"):
            definitions = event.get("tool_definitions", [])
            names = [tool.get("name") for tool in definitions if isinstance(tool, dict)]
            canonical = json.dumps({"tools": definitions}, sort_keys=True, separators=(",", ":")).encode()
            listed |= (len(names) == len(definitions) == len(expected) and all(isinstance(name, str) for name in names) and set(names) == expected
                       and hashlib.sha256(canonical).hexdigest() == event.get("schema_sha256")
                       and all(isinstance(tool.get("inputSchema"), dict)
                               and tool["inputSchema"].get("type") == "object"
                               and isinstance(tool["inputSchema"].get("properties"), dict)
                               and tool["inputSchema"].get("additionalProperties") is False
                               for tool in definitions))
    return bool(initialized), bool(listed)


def expected_interrupt_exit(run, client=None, version=None):
    # SIGKILL/SIGSEGV and other failures never become expected SIGINT cleanup.
    # Codex 0.153.4 deliberately maps Interrupted to error_seen -> exit(1):
    # https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/exec/src/lib.rs
    # Keep this qualification version-specific, not a blanket nonzero allowance.
    return run.get("exit_code") == 0 or (run.get("interrupted") is True and
        (run.get("exit_code") in (-signal.SIGINT, 128 + signal.SIGINT) or
         (client == "codex" and version == "codex-cli 0.153.4" and run.get("exit_code") == 1)))


def expected_client_exit(run, client, version):
    if expected_interrupt_exit(run, client, version):
        return True
    # Measured Droid 0.218.1 default-headless behavior: this exact denied
    # scripted write terminates exec with 1. Do not generalize to other exits,
    # clients, versions, operations or permissive/lifecycle scenarios.
    return (client == "factory-droid" and version == "0.218.1"
            and run.get("scenario") == "default"
            and run.get("permission_profile") == "default_headless"
            and run.get("interrupted") is False and run.get("exit_code") == 1
            and run.get("observed_policy_denials") == [
                {"tool_name": "locust___locust_note_add", "reason": "higher autonomy required"}])


def exact_claim(events, observed, work, instance):
    for event in successful(events, CLAIM):
        claim = record_result(event).get("claimed", {})
        generation = claim.get("generation")
        if (claim.get("goal") == work["goal"] and claim.get("task") == work["task"]
                and claim.get("assignment") == work["assignment"] and claim.get("instance") == instance
                and type(generation) is int and generation > 0 and claim in observed):
            return True
    return False


def persisted_restart(daemon, work, progress_ids):
    before = daemon.call(["goal", "status", "--goal", daemon.goal])["goal_status"]
    task_before = daemon.call(["task", "show", "--goal", daemon.goal, "--task", work["task"]])["task"]
    claims_before = daemon.call(["pending", "--goal", daemon.goal])["pending"]["claimed"]
    progress_before = [daemon.call(["event", "show", "--goal", daemon.goal, "--event", event])["event"] for event in progress_ids]
    endpoint = daemon.endpoint
    daemon.restart()
    after = daemon.call(["goal", "status", "--goal", daemon.goal])["goal_status"]
    task_after = daemon.call(["task", "show", "--goal", daemon.goal, "--task", work["task"]])["task"]
    claims_after = daemon.call(["pending", "--goal", daemon.goal])["pending"]["claimed"]
    progress_after = [daemon.call(["event", "show", "--goal", daemon.goal, "--event", event])["event"] for event in progress_ids]
    return (endpoint == daemon.endpoint and before == after and before.get("goal") == daemon.goal
            and before.get("coordinator") == daemon.principal and task_before == task_after
            and claims_before == claims_after and progress_before == progress_after)


def denied(run, tool):
    return any(item.get("tool_name", "").endswith(tool) for item in fixture.permission_denials(run))


def git(profile, directory, args):
    return subprocess.run(["git", "-C", str(directory), *args], env=profile.environment("/usr/bin/git"),
                          capture_output=True, text=True, check=True).stdout.strip()


def prepare_work(profile, daemon, client):
    source = profile.workspace / "source"
    source.mkdir(mode=0o700)
    git(profile, source, ["init", "-q"])
    git(profile, source, ["config", "user.name", "Locust synthetic fixture"])
    git(profile, source, ["config", "user.email", "qualification@example.invalid"])
    private_write(source / "code.txt", "before\n")
    git(profile, source, ["add", "code.txt"])
    git(profile, source, ["commit", "-qm", "synthetic base"])
    commit = git(profile, source, ["rev-parse", "HEAD"])
    private_write(source / "unrelated.txt", "local work\n")
    base = daemon.call(["workspace", "export", "--goal", daemon.goal, "--root", source,
                        "--commit", commit])["manifest"]
    task = daemon.call(["task", "propose", "--goal", daemon.goal, "--input", base,
                       "Update only code.txt in the synthetic qualification workspace"])["recorded"]["event"]
    assignment = daemon.call(["task", "assign", "--goal", daemon.goal, "--task", task,
                             "--assignee", daemon.principal])["recorded"]["event"]
    daemon.call(["task", "authorize", "--goal", daemon.goal, "--assignment", assignment], owner=True)
    return {"source": str(source), "destination": str(profile.workspace / "worker"), "commit": commit,
            "base": base, "task": task, "assignment": assignment, "goal": daemon.goal,
            "expected": "after-" + client + "\n", "principal": daemon.principal}


def native_step(client, command, timeout_ms):
    if client == "codex":
        return step("exec_command", {"cmd": command, "login": False, "yield_time_ms": 30000})
    if client == "claude-code":
        return step("Bash", {"command": command, "timeout": timeout_ms,
                             "description": "Exercise the private Locust workspace"})
    if client == "factory-droid":
        return step("Execute", {"command": command, "timeout": timeout_ms / 1000,
                                "summary": "Exercise private Locust workspace",
                                "riskLevel": "medium",
                                "riskLevelReason": "Only synthetic files and a private disposable daemon are modified."})
    return step("bash", {"command": command, "timeout": timeout_ms / 1000})


WORKSPACE_DRIVER = '''import json, pathlib, subprocess, sys
settings=json.loads(pathlib.Path(sys.argv[1]).read_text())
def call(*args):
    result=subprocess.run(settings['cli']+list(args), capture_output=True, text=True, timeout=settings['timeout'])
    body=json.loads(result.stdout)
    if result.returncode or not body.get('ok'):
        raise RuntimeError('Locust workspace command failed: '+body.get('error',{}).get('code','unknown'))
    return body['result']
w=settings['work']; goal=w['goal']; source=pathlib.Path(w['source'])
claim=next(x for x in call('pending','--goal',goal)['pending']['claimed'] if x['assignment']==w['assignment'])
preview=call('workspace','preview','--root',w['source'],'--commit',w['commit'])
exported=call('workspace','export','--goal',goal,'--root',w['source'],'--commit',w['commit'])
assert exported['manifest']==w['base']
call('workspace','materialize','--goal',goal,'--manifest',w['base'],'--destination',w['destination'])
path=pathlib.Path(w['destination'])/'code.txt'
assert path.read_text()=='before\\n'
path.write_text(w['expected'])
patch=call('patch','create','--goal',goal,'--base',w['base'],'--root',w['destination'],'--path','code.txt')
pid=patch['contribution_id']; head=patch['contribution']['head']
review=call('patch','review','--goal',goal,'--patch',pid)
assert len(review['changes'])==1 and '+after-' in review['changes'][0]['unified_diff']
submitted=call('patch','submit','--goal',goal,'--patch',pid,'--assignment',w['assignment'],
               '--generation',str(claim['generation']),'Verified synthetic workspace change')
rid=submitted['recorded']['event']
call('patch','accept','--goal',goal,'--result',rid,'--patch',pid)
accepted=call('goal','status','--goal',goal)['goal_status']
assert accepted['head']==head and accepted['workspace']['integrated'] is None
assert (source/'code.txt').read_text()=='before\\n'
call('patch','apply','--goal',goal,'--patch',pid,'--root',w['source'],'--expected-base',w['base'],
     '--expected-git-head',w['commit'])
integrated=call('goal','status','--goal',goal)['goal_status']
assert integrated['workspace']['integrated']==head
assert (source/'code.txt').read_text()==w['expected']
assert (source/'unrelated.txt').read_text()=='local work\\n'
receipt={'patch':pid,'head':head,'result':rid,'generation':claim['generation'],
         'accepted_before_integrated':True,'preview':preview,'review':review}
pathlib.Path(settings['receipt']).write_text(json.dumps(receipt))
print(json.dumps({'workspace_driver':'completed','result':rid,'head':head}))
'''


def workspace_driver(profile, daemon, work, timeout):
    script = profile.workspace / "exercise_workspace.py"
    settings = profile.workspace / "workspace-settings.json"
    receipt = profile.logs / "workspace-result.json"
    private_write(script, WORKSPACE_DRIVER)
    private_write(settings, json.dumps({"cli": daemon.command([]), "work": work, "timeout": timeout,
                                       "receipt": str(receipt)}))
    return shlex.join([sys.executable, str(script), str(settings)]), receipt


def network_preflight(profile, env, provider, timeout):
    code = ('import errno,json,socket,sys; '
            'local=socket.create_connection(("127.0.0.1",int(sys.argv[1]))); local.close(); '
            'remote=socket.socket(); remote.settimeout(float(sys.argv[2])); '
            'code=remote.connect_ex(("192.0.2.1",9)); remote.close(); '
            'print(json.dumps({"loopback":True,"external_errno":code})); '
            'sys.exit(0 if code in (errno.EPERM,errno.EACCES) else 1)')
    return Process([sys.executable, "-c", code, str(provider.server.server_port), str(timeout)],
                   env, profile.workspace, profile.logs, "network-guard", timeout).wait()


def qualify(client, binary, args):
    result = {"client": client, "binary": binary, "version": None, "runs": [],
              "assertions": {name: fixture.assertion("not_run", "Scenario not executed") for name in ASSERTIONS}}
    if binary is None:
        return result
    profile = Profile(args.output, client)
    checks = result["assertions"]
    if client == "pi":
        (profile.home / ".pi/agent/sessions").mkdir(mode=0o700, parents=True, exist_ok=True)
    try:
        if platform.system() != "Darwin" or not Path("/usr/bin/sandbox-exec").is_file():
            return result
        timeout = args.timeout_ms / 1000
        env = profile.environment(binary)
        version = Process([binary, "--version"], env, profile.workspace, profile.logs, "version", timeout).wait()
        if version["exit_code"] != 0:
            raise ProductionError("Client version check failed")
        result.update(version=Path(version["stdout"]).read_text().strip(), version_check=version,
                      binary_sha256=hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
                      permission_note="Default headless and explicitly permissive scenarios are separate; Claude uses --bare",
                      provider_mode="scripted-loopback; no model service or real credentials")
        with ProductionDaemon(profile, args.locust, timeout) as daemon, Provider([]) as provider:
            result["locust_artifact"] = daemon.binary_metadata
            result["daemon_receipts"] = str(daemon.events)
            work = prepare_work(profile, daemon, client)
            result["work"] = work
            command, workspace_receipt = workspace_driver(profile, daemon, work, timeout)
            guard = network_preflight(profile, env, provider, timeout)
            result["network_guard_check"] = guard
            if guard["exit_code"] != 0 or guard["timed_out"]:
                raise ProductionError("External-network denial preflight failed")
            checks["network_isolation"] = fixture.assertion("pass", "Client loopback allowed, external raw IP denied; production daemon has separate loopback/Unix-only guard")
            env.update(fixture.provider_settings(client, profile, provider.url))

            def execute(label, plan, permissive=False, resume=None, interrupt=False):
                events_path = profile.logs / (label + ".mcp.jsonl")
                ready_path = profile.logs / (label + ".bridge.jsonl")
                private_write(events_path, b""); private_write(ready_path, b"")
                # A scripted provider can answer before a client's asynchronous
                # MCP registration finishes. Await actual discovery, not a fixed
                # delay or repeated failed searches that consume client turns.
                def after_discovery(arguments):
                    deadline = time.monotonic() + timeout
                    while not any(e.get("direction") == "bridge_response" and
                                  e.get("request_method") == "tools/list" for e in records(events_path)):
                        if time.monotonic() >= deadline:
                            raise ProductionError("Production MCP discovery did not complete before scripted call")
                        time.sleep(0.02)
                    return arguments() if callable(arguments) else arguments

                plan = list(plan)
                if plan:
                    first = plan[0]
                    plan[0] = step(first["tool"], lambda: after_discovery(first["arguments"]))
                with provider.lock:
                    provider.plan, provider.index = plan, 0
                arguments = [str(ROOT / "scripts/client_qualification/observe_mcp.py"),
                             "--events-file", str(events_path), "--executable", args.locust,
                             "--argument=mcp", "--argument=--lifecycle-receipt", "--argument=" + str(ready_path)]
                config = [args.config_probe, "--client", client, "--executable", sys.executable,
                          "--locust-home", str(daemon.home), "--credential", str(daemon.credential),
                          "--session", str(daemon.session), *["--argument=" + value for value in arguments]]
                proposal = json.loads(subprocess.run(config, env=profile.environment(binary), cwd=profile.workspace,
                                                    capture_output=True, timeout=timeout, check=True).stdout)
                profile.apply(proposal)
                argv = fixture.invocation(client, binary, proposal["arguments"], permissive, resume, profile)
                # This prompt describes the authored qualification scope; the local
                # provider emits the exact test calls, without a real model.
                argv[-1] = "Execute the scripted Locust production-daemon qualification calls in this private synthetic workspace."
                process = Process(argv, env, profile.workspace, profile.logs, label, timeout)
                observed_at = None

                def observe(child):
                    for event in records(events_path):
                        if event.get("event") == "bridge_started":
                            child.register_child(event.get("pid"), args.locust, ready_path)
                            child.register_child(event.get("observer_pid"), sys.executable, events_path)

                def held():
                    nonlocal observed_at
                    events = records(events_path)
                    if not outstanding_wait(events):
                        observed_at = None
                        return False
                    if observed_at is None:
                        observed_at = time.monotonic()
                    # Observe an unanswered forwarded wait, rather than interrupting
                    # at the provider's emission before the client calls the bridge.
                    return time.monotonic() - observed_at >= 0.25

                try:
                    run = process.wait(held if interrupt else None, observe=observe)
                finally:
                    process.close()
                run.update(scenario=label, mcp_events=str(events_path), lifecycle_receipts=str(ready_path),
                           configuration=proposal, permission_profile=("deliberately_permissive" if permissive and client != "pi" else "default_headless"),
                           client_opt_ins=["--bare"] if client == "claude-code" else [])
                result["runs"].append(run)
                return run, records(events_path, strict=True)

            default, de = execute("default", [step(READ, {"goal": daemon.goal}),
                                               step(WRITE, {"goal": daemon.goal, "text": "default-" + client})])
            default["observed_policy_denials"] = fixture.permission_denials(default)
            for kind, tool in (("read", READ), ("write", WRITE)):
                passed = bool(successful(de, tool))
                checks["default_" + kind] = fixture.assertion("pass" if passed else ("not_run" if denied(default, tool) else "fail"),
                    "Actual production tool completed" if passed else "Explicit default policy denial" if denied(default, tool) else "No successful production response or explicit denial")

            observed_claims = []

            def claim_args():
                pending = daemon.call(["pending", "--goal", daemon.goal])["pending"]
                claim = next(item for item in pending["claimed"] if item["assignment"] == work["assignment"])
                observed_claims.append(claim.copy())
                return {"goal": daemon.goal, "assignment": work["assignment"], "generation": claim["generation"],
                        "text": "progress-" + client}

            def wait_args():
                seen = daemon.call(["pending", "--goal", daemon.goal])["pending"]["revision"]
                return {"goal": daemon.goal, "seen": seen, "timeout_ms": args.timeout_ms}

            lifecycle, events = execute("lifecycle", [step(READ, {"goal": daemon.goal}),
                step(CLAIM, {"goal": daemon.goal, "assignment": work["assignment"]}),
                step(PROGRESS, claim_args), native_step(client, command, args.timeout_ms), step(WAIT, wait_args)],
                permissive=True, interrupt=True)
            combined = de + events
            result["tool_registry_source"] = {}
            registry = expected_tools(daemon.call(["status"])["status"]["daemon_version"], result["tool_registry_source"])
            initialized, listed = handshake(combined, registry)
            checks["configuration"] = fixture.assertion("pass" if initialized and listed else "fail", "Actual generated registration must expose a valid production handshake and exact pinned API tool registry")
            checks["initialize"] = fixture.assertion("pass" if initialized else "fail", "Valid JSON-RPC response, supported negotiated version, Locust serverInfo and tool capability required")
            checks["tools_list"] = fixture.assertion("pass" if listed else "fail", "Exact pinned API tool names, structured object schemas and coherent schema digest required")
            reads = successful(events, READ)
            authenticated = any(record_result(e).get("goal_status", {}).get("coordinator") == daemon.principal for e in reads)
            checks["scoped_daemon_authentication"] = fixture.assertion("pass" if authenticated else "fail", "Production bridge checked the enrolled non-owner credential; observed goal authority matches independent daemon setup")
            valid_claim = exact_claim(events, observed_claims, work, daemon.instance)
            checks["claim"] = fixture.assertion("pass" if valid_claim else "fail", "Actual bridge returned exact assignment, session instance and generation")
            progress = successful(events, PROGRESS)
            verified_progress = False
            progress_ids = []
            for event in progress:
                event_id = record_result(event).get("recorded", {}).get("event")
                if event_id:
                    detail = daemon.call(["event", "show", "--goal", daemon.goal, "--event", event_id])["event"]
                    matches = detail["view"]["author"] == daemon.principal and detail["text"] == "progress-" + client
                    verified_progress |= matches
                    if matches:
                        progress_ids.append(event_id)
            checks["progress"] = fixture.assertion("pass" if verified_progress else "fail", "Independent event read must match client-authored progress and principal")
            validate_workspace(checks, daemon, profile, work, workspace_receipt)
            held = outstanding_wait(events) and lifecycle["interrupted"]
            checks["held_wait"] = fixture.assertion("pass" if held else "fail", "Actual client's production wait was observed outstanding before leader-only SIGINT")
            clean = (lifecycle["natural_cleanup"] and lifecycle["cleanup_verified"] and not lifecycle["forced_cleanup"]
                     and expected_interrupt_exit(lifecycle, client, result["version"]))
            checks["interruption"] = fixture.assertion("pass" if held and clean else "fail", "Natural exit of client session and identified observer/bridge; no forced cleanup counts as success")
            session = fixture.session_identifier(client, lifecycle, profile)
            pi_path = profile.home / ".pi/agent/sessions/qualification-session.jsonl"
            pi_history = pi_path.read_bytes() if client == "pi" and pi_path.exists() else None
            if session and clean:
                restarted = persisted_restart(daemon, work, progress_ids)
                checks["daemon_restart"] = fixture.assertion("pass" if restarted else "fail", "Independent goal, task/assignment, claims, known progress and endpoint persist across SQLite restart; contribution head is optional")
                resumed, re = execute("resume", [step(READ, {"goal": daemon.goal}),
                    step(WRITE, {"goal": daemon.goal, "text": "resumed-" + client})], permissive=True,
                    resume=str(pi_path) if client == "pi" else session)
                same = fixture.session_identifier(client, resumed, profile) == session
                if pi_history is not None:
                    now = pi_path.read_bytes()
                    same = same and now.startswith(pi_history) and len(now) > len(pi_history)
                fresh_write = successful(re, WRITE)
                persisted = False
                for event in fresh_write:
                    event_id = record_result(event).get("recorded", {}).get("event")
                    if event_id:
                        note = daemon.call(["event", "show", "--goal", daemon.goal, "--event", event_id])["event"]
                        persisted = note["text"] == "resumed-" + client and note["view"]["author"] == daemon.principal
                checks["explicit_resume"] = fixture.assertion("pass" if same and persisted else "fail", "Same native session plus fresh independently read production note after daemon restart")
                old = {e["pid"] for e in events if e.get("event") == "bridge_started"}
                new = {e["pid"] for e in re if e.get("event") == "bridge_started"}
                checks["bridge_restart"] = fixture.assertion("pass" if new and not old & new and persisted else "fail", "New observed production bridge PID and committed resumed operation")
                resumed.update(native_session_id_before=session, native_session_id_after=fixture.session_identifier(client, resumed, profile))
            result["provider_requests"] = provider.requests
            result["provider_errors"] = provider.errors
            result["backend_requests"] = provider.backend_requests
            integrity = all(receipt_integrity(records(run["mcp_events"], strict=True)) for run in result["runs"])
            checks["receipt_integrity"] = fixture.assertion("pass" if integrity else "fail", "Parsed JSON-RPC 2.0 receipts have valid IDs and matched response method/tool correlation; interrupted requests may remain outstanding")
            good = not provider.errors and all(not run["timed_out"] and run["cleanup_verified"] and not run["forced_cleanup"] and
                    expected_client_exit(run, client, result["version"]) for run in result["runs"])
            checks["client_execution"] = fixture.assertion("pass" if good else "fail", "Provider errors, client exits, timeouts and natural owned-process cleanup checked")
    except Exception as error:
        result["harness_error"] = type(error).__name__ + ": " + str(error)
        checks["client_execution"] = fixture.assertion("fail", "Qualification failed; retain harness_error and receipts")
    finally:
        profile.close()
        result["private_runtime_profile_removed"] = not profile.root.exists()
    return result


def validate_workspace(checks, daemon, profile, work, receipt_path):
    exists = receipt_path.is_file()
    checks["workspace_native_tool"] = fixture.assertion("pass" if exists else "fail", "Client native command must execute the authored driver; the harness does not materialize/edit/submit/apply on its behalf")
    if not exists:
        return
    receipt = json.loads(receipt_path.read_text())
    status = daemon.call(["goal", "status", "--goal", daemon.goal])["goal_status"]
    task = daemon.call(["task", "show", "--goal", daemon.goal, "--task", work["task"]])["task"]
    result = daemon.call(["event", "show", "--goal", daemon.goal, "--event", receipt["result"]])["event"]
    passed = (task["view"]["state"] == "accepted" and task["view"]["applied"] is True and
              task["view"]["result"] == receipt["result"] and result["view"]["author"] == daemon.principal and
              status["head"] == receipt["head"] and status["workspace"]["integrated"] == receipt["head"] and
              (Path(work["source"]) / "code.txt").read_text() == work["expected"])
    checks["contribution_flow"] = fixture.assertion("pass" if passed else "fail", "Independent task/event/head/binding and file observations agree with actual client-produced contribution", str(receipt_path))
    checks["accepted_before_integrated"] = fixture.assertion("pass" if receipt.get("accepted_before_integrated") is True else "fail", "Client-executed driver observed accepted head while source remained unchanged and integration was null", str(receipt_path))
    untouched = ((Path(work["source"]) / "unrelated.txt").read_text() == "local work\n" and
                 git(profile, work["source"], ["rev-parse", "HEAD"]) == work["commit"])
    checks["dirty_work_preserved"] = fixture.assertion("pass" if untouched else "fail", "Independent file/HEAD checks preserve untracked local work and original commit")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout-ms", required=True, type=int)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--locust", required=True)
    parser.add_argument("--config-probe", required=True)
    for client in fixture.CLIENTS:
        parser.add_argument("--" + client)
    args = parser.parse_args(argv)
    if args.timeout_ms <= 0:
        parser.error("--timeout-ms must be positive")
    args.output = args.output.resolve()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=True)
    for name in ("locust", "config_probe"):
        binary = fixture.checked_binary(getattr(args, name))
        if binary is None:
            parser.error(name + " executable is missing")
        setattr(args, name, binary)
    clients = {client: fixture.checked_binary(getattr(args, client.replace("-", "_"))) for client in fixture.CLIENTS}
    report = {"schema": "locust-t2-client-qualification", "schema_version": 1,
              "created_at": datetime.now(timezone.utc).isoformat(), "timeout_ms": args.timeout_ms,
              "evidence_level": "actual-client/scripted-provider/production-daemon/local-synthetic-workspace",
              "scope": "One host, same principal in worker/coordinator roles; not real-model, independent-account, peer, managed-launch or installed-release qualification",
              "platform": {"os": platform.system(), "release": platform.release(), "architecture": platform.machine()},
              "candidate_commit": subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip(),
              "clients": []}
    sources = [Path(__file__), ROOT / "scripts/check_clients.py", *sorted((ROOT / "scripts/client_qualification").glob("*.py"))]
    report["harness_sources"] = [{"path": str(path.relative_to(ROOT)), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for path in sources]
    report["working_tree_status"] = subprocess.run(["git", "status", "--short", "--untracked-files=all"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.splitlines()
    report["artifacts"] = [{"path": path, "sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest()} for path in (args.locust, args.config_probe)]
    for client, binary in clients.items():
        print(json.dumps({"client": client, "phase": "starting"}), flush=True)
        result = qualify(client, binary, args)
        report["clients"].append(result)
        print(json.dumps({"client": client, "phase": "complete", "version": result["version"],
                          "assertions": {key: value["status"] for key, value in result["assertions"].items()},
                          "harness_error": result.get("harness_error")}), flush=True)
    path = args.output / "report.json"
    if path.exists():
        previous = path.read_bytes()
        private_write(args.output / ("report-" + hashlib.sha256(previous).hexdigest() + ".json"), previous)
    private_write(path, json.dumps(report, indent=2) + "\n")
    print(json.dumps({"report": str(path)}), flush=True)
    return int(any(check["status"] == "fail" for item in report["clients"] for check in item["assertions"].values()))


if __name__ == "__main__":
    sys.exit(main())
