#!/usr/bin/env python3
"""Actual-client qualification of explicit Locust-managed launch and resume.

Uses scripted providers, synthetic task IDs and private profiles. The production
daemon remains authoritative. No real provider, remote peer or installed package
is exercised. Operator timeout is supplied explicitly and retained in evidence.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import signal
import stat
from pathlib import Path
import subprocess
import sys
import threading
import time

import check_clients as fixture
from check_t2_clients import expected_interrupt_exit, step
from client_qualification.production import ProductionDaemon, ProductionError
from client_qualification.provider import Provider
from client_qualification.runtime import Process, Profile, private_write, records


CHECKS = ("ready", "binding", "claim", "ordinary_pending", "held_wait", "interruption",
          "exited", "cancellation_durable", "daemon_restart", "explicit_resume",
          "cancellation_acknowledged", "profile_restored", "cleanup", "blocked",
          "uncertain_recovery", "automatic_wake", "real_model", "packaged_install")


def cancellation_acknowledged(cancel_detail, ack_detail, pending, *, cancel, attempt, task, principal):
    """Exact effective protocol-2 acknowledgment plus removed local obligation."""
    return (cancel_detail.get("view", {}).get("event") == cancel
        and cancel_detail.get("view", {}).get("standing") == "effective"
        and cancel_detail.get("body") == {"cancel_requested": {"attempt": attempt}}
        and cancel_detail.get("task") == task
        and ack_detail.get("view", {}).get("kind") == "cancel_acknowledged"
        and ack_detail.get("view", {}).get("standing") == "effective"
        and ack_detail.get("view", {}).get("author") == principal
        and ack_detail.get("body") == {"cancel_acknowledged": {"cancel": cancel, "outcome": "uncertain"}}
        and ack_detail.get("task") == task
        and not any(item.get("cancel") == cancel for item in pending.get("to_acknowledge", [])))


def metadata(view):
    detail = view.get("record", view).get("detail", [])
    return json.loads(bytes(detail)) if detail else {}


def native_wait_ids(path):
    """Outstanding structured native invocations, matched to completion IDs."""
    pending = set()
    for event in records(path):
        kind = event.get("type")
        if kind in ("item.started", "item.completed"):
            item = event.get("item", {})
            identifier = item.get("id")
            if kind == "item.completed":
                pending.discard(identifier)
            elif item.get("type") == "mcp_tool_call" and item.get("tool", "").endswith("locust_wait") and identifier:
                pending.add(identifier)
        elif kind in ("tool_call", "tool_result"):
            identifier = event.get("id")
            if kind == "tool_result":
                pending.discard(identifier)
            elif event.get("toolId", "").endswith("locust_wait") and identifier:
                pending.add(identifier)
        elif kind in ("assistant", "user"):
            for item in event.get("message", {}).get("content", []):
                if not isinstance(item, dict):
                    continue
                if item.get("type") == "tool_result":
                    pending.discard(item.get("tool_use_id"))
                elif item.get("type") == "tool_use" and item.get("name", "").endswith("locust_wait") and item.get("id"):
                    pending.add(item["id"])
        elif kind in ("tool_execution_start", "tool_execution_end"):
            identifier = event.get("toolCallId")
            if kind == "tool_execution_end":
                pending.discard(identifier)
            elif identifier and (event.get("toolName", "").endswith("locust_wait") or
                                 (event.get("toolName") == "codemode" and
                                  "tools.mcp__locust__locust_wait(" in event.get("args", {}).get("code", ""))):
                pending.add(identifier)
    return pending


def native_wait_started(path):
    return bool(native_wait_ids(path))


def ready_evidence(view, native_output):
    record = view.get("record", {})
    detail = metadata(view)
    native = record.get("client_session")
    if (record.get("state") != "ready" or not native or not detail.get("initialized")
            or not detail.get("tools_ready")):
        return False
    events = records(native_output)
    native_path = detail.get("native_session_file")
    if native_path:
        events += records(native_path)
    identified = any((event.get("type") == "thread.started" and event.get("thread_id") == native)
                     or (event.get("type") == "system" and event.get("subtype") == "init" and event.get("session_id") == native)
                     or (event.get("type") == "session" and event.get("id") == native) for event in events)
    path = Path(detail.get("lifecycle_receipt", ""))
    try:
        permissions = path.lstat()
        if (not stat.S_ISREG(permissions.st_mode) or stat.S_IMODE(permissions.st_mode) != 0o600
                or permissions.st_uid != os.getuid() or permissions.st_nlink != 1):
            return False
        receipts = records(path, strict=True)
    except (OSError, ValueError):
        return False
    launch = any(event.get("event") == "launch_receipt" and event.get("launch_id") == detail.get("launch_id") for event in receipts)
    tools = any(event.get("schema") == 1 and event.get("event") == "tools_ready"
                and event.get("instance") == view.get("instance")
                and type(event.get("pid")) is int and event["pid"] > 0 for event in receipts)
    return identified and launch and tools


def clean_run(run):
    return (run.get("exit_code") == 0 and run.get("natural_cleanup") is True
            and run.get("cleanup_verified") is True and not run.get("forced_cleanup")
            and not run.get("timed_out"))


def expected_native_exit(view, interrupted):
    process = metadata(view).get("process", {})
    client = view.get("record", {}).get("client", "")
    codex_interrupt = (process.get("termination_signal") is None and
                       client == "codex codex-cli 0.153.4" and
                       expected_interrupt_exit({"exit_code": process.get("exit_code"), "interrupted": interrupted},
                                               "codex", "codex-cli 0.153.4"))
    return (process.get("exited") is True and
            ((process.get("exit_code") == 0 and process.get("termination_signal") is None)
             or (interrupted and process.get("exit_code") is None and process.get("termination_signal") == signal.SIGINT)
             or codex_interrupt))


def flags(client, permissive):
    if client == "codex":
        return (["-a", "never", "-s", "danger-full-access"] if permissive else []), ["--skip-git-repo-check"]
    if client == "claude-code":
        return ["--bare"], ["--model", "claude-sonnet-4-5-20250929", *(["--dangerously-skip-permissions"] if permissive else [])]
    if client == "factory-droid":
        return [], ["--model", "custom:Locust-Fixture-0", *(["--skip-permissions-unsafe"] if permissive else [])]
    return [], ["--provider", "locust-fixture", "--model", "locust-fixture", "--thinking", "off"]


def qualify(client, binary, args):
    result = {"client": client, "binary": binary, "runs": [],
              "assertions": {name: fixture.assertion("not_run", "Scenario not executed") for name in CHECKS}}
    if binary is None:
        return result
    profile = Profile(args.output, client)
    checks = result["assertions"]
    timeout = args.timeout_ms / 1000
    try:
        env = profile.environment(binary)
        version = Process([binary, "--version"], env, profile.workspace, profile.logs, "version", timeout).wait()
        if version["exit_code"] != 0:
            raise ProductionError("Client version check failed")
        result["version"] = Path(version["stdout"]).read_text().strip()
        result["binary_sha256"] = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
        result["policy"] = "explicitly permissive lifecycle; native default-policy runs reported separately"
        with ProductionDaemon(profile, args.locust, timeout) as daemon, Provider([]) as provider:
            result["locust_artifact"] = daemon.binary_metadata
            result["daemon_receipts"] = str(daemon.events)
            env.update(fixture.provider_settings(client, profile, provider.url))
            task = "task:" + daemon.call(["task", "open", "--goal", daemon.goal, "Managed synthetic task"])["recorded"]["event"]
            offer = daemon.call(["work", "offer", "--goal", daemon.goal, "--task", task,
                "--recipient", daemon.principal])["recorded"]["event"]
            daemon.call(["task", "authorize", "--goal", daemon.goal, "--task", task, "--agent", daemon.principal], owner=True)
            claim = daemon.call(["attempt", "start", "--goal", daemon.goal, "--task", task, "--offer", offer])["claimed"]
            attempt = claim["attempt"]
            result["binding"] = {"goal": daemon.goal, "task": task, "attempt": attempt,
                                 "instance": daemon.instance, "principal": daemon.principal}
            pi_file = profile.home / ".pi/agent/sessions/managed-session.jsonl"
            if client == "pi":
                pi_file.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            config_file = profile.home / (".factory/mcp.json" if client == "factory-droid" else ".pi/agent/mcp.json")
            if client in ("factory-droid", "pi"):
                private_write(config_file, json.dumps({"mcpServers": {}, "qualification_marker": "preserve"}))
            baseline = config_file.read_bytes() if config_file.exists() else None

            def run(label, plan, resume=None, interrupt=False):
                ready = threading.Event()
                started_wait = None
                last_wait_ids = set()
                held_wait_ids = []
                views = []
                last_view = None
                first = plan[0]

                def after_ready():
                    if not ready.wait(timeout):
                        raise ProductionError("Managed readiness not observed before operator timeout")
                    arguments = first["arguments"]
                    return arguments() if callable(arguments) else arguments

                with provider.lock:
                    provider.plan = [step(first["tool"], after_ready), *plan[1:]]
                    provider.index = 0
                global_args, client_args = flags(client, True)
                command = daemon.command(["client", "run", "--client", client, "--executable", binary,
                    "--workspace", profile.workspace, "--profile", profile.home, "--client-version", result["version"],
                    "--goal", daemon.goal, "--attempt", attempt,
                    "--prompt", "Perform the scripted Locust managed-session qualification in this private synthetic task."])
                command += ["--global-arg=" + value for value in global_args]
                command += ["--arg=" + value for value in client_args]
                if client == "pi":
                    command += ["--native-session", str(pi_file)]
                if resume:
                    command += ["--resume", resume]
                process = Process(command, env, profile.workspace, profile.logs, label, timeout)
                next_poll = 0

                def observe(child):
                    nonlocal last_view, next_poll
                    if time.monotonic() < next_poll:
                        return
                    next_poll = time.monotonic() + 0.1
                    try:
                        view = daemon.call(["client", "status"])["session"]
                    except ProductionError as error:
                        if error.code != "not_found":
                            raise
                        return
                    detail = metadata(view)
                    if detail.get("launch_id") != (metadata(last_view).get("launch_id") if last_view else None) or view != last_view:
                        views.append(view)
                        last_view = view
                    if ready_evidence(view, process.stderr_path):
                        ready.set()
                    path = detail.get("lifecycle_receipt")
                    if path:
                        for event in records(path):
                            if event.get("event") == "tools_ready":
                                child.register_child(event.get("pid"), args.locust, path)

                def held():
                    nonlocal started_wait, last_wait_ids, held_wait_ids
                    outstanding = native_wait_ids(process.stderr_path)
                    if not outstanding:
                        started_wait = None
                        last_wait_ids = set()
                        return False
                    if started_wait is None or outstanding != last_wait_ids:
                        started_wait = time.monotonic()
                        last_wait_ids = outstanding
                    if time.monotonic() - started_wait >= 0.25:
                        held_wait_ids = sorted(outstanding)
                        return True
                    return False

                try:
                    invocation = process.wait(held if interrupt else None, observe=observe)
                finally:
                    ready.set()
                    process.close()
                final = daemon.call(["client", "status"])["session"]
                views.append(final)
                path = profile.logs / (label + ".sessions.json")
                private_write(path, json.dumps(views, indent=2) + "\n")
                invocation.update(scenario=label, sessions=str(path), outstanding_wait_at_interrupt=held_wait_ids, native_exit_expected=expected_native_exit(final, invocation["interrupted"]), states=list(dict.fromkeys(v["record"]["state"] for v in views)))
                result["runs"].append(invocation)
                return invocation, views, final

            def wait_args():
                pending = daemon.call(["pending", "--goal", daemon.goal])["pending"]
                return {"goal": daemon.goal, "seen": pending["revision"], "timeout_ms": args.timeout_ms}

            first, views, exited = run("launch", [step("locust_pending", {"goal": daemon.goal}),
                step("locust_attempt_start", {"goal": daemon.goal, "task": task, "offer": offer}),
                step("locust_wait", wait_args)], interrupt=True)
            detail = metadata(exited)
            ready_views = [v for v in views if v["record"]["state"] == "ready"]
            checks["ready"] = fixture.assertion("pass" if ready_views and all(ready_evidence(v, first["stderr"]) for v in ready_views) else "fail", "Durable Ready requires structured native identity and authenticated production MCP receipt")
            bound = detail.get("binding", {})
            exact = (bound.get("instance") == daemon.instance and bound.get("principal") == daemon.principal and
                     bound.get("goal") == daemon.goal and bound.get("attempt") == {"task": task, "attempt": attempt})
            checks["binding"] = fixture.assertion("pass" if exact else "fail", "Persisted exact principal/session/goal/task/assignment/attempt")
            pending_before = daemon.call(["pending", "--goal", daemon.goal])["pending"]
            claim = next((c for c in pending_before["claimed"] if c["attempt"] == attempt), None)
            checks["claim"] = fixture.assertion("pass" if claim and bound.get("claim") == claim and claim["generation"] > 0 else "fail", "Managed metadata matches independently authenticated claim and generation")
            notice = daemon.call(["client", "pending"])
            checks["ordinary_pending"] = fixture.assertion("pass" if claim and claim in notice["notice"]["pending"]["claimed"] and notice["task_ownership_changed"] is False and notice["cancellation_acknowledged"] is False else "fail", "Read-only pending fallback retains claimed work after client exit")
            checks["held_wait"] = fixture.assertion("pass" if first["interrupted"] and bool(first["outstanding_wait_at_interrupt"]) else "fail", "Structured native wait invocation preceded leader-only interruption")
            checks["interruption"] = fixture.assertion("pass" if first["interrupted"] and clean_run(first) and first["native_exit_expected"] else "fail", "Managed parent forwarded the local interrupt and durably observed native exit without forced cleanup")
            checks["exited"] = fixture.assertion("pass" if exited["record"]["state"] == "exited" and detail.get("process", {}).get("exited") else "fail", "Owned child exit is recorded; task completion is not inferred")
            cancel = daemon.call(["attempt", "cancel", "--goal", daemon.goal, "--attempt", attempt])["recorded"]["event"]
            daemon.restart()
            restarted = daemon.call(["client", "status"])["session"]
            waiting = daemon.call(["client", "pending"])["notice"]["pending"]
            checks["daemon_restart"] = fixture.assertion("pass" if restarted["record"] == exited["record"] else "fail", "SQLite retained exact exited managed record across restart")
            checks["cancellation_durable"] = fixture.assertion("pass" if any(c["cancel"] == cancel for c in waiting["to_acknowledge"]) else "fail", "Cancellation requested while client exited remains pending after daemon restart")
            native_id = exited["record"].get("client_session")
            if native_id and claim:
                resumed, resumed_views, finished = run("resume", [step("locust_pending", {"goal": daemon.goal}),
                    step("locust_cancel_acknowledge", {"goal": daemon.goal, "cancel": cancel,
                        "generation": claim["generation"], "outcome": "uncertain"})], resume=native_id)
                checks["explicit_resume"] = fixture.assertion("pass" if finished["record"].get("client_session") == native_id and clean_run(resumed) and resumed["native_exit_expected"]
                    and any(ready_evidence(v, resumed["stderr"]) for v in resumed_views) and metadata(finished).get("resume_from") == native_id else "fail", "Explicit local resume observed the same native session with a fresh managed launch")
                task_view = daemon.call(["task", "show", "--goal", daemon.goal, "--task", task])["task"]["view"]
                after = daemon.call(["pending", "--goal", daemon.goal])["pending"]
                result["task_after_resume"] = task_view
                cancel_detail = daemon.call(["event", "show", "--goal", daemon.goal, "--event", cancel])["event"]
                ack_details = []
                cursor = 0
                while True:
                    entries = daemon.call(["events", "--goal", daemon.goal, "--after", str(cursor), "--limit", "256"])["events"]
                    if not entries:
                        break
                    for entry in entries:
                        if entry["kind"] == "cancel_acknowledged":
                            ack_details.append(daemon.call(["event", "show", "--goal", daemon.goal, "--event", entry["event"]])["event"])
                    cursor = entries[-1]["position"]
                result["cancellation_evidence"] = {"requested": cancel_detail, "acknowledgments": ack_details}
                acknowledged = any(cancellation_acknowledged(cancel_detail, detail, after,
                    cancel=cancel, attempt=claim["attempt"], task=task, principal=daemon.principal) for detail in ack_details)
                checks["cancellation_acknowledged"] = fixture.assertion("pass" if acknowledged else "fail", "Actual model-client tool explicitly recorded uncertain executor outcome; notification alone did not acknowledge cancellation")
            restored = not config_file.exists() if baseline is None else config_file.read_bytes() == baseline
            checks["profile_restored"] = fixture.assertion("pass" if restored else "fail", "Selected profile MCP bytes restored after owned client exit")
            clean = all(clean_run(r) and r["native_exit_expected"] for r in result["runs"])
            checks["cleanup"] = fixture.assertion("pass" if clean and not provider.errors else "fail", "All owned sessions/receipt-identified bridges exited naturally; scripted provider reported no errors")
            result["provider_errors"] = provider.errors
            result["provider_requests"] = provider.requests
            result["capabilities"] = exited["record"]["capabilities"]
    except Exception as error:
        result["harness_error"] = type(error).__name__ + ": " + str(error)
        checks["cleanup"] = fixture.assertion("fail", "Harness did not complete; retain receipts")
    finally:
        profile.close()
        result["private_runtime_profile_removed"] = not profile.root.exists()
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--locust", required=True)
    parser.add_argument("--timeout-ms", required=True, type=int)
    parser.add_argument("--output", required=True, type=Path)
    for client in fixture.CLIENTS:
        parser.add_argument("--" + client)
    args = parser.parse_args(argv)
    if args.timeout_ms <= 0:
        parser.error("--timeout-ms must be positive")
    args.locust = fixture.checked_binary(args.locust)
    if not args.locust:
        parser.error("Locust binary is missing")
    args.output = args.output.resolve()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=True)
    report = {"schema": "locust-managed-client-qualification", "schema_version": 2,
              "created_at": datetime.now(timezone.utc).isoformat(), "timeout_ms": args.timeout_ms,
              "evidence_level": "actual-client/scripted-provider/managed-production-daemon",
              "scope": "One host and principal; explicit local permissive new/resume; no real provider, active hook, automatic wake or installed release",
              "clients": []}
    for client in fixture.CLIENTS:
        binary = fixture.checked_binary(getattr(args, client.replace("-", "_")))
        print(json.dumps({"client": client, "phase": "starting"}), flush=True)
        result = qualify(client, binary, args)
        report["clients"].append(result)
        print(json.dumps({"client": client, "phase": "complete", "assertions": {k: v["status"] for k, v in result["assertions"].items()}, "harness_error": result.get("harness_error")}), flush=True)
    path = args.output / "report.json"
    private_write(path, json.dumps(report, indent=2) + "\n")
    print(json.dumps({"report": str(path)}), flush=True)
    return int(any(v["status"] == "fail" for c in report["clients"] for v in c["assertions"].values()))


if __name__ == "__main__":
    sys.exit(main())
