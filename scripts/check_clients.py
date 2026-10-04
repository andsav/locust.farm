#!/usr/bin/env python3
"""Qualify four actual clients using Locust-generated MCP registration.

Example (build both Rust examples first):
  python3 scripts/check_clients.py --timeout-ms 30000 \
    --codex /absolute/codex --claude-code /absolute/claude \
    --factory-droid /absolute/droid --pi /absolute/pi

All runs use private profiles, dummy provider authentication, a local scripted
provider, and OS-enforced external network denial. No existing auth/profile is
read intentionally. Default headless policy, deliberate permissive lifecycle,
real account/model, daemon task flow, and packaged installation are distinct.
Missing clients/OS guards remain explicit not_run assertions. The caller selects
each operation/cleanup timeout; there are no model turn/tool-count caps.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

from client_qualification.provider import Provider
from client_qualification.runtime import Process, Profile, SocketFixture, private_write, records


CLIENTS = ("codex", "claude-code", "factory-droid", "pi")
READ = "locust_probe_socket"
WRITE = "locust_probe_write"
WAIT = "locust_probe_wait"
ASSERTIONS = ("configuration", "isolated_environment", "mcp_environment", "initialize", "tools_list",
              "default_read", "default_write", "default_interactive_approval", "permissive_read",
              "permissive_write", "held_wait", "interruption", "explicit_resume", "bridge_restart",
              "fixture_authentication", "daemon_task_flow", "daemon_authentication", "real_model",
              "own_account_authentication", "packaged_install", "active_session_delivery", "client_execution", "receipt_integrity", "network_isolation")


def assertion(status, reason, evidence=None):
    assert status in ("pass", "fail", "not_run")
    result = {"status": status, "reason": reason}
    if evidence is not None:
        result["evidence"] = evidence
    return result


def initial_client(name, binary):
    return {"client": name, "binary": binary, "version": None, "profiles": [], "runs": [],
            "assertions": {key: assertion("not_run", "Scenario not executed") for key in ASSERTIONS}}


def checked_binary(value):
    if value is None:
        return None
    path = Path(value)
    if not path.is_absolute():
        raise ValueError("Client/probe executable paths must be absolute")
    if not path.is_file() or not os.access(path, os.X_OK):
        return None
    return str(path.absolute())  # Preserve launch path; report resolved target too.


def provider_settings(client, profile, url):
    if client == "codex":
        private_write(profile.home / ".codex/config.toml", '\n'.join([
            'model = "locust-fixture"', 'model_provider = "locust_fixture"',
            '[model_providers.locust_fixture]', 'name = "Locust scripted fixture"',
            f'base_url = "{url}/v1"', 'wire_api = "responses"', 'requires_openai_auth = false', '']))
        return {"CODEX_DISABLE_UPDATE_CHECK": "1"}
    if client == "claude-code":
        return {"ANTHROPIC_BASE_URL": url, "ANTHROPIC_API_KEY": "locust-dummy-key",
                "DISABLE_TELEMETRY": "1", "DISABLE_ERROR_REPORTING": "1",
                "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1"}
    if client == "factory-droid":
        private_write(profile.home / ".factory/settings.json", json.dumps({"customModels": [{
            "model": "locust-fixture", "displayName": "Locust Fixture", "baseUrl": url + "/v1",
            "apiKey": "locust-dummy-key", "provider": "generic-chat-completion-api"}]}))
        # Native resume checks the Factory backend first. A scripted local 404
        # permits its local-session fallback; dummy auth remains isolated.
        return {"FACTORY_API_KEY": "fk-locust-dummy-key", "FACTORY_API_BASE_URL": url,
                "FACTORY_API_BASE_URL_EU": url}
    private_write(profile.home / ".pi/agent/models.json", json.dumps({"providers": {"locust-fixture": {
        "baseUrl": url + "/v1", "api": "openai-completions", "apiKey": "locust-dummy-key",
        "models": [{"id": "locust-fixture", "reasoning": False}]}}}))
    return {}


def invocation(client, binary, overlay, permissive=False, resume=None, profile=None):
    prompt = "Execute the fixture tool requests, then finish. Use only the Locust MCP fixture."
    if client == "codex":
        argv = [binary, *overlay]
        if permissive:
            argv += ["-a", "never", "-s", "danger-full-access"]
        argv += ["exec", "--skip-git-repo-check", "--json"]
        if resume:
            argv += ["resume", resume]
        return [*argv, prompt]
    if client == "claude-code":
        argv = [binary, "--bare", "-p", "--verbose", "--output-format", "stream-json",
                "--model", "claude-sonnet-4-5-20250929", *overlay]
        if permissive:
            argv += ["--dangerously-skip-permissions"]
        if resume:
            argv += ["--resume", resume]
        return [*argv, "--", prompt]
    if client == "factory-droid":
        argv = [binary, "exec", "--output-format", "stream-json", "--model", "custom:Locust-Fixture-0", *overlay]
        if permissive:
            argv += ["--skip-permissions-unsafe"]
        if resume:
            argv += ["--session-id", resume]
        return [*argv, prompt]
    argv = [binary, "--print", "--mode", "json", "--provider", "locust-fixture",
            "--model", "locust-fixture", "--thinking", "off", *overlay]
    if resume:
        argv += ["--session", resume]
    else:
        argv += ["--session", str(profile.home / ".pi/agent/sessions/qualification-session.jsonl")]
    return [*argv, prompt]


def session_identifier(client, run, profile):
    for event in records(run["stdout"]):
        if client == "codex" and event.get("type") == "thread.started":
            return event.get("thread_id")
        if client in ("claude-code", "factory-droid") and event.get("session_id"):
            return event["session_id"]
    if client == "pi":
        path = profile.home / ".pi/agent/sessions/qualification-session.jsonl"
        if path.exists():
            return next((event.get("id") for event in records(path, strict=True) if event.get("type") == "session"), None)
    return None


def tool_pass(events, receipts, tool, operation):
    return (any(event.get("event") == "tools/call" and event.get("phase") == "complete" and
                event.get("tool") == tool and event.get("success") is True for event in events) and
            any(receipt.get("operation") == operation and receipt.get("phase") == "complete" and
                receipt.get("authenticated_fixture") is True for receipt in receipts))


def permission_denials(run):
    result = []
    for event in records(run["stdout"]):
        result.extend(event.get("permission_denials", []))
        item = event.get("item", {})
        if event.get("type") == "item.completed" and item.get("type") == "mcp_tool_call" and item.get("status") == "failed":
            message = (item.get("error") or {}).get("message", "")
            if message == "MCP tool call requires approval, but approval policy is never":
                result.append({"tool_name": item.get("tool", ""), "reason": message})
        if event.get("type") == "tool_result" and event.get("isError") is True:
            message = event.get("error", {}).get("message", "")
            if "requires higher autonomy" in message:
                result.append({"tool_name": event.get("toolId"), "reason": "higher autonomy required"})
    return result


def qualify(name, binary, args):
    result = initial_client(name, binary)
    if not binary:
        for key in result["assertions"]:
            result["assertions"][key] = assertion("not_run", "Client executable missing; supply an absolute --" + name + " path")
        return result
    profile = Profile(args.output, name)
    try:
        return _qualify(name, binary, args, result, profile)
    except Exception as error:
        result["harness_error"] = type(error).__name__ + ": " + str(error)
        result["assertions"]["client_execution"] = assertion("fail", "Execution/receipt processing failed; see harness_error")
        return result
    finally:
        profile.close()
        result["private_runtime_profile_removed"] = not profile.root.exists()


def _qualify(name, binary, args, result, profile):
    checks = result["assertions"]
    if not binary:
        for key in checks:
            checks[key] = assertion("not_run", "Client executable missing; supply an absolute --" + name + " path")
        return result
    result["resolved_binary"] = str(Path(binary).resolve())
    result["binary_sha256"] = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    result["resolved_binary_sha256"] = hashlib.sha256(Path(binary).resolve().read_bytes()).hexdigest()
    result["profiles"].append({"root": str(profile.root), "directory_mode": "0700",
                               "authentication": "dummy isolated keys only", "client_environment_keys": []})
    env = profile.environment(binary)
    result["profiles"][0]["client_environment_keys"] = sorted(env)
    guard_available = sys.platform == "darwin" and Path("/usr/bin/sandbox-exec").exists()
    # Even version checks are isolated. No client is launched on platforms where
    # this implementation lacks an OS external-network guard.
    if not guard_available:
        for key in checks:
            checks[key] = assertion("not_run", "OS external-network denial unavailable on this platform")
        return result
    version = Process([binary, "--version"], env, profile.workspace, profile.logs,
                      "version", args.timeout_ms / 1000).wait()
    result["version"] = Path(version["stdout"]).read_text().strip()
    result["version_check"] = version
    if version["exit_code"] != 0 or not result["version"]:
        raise RuntimeError(name + " version check failed")
    checks["isolated_environment"] = assertion("pass", "Cleared client environment; private HOME/config/tmp/workspace; bridge paths omitted")
    proposal_command = [args.config_probe, "--client", name, "--executable", args.stdio_probe,
                        "--locust-home", str(profile.fixture), "--session", str(profile.session),
                        "--credential", str(profile.credential), "--argument=--io-timeout-ms",
                        "--argument=" + str(args.timeout_ms)]
    config = subprocess.run(proposal_command, env=env, cwd=profile.workspace, capture_output=True,
                            timeout=args.timeout_ms / 1000, check=True)
    proposal = json.loads(config.stdout)
    profile.apply(proposal)
    result["generated_configuration"] = proposal
    checks["configuration"] = assertion("pass", "Registration generated and applied to private profile; parsing/readiness checked separately")
    checks["default_interactive_approval"] = assertion("not_run", "Headless default policy observed; interactive approval requires a human; permissive run is separate")
    for key, reason in (("daemon_task_flow", "No actual Locust task/daemon implemented in this fixture"),
                        ("daemon_authentication", "Fixture checks protected bytes only; not the daemon handshake"),
                        ("real_model", "Scripted local provider, no real model invoked"),
                        ("own_account_authentication", "Dummy auth only; existing user auth is intentionally excluded"),
                        ("packaged_install", "Local debug fixture; no packaged Locust artifact"),
                        ("active_session_delivery", "No actual daemon pending-work delivery path under test")):
        checks[key] = assertion("not_run", reason)
    with Provider([]) as provider, SocketFixture(profile) as fixture:
        guard_code = '\n'.join([
            'import errno,json,socket,sys',
            'local=socket.create_connection(("127.0.0.1",int(sys.argv[1])))', 'local.close()',
            'external=socket.socket()', 'external.settimeout(float(sys.argv[2]))',
            'code=external.connect_ex(("192.0.2.1",9))', 'external.close()',
            'print(json.dumps({"loopback":True,"external_errno":code}))',
            'sys.exit(0 if code in (errno.EPERM,errno.EACCES) else 1)'])
        guard_run = Process([sys.executable, "-c", guard_code, str(provider.server.server_port),
                             str(args.timeout_ms / 1000)], env, profile.workspace, profile.logs,
                            "network-guard", args.timeout_ms / 1000).wait()
        result["network_guard_check"] = guard_run
        if guard_run["exit_code"] != 0 or guard_run["timed_out"]:
            checks["network_isolation"] = assertion("fail", "OS guard failed local-allowed/external-EPERM preflight; refusing model/client run")
            return result
        checks["network_isolation"] = assertion("pass", "Guard allowed loopback and rejected raw TEST-NET address with EPERM/EACCES before client calls", guard_run["stdout"])
        env.update(provider_settings(name, profile, provider.url))
        env.update(proposal.get("environment", {}))
        result["provider"] = {"mode": "scripted-loopback", "base_url": provider.url,
                              "network_guard": "macOS sandbox-exec: deny network except localhost/Unix sockets"}
        if name == "factory-droid":
            result["provider"]["factory_backend"] = {
                "mode": "scripted-loopback-session-lookup-404", "base_url": provider.url,
                "purpose": "Native Droid resume performs a backend lookup; fixture 404 allows its local-session fallback"}

        def execute(label, plan, permissive=False, resume=None, interrupt=False):
            provider.plan, provider.index = list(plan), 0
            before = len(fixture.receipts)
            events_path = profile.logs / (label + ".mcp.jsonl")
            # The events option belongs only to the MCP executable's argv.
            command = [*proposal_command, "--argument=--events-file", "--argument=" + str(events_path)]
            registration = json.loads(subprocess.run(command, env=profile.environment(binary),
                                                     cwd=profile.workspace, capture_output=True,
                                                     timeout=args.timeout_ms / 1000, check=True).stdout)
            profile.apply(registration)
            argv = invocation(name, binary, registration["arguments"], permissive, resume, profile)
            process = Process(argv, env, profile.workspace, profile.logs, label, args.timeout_ms / 1000)
            try:
                def observe(child):
                    for event in records(events_path):
                        if event.get("event") == "environment":
                            child.register_child(event.get("pid"), args.stdio_probe, events_path)
                run = process.wait(fixture.wait_started.is_set if interrupt else None, observe=observe)
            finally:
                process.close()
            permission_profile = "deliberately_permissive" if permissive and name != "pi" else "default_headless"
            if name == "claude-code":
                permission_profile += "_bare"
            run.update({"scenario": label, "permission_profile": permission_profile,
                        "mcp_events": str(events_path), "receipts": fixture.receipts[before:],
                        "client_opt_ins": ["--bare (no hooks/keychain/profile discovery)"] if name == "claude-code" else []})
            result["runs"].append(run)
            return run, records(events_path, strict=True)

        default_run, default_events = execute("default", [READ, WRITE])
        denials = permission_denials(default_run)
        default_run["observed_policy_denials"] = denials
        result["default_permission_outcomes"] = []
        for tool, operation in ((READ, "read"), (WRITE, "write")):
            passed = tool_pass(default_events, default_run["receipts"], tool, operation)
            denied = any(item.get("tool_name", "").endswith(tool) for item in denials)
            result["default_permission_outcomes"].append({"tool": tool, "outcome": "completed" if passed else ("denied" if denied else "no_receipt")})
            checks["default_" + operation] = assertion("pass" if passed else ("not_run" if denied else "fail"),
                "Observed successful tool receipt" if passed else ("Default headless policy explicitly denied tool; human approval not supplied" if denied else "Attempted default headless scenario produced no successful receipt"),
                default_run["mcp_events"])
        lifecycle, events = execute("lifecycle", [READ, WRITE, WAIT], permissive=True, interrupt=True)
        combined = default_events + events
        for key, event_name in (("initialize", "initialize"), ("tools_list", "tools/list")):
            passed = any(event.get("event") == event_name and event.get("success") is True and
                         event.get("phase") == "complete" for event in combined)
            checks[key] = assertion("pass" if passed else "fail", "MCP protocol completion receipt required", lifecycle["mcp_events"])
        environment = any(event.get("event") == "environment" and all(event.get(key) is True for key in ("home", "session", "credential")) for event in combined)
        checks["mcp_environment"] = assertion("pass" if environment else "fail", "MCP child validated three protected environment/file references")
        for tool, operation in ((READ, "read"), (WRITE, "write")):
            passed = tool_pass(events, lifecycle["receipts"], tool, operation)
            checks["permissive_" + operation] = assertion("pass" if passed else "fail", ("Default isolated Pi policy, no permission extension or bypass installed; " if name == "pi" else "Deliberately permissive policy; ") + "actual protocol + authenticated fixture receipts required", lifecycle["mcp_events"])
        held = fixture.wait_started.is_set() and any(event.get("tool") == WAIT and event.get("phase") == "start" for event in events)
        checks["held_wait"] = assertion("pass" if held else "fail", "Harness-held Unix socket wait reached by actual client MCP call")
        stopped = lifecycle["natural_cleanup"] and lifecycle["cleanup_verified"] and not lifecycle["forced_cleanup"]
        checks["interruption"] = assertion("pass" if held and lifecycle["interrupted"] and stopped else ("fail" if held else "not_run"), "Held wait, leader-only SIGINT, and natural exit of the owned session and receipt-identified bridges required; forced cleanup cannot pass; unobserved fully detached descendants are outside this evidence")
        session = session_identifier(name, lifecycle, profile)
        pi_path = profile.home / ".pi/agent/sessions/qualification-session.jsonl"
        pi_history = pi_path.read_bytes() if name == "pi" and pi_path.exists() else None
        fixture.hold = False
        fixture.release.set()
        if held and session:
            resumed, resumed_events = execute("resume", [READ], permissive=True,
                                              resume=str(pi_path) if name == "pi" else session)
            passed = tool_pass(resumed_events, resumed["receipts"], READ, "read")
            same_session = session_identifier(name, resumed, profile) == session
            if pi_history is not None:
                history_now = pi_path.read_bytes()
                same_session = same_session and history_now.startswith(pi_history) and len(history_now) > len(pi_history)
            resumed["native_session_id_before"] = session
            resumed["native_session_id_after"] = session_identifier(name, resumed, profile)
            checks["explicit_resume"] = assertion("pass" if passed and same_session else "fail", "Same native session identifier and fresh successful MCP read receipt required", resumed["mcp_events"])
            old_pids = {event.get("pid") for event in events if event.get("event") == "environment"}
            new_pids = {event.get("pid") for event in resumed_events if event.get("event") == "environment"}
            fresh_environment = any(event.get("event") == "environment" and all(event.get(key) is True for key in ("home", "session", "credential")) for event in resumed_events)
            checks["bridge_restart"] = assertion("pass" if passed and fresh_environment and new_pids and not (old_pids & new_pids) else "fail", "Fresh bridge PID/environment and successful resumed read required; not daemon durable recovery")
        else:
            checks["explicit_resume"] = assertion("not_run", "No held wait or native session identifier available")
            checks["bridge_restart"] = assertion("not_run", "Resume/restart preconditions not reached")
        authenticated = bool(fixture.receipts) and all(receipt.get("authenticated_fixture") is True for receipt in fixture.receipts)
        checks["fixture_authentication"] = assertion("pass" if authenticated else "fail", "Socket fixture compared session/credential bytes without retaining them")
        result["provider_requests"] = provider.requests
        result["backend_requests"] = provider.backend_requests
        result["provider_errors"] = provider.errors
        good_execution = not provider.errors and all(not run["timed_out"] and run["cleanup_verified"] and
                            (run["exit_code"] == 0 or run["interrupted"] or bool(run.get("observed_policy_denials"))) for run in result["runs"])
        checks["client_execution"] = assertion("pass" if good_execution else "fail", "Provider errors, timeout, exit code and cleanup of the owned session and receipt-identified bridges checked separately from individual tool receipts; unobserved fully detached descendants are outside this evidence")
        checks["receipt_integrity"] = assertion("pass", "Completed MCP logs parsed strictly; no malformed records ignored")
        private_write(profile.logs / "fixture-receipts.json", json.dumps(fixture.receipts, indent=2) + "\n")
    return result


def main(argv=None):
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout-ms", required=True, type=int, help="Operator-authored timeout for each run and cleanup")
    parser.add_argument("--output", type=Path, default=root / "output/client-qualification")
    parser.add_argument("--config-probe", default=str(root / "target/debug/examples/config_probe"))
    parser.add_argument("--stdio-probe", default=str(root / "target/debug/examples/stdio_probe"))
    for client in CLIENTS:
        parser.add_argument("--" + client, help="Absolute installed executable; omitted means explicit not_run")
    args = parser.parse_args(argv)
    if args.timeout_ms <= 0:
        parser.error("--timeout-ms must be positive")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True, mode=0o700)
    try:
        binaries = {client: checked_binary(getattr(args, client.replace("-", "_"))) for client in CLIENTS}
        args.config_probe = checked_binary(args.config_probe)
        args.stdio_probe = checked_binary(args.stdio_probe)
        if not args.config_probe or not args.stdio_probe:
            parser.error("Build config_probe and stdio_probe with cargo build --locked -p locust-adapter --examples, or pass absolute paths")
    except ValueError as error:
        parser.error(str(error))
    report = {"schema": "locust-client-qualification", "schema_version": 1,
              "created_at": datetime.now(timezone.utc).isoformat(),
              "platform": {"os": platform.system(), "release": platform.release(), "architecture": platform.machine()},
              "timeout_ms": args.timeout_ms, "evidence_level": "actual-client/scripted-provider/local-fixture",
              "scope": "Configuration, MCP readiness and explicit client resume only; R6/R6L/R8 remain open",
              "sources": ["https://docs.factory.com/droid-exec/overview",
                          "https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/mcp.md",
                          "https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/models.md"],
              "clients": []}
    report["candidate_commit"] = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root,
                                               capture_output=True, text=True, check=True).stdout.strip()
    report["candidate_source_note"] = "Repository base commit; source snapshots and dirty inputs below identify the current harness state"
    harness_sources = [Path(__file__).resolve(), *sorted((root / "scripts/client_qualification").glob("*.py")),
                       root / "scripts/tests/test_client_qualification.py"]
    report["harness_sources"] = [{"path": str(path.relative_to(root)),
                                  "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for path in harness_sources]
    build_inputs = ["scripts/check_clients.py", "scripts/client_qualification", "scripts/tests/test_client_qualification.py",
                    "crates/locust-adapter", "crates/locust-proto", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
    dirty = subprocess.run(["git", "status", "--porcelain=v1", "--untracked-files=all", "--", *build_inputs],
                           cwd=root, capture_output=True, text=True, check=True).stdout.splitlines()
    report["working_tree_inputs"] = {"dirty": bool(dirty), "status": dirty,
                                    "scope": build_inputs, "build_attestation": "Source snapshot only; exact probe binaries hashed separately"}
    report["probe_artifacts"] = [{"path": path, "sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest()}
                                 for path in (args.config_probe, args.stdio_probe)]
    for client, binary in binaries.items():
        try:
            report["clients"].append(qualify(client, binary, args))
        except Exception as error:
            result = initial_client(client, binary)
            result["harness_error"] = type(error).__name__ + ": " + str(error)
            result["assertions"]["configuration"] = assertion("fail", "Harness/client execution failed; see harness_error")
            report["clients"].append(result)
    path = args.output / "report.json"
    if path.exists():
        previous = path.read_bytes()
        previous_path = args.output / ("report-" + hashlib.sha256(previous).hexdigest() + ".json")
        if not previous_path.exists():
            private_write(previous_path, previous)
    private_write(path, json.dumps(report, indent=2) + "\n")
    print(json.dumps({"report": str(path), "clients": [{"client": item["client"], "version": item["version"],
                     "assertions": {key: value["status"] for key, value in item["assertions"].items()}}
                    for item in report["clients"]]}, indent=2))
    return int(any(check["status"] == "fail" for item in report["clients"] for check in item["assertions"].values()))


if __name__ == "__main__":
    sys.exit(main())
