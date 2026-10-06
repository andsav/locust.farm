#!/usr/bin/env python3
"""Exercise a farm with two production daemons and the real local HTTP service.

Scripted CLI actions on one host are transport/consent evidence, not a real-model
or two-physical-machine rehearsal. Only temporary homes created here are used.
"""
from datetime import datetime, timezone
import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

from check_t1 import CheckFailure, Machine, Qualification, binary_facts, process_stop, redact, redact_text, variant

ROOT = Path(__file__).resolve().parents[1]


class FarmQualification(Qualification):
    def __init__(self, binary, service_binary, timeout, output):
        super().__init__(binary, timeout, output, network="local")
        self.service_binary = service_binary
        self.service = None
        self.summary["qualification"] = "farm pipeline: two local production daemons, scripted clients, real HTTP service"
        self.summary["boundary"] = "One physical host; no model/harness activity, production deployment or cross-host qualification."
        self.summary["service_sha256"] = hashlib.sha256(service_binary.read_bytes()).hexdigest()
        self.summary["source_commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        self.summary["source_dirty"] = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True))
        self.active_goal = None
        self.binds = {}
        self.summary["port_changes"] = "not exercised; explicit stable loopback UDP binds across restarts"
        self.http = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def environment(self, machine):
        env = super().environment(machine)
        if machine.number not in self.binds:
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as available:
                available.bind(("127.0.0.1", 0))
                self.binds[machine.number] = "127.0.0.1:" + str(available.getsockname()[1])
        env["LOCUST_BIND"] = self.binds[machine.number]
        return env

    def http_get(self, path):
        try:
            with self.http.open(self.origin + path, timeout=self.remaining()) as response:
                return response.status, json.load(response)
        except urllib.error.HTTPError as error:
            return error.code, None
        except urllib.error.URLError:
            return 0, None

    def start_service(self):
        self.service_log = (self.artifact_dir / "service.log").open("a")
        self.service = subprocess.Popen([str(self.service_binary), "serve", "--bind", self.bind,
            "--database", str(self.database), "--public-enrollment", "--min-mutation-interval-ms", "0"],
            stdout=subprocess.DEVNULL, stderr=self.service_log, stdin=subprocess.DEVNULL)
        self.wait("farm service is reachable", lambda: self.http_get("/api/farms")[0] == 200)

    def stop_service(self):
        if self.service is not None:
            process_stop(self.service, self.timeout)
            self.service = None
            self.service_log.close()

    def preview(self, machine, goal):
        return variant(self.cli(machine, ["farm", "show", "--goal", goal], owner=True), "farm_preview")

    def consent(self, machine, goal, accept=True):
        args = ["farm", "consent", "--goal", goal, "--agent", machine.agent,
            "--accept" if accept else "--decline"]
        if accept:
            args += ["--name", "Coordinator" if machine.number == 1 else "Worker", "--group-label", f"Daemon {machine.number}"]
        return self.cli(machine, args, owner=True)

    def public(self, farm):
        code, result = self.http_get("/api/farms/" + farm)
        return result if code == 200 and result and result.get("snapshot") else None

    def execute(self):
        self.summary["binary"] = binary_facts(self.binary, self.timeout)
        with tempfile.TemporaryDirectory(prefix="lcf-", dir="/tmp") as directory:
            root = Path(directory)
            root.chmod(0o700)
            for source, name in [(self.binary, "locust"), (self.service_binary, "locust-farm")]:
                image = root / name
                shutil.copyfile(source, image)
                image.chmod(0o700)
                if hashlib.sha256(image.read_bytes()).digest() != hashlib.sha256(source.read_bytes()).digest():
                    raise CheckFailure("binary changed during execution snapshot")
            self.binary, self.service_binary = root / "locust", root / "locust-farm"
            self.database = root / "farm.sqlite"
            with socket.socket() as available:
                available.bind(("127.0.0.1", 0))
                self.bind = "127.0.0.1:" + str(available.getsockname()[1])
            self.origin = "http://" + self.bind
            for number in (1, 2):
                home = root / f"m{number}"
                home.mkdir(mode=0o700)
                self.machines.append(Machine(number, home))
            try:
                self.start_service()
                self.flow()
                self.summary["status"] = "passed"
            except Exception:
                if self.active_goal:
                    for machine in self.machines:
                        try:
                            self.summary[f"failure_m{machine.number}"] = {
                                "preview": self.preview(machine, self.active_goal),
                                "status": self.goal_status(machine, self.active_goal),
                                "events": self.history(machine, self.active_goal),
                            }
                        except Exception as diagnostic:
                            self.summary[f"failure_m{machine.number}"] = {"diagnostic": str(diagnostic)}
                raise
            finally:
                self.stop_service()
                self.cleanup()

    def flow(self):
        m1, m2 = self.machines
        for machine in self.machines:
            self.start(machine)
            result = self.cli(machine, ["agent", "enroll", f"m{machine.number}"], owner=True)
            machine.agent = variant(result, "agent_enrolled")["agent"]
        goal = self.create_goal(m1, "PRIVATE TITLE MUST NOT BE EXPORTED")
        self.summary["goal"] = goal
        self.active_goal = goal
        ticket = variant(self.cli(m1, ["goal", "invite", "--goal", goal], owner=True), "invited")["ticket"]
        self.join_goal(m2, ticket)
        del ticket
        self.wait("both daemons know both members", lambda: all(
            (s := self.goal_status(m, goal)) and len(s["members"]) == 2 for m in self.machines))
        self.grant_contributions(goal)
        self.cli(m1, ["farm", "on", "--goal", goal, "--service", self.origin,
            "--formation", "Reviewed collaboration", "--title", "Farm integration check", "--listed"], owner=True)
        initial = self.preview(m1, goal)
        self.summary["initial_preview"] = initial
        if initial["status"]["eligible"]:
            raise CheckFailure("farm was eligible before consent")
        # An agent credential must not gain publication authority through raw API calls.
        self.cli(m1, ["call", "farm.off", json.dumps({"goal": goal})], expected_errors=("denied",))
        self.consent(m1, goal)
        self.wait("remote daemon receives the policy", lambda: self.preview(m2, goal).get("policy"))
        self.consent(m2, goal)
        ready = self.wait("creator receives matching remote consent", lambda: (p if (p := self.preview(m1, goal)).get("status", {}).get("eligible") and p.get("snapshot") else None))
        farm = ready["snapshot"]["farm_id"]
        self.summary["farm_id"] = farm
        live = self.wait("eligible snapshot reaches the HTTP service", lambda: self.public(farm))
        self.check_snapshot(live["snapshot"])
        self.wait("listed farm appears in gallery", lambda: any(
            f["farm_id"] == farm for f in (self.http_get("/api/farms")[1] or {}).get("farms", [])))

        task = "task:" + self.recorded(m1, ["task", "open", "--goal", goal, "PRIVATE TASK BODY MUST NOT BE EXPORTED"])
        offer = self.recorded(m1, ["work", "offer", "--goal", goal, "--task", task, "--member", m2.agent])
        self.wait("worker receives the task", lambda: any(t["task"] == task for t in variant(self.cli(m2, ["board", "--goal", goal]), "board")))
        self.cli(m2, ["task", "authorize", "--goal", goal, "--task", task, "--agent", m2.agent], owner=True)
        session = m2.home / "sessions" / "farm.secret"
        self.cli(m2, ["session", "create", session], local=True)
        claim = variant(self.cli(m2, ["attempt", "start", "--goal", goal, "--task", task, "--offer", offer], session=session), "claimed")
        result = self.recorded(m2, ["contribution", "publish", "--goal", goal, "--task", task,
            "--attempt", claim["attempt"], "--generation", claim["generation"], "PRIVATE RESULT MUST NOT BE EXPORTED"], session=session)
        self.wait("creator receives worker contribution", lambda: any(c["contribution"] == result for c in variant(self.cli(m1, ["contributions", "--goal", goal]), "contributions")))
        self.recorded(m1, ["review", "record", "--goal", goal, "--subject", result, "--verdict", "approve", "PRIVATE REVIEW"])
        self.recorded(m1, ["scope", "select", "--goal", goal, "--subject", result])
        updated = self.wait("public task shows evaluated completion and selection", lambda: (v if (v := self.public(farm)) and any(t["completed"] and t["selected_candidate"] is not None for t in v["snapshot"]["tasks"]) else None))
        self.check_snapshot(updated["snapshot"])
        identifiers = {key: updated["snapshot"][key] for key in ("agents", "tasks")}
        self.stop_service()
        self.restart(m1)
        self.start_service()
        after_restart = self.recorded(m1, ["contribution", "publish", "--goal", goal, "PRIVATE NOTE AFTER RESTART"])
        self.wait("peer receives new work after publisher restart", lambda: after_restart in self.history(m2, goal))
        resumed = self.wait("new work reaches the service after daemon and service restart", lambda: (v if (v := self.public(farm)) and v["stream_version"] > updated["stream_version"] and len(v["snapshot"]["candidates"]) > len(updated["snapshot"]["candidates"]) else None))
        for key in identifiers:
            if [(v["id"], v.get("reference")) for v in identifiers[key]] != [(v["id"], v.get("reference")) for v in resumed["snapshot"][key]]:
                raise CheckFailure("public identifiers changed across restart")
        closed = self.recorded(m1, ["call", "scope.close", json.dumps({"goal": goal, "scope": "goal", "expected": None})])
        self.wait("explicit goal closure reaches the public page", lambda: (v := self.public(farm)) and v["snapshot"]["goal_state"] == "ended")
        self.recorded(m1, ["call", "scope.reopen", json.dumps({"goal": goal, "scope": "goal", "expected": closed})])
        self.wait("goal reopen resumes the same public farm", lambda: (v := self.public(farm)) and v["snapshot"]["goal_state"] == "open")
        self.consent(m2, goal, accept=False)
        self.wait("remote revocation suspends public snapshot", lambda: self.http_get("/api/farms/" + farm)[0] == 404)
        def absent_from_gallery():
            code, body = self.http_get("/api/farms")
            return code == 200 and not any(f["farm_id"] == farm for f in body["farms"])
        self.wait("suspended farm disappears from gallery", absent_from_gallery)
        self.consent(m2, goal)
        self.wait("restored consent resumes same farm", lambda: self.public(farm))
        self.stop_service()
        self.cli(m1, ["farm", "off", "--goal", goal], owner=True)
        self.restart(m1)
        self.start_service()
        self.wait("offline deletion survives publisher restart", lambda: self.http_get("/api/farms/" + farm)[0] == 404)
        def deletion_acknowledged():
            status = self.preview(m1, goal)["status"]
            return status and status["desired"] is None and status["pending"] is None and status["receipt"]
        self.wait("publisher records deletion acknowledgment", deletion_acknowledged)
        self.stop_service()
        self.start_service()
        if self.http_get("/api/farms/" + farm)[0] != 404:
            raise CheckFailure("deleted farm returned after service restart")
        self.summary["final_preview"] = self.preview(m1, goal)
        self.summary["public_snapshot"] = updated["snapshot"]

    def check_snapshot(self, snapshot):
        wire = json.dumps(snapshot)
        for forbidden in ["PRIVATE", self.origin, *(m.agent for m in self.machines), *(m.endpoint for m in self.machines)]:
            if forbidden in wire:
                raise CheckFailure("private canary or protocol identifier leaked into snapshot")
        if len(snapshot["agents"]) != 2 or len(snapshot["groups"]) != 2:
            raise CheckFailure("snapshot did not contain both consenting daemon groups")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/locust")
    parser.add_argument("--service-binary", type=Path, default=ROOT / "target/debug/locust-farm")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout-seconds", type=float, default=45)
    args = parser.parse_args()
    if not math.isfinite(args.timeout_seconds) or args.timeout_seconds <= 0:
        parser.error("--timeout-seconds must be finite and positive")
    check = FarmQualification(args.binary.resolve(), args.service_binary.resolve(), args.timeout_seconds, args.output.resolve())
    try:
        check.execute()
    except Exception as error:
        check.summary["status"] = "failed"
        check.summary["failures"].append(redact_text(str(error)))
        raise
    finally:
        check.summary["finished_at"] = datetime.now(timezone.utc).isoformat()
        (check.artifact_dir / "summary.json").write_text(json.dumps(redact(check.summary), indent=2) + "\n")
        check.transcript.close()
    if check.summary["status"] != "passed":
        raise CheckFailure("farm cleanup failed; inspect summary")
    print(json.dumps({"status": "passed", "checks": len(check.summary["checks"]), "output": str(check.artifact_dir)}))


if __name__ == "__main__":
    main()
