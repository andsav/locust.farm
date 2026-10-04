#!/usr/bin/env python3
"""Print a compact digest of one or more run directories written by run.py.

Usage: python3 report.py ../results/<stamp> [...]   (default: the newest run)
"""

import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent / "results"


def digest(run):
    report = json.loads((run / "results.json").read_text())
    print(f"== {run.name}  profile={report.get('network_profile')}  total={report.get('total_seconds')}s")
    for name, entry in report["scenarios"].items():
        print(f"  {name:24} {'PASS' if entry['passed'] else 'FAIL'} {entry['seconds']:>7}s")
        for failure in entry.get("failures", []):
            print(f"      failure: {failure[:300]}")
        summary = json.loads(Path(entry["summary"]).read_text())
        for item in summary.get("results", []):
            keys = ("combo", "case", "role", "stopped_seconds", "restart", "passed", "join_seconds",
                    "catch_up_seconds", "own_write_spread_seconds", "max_write_latency_on_others_s",
                    "acknowledged", "selected_paths", "first_write_arrival_seconds",
                    "member_synced_with_coordinator_seconds", "ticket_hint_kinds", "failure")
            line = {k: item[k] for k in keys if k in item}
            if "failure" in line:
                line["failure"] = line["failure"][:160]
            print(f"      {json.dumps(line)}")
            for move in item.get("moves", []):
                print(f"        move {json.dumps(move)[:400]}")
        slow = {k: v for k, v in summary.get("wait_seconds", {}).items() if v >= 5}
        if slow:
            print(f"      waits >= 5 s: {json.dumps(slow)}")
        for key in ("readable_everywhere_seconds", "join_paths", "version_lines", "daemon_versions"):
            if key in summary:
                print(f"      {key}: {json.dumps(summary[key])}")


def main(argv):
    runs = [Path(a) for a in argv] or [max(ROOT.iterdir(), key=lambda p: p.name)]
    for run in runs:
        digest(run)


if __name__ == "__main__":
    main(sys.argv[1:])
