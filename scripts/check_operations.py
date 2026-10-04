#!/usr/bin/env python3
"""Production-daemon operational workflows on three private same-host replicas.

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
from pathlib import Path
import subprocess
import threading
import time

from check_t1 import CheckFailure, Qualification, identity, variant


def require(condition, message):
    if not condition:
        raise CheckFailure(message)


def require_unchanged_export_binding(before, after, base):
    require(before.get("exported") == base and before.get("integrated") is None,
            "export-only workspace binding did not identify the exported base without integration")
    require(after == before, "conflicting apply changed workspace binding")


class Operations(Qualification):
    def __init__(self, binary, timeout, artifact_dir, network="default"):
        super().__init__(binary, timeout, artifact_dir, network=network)
        self.summary["qualification"] = "three production daemons on one host; synthetic operational workflows"
        self.summary["cases"] = {}
        self.summary["resources"] = {"verdict": "measurements only; no performance thresholds",
            "rss_scope": "receiver paused at observed partial transfer and after completed resume; sampled, not maximum",
            "network_bytes": "unmeasured", "cpu": "unmeasured"}
        self.summary["harness_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()

    def environment(self, machine):
        env = super().environment(machine)
        env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null")
        return env

    def api(self, machine, operation, **fields):
        return self.cli(machine, ["call", operation, json.dumps(fields)])

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

    def materialize(self, machine, goal, manifest, destination):
        result = self.cli(machine, ["workspace", "materialize", "--goal", goal,
                                   "--manifest", manifest, "--destination", destination],
                          expected_errors=("unavailable",))
        return result

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
            machine.agent = identity(variant(self.cli(machine, ["agent", "enroll", f"m{machine.number}",
                                      "--manage-goals"], owner=True), "agent_enrolled")["agent"], "principal")
        goal = self.create_goal(lead, "Synthetic operations")
        self.summary["goal"] = goal
        for machine in (first, second):
            ticket = variant(self.cli(lead, ["goal", "invite", "--goal", goal]), "invited")["ticket"]
            self.cli(machine, ["goal", "join", "--ticket", ticket])
        del ticket
        self.wait("three members converge", lambda: all(
            (state := self.goal_status(m, goal)) and len(state["members"]) == 3 for m in self.machines))
        self.grant_contributions(goal)
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

        self.phase = "snapshot"
        source = root / "source"
        source.mkdir()
        # One file is much larger than the production 1 MiB transfer chunk.
        content = bytes(range(256)) * (48 * 1024)
        (source / "large.bin").write_bytes(content)
        (source / "code.txt").write_text("base\n")
        git_env = self.environment(lead)
        git_env.update(GIT_AUTHOR_NAME="Synthetic operations", GIT_AUTHOR_EMAIL="operations@example.invalid",
                       GIT_COMMITTER_NAME="Synthetic operations", GIT_COMMITTER_EMAIL="operations@example.invalid")
        for args in (["init", "-q"], ["add", "large.bin", "code.txt"],
                     ["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgSign=false", "commit", "-qm", "synthetic base"]):
            subprocess.run(["git", *args], cwd=source, env=git_env, check=True, capture_output=True)
        commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source, env=git_env, text=True).strip()
        self.stop(second)
        base = self.cli(lead, ["workspace", "export", "--goal", goal, "--root", source, "--commit", commit])["manifest"]
        task = "task:" + self.recorded(lead, ["task", "open", "--goal", goal, "--inputs", json.dumps({"snapshot": base}), "Competing worker patches"])
        tasks = [task, task]
        destinations = [root / "worker2", root / "worker3"]
        self.wait("first replica retains complete snapshot", lambda: self.materialize(first, goal, base, destinations[0]))
        require((destinations[0] / "large.bin").read_bytes() == content, "first snapshot bytes differ")
        self.restart(first)
        self.stop(lead)
        interrupted = self.interrupt_transfer(second, lambda: self.start_unchecked(second), len(content))
        self.start(second)
        self.wait("third replica resumes snapshot with original source offline",
                  lambda: self.materialize(second, goal, base, destinations[1]))
        require((destinations[1] / "large.bin").read_bytes() == content, "resumed snapshot bytes differ")
        require(not (second.home / "blobs" / (interrupted["hash"] + ".staged")).exists(), "completed stage was not promoted")
        self.passed("snapshot_resume_retained_replica", original_source_offline=True,
                    bytes=len(content), sha256=hashlib.sha256(content).hexdigest(), interruption=interrupted,
                    resumed_rss=self.sample_rss(second, "snapshot_resume_complete"))
        self.start(lead)

        self.phase = "competing_patches"
        patches, results, claims = [], [], []
        for machine, task, destination in zip((first, second), tasks, destinations):
            offer = self.recorded(lead, ["work", "offer", "--goal", goal, "--task", task, "--recipient", machine.agent])
            self.wait(f"worker {machine.number} receives assignment", lambda m=machine, a=offer: self.event(m, goal, a))
            self.cli(machine, ["task", "authorize", "--goal", goal, "--task", task, "--agent", machine.agent], owner=True)
            session = machine.home / "sessions" / "operations.secret"
            self.cli(machine, ["session", "create", session], local=True)
            claim = variant(self.cli(machine, ["attempt", "start", "--goal", goal, "--task", task, "--offer", offer], session=session), "claimed")
            claims.append((claim["attempt"], session, claim["generation"]))
            (destination / "code.txt").write_text(f"worker {machine.number}\n")
            (destination / "large.bin").write_bytes(content[:-1] + bytes([machine.number]))
            patch = self.cli(machine, ["patch", "create", "--goal", goal, "--root", destination, "--base", base,
                                       "--path", "code.txt", "--path", "large.bin"])
            patches.append(patch)
        for index, (machine, patch, claim) in enumerate(zip((first, second), patches, claims)):
            attempt, session, generation = claim
            def submit(m=machine, p=patch, a=attempt, s=session, g=generation):
                results.append(self.recorded(m, ["patch", "submit", "--goal", goal,
                    "--patch", p["contribution_id"], "--attempt", a, "--generation", g, "Synthetic patch"], session=s))
            if index == 0:
                interruption = self.interrupt_transfer(lead, submit, len(content))
                self.start(lead)
            else:
                submit()
        for patch in patches:
            self.wait("complete patch review after peer transfer", lambda p=patch: self.cli(lead,
                ["patch", "review", "--goal", goal, "--patch", p["contribution_id"]], expected_errors=("unavailable", "not_found")))
        self.api(lead, "review.record", goal=goal, subject=results[0], verdict="approve", text="Reviewed first patch")
        self.api(lead, "review.record", goal=goal, subject=results[1], verdict="approve", text="Reviewed second patch")
        self.cli(lead, ["patch", "select", "--goal", goal, "--subject", results[0]])
        self.expect_error(lead, ["patch", "select", "--goal", goal, "--subject", results[1]], "conflict")
        apply = ["patch", "apply", "--goal", goal, "--subject", results[0], "--root", source,
                 "--expected-git-head", commit]
        (source / "code.txt").write_text("uncommitted conflict\n")
        (source / "unrelated.txt").write_text("unrelated dirty work\n")
        before = {p.name: p.read_bytes() for p in source.iterdir() if p.is_file()}
        binding_before = self.goal_status(lead, goal)["workspace"]
        self.expect_error(lead, apply, "conflict")
        require(before == {p.name: p.read_bytes() for p in source.iterdir() if p.is_file()}, "conflicting apply changed files")
        require_unchanged_export_binding(binding_before, self.goal_status(lead, goal)["workspace"], base)
        (source / "code.txt").write_text("base\n")
        self.cli(lead, apply)
        require((source / "code.txt").read_text() == "worker 2\n", "wrong worker output integrated")
        require((source / "unrelated.txt").read_text() == "unrelated dirty work\n", "unrelated work changed")
        require((destinations[1] / "code.txt").read_text() == "worker 3\n", "losing worker output changed")
        require(self.goal_status(lead, goal)["workspace"]["integrated"] == patches[0]["contribution"]["head"], "integration not recorded")
        self.passed("competing_patches", base=base, patches=[p["contribution_id"] for p in patches],
                    results=results, patch_interruption=interruption,
                    resumed_rss=self.sample_rss(lead, "patches_resumed_and_integrated"), stale_selection="conflict", dirty_apply="conflict")

        self.phase = "offline_cancellation"
        task = "task:" + self.recorded(lead, ["task", "open", "--goal", goal, "Explicit cancellation acknowledgment"])
        offer = self.recorded(lead, ["work", "offer", "--goal", goal, "--task", task, "--recipient", second.agent])
        self.wait("cancellation assignment arrives", lambda: self.event(second, goal, offer))
        self.cli(second, ["task", "authorize", "--goal", goal, "--task", task, "--agent", second.agent], owner=True)
        session = claims[1][1]
        claim = variant(self.cli(second, ["attempt", "start", "--goal", goal, "--task", task, "--offer", offer], session=session), "claimed")
        self.stop(second)
        cancellation = self.recorded(lead, ["attempt", "cancel", "--goal", goal, "--attempt", claim["attempt"]])
        self.start(second)
        self.wait("offline worker receives durable cancellation", lambda:
            any(item["cancel"] == cancellation for item in
                variant(self.cli(second, ["pending", "--goal", goal], session=session), "pending")["to_acknowledge"]))
        self.expect_error(second, ["contribution", "publish", "--goal", goal, "--task", task, "--attempt", claim["attempt"],
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
        self.api(first, "goal.leave", goal=goal)
        self.expect_error(first, ["contribution", "publish", "--goal", goal, "must be refused"], "denied")
        self.restart(first)
        self.expect_error(first, ["contribution", "publish", "--goal", goal, "must still be refused"], "denied")
        self.passed("withdrawal_leave", payload=payload, metadata_event=note, retained_other_replica=True)

        self.phase = "offline_rotation"
        goal = self.create_goal(lead, "Synthetic offline rotation")
        for machine in (first, second):
            ticket = variant(self.cli(lead, ["goal", "invite", "--goal", goal]), "invited")["ticket"]
            self.cli(machine, ["goal", "join", "--ticket", ticket])
        del ticket
        self.wait("rotation goal admitted", lambda: all(
            (state := self.goal_status(m, goal)) and len(state["members"]) == 3 for m in self.machines))
        self.grant_contributions(goal)
        # Stop all other holders before the member authors its offline suffix.
        self.stop(lead)
        self.stop(first)
        stale = self.recorded(second, ["contribution", "publish", "--goal", goal, "offline old epoch contribution"])
        self.stop(second)
        self.start(lead)
        removal = variant(self.api(lead, "member.remove", goal=goal, member=second.agent), "recorded")["event"]
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
        # A transport path is not a successful authorized exchange. Protocol 2
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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--timeout-seconds", type=float, default=90)
    parser.add_argument("--network", choices=("local", "default"), default="default")
    args = parser.parse_args(argv)
    if not args.binary.is_absolute() or not args.binary.is_file() or not os.access(args.binary, os.X_OK):
        parser.error("--binary must be an absolute executable path")
    if not math.isfinite(args.timeout_seconds) or args.timeout_seconds <= 0:
        parser.error("--timeout-seconds must be finite and positive")
    name = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + os.urandom(4).hex()
    artifacts = Path(__file__).resolve().parents[1] / "output" / "operations" / name
    check = Operations(args.binary.resolve(), args.timeout_seconds, artifacts, args.network)
    passed = check.run()
    print(json.dumps({"ok": passed, "summary": str(artifacts / "summary.json")}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
