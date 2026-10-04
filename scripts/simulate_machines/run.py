#!/usr/bin/env python3
"""Run simulated multi-machine Locust scenarios on one Mac.

Each simulated machine is one `locust daemon run` process of a real binary with
its own private --home under /tmp/locust-sim-<n>, driven only through the CLI.
Usage:
  python3 scripts/simulate_machines/run.py --list
  python3 scripts/simulate_machines/run.py --binary PATH --quick
  python3 scripts/simulate_machines/run.py --binary PATH [--mixed-binary PATH] [scenario ...]
Without scenario names or --quick, every scenario runs (about half an hour).
--quick runs the short ones (under three minutes). The mixed-build scenario
needs --mixed-binary, a second build with another version line.
Results: output/sim/<stamp>/<scenario>/{summary.json,transcript.jsonl} and
output/sim/<stamp>/results.json. Exit status 0 only when every scenario passed.

What this cannot show, and real machines can: separate network stacks and
addresses, address translation, a path forced through the relay, real
local-network discovery, operating-system sleep and a first-time download.
"""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import signal
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
import simlib  # noqa: E402
import scen_basic  # noqa: E402
import scen_faults  # noqa: E402
import scen_network  # noqa: E402
import scen_probe  # noqa: E402

SCENARIOS = {**scen_basic.SCENARIOS, **scen_faults.SCENARIOS, **scen_network.SCENARIOS,
             **scen_probe.SCENARIOS}
ORDER = ["two-machines-complete", "three-machines", "sleep", "crash", "new-address",
         "network-modes", "mixed-build"]
QUICK = ["two-machines-complete", "three-machines", "crash"]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("scenarios", nargs="*", help="names to run (default: all, in the listed order)")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--timeout", type=float, default=90, help="default deadline per wait/command, seconds")
    parser.add_argument("--network", choices=sorted(simlib.PROFILES), default="lan",
                        help="profile for scenarios that do not choose their own (default: lan)")
    parser.add_argument("--out", type=Path, default=simlib.ROOT / "output" / "sim")
    parser.add_argument("--binary", type=Path, default=os.environ.get("SIM_BIN"),
                        help="the locust binary every simulated machine runs (or set SIM_BIN)")
    parser.add_argument("--mixed-binary", type=Path, default=os.environ.get("SIM_MIXED_BIN"),
                        help="a second build with another version line, for mixed-build")
    parser.add_argument("--quick", action="store_true", help="only the short scenarios")
    args = parser.parse_args(argv)
    names = [n for n in ORDER if n in SCENARIOS] + sorted(set(SCENARIOS) - set(ORDER))
    if args.list:
        print("\n".join(names))
        return 0
    if args.binary is None:
        parser.error("--binary is required (or set SIM_BIN)")
    simlib.BINARIES["candidate"] = Path(args.binary).resolve()
    if args.mixed_binary is not None:
        simlib.BINARIES["mixed"] = Path(args.mixed_binary).resolve()
    if args.scenarios:
        chosen = args.scenarios
    else:
        chosen = [n for n in (QUICK if args.quick else names)
                  if n != "mixed-build" or "mixed" in simlib.BINARIES]
        if args.quick and "mixed" in simlib.BINARIES:
            chosen.append("mixed-build")
    unknown = [n for n in chosen if n not in SCENARIOS]
    if unknown:
        parser.error(f"unknown scenario(s): {unknown}; see --list")
    if "mixed-build" in chosen and "mixed" not in simlib.BINARIES:
        parser.error("mixed-build needs --mixed-binary")

    def interrupted(signum, _frame):
        simlib.reap_all()
        raise SystemExit(128 + signum)

    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    stale = simlib.clean_stale_homes()
    if stale:
        print(f"removed stale homes from an aborted run: {stale}", flush=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + os.urandom(2).hex()
    out = args.out / stamp
    out.mkdir(parents=True)
    report = {"network_profile": args.network, "started_at": datetime.now(timezone.utc).isoformat(), "scenarios": {}}
    total = time.monotonic()
    try:
        for name in chosen:
            began = time.monotonic()
            cluster = simlib.Cluster(name, out / name, args.timeout, args.network)
            print(f"[{name}] running", flush=True)
            passed = cluster.run_scenario(SCENARIOS[name])
            entry = {"passed": passed, "seconds": round(time.monotonic() - began, 1),
                     "failures": cluster.summary["failures"],
                     "summary": str(out / name / "summary.json"),
                     "reproduce": f"python3 {Path(__file__).resolve()} --timeout {args.timeout:g} --network {args.network} {name}"}
            for key in ("results", "readable_everywhere_seconds", "join_seconds", "join_paths", "version_lines", "daemon_versions"):
                if key in cluster.summary:
                    entry[key] = cluster.summary[key]
            report["scenarios"][name] = entry
            print(f"[{name}] {'PASS' if passed else 'FAIL'} in {entry['seconds']}s"
                  + ("" if passed else f": {entry['failures']}"), flush=True)
    finally:
        simlib.reap_all()
        report["total_seconds"] = round(time.monotonic() - total, 1)
        report["finished_at"] = datetime.now(timezone.utc).isoformat()
        (out / "results.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    ok = all(entry["passed"] for entry in report["scenarios"].values())
    print(json.dumps({"ok": ok, "total_seconds": report["total_seconds"], "results": str(out / "results.json")}))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
