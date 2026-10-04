"""Diagnostic scenario (not one of the seven): how long the first write of a
restarted daemon takes to reach a peer, after a clean stop and after kill -9.
The crash scenario showed about 30 s after every kill -9; this measures it with
a timeline of the peer's view of the link."""

import threading
import time

import flows
from scen_faults import three_member_goal


def timeline(c, victim, watcher, goal, text, deadline=90):
    """Write on the victim; poll each watcher until all hold the note."""
    note = flows.add_finding(c, victim, goal, text)
    began, samples, arrived = time.monotonic(), {w.number: [] for w in watcher}, {}
    while time.monotonic() - began < deadline and len(arrived) < len(watcher):
        for w in watcher:
            if w.number in arrived:
                continue
            state = c.goal_status(w, goal)
            peer = next((p for p in state["peers"] if p["endpoint"] == victim.endpoint), {})
            notes = c.finding_ids(w, goal)
            held = note in notes
            readable = notes.get(note) == text
            samples[w.number].append({"t": round(time.monotonic() - began, 2), "connected": peer.get("connected"),
                                      "event_held": held, "text_readable": readable,
                                      "last_sync_age_ms": (int(time.time() * 1000) - peer["last_sync_ms"])
                                      if peer.get("last_sync_ms") else None})
            if readable:
                arrived[w.number] = round(time.monotonic() - began, 2)
        time.sleep(0.5)
    return arrived, samples


def restart_latency(c):
    (m1, m2, m3), goal = three_member_goal(c, "Restart latency probe")
    results = c.summary.setdefault("results", [])
    for how in ("clean-stop", "kill-9", "clean-stop-busy", "kill-9-busy", "kill-9-busy"):
        c.phase = how
        time.sleep(2)
        busy = threading.Event()
        writer = None
        if how.endswith("-busy"):
            # M1 and M3 write continuously so exchanges with M2 are in flight at the kill.
            def write():
                n = 0
                while not busy.is_set():
                    for w in (m1, m3):
                        flows.add_finding(c, w, goal, f"{how} background {n} by M{w.number}")
                    n += 1
            writer = threading.Thread(target=write)
            writer.start()
            time.sleep(1.0)
        if how.startswith("kill-9"):
            c.kill9(m2)
        else:
            c.stop(m2)
        if writer is not None:
            busy.set()
            writer.join()
        c.relaunch(m2)
        arrived, samples = timeline(c, m2, [m1, m3], goal, f"first write after {how}")
        views = {}
        for number, series in samples.items():
            views[f"M{number}"] = [s for i, s in enumerate(series) if i == 0 or i == len(series) - 1
                                   or (s["connected"], s["event_held"]) != (series[i - 1]["connected"], series[i - 1]["event_held"])][:12]
        results.append({"restart": how, "first_write_arrival_seconds": {f"M{k}": v for k, v in arrived.items()},
                        "view_of_M2": views})
    # The coordinator restarting with the default bind (a new port each start):
    # how long until each member has completed a sync with it again.
    for how in ("coordinator-clean-stop", "coordinator-kill-9"):
        c.phase = how
        time.sleep(2)
        since_ms = int(time.time() * 1000)
        if how.endswith("kill-9"):
            c.kill9(m1)
        else:
            c.stop(m1)
        c.relaunch(m1)
        began, synced = time.monotonic(), {}
        while time.monotonic() - began < 90 and len(synced) < 2:
            for m in (m2, m3):
                if m.number not in synced and flows.peer_connected(c, m, goal, m1, since_ms):
                    synced[m.number] = round(time.monotonic() - began, 2)
            time.sleep(0.25)
        results.append({"restart": how, "member_synced_with_coordinator_seconds":
                         {f"M{k}": v for k, v in synced.items()}})
    c.summary["note"] = ("connected/last_sync_age are each watcher's goal-status view of M2's endpoint, sampled "
                         "every 0.5 s after M2's first write following its restart")


SCENARIOS = {"restart-latency": restart_latency}
