#!/usr/bin/env python3
"""Pair two release binaries for CLI and authenticated MCP cost measurements.

macOS production fixture, private identities, loopback-only daemon. Samples
alternate before/after order. No model or provider is called. This measures
client wall time and bytes, not tokens or billing. Benchmark sample counts and
RPC/cleanup watchdogs are explicit arguments, not production execution limits.
"""

import argparse
from contextlib import ExitStack
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import select
import statistics
import subprocess
import time

from client_qualification.production import ProductionDaemon
from client_qualification.runtime import Profile


def compact(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def summary(samples):
    return {"samples_ms": samples, "median_ms": statistics.median(samples),
            "min_ms": min(samples), "max_ms": max(samples)}


def identity(binary, timeout):
    return {"path": str(binary), "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            "version": subprocess.check_output([binary, "--version"], text=True, timeout=timeout).strip()}


class Bridge:
    def __init__(self, binary, daemon, timeout):
        self.timeout = timeout
        self.sequence = 0
        self.buffer = b""
        self.process = subprocess.Popen(
            [str(binary), "--home", str(daemon.home), "--credential", str(daemon.credential),
             "--session", str(daemon.session), "mcp"], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=daemon.profile.root,
            env=daemon.environment())

    def send(self, value):
        self.process.stdin.write(compact(value) + b"\n")
        self.process.stdin.flush()

    def request(self, method, params):
        self.sequence += 1
        self.send({"jsonrpc": "2.0", "id": self.sequence, "method": method, "params": params})
        deadline = time.monotonic() + self.timeout
        while b"\n" not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([self.process.stdout], [], [], remaining)[0]:
                raise RuntimeError("MCP benchmark RPC watchdog expired")
            chunk = os.read(self.process.stdout.fileno(), 65536)
            if not chunk:
                raise RuntimeError("MCP bridge closed before response")
            self.buffer += chunk
        line, self.buffer = self.buffer.split(b"\n", 1)
        value = json.loads(line)
        if value.get("id") != self.sequence or "error" in value:
            raise RuntimeError("MCP benchmark received unexpected response")
        return value["result"]

    def initialize(self):
        self.request("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                    "clientInfo": {"name": "cost-benchmark", "version": "1"}})
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def close(self):
        try:
            try:
                self.process.stdin.close()
            except BrokenPipeError:
                pass
            try:
                self.process.wait(timeout=self.timeout)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=self.timeout)
                raise RuntimeError("MCP benchmark needed forced cleanup") from None
            stderr = self.process.stderr.read()
            if self.process.returncode != 0 or stderr:
                raise RuntimeError("MCP benchmark bridge did not exit cleanly")
        finally:
            self.process.stdout.close()
            self.process.stderr.close()


def compare(args):
    binaries = {label: Path(getattr(args, label)).resolve() for label in ("before", "after")}
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        raise ValueError("output directory must be empty")
    report = {"schema": 1, "host": {"system": platform.system(), "machine": platform.machine()},
              "samples": args.samples, "warmups": args.warmups,
              "operation_timeout_seconds": args.operation_timeout_seconds,
              "binaries": {label: identity(binary, args.operation_timeout_seconds) for label, binary in binaries.items()},
              "method": "alternating before/after, warm filesystem caches, new CLI processes with identical argv[0]; persistent MCP bridges",
              "boundary": "same baseline daemon for both clients; core benchmarks measure daemon-side changes separately; no provider token or dollar measurement",
              "cli": {}, "mcp": {}}
    profile = Profile(output, "fixture")
    try:
        with ProductionDaemon(profile, binaries["before"], args.operation_timeout_seconds) as daemon:
            commands = {"version": ["--version"], "help": ["--help"],
                        "contract_json": ["--json", "contract"],
                        "blueprint_contract_json": ["--json", "blueprint", "contract"],
                        "pending_json": ["--home", str(daemon.home), "--credential", str(daemon.credential),
                                         "--session", str(daemon.session), "--json", "pending", "--goal", daemon.goal]}
            for name, arguments in commands.items():
                samples = {label: [] for label in binaries}
                responses = {}
                for iteration in range(args.warmups + args.samples):
                    for label in (list(binaries) if iteration % 2 == 0 else list(reversed(binaries))):
                        start = time.perf_counter_ns()
                        result = subprocess.run(["locust", *arguments], executable=binaries[label], capture_output=True,
                                                cwd=profile.root, env=profile.environment(binaries[label]),
                                                timeout=args.operation_timeout_seconds, check=True)
                        elapsed = (time.perf_counter_ns() - start) / 1e6
                        if result.stderr:
                            raise RuntimeError("CLI benchmark wrote unexpected stderr")
                        if iteration >= args.warmups:
                            samples[label].append(elapsed)
                        previous = responses.setdefault(label, result.stdout)
                        if previous != result.stdout:
                            raise RuntimeError("read-only CLI response changed during measurement")
                equal = responses["before"] == responses["after"]
                if name != "version" and not equal:
                    raise RuntimeError(f"CLI {name} changed output")
                report["cli"][name] = {label: {**summary(samples[label]), "stdout_bytes": len(responses[label])}
                                       for label in binaries}
                report["cli"][name]["responses_equal"] = equal
            with ExitStack() as cleanup:
                bridges = {}
                first = {}
                for label, binary in binaries.items():
                    start = time.perf_counter_ns()
                    bridge = Bridge(binary, daemon, args.operation_timeout_seconds)
                    cleanup.callback(bridge.close)
                    bridge.initialize()
                    first[label] = bridge.request("tools/list", {})
                    report["mcp"][label] = {"first_observation_initialize_and_list_ms": (time.perf_counter_ns() - start) / 1e6,
                                            "tool_count": len(first[label]["tools"]),
                                            "tools_compact_bytes": len(compact(first[label]["tools"])),
                                            "tools_sha256": hashlib.sha256(compact(first[label]["tools"])).hexdigest()}
                    bridges[label] = bridge
                if [tool["name"] for tool in first["before"]["tools"]] != [tool["name"] for tool in first["after"]["tools"]]:
                    raise RuntimeError("tool inventory changed")
                samples = {label: [] for label in binaries}
                for iteration in range(args.warmups + args.samples):
                    for label in (list(binaries) if iteration % 2 == 0 else list(reversed(binaries))):
                        start = time.perf_counter_ns()
                        value = bridges[label].request("tools/list", {})
                        elapsed = (time.perf_counter_ns() - start) / 1e6
                        if value != first[label]:
                            raise RuntimeError("tool list changed during measurement")
                        if iteration >= args.warmups:
                            samples[label].append(elapsed)
                for label in binaries:
                    report["mcp"][label]["warm_list"] = summary(samples[label])
        report["clean_exit"] = True
    except BaseException as error:
        report["error"] = type(error).__name__ + ": " + str(error)
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        raise
    finally:
        profile.close()
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", required=True)
    parser.add_argument("--after", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--samples", type=int, required=True)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--operation-timeout-seconds", type=float, default=30)
    args = parser.parse_args()
    if args.samples < 1 or args.warmups < 0 or not math.isfinite(args.operation_timeout_seconds) or args.operation_timeout_seconds <= 0:
        parser.error("samples and operation timeout must be positive; warmups must be nonnegative")
    report = compare(args)
    print(json.dumps({"cli": {name: {label: value[label]["median_ms"] for label in ("before", "after")}
                              for name, value in report["cli"].items()},
                      "mcp": {label: {"bytes": value["tools_compact_bytes"], "median_ms": value["warm_list"]["median_ms"]}
                              for label, value in report["mcp"].items()}}, indent=2))


if __name__ == "__main__":
    main()
