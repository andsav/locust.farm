#!/usr/bin/env python3
"""Read a local Merak4 checkout and export allowlisted historical-run metadata.

This inventories retained files; it does not estimate a treatment effect. Raw
logs, model responses, environment settings and exception messages stay local.
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess


SOURCE_FILES = (
    "observer/tagteam.go", "observer/observer.go", "observer/prompt.go",
    "agent/compaction.go", "taskrunner/models.go", "taskrunner/slots.go",
    "taskrunner/main_path_runner.go", "taskrunner/router.go",
    "taskrunner/pipeline_config.go", "cmd/merak/swarm_explore.go",
    "cmd/merak/swarm_explore_test.go", "benchmark-tracker.md",
    "swe-bench-pro-tracker.md",
)


def fingerprint(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def audit(root):
    results = []
    for path in sorted(root.glob("jobs/*/*/result.json")):
        data = json.loads(path.read_text())
        results.append({
            "path": path.relative_to(root).as_posix(),
            "sha256": fingerprint(path),
            "task": data.get("task_name"),
            "rewards": (data.get("verifier_result") or {}).get("rewards"),
            "cost_usd": (data.get("agent_result") or {}).get("cost_usd"),
            "config_model_name": ((data.get("config") or {}).get("agent") or {}).get("model_name"),
            "exception_type": (data.get("exception_info") or {}).get("exception_type"),
            "has_log": (path.parent / "agent/merak.log").is_file(),
        })

    logs = []
    for path in sorted(root.glob("jobs/*/*/agent/merak.log")):
        lines = path.read_text(errors="replace").splitlines()
        turns, flips, swarm_completions = [], [], []
        markers = {key: 0 for key in (
            "observer fired", "starting swarm explore", "swarm explore complete",
            "swarm explore failed", "swarm worker failed", "swarm judge failed",
        )}
        for line_number, line in enumerate(lines, 1):
            for marker in markers:
                if f'msg="{marker}"' in line:
                    markers[marker] += 1
            if 'msg="swarm explore complete"' in line:
                completion = {"line": line_number}
                for key in ("workers", "hypotheses", "tests"):
                    field = re.search(r"\b" + key + r"=(\d+)", line)
                    completion[key] = int(field[1]) if field else None
                swarm_completions.append(completion)
            if 'msg="tag team turn"' not in line and 'msg="tag team flip"' not in line:
                continue
            model = re.search(r'executor=(?:"([^"]+)"|(\S+))', line)
            turn = re.search(r'turn=(\d+)', line)
            if model:
                item = {"line": line_number, "turn": int(turn[1]) if turn else None,
                        "model": model[1] or model[2]}
                (flips if 'msg="tag team flip"' in line else turns).append(item)
        logs.append({
            "path": path.relative_to(root).as_posix(), "sha256": fingerprint(path),
            "turns": turns, "flips": flips, "markers": markers,
            "swarm_completions": swarm_completions,
            "has_result": (path.parent.parent / "result.json").is_file(),
        })

    task_counts = Counter(row["task"] for row in results)
    reward_counts = Counter(json.dumps(row["rewards"], sort_keys=True) for row in results)
    model_counts = Counter(row["config_model_name"] for row in results)
    return {
        "schema_version": 1,
        "inspection_date": "2026-10-07",
        "source": {
            "repository": "merak4",
            "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
            "tracked_status": subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root, text=True).splitlines(),
            "files": [{"path": name, "sha256": fingerprint(root / name)} for name in SOURCE_FILES],
        },
        "scope": {
            "result_glob": "jobs/*/*/result.json",
            "log_glob": "jobs/*/*/agent/merak.log",
            "selection": "All matching retained files, sorted by relative path; fail on malformed result JSON.",
            "limitations": [
                "Mixed tasks, repeated attempts and changing harness configurations; not a controlled comparison.",
                "Log coverage is partial. A missing marker does not establish that a feature was absent.",
                "config_model_name does not enumerate the actual executor/observer model sequence.",
                "Current source HEAD is not established as the build used by historical runs.",
                "Null cost fields do not mean zero cost; no cost-matched estimate is possible from these fields.",
                "Hashes identify inspected original files; raw logs and environment settings are not exported.",
            ],
        },
        "summary": {
            "result_files": len(results), "distinct_tasks": len(task_counts),
            "tasks_with_multiple_results": sum(count > 1 for count in task_counts.values()),
            "reward_counts": dict(sorted(reward_counts.items())),
            "config_model_name_counts": dict(sorted(model_counts.items())),
            "populated_cost_fields": sum(row["cost_usd"] is not None for row in results),
            "log_files": len(logs), "results_with_logs": sum(row["has_log"] for row in results),
            "logs_without_results": sum(not row["has_result"] for row in logs),
            "logs_with_flips": sum(bool(row["flips"]) for row in logs),
            "logs_with_multiple_executor_models": sum(len({item["model"] for item in row["turns"] + row["flips"]}) > 1 for row in logs),
            "logs_with_swarm_start": sum(bool(row["markers"]["starting swarm explore"]) for row in logs),
            "logs_with_swarm_complete": sum(bool(row["markers"]["swarm explore complete"]) for row in logs),
        },
        "results": results, "logs": logs,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    record = audit(args.source.resolve())
    args.destination.write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record["summary"], indent=2))
