#!/usr/bin/env python3
"""Checks that the simulator can fail: injects one fault at a time into the
product code of the scratch copy, runs the simulator, and restores the file.

Usage: mutate.py [name ...]   (no names: every mutation)
Run from anywhere; paths are relative to this file. Needs CARGO_TARGET_DIR.
"""
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.environ.get("MUTATE_WORK", os.path.join(HERE, "work"))
SRC = os.path.join(WORK, "crates", "locust-core", "src")

MUTATIONS = {
    # A goal that changes while an exchange about it is in flight is not
    # reconciled again when that exchange ends; anti-entropy finds it later.
    "drop-rerun-after-change-in-flight": (
        "sync/driver.rs",
        "            if !link.due || link.in_flight.is_some() || !ready {\n",
        "            if link.in_flight.is_some() {\n                link.due = false;\n            }\n"
        "            if !link.due || link.in_flight.is_some() || !ready {\n",
    ),
    # The initiator never asks for the content keys it lacks.
    "skip-key-request": (
        "sync/initiator.rs",
        "                        for epoch in replica.wanted_keys() {\n",
        "                        for epoch in replica.wanted_keys().into_iter().take(0) {\n",
    ),
    # Claims are not loaded when the daemon starts.
    "forget-claims-on-restart": (
        "node/mod.rs",
        "    Space::Claim,\n    Space::Cursor,\n];",
        "    Space::Invite,\n    Space::Cursor,\n];",
    ),
    # A join presented again after its admission is refused instead of
    # confirmed, so a join exchange cut after the admission strands the joiner.
    "refuse-repeated-join": (
        "node/peers.rs",
        "                && entry.state().members.get(&member) == Some(remote)\n            {\n                Ok(",
        "                && entry.state().members.get(&member) == Some(remote)\n            {\n                Err(refused)?;\n                Ok(",
    ),
    # A failed exchange is not retried; only anti-entropy opens the next one.
    "no-retry-after-failure": (
        "sync/driver.rs",
        "        } else {\n            link.due = true;\n",
        "        } else {\n",
    ),
    # No periodic reconciliation: only changes and failures open exchanges.
    "no-anti-entropy": (
        "sync/driver.rs",
        "                    .is_none_or(|last| now_ms.saturating_sub(last) >= ANTI_ENTROPY_MS);",
        "                    .is_none_or(|_| false);",
    ),
}


MUTATIONS["no-retry-and-no-anti-entropy"] = [
    MUTATIONS["no-retry-after-failure"],
    MUTATIONS["no-anti-entropy"],
]


def run(args, env_extra):
    env = dict(os.environ, **env_extra)
    cmd = ["cargo", "test", "--locked", "-p", "locust-core", "--lib"] + args
    out = subprocess.run(cmd, cwd=WORK, env=env, capture_output=True, text=True)
    return out.stdout + out.stderr


def summary(output):
    lines = [l for l in output.splitlines() if l.startswith("sim ") or "FAILED at" in l]
    failed = re.findall(r"^sim \w+: .*?, (\d+) failed;", output, re.M)
    return lines, [int(n) for n in failed]


def main():
    names = sys.argv[1:] or list(MUTATIONS)
    seeds = os.environ.get("MUTATE_SEEDS", "120")
    for name in names:
        edits = MUTATIONS[name]
        edits = edits if isinstance(edits, list) else [edits]
        originals = {}
        for path, old, new in edits:
            full = os.path.join(SRC, path)
            text = open(full).read()
            originals.setdefault(full, text)
            assert text.count(old) == 1, f"{name}: site not found exactly once"
            open(full, "w").write(text.replace(old, new))
        try:
            print(f"== {name} ({', '.join(sorted({e[0] for e in edits}))})", flush=True)
            for scenario in ("guide", "prompt"):
                output = run(
                    ["node::sim::tests::sim_many", "--", "--ignored", "--nocapture"],
                    {"LOCUST_SIM_SEEDS": seeds, "LOCUST_SIM_SCENARIO": scenario},
                )
                lines, failed = summary(output)
                if not failed:
                    print(output[-3000:])
                    raise SystemExit(f"{name}: the run did not complete")
                first = [l for l in lines if "FAILED at" in l][:2]
                print(f"   {scenario}: {failed[0]} of {seeds} seeds fail", flush=True)
                for line in first:
                    print(f"      {line.strip()[:240]}")
        finally:
            for full, text in originals.items():
                open(full, "w").write(text)
    print("restored every file")


if __name__ == "__main__":
    main()
