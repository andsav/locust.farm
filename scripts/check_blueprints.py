#!/usr/bin/env python3
"""Export or check public authoring artifacts against the built Locust binary.

Build with `cargo build --locked -p locust`, then run this script. `--write`
updates exports; the default checks drift and validates each bundled example.
"""

import argparse
import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]


def run(binary: Path, *arguments: str, source: str | None = None):
    process = subprocess.run(
        [str(binary), "--json", "blueprint", *arguments],
        input=source, text=True, capture_output=True, check=True,
    )
    envelope = json.loads(process.stdout)
    if not envelope["ok"]:
        raise ValueError(f"blueprint {arguments[0]} did not succeed")
    return envelope["result"]


def encoded(value) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/locust")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    binary = args.binary.resolve()
    try:
        contract = run(binary, "contract")
        artifacts = {
            ROOT / "docs/reference/generated/organization.contract.json": encoded(contract),
            ROOT / "docs/reference/generated/organization.schema.json": encoded(run(binary, "schema")),
        }
        runtime = subprocess.run([str(binary), "--json", "contract"], text=True, capture_output=True, check=True)
        envelope = json.loads(runtime.stdout)
        if not envelope["ok"]:
            raise ValueError("runtime contract export did not succeed")
        artifacts[ROOT / "docs/reference/generated/runtime.contract.json"] = encoded(envelope["result"])
        examples = run(binary, "examples")
        for example in examples:
            name = example["name"]
            if not name or any(character not in "abcdefghijklmnopqrstuvwxyz0123456789-" for character in name):
                raise ValueError(f"invalid exported example name: {name!r}")
            source = encoded(run(binary, "example", name))
            inspection = run(binary, "validate", "-", source=source)
            if not inspection["valid"]:
                raise ValueError(f"invalid bundled example: {name}")
            artifacts[ROOT / f"examples/blueprints/{name}.json"] = source
        differences = []
        for path, expected in artifacts.items():
            if args.write:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(expected, encoding="utf-8")
            elif not path.is_file() or path.read_text(encoding="utf-8") != expected:
                differences.append(str(path.relative_to(ROOT)))
        # Removed examples must not remain in the public contract.
        stale = set((ROOT / "examples/blueprints").glob("*.json")) - artifacts.keys()
        differences.extend(str(path.relative_to(ROOT)) for path in sorted(stale))
        if differences:
            print("Blueprint export drift: " + ", ".join(differences), file=sys.stderr)
            return 1
        print(f"Blueprint exports {'written' if args.write else 'verified'}; {len(examples)} examples validate.")
        return 0
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"Blueprint export check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
