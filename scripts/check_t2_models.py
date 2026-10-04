#!/usr/bin/env python3
"""Real-model mixed-client workflow against a pinned production Locust binary.

Owner setup/export/assignment is harness-authored. Worker and coordinator models
perform the target operations through actual MCP and native client tools. This
is local capability evidence under explicit permissive policy, never independent
participant/account, peer, default approval, managed-launch or release support.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import re
import shlex
import subprocess
import sys

import check_clients as fixture
from check_t2_clients import git, successful, record_result
from client_qualification.production import ProductionDaemon, ProductionError
from client_qualification.real_models import (PROVIDERS, RedactingProcess,
                                              configure_real_provider, install_locust_skill)
from client_qualification.runtime import Process, Profile, private_write, records


ROOT = Path(__file__).resolve().parents[1]
PAIRS = (("codex", "claude-code"), ("claude-code", "factory-droid"),
         ("factory-droid", "pi"), ("pi", "codex"))
BASE_CODE = '"""Small synthetic arithmetic module."""\n\ndef add(a, b):\n    return a - b\n'
FIXED_CODE = BASE_CODE.replace("a - b", "a + b")
TEST_CODE = '''import unittest
from calculator import add

class AddTests(unittest.TestCase):
    def test_positive(self): self.assertEqual(add(2, 3), 5)
    def test_negative(self): self.assertEqual(add(-2, -3), -5)
    def test_mixed(self): self.assertEqual(add(-2, 3), 1)
    def test_zero(self): self.assertEqual(add(7, 0), 7)
    def test_float(self): self.assertEqual(add(1.5, 2.5), 4.0)
'''
ASSERTIONS = ("worker_skill_read", "worker_mcp_inspection", "worker_scoped_claim", "worker_progress",
              "worker_materialize", "worker_fix_and_test", "worker_capture_review_submit",
              "coordinator_skill_read", "coordinator_event_review", "coordinator_diff_review",
              "selected_before_integrated", "coordinator_apply", "explicit_coordinator_resume", "fresh_coordinator_application",
              "final_files_and_tests", "unrelated_work_and_git_head", "daemon_result_integrity",
              "real_model_completion", "client_execution_and_cleanup", "independent_collaborators", "default_approval",
              "packaged_release")


def native_calls(client, stdout):
    """Extract completed native tool receipts, never echoed user prompt text."""
    result, pending = [], {}
    for event in records(stdout):
        kind = event.get("type")
        if client == "codex" and kind == "item.completed":
            item = event.get("item", {})
            if item.get("type") == "command_execution":
                result.append({"tool": "command_execution", "arguments": {"command": item.get("command", "")},
                               "success": item.get("exit_code") == 0, "output": item.get("aggregated_output", "")})
        elif client == "claude-code":
            for block in event.get("message", {}).get("content", []):
                if not isinstance(block, dict):
                    continue
                if kind == "assistant" and block.get("type") == "tool_use":
                    pending[block.get("id")] = {"tool": block.get("name"), "arguments": block.get("input", {})}
                elif kind == "user" and block.get("type") == "tool_result":
                    call = pending.pop(block.get("tool_use_id"), None)
                    if call:
                        result.append({**call, "success": not block.get("is_error", False),
                                       "output": block.get("content", "")})
        elif client == "factory-droid":
            if kind == "tool_call":
                pending[event.get("id")] = {"tool": event.get("toolId"), "arguments": event.get("parameters", {})}
            elif kind == "tool_result":
                call = pending.pop(event.get("id"), None)
                if call:
                    result.append({**call, "success": event.get("isError") is False, "output": event.get("value", "")})
        elif client == "pi":
            if kind == "tool_execution_start":
                pending[event.get("toolCallId")] = {"tool": event.get("toolName"), "arguments": event.get("args", {})}
            elif kind == "tool_execution_end":
                call = pending.pop(event.get("toolCallId"), None)
                if call:
                    result.append({**call, "success": event.get("isError") is False, "output": event.get("result", {})})
    return result


def shell_calls(calls):
    return [call for call in calls if call.get("tool") in ("command_execution", "Bash", "Execute", "bash")]


def native_operation(calls, operation, binary):
    pattern = r"\b" + r"\s+".join(map(re.escape, operation.split())) + r"\b"
    return any(call.get("success") is True and str(binary) in call.get("arguments", {}).get("command", "") and
               re.search(pattern, call.get("arguments", {}).get("command", "")) for call in shell_calls(calls))


def skill_read(calls, path):
    for call in calls:
        if call.get("success") is not True:
            continue
        arguments = call.get("arguments", {})
        output = json.dumps(call.get("output"), ensure_ascii=False)
        if (call.get("tool") in ("Read", "read") and
                (arguments.get("file_path") == str(path) or arguments.get("path") == str(path)) and
                "# Locust collaboration" in output):
            return True
        command = arguments.get("command", "")
        if call in shell_calls(calls) and str(path) in command and "# Locust collaboration" in output:
            return True
    return False


def check_tests(profile, source):
    result = subprocess.run([sys.executable, "-B", "-m", "unittest", "discover", "-s", ".", "-v"],
                            cwd=source, env=profile.environment(sys.executable), capture_output=True, text=True, check=False)
    return {"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr}


def model_completion(client, stdout):
    """Structured successful client completion/usage, not generated prose."""
    for event in records(stdout):
        if client == "codex" and event.get("type") == "turn.completed" and event.get("usage", {}).get("output_tokens", 0) > 0:
            return True
        if client == "claude-code" and event.get("type") == "result" and event.get("subtype") == "success" and event.get("is_error") is False:
            return event.get("usage", {}).get("output_tokens", 0) > 0
        if client == "factory-droid" and event.get("type") == "completion":
            return event.get("usage", {}).get("output_tokens", 0) > 0
        if client == "pi" and event.get("type") == "message_end":
            message = event.get("message", {})
            if message.get("role") == "assistant" and message.get("stopReason") == "stop" and message.get("usage", {}).get("output", 0) > 0:
                return True
    return False


def prepare_work(profile, daemon):
    """Harness-only committed baseline, sharing scope and owner authorization."""
    source = profile.workspace / "source"
    source.mkdir(mode=0o700)
    git(profile, source, ["init", "-q"])
    git(profile, source, ["config", "user.name", "Locust synthetic qualification"])
    git(profile, source, ["config", "user.email", "qualification@example.invalid"])
    private_write(source / "calculator.py", BASE_CODE)
    private_write(source / "test_calculator.py", TEST_CODE)
    git(profile, source, ["add", "calculator.py", "test_calculator.py"])
    git(profile, source, ["commit", "-qm", "synthetic arithmetic base"])
    commit = git(profile, source, ["rev-parse", "HEAD"])
    preview = daemon.call(["workspace", "preview", "--root", source, "--commit", commit])
    if set(item["path"] for item in preview["entries"]) != {"calculator.py", "test_calculator.py"}:
        raise ProductionError("Synthetic sharing scope differs from authored files")
    base = daemon.call(["workspace", "export", "--goal", daemon.goal, "--root", source, "--commit", commit])["manifest"]
    private_write(source / "unrelated.txt", "preserve unrelated local work\n")
    task = "task:" + daemon.call(["task", "open", "--goal", daemon.goal, "--inputs", json.dumps({"snapshot": base}),
                       "Fix add(a,b) in calculator.py so the supplied five tests pass. Change only calculator.py; inspect received files before executing tests."])["recorded"]["event"]
    offer = daemon.call(["work", "offer", "--goal", daemon.goal, "--task", task,
                             "--recipient", daemon.principal])["recorded"]["event"]
    daemon.call(["task", "authorize", "--goal", daemon.goal, "--task", task, "--agent", daemon.principal], owner=True)
    baseline = check_tests(profile, source)
    if baseline["exit_code"] == 0:
        raise ProductionError("Synthetic baseline unexpectedly passes")
    return {"source": str(source), "commit": commit, "base": base, "task": task,
            "offer": offer, "goal": daemon.goal, "principal": daemon.principal,
            "preview": preview, "baseline_tests": baseline}


def model_context(daemon, role, work, binary):
    prefix = [binary, "--home", str(daemon.home), "--credential", str(daemon.credential),
              "--session", str(role["session"]), "--json"]
    return f'''You are the {role['role']} in an explicitly authorized LOCAL synthetic Locust qualification.
Use the Locust operating skill: FIRST read {role['skill']['path']} with your native read tool or cat.
Use actual registered Locust MCP tools for status/task/claim operations. If tools are deferred, discover them.
Goal: {work['goal']}; task: {work['task']}; offer: {work['offer']}; input manifest: {work['base']}.
Use this exact scoped CLI prefix for filesystem commands: {shlex.join(prefix)}
The API credential and session paths are capabilities: pass them to Locust only. Never read, print, upload or copy their bytes.
Never inspect process environments (including ps eww or /proc), print environment variables, or search for credentials. If a named path is unavailable, stop and report it.
Do not access owner credentials, other user profiles, or files outside the installed skill, assigned synthetic workspace and named Locust CLI paths.
The harness already created the committed synthetic snapshot, exported it, opened/offered the task and authorized execution for this agent and task.
The user authorizes inspection, execution of the supplied tiny tests, this scoped edit, sharing the exact patch, review, acceptance and local application.
Issue actual native tool calls and individual Locust CLI commands. Do not merely describe commands, and do not create a driver script to run the workflow.
Preserve tests, unrelated local work and original Git HEAD. Do not commit, stage, reset or push.
'''


def worker_prompt(daemon, role, work, binary):
    return model_context(daemon, role, work, binary) + f'''
1. Call locust_status, locust_goal_status, locust_task_show and locust_pending via MCP. Inspect goal membership/halted state and exact task input/assignment.
2. Start this task and exact offer with locust_attempt_start. Retain returned attempt, generation and instance; call locust_attempt_report with status progress using that attempt and generation.
3. Run the scoped CLI workspace materialize --goal {work['goal']} --manifest {work['base']} --destination {work['destination']}; this directory must be new.
4. Read calculator.py and test_calculator.py in that directory before executing them. Diagnose the bug, run the supplied tests, correct only calculator.py, and run {shlex.quote(sys.executable)} -B -m unittest discover -s . -v there until all five tests pass.
5. Capture only calculator.py with CLI patch create --goal {work['goal']} --base {work['base']} --root {work['destination']} --path calculator.py.
6. Review the returned exact patch using CLI patch review, then CLI patch submit --goal GOAL --patch PATCH --attempt RETURNED_ATTEMPT --generation RETURNED_GENERATION 'Fixed arithmetic; five supplied tests pass'. Use actual returned patch/head identifiers.
7. Stop after submission. Do not accept or apply. Briefly report the result/patch/head/generation and test outcome, without capability bytes.
'''


def review_prompt(daemon, role, work, binary):
    return model_context(daemon, role, work, binary) + f'''
You are a separate coordinator client reviewing a real worker's contribution.
Result: {work['result']}; patch: {work['patch']}; claimed head: {work['head']}.
1. Inspect locust_status, locust_goal_status, locust_task_show and locust_event_show (event={work['result']}) through MCP. Confirm this exact task/assignment/base/patch/head result.
2. Independently run CLI patch review --goal {work['goal']} --patch {work['patch']}; read and judge the actual diff. Require only calculator.py changed and the fix matches the test requirements. The worker summary is not independent test proof.
3. Inspect original source {work['source']}: calculator.py must still have the bug, test_calculator.py unchanged, unrelated.txt preserved, and Git HEAD exactly {work['commit']}. Do not execute code until inspected.
4. If the exact contribution is correct, first record locust_review_record with verdict approve and an evidence-based explanation for its subject, then run CLI patch select --goal {work['goal']} --subject {work['result']}.
5. Check locust_task_show after selection; selected must equal {work['result']}. Check locust_goal_status: workspace.integrated must still identify base {work['base']}. Confirm original calculator.py unchanged.
6. STOP BEFORE APPLYING. Do not edit or apply files in this turn. Report this observed acceptance/application boundary.
'''


def apply_prompt(daemon, role, work, binary):
    return model_context(daemon, role, work, binary) + f'''Continue the same authorized coordinator session. The harness independently verified acceptance of patch {work['patch']}, accepted head {work['head']}, no integration and unchanged original files/HEAD/WIP.
Use the exact scoped CLI prefix supplied again above; never infer credential/session paths or inspect process environments.
Read locust_goal_status and task via MCP to reconcile current state. Apply exactly this accepted contribution with CLI patch apply --goal {work['goal']} --subject {work['result']} --root {work['source']} --expected-git-head {work['commit']}.
Inspect resulting calculator.py and unchanged test_calculator.py, then run {shlex.quote(sys.executable)} -B -m unittest discover -s . -v in {work['source']}. Verify unrelated.txt still says "preserve unrelated local work" and Git HEAD remains {work['commit']}.
Read locust_goal_status again and confirm workspace.integrated equals accepted head {work['head']}. Do not stage, commit, reset or push. Report actual outcomes.
'''


def execute(role, label, prompt, daemon, args, resume=None):
    profile, client = role["profile"], role["client"]
    events_path, ready_path = profile.logs / (label + ".mcp.jsonl"), profile.logs / (label + ".bridge.jsonl")
    private_write(events_path, b"")
    private_write(ready_path, b"")
    observer = [str(ROOT / "scripts/client_qualification/observe_mcp.py"), "--events-file", str(events_path),
                "--executable", args.locust, "--argument=mcp", "--argument=--lifecycle-receipt", "--argument=" + str(ready_path)]
    config = [args.config_probe, "--client", client, "--executable", sys.executable,
              "--locust-home", str(daemon.home), "--credential", str(daemon.credential),
              "--session", str(role["session"]), *["--argument=" + value for value in observer]]
    proposal = json.loads(subprocess.run(config, env=profile.environment(role["binary"]), cwd=profile.workspace,
                                        capture_output=True, timeout=args.timeout_ms / 1000, check=True).stdout)
    profile.apply(proposal)
    argv = role["provider"].invocation(prompt, proposal["arguments"], resume=resume, policy="deliberately-permissive")
    private_write(profile.logs / (label + ".prompt.txt"), prompt)
    process = RedactingProcess(argv, role["provider"].environment, profile.workspace, profile.logs, label,
                               args.timeout_ms / 1000, [PROVIDERS[role["provider"].provider][0]])

    def observe(child):
        for event in records(events_path):
            if event.get("event") == "bridge_started":
                child.register_child(event.get("pid"), args.locust, ready_path)
                child.register_child(event.get("observer_pid"), sys.executable, events_path)

    try:
        run = process.wait(observe=observe)
    finally:
        process.close()
    run.update(role=role["role"], client=client, scenario=label, mcp_events=str(events_path),
               lifecycle_receipts=str(ready_path), configuration=proposal,
               policy="Pi default; no permission extension" if client == "pi" else "deliberately-permissive",
               native_calls=native_calls(client, run["stdout"]))
    role["runs"].append(run)
    return run, records(events_path, strict=True)


def original_preserved(profile, work, code=BASE_CODE):
    source = Path(work["source"])
    return ((source / "calculator.py").read_text() == code and
            (source / "test_calculator.py").read_text() == TEST_CODE and
            (source / "unrelated.txt").read_text() == "preserve unrelated local work\n" and
            git(profile, source, ["rev-parse", "HEAD"]) == work["commit"])


def native_session_identifier(role, run):
    if role["client"] == "pi":
        return next((event.get("id") for event in records(role["provider"].metadata["session_file"], strict=True)
                     if event.get("type") == "session"), None)
    return fixture.session_identifier(role["client"], run, role["profile"])


def qualify_pair(coordinator, worker, binaries, args):
    name = coordinator + "--" + worker
    result = {"coordinator": coordinator, "worker": worker, "roles": [],
              "assertions": {key: fixture.assertion("not_run", "Scenario not executed") for key in ASSERTIONS}}
    checks = result["assertions"]
    for key, reason in (("independent_collaborators", "One local host and enrolled principal, separate protected sessions; no independent people/accounts or peer transport"),
                        ("default_approval", "Explicit permissive model scenario, not default or interactive approval"),
                        ("packaged_release", "Pinned local debug artifact, no installed signed package")):
        checks[key] = fixture.assertion("not_run", reason)
    if not binaries.get(coordinator) or not binaries.get(worker):
        result["harness_error"] = "Required client executable missing"
        return result
    setup = Profile(args.output, name + "-setup")
    profiles = [setup]
    roles = []
    try:
        with ProductionDaemon(setup, args.locust, args.timeout_ms / 1000) as daemon:
            result.update(locust_artifact=daemon.binary_metadata, daemon_receipts=str(daemon.events),
                          daemon_endpoint=daemon.endpoint, enrolled_principal=daemon.principal)
            work = prepare_work(setup, daemon)
            for client, kind in ((worker, "worker"), (coordinator, "coordinator")):
                profile = Profile(args.output, name + "-" + kind)
                profiles.append(profile)
                session = daemon.session if kind == "coordinator" else profile.session
                instance = daemon.instance if kind == "coordinator" else daemon.call(["session", "create", session], owner=True)["instance"]
                selected_provider = args.droid_provider if client == "factory-droid" else ("anthropic" if client == "claude-code" else "openai")
                model = args.anthropic_model if selected_provider == "anthropic" else args.openai_model
                provider = configure_real_provider(client, profile, binaries[client], model, provider=selected_provider)
                skill = install_locust_skill(client, profile, ROOT / "skills/locust/SKILL.md")
                role = {"role": kind, "client": client, "binary": binaries[client], "profile": profile,
                        "session": session, "instance": instance, "provider": provider, "skill": skill, "runs": []}
                roles.append(role)
                version = subprocess.run([binaries[client], "--version"], env=profile.environment(binaries[client]),
                                         cwd=profile.workspace, capture_output=True, text=True,
                                         timeout=args.timeout_ms / 1000, check=True).stdout.strip()
                result["roles"].append({"role": kind, "client": client, "model": model,
                    "instance": instance, "provider": provider.metadata, "skill": skill, "runs": role["runs"],
                    "version": version, "binary_sha256": hashlib.sha256(Path(binaries[client]).read_bytes()).hexdigest()})
            worker_role, coord_role = roles
            work["destination"] = str(worker_role["profile"].workspace / "materialized")
            result["work"] = work
            run, events = execute(worker_role, "worker", worker_prompt(daemon, worker_role, work, args.locust), daemon, args)
            calls = run["native_calls"]
            checks["worker_skill_read"] = fixture.assertion("pass" if skill_read(calls, worker_role["skill"]["path"]) else "fail", "Completed native manifest read required")
            inspection = all(successful(events, tool) for tool in ("locust_status", "locust_goal_status", "locust_task_show", "locust_pending"))
            checks["worker_mcp_inspection"] = fixture.assertion("pass" if inspection else "fail", "Successful matched production MCP status/goal/task/pending responses required")
            claimed = [record_result(e).get("claimed", {}) for e in successful(events, "locust_attempt_start")]
            claims = [claim for claim in claimed if claim.get("task") == work["task"] and
                      claim.get("instance") == worker_role["instance"] and isinstance(claim.get("generation"), int)]
            checks["worker_scoped_claim"] = fixture.assertion("pass" if claims else "fail", "Exact authorized assignment, worker protected session instance and generation")
            checks["worker_progress"] = fixture.assertion("pass" if successful(events, "locust_attempt_report") else "fail", "Actual worker MCP progress call completed")
            task = daemon.call(["task", "show", "--goal", daemon.goal, "--task", work["task"]])["task"]
            work["attempt"] = claims[0]["attempt"] if claims else None
            contributions = task["view"]["contributions"]
            work["result"] = contributions[-1] if len(contributions) == 1 else None
            if not work["result"]:
                raise ProductionError("Worker did not submit a production task result")
            submitted = daemon.call(["event", "show", "--goal", daemon.goal, "--event", work["result"]])["event"]
            result["worker_result_event"] = submitted
            result_fields = submitted.get("body", {}).get("contribution_published", {})
            work["patch"] = result_fields.get("patch")
            if not work["patch"]:
                raise ProductionError("Submitted event did not expose exact contribution patch")
            review = daemon.call(["patch", "review", "--goal", daemon.goal, "--patch", work["patch"]])
            work["head"] = review["head"]
            result["independent_worker_patch_review"] = review
            destination = Path(work["destination"])
            tests = check_tests(worker_role["profile"], destination)
            result["independent_worker_tests"] = tests
            checks["worker_materialize"] = fixture.assertion("pass" if native_operation(calls, "workspace materialize", args.locust) and destination.is_dir() else "fail", "Real worker native CLI materialized exact input to new path")
            test_command = any(call["success"] and "unittest" in call.get("arguments", {}).get("command", "") for call in shell_calls(calls))
            fixed = tests["exit_code"] == 0 and (destination / "calculator.py").read_text() == FIXED_CODE and (destination / "test_calculator.py").read_text() == TEST_CODE
            checks["worker_fix_and_test"] = fixture.assertion("pass" if fixed and test_command else "fail", "Native worker test command and independent exact file/test verification required")
            operations = all(native_operation(calls, op, args.locust) for op in ("patch create", "patch review", "patch submit"))
            checks["worker_capture_review_submit"] = fixture.assertion("pass" if operations and review["base"] == work["base"] and len(review["changes"]) == 1 and review["changes"][0]["path"] == "calculator.py" else "fail", "Actual native CLI capture/review/submit and independently authenticated exact single-file delta")
            if not original_preserved(setup, work):
                raise ProductionError("Worker changed original coordinator source, tests, HEAD or unrelated work")
            before = daemon.call(["goal", "status", "--goal", daemon.goal])["goal_status"]
            if task["view"]["selected"] is not None or before["workspace"]["integrated"] != work["base"]:
                raise ProductionError("Worker accepted or integrated before coordinator review")
            crun, cevents = execute(coord_role, "coordinator-review", review_prompt(daemon, coord_role, work, args.locust), daemon, args)
            ccalls = crun["native_calls"]
            checks["coordinator_skill_read"] = fixture.assertion("pass" if skill_read(ccalls, coord_role["skill"]["path"]) else "fail", "Separate coordinator client completed native skill-manifest read")
            checks["coordinator_event_review"] = fixture.assertion("pass" if any(record_result(e).get("event", {}).get("view", {}).get("event") == work["result"] for e in successful(cevents, "locust_event_show")) else "fail", "Coordinator actually read exact worker event through MCP")
            checks["coordinator_diff_review"] = fixture.assertion("pass" if native_operation(ccalls, "patch review", args.locust) and native_operation(ccalls, "patch select", args.locust) else "fail", "Separate coordinator native tools reviewed authenticated diff and accepted exact result")
            accepted = daemon.call(["goal", "status", "--goal", daemon.goal])["goal_status"]
            selected = daemon.call(["task", "show", "--goal", daemon.goal, "--task", work["task"]])["task"]["view"]["selected"]
            boundary = selected == work["result"] and accepted["workspace"]["integrated"] == work["base"] and original_preserved(setup, work)
            checks["selected_before_integrated"] = fixture.assertion("pass" if boundary else "fail", "Independent harness after review confirms accepted exact head, null integration and unchanged original source/HEAD/WIP")
            result["accepted_boundary"] = accepted
            if not boundary:
                raise ProductionError("Coordinator acceptance/application boundary not established")
            session_id = native_session_identifier(coord_role, crun)
            if not session_id:
                raise ProductionError("Coordinator did not expose native session identifier")
            resume = coord_role["provider"].metadata["session_file"] if coordinator == "pi" else session_id
            before_history = Path(resume).read_bytes() if coordinator == "pi" else None
            result["native_coordinator_session_before_resume"] = session_id
            arun, aevents = execute(coord_role, "coordinator-apply", apply_prompt(daemon, coord_role, work, args.locust), daemon, args, resume=resume)
            result["native_coordinator_session_after_resume"] = native_session_identifier(coord_role, arun)
            preserved_history = (before_history is None or (Path(resume).read_bytes().startswith(before_history) and
                                 len(Path(resume).read_bytes()) > len(before_history)))
            checks["explicit_coordinator_resume"] = fixture.assertion("pass" if native_session_identifier(coord_role, arun) == session_id and successful(aevents, "locust_goal_status") and preserved_history else "fail", "Exact native coordinator ID, new bridge with same scoped protected session; Pi history must extend original bytes")
            if coordinator == "factory-droid" and arun["exit_code"] != 0 and args.continue_after_resume_failure:
                if not original_preserved(setup, work):
                    raise ProductionError("Failed resume changed the coordinator's source")
                result["native_resume_failure"] = {"exit_code": arun["exit_code"], "stdout_bytes": Path(arun["stdout"]).stat().st_size,
                                                   "stderr_bytes": Path(arun["stderr"]).stat().st_size}
                arun, aevents = execute(coord_role, "coordinator-fresh-apply", apply_prompt(daemon, coord_role, work, args.locust), daemon, args)
                fresh_id = native_session_identifier(coord_role, arun)
                result["fresh_coordinator_native_session"] = fresh_id
                checks["fresh_coordinator_application"] = fixture.assertion("pass" if fresh_id and fresh_id != session_id and
                    native_operation(arun["native_calls"], "patch apply", args.locust) else "fail", "Explicitly fresh native coordinator session reconciles scoped daemon state and applies; native resume failure retained")
            checks["coordinator_apply"] = fixture.assertion("pass" if native_operation(arun["native_calls"], "patch apply", args.locust) else "fail", "Actual resumed coordinator native CLI applied accepted exact patch")
            final = daemon.call(["goal", "status", "--goal", daemon.goal])["goal_status"]
            task_final = daemon.call(["task", "show", "--goal", daemon.goal, "--task", work["task"]])["task"]
            result.update(final_goal_status=final, final_task=task_final, independent_final_tests=check_tests(setup, Path(work["source"])))
            checks["final_files_and_tests"] = fixture.assertion("pass" if original_preserved(setup, work, FIXED_CODE) and result["independent_final_tests"]["exit_code"] == 0 else "fail", "Harness independently checks exact repaired source, unchanged five-test suite and all five tests passing")
            checks["unrelated_work_and_git_head"] = fixture.assertion("pass" if original_preserved(setup, work, FIXED_CODE) else "fail", "Original Git HEAD and unrelated untracked file preserved")
            integrity = (work["head"] == final["workspace"]["integrated"] and
                         task_final["view"]["completed"] is True and
                         task_final["view"]["selected"] == work["result"] and submitted["view"]["author"] == daemon.principal and
                         result_fields.get("attempt") == work["attempt"] and result_fields.get("base") == work["base"] and
                         work["head"] in result_fields.get("artifacts", []))
            checks["daemon_result_integrity"] = fixture.assertion("pass" if integrity else "fail", "Independent authenticated daemon task/event/base/patch/head/integration identities agree")
    except Exception as error:
        result["harness_error"] = type(error).__name__ + ": " + str(error)
        checks["client_execution_and_cleanup"] = fixture.assertion("fail", "Workflow failed; no prose-only success")
    finally:
        runs = [run for role in roles for run in role["runs"]]
        target_runs = [run for run in runs if run["scenario"] != "coordinator-apply" or len(runs) == 3]
        checks["real_model_completion"] = fixture.assertion("pass" if len(target_runs) == 3 and all(model_completion(run["client"], run["stdout"]) for run in target_runs) else "fail", "Direct selected real-provider profiles; each target worker/review/application turn has structured successful model completion and output usage; failed resume attempts remain separately recorded")
        good = len(runs) == 3 and all(run["exit_code"] == 0 and not run["timed_out"] and
                    run["cleanup_verified"] and run["natural_cleanup"] and not run["forced_cleanup"] for run in runs)
        if "harness_error" not in result:
            checks["client_execution_and_cleanup"] = fixture.assertion("pass" if good else "fail", "Three real-model turns completed with natural owned-process/receipt-identified bridge cleanup")
        for profile in reversed(profiles):
            profile.close()
        result["private_runtime_profiles_removed"] = all(not profile.root.exists() for profile in profiles)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout-ms", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--locust", required=True)
    parser.add_argument("--config-probe", required=True)
    parser.add_argument("--openai-model", required=True, help="Explicit current provider-supported model ID")
    parser.add_argument("--anthropic-model", required=True, help="Explicit current provider-supported model ID")
    parser.add_argument("--droid-provider", choices=("openai", "anthropic"), default="openai",
                        help="Explicit Droid BYOK provider; a fallback is separately labeled evidence")
    parser.add_argument("--continue-after-resume-failure", action="store_true",
                        help="Explicit fresh Droid coordinator native turn after failed resume; preserve resume failure")
    parser.add_argument("--pair", action="append", choices=[a + ":" + b for a, b in PAIRS])
    for client in fixture.CLIENTS:
        parser.add_argument("--" + client)
    args = parser.parse_args(argv)
    if args.timeout_ms <= 0:
        parser.error("--timeout-ms must be positive")
    args.output = args.output.resolve()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=True)
    for name in ("locust", "config_probe"):
        value = fixture.checked_binary(getattr(args, name))
        if not value:
            parser.error(name + " executable unavailable")
        setattr(args, name, value)
    binaries = {client: fixture.checked_binary(getattr(args, client.replace("-", "_"))) for client in fixture.CLIENTS}
    pairs = [tuple(pair.split(":")) for pair in args.pair] if args.pair else PAIRS
    report = {"schema": "locust-t2-real-model-qualification", "schema_version": 1,
              "created_at": datetime.now(timezone.utc).isoformat(), "timeout_ms": args.timeout_ms,
              "evidence_level": "actual-clients/real-providers/production-daemon/local-synthetic-code",
              "scope": "One host, same principal, separate protected sessions; explicitly permissive model runs. Not independent-collaborator, peer, default approval, managed-launch or packaged-release qualification.",
              "setup_author": "harness: committed baseline, preview/export, propose/assign, owner authorization",
              "target_authors": "actual real-model worker/coordinator native/MCP tools; no authored target-operation driver",
              "platform": {"os": platform.system(), "release": platform.release(), "architecture": platform.machine()},
              "artifacts": [{"path": value, "sha256": hashlib.sha256(Path(value).read_bytes()).hexdigest()} for value in (args.locust, args.config_probe)],
              "pairs": []}
    source = Path(__file__)
    report["harness_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
    for coordinator, worker in pairs:
        print(json.dumps({"coordinator": coordinator, "worker": worker, "phase": "starting"}), flush=True)
        result = qualify_pair(coordinator, worker, binaries, args)
        report["pairs"].append(result)
        print(json.dumps({"coordinator": coordinator, "worker": worker, "phase": "complete",
                          "assertions": {key: value["status"] for key, value in result["assertions"].items()},
                          "harness_error": result.get("harness_error")}), flush=True)
        private_write(args.output / "report.partial.json", json.dumps(report, indent=2) + "\n")
    path = args.output / "report.json"
    if path.exists():
        prior = path.read_bytes()
        private_write(args.output / ("report-" + hashlib.sha256(prior).hexdigest() + ".json"), prior)
    private_write(path, json.dumps(report, indent=2) + "\n")
    print(json.dumps({"report": str(path)}), flush=True)
    return int(any(check["status"] == "fail" for pair in report["pairs"] for check in pair["assertions"].values()))


if __name__ == "__main__":
    sys.exit(main())
