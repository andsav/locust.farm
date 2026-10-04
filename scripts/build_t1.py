#!/usr/bin/env python3
"""Build one identified Apple Silicon artifact for the T1 machines.

This records build identity only. It does not qualify daemon behavior, deploy,
install, sign, notarize or transfer the binary. Compile a verified archive of
committed Rust/build inputs with isolated build settings; unrelated docs and
website work may remain dirty. This is not a hermetic/reproducible-build claim.
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
import tarfile
from typing import Callable

if sys.version_info >= (3, 11):
    import tomllib
else:
    tomllib = None

TARGET = "aarch64-apple-darwin"
SYSTEM_PATH = "/usr/bin:/bin:/usr/sbin:/sbin"
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


def run_command(argv: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True, check=False)
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
    return Path(path).name != ".DS_Store" and (path in BUILD_INPUTS or path.startswith(("crates/", ".cargo/")))


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
    # build.rs accepts LOCUST_BUILD_COMMIT for an exported committed tree.
    # Verify its output independently before publishing the copied artifact.
    if "\n" in output or not re.match(r"^locust\s", output):
        raise BuildError("version_contract_missing", "locust --version must print locust, its package version and Git commit; the name-only scaffold is not ready")
    versions = re.findall(r"(?<![0-9A-Za-z.])\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.+-]+)?(?![0-9A-Za-z.])", output)
    if version not in versions:
        raise BuildError("version_mismatch", f"locust --version does not identify package version {version}")
    if re.search(r"(?<![0-9A-Za-z])[0-9a-fA-F]{7,64}-dirty(?![0-9A-Za-z])", output):
        raise BuildError("version_dirty", "locust --version identifies dirty source; a published artifact must identify a clean Git commit")
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
    identity_fields = ("commit", "target", "version", "version_output", "toolchain", "cargo_version", "rustc_version", "sha256", "source_snapshot", "build_environment")
    if any(metadata.get(key) != expected[key] for key in identity_fields) or actual != expected["sha256"] or sums != f"{actual}  locust\n":
        raise BuildError("artifact_collision", f"Existing artifact has different identity or content: {directory}")
    if metadata.get("dirty_build_inputs") is not False or metadata.get("qualification") != "not_run":
        raise BuildError("artifact_collision", f"Existing artifact has unexpected evidence metadata: {directory}")
    return metadata


def require_python() -> None:
    if sys.version_info < (3, 11):
        found = ".".join(str(part) for part in sys.version_info[:3])
        raise BuildError("python_version_unsupported", f"Python 3.11 or newer is required; found {found}. Run with python3.11 or a newer interpreter.")


def base_environment(home: Path) -> dict[str, str]:
    """No inherited compiler flags, wrappers, loader hooks or Git overrides."""
    return {"PATH": SYSTEM_PATH, "HOME": str(home), "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
            "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_ATTR_NOSYSTEM": "1", "GIT_NO_REPLACE_OBJECTS": "1"}


def with_environment(runner: Runner, environment: dict[str, str]) -> Runner:
    def run(argv: list[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
        if argv[0] == "git":
            argv = ["git", "-c", "core.fsmonitor=false", "-c", f"core.attributesFile={os.devnull}", *argv[1:]]
        return runner(argv, cwd=cwd, env=environment)
    return run


def archive_source(repo: Path, commit: str, scratch: Path, runner: Runner) -> tuple[Path, dict]:
    """Export captured HEAD, then verify archive bytes against the Git tree.

    The comparison also rejects local info/attributes export-ignore/subst
    changes. Index flags and subsequent checkout writes cannot alter a blob.
    Symlinks/submodules are deliberately unsupported by this release helper.
    """
    listing = checked(runner, ["git", "ls-tree", "-r", "-z", "--full-tree", commit], repo)
    entries: dict[str, tuple[str, str]] = {}
    for record in listing.split("\0"):
        if not record:
            continue
        identity, name = record.split("\t", 1)
        mode, kind, object_id = identity.split(" ")
        if kind != "blob" or mode not in ("100644", "100755"):
            raise BuildError("unsupported_source_entry", f"T1 archives require regular files, not {mode} {name}")
        if Path(name).is_absolute() or ".." in Path(name).parts:
            raise BuildError("invalid_source_path", "Git tree contains an unsafe path")
        entries[name] = (mode, object_id)
    source = scratch / "source"
    source.mkdir()
    archive = scratch / "source.tar"
    checked(runner, ["git", "archive", "--format=tar", f"--output={archive}", commit], repo)
    seen = set()
    with tarfile.open(archive, "r:") as exported:
        for member in exported.getmembers():
            if member.isdir():
                continue
            name = member.name
            if name not in entries or name in seen or not member.isfile():
                raise BuildError("archive_mismatch", f"Git archive contains an unexpected entry: {name}")
            stream = exported.extractfile(member)
            if stream is None:
                raise BuildError("archive_mismatch", f"Cannot read archived file: {name}")
            data = stream.read()
            mode, object_id = entries[name]
            hasher = hashlib.new("sha1" if len(object_id) == 40 else "sha256", usedforsecurity=False)
            hasher.update(f"blob {len(data)}\0".encode() + data)
            if hasher.hexdigest() != object_id:
                raise BuildError("archive_mismatch", f"Archive bytes differ from the committed Git blob: {name}")
            destination = source / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
            destination.chmod(0o755 if mode == "100755" else 0o644)
            seen.add(name)
    if seen != entries.keys():
        raise BuildError("archive_mismatch", "Git archive omitted committed files: " + ", ".join(sorted(entries.keys() - seen)))
    tree = checked(runner, ["git", "rev-parse", f"{commit}^{{tree}}"], repo)
    return source, {"method": "verified_git_archive", "tree": tree, "verified_files": len(seen),
                    "checkout_bytes_used": False}


def reject_cargo_configs(source: Path, cargo_home: Path) -> None:
    # Cargo searches the invocation directory and every ancestor, not merely
    # CARGO_HOME. Reject overrides rather than pretending a clean env defeats
    # a configured rustc-wrapper/linker/rustflags/source replacement.
    directories = [source, *source.parents]
    candidates = [directory / ".cargo" / name for directory in directories for name in ("config", "config.toml")]
    candidates += [cargo_home / name for name in ("config", "config.toml")]
    found = [str(path) for path in candidates if path.exists()]
    if found:
        raise BuildError("unsupported_cargo_config", "T1's controlled build does not accept Cargo config files: " + ", ".join(found))


def executable(path: str, name: str) -> Path:
    candidate = Path(path)
    if not candidate.is_absolute() or not candidate.is_file() or not os.access(candidate, os.X_OK):
        raise BuildError("tool_unavailable", f"{name} must resolve to an installed absolute executable")
    # Preserve argv[0]: clang++ is a symlink to clang, but its invocation name
    # selects C++ driver behavior. The digest follows the link's target.
    return candidate


def build(repo: Path, runner: Runner = run_command) -> dict:
    require_python()
    repo = repo.resolve()
    discovery = base_environment(Path.home())
    git_runner = with_environment(runner, discovery)
    root = Path(checked(git_runner, ["git", "rev-parse", "--show-toplevel"], repo)).resolve()
    if root != repo:
        raise BuildError("wrong_repository_root", "Run against the repository root")
    commit = source_identity(repo, git_runner)
    # Outside the checkout: its ancestor Cargo configuration must not leak in.
    with tempfile.TemporaryDirectory(prefix="locust-t1-build-", dir="/tmp") as temporary:
        scratch = Path(temporary).resolve()
        source, snapshot = archive_source(repo, commit, scratch, git_runner)
        toolchain = tomllib.loads((source / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
        if not re.fullmatch(r"\d+\.\d+\.\d+", toolchain):
            raise BuildError("unpinned_toolchain", "T1 requires the exact compiler pinned in rust-toolchain.toml")
        version = package_version(source)
        rustup_path = shutil.which("rustup", path=str(Path.home() / ".cargo/bin") + os.pathsep + SYSTEM_PATH)
        if rustup_path is None:
            raise BuildError("tool_unavailable", "Install rustup and the pinned toolchain before building; this helper does not install tools")
        rustup = executable(rustup_path, "rustup")
        cargo = executable(checked(git_runner, [str(rustup), "which", "--toolchain", toolchain, "cargo"], source), "cargo")
        rustc = executable(checked(git_runner, [str(rustup), "which", "--toolchain", toolchain, "rustc"], source), "rustc")
        cargo_version = checked(git_runner, [str(cargo), "--version"], source)
        rustc_version = checked(git_runner, [str(rustc), "--version", "--verbose"], source)
        if f"release: {toolchain}" not in rustc_version.splitlines():
            raise BuildError("toolchain_mismatch", "rustc does not report the pinned release")
        installed = checked(git_runner, [str(rustup), "target", "list", "--installed", "--toolchain", toolchain], source)
        if TARGET not in installed.splitlines():
            raise BuildError("target_unavailable", f"The pinned toolchain lacks {TARGET}; this helper does not install targets")
        developer = checked(git_runner, ["/usr/bin/xcode-select", "--print-path"], source)
        apple_runner = with_environment(runner, {**discovery, "DEVELOPER_DIR": developer})
        sdk = checked(apple_runner, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], source)
        sdk_version = checked(apple_runner, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"], source)
        clang = executable(checked(apple_runner, ["/usr/bin/xcrun", "--find", "clang"], source), "clang")
        clangxx = executable(checked(apple_runner, ["/usr/bin/xcrun", "--find", "clang++"], source), "clang++")
        ar = executable(checked(apple_runner, ["/usr/bin/xcrun", "--find", "ar"], source), "ar")
        clang_version = checked(apple_runner, [str(clang), "--version"], source)
        for name in ("home", "cargo-home", "tmp", "target"):
            (scratch / name).mkdir()
        environment = base_environment(scratch / "home")
        environment.update({
            "PATH": str(rustc.parent) + os.pathsep + SYSTEM_PATH,
            "CARGO_HOME": str(scratch / "cargo-home"), "TMPDIR": str(scratch / "tmp"),
            "RUSTC": str(rustc), "CARGO_BUILD_JOBS": "2", "CARGO_INCREMENTAL": "0",
            "CARGO_ENCODED_RUSTFLAGS": f"--remap-path-prefix={scratch}=/locust/build",
            "CC": str(clang), "CXX": str(clangxx), "AR": str(ar),
            "CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER": str(clang),
            "DEVELOPER_DIR": developer, "SDKROOT": sdk,
            "LOCUST_BUILD_COMMIT": commit[:12],
        })
        build_runner = with_environment(runner, environment)
        reject_cargo_configs(source, scratch / "cargo-home")
        target_dir = scratch / "target"
        command = [str(cargo), "build", "--locked", "--release", "--target", TARGET,
                   "--package", "locust", "--bin", "locust", "--target-dir", str(target_dir)]
        tool_paths = (("rustup", rustup), ("cargo", cargo), ("rustc", rustc),
                      ("clang", clang), ("clang++", clangxx), ("ar", ar))
        tool_evidence = {name: {"path": str(path), "resolved_path": str(path.resolve()), "sha256": digest(path)} for name, path in tool_paths}
        checked(build_runner, command, source)
        if any(digest(path) != tool_evidence[name]["sha256"] for name, path in tool_paths):
            raise BuildError("tool_changed", "A selected compiler/build tool changed while building; no artifact was published")
        binary = target_dir / TARGET / "release/locust"
        if not binary.is_file():
            raise BuildError("binary_missing", "Cargo succeeded without producing the requested locust binary")
        if source_identity(repo, git_runner) != commit:
            raise BuildError("source_changed", "HEAD changed while building; no artifact was published")
        evidence = {
            "policy": "isolated-home-and-cargo-home; no Cargo config; pinned absolute compiler",
            "variables": {key: value.replace(str(scratch), "<build>") for key, value in environment.items()},
            "cargo_arguments": [value.replace(str(scratch), "<build>") for value in command[1:]],
            "tools": tool_evidence,
            "sdk_version": sdk_version, "clang_version": clang_version,
            "reproducibility": "not_verified", "hermetic": False,
        }
        return publish(repo, binary, commit, version, toolchain, cargo_version, rustc_version,
                       snapshot, evidence, build_runner, git_runner)


def publish(repo: Path, binary: Path, commit: str, version: str, toolchain: str,
            cargo_version: str, rustc_version: str, snapshot: dict, environment: dict,
            runner: Runner, git_runner: Runner) -> dict:
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
        if source_identity(repo, git_runner) != commit:
            raise BuildError("source_changed", "HEAD changed while identifying the binary; no artifact was published")
        metadata = {
            "commit": commit, "dirty_build_inputs": False,
            "target": TARGET, "version": version, "version_output": version_output,
            "toolchain": toolchain, "cargo_version": cargo_version, "rustc_version": rustc_version,
            "sha256": sha256, "qualification": "not_run",
            "source_snapshot": snapshot, "build_environment": environment,
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
    except (OSError, KeyError, ValueError, tarfile.TarError) as error:
        print(json.dumps({"status": "failed", "state": "build_helper_error", "detail": str(error)}, sort_keys=True))
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
