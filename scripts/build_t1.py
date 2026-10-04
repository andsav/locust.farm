#!/usr/bin/env python3
"""Build one identified Apple Silicon artifact for the T1 machines.

This records build identity only. It does not qualify daemon behavior, deploy,
install, sign, notarize or transfer the binary. Run from committed Rust/build
inputs; unrelated docs and website work may remain dirty.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib
from typing import Callable

TARGET = "aarch64-apple-darwin"
BUILD_INPUTS = {
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "rust-toolchain",
    "scripts/build_t1.py",
    "scripts/tests/test_build_t1.py",
}
Runner = Callable[..., subprocess.CompletedProcess[str]]


class BuildError(Exception):
    def __init__(self, state: str, detail: str):
        super().__init__(detail)
        self.state = state
        self.detail = detail


def run_command(argv: list[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(argv, cwd=cwd, capture_output=True, text=True, check=False)
    except OSError as error:
        raise BuildError("command_unavailable", f"Cannot run {argv[0]}: {error}") from error


def checked(runner: Runner, argv: list[str], repo: Path) -> str:
    result = runner(argv, cwd=repo)
    if result.returncode:
        if result.stdout:
            print(result.stdout, file=sys.stderr, end="")
        if result.stderr:
            print(result.stderr, file=sys.stderr, end="")
        raise BuildError("command_failed", f"Command failed: {' '.join(argv)}")
    if result.stderr:
        print(result.stderr, file=sys.stderr, end="")
    return result.stdout.strip()


def relevant(path: str) -> bool:
    return path in BUILD_INPUTS or path.startswith(("crates/", ".cargo/"))


def dirty_inputs(status: str) -> list[str]:
    """Read porcelain -z, including both names of a rename/copy."""
    paths: list[str] = []
    records = iter(status.split("\0"))
    for record in records:
        if not record:
            continue
        if len(record) < 4 or record[2] != " ":
            raise BuildError("invalid_git_status", "Git returned malformed porcelain status")
        names = [record[3:]]
        if "R" in record[:2] or "C" in record[:2]:
            original = next(records, "")
            if not original:
                raise BuildError("invalid_git_status", "Git rename lacks its original path")
            names.append(original)
        paths.extend(name for name in names if relevant(name))
    return sorted(set(paths))


def source_identity(repo: Path, runner: Runner) -> str:
    commit = checked(runner, ["git", "rev-parse", "--verify", "HEAD"], repo)
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", commit):
        raise BuildError("invalid_git_commit", "HEAD is not a full Git object identifier")
    # Do not strip porcelain output: leading spaces are index/worktree status.
    result = runner(["git", "status", "--porcelain=v1", "-z", "--untracked-files=all"], cwd=repo)
    if result.returncode:
        raise BuildError("git_status_failed", "Cannot inspect source changes")
    dirty = dirty_inputs(result.stdout)
    ignored = runner(["git", "ls-files", "--others", "--ignored", "--exclude-standard", "-z", "--", "crates/", ".cargo/", *sorted(BUILD_INPUTS)], cwd=repo)
    if ignored.returncode:
        raise BuildError("git_status_failed", "Cannot inspect ignored build inputs")
    dirty = sorted(set(dirty) | {path for path in ignored.stdout.split("\0") if path and relevant(path)})
    if dirty:
        raise BuildError("dirty_build_inputs", "Commit Rust/build inputs before building: " + ", ".join(dirty))
    return commit


def package_version(repo: Path) -> str:
    package = tomllib.loads((repo / "crates/locust/Cargo.toml").read_text())["package"]
    version = package["version"]
    if isinstance(version, dict) and version.get("workspace") is True:
        version = tomllib.loads((repo / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    if not isinstance(version, str) or not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.+-]+)?", version):
        raise BuildError("invalid_package_version", "locust must declare a semantic package version")
    return version


def verify_version(output: str, version: str, commit: str) -> None:
    # Lane A owns embedding the commit. No build-time environment variable is
    # invented here. Accept its plain single-line format with both identities.
    if "\n" in output or not re.match(r"^locust\s", output):
        raise BuildError("version_contract_missing", "locust --version must print locust, its package version and Git commit; the name-only scaffold is not ready")
    versions = re.findall(r"(?<![0-9A-Za-z.])\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.+-]+)?(?![0-9A-Za-z.])", output)
    if version not in versions:
        raise BuildError("version_mismatch", f"locust --version does not identify package version {version}")
    commits = re.findall(r"(?<![0-9A-Za-z])[0-9a-fA-F]{7,64}(?![0-9A-Za-z])", output)
    if not commits:
        raise BuildError("version_contract_missing", "locust --version must include at least seven hexadecimal characters of the built Git commit")
    if not any(commit.startswith(value.lower()) for value in commits):
        raise BuildError("version_commit_mismatch", "locust --version does not identify the committed source that was built")


def verify_target(binary: Path) -> None:
    with binary.open("rb") as stream:
        header = stream.read(32)
    if len(header) != 32 or header[:4] != b"\xcf\xfa\xed\xfe" or struct.unpack_from("<I", header, 4)[0] != 0x0100000C:
        raise BuildError("wrong_binary_target", "Expected a thin 64-bit arm64 Mach-O binary for aarch64-apple-darwin")
    if not os.access(binary, os.X_OK):
        raise BuildError("binary_not_executable", "The built locust binary lacks executable permissions")


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_artifact(directory: Path, expected: dict) -> dict:
    """Verify persisted bytes and their records; never overwrite a collision."""
    try:
        metadata = json.loads((directory / "metadata.json").read_text())
        if not isinstance(metadata, dict):
            raise BuildError("artifact_collision", "Artifact metadata must be an object")
        binary = directory / "locust"
        verify_target(binary)
        actual = digest(binary)
        sums = (directory / "SHA256SUMS").read_text()
    except (OSError, ValueError, BuildError) as error:
        raise BuildError("artifact_collision", f"Existing artifact is incomplete or invalid: {directory}") from error
    identity_fields = ("commit", "target", "version", "version_output", "toolchain", "cargo_version", "rustc_version", "sha256")
    if any(metadata.get(key) != expected[key] for key in identity_fields) or actual != expected["sha256"] or sums != f"{actual}  locust\n":
        raise BuildError("artifact_collision", f"Existing artifact has different identity or content: {directory}")
    if metadata.get("dirty_build_inputs") is not False or metadata.get("qualification") != "not_run":
        raise BuildError("artifact_collision", f"Existing artifact has unexpected evidence metadata: {directory}")
    return metadata


def build(repo: Path, runner: Runner = run_command) -> dict:
    repo = repo.resolve()
    root = Path(checked(runner, ["git", "rev-parse", "--show-toplevel"], repo)).resolve()
    if root != repo:
        raise BuildError("wrong_repository_root", "Run against the repository root")
    commit = source_identity(repo, runner)
    toolchain = tomllib.loads((repo / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", toolchain):
        raise BuildError("unpinned_toolchain", "T1 requires the exact compiler pinned in rust-toolchain.toml")
    version = package_version(repo)
    cargo_version = checked(runner, ["cargo", f"+{toolchain}", "--version"], repo)
    rustc_version = checked(runner, ["rustc", f"+{toolchain}", "--version", "--verbose"], repo)
    if f"release: {toolchain}" not in rustc_version.splitlines():
        raise BuildError("toolchain_mismatch", "rustc does not report the pinned release")
    installed = checked(runner, ["rustup", "target", "list", "--installed", "--toolchain", toolchain], repo)
    if TARGET not in installed.splitlines():
        raise BuildError("target_unavailable", f"The pinned toolchain lacks {TARGET}; this helper does not install targets")
    target_dir = repo / "target/lane-b-t1"
    checked(runner, ["cargo", f"+{toolchain}", "build", "--locked", "--release", "--target", TARGET, "--package", "locust", "--bin", "locust", "--target-dir", str(target_dir)], repo)
    binary = target_dir / TARGET / "release/locust"
    if not binary.is_file():
        raise BuildError("binary_missing", "Cargo succeeded without producing the requested locust binary")
    if source_identity(repo, runner) != commit:
        raise BuildError("source_changed", "HEAD changed while building; no artifact was published")
    output = repo / "output/t1"
    output.mkdir(parents=True, exist_ok=True)
    destination = output / commit
    with tempfile.TemporaryDirectory(prefix=".build-", dir=output) as temporary:
        staged = Path(temporary)
        staged_binary = staged / "locust"
        shutil.copy2(binary, staged_binary)
        verify_target(staged_binary)
        version_output = checked(runner, [str(staged_binary), "--version"], repo)
        verify_version(version_output, version, commit)
        sha256 = digest(staged_binary)
        if source_identity(repo, runner) != commit:
            raise BuildError("source_changed", "HEAD changed while identifying the binary; no artifact was published")
        metadata = {
            "commit": commit, "dirty_build_inputs": False,
            "target": TARGET, "version": version, "version_output": version_output,
            "toolchain": toolchain, "cargo_version": cargo_version, "rustc_version": rustc_version,
            "sha256": sha256, "qualification": "not_run",
            "created_utc": datetime.now(timezone.utc).isoformat(),
        }
        (staged / "metadata.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
        (staged / "SHA256SUMS").write_text(f"{sha256}  locust\n")
        verify_artifact(staged, metadata)
        reused = destination.exists()
        if reused:
            metadata = verify_artifact(destination, metadata)
        else:
            # Rename a completed bundle, never individual files into an
            # existing bundle. A simultaneous publisher cannot replace a
            # nonempty destination; verify it if that race occurs.
            try:
                os.rename(staged, destination)
            except OSError:
                if not destination.exists():
                    raise
                metadata = verify_artifact(destination, metadata)
                reused = True
        verify_artifact(destination, metadata)
    return {
        "status": "reused" if reused else "built", **metadata,
        "binary": str(destination / "locust"), "metadata": str(destination / "metadata.json"),
        "sha256_file": str(destination / "SHA256SUMS"),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1], help="repository root (default: this script's checkout)")
    args = parser.parse_args(argv)
    try:
        result = build(args.repo)
    except BuildError as error:
        print(json.dumps({"status": "blocked", "state": error.state, "detail": error.detail}, sort_keys=True))
        return 1
    except (OSError, KeyError, ValueError) as error:
        print(json.dumps({"status": "failed", "state": "build_helper_error", "detail": str(error)}, sort_keys=True))
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
