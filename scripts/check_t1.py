#!/usr/bin/env python3
"""Qualify one explicit Locust binary with three private local daemon processes.

This is a local integration gate, not published-build or three-Mac T1 evidence.
It exercises the CLI, SQLite restart persistence, sealed content, and Iroh with
the daemon defaults for relay/address discovery. It does not simulate OS
sleep/wake. --timeout-seconds bounds each qualification wait and child command;
it is a harness deadline, not daemon policy. Only Python's standard library is
used. Tickets live only in transient command/response variables and are redacted
from artifacts. Credential and session secret files are never read by Python.
"""

import argparse
from datetime import datetime, timezone
import errno
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import secrets
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time

from owner_plans import confirmation_arguments


class CheckFailure(Exception):
    """A qualification expectation failed; messages must contain no secrets."""


TICKET = re.compile(r"locust-invite-[A-Za-z0-9_-]+")
SECRET_FIELD = re.compile(
    r"(?i)\b(credential|session_secret|secret|secret_key)\s*[:=]\s*"
    r"(?:\[[^\]]*\]|[\"'][^\"']*[\"']|[^\s,}]+)"
)
ROUTE = re.compile(r"^locust: peer ([0-9a-f]{64}) paths (.*)$")
PATH = re.compile(r"kind: (Direct|Relay|Other), selected: (true|false), rtt: ([^}]+)")


def redact_text(text):
    text = TICKET.sub("<redacted-ticket>", text)
    return SECRET_FIELD.sub(lambda match: match.group(1) + "=<redacted>", text)


def redact(value):
    """Keep public identifiers and file paths; discard capability/secret fields."""
    if isinstance(value, dict):
        return {
            key: "<redacted>" if key.lower() in {
                "ticket", "credential", "session_secret", "secret", "secret_key"
            } else redact(item)
            for key, item in value.items()
        }
    if isinstance(value, (list, tuple)):
        return [redact(item) for item in value]
    if isinstance(value, str):
        return redact_text(value)
    return value


def route_fact(line):
    """Read only the daemon's public, address-free PathSnapshot log format."""
    match = ROUTE.match(line.strip())
    if not match:
        return None
    paths = [
        {"kind": kind.lower(), "selected": selected == "true", "rtt": rtt.strip()}
        for kind, selected, rtt in PATH.findall(match.group(2))
    ]
    return {"peer_endpoint": match.group(1), "paths": paths}


MDNS_GROUP = ("224.0.0.251", 5353)


def mdns_query(transaction):
    """One standard mDNS question: PTR records of the service Iroh's local lookup uses."""
    name = b"".join(bytes([len(label)]) + label for label in (b"_irohv1", b"_udp", b"local")) + b"\0"
    return struct.pack("!6H", transaction, 0, 1, 0, 0, 0) + name + struct.pack("!2H", 12, 1)


def local_multicast(timeout=2.0):
    """None when this process can send an mDNS query to the local multicast group
    and hear it back over multicast loopback, else why not.

    With no relay, members that are not the host find each other only through
    multicast lookup (crates/locust-net/src/lib.rs, Lookup::local_network). A
    sandbox, or macOS Local Network privacy for the app that started this run,
    can deny multicast while unicast works: the daemons then reach only the host
    their ticket names, and only until it restarts on a new port. The daemons
    started here inherit this process's permission."""
    query = mdns_query(secrets.randbits(16))
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP) as sock:
            # Share the mDNS port as the system responder and the daemons do.
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            if hasattr(socket, "SO_REUSEPORT"):
                sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEPORT, 1)
            sock.bind(("", MDNS_GROUP[1]))
            sock.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP,
                            socket.inet_aton(MDNS_GROUP[0]) + socket.inet_aton("0.0.0.0"))
            sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_LOOP, 1)
            sock.sendto(query, MDNS_GROUP)
            deadline = time.monotonic() + timeout
            while (remaining := deadline - time.monotonic()) > 0:
                sock.settimeout(remaining)
                if sock.recvfrom(9000)[0] == query:
                    return None
    except TimeoutError:
        pass
    except OSError as error:
        if not error.errno:
            return type(error).__name__
        return f"{errno.errorcode.get(error.errno, error.errno)} ({os.strerror(error.errno)})"
    return f"the query did not come back over multicast loopback within {timeout:g}s"


def require_local_multicast(transport):
    """The precondition of a relay-free local-lookup run. Records the probe in
    `transport` and fails as the environment's fault, before any daemon starts."""
    reason = local_multicast()
    transport["local_multicast"] = reason or "available"
    if reason is not None:
        raise CheckFailure(
            f"environment: local-network multicast is unavailable to this run: {reason}. "
            "With no relay, members find each other only by multicast lookup. On macOS, "
            "allow Local Network access for the app that started this run (System Settings, "
            "Privacy & Security, Local Network) or start it from one that has it")


def identity(value, name):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        raise CheckFailure(f"{name} was not a full canonical public identifier")
    return value


def variant(result, kind):
    if not isinstance(result, dict) or kind not in result:
        raise CheckFailure(f"CLI response did not contain {kind}")
    return result[kind]


def binary_facts(binary, timeout):
    digest = hashlib.sha256()
    with binary.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    try:
        result = subprocess.run([str(binary), "--version"], capture_output=True,
                                text=True, timeout=timeout, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise CheckFailure(f"binary version command failed ({type(error).__name__})") from None
    match = re.fullmatch(r"locust (\S+) \(([^\s)]+)\) api (\d+) protocol (\d+)\s*", result.stdout)
    if result.returncode != 0 or not match:
        raise CheckFailure("binary did not report the published version/commit/API/protocol format")
    return {
        "path": str(binary), "sha256": digest.hexdigest(),
        "version": match[1], "embedded_commit": match[2],
        "api_version": int(match[3]), "protocol_version": int(match[4]),
        "version_line": result.stdout.strip(),
    }


def process_stop(process, timeout):
    """Reap only the process launched by this harness, escalating if needed."""
    if process.poll() is None:
        try:
            process.terminate()
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            try:
                process.kill()
            except ProcessLookupError:
                pass
            process.wait()
    else:
        process.wait()
    return process.returncode


class Machine:
    def __init__(self, number, home):
        self.number = number
        self.home = home
        self.credential = home / "agents" / f"m{number}.credential"
        self.process = None
        self.reader = None
        self.generation = 0
        self.agent = None
        self.endpoint = None


class Qualification:
    def __init__(self, binary, timeout, artifact_dir, network="default"):
        self.binary = binary
        self.network = network
        self.timeout = timeout
        self.artifact_dir = artifact_dir
        self.artifact_dir.mkdir(parents=True, exist_ok=False)
        self.transcript = (artifact_dir / "transcript.jsonl").open("w", encoding="utf-8")
        self.lock = threading.Lock()
        self.phase = "startup"
        self.deadline = None
        self.routes = []
        self.machines = []
        self.summary = {
            "qualification": "three local processes; not published-build or three-Mac T1",
            "status": "running", "started_at": self.timestamp(),
            "platform": platform.platform(), "architecture": platform.machine(),
            "transport": {"mode": network, "LOCUST_RELAY": "none" if network == "local" else "n0",
                          "LOCUST_LOOKUP": "local" if network == "local" else "all",
                          "configuration": "explicit local reproduction" if network == "local" else "daemon defaults; inherited overrides removed"},
            "timeout_seconds": timeout, "sleep_wake": "not exercised",
            "port_changes": "permitted; socket addresses are not exposed by status/route snapshots",
            "failures": [], "events": {}, "checks": [],
        }

    @staticmethod
    def timestamp():
        return datetime.now(timezone.utc).isoformat()

    def record(self, kind, **fields):
        record = redact({"time": self.timestamp(), "phase": self.phase, "kind": kind, **fields})
        with self.lock:
            self.transcript.write(json.dumps(record, sort_keys=True) + "\n")
            self.transcript.flush()
        return record

    def remaining(self):
        if self.deadline is None:
            return self.timeout
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise CheckFailure(f"qualification wait timed out in {self.phase}")
        return min(self.timeout, remaining)

    def environment(self, machine):
        env = os.environ.copy()
        for key in ("LOCUST_CREDENTIAL", "LOCUST_SESSION", "LOCUST_BIND", "LOCUST_RELAY", "LOCUST_LOOKUP"):
            env.pop(key, None)
        env["LOCUST_HOME"] = str(machine.home)
        if self.network == "local":
            env.update(LOCUST_RELAY="none", LOCUST_LOOKUP="local")
        return env

    def cli(self, machine, arguments, *, owner=False, session=None, local=False,
            expected_errors=()):
        command = [str(self.binary), "--home", str(machine.home), "--json"]
        if owner:
            command.append("--owner")
        elif not local:
            command.extend(["--credential", str(machine.credential)])
        if session is not None:
            command.extend(["--session", str(session)])
        command.extend(map(str, arguments))
        try:
            output = subprocess.run(command, env=self.environment(machine), capture_output=True,
                                    text=True, timeout=self.remaining(), check=False)
        except subprocess.TimeoutExpired:
            self.record("cli_timeout", machine=machine.number, command=command)
            raise CheckFailure(f"M{machine.number} CLI command timed out") from None
        except OSError as error:
            self.record("cli_launch_failure", machine=machine.number, command=command,
                        error_kind=type(error).__name__)
            raise CheckFailure(f"M{machine.number} CLI command could not start") from None
        # Never copy undecodable stdout into evidence: it might contain a ticket.
        try:
            body = json.loads(output.stdout)
        except (json.JSONDecodeError, UnicodeError):
            self.record("invalid_cli_json", machine=machine.number, command=command,
                        exit_status=output.returncode)
            raise CheckFailure(f"M{machine.number} CLI did not return one JSON envelope") from None
        if not isinstance(body, dict) or not isinstance(body.get("ok"), bool):
            raise CheckFailure(f"M{machine.number} CLI returned an invalid envelope")
        error = body.get("error", {})
        if not body["ok"] and (not isinstance(error, dict) or not isinstance(error.get("code"), str)):
            raise CheckFailure(f"M{machine.number} CLI returned an invalid error envelope")
        expected = not body["ok"] and error.get("code") in expected_errors
        self.record("cli", machine=machine.number, command=command, exit_status=output.returncode,
                    response=body, stderr=redact_text(output.stderr), expected_error=expected)
        if output.stderr:
            raise CheckFailure(f"M{machine.number} JSON command wrote diagnostics to stderr")
        if not body["ok"]:
            if expected:
                return None
            raise CheckFailure(f"M{machine.number} API error {error.get('code', 'missing_code')}")
        if output.returncode != 0 or "result" not in body:
            raise CheckFailure(f"M{machine.number} successful command had an unexpected exit/result")
        result = body["result"]
        if owner and "--plan" not in arguments:
            confirmed = confirmation_arguments(arguments, result)
            if confirmed is not None:
                return self.cli(machine, confirmed, owner=owner, session=session, local=local,
                                expected_errors=expected_errors)
        return result

    def wait(self, label, predicate):
        previous = self.deadline
        self.deadline = time.monotonic() + self.timeout
        self.record("wait_started", label=label, timeout_seconds=self.timeout)
        try:
            while True:
                for machine in self.machines:
                    if machine.process is not None and machine.process.poll() is not None:
                        raise CheckFailure(f"M{machine.number} daemon exited unexpectedly ({machine.process.returncode})")
                result = predicate()
                if result:
                    self.record("check_passed", label=label)
                    self.summary["checks"].append(label)
                    return result
                time.sleep(min(0.2, self.remaining()))
        except CheckFailure as error:
            raise CheckFailure(f"{label}: {error}") from None
        finally:
            self.deadline = previous

    def read_stderr(self, machine, process, generation):
        for line in process.stderr:
            fact = route_fact(line)
            if fact is not None:
                fact = self.record("route_snapshot", machine=machine.number,
                                   generation=generation, **fact)
                with self.lock:
                    self.routes.append(fact)
            else:
                # Public startup/error diagnostics only; never expose a raw
                # arbitrary daemon line in a reported artifact.
                if line.startswith("locust:") and not TICKET.search(line) and not SECRET_FIELD.search(line):
                    self.record("daemon_diagnostic", machine=machine.number,
                                generation=generation, text=redact_text(line.rstrip()))

    def start(self, machine):
        command = [str(self.binary), "--home", str(machine.home), "daemon", "run"]
        process = subprocess.Popen(command, env=self.environment(machine), stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        machine.process = process
        machine.reader = threading.Thread(target=self.read_stderr,
                                          args=(machine, process, machine.generation), daemon=True)
        machine.reader.start()
        self.record("daemon_started", machine=machine.number, generation=machine.generation,
                    pid=process.pid, command=command)

        def ready():
            result = self.cli(machine, ["status"], owner=True, expected_errors=("unavailable",))
            return result and variant(result, "status").get("endpoint") and result

        result = self.wait(f"M{machine.number} daemon endpoint ready", ready)
        current = identity(variant(result, "status")["endpoint"], "endpoint")
        if machine.endpoint is not None and current != machine.endpoint:
            raise CheckFailure(f"M{machine.number} endpoint identity changed after restart")
        machine.endpoint = current
        self.cli(machine, ["doctor"], owner=True)

    def stop(self, machine):
        if machine.process is None:
            return
        process = machine.process
        self.cli(machine, ["daemon", "stop"], owner=True)
        try:
            process.wait(timeout=self.timeout)
        except subprocess.TimeoutExpired:
            raise CheckFailure(f"M{machine.number} daemon did not exit after daemon stop") from None
        if process.returncode != 0:
            raise CheckFailure(f"M{machine.number} stopped daemon exited with {process.returncode}")
        machine.reader.join(timeout=self.timeout)
        process.stderr.close()
        machine.process = None
        self.record("daemon_stopped", machine=machine.number, generation=machine.generation,
                    exit_status=process.returncode)

    def restart(self, machine):
        self.stop(machine)
        machine.generation += 1
        self.start(machine)

    def goal_status(self, machine, goal):
        result = self.cli(machine, ["goal", "status", "--goal", goal],
                          expected_errors=("not_found", "unavailable"))
        return variant(result, "goal_status") if result else None

    def history(self, machine, goal):
        events, after = {}, 0
        while True:
            result = self.cli(machine, ["events", "--goal", goal, "--after", after, "--limit", 256])
            page = variant(result, "events")
            if not isinstance(page, list):
                raise CheckFailure("event feed was not an array")
            if not page:
                return events
            for event in page:
                event_id = identity(event.get("event"), "event")
                position = event.get("position")
                if not isinstance(position, int) or position <= after:
                    raise CheckFailure("event feed did not advance its explicit cursor")
                after = position
                events[event_id] = event

    def create_goal(self, machine, title):
        formation = self.cli(machine, ["formation", "example", "directed"], local=True)
        formation["context"]["inputs"] = {"snapshot": {"kind": "artifact", "required": False}}
        created = self.cli(machine, ["--agent", f"m{machine.number}", "goal", "create", "--title", title,
            "--formation-json", json.dumps(formation)], owner=True)
        return identity(variant(created, "goal_created")["goal"], "goal")

    def join_goal(self, machine, ticket):
        """Give one owner-reviewed join a private ticket file, then remove it."""
        ticket_file = machine.home / ("invite-" + secrets.token_hex(8))
        descriptor = os.open(ticket_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        try:
            with os.fdopen(descriptor, "w", encoding="utf-8") as output:
                output.write(ticket)
            return self.cli(machine, ["--agent", f"m{machine.number}", "goal", "join",
                                      "--ticket-file", ticket_file], owner=True)
        finally:
            ticket_file.unlink(missing_ok=True)

    def set_ask_levels(self, goal):
        for machine in self.machines:
            self.cli(machine, ["--agent", f"m{machine.number}", "level", "--goal", goal, "ask"], owner=True)

    def board_selected(self, machine, goal, task, result_id):
        result = self.cli(machine, ["board", "--goal", goal])
        return any(item.get("task") == task and item.get("completed") is True
                   and item.get("selected") == result_id for item in variant(result, "board"))

    def contributions_contain(self, machine, goal, expected):
        result = self.cli(machine, ["contributions", "--goal", goal])
        held = {note["contribution"]: note.get("text") for note in variant(result, "contributions")}
        return all(held.get(event) == text for event, text in expected.items())

    def recorded(self, machine, args, **options):
        event = identity(variant(self.cli(machine, args, **options), "recorded")["event"], "event")
        return event

    def execute(self):
        self.summary["binary"] = binary_facts(self.binary, self.timeout)
        if self.network == "local":
            require_local_multicast(self.summary["transport"])
        with tempfile.TemporaryDirectory(prefix="lct-", dir="/tmp") as directory:
            root = Path(directory)
            root.chmod(0o700)
            # The shared checkout's binary may be rebuilt during a run. Use
            # one verified private copy so every daemon/CLI executes the
            # exact image whose hash/version the evidence identifies.
            image = root / "locust"
            shutil.copyfile(self.binary, image)
            image.chmod(0o700)
            image_facts = binary_facts(image, self.timeout)
            if image_facts["sha256"] != self.summary["binary"]["sha256"]:
                raise CheckFailure("selected binary changed while taking its execution snapshot")
            self.summary["binary"]["execution_copy_path"] = str(image)
            self.binary = image
            for number in range(1, 4):
                home = root / f"m{number}"
                home.mkdir(mode=0o700)
                self.machines.append(Machine(number, home))
            try:
                self.flow()
            finally:
                self.cleanup()

    def flow(self):
        m1, m2, m3 = self.machines
        for machine in self.machines:
            self.start(machine)
            result = self.cli(machine, ["agent", "enroll", f"m{machine.number}"], owner=True)
            machine.agent = identity(variant(result, "agent_enrolled")["agent"], "principal")
        self.summary["machines"] = [{"machine": m.number, "home": str(m.home), "principal": m.agent,
                                     "endpoint": m.endpoint} for m in self.machines]
        self.phase = "join"
        title = "Local three-process T1 qualification"
        goal = self.create_goal(m1, title)
        self.summary["goal"] = goal
        for invitee in (m2, m3):
            ticket = variant(self.cli(m1, ["goal", "invite", "--goal", goal], owner=True), "invited")["ticket"]
            joined = variant(self.join_goal(invitee, ticket), "joined")
            del ticket
            if joined.get("goal") != goal:
                raise CheckFailure("join answered with another goal")

        expected_members = {machine.agent for machine in self.machines}
        def admitted():
            states = [self.goal_status(machine, goal) for machine in self.machines]
            return all(state and not state.get("halted") and state.get("title") == title
                       and {member["member"] for member in state["members"]} == expected_members
                       for state in states)
        self.wait("all three replicas admit all three principals and decrypt title", admitted)
        self.set_ask_levels(goal)
        self.wait("all three daemons report a selected peer path", lambda: all(
            any(fact["machine"] == machine.number and any(path["selected"]
                                                        for path in fact["paths"]) for fact in self.routes)
            for machine in self.machines))

        self.phase = "task"
        task_text = "Return a deterministic local qualification result."
        summary = "Completed the deterministic local qualification task."
        task = "task:" + self.recorded(m1, ["task", "open", "--goal", goal, task_text])
        offer = self.recorded(m1, ["work", "offer", "--goal", goal, "--task", task, "--member", m2.agent])
        self.summary["events"].update(task=task, offer=offer)
        self.wait("M2 receives offer", lambda: any(
            item.get("task") == task and item.get("offer") == offer
            for item in variant(self.cli(m2, ["pending", "--goal", goal]), "pending")["ask_first"]))
        self.cli(m2, ["--agent", "m2", "allow", "--goal", goal, "--task", task], owner=True)
        session_path = m2.home / "sessions" / "qualification.secret"
        session = self.cli(m2, ["session", "create", session_path], local=True)
        if session_path.stat().st_mode & 0o7777 != 0o600:
            raise CheckFailure("explicit execution session file was not mode 0600")
        self.summary["session"] = session
        claim = variant(self.cli(m2, ["attempt", "start", "--goal", goal, "--task", task, "--offer", offer], session=session_path), "claimed")
        if claim.get("task") != task or claim.get("instance") != session.get("instance"):
            raise CheckFailure("claim did not bind the requested task/session")
        result_id = self.recorded(m2, ["contribution", "publish", "--goal", goal, "--attempt", claim["attempt"],
                                      "--generation", claim["generation"], summary], session=session_path)
        self.summary["events"]["result"] = result_id
        def result_held(machine):
            output = self.cli(machine, ["event", "show", "--goal", goal, "--event", result_id], expected_errors=("not_found",))
            if not output:
                return False
            detail = variant(output, "event")
            return detail.get("text") == summary and detail["view"]["kind"] == "contribution_published" and all(
                content.get("state") == "held" for content in detail["content"])
        self.wait("M1 reads/decrypts the actual submitted result", lambda: result_held(m1))
        self.recorded(m1, ["review", "record", "--goal", goal, "--subject", result_id, "--verdict", "approve", "Verified deterministic result"])
        accepted = self.recorded(m1, ["scope", "select", "--goal", goal, "--subject", result_id])
        self.summary["events"]["acceptance"] = accepted
        self.wait("all replicas show accepted task and submitted content", lambda: all(
            self.board_selected(machine, goal, task, result_id) and result_held(machine)
            for machine in self.machines))
        baseline = self.history(m1, goal)
        self.wait("M3 holds the full effective task history", lambda: set(self.history(m3, goal)) == set(baseline))
        self.summary["task_history"] = sorted(baseline)
        self.cli(m1, ["pending", "--goal", goal])

        self.phase = "coordinator_offline"
        self.stop(m1)
        # Restart a non-coordinator while the coordinator is absent, removing
        # its live connection/discovery cache before peer-only reconciliation.
        self.restart(m3)
        note2_text, note3_text = "M2 note while coordinator is stopped.", "M3 note while coordinator is stopped."
        note2 = self.recorded(m2, ["contribution", "publish", "--goal", goal, note2_text])
        note3 = self.recorded(m3, ["contribution", "publish", "--goal", goal, note3_text])
        notes = {note2: note2_text, note3: note3_text}
        self.summary["events"].update(note_m2=note2, note_m3=note3)
        self.wait("M2 and M3 exchange/decrypt notes without coordinator", lambda: all([
            self.contributions_contain(machine, goal, notes) for machine in (m2, m3)]))
        self.wait("restarted M3 reports a selected non-coordinator path", lambda: any(
            fact["machine"] == 3 and fact["generation"] == m3.generation
            and fact["peer_endpoint"] == m2.endpoint and any(path["selected"] for path in fact["paths"])
            for fact in self.routes))
        self.phase = "coordinator_catchup"
        m1.generation += 1
        self.start(m1)
        self.wait("M1 restart catches up with both offline notes", lambda: self.contributions_contain(m1, goal, notes))
        expected_history = set(self.history(m2, goal))
        self.wait("all replicas retain complete effective history", lambda: all(
            set(self.history(machine, goal)) == expected_history for machine in self.machines))

        self.phase = "sequential_restarts"
        for machine in self.machines:
            self.restart(machine)
            self.wait(f"M{machine.number} preserves principal, goal, history, accepted task and notes", lambda: (
                variant(self.cli(machine, ["status"], owner=True), "status")["endpoint"] == machine.endpoint
                and any(agent["agent"] == machine.agent for agent in variant(self.cli(machine, ["status"], owner=True), "status")["agents"])
                and admitted() and set(self.history(machine, goal)) == expected_history
                and self.board_selected(machine, goal, task, result_id)
                and self.contributions_contain(machine, goal, notes) and result_held(machine)))
        self.summary["final_history"] = sorted(expected_history)
        self.summary["endpoint_identity_preserved"] = True
        self.summary["status"] = "passed"

    def cleanup(self):
        self.phase = "cleanup"
        for machine in reversed(self.machines):
            process = machine.process
            if process is None:
                continue
            try:
                if process.poll() is None:
                    self.stop(machine)
                else:
                    raise CheckFailure(f"M{machine.number} daemon had already exited ({process.returncode})")
            except Exception as error:
                self.summary["failures"].append(redact_text(str(error)))
                self.summary["status"] = "failed"
                code = process_stop(process, self.timeout)
                if machine.reader:
                    machine.reader.join(timeout=self.timeout)
                if process.stderr:
                    process.stderr.close()
                machine.process = None
                self.record("daemon_forced_cleanup", machine=machine.number, exit_status=code)

    def run(self):
        try:
            self.execute()
        except Exception as error:
            self.summary["status"] = "failed"
            self.summary["failures"].append(redact_text(str(error)))
            self.record("qualification_failed", error_kind=type(error).__name__, message=redact_text(str(error)))
        finally:
            self.summary["finished_at"] = self.timestamp()
            self.summary["route_facts"] = self.routes
            (self.artifact_dir / "summary.json").write_text(
                json.dumps(redact(self.summary), indent=2, sort_keys=True) + "\n", encoding="utf-8")
            self.transcript.close()
        return self.summary["status"] == "passed"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path, help="absolute path to the exact binary to qualify")
    parser.add_argument("--timeout-seconds", type=float, default=60, help="deadline for each qualification wait/command (default: 60)")
    parser.add_argument("--network", choices=("default", "local"), default="default", help="daemon defaults, or relay-free local multicast reproduction")
    args = parser.parse_args(argv)
    if not args.binary.is_absolute() or not args.binary.is_file() or not os.access(args.binary, os.X_OK):
        parser.error("--binary must name an executable file by its absolute path")
    if not math.isfinite(args.timeout_seconds) or args.timeout_seconds <= 0:
        parser.error("--timeout-seconds must be finite and positive")
    root = Path(__file__).resolve().parents[1]
    name = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + os.urandom(4).hex()
    artifacts = root / "output" / "t1-local" / name
    qualification = Qualification(args.binary.resolve(), args.timeout_seconds, artifacts, args.network)
    passed = qualification.run()
    print(json.dumps({"ok": passed, "qualification": "local three-process gate; not three-Mac T1",
                      "summary": str(artifacts / "summary.json"), "transcript": str(artifacts / "transcript.jsonl")}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
