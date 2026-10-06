#!/usr/bin/env python3
"""Production-daemon operational workflows on private same-host replicas.

The default workflow uses three replicas, including interrupted large transfers.
--workflow workspace runs two replicas through ordinary-directory workspace work,
peer readback and both SQLite restarts; it does not qualify partial transfers.

Uses only synthetic files, isolated homes and the explicitly identified binary.
Partial-transfer interruption is observed from durable staging files before
SIGKILL; a run without an observed partial object fails rather than claiming
resume evidence. Deadlines belong to this test harness, never daemon policy.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
import signal
import shutil
from pathlib import Path
import subprocess
import threading
import time

from check_t1 import CheckFailure, Qualification, identity, variant


def require(condition, message):
    if not condition:
        raise CheckFailure(message)


def require_unchanged_checkout_binding(before, after, revision, manifest):
    require(before.get("base_revision") == revision and before.get("base_manifest") == manifest
            and before.get("active_operation") is None,
            "checkout binding did not identify the unchanged accepted base")
    require(after == before, "conflicting update changed checkout binding")


def operation_event(operation):
    receipt = variant(variant(operation, "state"), "recorded")
    require(isinstance(receipt, dict), "workspace receipt was not a recorded event")
    return identity(receipt.get("event"), "workspace receipt")


def directory_files(root):
    return {str(path.relative_to(root)): (path.read_bytes(), path.stat().st_mode & 0o777)
            for path in root.rglob("*") if path.is_file()}


class Operations(Qualification):
    def __init__(self, binary, timeout, artifact_dir, network="default"):
        super().__init__(binary, timeout, artifact_dir, network=network)
        self.summary["qualification"] = "three production daemons on one host; synthetic operational workflows"
        self.summary["cases"] = {}
        self.summary["resources"] = {"verdict": "measurements only; no performance thresholds",
            "rss_scope": "receiver paused at observed partial transfer and after completed resume; sampled, not maximum",
            "network_bytes": "unmeasured", "cpu": "unmeasured"}
        self.summary["harness_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()

    def api(self, machine, request_name, **fields):
        return self.cli(machine, ["call", request_name, json.dumps(fields)])

    def expect_error(self, machine, args, code, **options):
        require(self.cli(machine, args, expected_errors=(code,), **options) is None,
                f"operation unexpectedly succeeded; expected {code}")

    def passed(self, case, **evidence):
        self.summary["cases"][case] = {"status": "passed", **evidence}
        self.record("operation_case_passed", case=case, **evidence)

    def event(self, machine, goal, event):
        result = self.cli(machine, ["event", "show", "--goal", goal, "--event", event],
                          expected_errors=("not_found",))
        return variant(result, "event") if result else None

    def workspace(self, machine, goal):
        return self.cli(machine, ["workspace", "head", "--goal", goal])

    def complete_workspace(self, machine, goal, revision):
        workspace = self.workspace(machine, goal)
        return workspace if (workspace.get("authority") == "ready"
            and (workspace.get("head") or {}).get("revision") == revision
            and isinstance(workspace.get("content"), dict)
            and "complete" in workspace["content"]) else None

    def checkout(self, machine, goal, revision, destination):
        self.wait(f"M{machine.number} retains the complete accepted workspace",
                  lambda: self.complete_workspace(machine, goal, revision))
        return variant(self.cli(machine, ["--agent", f"m{machine.number}", "workspace", "connect", "--goal", goal,
            "--revision", revision, "--folder", destination], owner=True), "checkout")

    def checkout_binding(self, machine, goal, checkout):
        bindings = variant(self.api(machine, "checkouts", goal=goal), "checkouts")
        found = [binding for binding in bindings if binding["id"] == checkout]
        require(len(found) == 1, "exact checkout binding was not retained")
        return found[0]

    def start_unchecked(self, machine):
        machine.generation += 1
        process = subprocess.Popen([str(self.binary), "--home", str(machine.home), "daemon", "run"],
                                   env=self.environment(machine), stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        machine.process = process
        machine.reader = threading.Thread(target=self.read_stderr,
                                          args=(machine, process, machine.generation), daemon=True)
        machine.reader.start()

    def sample_rss(self, machine, checkpoint):
        require(machine.process is not None, "resource sample has no owned daemon")
        result = subprocess.run(["/bin/ps", "-p", str(machine.process.pid), "-o", "rss="],
            capture_output=True, text=True, timeout=self.timeout, env=self.environment(machine))
        require(result.returncode == 0 and result.stdout.strip().isdigit(), "owned daemon RSS sample unavailable")
        value = int(result.stdout.strip()) * 1024
        require(value > 0, "owned daemon RSS sample must be positive")
        sample = {"checkpoint": checkpoint, "resident_bytes": value, "sampled_not_maximum": True}
        self.record("resource_sample", machine=machine.number, **sample)
        return sample

    def interrupt_transfer(self, machine, trigger, minimum_size):
        """Kill only this harness's receiver after an actual partial object exists."""
        observed = {}
        stop = threading.Event()

        def observe():
            while not stop.is_set():
                for path in (machine.home / "blobs").glob("*.staged"):
                    try:
                        size = path.stat().st_size
                    except FileNotFoundError:
                        continue
                    if 1024 * 1024 <= size < minimum_size and machine.process is not None:
                        process = machine.process
                        process.send_signal(signal.SIGSTOP)
                        try:
                            observed.update(hash=path.stem, prefix_bytes=size,
                                rss=self.sample_rss(machine, "paused_at_partial_transfer"))
                        except Exception as error:
                            observed["sampling_error"] = str(error)
                        finally:
                            process.kill()
                        return
                stop.wait(0.001)

        watcher = threading.Thread(target=observe, daemon=True)
        watcher.start()
        try:
            trigger()
            watcher.join(self.timeout)
        finally:
            stop.set()
            watcher.join()
        require(bool(observed), "no interrupted partial transfer observed")
        machine.process.wait(timeout=self.timeout)
        machine.reader.join(timeout=self.timeout)
        machine.process.stderr.close()
        machine.process = None
        require("sampling_error" not in observed, "partial transfer resource sample failed: " + observed.get("sampling_error", ""))
        staged = machine.home / "blobs" / (observed["hash"] + ".staged")
        require(staged.is_file(), "interrupted durable staging was lost")
        observed["durable_prefix_bytes"] = staged.stat().st_size
        require(1024 * 1024 <= observed["durable_prefix_bytes"] < minimum_size,
                "receiver finished the object before interruption")
        observed["prefix_sha256"] = hashlib.sha256(staged.read_bytes()).hexdigest()
        self.record("transfer_interrupted", machine=machine.number, **observed)
        return observed

    def flow(self):
        lead, first, second = self.machines
        root = lead.home.parent
        for machine in self.machines:
            self.start(machine)
            machine.agent = identity(variant(self.cli(machine, ["agent", "enroll", f"m{machine.number}"],
                                      owner=True), "agent_enrolled")["agent"], "principal")
        goal = self.create_goal(lead, "Synthetic operations")
        self.summary["goal"] = goal
        for machine in (first, second):
            ticket = variant(self.cli(lead, ["goal", "invite", "--goal", goal], owner=True), "invited")["ticket"]
            self.join_goal(machine, ticket)
        del ticket
        self.wait("three members converge", lambda: all(
            (state := self.goal_status(m, goal)) and len(state["members"]) == 3 for m in self.machines))
        self.set_ask_levels(goal)
        self.wait("workers establish their independent peer link", lambda:
                  any(p["endpoint"] == second.endpoint and p["connected"]
                      for p in self.goal_status(first, goal)["peers"]))

        self.phase = "documents"
        revisions = [variant(self.api(m, "doc.revise", goal=goal, doc="plan", base=None,
                                      text=f"proposal {m.number}"), "recorded")["event"]
                     for m in (first, second)]
        self.wait("both document proposals arrive", lambda: all(
            (detail := self.event(lead, goal, r)) and detail["text"] == f"proposal {m.number}"
            for m, r in zip((first, second), revisions)))
        self.api(lead, "review.record", goal=goal, subject=revisions[0], verdict="approve", text="Reviewed document proposal")
        self.api(lead, "scope.select", goal=goal, subject=revisions[0], expected=None)
        self.expect_error(lead, ["call", "scope.select", json.dumps({"goal": goal, "subject": revisions[1], "expected": None})], "conflict")
        require(self.event(lead, goal, revisions[1])["text"] == "proposal 3", "stale revision evidence disappeared")
        self.passed("conflicting_documents", accepted=revisions[0], retained_stale=revisions[1])

        self.phase = "workspace_seed"
        source = root / "seed-input"
        source.mkdir()
        # One file is much larger than the production 1 MiB transfer chunk.
        content = bytes(range(256)) * (48 * 1024)
        (source / "large.bin").write_bytes(content)
        (source / "code.txt").write_text("base\n")
        (source / "notes.txt").write_text("base notes\n")
        self.stop(second)
        captured = self.cli(lead, ["workspace", "init", "--goal", goal, "--root", source,
            "--path", "large.bin", "--path", "code.txt", "--path", "notes.txt", "--completion", json.dumps({"kind": "declaration", "by": {"kind": "contribution_author"}}), "--publish"], owner=True)
        seed_proposal = operation_event(captured["operation"])
        base = captured["candidate"]["result_manifest"]
        epoch = captured["candidate"]["context"]["round"]
        require(not (source / ".git").exists(), "ordinary seed unexpectedly acquired Git metadata")
        seed_receipt = variant(self.cli(lead, ["workspace", "integrate", "--goal", goal,
            "--proposal", seed_proposal, "--expected-empty", "--expected-epoch", epoch]), "workspace_operation")
        seed_revision = operation_event(seed_receipt)
        task = "task:" + self.recorded(lead, ["task", "open", "--goal", goal,
            "--inputs", json.dumps({"snapshot": base}), "Competing workspace proposals"])
        destinations = [root / "worker2", root / "worker3"]
        checkouts = [self.checkout(first, goal, seed_revision, destinations[0])]
        require((destinations[0] / "large.bin").read_bytes() == content, "first checkout bytes differ")
        self.restart(first)
        self.stop(lead)
        interrupted = self.interrupt_transfer(second, lambda: self.start_unchecked(second), len(content))
        self.start(second)
        checkouts.append(self.checkout(second, goal, seed_revision, destinations[1]))
        require((destinations[1] / "large.bin").read_bytes() == content, "resumed checkout bytes differ")
        require(not (second.home / "blobs" / (interrupted["hash"] + ".staged")).exists(), "completed stage was not promoted")
        require(all(not (destination / ".git").exists() for destination in destinations),
                "ordinary checkout unexpectedly acquired Git metadata")
        self.passed("snapshot_resume_retained_replica", original_source_offline=True,
                    revision=seed_revision, manifest=base, git_required=False,
                    bytes=len(content), sha256=hashlib.sha256(content).hexdigest(), interruption=interrupted,
                    resumed_rss=self.sample_rss(second, "snapshot_resume_complete"))
        self.start(lead)
        accepted_directory = root / "integrator"
        accepted_checkout = self.checkout(lead, goal, seed_revision, accepted_directory)

        self.phase = "competing_workspace_proposals"
        captures, results, claims = [], [], []
        for machine, checkout, destination in zip((first, second), checkouts, destinations):
            offer = self.recorded(lead, ["work", "offer", "--goal", goal, "--task", task, "--member", machine.agent])
            self.wait(f"worker {machine.number} receives assignment", lambda m=machine, a=offer: self.event(m, goal, a))
            self.cli(machine, ["--agent", f"m{machine.number}", "allow", "--goal", goal, "--task", task], owner=True)
            session = machine.home / "sessions" / "operations.secret"
            self.cli(machine, ["session", "create", session], local=True)
            claim = variant(self.cli(machine, ["attempt", "start", "--goal", goal, "--task", task, "--offer", offer], session=session), "claimed")
            claims.append((claim["attempt"], session, claim["generation"]))
            (destination / "code.txt").write_text(f"worker {machine.number}\n")
            (destination / "large.bin").write_bytes(content[:-1] + bytes([machine.number]))
            (destination / "private.txt").write_text("unselected private work\n")
            captured = self.cli(machine, ["workspace", "propose", "--goal", goal,
                "--checkout", checkout["id"]])
            require(captured["candidate"]["parent"] == seed_revision, "candidate captured the wrong accepted base")
            require("private.txt" not in captured["candidate"]["captured_paths"],
                    "unselected private file entered the frozen candidate")
            captures.append(captured)
        for index, (machine, captured) in enumerate(zip((first, second), captures)):
            def submit(m=machine, c=captured):
                receipt = variant(self.cli(m, ["workspace", "publish", "--goal", goal,
                    "--operation", c["operation"]["id"]]), "workspace_operation")
                results.append(operation_event(receipt))
                self.api(m, "completion.declare", goal=goal, subject=results[-1])
            if index == 0:
                interruption = self.interrupt_transfer(lead, submit, len(content))
                self.start(lead)
            else:
                submit()
        for proposal in results:
            self.wait("complete workspace proposal review after peer transfer", lambda p=proposal: self.cli(lead,
                ["workspace", "review", "--goal", goal, "--proposal", p], expected_errors=("unavailable", "not_found")))
        self.wait("exact candidate declarations arrive", lambda: all(
            variant(self.api(lead, "workspace.proposal", goal=goal, proposal=proposal), "workspace_proposal")["approved"]
            for proposal in results))
        stale_operation = {
            "id": "22" * 16, "checkout": None, "idempotency_key": "22" * 16,
            "kind": {"integrate": {"expected_epoch": epoch, "expected_head": seed_revision, "proposal": results[1]}},
            "state": "prepared",
        }
        self.api(lead, "workspace.operation.prepare", goal=goal, operation=stale_operation)
        acceptance = variant(self.cli(lead, ["--idempotency-key", "11" * 16, "workspace", "integrate",
            "--goal", goal, "--proposal", results[0], "--expected-head", seed_revision,
            "--expected-epoch", epoch]), "workspace_operation")
        accepted_revision = operation_event(acceptance)
        accepted_manifest = captures[0]["candidate"]["result_manifest"]
        self.expect_error(lead, ["call", "workspace.integrate", json.dumps({"goal": goal,
            "operation": stale_operation["id"]})], "conflict")
        require(variant(self.api(lead, "workspace.operation.show", goal=goal,
            operation=stale_operation["id"]), "workspace_operation") == stale_operation,
            "stale integration changed its prepared operation or invented a receipt")
        self.expect_error(lead, ["workspace", "integrate", "--goal", goal, "--proposal", results[1],
            "--expected-head", seed_revision, "--expected-epoch", epoch], "conflict")
        self.wait("accepted revision and file objects converge", lambda: all(
            self.complete_workspace(machine, goal, accepted_revision) for machine in self.machines))

        self.phase = "checkout_update"
        (accepted_directory / "code.txt").write_text("uncommitted conflict\n")
        (accepted_directory / "notes.txt").write_text("compatible local notes\n")
        (accepted_directory / "unrelated.txt").write_text("unrelated dirty work\n")
        before = directory_files(accepted_directory)
        binding_before = self.checkout_binding(lead, goal, accepted_checkout["id"])
        update = ["workspace", "update", "--goal", goal, "--checkout", accepted_checkout["id"],
                  "--revision", accepted_revision]
        self.expect_error(lead, update, "conflict")
        require(before == directory_files(accepted_directory), "conflicting update changed file bytes or modes")
        require_unchanged_checkout_binding(binding_before,
            self.checkout_binding(lead, goal, accepted_checkout["id"]), seed_revision, base)
        (accepted_directory / "code.txt").write_text("base\n")
        completion = self.cli(lead, update)
        require(completion["target_in_lineage_at_completion"] is True, "updated checkout target was not accepted")
        require((accepted_directory / "code.txt").read_text() == "worker 2\n", "wrong worker output updated")
        require((accepted_directory / "large.bin").read_bytes() == content[:-1] + bytes([first.number]),
                "updated large file bytes differ")
        require((accepted_directory / "notes.txt").read_text() == "compatible local notes\n", "compatible managed edit changed")
        require((accepted_directory / "unrelated.txt").read_text() == "unrelated dirty work\n", "unrelated work changed")
        require((destinations[1] / "code.txt").read_text() == "worker 3\n", "losing worker output changed")
        require((destinations[1] / "private.txt").read_text() == "unselected private work\n", "private worker file changed")
        for machine in self.machines:
            tree = self.cli(machine, ["workspace", "tree", "--goal", goal, "--revision", accepted_revision])
            require(tree["revision"]["result_manifest"] == accepted_manifest, "peer tree names a different manifest")
            require(self.cli(machine, ["workspace", "read", "--goal", goal, "--revision", accepted_revision,
                "--path", "code.txt"])["text"] == "worker 2\n", "peer accepted bytes differ")
            require(self.cli(machine, ["workspace", "read", "--goal", goal, "--revision", accepted_revision,
                "--path", "notes.txt"])["text"] == "base notes\n", "local dirty edit became shared authority")
        self.restart(lead)
        require(self.complete_workspace(lead, goal, accepted_revision), "accepted workspace changed after restart")
        require(variant(self.cli(lead, ["--idempotency-key", "11" * 16, "workspace", "integrate",
            "--goal", goal, "--proposal", results[0], "--expected-head", seed_revision,
            "--expected-epoch", epoch]), "workspace_operation") == acceptance,
            "durable integration retry changed its exact receipt")
        binding_after = self.checkout_binding(lead, goal, accepted_checkout["id"])
        require(binding_after["base_revision"] == accepted_revision and binding_after["base_manifest"] == accepted_manifest
            and binding_after["active_operation"] is None, "completed checkout base did not persist")
        status = self.cli(lead, ["workspace", "status", "--goal", goal, "--checkout", accepted_checkout["id"]])
        require("notes.txt" in status["dirty_paths"] and "unrelated.txt" in status["untracked_paths"],
                "preserved local work disappeared from status after restart")
        self.passed("competing_workspace_proposals", base_revision=seed_revision, base_manifest=base,
                    proposals=results, accepted_revision=accepted_revision, accepted_manifest=accepted_manifest,
                    publication_interruption=interruption,
                    resumed_rss=self.sample_rss(lead, "workspace_resumed_and_updated"),
                    stale_integration="conflict", dirty_update="conflict", compatible_dirty_edit="preserved",
                    exact_integration_receipt_after_restart=True, peer_readback=True, git_required=False)

        self.phase = "offline_cancellation"
        task = "task:" + self.recorded(lead, ["task", "open", "--goal", goal, "Explicit cancellation acknowledgment"])
        offer = self.recorded(lead, ["work", "offer", "--goal", goal, "--task", task, "--member", second.agent])
        self.wait("cancellation assignment arrives", lambda: self.event(second, goal, offer))
        self.cli(second, ["--agent", f"m{second.number}", "allow", "--goal", goal, "--task", task], owner=True)
        session = claims[1][1]
        claim = variant(self.cli(second, ["attempt", "start", "--goal", goal, "--task", task, "--offer", offer], session=session), "claimed")
        self.stop(second)
        cancellation = self.recorded(lead, ["attempt", "cancel", "--goal", goal, "--attempt", claim["attempt"]])
        self.start(second)
        self.wait("offline worker receives durable cancellation", lambda:
            any(item["cancel"] == cancellation for item in
                variant(self.cli(second, ["pending", "--goal", goal], session=session), "pending")["to_acknowledge"]))
        self.expect_error(second, ["contribution", "publish", "--goal", goal, "--attempt", claim["attempt"],
                          "--generation", claim["generation"], "late completion"], "conflict", session=session)
        acknowledged = self.recorded(second, ["call", "cancel.acknowledge", json.dumps({"goal": goal,
            "cancel": cancellation, "generation": claim["generation"], "outcome": "stopped"})], session=session)
        self.wait("coordinator receives explicit cancellation acknowledgment", lambda: self.event(lead, goal, acknowledged))
        self.restart(second)
        require(not any(item["cancel"] == cancellation for item in
                variant(self.cli(second, ["pending", "--goal", goal], session=session), "pending")["to_acknowledge"]),
                "acknowledged cancellation returned after restart")
        self.passed("offline_cancellation", attempt=claim["attempt"], cancellation=cancellation, acknowledgment=acknowledged)

        self.phase = "withdrawal_leave"
        note = self.recorded(first, ["contribution", "publish", "--goal", goal, "withdraw this local payload"])
        self.wait("note retained on coordinator", lambda: self.contributions_contain(lead, goal, {note: "withdraw this local payload"}))
        payload = self.event(first, goal, note)["content"][0]["hash"]
        self.api(first, "blob.withdraw", goal=goal, hash=payload)
        self.restart(first)
        require(self.event(first, goal, note)["text"] is None, "withdrawn payload readable after restart")
        require(self.event(lead, goal, note)["text"] == "withdraw this local payload", "withdrawal affected another replica")
        self.cli(first, ["--agent", f"m{first.number}", "goal", "leave", "--goal", goal], owner=True)
        self.expect_error(first, ["contribution", "publish", "--goal", goal, "must be refused"], "denied")
        self.restart(first)
        self.expect_error(first, ["contribution", "publish", "--goal", goal, "must still be refused"], "denied")
        self.passed("withdrawal_leave", payload=payload, metadata_event=note, retained_other_replica=True)

        self.phase = "offline_rotation"
        goal = self.create_goal(lead, "Synthetic offline rotation")
        for machine in (first, second):
            ticket = variant(self.cli(lead, ["goal", "invite", "--goal", goal], owner=True), "invited")["ticket"]
            self.join_goal(machine, ticket)
        del ticket
        self.wait("rotation goal admitted", lambda: all(
            (state := self.goal_status(m, goal)) and len(state["members"]) == 3 for m in self.machines))
        self.set_ask_levels(goal)
        # Stop all other holders before the member authors its offline suffix.
        self.stop(lead)
        self.stop(first)
        stale = self.recorded(second, ["contribution", "publish", "--goal", goal, "offline old epoch contribution"])
        self.stop(second)
        self.start(lead)
        removal = variant(self.cli(lead, ["member", "remove", "--goal", goal, "--member", second.agent], owner=True), "recorded")["event"]
        future = self.recorded(lead, ["contribution", "publish", "--goal", goal, "new epoch only"])
        removal_detail = self.event(lead, goal, removal)
        future_detail = self.event(lead, goal, future)
        require(removal_detail["payload"]["key_epoch"] == 1 and future_detail["payload"]["key_epoch"] == 1,
                "removal and future content did not rotate to epoch one")
        self.start(first)
        self.wait("offline current member recovers rotated epoch", lambda:
                  self.contributions_contain(first, goal, {future: "new epoch only"}))
        route_start = len(self.routes)
        self.start(second)
        # A transport path is not a successful authorized exchange. The protocol
        # refuses ordinary sync to a removed endpoint; it does not deliver a
        # revocation notification to update that endpoint's offline local view.
        self.wait("removed endpoint attempts a fresh transport path", lambda:
            any(f["machine"] == second.number and f["peer_endpoint"] == lead.endpoint
                and any(p["selected"] for p in f["paths"]) for f in self.routes[route_start:]))
        self.wait("current replicas agree on removal", lambda: all(
            all(m["member"] != second.agent for m in self.goal_status(peer, goal)["members"])
            for peer in (lead, first)))
        require(not self.contributions_contain(lead, goal, {stale: "offline old epoch contribution"}), "offline suffix escaped removal cutoff")
        stale_detail = self.event(second, goal, stale)
        require(stale_detail and stale_detail["text"] == "offline old epoch contribution",
                "offline contribution was not retained locally")
        require(self.cli(second, ["call", "blob.get", json.dumps({"goal": goal, "hash": future_detail["content"][0]["hash"]})],
                         expected_errors=("denied", "not_found")) is None, "removed member read future content")
        self.passed("offline_member_rotation", removal=removal, future=future, unreplicated_offline_event=stale,
                    removal_body=removal_detail["body"], goal=goal, current_member_recovered_key=True,
                    removed_local_view="may remain stale; protocol refuses ordinary sync before notification")
        self.summary["status"] = "passed"


class WorkspaceSmoke(Operations):
    def environment(self, machine):
        env = super().environment(machine)
        unavailable_tools = machine.home / "unavailable-tools"
        require(not unavailable_tools.exists(), "workspace smoke tool PATH unexpectedly exists")
        env["PATH"] = str(unavailable_tools)
        require(shutil.which("git", path=env["PATH"]) is None,
                "Git remains available through the child command PATH")
        return env

    def flow(self):
        self.machines = self.machines[:2]
        lead, worker = self.machines
        self.summary["qualification"] = "two production daemons on one host; ordinary-directory shared workspace; configured Iroh transport"
        self.summary["resources"] = {"verdict": "unmeasured; no performance claim", "rss_scope": "unmeasured",
                                     "network_bytes": "unmeasured", "cpu": "unmeasured"}
        self.summary["git"] = {"path_lookup": "unavailable", "child_scope": ["daemon", "cli"],
                               "configuration": "isolated nonexistent tool directory; outer harness PATH unchanged",
                               "optional_commit_import": "not exercised"}
        for machine in self.machines:
            self.environment(machine)
            self.record("git_path_unavailable", machine=machine.number, scope="daemon and CLI children")
            self.start(machine)
            machine.agent = identity(variant(self.cli(machine, ["agent", "enroll", f"m{machine.number}"], owner=True), "agent_enrolled")["agent"], "principal")
        goal = self.create_goal(lead, "Native workspace smoke")
        self.summary["goal"] = goal
        ticket = variant(self.cli(lead, ["goal", "invite", "--goal", goal], owner=True), "invited")["ticket"]
        self.join_goal(worker, ticket)
        del ticket
        self.wait("both members converge", lambda: all((state := self.goal_status(machine, goal)) and len(state["members"]) == 2 for machine in self.machines))
        self.set_ask_levels(goal)
        self.phase = "seed"
        root = lead.home.parent
        seed_input = root / "seed-input"
        seed_input.mkdir()
        (seed_input / "code.txt").write_text("base\n")
        (seed_input / "notes.txt").write_text("base notes\n")
        capture = self.cli(lead, ["workspace", "init", "--goal", goal, "--root", seed_input, "--path", "code.txt", "--path", "notes.txt", "--completion", json.dumps({"kind": "declaration", "by": {"kind": "contribution_author"}}), "--publish"], owner=True)
        seed_proposal = operation_event(capture["operation"])
        seed = operation_event(variant(self.cli(lead, ["workspace", "integrate", "--goal", goal, "--proposal", seed_proposal, "--expected-empty"]), "workspace_operation"))
        accepted_directory = root / "accepted"
        accepted_checkout = self.checkout(lead, goal, seed, accepted_directory)
        worker_directory = root / "worker"
        worker_checkout = self.checkout(worker, goal, seed, worker_directory)
        self.passed("seed_replicated", revision=seed, files=2, git_required=False)
        self.phase = "proposal"
        (worker_directory / "code.txt").write_text("worker edit\n")
        (worker_directory / "private.txt").write_text("worker private\n")
        proposed = self.cli(worker, ["workspace", "propose", "--goal", goal, "--checkout", worker_checkout["id"], "--publish"])
        proposal = operation_event(proposed["operation"])
        self.api(worker, "completion.declare", goal=goal, subject=proposal)
        self.wait("proposal complete at integrator", lambda: self.cli(lead, ["workspace", "review", "--goal", goal, "--proposal", proposal], expected_errors=("not_found", "unavailable")))
        self.wait("exact declaration replicated", lambda: variant(self.api(lead, "workspace.proposal", goal=goal, proposal=proposal), "workspace_proposal")["approved"])
        integration = variant(self.cli(lead, ["--idempotency-key", "11" * 16, "workspace", "integrate", "--goal", goal, "--proposal", proposal, "--expected-head", seed]), "workspace_operation")
        head = operation_event(integration)
        self.wait("accepted head replicated", lambda: all(self.complete_workspace(machine, goal, head) for machine in self.machines))
        self.passed("proposal_declared_and_integrated", proposal=proposal, revision=head, result_manifest=proposed["candidate"]["result_manifest"])
        self.phase = "update"
        (accepted_directory / "notes.txt").write_text("compatible local notes\n")
        (accepted_directory / "unrelated.txt").write_text("local private\n")
        updated = self.cli(lead, ["workspace", "update", "--goal", goal, "--checkout", accepted_checkout["id"]])
        require(updated["target_in_lineage_at_completion"], "updated revision was not accepted")
        require((accepted_directory / "code.txt").read_text() == "worker edit\n", "accepted code did not update")
        require((accepted_directory / "notes.txt").read_text() == "compatible local notes\n", "compatible local edit was lost")
        require((accepted_directory / "unrelated.txt").read_text() == "local private\n", "untracked local file was lost")
        require(all(not (directory / ".git").exists() for directory in (seed_input, accepted_directory, worker_directory)), "Git metadata appeared")
        self.passed("ordinary_checkout_update", compatible_dirty_edit="preserved", untracked_file="preserved")
        self.phase = "restart_and_readback"
        for machine in self.machines:
            self.restart(machine)
        self.wait("both heads complete after restart", lambda: all(self.complete_workspace(machine, goal, head) for machine in self.machines))
        for machine in self.machines:
            require(self.cli(machine, ["workspace", "read", "--goal", goal, "--path", "code.txt"])["text"] == "worker edit\n", "accepted bytes differ after restart")
            require(self.cli(machine, ["workspace", "read", "--goal", goal, "--path", "notes.txt"])["text"] == "base notes\n", "local edit became accepted content")
        retried = variant(self.cli(lead, ["--idempotency-key", "11" * 16, "workspace", "integrate", "--goal", goal, "--proposal", proposal, "--expected-head", seed]), "workspace_operation")
        require(retried == integration, "exact recorded receipt changed after restart")
        require(self.checkout_binding(lead, goal, accepted_checkout["id"])["base_revision"] == head, "checkout base did not survive restart")
        self.passed("both_restarts_and_peer_readback", exact_receipt_retained=True)
        self.summary["status"] = "passed"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--timeout-seconds", type=float, default=90)
    parser.add_argument("--network", choices=("local", "default"), default="default")
    parser.add_argument("--workflow", choices=("operations", "workspace"), default="operations",
                        help="full three-daemon operations, or two-daemon ordinary-directory workspace smoke")
    args = parser.parse_args(argv)
    if not args.binary.is_absolute() or not args.binary.is_file() or not os.access(args.binary, os.X_OK):
        parser.error("--binary must be an absolute executable path")
    if not math.isfinite(args.timeout_seconds) or args.timeout_seconds <= 0:
        parser.error("--timeout-seconds must be finite and positive")
    name = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + os.urandom(4).hex()
    artifacts = Path(__file__).resolve().parents[1] / "output" / "operations" / name
    workflow = WorkspaceSmoke if args.workflow == "workspace" else Operations
    check = workflow(args.binary.resolve(), args.timeout_seconds, artifacts, args.network)
    check.summary["workflow"] = args.workflow
    passed = check.run()
    print(json.dumps({"ok": passed, "summary": str(artifacts / "summary.json")}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
