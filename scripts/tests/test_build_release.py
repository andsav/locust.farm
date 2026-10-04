"""Focused checks for unsigned native release candidates."""

import io
import json
from pathlib import Path
import struct
import subprocess
import tarfile
import tempfile
import unittest

from scripts import build_release


COMMIT = "a" * 40


class ReleaseCandidateTests(unittest.TestCase):
    def fixture(self):
        binary = b"\x7fELF\x02\x01" + bytes(12) + struct.pack("<H", 62) + bytes(12)
        skill = b"# Locust collaboration\n"
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            (repo / "docs").mkdir()
            (repo / "LICENSE").write_text("License fixture\n")
            (repo / "docs/page.md").write_text("# Installed manual\n")
            (repo / "docs/site.json").write_text(json.dumps({
                "versions": {"api": 2, "protocol": 2, "blueprintSchema": 1},
                "sourceLinks": [], "pages": [{"source": "docs/page.md"}], "artifacts": []}))
            manual = build_release.make_manual(repo, COMMIT, 2, 2)
        manifest = build_release.make_manifest(
            binary=binary, skill=skill, manual=manual, commit=COMMIT, version="0.1.0",
            target="x86_64-unknown-linux-gnu", machine_format="elf-x86_64",
            toolchain="1.96.1", protocol=2, api=2)
        return binary, skill, manual, manifest

    def test_manifest_contract_and_stable_archive(self):
        binary, skill, manual, manifest = self.fixture()
        decoded = json.loads(manifest)
        self.assertEqual(manifest, build_release.canonical_json(decoded))
        self.assertEqual(decoded["files"][0]["mode"], 493)
        self.assertEqual(decoded["files"][1]["mode"], 420)
        self.assertEqual(decoded["source_commit"], COMMIT)
        self.assertEqual(decoded["machine_format"], "elf-x86_64")
        archive = build_release.candidate_archive(binary, skill, manual, manifest)
        self.assertEqual(archive, build_release.candidate_archive(binary, skill, manual, manifest))
        build_release.verify_candidate(archive, manifest, binary, skill, manual)
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
            self.assertEqual(tar.getnames(), ["locust", "skills/locust/SKILL.md", "manual.tar", "manifest.json"])
            self.assertTrue(all(member.isfile() and member.mtime == 0 for member in tar))

    def test_tampered_payload_or_manifest_is_rejected(self):
        binary, skill, manual, manifest = self.fixture()
        tampered = build_release.candidate_archive(binary + b"x", skill, manual, manifest)
        with self.assertRaisesRegex(build_release.BuildError, "differs"):
            build_release.verify_candidate(tampered, manifest, binary, skill, manual)
        decoded = json.loads(manifest)
        decoded["files"][0]["sha256"] = "0" * 64
        mismatched = build_release.canonical_json(decoded)
        archive = build_release.candidate_archive(binary, skill, manual, mismatched)
        with self.assertRaisesRegex(build_release.BuildError, "records differ"):
            build_release.verify_candidate(archive, mismatched, binary, skill, manual)

    def test_manual_is_self_contained_and_rejects_identity_drift(self):
        _, _, manual, _ = self.fixture()
        identity = build_release.verify_manual(manual, COMMIT, 2, 2)
        self.assertEqual([entry["path"] for entry in identity["files"]], ["LICENSE", "docs/page.md", "docs/site.json"])
        with self.assertRaisesRegex(build_release.BuildError, "identity"):
            build_release.verify_manual(manual, "b" * 40, 2, 2)
        with self.assertRaisesRegex(build_release.BuildError, "identity"):
            build_release.verify_manual(manual, COMMIT, 3, 2)
        with tarfile.open(fileobj=io.BytesIO(manual), mode="r:") as tar:
            self.assertEqual(tar.extractfile("LICENSE").read(), b"License fixture\n")
        license_changed = manual.replace(b"License fixture", b"License changed", 1)
        with self.assertRaisesRegex(build_release.BuildError, "records differ"):
            build_release.verify_manual(license_changed, COMMIT, 2, 2)
        changed = manual.replace(b"# Installed manual", b"# Corrupted manual", 1)
        with self.assertRaisesRegex(build_release.BuildError, "records differ"):
            build_release.verify_manual(changed, COMMIT, 2, 2)

    def test_native_target_header_and_version(self):
        binary, _, _, _ = self.fixture()
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "locust"
            path.write_bytes(binary)
            path.chmod(0o755)
            build_release.verify_machine(path, "elf-x86_64")
            with self.assertRaisesRegex(build_release.BuildError, "does not match"):
                build_release.verify_machine(path, "mach-o-arm64")
            path.write_bytes(b"\xcf\xfa\xed\xfe" + struct.pack("<I", 0x0100000C) + bytes(24))
            build_release.verify_machine(path, "mach-o-arm64")
        build_release.verify_embedded_version("locust 0.1.0 (aaaaaaaaaaaa) api 2 protocol 2", "0.1.0", COMMIT, 2, 2)
        with self.assertRaises(build_release.BuildError):
            build_release.verify_embedded_version("locust 0.1.0 (aaaaaaaaaaaa) api 3 protocol 2", "0.1.0", COMMIT, 2, 2)

    def test_only_native_supported_hosts(self):
        self.assertEqual(build_release.native_target("Darwin", "arm64"), ("aarch64-apple-darwin", "mach-o-arm64"))
        self.assertEqual(build_release.native_target("Linux", "x86_64"), ("x86_64-unknown-linux-gnu", "elf-x86_64"))
        with self.assertRaises(build_release.BuildError):
            build_release.native_target("Linux", "aarch64")

    def test_dirty_release_input_is_rejected(self):
        def runner(argv, *, cwd):
            self.assertEqual(argv[:2], ["git", "status"])
            return subprocess.CompletedProcess(argv, 0, " M skills/locust/SKILL.md\0 M README.md\0", "")

        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            (repo / "scripts").mkdir()
            (repo / "skills/locust").mkdir(parents=True)
            (repo / "scripts/build_release.py").write_text("# fixture")
            (repo / "skills/locust/SKILL.md").write_text("# fixture")
            (repo / "docs").mkdir()
            (repo / "LICENSE").write_text("License fixture\n")
            (repo / "docs/site.json").write_text(json.dumps({"sourceLinks": [], "pages": [], "artifacts": []}))
            with self.assertRaisesRegex(build_release.BuildError, "skills/locust/SKILL.md"):
                build_release.verify_release_inputs(repo, runner)


if __name__ == "__main__":
    unittest.main()
