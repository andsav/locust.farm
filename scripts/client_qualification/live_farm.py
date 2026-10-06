"""Explicit public publication for isolated, real-client qualification runs.

The caller must have owner authorization to publish. This fixture permits HTTPS
publication; unlike ProductionDaemon it does not enforce loopback-only traffic.
Agent peer networking remains configured for loopback, with no lookup or relay.
"""

import json
import time
import urllib.error
import urllib.request

from .production import ProductionDaemon


class FarmDaemon(ProductionDaemon):
    def _guard(self):
        return []

    def _record(self, event, **fields):
        if event == "daemon_started":
            fields["network_guard"] = "none; public farm HTTPS publication enabled"
        if event == "fixture_ready":
            fields["role"] = "fixture_host_agent; client roles enrolled separately"
        return super()._record(event, **fields)


def _call(daemon, args, **kwargs):
    from check_shared_context_models import raw_call
    return raw_call(daemon, args, **kwargs)


def configure_farm(daemon, roles, service, title):
    """Publish only explicit labels after consent from every enrolled member."""
    service = service.rstrip("/")
    preview = _call(daemon, [
        "farm", "on", "--goal", daemon.goal, "--service", service, "--listed",
        "--title", title, "--formation", "Live agent qualification: Luna and Haiku",
    ], owner=True)["farm_preview"]
    members = _call(daemon, ["goal", "status", "--goal", daemon.goal])["goal_status"]["members"]
    labels = {role["principal"]: role["name"] for role in roles}
    if {member["member"] for member in members} != set(labels):
        raise RuntimeError("Every goal member needs an explicit public label")
    for principal, name in labels.items():
        _call(daemon, ["farm", "consent", "--goal", daemon.goal,
            "--agent", principal, "--accept", "--name", name,
            "--group-label", "One local Mac"], owner=True)
    preview = _call(daemon, ["farm", "show", "--goal", daemon.goal], owner=True)["farm_preview"]
    farm_id = preview["status"]["farm_id"]
    daemon.public_farm = {"service": service, "labels": labels, "farm_id": farm_id}
    return {"farm_id": farm_id, "url": service + "/farm/" + farm_id,
            "service": service, "topology": "Two real clients on one local daemon"}


def report_session(daemon, role, state, summary):
    """Record only lifecycle states observed by the harness, not model intent."""
    harness = {"codex": "codex", "claude": "claude_code",
               "claude-code": "claude_code", "claude_code": "claude_code"}[role["client"]]
    record = {"harness": harness, "client": role["client"], "state": state,
        "client_session": role.get("client_session"),
        "capabilities": {"tools": True, "active_delivery": False, "idle_wake": False,
            "manual_resume": role["client"] == "codex", "recovery": False, "confinement": False},
        "detail": list(json.dumps({"observation": summary}).encode())}
    result = _call(daemon, ["call", "session.report", json.dumps({"record": record})], role=role)
    if state == "started" and hasattr(daemon, "public_farm"):
        # Consent freezes the reported harness. Refresh only after observing
        # this actual client, preserving the owner's exact public labels.
        _call(daemon, ["farm", "consent", "--goal", daemon.goal, "--agent", role["principal"],
            "--accept", "--name", daemon.public_farm["labels"][role["principal"]],
            "--group-label", "One local Mac"], owner=True)
    return result


def public_ended(daemon, receipt):
    """A newer receipt alone can acknowledge an older, still-open upload."""
    farm = daemon.public_farm
    request = urllib.request.Request(farm["service"] + "/api/farms/" + farm["farm_id"],
                                     headers={"Cache-Control": "no-cache"})
    try:
        with urllib.request.urlopen(request, timeout=daemon.timeout_seconds) as response:
            value = json.load(response)
    except (urllib.error.URLError, ValueError):
        return None
    snapshot = value.get("snapshot") or {}
    if (value.get("farm_id") == farm["farm_id"] and value.get("status") == "available"
            and value.get("stream_version", 0) >= receipt["stream_version"]
            and snapshot.get("farm_id") == farm["farm_id"] and snapshot.get("goal_state") == "ended"):
        return value
    return None


def finish_farm(daemon):
    """Close the successful goal and retain the daemon until its upload is acked."""
    before = _call(daemon, ["farm", "show", "--goal", daemon.goal], owner=True)["farm_preview"]
    previous_sequence = (before["status"].get("receipt") or {}).get("sequence", 0)
    _call(daemon, ["scope", "close", "--goal", daemon.goal, "--scope", '"goal"'])
    deadline = time.monotonic() + daemon.timeout_seconds
    while True:
        preview = _call(daemon, ["farm", "show", "--goal", daemon.goal], owner=True)["farm_preview"]
        status = preview["status"]
        if ((preview.get("snapshot") or {}).get("goal_state") == "ended"
                and status.get("pending") is None
                and (status.get("receipt") or {}).get("sequence", 0) > previous_sequence):
            public = public_ended(daemon, status["receipt"])
            if public is not None:
                return {"local": preview, "public": public}
        if time.monotonic() >= deadline:
            raise RuntimeError("Ended farm publication has not been acknowledged")
        time.sleep(.2)
