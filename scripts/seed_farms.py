#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = ["blake3>=1.0,<2", "cryptography>=46,<47"]
# ///
"""Prepare and publish two explicitly synthetic, ended website demo farms.

No agents are run and no private goal data is read. The service validates the
ordinary signed public snapshots. Keep --state outside Git: it holds upload keys
and frozen requests so retries use the same identities, sequence and payload.
"""
import argparse
from datetime import datetime
import json
import os
from pathlib import Path
import struct
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
SCENARIOS = Path(__file__).with_name("farm_seeds.json")
AGENTS = [("Codex", "codex"), ("Claude", "claude_code"), ("Pi", "pi"), ("Kimi", "kimi_code")]


def build_snapshot(scenario, farm_id):
    start = int(datetime.fromisoformat(scenario["started_at"]).timestamp() * 1000)
    timestamp = lambda minute: start + round(minute * 60000)
    end = max(task["end"] for task in scenario["tasks"]) + 2
    snapshot = {
        "version": 1, "farm_id": farm_id, "title": scenario["title"],
        "formation": scenario["formation"], "goal_state": "ended",
        "observed_at_ms": timestamp(end), "omitted_changes": 0,
        "groups": [{"id": 1, "label": "Andrei's agents (demo)", "last_sync_at_ms": timestamp(end)}],
        "agents": [{"id": i, "name": name, "harness": harness, "group": 1,
                    "roles": [scenario["roles"][i - 1]]}
                   for i, (name, harness) in enumerate(AGENTS, 1)],
        "stages": scenario["stages"], "tasks": [], "attempts": [], "candidates": [],
        "changes": [],
    }
    changes = []

    def event(minute, kind, text, agent=None, task=None):
        changes.append({"kind": kind, "text": text, "agent": agent, "task": task,
                        "observed_at_ms": timestamp(minute)})

    event(0, "publication", "Synthetic demo: all activity, results and evidence counts are illustrative; no agents were run.")
    for i, (name, _) in enumerate(AGENTS, 1):
        event(i / 10, "membership", f"{name} joined the demo as {scenario['roles'][i - 1].lower()}.", i)
    for i, task in enumerate(scenario["tasks"], 1):
        name = AGENTS[task["agent"] - 1][0]
        reviewer = AGENTS[task["reviewer"] - 1][0]
        reference = f"{i:04}"
        retry = task.get("retry")
        round_ = 2 if retry else 1
        snapshot["tasks"].append({
            "id": i, "reference": reference, "stage": task["stage"], "round": round_,
            "state": "completed", "completed": True, "closed": False, "selected_candidate": i,
        })
        snapshot["attempts"].append({
            "id": i, "task": i, "agent": task["agent"], "round": round_,
            "state": "completed", "observed_at_ms": timestamp(task["end"] - 2),
        })
        snapshot["candidates"].append({
            "id": i, "task": i, "agent": task["agent"], "round": round_,
            "completed": True, "selected": True, "evidence_count": 2,
            "requirement": task["check"],
        })
        event(task["start"] - 0.2, "task", f"Codex opened task {reference}: {task['work']}.", 1, i)
        event(task["start"], "attempt", f"{name} started task {reference}.", task["agent"], i)
        if retry:
            rejected = len(scenario["tasks"]) + i
            snapshot["attempts"].append({
                "id": rejected, "task": i, "agent": task["agent"], "round": 1,
                "state": "failed", "observed_at_ms": timestamp(retry["at"]),
            })
            snapshot["candidates"].append({
                "id": rejected, "task": i, "agent": task["agent"], "round": 1,
                "completed": False, "selected": False, "evidence_count": 1,
                "requirement": "Review requested changes: " + retry["reason"],
            })
            event(retry["at"] - 1, "contribution", f"{name} submitted task {reference} for review.", task["agent"], i)
            event(retry["at"], "evidence", f"{reviewer} requested changes on {reference}: {retry['reason']}.", task["reviewer"], i)
            event(retry["at"] + 1, "task", f"Codex opened round 2 of task {reference} after review.", 1, i)
            event(retry["at"] + 2, "attempt", f"{name} restarted {reference}: {retry['fix']}.", task["agent"], i)
        event(task["end"] - 2, "contribution", f"{name} submitted {reference}: {task['result']}.", task["agent"], i)
        event(task["end"] - 1, "evidence", f"{reviewer} approved {reference}: {task['check']}.", task["reviewer"], i)
        event(task["end"], "task", f"Codex selected the reviewed result for {reference}; task complete.", 1, i)
    event(end, "closure", f"Codex closed the demo goal: {len(scenario['tasks'])} of {len(scenario['tasks'])} tasks complete. All events and evidence counts are synthetic.", 1)
    snapshot["changes"] = [dict(change, id=i) for i, change in enumerate(
        sorted(changes, key=lambda item: item["observed_at_ms"]), 1)]
    return snapshot


def domain_hash(context, data):
    from blake3 import blake3
    return blake3(data, derive_key_context=context).digest()


def identity(seed):
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    key = Ed25519PrivateKey.from_private_bytes(seed)
    public = key.public_key().public_bytes_raw()
    return key, public, domain_hash("locust v1 farm identity", public)[:16].hex()


def request_digest(request):
    from blake3 import blake3
    operation = ["upload", "check_in", "suspend", "delete"].index(request["operation"])
    envelope = (struct.pack(">HB", request["version"], operation)
                + bytes.fromhex(request["farm_id"]) + struct.pack(">Q", request["sequence"])
                + blake3(request["body"].encode()).digest())
    return domain_hash("locust v1 farm request envelope", envelope)


def sign(seed, operation, body, sequence=1):
    key, public, farm_id = identity(seed)
    request = {"version": 1, "operation": operation, "farm_id": farm_id,
               "sequence": sequence, "public_key": public.hex(), "body": body}
    request["signature"] = key.sign(domain_hash(
        "locust v1 farm request signature", request_digest(request))).hex()
    return request


def private_create(path, content):
    with path.open("xb") as file:
        os.chmod(path, 0o600)
        file.write(content)


def prepare(state):
    state.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(state, 0o700)
    requests = []
    for scenario in json.loads(SCENARIOS.read_text()):
        seed_path = state / (scenario["slug"] + ".key")
        if not seed_path.exists():
            private_create(seed_path, os.urandom(32))
        seed = seed_path.read_bytes()
        _, _, farm_id = identity(seed)
        request_path = state / (scenario["slug"] + ".request.json")
        if not request_path.exists():
            body = {"visibility": "listed", "snapshot": build_snapshot(scenario, farm_id)}
            request = sign(seed, "upload", json.dumps(body, separators=(",", ":")))
            private_create(request_path, (json.dumps(request, indent=2) + "\n").encode())
        request = json.loads(request_path.read_text())
        # Never silently replace a stored request or advance a sequence on retry.
        if request != sign(seed, "upload", request["body"]):
            raise ValueError(f"Stored request does not match its key: {request_path}")
        requests.append((scenario["slug"], request))
    return requests


def http_json(url, request=None):
    body = json.dumps(request).encode() if request else None
    req = urllib.request.Request(url, data=body, method="PUT" if request else "GET",
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


def publish(service, request):
    endpoint = service.rstrip("/") + "/api/farms/" + request["farm_id"]
    receipt = http_json(endpoint, request)
    for field, expected in [("farm_id", request["farm_id"]), ("sequence", request["sequence"]),
                            ("request_digest", request_digest(request).hex())]:
        if receipt.get(field) != expected:
            raise ValueError(f"Receipt {field} does not match the upload")
    view = http_json(endpoint)
    if (view.get("status") != "available" or view.get("visibility") != "listed"
            or view.get("snapshot") != json.loads(request["body"])["snapshot"]):
        raise ValueError("Read-back does not match the listed demo snapshot")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["prepare", "publish"])
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--service", help="Farm service origin; required to publish")
    args = parser.parse_args()
    state = args.state.expanduser().resolve()
    if state == ROOT or ROOT in state.parents:
        parser.error("Keep signing keys outside the repository; use a private --state directory")
    if args.command == "publish" and not args.service:
        parser.error("--service is required to publish")
    for slug, request in prepare(state):
        result = {"demo": slug, "farm_id": request["farm_id"]}
        if args.command == "publish":
            result["receipt"] = publish(args.service, request)
            result["url"] = args.service.rstrip("/") + "/farm/" + request["farm_id"]
        print(json.dumps(result), flush=True)


if __name__ == "__main__":
    main()
