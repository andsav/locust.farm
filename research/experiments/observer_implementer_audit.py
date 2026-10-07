#!/usr/bin/env python3
"""Inventory saved Merak8 observer/implementer trials without exporting transcripts."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess


SOURCES = (
    "polaris/src-tauri/builtin_blueprints/swe-pipeline-gold-v2.json",
    "merak/fixtures/legacy/builtin_blueprints/swe-pipeline-gold-v2.json",
    "merak/crates/merak-engine/tests/swe_gold_real.rs",
    "merak/fixtures/parity/swe-pipeline-gold-trace.json",
    "docs/internal/TB2_HILL_CLIMB_RUNBOOK.md",
    "campaigns/tb2/runs.jsonl",
)


def identity(root, path):
    return {"path": path.relative_to(root).as_posix(),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def audit(root):
    ledger = [json.loads(line) for line in (root / "campaigns/tb2/runs.jsonl").read_text().splitlines() if line.strip()]
    selected = []
    for row in ledger:
        if row.get("candidate_id") in ("swe-polaris-default", "smoke-normal"):
            selected.append({key: row.get(key) for key in (
                "run_id", "task", "candidate_id", "score", "failure_stage", "duration_seconds",
            )})

    trials = []
    for path in sorted(root.glob("jobs/*swe-polaris-default*/*/result.json")):
        data = json.loads(path.read_text())
        trials.append({
            **identity(root, path), "task": data.get("task_name"),
            "rewards": (data.get("verifier_result") or {}).get("rewards"),
            "exception_type": (data.get("exception_info") or {}).get("exception_type"),
            "cost_usd": (data.get("agent_result") or {}).get("cost_usd"),
        })

    traces = []
    for path in sorted(root.glob("jobs/*swe-polaris-default*/*/agent/merak-trace.json")):
        data = json.loads(path.read_text())
        events = data["runtime_events"]
        nodes, failures = [], []
        for row in events:
            event = row["event"]
            if event["type"] == "run_node_failed":
                failures.append({"node_id": event.get("node_id"), "sequence": row["sequence"],
                                 "timestamp": row["timestamp"], "message": event.get("message")})
            if event.get("node_id") not in ("executor", "observer", "superego") or event["type"] != "run_node_completed":
                continue
            outputs = event.get("outputs", {})
            nodes.append({
                "node_id": event["node_id"], "sequence": row["sequence"], "timestamp": row["timestamp"],
                "outputs": {key: outputs[key] for key in (
                    "status", "run_status", "steps", "tool_calls_count", "task_completed",
                    "stopped_by_feed", "auto_completed_from_verification", "executor.rotation.model",
                ) if key in outputs},
            })
        executor_failure = next((row for row in failures if row["node_id"] == "executor"), None)
        if executor_failure:
            for node in nodes:
                if node["node_id"] in ("observer", "superego"):
                    node["seconds_after_executor_failure"] = round(float(node["timestamp"]) - float(executor_failure["timestamp"]), 3)
        traces.append({
            **identity(root, path), "event_count": len(events),
            "event_types": dict(sorted(Counter(row["event"]["type"] for row in events).items())),
            "failures": failures, "completed_nodes": nodes,
        })

    configurations = []
    for name in SOURCES[:2]:
        data = json.loads((root / name).read_text())
        roles = []
        for node in data["nodes"]:
            if node["id"] in ("executor", "observer", "superego"):
                config = node["config"]
                roles.append({"node_id": node["id"], **{key: config.get(key) for key in (
                    "model", "publish", "subscribe", "heartbeat_seconds", "stop_feed", "tools",
                )}})
        configurations.append({"path": name, "roles": roles})

    return {
        "inspection_date": "2026-10-07",
        "repository": "merak8",
        "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
        "tracked_worktree_dirty": bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root, text=True).strip()),
        "scope": {
            "result_glob": "jobs/*swe-polaris-default*/*/result.json",
            "trace_glob": "jobs/*swe-polaris-default*/*/agent/merak-trace.json",
            "ledger_candidates": ["swe-polaris-default", "smoke-normal"],
            "limitations": [
                "Local retained archive only; traces and campaign-ledger coverage are incomplete.",
                "Candidate labels do not establish identical historical code, prompts or model configurations.",
                "Inspected source configurations are not asserted to be the configurations used by saved runs.",
                "Node completion does not imply a completed model step, delivered advice, or a passing external verifier.",
                "No matched observer ablation, complete cost accounting, or causal benefit estimate is established.",
                "No raw prompts, reasoning, tool payloads, model transcripts or environment values are exported.",
            ],
        },
        "sources": [identity(root, root / name) for name in SOURCES],
        "configurations": configurations,
        "ledger_total_records": len(ledger), "selected_ledger_records": selected,
        "trial_results": trials, "runtime_traces": traces,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    record = audit(args.source.resolve())
    args.destination.write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps({"results": len(record["trial_results"]), "traces": len(record["runtime_traces"]),
                      "selected_ledger_records": len(record["selected_ledger_records"])}))
