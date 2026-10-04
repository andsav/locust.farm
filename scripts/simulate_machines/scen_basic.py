"""Scenarios 1, 2 and 7: the two-machine flow carried to the end, the existing
three-machine sequence as a baseline, and a mixed build."""

import subprocess

from simlib import BINARIES, CheckFailure, variant
import simlib
import flows


def two_machine_flow(c, m1, m2, title="Two Mac T1", restarts=True, timeout=None):
    """The guide's two-Mac run: join, coordination note, task to acceptance on the
    worker, then offline note and catch-up and one restart each (guide steps 1-4)."""
    flows.boot(c, [m1, m2])
    c.phase = "join"
    goal = flows.found(c, m1, title)
    flows.invite_join(c, m1, m2, goal, title, [m1, m2], timeout)
    c.summary["join_paths"] = {f"M{m.number}": c.selected_kinds(m) for m in (m1, m2)}
    c.phase = "note"
    first = {flows.add_note(c, m1, goal, "Coordination note from M1."): "Coordination note from M1."}
    flows.notes_everywhere(c, [m2], goal, first, "M2 decrypts M1's coordination note", timeout)
    c.phase = "task"
    events = flows.complete_task(c, m1, m2, goal, [m1, m2], timeout)
    c.mark("task_accepted_everywhere")
    if not restarts:
        return goal, events
    c.phase = "coordinator_offline"
    c.stop(m1)
    text = "M2 wrote this while M1 was offline"
    offline = {flows.add_note(c, m2, goal, text): text}
    if not c.notes_contain(m2, goal, offline):
        raise CheckFailure("M2 cannot read its own offline note")
    c.relaunch(m1)
    flows.notes_everywhere(c, [m1], goal, offline, "M1 catches up with M2's offline note", timeout)
    c.phase = "sequential_restarts"
    history = flows.same_history(c, [m1, m2], goal, "M1 and M2 hold one history", timeout)
    for machine in (m2, m1):
        c.stop(machine)
        c.relaunch(machine)
        c.wait(f"M{machine.number} keeps identity, membership, task and notes after restart", lambda: (
            any(a["agent"] == machine.agent for a in variant(c.cli(machine, ["status"], owner=True), "status")["agents"])
            and flows.members_ok(c, [machine], goal, title, {m1.agent, m2.agent})
            and set(c.history(machine, goal)) == history
            and c.board_accepted(machine, goal, events["task"], events["result"])
            and c.notes_contain(machine, goal, {**first, **offline})), timeout)
        other = m2 if machine is m1 else m1
        c.wait(f"restarted M{machine.number} reconnects to M{other.number}",
               lambda: flows.peer_connected(c, machine, goal, other), timeout)
    c.summary["final_history_size"] = len(history)
    return goal, events


def two_machines_complete(c):
    m1, m2 = c.add(), c.add()
    two_machine_flow(c, m1, m2)


def three_machines(c):
    """The repository harness's own sequence (check_t1.Qualification.flow), unchanged,
    run on three simulated machines with homes under /tmp/locust-sim-<n>."""
    for _ in range(3):
        c.add()
    c.flow()


def mixed_build(c):
    m1, m2 = c.add("candidate"), c.add("mixed")
    lines = {}
    for name in ("candidate", "mixed"):
        out = subprocess.run([str(BINARIES[name]), "--version"], capture_output=True, text=True, check=True)
        lines[name] = out.stdout.strip()
    c.summary["version_lines"] = lines
    if lines["candidate"] == lines["mixed"]:
        raise CheckFailure("mixed build reports the same version line as the candidate")
    two_machine_flow(c, m1, m2, title="Mixed build T1")
    # Each daemon reports its own build in status; record both as seen through the CLI.
    versions = {f"M{m.number}": variant(c.cli(m, ["status"], owner=True), "status").get("daemon_version")
                for m in (m1, m2)}
    c.summary["daemon_versions"] = versions
    if versions["M1"] == versions["M2"] or None in versions.values():
        raise CheckFailure(f"daemons did not report two different builds: {versions}")


SCENARIOS = {"two-machines-complete": two_machines_complete,
             "three-machines": three_machines,
             "mixed-build": mixed_build}
_ = simlib
