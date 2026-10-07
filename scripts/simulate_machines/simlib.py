"""Several simulated machines as several daemon processes of the real binary.

Builds on scripts/check_t1.py: its redaction, CLI envelope checks, transcript
and stderr route parsing are reused unchanged. This module adds per-machine
binaries and network overrides, per-wait deadlines, kill -9, SIGSTOP/SIGCONT
and fixed short homes under /tmp/locust-sim-<n>. Python 3.10, standard library
only.
"""

import itertools
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import threading
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent  # the repository
sys.path.insert(0, str(HERE.parent))
import check_t1 as base  # noqa: E402
from owner_plans import confirmation_arguments  # noqa: E402

CheckFailure = base.CheckFailure
identity, variant, redact_text = base.identity, base.variant, base.redact_text
# Set by run.py from --binary and --mixed-binary. "mixed" is a second build
# with another version line, for the mixed-build scenario only.
BINARIES = {}
HOME_PREFIX = "/tmp/locust-sim-"
OVERRIDES = ("LOCUST_CREDENTIAL", "LOCUST_SESSION", "LOCUST_BIND", "LOCUST_RELAY", "LOCUST_LOOKUP")
# Network profiles for scenarios that do not choose their own transport.
# "lan": no relay, local-network (multicast) lookup only; no Internet traffic.
# "isolated": no relay and no lookup; peers know only the ticket's hints.
# "defaults": the daemon's own n0 relays and local+Mainline lookup, as on the
# real Macs (needs the Internet). The daemon's hard-wired port mapping may still
# probe the local gateway in every profile.
PROFILES = {"lan": {"LOCUST_RELAY": "none", "LOCUST_LOOKUP": "local"},
            "isolated": {"LOCUST_RELAY": "none", "LOCUST_LOOKUP": "none"},
            "defaults": {}}
_homes = itertools.count(1)
LIVE = set()  # every daemon Popen this process started and has not reaped


def multicast_only(env):
    """No relay and local lookup alone: members find each other only by multicast."""
    return env.get("LOCUST_RELAY") == "none" and env.get("LOCUST_LOOKUP") == "local"


def marks_of(home):
    """The marks directory a daemon keeps beside its home (`marks_dir` in
    crates/locust-proto/src/local.rs). It is part of the home's state."""
    return home.with_name(home.name + ".marks")


def fresh_home():
    """A short private home, claimed atomically; an existing one is never reused,
    nor one beside an earlier daemon's marks directory."""
    while True:
        home = Path(f"{HOME_PREFIX}{next(_homes)}")
        try:
            home.mkdir(mode=0o700)
        except FileExistsError:
            continue
        if marks_of(home).exists():
            home.rmdir()
            continue
        return home


def _commands():
    out = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True, check=True)
    return [line.strip().split(None, 1) for line in out.stdout.splitlines() if line.strip()]


def clean_stale_homes():
    """Remove homes left by an aborted earlier run, but only when no process at
    all names a home with this prefix and no other copy of this runner runs.
    (An earlier version used `pgrep -f "--home ..."`, which pgrep parsed as an
    option, and deleted a concurrent run's live homes.)"""
    me = str(os.getpid())
    for pid, *rest in _commands():
        command = rest[0] if rest else ""
        if pid != me and (HOME_PREFIX in command or ("simulate_machines/run.py" in command and "python" in command)):
            return []
    removed = []
    for home in sorted(Path("/tmp").glob(Path(HOME_PREFIX).name + "*")):
        shutil.rmtree(home, ignore_errors=True)
        removed.append(str(home))
    return removed


class Machine(base.Machine):
    def __init__(self, number, home, binary="candidate", env=None):
        super().__init__(number, home)
        self.binary = BINARIES[binary]
        self.binary_name = binary
        self.env = dict(env or {})
        self.paused = False


class Cluster(base.Qualification):
    """One scenario's simulated machines; every wait has its own deadline."""

    def __init__(self, name, artifact_dir, timeout=90, profile="lan"):
        super().__init__(BINARIES["candidate"], timeout, artifact_dir, network="default")
        self.name = name
        self.profile = profile
        self.summary["qualification"] = f"procs simulation scenario {name}; one Mac, not multi-machine T1"
        self.summary["transport"] = {"profile": profile, "per_machine": {}}
        self.timings = {}
        self.marks = {}

    # -- machines -------------------------------------------------------
    def add(self, binary="candidate", env=None):
        env = PROFILES[self.profile] if env is None else env
        machine = Machine(len(self.machines) + 1, fresh_home(), binary, env)
        self.machines.append(machine)
        self.summary["transport"]["per_machine"][f"M{machine.number}"] = {
            "binary": binary, **{k: v for k, v in machine.env.items()}}
        return machine

    def environment(self, machine):
        env = os.environ.copy()
        for key in OVERRIDES:
            env.pop(key, None)
        env["LOCUST_HOME"] = str(machine.home)
        env.update(machine.env)
        return env

    def cli(self, machine, arguments, *, owner=False, session=None, local=False,
            expected_errors=(), tolerate=False, timeout=None):
        """check_t1's CLI call with this machine's binary. tolerate=True returns
        ('error', code) instead of raising, for calls racing a kill."""
        if machine.paused:
            raise CheckFailure(f"harness bug: CLI call to paused M{machine.number}")
        command = [str(machine.binary), "--home", str(machine.home), "--json"]
        if owner:
            command.append("--owner")
        elif not local:
            command.extend(["--credential", str(machine.credential)])
        if session is not None:
            command.extend(["--session", str(session)])
        command.extend(map(str, arguments))
        try:
            output = subprocess.run(command, env=self.environment(machine), capture_output=True,
                                    text=True, timeout=timeout or self.remaining(), check=False)
        except subprocess.TimeoutExpired:
            self.record("cli_timeout", machine=machine.number, command=command)
            if tolerate:
                return ("error", "timeout")
            raise CheckFailure(f"M{machine.number} CLI command timed out") from None
        try:
            body = json.loads(output.stdout)
        except (json.JSONDecodeError, UnicodeError):
            self.record("invalid_cli_json", machine=machine.number, command=command,
                        exit_status=output.returncode, stderr=redact_text(output.stderr)[:400])
            if tolerate:
                return ("error", f"no_json_exit_{output.returncode}")
            raise CheckFailure(f"M{machine.number} CLI did not return one JSON envelope") from None
        if not isinstance(body, dict) or not isinstance(body.get("ok"), bool):
            raise CheckFailure(f"M{machine.number} CLI returned an invalid envelope")
        error = body.get("error", {}) if isinstance(body.get("error"), dict) else {}
        expected = not body["ok"] and error.get("code") in expected_errors
        self.record("cli", machine=machine.number, command=command, exit_status=output.returncode,
                    response=body, stderr=redact_text(output.stderr), expected_error=expected)
        if not body["ok"]:
            if expected:
                return None
            if tolerate:
                return ("error", error.get("code", "missing_code"))
            raise CheckFailure(f"M{machine.number} API error {error.get('code', 'missing_code')}: "
                               f"{redact_text(str(error.get('message', '')))[:200]}")
        if output.stderr and not tolerate:
            raise CheckFailure(f"M{machine.number} JSON command wrote diagnostics to stderr")
        result = body["result"]
        if owner and "--plan" not in arguments:
            confirmed = confirmation_arguments(arguments, result)
            if confirmed is not None:
                return self.cli(machine, confirmed, owner=owner, session=session, local=local,
                                expected_errors=expected_errors, tolerate=tolerate,
                                timeout=timeout)
        return result

    def wait(self, label, predicate, timeout=None):
        """check_t1's wait with a per-wait deadline; records its duration."""
        saved = self.timeout
        self.timeout = timeout or saved
        started = time.monotonic()
        try:
            result = super().wait(label, predicate)
        finally:
            self.timeout = saved
            self.timings[label] = round(time.monotonic() - started, 2)
        return result

    def start(self, machine, ready_timeout=None):
        command = [str(machine.binary), "--home", str(machine.home), "daemon", "run"]
        process = subprocess.Popen(command, env=self.environment(machine), stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        LIVE.add(process)
        machine.process = process
        machine.paused = False
        machine.reader = threading.Thread(target=self.read_stderr,
                                          args=(machine, process, machine.generation), daemon=True)
        machine.reader.start()
        self.record("daemon_started", machine=machine.number, generation=machine.generation,
                    pid=process.pid, command=command, env={k: v for k, v in machine.env.items()})

        def ready():
            result = self.cli(machine, ["status"], owner=True, expected_errors=("unavailable",))
            return result and variant(result, "status").get("endpoint") and result

        result = self.wait(f"M{machine.number} gen {machine.generation} endpoint ready", ready,
                           ready_timeout)
        current = identity(variant(result, "status")["endpoint"], "endpoint")
        if machine.endpoint is not None and current != machine.endpoint:
            raise CheckFailure(f"M{machine.number} endpoint identity changed after restart")
        machine.endpoint = current
        self.cli(machine, ["doctor"], owner=True)

    def stop(self, machine, grace=20):
        """`daemon stop`, as check_t1 does; a daemon that does not exit within
        `grace` seconds is sampled with macOS `sample` (stacks saved next to the
        transcript), then sent SIGTERM, sampled again and killed; that is a failure."""
        process = machine.process
        if process is None:
            return
        self.cli(machine, ["daemon", "stop"], owner=True)
        try:
            process.wait(timeout=grace)
        except subprocess.TimeoutExpired:
            stuck = []
            for signal_number in (signal.SIGTERM, signal.SIGKILL):
                path = self.artifact_dir / f"stuck-M{machine.number}-gen{machine.generation}-{len(stuck)}.sample.txt"
                subprocess.run(["sample", str(process.pid), "3", "-file", str(path)],
                               capture_output=True, check=False, timeout=60)
                # `sample` may get no call graph without task access; keep thread
                # states, open descriptors and whether the socket still answers.
                with path.with_suffix(".state.txt").open("w") as state:
                    for probe in (["ps", "-M", "-p", str(process.pid)], ["lsof", "-nP", "-p", str(process.pid)]):
                        out = subprocess.run(probe, capture_output=True, text=True, check=False, timeout=30)
                        state.write(f"$ {' '.join(probe)}\n{out.stdout}{out.stderr}\n")
                    answer = self.cli(machine, ["status"], owner=True, tolerate=True, timeout=5)
                    state.write(f"status while stuck: {'answered' if isinstance(answer, dict) else answer}\n")
                stuck.append(path.name)
                os.kill(process.pid, signal_number)
                try:
                    process.wait(timeout=grace)
                    break
                except subprocess.TimeoutExpired:
                    continue
            process.wait()
            machine.reader.join(timeout=10)
            process.stderr.close()
            LIVE.discard(process)
            machine.process = None
            self.record("daemon_stop_hung", machine=machine.number, generation=machine.generation,
                        exit_status=process.returncode, samples=stuck)
            raise CheckFailure(f"M{machine.number} acknowledged daemon stop but did not exit within "
                               f"{grace}s (exit {process.returncode} after signals; stacks: {stuck})")
        if process.returncode != 0:
            raise CheckFailure(f"M{machine.number} stopped daemon exited with {process.returncode}")
        machine.reader.join(timeout=10)
        process.stderr.close()
        LIVE.discard(process)
        machine.process = None
        self.record("daemon_stopped", machine=machine.number, generation=machine.generation,
                    exit_status=process.returncode)

    def kill9(self, machine):
        """SIGKILL: no flush, no socket cleanup, no goodbye to peers."""
        process = machine.process
        os.kill(process.pid, signal.SIGKILL)
        process.wait(timeout=10)
        machine.reader.join(timeout=10)
        process.stderr.close()
        LIVE.discard(process)
        machine.process = None
        machine.paused = False
        self.record("daemon_killed", machine=machine.number, generation=machine.generation,
                    exit_status=process.returncode)

    def relaunch(self, machine, env=None):
        """Start again on the same home (after kill9 or stop), optionally with new overrides."""
        if env is not None:
            machine.env = dict(env)
        machine.generation += 1
        self.start(machine)

    def pause(self, machine):
        os.kill(machine.process.pid, signal.SIGSTOP)
        machine.paused = True
        self.record("daemon_sigstop", machine=machine.number, generation=machine.generation)

    def resume(self, machine):
        os.kill(machine.process.pid, signal.SIGCONT)
        machine.paused = False
        self.record("daemon_sigcont", machine=machine.number, generation=machine.generation)

    def mark(self, label):
        self.marks[label] = round(time.monotonic() - self.t0, 2)

    # -- facts ----------------------------------------------------------
    def routes_for(self, machine, generation=None, peer=None):
        with self.lock:
            return [fact for fact in self.routes if fact["machine"] == machine.number
                    and (generation is None or fact["generation"] == generation)
                    and (peer is None or fact["peer_endpoint"] == peer)]

    def selected_kinds(self, machine, generation=None, peer=None):
        kinds = []
        for fact in self.routes_for(machine, generation, peer):
            kinds.extend(p["kind"] for p in fact["paths"] if p["selected"])
        return kinds

    def finding_ids(self, machine, goal):
        result = self.cli(machine, ["contributions", "--goal", goal])
        return {note["contribution"]: note.get("text") for note in variant(result, "contributions")}

    def halted(self, machine, goal):
        state = self.goal_status(machine, goal)
        return state is None or state.get("halted") is not None

    # -- lifecycle ------------------------------------------------------
    def execute_scenario(self, body):
        self.summary["binaries"] = {name: base.binary_facts(path, 30) for name, path in BINARIES.items()}
        self.t0 = time.monotonic()
        try:
            if multicast_only(PROFILES[self.profile]):
                base.require_local_multicast(self.summary["transport"])
            body(self)
            self.summary["status"] = "passed"
        finally:
            self.cleanup()
            for machine in self.machines:
                self.reap(machine)
                shutil.rmtree(machine.home, ignore_errors=True)
                shutil.rmtree(marks_of(machine.home), ignore_errors=True)

    def cleanup(self):
        for machine in self.machines:
            if machine.paused and machine.process is not None:
                self.resume(machine)
        super().cleanup()

    def reap(self, machine):
        process = machine.process
        if process is not None:
            base.process_stop(process, 10)
            LIVE.discard(process)
            machine.process = None

    def run_scenario(self, body):
        try:
            self.execute_scenario(body)
        except Exception as error:  # noqa: BLE001 - every failure is evidence
            self.summary["status"] = "failed"
            self.summary["failures"].append(redact_text(f"{type(error).__name__}: {error}"))
            self.record("scenario_failed", error_kind=type(error).__name__, message=redact_text(str(error)))
        finally:
            self.summary["finished_at"] = self.timestamp()
            self.summary["route_facts"] = self.routes
            self.summary["wait_seconds"] = self.timings
            self.summary["marks_seconds"] = self.marks
            (self.artifact_dir / "summary.json").write_text(
                json.dumps(base.redact(self.summary), indent=2, sort_keys=True) + "\n", encoding="utf-8")
            self.transcript.close()
        return self.summary["status"] == "passed"


def reap_all():
    """Last-resort reaper for every daemon this Python process started."""
    for process in list(LIVE):
        try:
            os.kill(process.pid, signal.SIGCONT)
        except ProcessLookupError:
            pass
        base.process_stop(process, 5)
        LIVE.discard(process)
