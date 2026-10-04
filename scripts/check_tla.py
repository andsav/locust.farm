#!/usr/bin/env python3
"""Run pinned TLC safety configurations; retain completion and trace evidence.

Tools and raw results live in ignored output/tla. No system Java installation or
Locust state is used. TLC v1.7.4 lacks newer -dumpTrace support: normalize its
text counterexample states without evaluating TLA expressions. Variable values
are retained as lossless TLA text, not falsely represented as decoded JSON.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import time
import urllib.request
import uuid


ROOT = Path(__file__).resolve().parents[1]
MODELS = ROOT / "research/tla"
MANIFEST = MODELS / "toolchain.json"
CASES = MODELS / "cases.json"
TOOLS = ROOT / "output/tla/tools"
COUNTS = re.compile(r"([\d,]+) states generated, ([\d,]+) distinct states found, ([\d,]+) states left on queue")
STATE = re.compile(r"^State (\d+): <([^\n]+)>\s*$", re.M)
VARIABLE = re.compile(r"^(?:/\\ )?([A-Za-z_]\w*) = (.*)$", re.M)
VIOLATION = re.compile(r"^Error: Invariant (\w+) is violated\.", re.M)


class CheckError(Exception):
    pass


def sha256(path):
    with Path(path).open("rb") as source:
        digest = hashlib.file_digest(source, "sha256")
    return digest.hexdigest()


def verify(path, expected):
    if not Path(path).is_file() or sha256(path) != expected:
        raise CheckError(f"missing or mismatched checksum: {path}")


def download(url, path, expected):
    """An existing bad cache is refused; only explicit bootstrap downloads."""
    path = Path(path)
    if path.exists():
        verify(path, expected)
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".download-" + uuid.uuid4().hex)
    try:
        request = urllib.request.Request(url, headers={"User-Agent": "locust-tla-check"})
        with urllib.request.urlopen(request, timeout=60) as response, temporary.open("wb") as output:
            shutil.copyfileobj(response, output)
        verify(temporary, expected)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def extract_jdk(archive, destination):
    # data_filter rejects archive paths/links escaping the destination.
    # Python 3.12+ is required so extraction has a consistent safe filter.
    with tarfile.open(archive) as source:
        source.extractall(destination, filter="data")


def verify_runtime(archive, destination):
    """Verify extracted files and links against the already verified archive."""
    destination = Path(destination).resolve()
    with tarfile.open(archive) as source:
        for member in source:
            path = destination / member.name
            if not path.resolve().is_relative_to(destination):
                raise CheckError(f"runtime path escapes cache: {member.name}")
            if member.issym():
                if not path.is_symlink() or os.readlink(path) != member.linkname:
                    raise CheckError(f"runtime link mismatch: {member.name}")
            elif member.isfile() or member.islnk():
                if not path.is_file() or path.is_symlink():
                    raise CheckError(f"runtime file missing: {member.name}")
                with source.extractfile(member) as content:
                    expected = hashlib.file_digest(content, "sha256").hexdigest()
                if sha256(path) != expected:
                    raise CheckError(f"runtime file mismatch: {member.name}")


def clean_java_env():
    env = dict(os.environ)
    for name in ("JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS", "CLASSPATH"):
        env.pop(name, None)
    return env


def tools(manifest, *, bootstrap=False, directory=TOOLS):
    key = f"{platform.system()}-{platform.machine()}"
    try:
        target = manifest["java"]["platforms"][key]
    except KeyError as error:
        raise CheckError(f"no pinned JDK for {key}") from error
    directory = Path(directory)
    jar = directory / "tla2tools.jar"
    archive = directory / target["archive"]
    java = directory / target["java"]
    if bootstrap:
        download(manifest["tlc"]["url"], jar, manifest["tlc"]["sha256"])
        download(target["url"], archive, target["sha256"])
        if not java.exists():
            extract_jdk(archive, directory)
    verify(jar, manifest["tlc"]["sha256"])
    verify(archive, target["sha256"])
    if not java.is_file():
        raise CheckError("pinned JDK missing; run with --bootstrap")
    verify_runtime(archive, directory)
    result = subprocess.run([str(java), "-XshowSettings:properties", "-version"],
                            capture_output=True, text=True, timeout=20, env=clean_java_env())
    properties = dict(re.findall(r"^\s+([\w.]+) = (.+)$", result.stderr, re.M))
    required = {"java.vendor": manifest["java"]["vendor"],
                "java.runtime.version": manifest["java"]["runtime_version"], "os.arch": target["arch"]}
    if result.returncode or any(properties.get(key) != value for key, value in required.items()):
        raise CheckError(f"pinned Java identity mismatch: {properties}")
    return java, jar, {"platform": key, "archive_sha256": target["sha256"],
                       "executable_sha256": sha256(java), "properties": properties}


def trace_states(output):
    """Parse full TLC state blocks, retaining opaque TLA values verbatim."""
    if "Error: The behavior up to this point is:" not in output:
        return []
    tail = output.split("Error: The behavior up to this point is:", 1)[1]
    tail = re.split(r"\n(?:The coverage statistics|[\d,]+ states generated|Finished in)", tail, maxsplit=1)[0]
    headers = list(STATE.finditer(tail))
    states = []
    for index, header in enumerate(headers):
        end = headers[index + 1].start() if index + 1 < len(headers) else len(tail)
        body = tail[header.end():end].strip()
        variables = list(VARIABLE.finditer(body))
        values = {}
        for number, variable in enumerate(variables):
            stop = variables[number + 1].start() if number + 1 < len(variables) else len(body)
            values[variable[1]] = body[variable.start(2):stop].strip()
        if not values:
            raise CheckError("counterexample state has no readable variables")
        states.append({"state": int(header[1]), "action": header[2], "variables_tla": values})
    if states and [state["state"] for state in states] != list(range(1, len(states) + 1)):
        raise CheckError("counterexample state numbering is incomplete")
    return states


def classify(output, returncode, case, *, timed_out=False):
    if timed_out:
        return {"status": "incomplete", "reason": "timeout", "matched_expectation": False}
    count_matches = list(COUNTS.finditer(output))
    counts = ({key: int(value.replace(",", "")) for key, value in
               zip(("generated", "distinct", "queue"), count_matches[-1].groups())}
              if count_matches else None)
    finished = bool(re.search(r"^Finished in .+ at \(", output, re.M))
    name = VIOLATION.search(output)
    try:
        trace = trace_states(output)
    except CheckError as error:
        return {"status": "tool_failure", "reason": str(error), "matched_expectation": False}
    depth = re.search(r"The depth of the complete state graph search is (\d+)\.", output)
    collisions = re.findall(r"^\s+(calculated \(optimistic\)|based on the actual fingerprints):\s+val = ([\d.Ee+-]+)", output, re.M)
    result = {"counts": counts, "trace": trace, "matched_expectation": False,
              "depth": int(depth[1]) if depth else None,
              "fingerprint_collision_estimates": dict(collisions)}
    if returncode == 0 and finished and counts and counts["queue"] == 0 and \
            "Model checking completed. No error has been found." in output and not re.search(r"^Error:", output, re.M):
        result.update(status="complete", matched_expectation=case["expect"] == "pass")
    elif returncode == 12 and finished and counts and name and len(trace) >= 2:
        requirements = case.get("trace_require", [])
        matched = bool(requirements) and all(requirement and
                    all(trace[-1]["variables_tla"].get(key) == value for key, value in requirement.items())
                    for requirement in requirements)
        cursor = 0
        for requirement in case.get("trace_prefix", []):
            while cursor < len(trace) - 1 and not all(trace[cursor]["variables_tla"].get(key) == value
                                                     for key, value in requirement.items()):
                cursor += 1
            if not requirement or cursor >= len(trace) - 1:
                matched = False
                break
            cursor += 1
        result.update(status="counterexample", violation=name[1],
                      matched_expectation=case["expect"] == "violation" and
                      name[1] == case.get("violation") and bool(requirements) and matched)
    elif "Error: Deadlock reached." in output:
        result.update(status="unexpected_violation", reason="deadlock")
    else:
        result.update(status="tool_failure" if returncode else "incomplete",
                      reason="checker did not finish with an accepted result")
    return result


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def source_hashes(paths):
    files = set()
    for relative in paths:
        path = ROOT / relative
        if path.is_dir():
            files.update(path.rglob("*.rs"))
        else:
            files.add(path)
    return {path.relative_to(ROOT).as_posix(): sha256(path) for path in sorted(files)}


def validate_cases(registry, model_root=MODELS):
    ids = set()
    for case in registry["cases"]:
        if not re.fullmatch(r"[a-z0-9-]+", case["id"]) or case["id"] in ids:
            raise CheckError("invalid or duplicate case ID")
        ids.add(case["id"])
        for key in ("module", "config"):
            path = (model_root / case[key]).resolve()
            if not path.is_relative_to(model_root.resolve()) or not path.is_file():
                raise CheckError(f"invalid case {key}: {case[key]}")
        if case["expect"] not in ("pass", "violation"):
            raise CheckError("unknown expected outcome")
        if case["expect"] == "violation" and (not case.get("violation") or not case.get("trace_require")):
            raise CheckError("expected violation needs a named property and trace requirements")
        for requirement in case.get("trace_require", []) + case.get("trace_prefix", []):
            if not isinstance(requirement, dict) or not requirement or any(
                    not isinstance(key, str) or not isinstance(value, str)
                    for key, value in requirement.items()):
                raise CheckError("trace requirements must be nonempty maps of TLA variable text")
        if not case.get("properties"):
            raise CheckError("case declares no checked properties")
        # Configs in this registry use plain INVARIANT(S) names, without overrides.
        config = re.sub(r"\\\*[^\n]*", "", (model_root / case["config"]).read_text())
        sections = re.findall(r"^INVARIANTS?\b(.*?)(?=^[A-Z_]+\b|\Z)", config, re.M | re.S)
        configured = set(re.findall(r"\b\w+\b", " ".join(sections)))
        if configured != set(case["properties"]):
            raise CheckError(f"property/config mismatch: {case['id']}")
        if case["expect"] == "violation" and case["violation"] not in configured:
            raise CheckError("expected property is not configured")


def snapshot_inputs(destination):
    hashes = {}
    for source in sorted(MODELS.rglob("*")):
        if source.is_file() and source.suffix in (".tla", ".cfg"):
            target = destination / source.relative_to(MODELS)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(source.read_bytes())
            hashes[str(source.relative_to(ROOT))] = sha256(target)
    return hashes


def model_scope(registry, cases):
    """Keep the modeled protocol identity separate from the running checkout."""
    if all(case["kind"] == "runner-fixture" for case in cases):
        return {"kind": "runner-fixtures", "runtime_conformance_claimed": False}
    baseline = registry.get("model_baseline")
    if not isinstance(baseline, dict) or not re.fullmatch(r"[a-f0-9]{40}", baseline.get("source_commit", "")):
        raise CheckError("protocol model run requires an explicit source baseline")
    return {"kind": "protocol-models", "modeled_baseline": baseline,
            "runtime_conformance_claimed": False}


def run_case(case, java, jar, run_dir, *, memory_mb=1024, timeout=None, model_root=MODELS):
    directory = run_dir / case["id"]
    directory.mkdir()
    module, config = model_root / case["module"], model_root / case["config"]
    # Pinned TLC coverage instrumentation exhausts the heap before initial
    # states for recursive GoalLog operators. Named witness cases establish
    # action reachability separately, without altering safety exploration.
    command = [str(java), f"-Xmx{memory_mb}m", "-XX:+UseParallelGC", "-cp", str(jar),
               "tlc2.TLC", "-workers", "1", "-fp", "0", "-seed", "1",
               "-metadir", str(directory / "states"), "-config", str(config), str(module)]
    started = time.monotonic()
    process = subprocess.Popen(command, cwd=model_root, env=clean_java_env(), stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, text=True, start_new_session=True)
    timed_out = False
    deadline = timeout if timeout is not None else case.get("timeout_seconds", 120)
    try:
        output, _ = process.communicate(timeout=deadline)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGKILL)
        output, _ = process.communicate()
    (directory / "tlc.log").write_text(output)
    result = classify(output, process.returncode, case, timed_out=timed_out)
    result.update(case=case, command=command, elapsed_seconds=round(time.monotonic() - started, 3),
                  returncode=process.returncode, timeout_seconds=deadline, memory_mb=memory_mb,
                  mode="exhaustive-breadth-first", workers=1, fingerprint=0, seed=1,
                  coverage_instrumentation=False,
                  config_sha256=sha256(config), module_sha256=sha256(module),
                  configuration_tla=config.read_text(),
                  coverage_output=output.split("The coverage statistics", 1)[-1].split("End of statistics.", 1)[0]
                  if "The coverage statistics" in output else None)
    if result.get("trace"):
        (directory / "trace.json").write_text(json.dumps(result["trace"], indent=2) + "\n")
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bootstrap", action="store_true", help="fetch verified private tool cache")
    parser.add_argument("--suite", default="fast", choices=("fixtures", "fast", "extended"))
    parser.add_argument("--case", action="append", help="run only specified case IDs")
    parser.add_argument("--timeout", type=float, help="override per-case seconds (timeout never passes)")
    parser.add_argument("--memory-mb", type=int, default=4096)
    args = parser.parse_args(argv)
    if (args.timeout is not None and args.timeout <= 0) or args.memory_mb <= 0:
        parser.error("resource limits must be positive")
    try:
        if sys.version_info < (3, 12):
            raise CheckError("Python 3.12+ required for safe tool extraction")
        manifest_bytes, registry_bytes = MANIFEST.read_bytes(), CASES.read_bytes()
        runner_hash = sha256(Path(__file__))
        manifest, registry = json.loads(manifest_bytes), json.loads(registry_bytes)
        cases = [case for case in registry["cases"] if case["id"] in args.case] if args.case else \
                [case for case in registry["cases"] if args.suite in case["suite"]]
        if not cases or (args.case and set(args.case) != {case["id"] for case in cases}):
            raise CheckError("empty suite or unknown case")
        scope = model_scope(registry, cases)
        if scope.get("modeled_baseline", {}).get("status") == "historical":
            print("Historical protocol model check; current Rust conformance is not established.", flush=True)
        java, jar, identity = tools(manifest, bootstrap=args.bootstrap)
        run_dir = ROOT / "output/tla/runs" / (datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:8])
        run_dir.mkdir(parents=True)
        inputs = run_dir / "inputs"
        input_hashes = snapshot_inputs(inputs)
        (inputs / "toolchain.json").write_bytes(manifest_bytes)
        (inputs / "cases.json").write_bytes(registry_bytes)
        validate_cases(registry, inputs)
        record = {"schema": 1, "source_commit": git("rev-parse", "HEAD"),
                  "dirty_status": git("status", "--porcelain"),
                  "source_hashes": source_hashes(registry["source_paths"]),
                  "input_hashes": input_hashes,
                  "runner_sha256": runner_hash,
                  "toolchain_manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
                  "cases_manifest_sha256": hashlib.sha256(registry_bytes).hexdigest(),
                  "model_scope": scope,
                  "toolchain": manifest, "java": identity,
                  "platform": platform.platform(), "architecture": platform.machine(), "results": []}
        for case in cases:
            result = run_case(case, java, jar, run_dir, memory_mb=args.memory_mb,
                              timeout=args.timeout, model_root=inputs)
            record["results"].append(result)
            print(f"{case['id']}: {result['status']} ({'expected' if result['matched_expectation'] else 'UNEXPECTED'})", flush=True)
            if not result["matched_expectation"]:
                print(f"  {result.get('reason', 'property/scenario mismatch')}; violation={result.get('violation')}", flush=True)
            (run_dir / "results.json").write_text(json.dumps(record, indent=2) + "\n")
        record["source_changed_during_run"] = record["source_hashes"] != source_hashes(registry["source_paths"])
        (run_dir / "results.json").write_text(json.dumps(record, indent=2) + "\n")
        print(f"Evidence: {run_dir.relative_to(ROOT)}/results.json")
        return 0 if not record["source_changed_during_run"] and all(
            result["matched_expectation"] for result in record["results"]) else 1
    except (CheckError, OSError, subprocess.SubprocessError, ValueError, KeyError) as error:
        print(f"TLA check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
