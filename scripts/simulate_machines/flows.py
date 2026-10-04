"""Reusable steps of the T1 guide, driven only through the CLI of each daemon."""

import json
import re
import time

from simlib import CheckFailure, identity, variant

PREFIX = "locust-invite-"


def hint_kinds(ticket):
    """Only the kinds of contact hint a ticket carries (never the ticket itself):
    the ticket is hex of the encoded invitation, whose hints are plain strings."""
    try:
        text = bytes.fromhex(ticket[len(PREFIX):]).decode("latin-1")
    except ValueError:
        return {"decoded": False}
    return {"relay_url_hints": len(re.findall(r"https?://[!-~]+?/", text)),
            "ip_hints": len(re.findall(r"(?:\d{1,3}\.){3}\d{1,3}:\d+|\[[0-9a-fA-F:.%a-z0-9]+\]:\d+", text))}


def boot(cluster, machines):
    """Start and enroll each machine as in the guide (m<n>, --manage-goals)."""
    for machine in machines:
        cluster.start(machine)
        result = cluster.cli(machine, ["agent", "enroll", f"m{machine.number}", "--manage-goals"], owner=True)
        machine.agent = identity(variant(result, "agent_enrolled")["agent"], "principal")
        cluster.cli(machine, ["status"])
        cluster.cli(machine, ["doctor"])
    cluster.summary["machines"] = [
        {"machine": m.number, "home": str(m.home), "binary": m.binary_name, "principal": m.agent,
         "endpoint": m.endpoint, "env": m.env} for m in cluster.machines]


def grant(cluster, machine, goal):
    cluster.cli(machine, ["goal", "grant", "--goal", goal, "--agent", machine.agent,
        "--grants", json.dumps({"administer": True, "contribute": True, "review": True,
            "select": True, "flow": True, "execute": False, "takeover": False})], owner=True)


def found(cluster, coordinator, title):
    goal = cluster.create_goal(coordinator, title)
    grant(cluster, coordinator, goal)
    cluster.summary["goal"] = goal
    return goal


def members_ok(cluster, machines, goal, title, expected):
    for machine in machines:
        state = cluster.goal_status(machine, goal)
        if not (state and state.get("halted") is None and state.get("title") == title
                and {m["member"] for m in state["members"]} == expected):
            return False
    return True


def invite_join(cluster, coordinator, joiner, goal, title, everyone, timeout=None, after_join=None):
    """Invite, join, and wait until every listed replica admits everyone.
    Returns seconds from the join command's answer to full admission."""
    ticket = variant(cluster.cli(coordinator, ["goal", "invite", "--goal", goal]), "invited")["ticket"]
    cluster.summary.setdefault("ticket_hint_kinds", {})[f"M{joiner.number}"] = hint_kinds(ticket)
    joined = variant(cluster.cli(joiner, ["goal", "join", "--ticket", ticket]), "joined")
    del ticket
    if joined.get("goal") != goal:
        raise CheckFailure("join answered with another goal")
    sent = time.monotonic()
    if after_join is not None:
        after_join()
    expected = {m.agent for m in everyone}
    cluster.wait(f"M{joiner.number} join admitted on all of {[m.number for m in everyone]}",
                 lambda: members_ok(cluster, everyone, goal, title, expected), timeout)
    for machine in everyone:
        grant(cluster, machine, goal)
    seconds = round(time.monotonic() - sent, 2)
    cluster.summary.setdefault("join_seconds", {})[f"M{joiner.number}"] = seconds
    return seconds


def result_held(cluster, machine, goal, result_id, text):
    output = cluster.cli(machine, ["event", "show", "--goal", goal, "--event", result_id],
                         expected_errors=("not_found",))
    if not output:
        return False
    detail = variant(output, "event")
    return (detail.get("text") == text and detail["view"]["kind"] == "contribution_published"
            and all(content.get("state") == "held" for content in detail["content"]))


def pending_has(cluster, machine, goal, bucket, task):
    pending = variant(cluster.cli(machine, ["pending", "--goal", goal]), "pending")
    return any(item.get("task") == task for item in pending.get(bucket, []))


def complete_task(cluster, coordinator, worker, goal, observers, timeout=None, label="T1"):
    """Open, offer, authorize, start, publish, review and select across replicas."""
    task_text = f"Return the text: {label} task completed."
    summary = f"{label} task completed."
    task = "task:" + cluster.recorded(coordinator, ["task", "open", "--goal", goal, task_text])
    offer = cluster.recorded(coordinator, ["work", "offer", "--goal", goal, "--task", task,
                                                "--recipient", worker.agent])
    cluster.wait(f"M{worker.number} receives offer {label}", lambda: pending_has(
        cluster, worker, goal, "to_authorize", task), timeout)
    cluster.cli(worker, ["task", "authorize", "--goal", goal, "--task", task, "--agent", worker.agent], owner=True)
    session_path = worker.home / "sessions" / f"{label.lower()}.secret"
    session = cluster.cli(worker, ["session", "create", session_path], local=True)
    if session_path.stat().st_mode & 0o7777 != 0o600:
        raise CheckFailure("execution session file was not mode 0600")
    if not pending_has(cluster, worker, goal, "to_start", task):
        raise CheckFailure("authorized task is not listed to_start")
    claim = variant(cluster.cli(worker, ["attempt", "start", "--goal", goal, "--task", task, "--offer", offer],
                                session=session_path), "claimed")
    if claim.get("task") != task or claim.get("instance") != session.get("instance"):
        raise CheckFailure("claim did not bind the requested task/session")
    result_id = cluster.recorded(worker, ["contribution", "publish", "--goal", goal, "--task", task, "--attempt", claim["attempt"],
                                          "--generation", claim["generation"], summary], session=session_path)
    cluster.wait(f"M{coordinator.number} reads the submitted result {label}",
                 lambda: result_held(cluster, coordinator, goal, result_id, summary), timeout)
    cluster.recorded(coordinator, ["review", "record", "--goal", goal, "--subject", result_id, "--verdict", "approve", "Verified deterministic contribution"])
    selection = cluster.recorded(coordinator, ["scope", "select", "--goal", goal, "--subject", result_id])
    cluster.cli(coordinator, ["pending", "--goal", goal])
    cluster.wait(f"selection {label} visible on {[m.number for m in observers]}", lambda: all(
        cluster.board_selected(m, goal, task, result_id) and result_held(cluster, m, goal, result_id, summary)
        for m in observers), timeout)
    events = dict(task=task, offer=offer, attempt=claim["attempt"], result=result_id, selection=selection)
    cluster.summary["events"][label] = events
    return events


def add_finding(cluster, machine, goal, text, **options):
    return cluster.recorded(machine, ["contribution", "publish", "--goal", goal, text], **options)


def findings_everywhere(cluster, machines, goal, notes, label, timeout=None):
    cluster.wait(label, lambda: all(cluster.contributions_contain(m, goal, notes) for m in machines), timeout)


def same_history(cluster, machines, goal, label, timeout=None):
    """All replicas converge on one effective history; returns it."""
    held = {}

    def check():
        histories = [set(cluster.history(m, goal)) for m in machines]
        held["h"] = histories[0]
        return all(h == histories[0] for h in histories)

    cluster.wait(label, check, timeout)
    return held["h"]


def not_halted(cluster, machines, goal):
    for machine in machines:
        if cluster.halted(machine, goal):
            raise CheckFailure(f"M{machine.number} reports goal halted or missing")


def selected_route(cluster, machine, peer=None, generation=None, label=None, timeout=None):
    """Wait until this daemon logged a selected path (optionally to one peer)."""
    peer_id = peer.endpoint if peer is not None else None
    found = {}

    def check():
        kinds = cluster.selected_kinds(machine, generation, peer_id)
        found["kinds"] = kinds
        return bool(kinds)

    cluster.wait(label or f"M{machine.number} logs a selected path", check, timeout)
    return found["kinds"]


def peer_connected(cluster, machine, goal, peer, since_ms=None):
    """A live link to the peer; with since_ms, also a completed sync after it."""
    state = cluster.goal_status(machine, goal)
    return bool(state) and any(
        p["endpoint"] == peer.endpoint and p["connected"]
        and (since_ms is None or (p.get("last_sync_ms") or 0) >= since_ms) for p in state["peers"])


def peer_disconnected(cluster, machine, goal, peer):
    state = cluster.goal_status(machine, goal)
    return bool(state) and not any(p["endpoint"] == peer.endpoint and p["connected"] for p in state["peers"])
