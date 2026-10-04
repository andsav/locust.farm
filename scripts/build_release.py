#!/usr/bin/env python3
"""Build an identified, unsigned native release candidate from committed source.

This prepares bytes for offline review and signing. It does not sign, install,
publish, or qualify the candidate. The archive is stable for identical input
files; byte-for-byte reproducibility of independently compiled binaries is not
claimed.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts import build_t1

BuildError = build_t1.BuildError
TARGETS = {
    ("Darwin", "arm64"): ("aarch64-apple-darwin", "mach-o-arm64"),
    ("Darwin", "aarch64"): ("aarch64-apple-darwin", "mach-o-arm64"),
    ("Linux", "x86_64"): ("x86_64-unknown-linux-gnu", "elf-x86_64"),
}
RELEASE_INPUTS = {"scripts/build_release.py", "skills/locust/SKILL.md"}
SKILL_PATH = "skills/locust/SKILL.md"
MANIFEST_PATH = "manifest.json"
MANUAL_PATH = "manual.tar"


def canonical_json(value: dict) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode("utf-8")


def native_target(system: str | None = None, machine: str | None = None) -> tuple[str, str]:
    key = (system or platform.system(), machine or platform.machine())
    try:
        return TARGETS[key]
    except KeyError as error:
        raise BuildError("unsupported_host", f"No native release target for {key[0]} {key[1]}") from error


def verify_machine(binary: Path, machine_format: str) -> None:
    with binary.open("rb") as stream:
        header = stream.read(32)
    if machine_format == "mach-o-arm64":
        valid = (len(header) >= 8 and header[:4] == b"\xcf\xfa\xed\xfe"
                 and struct.unpack_from("<I", header, 4)[0] == 0x0100000C)
    elif machine_format == "elf-x86_64":
        valid = (len(header) >= 20 and header[:6] == b"\x7fELF\x02\x01"
                 and struct.unpack_from("<H", header, 18)[0] == 62)
    else:
        raise BuildError("unsupported_machine_format", f"Unknown machine format: {machine_format}")
    if not valid:
        raise BuildError("wrong_binary_target", f"Binary does not match {machine_format}")
    if not os.access(binary, os.X_OK):
        raise BuildError("binary_not_executable", "The built locust binary lacks executable permissions")


def protocol_versions(source: Path) -> tuple[int, int]:
    code = (source / "crates/locust-proto/src/lib.rs").read_text()
    versions = []
    for name, integer_type in (("PROTOCOL_VERSION", "u8"), ("API_VERSION", "u16")):
        matches = re.findall(rf"pub const {name}: {integer_type} = (\d+);", code)
        if len(matches) != 1:
            raise BuildError("version_contract_missing", f"Cannot identify one {name} in archived source")
        versions.append(int(matches[0]))
    return versions[0], versions[1]


def verify_embedded_version(output: str, version: str, commit: str, protocol: int, api: int) -> None:
    build_t1.verify_version(output, version, commit)
    expected = f"locust {version} ({commit[:12]}) api {api} protocol {protocol}"
    if output != expected:
        raise BuildError("version_mismatch", f"Expected exact archived-source identity: {expected}; got: {output}")


def verify_release_inputs(repo: Path, runner: build_t1.Runner) -> None:
    result = runner(["git", "status", "--porcelain=v1", "-z", "--untracked-files=all"], cwd=repo)
    if result.returncode:
        raise BuildError("git_status_failed", "Cannot inspect release inputs")
    inputs = RELEASE_INPUTS | set(manual_paths(repo))
    records = iter(result.stdout.split("\0"))
    dirty = []
    for record in records:
        if not record:
            continue
        if len(record) < 4 or record[2] != " ":
            raise BuildError("invalid_git_status", "Git returned malformed porcelain status")
        paths = [record[3:]]
        if "R" in record[:2] or "C" in record[:2]:
            paths.append(next(records, ""))
        dirty.extend(path for path in paths if path in inputs)
    missing = [path for path in inputs if not (repo / path).is_file()]
    if dirty or missing:
        raise BuildError("dirty_release_inputs", "Commit release helper, skill and manual sources first: " + ", ".join(sorted(set(dirty + missing))))


def file_record(path: str, content: bytes, mode: int) -> dict:
    return {"path": path, "sha256": hashlib.sha256(content).hexdigest(), "size": len(content), "mode": mode}


def manual_paths(repo: Path) -> list[str]:
    site = json.loads((repo / "docs/site.json").read_bytes())
    paths = {"docs/site.json", *site["sourceLinks"]}
    paths.update(item["source"] for item in site["pages"] + site["artifacts"])
    for name in paths:
        path = Path(name)
        if path.is_absolute() or ".." in path.parts or name != path.as_posix():
            raise BuildError("invalid_manual_path", f"Unsafe manual source: {name}")
        if (not (repo / path).is_file() or (repo / path).is_symlink()
                or not (repo / path).resolve().is_relative_to(repo.resolve())):
            raise BuildError("invalid_manual_path", f"Missing or linked manual source: {name}")
    return sorted(paths)


def make_manual(repo: Path, commit: str, protocol: int, api: int) -> bytes:
    paths = manual_paths(repo)
    site = json.loads((repo / "docs/site.json").read_bytes())
    if site["versions"]["api"] != api or site["versions"]["protocol"] != protocol:
        raise BuildError("manual_version_mismatch", "Manual and binary API/protocol versions differ")
    contents = {name: (repo / name).read_bytes() for name in paths}
    identity = canonical_json({"format": "locust-manual-v1", "source_commit": commit,
                               "versions": site["versions"],
                               "files": [file_record(name, data, 0o644) for name, data in contents.items()]})
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for name, data in [("manual.json", identity), *contents.items()]:
            entry = tarfile.TarInfo(name)
            entry.size, entry.mode = len(data), 0o644
            entry.uid = entry.gid = entry.mtime = 0
            entry.uname = entry.gname = ""
            archive.addfile(entry, io.BytesIO(data))
    return buffer.getvalue()


def verify_manual(data: bytes, commit: str, protocol: int, api: int) -> dict:
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as archive:
        members = archive.getmembers()
        if not members or members[0].name != "manual.json":
            raise BuildError("manual_mismatch", "Manual identity is missing")
        contents = {}
        for member in members:
            path = Path(member.name)
            if (not member.isfile() or path.is_absolute() or ".." in path.parts
                    or member.name in contents or member.mode != 0o644
                    or member.uid or member.gid or member.mtime or member.uname or member.gname):
                raise BuildError("manual_mismatch", "Unsafe or duplicate manual member")
            contents[member.name] = archive.extractfile(member).read()
    identity_bytes = contents.pop("manual.json")
    identity = json.loads(identity_bytes)
    if canonical_json(identity) != identity_bytes:
        raise BuildError("manual_mismatch", "Manual identity is not canonical")
    if (identity["format"] != "locust-manual-v1" or identity["source_commit"] != commit
            or identity["versions"]["api"] != api or identity["versions"]["protocol"] != protocol
            or identity["files"] != [file_record(name, value, 0o644) for name, value in contents.items()]):
        raise BuildError("manual_mismatch", "Manual identity or file records differ")
    site = json.loads(contents["docs/site.json"])
    required = {"docs/site.json", *site["sourceLinks"]}
    required.update(item["source"] for item in site["pages"] + site["artifacts"])
    if set(contents) != required or identity["versions"] != site["versions"]:
        raise BuildError("manual_mismatch", "Manual differs from its documentation manifest")
    return identity


def make_manifest(*, binary: bytes, skill: bytes, manual: bytes, commit: str, version: str, target: str,
                  machine_format: str, toolchain: str, protocol: int, api: int) -> bytes:
    manifest = {
        "format": "locust-release-v2", "source_commit": commit, "version": version,
        "target": target, "machine_format": machine_format, "toolchain": toolchain,
        "protocol_version": protocol, "api_version": api,
        "files": [file_record("locust", binary, 0o755), file_record(SKILL_PATH, skill, 0o644), file_record(MANUAL_PATH, manual, 0o644)],
    }
    return canonical_json(manifest)


def candidate_archive(binary: bytes, skill: bytes, manual: bytes, manifest: bytes) -> bytes:
    """Create a flat tar.gz with fixed metadata; no path extraction is needed."""
    buffer = io.BytesIO()
    with gzip.GzipFile(fileobj=buffer, mode="wb", filename="", mtime=0, compresslevel=9) as zipped:
        with tarfile.open(fileobj=zipped, mode="w", format=tarfile.USTAR_FORMAT) as archive:
            for name, content, mode in (("locust", binary, 0o755), (SKILL_PATH, skill, 0o644),
                                        (MANUAL_PATH, manual, 0o644), (MANIFEST_PATH, manifest, 0o644)):
                entry = tarfile.TarInfo(name)
                entry.size, entry.mode = len(content), mode
                entry.uid = entry.gid = entry.mtime = 0
                entry.uname = entry.gname = ""
                archive.addfile(entry, io.BytesIO(content))
    return buffer.getvalue()


def verify_candidate(data: bytes, manifest: bytes, binary: bytes, skill: bytes, manual: bytes) -> None:
    expected = {"locust": (binary, 0o755), SKILL_PATH: (skill, 0o644), MANUAL_PATH: (manual, 0o644), MANIFEST_PATH: (manifest, 0o644)}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        members = archive.getmembers()
        if [member.name for member in members] != list(expected):
            raise BuildError("candidate_mismatch", "Candidate archive has unexpected paths or ordering")
        for member in members:
            content, mode = expected[member.name]
            stream = archive.extractfile(member)
            if (not member.isfile() or member.mode != mode or member.size != len(content)
                    or member.uid or member.gid or member.mtime or stream is None
                    or stream.read() != content):
                raise BuildError("candidate_mismatch", f"Candidate archive entry differs: {member.name}")
    decoded = json.loads(manifest)
    verify_manual(manual, decoded["source_commit"], decoded["protocol_version"], decoded["api_version"])
    if canonical_json(decoded) != manifest:
        raise BuildError("candidate_mismatch", "Manifest bytes are not canonical")
    records = [file_record(name, content, mode) for name, (content, mode) in expected.items()
               if name != MANIFEST_PATH]
    if decoded["files"] != records:
        raise BuildError("candidate_mismatch", "Manifest file records differ from candidate content")


def build(repo: Path, runner: build_t1.Runner = build_t1.run_command) -> dict:
    build_t1.require_python()
    repo = repo.resolve()
    discovery = build_t1.base_environment(Path.home())
    git_runner = build_t1.with_environment(runner, discovery)
    root = Path(build_t1.checked(git_runner, ["git", "rev-parse", "--show-toplevel"], repo)).resolve()
    if root != repo:
        raise BuildError("wrong_repository_root", "Run against the repository root")
    target, machine_format = native_target()
    commit = build_t1.source_identity(repo, git_runner)
    verify_release_inputs(repo, git_runner)
    with tempfile.TemporaryDirectory(prefix="locust-release-build-", dir="/tmp") as temporary:
        scratch = Path(temporary).resolve()
        source, snapshot = build_t1.archive_source(repo, commit, scratch, git_runner)
        if Path(__file__).read_bytes() != (source / "scripts/build_release.py").read_bytes():
            raise BuildError("helper_mismatch", "Running helper differs from the committed source archive")
        toolchain = build_t1.tomllib.loads((source / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
        if not re.fullmatch(r"\d+\.\d+\.\d+", toolchain):
            raise BuildError("unpinned_toolchain", "Release builds require an exact pinned compiler")
        version = build_t1.package_version(source)
        protocol, api = protocol_versions(source)
        skill = (source / SKILL_PATH).read_bytes()
        manual = make_manual(source, commit, protocol, api)
        rustup_path = shutil.which("rustup", path=str(Path.home() / ".cargo/bin") + os.pathsep + build_t1.SYSTEM_PATH)
        if rustup_path is None:
            raise BuildError("tool_unavailable", "Install rustup and the pinned toolchain before building")
        rustup = build_t1.executable(rustup_path, "rustup")
        cargo = build_t1.executable(build_t1.checked(git_runner, [str(rustup), "which", "--toolchain", toolchain, "cargo"], source), "cargo")
        rustc = build_t1.executable(build_t1.checked(git_runner, [str(rustup), "which", "--toolchain", toolchain, "rustc"], source), "rustc")
        rustc_version = build_t1.checked(git_runner, [str(rustc), "--version", "--verbose"], source)
        if f"release: {toolchain}" not in rustc_version.splitlines() or f"host: {target}" not in rustc_version.splitlines():
            raise BuildError("toolchain_mismatch", "Pinned rustc does not report the required release and native target")
        installed = build_t1.checked(git_runner, [str(rustup), "target", "list", "--installed", "--toolchain", toolchain], source)
        if target not in installed.splitlines():
            raise BuildError("target_unavailable", f"Pinned toolchain lacks native target {target}")
        for name in ("home", "cargo-home", "tmp", "target"):
            (scratch / name).mkdir()
        environment = build_t1.base_environment(scratch / "home")
        environment.update({
            "PATH": str(rustc.parent) + os.pathsep + build_t1.SYSTEM_PATH,
            "CARGO_HOME": str(scratch / "cargo-home"), "TMPDIR": str(scratch / "tmp"),
            "RUSTC": str(rustc), "CARGO_BUILD_JOBS": "2", "CARGO_INCREMENTAL": "0",
            "CARGO_ENCODED_RUSTFLAGS": f"--remap-path-prefix={scratch}=/locust/build",
            "LOCUST_BUILD_COMMIT": commit[:12],
        })
        if target == "aarch64-apple-darwin":
            developer = build_t1.checked(git_runner, ["/usr/bin/xcode-select", "--print-path"], source)
            apple_runner = build_t1.with_environment(runner, {**discovery, "DEVELOPER_DIR": developer})
            sdk = build_t1.checked(apple_runner, ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], source)
            environment.update({
                "DEVELOPER_DIR": developer, "SDKROOT": sdk,
                "CC": build_t1.checked(apple_runner, ["/usr/bin/xcrun", "--find", "clang"], source),
                "CXX": build_t1.checked(apple_runner, ["/usr/bin/xcrun", "--find", "clang++"], source),
                "AR": build_t1.checked(apple_runner, ["/usr/bin/xcrun", "--find", "ar"], source),
            })
            environment["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = environment["CC"]
        build_t1.reject_cargo_configs(source, scratch / "cargo-home")
        build_runner = build_t1.with_environment(runner, environment)
        tool_paths = {"rustup": rustup, "cargo": cargo, "rustc": rustc}
        for name in ("CC", "CXX", "AR"):
            if name in environment:
                tool_paths[name] = build_t1.executable(environment[name], name)
        tool_hashes = {name: build_t1.digest(path) for name, path in tool_paths.items()}
        command = [str(cargo), "build", "--locked", "--release", "--target", target,
                   "--package", "locust", "--bin", "locust", "--target-dir", str(scratch / "target")]
        build_t1.checked(build_runner, command, source)
        if any(build_t1.digest(path) != tool_hashes[name] for name, path in tool_paths.items()):
            raise BuildError("tool_changed", "A selected build tool changed while compiling")
        binary_path = scratch / "target" / target / "release/locust"
        if not binary_path.is_file():
            raise BuildError("binary_missing", "Cargo succeeded without a locust binary")
        verify_machine(binary_path, machine_format)
        embedded = build_t1.checked(build_runner, [str(binary_path), "--version"], source)
        verify_embedded_version(embedded, version, commit, protocol, api)
        if build_t1.source_identity(repo, git_runner) != commit:
            raise BuildError("source_changed", "Source changed during build")
        verify_release_inputs(repo, git_runner)
        binary = binary_path.read_bytes()
        manifest = make_manifest(binary=binary, skill=skill, manual=manual, commit=commit, version=version,
                                 target=target, machine_format=machine_format, toolchain=toolchain,
                                 protocol=protocol, api=api)
        candidate = candidate_archive(binary, skill, manual, manifest)
        verify_candidate(candidate, manifest, binary, skill, manual)
        output = repo / "output/release"
        output.mkdir(parents=True, exist_ok=True)
        name = f"locust-{target}-{commit[:12]}-unsigned.tar.gz"
        destination = output / name
        with tempfile.NamedTemporaryFile(dir=output, prefix=".candidate-", delete=False) as temporary_file:
            staged = Path(temporary_file.name)
            temporary_file.write(candidate)
        try:
            try:
                os.link(staged, destination)
            except FileExistsError:
                if destination.read_bytes() != candidate:
                    raise BuildError("artifact_collision", f"Existing candidate differs: {destination}")
        finally:
            staged.unlink(missing_ok=True)
        archive_hash = hashlib.sha256(candidate).hexdigest()
        sidecar = output / f"{name}.sha256"
        line = f"{archive_hash}  {name}\n"
        if sidecar.exists() and sidecar.read_text() != line:
            raise BuildError("artifact_collision", f"Existing checksum differs: {sidecar}")
        sidecar.write_text(line)
        return {"status": "unsigned_candidate", "archive": str(destination), "sha256": archive_hash,
                "manifest_sha256": hashlib.sha256(manifest).hexdigest(), "source_commit": commit,
                "target": target, "version": version, "protocol_version": protocol,
                "api_version": api, "source_snapshot": snapshot, "signing": "not_run"}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
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
