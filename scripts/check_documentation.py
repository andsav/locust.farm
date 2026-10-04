#!/usr/bin/env python3
"""Execute marked manual recipes against an explicitly selected local binary.

Only repository-authored bash fences beginning with `# locust-doc-test: NAME`
are executable. Runs use fresh disposable state and loopback-only daemon settings.
The optional --timeout is an explicit campaign watchdog, never a runtime limit.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MARKER = re.compile(r"# locust-doc-test: ([a-z0-9-]+)\n")


def recipes(source: str) -> list[tuple[str, str]]:
    found = []
    for block in re.findall(r"^```bash\n(.*?)^```\s*$", source, re.M | re.S):
        if not block.startswith("# locust-doc-test:"):
            continue
        match = MARKER.match(block)
        if not match:
            raise ValueError("invalid documentation recipe marker")
        name = match[1]
        if name in {name for name, _ in found}:
            raise ValueError(f"duplicate documentation recipe {name}")
        if not block[match.end():].strip():
            raise ValueError(f"empty documentation recipe {name}")
        found.append((name, block))
    if source.count("# locust-doc-test:") != len(found):
        raise ValueError("recipe marker must begin a closed bash fence")
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--timeout", type=float, help="explicit per-recipe wall time in seconds; omitted means no watchdog")
    parser.add_argument("--output", type=Path, help="optional sanitized JSON evidence")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if args.timeout is not None and args.timeout <= 0:
        parser.error("--timeout must be positive")
    cases = []
    for page in sorted((ROOT / "docs/guide").glob("*.md")):
        cases.extend((page, name, source) for name, source in recipes(page.read_text()))
    if not cases:
        parser.error("no executable manual recipes found")
    version = subprocess.run([str(binary), "--json", "--version"], check=True,
        capture_output=True, text=True, timeout=args.timeout)
    record = {
        "format": "locust-documentation-recipes-v1",
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "reported_version": json.loads(version.stdout)["result"]["version"],
        "timeout_seconds": args.timeout,
        "cases": [],
    }
    for page, name, source in cases:
        with tempfile.TemporaryDirectory(prefix="locust-doc-") as temp:
            environment = {key: value for key, value in os.environ.items() if not key.startswith("LOCUST_")}
            environment.update(LOCUST_BIN=str(binary), LOCUST_DOC_DIR=temp)
            process = subprocess.Popen(
                ["bash", "-c", source], cwd=temp, env=environment,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                start_new_session=True,
            )
            timed_out = False
            try:
                stdout, stderr = process.communicate(timeout=args.timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(process.pid, signal.SIGKILL)
                stdout, stderr = process.communicate()
            passed = not timed_out and process.returncode == 0
            observation = {
                "page": str(page.relative_to(ROOT)), "name": name,
                "source_sha256": hashlib.sha256(source.encode()).hexdigest(),
                "passed": passed, "timed_out": timed_out,
                "exit_code": process.returncode,
                "stdout": stdout.replace(temp, "<temporary>"),
                "stderr": stderr.replace(temp, "<temporary>"),
            }
            record["cases"].append(observation)
            print(f"{'PASS' if passed else 'FAIL'} {name}")
            if not passed:
                print(observation["stderr"])
    record["binary_unchanged"] = hashlib.sha256(binary.read_bytes()).hexdigest() == record["binary_sha256"]
    if not record["binary_unchanged"]:
        print("FAIL selected binary changed during recipe checks")
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(record, indent=2) + "\n")
    return 0 if record["binary_unchanged"] and all(item["passed"] for item in record["cases"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
