#!/usr/bin/env python3
"""Export or check public authoring artifacts against the built Locust binary.

Build with `cargo build --locked -p locust`, then run this script. `--write`
updates exports; the default checks drift and validates each bundled example.
It also runs every conformance case through `blueprint validate` and records
the results as vectors for the site's TypeScript checks, and fails when a
diagnostic code in the Rust source has no case.
"""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
CASES = ROOT / "docs/reference/conformance/organization.cases.json"
VECTORS = ROOT / "docs/reference/generated/organization.vectors.json"
SOURCES = [ROOT / "crates/locust-core/src/organization.rs"] + sorted(
    path for path in (ROOT / "crates/locust-core/src/organization").glob("*.rs")
    if path.name != "tests.rs"
)
CODE = re.compile(r'"((?:invalid|unknown|unsupported|duplicate|selector|empty|impossible|unavailable|flow)_[a-z_]+)"')


def run(binary: Path, *arguments: str, source: str | None = None):
    process = subprocess.run(
        [str(binary), "--json", "blueprint", *arguments],
        input=source, text=True, capture_output=True, check=True,
    )
    envelope = json.loads(process.stdout)
    if not envelope["ok"]:
        raise ValueError(f"blueprint {arguments[0]} did not succeed")
    return envelope["result"]


def inspect(binary: Path, source: str):
    """Run `blueprint validate` on exact text; invalid documents are results, not errors."""
    process = subprocess.run(
        [str(binary), "--json", "blueprint", "validate", "-"],
        input=source, text=True, capture_output=True,
    )
    envelope = json.loads(process.stdout)
    if "result" not in envelope or not isinstance(envelope["result"], dict):
        raise ValueError(f"blueprint validate returned no inspection: {process.stdout[:200]}")
    return envelope["result"]


def vectors(binary: Path, examples: dict[str, str]):
    cases = json.loads(CASES.read_text(encoding="utf-8"))["cases"]
    ids = [case["id"] for case in cases]
    if len(ids) != len(set(ids)):
        raise ValueError("conformance case ids must be unique")
    results = []
    for case in cases:
        source = examples[case["example"]] if "example" in case else case["source"]
        results.append({"id": case["id"], "source": source, "result": inspect(binary, source)})
    covered = {d["code"] for entry in results for d in entry["result"]["diagnostics"]}
    declared = {code for path in SOURCES for code in CODE.findall(path.read_text(encoding="utf-8"))}
    missing = sorted(declared - covered)
    if missing:
        raise ValueError("diagnostic codes without a conformance case: " + ", ".join(missing))
    return {"generated_by": "scripts/check_blueprints.py", "cases": results}


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
        sources = {}
        for example in examples:
            name = example["name"]
            if not name or any(character not in "abcdefghijklmnopqrstuvwxyz0123456789-" for character in name):
                raise ValueError(f"invalid exported example name: {name!r}")
            source = encoded(run(binary, "example", name))
            inspection = run(binary, "validate", "-", source=source)
            if not inspection["valid"]:
                raise ValueError(f"invalid bundled example: {name}")
            artifacts[ROOT / f"examples/blueprints/{name}.json"] = source
            sources[name] = source
        artifacts[VECTORS] = encoded(vectors(binary, sources))
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
        print(f"Blueprint exports {'written' if args.write else 'verified'}; {len(examples)} examples validate; conformance vectors cover every diagnostic code.")
        return 0
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"Blueprint export check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
