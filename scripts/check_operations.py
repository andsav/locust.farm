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
from pathlib import Path
import subprocess
import threading
import time

from check_t1 import CheckFailure, Qualification, identity, variant


def require(condition, message):
    if not condition:
        raise CheckFailure(message)


class Operations(Qualification):
    def __init__(self, binary, timeout, artifact_dir, network="default"):
        super().__init__(binary, timeout, artifact_dir, network=network)
        self.summary["qualification"] = "three production daemons on one host; synthetic operational workflows"
        self.summary["cases"] = {}
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
                        machine.process.kill()
                        observed.update(hash=path.stem, prefix_bytes=size)
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
        goal = variant(self.cli(lead, ["goal", "create", "--title", "Synthetic operations"]), "goal_created")["goal"]
        self.summary["goal"] = goal
        for machine in (first, second):
            ticket = variant(self.cli(lead, ["goal", "invite", "--goal", goal]), "invited")["ticket"]
            self.cli(machine, ["goal", "join", "--ticket", ticket])
        del ticket
        self.wait("three members converge", lambda: all(
            (state := self.goal_status(m, goal)) and len(state["members"]) == 3 for m in self.machines))
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
        self.api(lead, "doc.accept", goal=goal, revision=revisions[0])
        self.expect_error(lead, ["call", "doc.accept", json.dumps({"goal": goal, "revision": revisions[1]})], "conflict")
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
        tasks = [self.recorded(lead, ["task", "propose", "--goal", goal, "--input", base, f"Worker {m.number} patch"])
                 for m in (first, second)]
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
                    bytes=len(content), sha256=hashlib.sha256(content).hexdigest(), interruption=interrupted)
        self.start(lead)

        self.phase = "competing_patches"
        patches, results, claims = [], [], []
        for machine, task, destination in zip((first, second), tasks, destinations):
            assignment = self.recorded(lead, ["task", "assign", "--goal", goal, "--task", task, "--assignee", machine.agent])
            self.wait(f"worker {machine.number} receives assignment", lambda m=machine, a=assignment: self.event(m, goal, a))
            self.cli(machine, ["task", "authorize", "--goal", goal, "--assignment", assignment], owner=True)
            session = machine.home / "sessions" / "operations.secret"
            self.cli(machine, ["session", "create", session], local=True)
            claim = variant(self.cli(machine, ["task", "claim", "--goal", goal, "--assignment", assignment], session=session), "claimed")
            claims.append((assignment, session, claim["generation"]))
            (destination / "code.txt").write_text(f"worker {machine.number}\n")
            (destination / "large.bin").write_bytes(content[:-1] + bytes([machine.number]))
            patch = self.cli(machine, ["patch", "create", "--goal", goal, "--root", destination, "--base", base,
                                       "--path", "code.txt", "--path", "large.bin"])
            patches.append(patch)
        for index, (machine, patch, claim) in enumerate(zip((first, second), patches, claims)):
            assignment, session, generation = claim
            def submit(m=machine, p=patch, a=assignment, s=session, g=generation):
                results.append(self.recorded(m, ["patch", "submit", "--goal", goal,
                    "--patch", p["contribution_id"], "--assignment", a, "--generation", g, "Synthetic patch"], session=s))
            if index == 0:
                interruption = self.interrupt_transfer(lead, submit, len(content))
                self.start(lead)
            else:
                submit()
        for patch in patches:
            self.wait("complete patch review after peer transfer", lambda p=patch: self.cli(lead,
                ["patch", "review", "--goal", goal, "--patch", p["contribution_id"]], expected_errors=("unavailable", "not_found")))
        self.cli(lead, ["patch", "accept", "--goal", goal, "--result", results[0], "--patch", patches[0]["contribution_id"]])
        self.expect_error(lead, ["patch", "accept", "--goal", goal, "--result", results[1], "--patch", patches[1]["contribution_id"]], "conflict")
        apply = ["patch", "apply", "--goal", goal, "--root", source, "--patch", patches[0]["contribution_id"],
                 "--expected-base", base, "--expected-git-head", commit]
        (source / "code.txt").write_text("uncommitted conflict\n")
        (source / "unrelated.txt").write_text("unrelated dirty work\n")
        before = {p.name: p.read_bytes() for p in source.iterdir() if p.is_file()}
        self.expect_error(lead, apply, "conflict")
        require(before == {p.name: p.read_bytes() for p in source.iterdir() if p.is_file()}, "conflicting apply changed files")
        require(self.goal_status(lead, goal)["workspace"]["integrated"] is None, "conflicting apply recorded integration")
        (source / "code.txt").write_text("base\n")
        self.cli(lead, apply)
        require((source / "code.txt").read_text() == "worker 2\n", "wrong worker output integrated")
        require((source / "unrelated.txt").read_text() == "unrelated dirty work\n", "unrelated work changed")
        require((destinations[1] / "code.txt").read_text() == "worker 3\n", "losing worker output changed")
        require(self.goal_status(lead, goal)["workspace"]["integrated"] == patches[0]["contribution"]["head"], "integration not recorded")
        self.passed("competing_patches", base=base, patches=[p["contribution_id"] for p in patches],
                    results=results, patch_interruption=interruption, stale_acceptance="conflict", dirty_apply="conflict")

        self.phase = "offline_cancellation"
        task = self.recorded(lead, ["task", "propose", "--goal", goal, "Explicit cancellation acknowledgment"])
        assignment = self.recorded(lead, ["task", "assign", "--goal", goal, "--task", task, "--assignee", second.agent])
        self.wait("cancellation assignment arrives", lambda: self.event(second, goal, assignment))
        self.cli(second, ["task", "authorize", "--goal", goal, "--assignment", assignment], owner=True)
        session = claims[1][1]
        claim = variant(self.cli(second, ["task", "claim", "--goal", goal, "--assignment", assignment], session=session), "claimed")
        self.stop(second)
        cancellation = self.recorded(lead, ["task", "cancel", "--goal", goal, "--assignment", assignment])
        self.start(second)
        self.wait("offline worker receives durable cancellation", lambda:
            any(item["cancel"] == cancellation for item in
                variant(self.cli(second, ["pending", "--goal", goal], session=session), "pending")["to_acknowledge"]))
        self.expect_error(second, ["task", "submit", "--goal", goal, "--assignment", assignment,
                          "--generation", claim["generation"], "late completion"], "conflict", session=session)
        acknowledged = self.recorded(second, ["call", "cancel.acknowledge", json.dumps({"goal": goal,
            "cancel": cancellation, "generation": claim["generation"], "outcome": "stopped"})], session=session)
        self.wait("coordinator receives explicit cancellation acknowledgment", lambda: self.event(lead, goal, acknowledged))
        self.restart(second)
        require(not any(item["cancel"] == cancellation for item in
                variant(self.cli(second, ["pending", "--goal", goal], session=session), "pending")["to_acknowledge"]),
                "acknowledged cancellation returned after restart")
        self.passed("offline_cancellation", assignment=assignment, cancellation=cancellation, acknowledgment=acknowledged)

        self.phase = "withdrawal_leave"
        note = self.recorded(first, ["note", "add", "--goal", goal, "withdraw this local payload"])
        self.wait("note retained on coordinator", lambda: self.notes_contain(lead, goal, {note: "withdraw this local payload"}))
        payload = self.event(first, goal, note)["content"][0]["hash"]
        self.api(first, "blob.withdraw", goal=goal, hash=payload)
        self.restart(first)
        require(self.event(first, goal, note)["text"] is None, "withdrawn payload readable after restart")
        require(self.event(lead, goal, note)["text"] == "withdraw this local payload", "withdrawal affected another replica")
        self.api(first, "goal.leave", goal=goal)
        self.expect_error(first, ["note", "add", "--goal", goal, "must be refused"], "denied")
        self.restart(first)
        self.expect_error(first, ["note", "add", "--goal", goal, "must still be refused"], "denied")
        self.passed("withdrawal_leave", payload=payload, metadata_event=note, retained_other_replica=True)

        self.phase = "offline_rotation"
        goal = variant(self.cli(lead, ["goal", "create", "--title", "Synthetic offline rotation"]), "goal_created")["goal"]
        for machine in (first, second):
            ticket = variant(self.cli(lead, ["goal", "invite", "--goal", goal]), "invited")["ticket"]
            self.cli(machine, ["goal", "join", "--ticket", ticket])
        del ticket
        self.wait("rotation goal admitted", lambda: all(
            (state := self.goal_status(m, goal)) and len(state["members"]) == 3 for m in self.machines))
        # Stop all other holders before the member authors its offline suffix.
        self.stop(lead)
        self.stop(first)
        stale = self.recorded(second, ["note", "add", "--goal", goal, "offline old epoch contribution"])
        self.stop(second)
        self.start(lead)
        removal = variant(self.api(lead, "member.remove", goal=goal, member=second.agent), "recorded")["event"]
        future = self.recorded(lead, ["note", "add", "--goal", goal, "new epoch only"])
        removal_detail = self.event(lead, goal, removal)
        future_detail = self.event(lead, goal, future)
        require(removal_detail["payload"]["key_epoch"] == 1 and future_detail["payload"]["key_epoch"] == 1,
                "removal and future content did not rotate to epoch one")
        self.start(first)
        self.wait("offline current member recovers rotated epoch", lambda:
                  self.notes_contain(first, goal, {future: "new epoch only"}))
        route_start = len(self.routes)
        self.start(second)
        # A transport path is not a successful authorized exchange. Protocol 1
        # refuses ordinary sync to a removed endpoint; it does not deliver a
        # revocation notification to update that endpoint's offline local view.
        self.wait("removed endpoint attempts a fresh transport path", lambda:
            any(f["machine"] == second.number and f["peer_endpoint"] == lead.endpoint
                and any(p["selected"] for p in f["paths"]) for f in self.routes[route_start:]))
        self.wait("current replicas agree on removal", lambda: all(
            all(m["member"] != second.agent for m in self.goal_status(peer, goal)["members"])
            for peer in (lead, first)))
        require(not self.notes_contain(lead, goal, {stale: "offline old epoch contribution"}), "offline suffix escaped removal cutoff")
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
