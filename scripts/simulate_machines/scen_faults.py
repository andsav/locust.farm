"""Scenarios 3 and 4: a frozen process (SIGSTOP) and a killed process (kill -9)."""

import threading
import time

from simlib import CheckFailure
import flows

# Catch-up after a long outage can take a full backoff (up to 60 s) plus one
# anti-entropy round (30 s); these deadlines leave room for both.
CATCHUP = 180
SLEEP_CASES = (("worker", 45), ("worker", 120), ("coordinator", 45), ("coordinator", 120))


def three_member_goal(c, title, count=3):
    machines = [c.add() for _ in range(count)]
    flows.boot(c, machines)
    goal = flows.found(c, machines[0], title)
    for joiner in machines[1:]:
        everyone = machines[:machines.index(joiner) + 1]
        flows.invite_join(c, machines[0], joiner, goal, title, everyone)
    return machines, goal


def sleep(c):
    """SIGSTOP one daemon past the 30 s network deadlines while the others keep
    writing; after SIGCONT it must catch up and its own writes must spread."""
    (m1, m2, m3), goal = three_member_goal(c, "Sleep simulation")
    victims = {"worker": m2, "coordinator": m1}
    results = c.summary.setdefault("results", [])
    for round_number, (role, seconds) in enumerate(SLEEP_CASES, 1):
        victim = victims[role]
        others = [m for m in (m1, m2, m3) if m is not victim]
        c.phase = f"sleep_{role}_{seconds}s"
        entry = {"role": role, "machine": victim.number, "stopped_seconds": seconds}
        results.append(entry)
        c.pause(victim)
        began, written, latencies, i = time.monotonic(), {}, [], 0
        while time.monotonic() - began < seconds:
            writer = others[i % 2]
            text = f"round {round_number} note {i} by M{writer.number} while M{victim.number} is stopped"
            t = time.monotonic()
            written[flows.add_finding(c, writer, goal, text)] = text
            latencies.append(round(time.monotonic() - t, 2))
            i += 1
            time.sleep(min(10, max(0, seconds - (time.monotonic() - began))))
        entry["notes_written_while_stopped"] = len(written)
        entry["max_write_latency_on_others_s"] = max(latencies)
        flows.findings_everywhere(c, others, goal, written,
                               f"{role} {seconds}s: the others exchange while M{victim.number} is stopped", 60)
        c.resume(victim)
        resumed = time.monotonic()
        flows.findings_everywhere(c, [victim], goal, written,
                               f"{role} {seconds}s: M{victim.number} catches up after SIGCONT", CATCHUP)
        entry["catch_up_seconds"] = round(time.monotonic() - resumed, 1)
        text = f"round {round_number}: M{victim.number} writes after waking"
        own = {flows.add_finding(c, victim, goal, text): text}
        flows.findings_everywhere(c, others, goal, own,
                               f"{role} {seconds}s: M{victim.number}'s new note reaches the others", CATCHUP)
        entry["own_write_spread_seconds"] = round(time.monotonic() - resumed - entry["catch_up_seconds"], 1)
        if role == "coordinator":
            task = "task:" + c.recorded(m1, ["task", "open", "--goal", goal, f"Task after wake {round_number}"])
            c.wait(f"{role} {seconds}s: a decision after waking reaches M2 and M3", lambda: all(
                any(item.get("task") == task for item in c.cli(m, ["board", "--goal", goal])["board"])
                for m in (m2, m3)), CATCHUP)
        flows.not_halted(c, [m1, m2, m3], goal)
        entry["passed"] = True
    flows.same_history(c, [m1, m2, m3], goal, "one history after every sleep", 60)


def burst(c, machine, goal, threads=4, kill_after=12, per_thread=40):
    """Concurrent `contribution publish` calls on one daemon; kill -9 it mid-burst.
    Returns (acknowledged {event: text}, error codes seen)."""
    acked, errors, lock, killed = {}, [], threading.Lock(), threading.Event()

    def writer(index):
        for n in range(per_thread):
            text = f"burst on M{machine.number} thread {index} write {n}"
            result = c.cli(machine, ["contribution", "publish", "--goal", goal, text], tolerate=True, timeout=30)
            if isinstance(result, tuple):
                with lock:
                    errors.append(result[1])
                if killed.is_set():
                    return
                continue
            with lock:
                acked[result["recorded"]["event"]] = text
            if killed.is_set():
                return

    workers = [threading.Thread(target=writer, args=(i,)) for i in range(threads)]
    for w in workers:
        w.start()
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        with lock:
            if len(acked) >= kill_after:
                break
        time.sleep(0.005)
    killed.set()
    c.kill9(machine)
    for w in workers:
        w.join(timeout=60)
    with lock:
        return dict(acked), list(errors)


def crash(c):
    """kill -9 at rest, during a write burst (worker and coordinator), and during
    a join (joiner, then coordinator); every acknowledged write must survive."""
    (m1, m2, m3), goal = three_member_goal(c, "Crash simulation")
    results, acked = c.summary.setdefault("results", []), {}
    everyone = [m1, m2, m3]

    def rejoined(victim, label):
        flows.not_halted(c, everyone, goal)
        text = f"{label}: M{victim.number} writes after restart"
        own = {flows.add_finding(c, victim, goal, text): text}
        acked.update(own)
        began = time.monotonic()
        flows.findings_everywhere(c, everyone, goal, acked, f"{label}: every acknowledged note everywhere", CATCHUP)
        # Readable text everywhere, not just the event: see the restart-latency probe.
        c.summary.setdefault("readable_everywhere_seconds", {})[label] = round(time.monotonic() - began, 1)
        others = [m for m in everyone if m is not victim]
        c.wait(f"{label}: M{victim.number} connected to the others again", lambda: all(
            flows.peer_connected(c, victim, goal, o) for o in others), CATCHUP)

    c.phase = "crash_idle"
    c.kill9(m2)
    c.relaunch(m2)
    rejoined(m2, "idle")
    results.append({"case": "idle", "machine": 2, "passed": True})

    for victim in (m2, m1):
        label = f"burst_M{victim.number}"
        c.phase = label
        ack, errors = burst(c, victim, goal)
        acked.update(ack)
        c.relaunch(victim)
        if not c.contributions_contain(victim, goal, ack):
            missing = [e for e in ack if e not in c.finding_ids(victim, goal)]
            raise CheckFailure(f"{label}: {len(missing)} acknowledged notes lost by the killed daemon")
        rejoined(victim, label)
        results.append({"case": label, "machine": victim.number, "acknowledged": len(ack),
                        "errors_racing_kill": sorted(set(errors)), "error_count": len(errors), "passed": True})

    c.phase = "join_kill_joiner"
    m4 = c.add()
    flows.boot(c, [m4])
    everyone.append(m4)
    flows.invite_join(c, m1, m4, goal, "Crash simulation", everyone, CATCHUP,
                      after_join=lambda: (c.kill9(m4), c.relaunch(m4)))
    rejoined(m4, "join_kill_joiner")
    results.append({"case": "join_kill_joiner", "machine": 4, "passed": True})

    c.phase = "join_kill_coordinator"
    m5 = c.add()
    flows.boot(c, [m5])
    everyone.append(m5)

    def kill_coordinator():
        time.sleep(0.15)
        c.kill9(m1)
        c.relaunch(m1)

    flows.invite_join(c, m1, m5, goal, "Crash simulation", everyone, CATCHUP, after_join=kill_coordinator)
    rejoined(m1, "join_kill_coordinator")
    results.append({"case": "join_kill_coordinator", "machine": 1, "passed": True})
    history = flows.same_history(c, everyone, goal, "one history on all five after every crash", 60)
    c.summary["acknowledged_notes"] = len(acked)
    c.summary["final_history_size"] = len(history)


SCENARIOS = {"sleep": sleep, "crash": crash}
