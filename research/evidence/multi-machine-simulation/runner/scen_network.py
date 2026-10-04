"""Scenarios 5 and 6: a daemon that comes back on another port, and the
two-machine flow under each relay/lookup combination."""

import socket
import time

from simlib import CheckFailure, identity, variant
import flows

NONE = {"LOCUST_RELAY": "none"}
N0 = {}  # unset LOCUST_RELAY is the n0 default
# (name, environment, uses the Internet, a stale-address join can work on one host)
COMBOS = [
    ("relay-none/lookup-none", {**NONE, "LOCUST_LOOKUP": "none"}, False, False),
    ("relay-none/lookup-local", {**NONE, "LOCUST_LOOKUP": "local"}, False, True),
    ("relay-none/lookup-mainline", {**NONE, "LOCUST_LOOKUP": "mainline"}, True, False),
    ("relay-none/lookup-all", {**NONE, "LOCUST_LOOKUP": "all"}, True, True),
    ("relay-n0/lookup-none", {**N0, "LOCUST_LOOKUP": "none"}, True, None),  # see STALE_SETTLE
    ("relay-n0/lookup-local", {**N0, "LOCUST_LOOKUP": "local"}, True, True),
    ("relay-n0/lookup-mainline", {**N0, "LOCUST_LOOKUP": "mainline"}, True, True),
    ("defaults (relay-n0/lookup-all)", {}, True, True),
]
# new-address: daemons persist no contact hints for each other (only a joiner keeps
# the ticket's hints for the coordinator), so two members that are not the
# coordinator can find each other only through a lookup service; with lookup
# "none" only the moved node's own dial to the coordinator reconnects anything,
# relay or not. Those combinations are kept as negative controls.
MOVE_COMBOS = [(name, env, internet, env.get("LOCUST_LOOKUP", "all") != "none")
               for name, env, internet, _ in (COMBOS[0], COMBOS[1], COMBOS[4], COMBOS[6], COMBOS[7])]


def free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as probe:
        probe.bind(("0.0.0.0", 0))
        return probe.getsockname()[1]


def bound(env, port):
    return {**env, "LOCUST_BIND": f"0.0.0.0:{port}"}


def stop_all(c, machines):
    for machine in machines:
        if machine.process is not None:
            c.stop(machine)


def attempt(c, entry, body):
    """Run one sub-case; record its failure instead of aborting the scenario."""
    began = time.monotonic()
    try:
        body()
        entry["passed"] = True
    except CheckFailure as error:
        entry["passed"] = False
        entry["failure"] = str(error)
    entry["seconds"] = round(time.monotonic() - began, 1)


def new_address(c):
    """Restart the worker, then the coordinator, on a different port; the others
    must reconnect directly to it and notes must flow both ways."""
    results = c.summary.setdefault("results", [])
    for name, env, internet, can_work in MOVE_COMBOS:
        c.phase = f"move {name}"
        first = [free_port() for _ in range(3)]
        machines = [c.add(env=bound(env, port)) for port in first]
        m1, m2, m3 = machines
        ports = dict(zip((m.number for m in machines), first))
        entry = {"combo": name, "internet": internet, "expected_to_work": can_work, "moves": []}
        results.append(entry)

        def body():
            flows.boot(c, machines)
            goal = flows.found(c, m1, f"Move {name}")
            flows.invite_join(c, m1, m2, goal, f"Move {name}", [m1, m2], 60)
            flows.invite_join(c, m1, m3, goal, f"Move {name}", machines, 60)
            for mover in (m2, m1):
                others = [m for m in machines if m is not mover]
                move = {"machine": mover.number, "from": ports[mover.number]}
                entry["moves"].append(move)
                c.stop(mover)
                c.wait(f"{name}: the others see M{mover.number} gone", lambda: all(
                    flows.peer_disconnected(c, o, goal, mover) for o in others), 30)
                ports[mover.number] = free_port()
                move["to"] = ports[mover.number]
                mark = len(c.routes)
                since_ms = int(time.time() * 1000)
                c.relaunch(mover, bound(env, ports[mover.number]))
                moved = time.monotonic()
                for other in others:
                    leg = f"M{other.number}->M{mover.number}"
                    try:
                        c.wait(f"{name}: M{other.number} reconnects to moved M{mover.number}",
                               lambda: flows.peer_connected(c, other, goal, mover, since_ms), 120)
                        move[leg] = round(time.monotonic() - moved, 1)
                    except CheckFailure as error:
                        move[leg] = f"not reconnected: {error}"
                notes = {}
                for writer in machines:
                    text = f"{name}: M{writer.number} after M{mover.number} moved"
                    notes[flows.add_note(c, writer, goal, text)] = text
                flows.notes_everywhere(c, machines, goal, notes, f"{name}: notes flow after M{mover.number} moved", 120)
                move["route_kinds_after_move"] = {
                    f"M{m.number}": [p["kind"] for fact in c.routes[mark:] if fact["machine"] == m.number
                                     and fact["peer_endpoint"] in {mover.endpoint} | ({o.endpoint for o in others}
                                                                                     if m is mover else set())
                                     for p in fact["paths"] if p["selected"]] for m in machines}
                missing = [k for k, v in move.items() if "->" in k and isinstance(v, str)]
                if missing:
                    raise CheckFailure(f"{name}: legs never reconnected after M{mover.number} moved: {missing}")
            flows.not_halted(c, machines, goal)

        attempt(c, entry, body)
        stop_all(c, machines)
    bad = [e["combo"] for e in results if e["passed"] != e["expected_to_work"]]
    if bad:
        raise CheckFailure(f"new-address outcome differs from expectation for {bad}")


def network_modes(c):
    """For each relay/lookup combination: the two-machine flow with a fresh ticket,
    and a join whose ticket names the coordinator's previous port."""
    results = c.summary.setdefault("results", [])
    for name, env, internet, stale_can_work in COMBOS:
        c.phase = f"modes {name}"
        fresh = {"combo": name, "case": "fresh-ticket", "internet": internet, "expected_to_work": True}
        results.append(fresh)
        m1, m2 = c.add(env=env), c.add(env=env)

        def fresh_body():
            flows.boot(c, [m1, m2])
            goal = flows.found(c, m1, f"Modes {name}")
            fresh["join_seconds"] = flows.invite_join(c, m1, m2, goal, f"Modes {name}", [m1, m2], 60)
            flows.complete_task(c, m1, m2, goal, [m1, m2], 60, label="T1")

        attempt(c, fresh, fresh_body)
        fresh["selected_paths"] = {f"M{m.number}": c.selected_kinds(m) for m in (m1, m2)}
        stop_all(c, [m1, m2])

        for settle, can_work in STALE_SETTLE.get(name, ((0, stale_can_work),)):
            stale_case(c, results, name, env, internet, can_work, settle)
    bad = [f"{e['combo']} {e['case']}" for e in results if e["passed"] != e["expected_to_work"]]
    if bad:
        raise CheckFailure(f"network-modes outcome differs from expectation for {bad}")


# Extra stale-ticket run that invites only after the coordinator has run a while.
# Observed on the candidate: a ticket issued within about a second of daemon start
# carries one IP hint and no relay URL (the relay is not connected yet); 8 s later
# it carries a relay URL, so with lookup "none" only the later ticket survives
# the coordinator moving.
STALE_SETTLE = {"relay-n0/lookup-none": ((0, False), (8, True))}


def stale_case(c, results, name, env, internet, stale_can_work, settle):
    stale = {"combo": name, "case": f"stale-ticket (coordinator moved after invite; invited {settle} s after start)",
             "internet": internet, "expected_to_work": stale_can_work}
    results.append(stale)
    m3, m4 = c.add(env=bound(env, free_port())), c.add(env=env)

    def stale_body():
        flows.boot(c, [m3, m4])
        title = f"Stale {name}"
        goal = flows.found(c, m3, title)
        time.sleep(settle)
        ticket = variant(c.cli(m3, ["goal", "invite", "--goal", goal]), "invited")["ticket"]
        stale["ticket_hint_kinds"] = flows.hint_kinds(ticket)
        c.stop(m3)
        c.relaunch(m3, bound(env, free_port()))
        joined = variant(c.cli(m4, ["goal", "join", "--ticket", ticket]), "joined")
        del ticket
        identity(joined.get("goal"), "goal")
        sent = time.monotonic()
        c.wait(f"{name}: join with a stale-address ticket admitted", lambda: flows.members_ok(
            c, [m3, m4], goal, title, {m3.agent, m4.agent}), 75)
        stale["join_seconds"] = round(time.monotonic() - sent, 2)

    attempt(c, stale, stale_body)
    stale["selected_paths"] = {f"M{m.number}": c.selected_kinds(m) for m in (m3, m4)}
    stop_all(c, [m3, m4])


SCENARIOS = {"new-address": new_address, "network-modes": network_modes}
