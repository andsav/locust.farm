#!/usr/bin/env python3
"""Actual client discovery and task flow through a test-signed installed Locust.

Only setup plan/apply installs the MCP entry, skill and bound CLI. Native client JSON
receipts and independent durable daemon reads are required; provider-selected
calls alone never pass. Private profiles use dummy loopback providers and the
macOS OS network guard. Interactive human approval and real models are not tested.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shlex
import subprocess
import sys
import threading

import check_clients as fixture
import check_t2_clients as workflow
from check_installation import copy_bundle
from client_qualification.production import DAEMON_GUARD, ProductionDaemon, ProductionError
from client_qualification.provider import Provider
from client_qualification.runtime import Process, Profile, private_write, records

ROOT = Path(__file__).resolve().parents[1]
CLIENTS = ("codex", "claude-code", "pi")
SKILLS = {"codex": ".agents/skills/locust/SKILL.md", "claude-code": ".claude/skills/locust/SKILL.md",
          "pi": ".pi/agent/skills/locust/SKILL.md"}
CONFIGS = {"codex": ".codex/config.toml", "claude-code": ".claude.json", "pi": ".pi/agent/mcp.json"}
ASSERTIONS = ("test_signed_install", "persistent_setup", "setup_idempotent", "network_isolation",
              "bound_cli_launcher",
              "automatic_skill_metadata", "native_skill_read", "registered_mcp_roundtrip", "default_read",
              "default_write", "permissive_read", "permissive_write", "claim", "progress",
              "workspace_native_tool", "contribution_flow", "accepted_before_integrated", "dirty_work_preserved",
              "fresh_client_after_restart", "daemon_restart", "setup_removal", "unrelated_settings_preserved",
              "client_execution", "default_interactive_approval", "real_model", "production_release_trust")


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def require(value, message):
    if not value:
        raise ProductionError(message)


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for child in value.values():
            yield from strings(child)
    elif isinstance(value, list):
        for child in value:
            yield from strings(child)


def normalize_read(text):
    # Claude's native Read numbers lines before returning them to the model.
    return "\n".join(re.sub(r"^[ \t]*\d+(?:→|\t)", "", line) for line in text.replace("\r\n", "\n").splitlines()).strip()


class SkillObserver:
    """Transient request inspection; retained evidence is boolean/hash/path only."""
    def __init__(self, path):
        self.path = str(path)
        self.contents = Path(path).read_text()
        self.sha256 = digest(path)
        self.description = next(line.removeprefix("description: ") for line in self.contents.splitlines()
                                if line.startswith("description: "))
        self.observations = []
        self.lock = threading.Lock()

    def body_match(self, value):
        wanted = normalize_read(self.contents)
        return any(wanted in normalize_read(text) for text in strings(value))

    def read_projection(self, texts):
        expected = normalize_read(self.contents).splitlines()
        projected = []
        for text in texts:
            if "# Locust collaboration" not in text:
                continue
            actual = normalize_read(text).splitlines()
            prefix = next((line.split("# Locust collaboration", 1)[0] for line in text.splitlines() if "# Locust collaboration" in line), "")
            projected.append({"text_sha256":hashlib.sha256(text.encode()).hexdigest(),
                "text_bytes":len(text.encode()), "normalized_line_count":len(actual),
                "expected_line_count":len(expected),
                "missing_expected_line_numbers":[i+1 for i,line in enumerate(expected) if line and line not in actual],
                "header_prefix_codepoints":[ord(char) for char in prefix] if all(char.isspace() or char.isdigit() or not char.isalnum() for char in prefix) else None})
        return projected

    def __call__(self, body):
        texts = list(strings(body))
        observation = {"skill_path": self.path, "skill_sha256": self.sha256,
                       "description_sha256": hashlib.sha256(self.description.encode()).hexdigest(),
                       "description_present": any(self.description in text for text in texts),
                       "path_present": any(self.path in text for text in texts),
                       "body_present": self.body_match(body), "read_projection":self.read_projection(texts)}
        with self.lock:
            self.observations.append(observation)


def envelopes(value):
    """Decode only complete JSON text values, never substring-search an answer."""
    if isinstance(value, dict):
        if type(value.get("ok")) is bool and ("result" in value or "error" in value):
            yield value
        else:
            for child in value.values():
                yield from envelopes(child)
    elif isinstance(value, list):
        for child in value:
            yield from envelopes(child)
    elif isinstance(value, str):
        try:
            parsed = json.loads(value)
        except (ValueError, TypeError):
            return
        if not isinstance(parsed, str):
            yield from envelopes(parsed)


def project_envelope(value):
    """Only public IDs and booleans needed for independent state correlation."""
    if value.get("ok") is not True:
        return {"ok": False, "error_code": value.get("error", {}).get("code")}
    result = value.get("result", {})
    output = {"ok": True, "result": {}}
    if not isinstance(result, dict):
        return output
    for key, allowed in (("goal_status", ("goal", "coordinator", "head")),
                         ("recorded", ("event",)),
                         ("claimed", ("goal", "task", "assignment", "instance", "generation"))):
        if isinstance(result.get(key), dict):
            output["result"][key] = {name: result[key][name] for name in allowed if name in result[key]}
    return output


def native_receipts(client, events, issued, observer):
    """Project completed native events; selected calls and assistant text do not count."""
    result = []
    calls = {"call_fixture_" + str(index + 1): row for index, row in enumerate(issued)}
    native_calls = {}
    for event in events:
        if client == "claude-code" and event.get("type") == "assistant":
            for item in event.get("message", {}).get("content", []):
                if item.get("type") == "tool_use":
                    native_calls[item.get("id")] = {"tool":item.get("name"), "arguments":item.get("input")}
        elif client == "pi" and event.get("type") == "tool_execution_start":
            native_calls[event.get("toolCallId")] = {"tool":event.get("toolName"), "arguments":event.get("args")}
    for event in events:
        candidates = []
        if client == "codex" and event.get("type") == "item.completed":
            item = event.get("item", {})
            if item.get("type") == "mcp_tool_call" and item.get("server") == "locust":
                candidates.append((item.get("id"), item.get("tool"), item.get("result"),
                                   item.get("status") == "completed" and item.get("error") is None, item.get("arguments")))
            elif item.get("type") == "command_execution":
                candidates.append((item.get("id"), "exec_command", item.get("aggregated_output"),
                                   item.get("status") == "completed" and item.get("exit_code") == 0, item.get("command")))
        elif client == "claude-code" and event.get("type") == "user":
            for item in event.get("message", {}).get("content", []):
                if item.get("type") != "tool_result":
                    continue
                identifier = item.get("tool_use_id")
                called = calls.get(identifier, {})
                if called.get("discovery"):
                    continue
                native = native_calls.get(identifier, {})
                candidates.append((identifier, native.get("tool"), item.get("content"),
                                   item.get("is_error") is not True, native.get("arguments")))
        elif client == "pi" and event.get("type") == "tool_execution_end":
            candidates.append((event.get("toolCallId"), event.get("toolName"), event.get("result"),
                               event.get("isError") is not True,
                               native_calls.get(event.get("toolCallId"), {}).get("arguments")))
        for identifier, tool, contents, success, arguments in candidates:
            receipt = {"id": identifier, "tool": tool, "success": success,
                       "content_sha256": hashlib.sha256(json.dumps(contents, sort_keys=True).encode()).hexdigest(),
                       "skill_path_observed": any(observer.path in value for value in strings(arguments)),
                       "skill_body_match": success and observer.body_match(contents)}
            if isinstance(tool, str) and tool.startswith(("locust_", "mcp__locust__", "mcp__locust__locust_")):
                receipt["locust"] = [project_envelope(value) for value in envelopes(contents)]
            result.append(receipt)
    return result


def successful(receipts, tool):
    return [envelope for receipt in receipts if receipt.get("success") is True
            and isinstance(receipt.get("tool"), str) and receipt["tool"].endswith(tool)
            for envelope in receipt.get("locust", []) if envelope.get("ok") is True]


def read_matches(receipts, daemon):
    return any(e["result"].get("goal_status", {}).get("goal") == daemon.goal and
               e["result"].get("goal_status", {}).get("coordinator") == daemon.principal
               for e in successful(receipts, workflow.READ))


def persisted_note(receipts, daemon, text, tool=workflow.WRITE, assignment=None):
    if tool not in (workflow.WRITE, workflow.PROGRESS) or (tool == workflow.PROGRESS and assignment is None):
        return False
    expected_kind = "progress" if tool == workflow.PROGRESS else "note"
    for value in successful(receipts, tool):
        event = value["result"].get("recorded", {}).get("event")
        if event:
            observed = daemon.call(["event", "show", "--goal", daemon.goal, "--event", event])["event"]
            view = observed.get("view", {})
            if (observed.get("text") != text or view.get("author") != daemon.principal
                    or view.get("kind") != expected_kind or view.get("event") != event):
                continue
            if tool == workflow.PROGRESS and observed.get("body", {}).get("progress", {}).get("assignment") != assignment:
                continue
            return True
    return False


def registration_removed(client, config, skill, status):
    """Check remaining client bytes independently of the ownership status API."""
    if (status.get("owned") is not False or status.get("pending") is not False
            or os.path.lexists(skill) or os.path.lexists(skill.with_name("locust-cli"))):
        return False
    if not os.path.lexists(config):
        return True
    if config.is_symlink() or not config.is_file():
        return False
    try:
        if client == "codex":
            import tomllib
            document = tomllib.loads(config.read_text())
        else:
            document = json.loads(config.read_text())
    except (ValueError, OSError):
        return False
    if not isinstance(document, dict):
        return False
    servers = document.get("mcp_servers" if client == "codex" else "mcpServers", {})
    return isinstance(servers, dict) and "locust" not in servers


def invocation(client, binary, profile, label, permissive):
    # Empty overlay is intentional: only setup wrote MCP registration and skill.
    argv = fixture.invocation(client, binary, [], permissive, None, profile)
    argv = [argument for argument in argv if argument != "--bare"]
    if client == "pi":
        argv[argv.index("--session") + 1] = str(profile.home / ".pi/agent/sessions" / (label + ".jsonl"))
    argv[-1] = "Carry out the authored local qualification calls in this private synthetic workspace, then finish."
    return argv


def skill_read_step(client, path):
    if client == "claude-code":
        return workflow.step("Read", {"file_path": str(path)})
    if client == "pi":
        return workflow.step("read", {"path": str(path)})
    return workflow.step("exec_command", {"cmd": "cat " + shlex.quote(str(path)), "login": False,
                                          "yield_time_ms": 1000})


def command(profile, binary, arguments, timeout):
    value = subprocess.run(["/usr/bin/sandbox-exec", "-p", DAEMON_GUARD, str(binary),
                            "--home", str(profile.fixture), "--json", *map(str, arguments)],
                           cwd=profile.workspace, env=profile.environment(binary), capture_output=True,
                           text=True, timeout=timeout)
    try:
        body = json.loads(value.stdout)
    except ValueError:
        raise ProductionError("Installed-client CLI returned invalid JSON") from None
    require(value.returncode == 0 and body.get("ok") is True and not value.stderr,
            "Installed-client CLI operation failed: " + str(body.get("error", {}).get("code", "invalid")))
    return body["result"]


def install(profile, args):
    """Inert copy, disposable signer, then trusted installer activation."""
    bundle = profile.root / "bundle"
    copy_bundle(args.bundle, bundle)
    secret, public = profile.fixture / "signing.secret", profile.fixture / "signing.public"
    cli = lambda a: command(profile, args.bootstrap, a, args.timeout_ms / 1000)
    cli(["package", "keygen", "--secret-key", secret, "--public-key", public])
    cli(["package", "sign", "--bundle", bundle, "--secret-key", secret])
    registry = profile.fixture / "withdrawals.json"
    private_write(registry, json.dumps({"format": "locust-withdrawals-v1", "sequence": 1,
                                        "withdrawn_manifest_sha256": []}))
    cli(["package", "sign-withdrawals", "--registry", registry, "--secret-key", secret])
    prefix = profile.root / "software"
    inputs = ["--prefix", prefix, "--bundle", bundle, "--trust-key", public, "--withdrawals", registry]
    plan = cli(["install", "plan", *inputs])
    cli(["install", "apply", *inputs, "--expect-plan", plan["plan_sha256"]])
    status = cli(["install", "status", "--prefix", prefix])
    installed = prefix / "current/locust"
    require(status["installed"] and not status["withdrawn"] and
            status["manifest_sha256"] == digest(bundle / "manifest.json") and
            digest(installed) == digest(bundle / "locust"), "Installed artifact provenance mismatch")
    return prefix, installed, {"bootstrap_path": str(args.bootstrap), "bootstrap_sha256": digest(args.bootstrap),
        "manifest_sha256": status["manifest_sha256"], "manifest": status["manifest"],
        "installed_path": str(installed), "installed_binary_sha256": digest(installed),
        "skill_sha256": digest(prefix / "current/skills/locust/SKILL.md"),
        "test_trust_public_key_sha256": digest(public), "production_release_trust": False}


def setup(profile, daemon, prefix, binary, client, operation, timeout, expected=None):
    arguments = ["setup", operation, "--prefix", prefix, "--client", "claude" if client == "claude-code" else client,
                 "--profile-home", profile.home, "--workspace", profile.workspace,
                 "--daemon-home", daemon.home, "--credential-file", daemon.credential, "--session-file", daemon.session]
    if expected is not None:
        arguments += ["--expect-plan", expected]
    return command(profile, binary, arguments, timeout)


def provider_projection(provider):
    return [{key: row.get(key) for key in ("path", "declared_tools", "requested_tool", "selected_tool", "namespace", "discovery")}
            for row in provider.requests]


def qualify(client, binary, args):
    result = {"client": client, "binary": binary, "version": None, "runs": [],
              "assertions": {key: fixture.assertion("not_run", "Scenario not executed") for key in ASSERTIONS}}
    checks = result["assertions"]
    if binary is None:
        return result
    if platform.system() != "Darwin" or not Path("/usr/bin/sandbox-exec").is_file():
        checks["network_isolation"] = fixture.assertion("not_run", "macOS external-network guard unavailable; no client started")
        return result
    profile = Profile(args.output, client)
    timeout = args.timeout_ms / 1000
    try:
        env = profile.environment(binary)
        # Default HOME discovery locations, not a separate custom Claude config root.
        env.pop("CLAUDE_CONFIG_DIR", None)
        (profile.home / ".pi/agent/sessions").mkdir(mode=0o700, parents=True, exist_ok=True)
        version = Process([binary, "--version"], env, profile.workspace, profile.logs, "version", timeout).wait()
        require(version["exit_code"] == 0 and not version["timed_out"], "Client version check failed")
        result.update(version=Path(version["stdout"]).read_text().strip(), binary_sha256=digest(binary),
                      resolved_binary=str(Path(binary).resolve()), version_check=version,
                      provider_mode="scripted loopback; dummy auth; no real model",
                      binding_scope="dedicated profile and fixed explicit Locust session; not per-native-session identity")
        prefix, installed, artifact = install(profile, args)
        result["artifact"] = artifact
        checks["test_signed_install"] = fixture.assertion("pass", "Trusted bootstrap verified disposable test signature and exact payload before activation", artifact)
        with ProductionDaemon(profile, installed, timeout) as daemon:
            result["daemon_receipts"] = str(daemon.events)
            observer = None
            with Provider([], request_observer=lambda body: observer(body) if observer else None) as provider:
                env.update(fixture.provider_settings(client, profile, provider.url))
                config = profile.home / CONFIGS[client]
                if client != "codex":
                    private_write(config, json.dumps({"qualification_sentinel": "preserve", "mcpServers": {}}))
                baseline = config.read_bytes()
                plan = setup(profile, daemon, prefix, installed, client, "plan", timeout)
                applied = setup(profile, daemon, prefix, installed, client, "apply", timeout, plan["plan_sha256"])
                again = setup(profile, daemon, prefix, installed, client, "plan", timeout)
                repeated = setup(profile, daemon, prefix, installed, client, "apply", timeout, again["plan_sha256"])
                local_status = setup(profile, daemon, prefix, installed, client, "status", timeout)
                result["setup"] = {"plan": plan, "applied": applied, "repeat": repeated, "status": local_status}
                checks["persistent_setup"] = fixture.assertion("pass" if local_status["configured"] and local_status["binding_matches"] else "fail",
                    "Installed setup API owns the skill, bound launcher and persistent MCP entry; actual client discovery tested separately")
                checks["setup_idempotent"] = fixture.assertion("pass" if repeated["changed"] is False else "fail", "Second reviewed apply changes no owned file")
                skill_path = profile.home / SKILLS[client]
                launcher = skill_path.with_name("locust-cli")
                require(local_status.get("launcher") == str(launcher) and local_status.get("launcher_ready") is True,
                        "Setup did not return a ready bound CLI at the skill's installed path")
                observer = SkillObserver(skill_path)
                require(str(launcher) in observer.contents and os.access(launcher, os.X_OK),
                        "Installed skill must name its executable bound CLI")
                guard = workflow.network_preflight(profile, env, provider, timeout)
                result["network_guard_check"] = guard
                require(guard["exit_code"] == 0 and not guard["timed_out"], "Network guard preflight failed")
                checks["network_isolation"] = fixture.assertion("pass", "OS guard permits loopback and denies external raw-IP connection; daemon has separate loopback/Unix guard")
                work = workflow.prepare_work(profile, daemon, client)
                result["work"] = work
                native_command, workspace_receipt = workflow.workspace_driver(profile, daemon, work, timeout,
                                                                              cli=[str(launcher), "--json"])

                def execute(label, steps, permissive=False):
                    start = len(observer.observations)
                    with provider.lock:
                        provider.plan, provider.index = list(steps), 0
                    argv = invocation(client, binary, profile, label, permissive)
                    # Full native stdout (including the skill read) exists only in
                    # this disposable profile; durable output contains projections.
                    process = Process(argv, env, profile.workspace, profile.root / "logs", label, timeout)
                    run = process.wait()
                    events = records(run["stdout"], strict=True)
                    denials = fixture.permission_denials(run)
                    receipts = native_receipts(client, events, provider.requests, observer)
                    receipt_path = profile.logs / (label + ".native.json")
                    private_write(receipt_path, json.dumps(receipts, indent=2))
                    errors = Path(run["stderr"]).read_bytes()
                    native_id = next((event.get("thread_id") for event in events if event.get("type")=="thread.started"), None) if client=="codex" else next((event.get("session_id") for event in events if event.get("session_id")), None)
                    if client=="pi":
                        native_id = next((event.get("id") for event in records(profile.home / ".pi/agent/sessions" / (label+".jsonl")) if event.get("type")=="session"), None)
                    run.update(native_session_id=native_id, scenario=label, permission_profile="explicitly_permissive" if permissive and client != "pi" else "default_headless",
                               native_receipts=str(receipt_path), policy_denials=denials,
                               stderr_sha256=hashlib.sha256(errors).hexdigest(), stderr_bytes=len(errors),
                               skill_observations=observer.observations[start:])
                    run.pop("stdout"); run.pop("stderr")
                    result["runs"].append(run)
                    return run, receipts

                default, de = execute("default", [skill_read_step(client, skill_path), workflow.step(workflow.READ, {"goal": daemon.goal}),
                                                   workflow.step(workflow.WRITE, {"goal": daemon.goal, "text": "installed-default-" + client})])
                first_metadata = default["skill_observations"][:1]
                checks["automatic_skill_metadata"] = fixture.assertion("pass" if first_metadata and first_metadata[0]["description_present"] else "fail",
                    "Exact installed skill description observed in first provider request before any authored tool call; only hashes/booleans/paths retained", first_metadata)
                for key, tool, success in (("default_read", workflow.READ, read_matches(de, daemon)),
                                           ("default_write", workflow.WRITE, persisted_note(de, daemon, "installed-default-" + client))):
                    denied = any(row.get("tool_name", "").endswith(tool) for row in default["policy_denials"])
                    checks[key] = fixture.assertion("pass" if success else "not_run" if denied else "fail",
                        "Native successful response independently verified" if success else "Exact observed default policy denial" if denied else "No verified result or exact policy denial", default["policy_denials"] if denied else default["native_receipts"])

                independent_claims = []
                def progress_args():
                    pending = daemon.call(["pending", "--goal", daemon.goal])["pending"]
                    claim = next(c for c in pending["claimed"] if c["assignment"] == work["assignment"])
                    independent_claims.append(claim.copy())
                    return {"goal": daemon.goal, "assignment": work["assignment"], "generation": claim["generation"], "text": "installed-progress-" + client}

                active, ae = execute("permissive", [skill_read_step(client, skill_path), workflow.step(workflow.READ, {"goal": daemon.goal}),
                    workflow.step(workflow.WRITE, {"goal": daemon.goal, "text": "installed-permissive-" + client}),
                    workflow.step(workflow.CLAIM, {"goal": daemon.goal, "assignment": work["assignment"]}),
                    workflow.step(workflow.PROGRESS, progress_args), workflow.native_step(client, native_command, args.timeout_ms)], True)
                checks["native_skill_read"] = fixture.assertion("pass" if any(row["skill_body_match"] and row["skill_path_observed"] for row in de + ae) else "fail",
                    "Completed native tool response contains the exact installed skill body; retained receipt has hash and match boolean only")
                read = read_matches(ae, daemon)
                write = persisted_note(ae, daemon, "installed-permissive-" + client)
                checks["permissive_read"] = fixture.assertion("pass" if read else "fail", "Native MCP response matches independently created goal and principal")
                checks["permissive_write"] = fixture.assertion("pass" if write else "fail", "Native returned event ID independently resolves to expected committed note and author")
                checks["registered_mcp_roundtrip"] = fixture.assertion("pass" if read and write else "fail", "Actual native MCP result plus durable daemon state through setup-installed command, without registration overlays")
                observed_claims = [v["result"].get("claimed") for v in successful(ae, workflow.CLAIM)]
                claim = any(c in independent_claims and c.get("assignment") == work["assignment"] and c.get("instance") == daemon.instance
                            for c in observed_claims if isinstance(c, dict))
                checks["claim"] = fixture.assertion("pass" if claim else "fail", "Native claim matches fixed fixture instance and independently observed claim generation")
                checks["progress"] = fixture.assertion("pass" if persisted_note(ae, daemon, "installed-progress-" + client, workflow.PROGRESS, work["assignment"]) else "fail", "Native progress event independently resolves to exact event ID, progress kind, assignment, text and author")
                workflow.validate_workspace(checks, daemon, profile, work, workspace_receipt)
                checks["bound_cli_launcher"] = fixture.assertion(
                    "pass" if all(checks[key]["status"] == "pass" for key in
                                  ("workspace_native_tool", "contribution_flow", "accepted_before_integrated", "dirty_work_preserved")) else "fail",
                    "Native client workspace driver used only setup's launcher and --json; no executable/home/credential/session prefix supplied by the harness",
                    {"path": str(launcher), "sha256": digest(launcher), "skill_sha256": observer.sha256})
                before = daemon.call(["goal", "status", "--goal", daemon.goal])["goal_status"]
                endpoint = daemon.endpoint
                daemon.restart()
                after = daemon.call(["goal", "status", "--goal", daemon.goal])["goal_status"]
                checks["daemon_restart"] = fixture.assertion("pass" if before == after and endpoint == daemon.endpoint else "fail", "Installed daemon retains exact goal state and endpoint across restart")
                fresh, fe = execute("fresh-after-restart", [workflow.step(workflow.READ, {"goal": daemon.goal}),
                    workflow.step(workflow.WRITE, {"goal": daemon.goal, "text": "installed-restart-" + client})], True)
                fresh_good = (read_matches(fe, daemon) and persisted_note(fe, daemon, "installed-restart-" + client)
                              and bool(fresh["native_session_id"]) and fresh["native_session_id"] not in
                              {default["native_session_id"], active["native_session_id"]})
                checks["fresh_client_after_restart"] = fixture.assertion("pass" if fresh_good else "fail", "Fresh native process reads persisted state and commits new note through existing persistent setup after daemon restart")
                result["provider_requests"] = provider_projection(provider)
                result["provider_errors"] = provider.errors
                result["provider_backend_requests"] = provider.backend_requests
                clean = all(run["exit_code"] == 0 and not run["timed_out"] and run["cleanup_verified"] and not run["forced_cleanup"] for run in result["runs"])
                checks["client_execution"] = fixture.assertion("pass" if clean and not provider.errors else "fail", "All native scenarios exited naturally with verified owned-process cleanup and no provider scripting errors")
                remove_plan = setup(profile, daemon, prefix, installed, client, "remove-plan", timeout)
                removed = setup(profile, daemon, prefix, installed, client, "remove", timeout, remove_plan["plan_sha256"])
                result["removal"] = removed
                removed_status = setup(profile, daemon, prefix, installed, client, "status", timeout)
                checks["setup_removal"] = fixture.assertion("pass" if registration_removed(client, config, skill_path, removed_status) else "fail", "Independent config parse finds no Locust entry; skill, launcher, owner and pending journal are absent")
                if client == "codex":
                    import tomllib
                    retained = tomllib.loads(config.read_text())
                    original = tomllib.loads(baseline.decode())
                    preserved = all(retained.get(key) == value for key, value in original.items())
                else:
                    retained = json.loads(config.read_text())
                    preserved = retained.get("qualification_sentinel") == "preserve"
                checks["unrelated_settings_preserved"] = fixture.assertion("pass" if preserved else "fail", "Original provider settings or unrelated sentinel survives client execution and removal")
        for key, reason in (("real_model", "Scripted local provider; no real model invoked"),
                            ("default_interactive_approval", "Headless default policy only; no human approval interaction"),
                            ("production_release_trust", "Disposable test signer; no production publisher trust selected")):
            checks[key] = fixture.assertion("not_run", reason)
    except Exception as error:
        result["harness_error"] = type(error).__name__ + ": " + str(error)
        checks["client_execution"] = fixture.assertion("fail", "Qualification stopped; remaining assertions retain not_run")
    finally:
        profile.close()
        result["private_runtime_profile_removed"] = not profile.root.exists()
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bootstrap", type=Path, required=True)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout-ms", type=int, required=True)
    for client in CLIENTS:
        parser.add_argument("--" + client)
    args = parser.parse_args(argv)
    if not math.isfinite(args.timeout_ms) or args.timeout_ms <= 0:
        parser.error("--timeout-ms must be positive")
    if not args.bootstrap.is_absolute() or not args.bundle.is_absolute():
        parser.error("bootstrap and bundle must be absolute paths")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, mode=0o700, exist_ok=False)
    report = {"schema": "locust-installed-client-qualification", "schema_version": 1,
              "created_at": datetime.now(timezone.utc).isoformat(), "platform": platform.platform(),
              "evidence_level": "actual-native-client/scripted-loopback/test-signed-installed-daemon/synthetic-workspace",
              "limitations": ["No real model", "No interactive human approval", "No production publisher trust",
                              "One principal in worker/coordinator roles", "Dedicated profile and fixed explicit Locust session"],
              "harness_sources": [{"path": str(path.relative_to(ROOT)), "sha256": digest(path)} for path in
                  [Path(__file__), ROOT / "scripts/check_t2_clients.py", ROOT / "scripts/check_clients.py",
                   *sorted((ROOT / "scripts/client_qualification").glob("*.py"))]], "clients": []}
    for client in CLIENTS:
        binary = fixture.checked_binary(getattr(args, client.replace("-", "_")))
        print(json.dumps({"client": client, "phase": "starting"}), flush=True)
        result = qualify(client, binary, args)
        report["clients"].append(result)
        private_write(args.output / "report.json", json.dumps(report, indent=2) + "\n")
        print(json.dumps({"client": client, "phase": "complete", "harness_error": result.get("harness_error"),
                          "assertions": {key: value["status"] for key, value in result["assertions"].items()}}), flush=True)
    return int(any(check["status"] == "fail" for row in report["clients"] for check in row["assertions"].values()))


if __name__ == "__main__":
    raise SystemExit(main())
