from pathlib import Path
import hashlib
import json
import os
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build_t1 import (
    BuildError,
    TARGET,
    build,
    dirty_inputs,
    run_command,
    verify_artifact,
    verify_version,
    reject_cargo_configs,
)


class FakeBuilder:
    def __init__(self, repo):
        self.repo = repo
        self.commands = []
        self.environments = []
        self.body = b"identified test executable"
        self.version_output = None
        self.installed = TARGET
        self.compiler = "release: 1.96.1"
        self.build_hook = None
        self.build_failure = False
        self.wrong_target = False
        self.build_source = None
        self.compiled_main = None
        self.tools = repo / "test-tools"
        self.tools.mkdir()
        for tool in ("rustup", "cargo", "rustc", "clang", "clang++", "ar"):
            path = self.tools / tool
            path.write_text(f"fixture {tool}\n")
            path.chmod(0o755)

    def __call__(self, argv, *, cwd, env=None):
        self.commands.append(argv)
        self.environments.append((argv, cwd, dict(env or {})))
        if argv[0] == "git":
            return run_command(argv, cwd=cwd, env=env)
        output = ""
        tool = Path(argv[0]).name
        if tool == "cargo" and "--version" in argv:
            output = "cargo 1.96.1 (test)\n"
        elif tool == "rustc":
            output = "rustc 1.96.1 (test)\n" + self.compiler + "\n"
        elif tool == "rustup":
            output = str(self.tools / argv[-1]) + "\n" if "which" in argv else self.installed + "\n"
        elif tool == "xcode-select":
            output = str(self.repo / "test-developer") + "\n"
        elif tool == "xcrun":
            if "--find" in argv:
                output = str(self.tools / argv[-1]) + "\n"
            elif "--show-sdk-path" in argv:
                output = str(self.repo / "test-sdk") + "\n"
            else:
                output = "26.4\n"
        elif tool == "clang" and "--version" in argv:
            output = "Apple clang fixture\n"
        elif tool == "cargo" and "build" in argv:
            self.build_source = cwd
            self.compiled_main = (cwd / "crates/locust/src/main.rs").read_bytes()
            if self.build_failure:
                return subprocess.CompletedProcess(argv, 1, "", "fixture compilation failed\n")
            binary = Path(argv[argv.index("--target-dir") + 1]) / TARGET / "release/locust"
            binary.parent.mkdir(parents=True, exist_ok=True)
            header = struct.pack("<8I", 0xFEEDFACF, 0x0100000C, 0, 2, 0, 0, 0, 0)
            binary.write_bytes((b"not arm64" if self.wrong_target else header) + self.body)
            binary.chmod(0o755)
            if self.build_hook:
                self.build_hook()
        elif tool == "locust" and argv[1:] == ["--version"]:
            commit = env["LOCUST_BUILD_COMMIT"]
            output = self.version_output if self.version_output is not None else f"locust 0.1.0 ({commit})\n"
        else:
            raise AssertionError(f"Unexpected command: {argv}")
        return subprocess.CompletedProcess(argv, 0, output, "")


@unittest.skipIf(sys.version_info < (3, 11), "build helper requires Python 3.11+")
class T1BuildTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.repo = Path(directory.name).resolve()
        self.git("init", "-q")
        self.git("config", "user.name", "T1 fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.write("Cargo.toml", '[workspace.package]\nversion = "0.1.0"\n')
        self.write("Cargo.lock", "# fixture lock\n")
        self.write("rust-toolchain.toml", '[toolchain]\nchannel = "1.96.1"\n')
        self.write("crates/locust/Cargo.toml", '[package]\nname = "locust"\nversion.workspace = true\n')
        self.write("crates/locust/src/main.rs", 'fn main() { println!("locust"); }\n')
        self.write("scripts/build_t1.py", "# fixture build helper\n")
        self.write("scripts/tests/test_build_t1.py", "# fixture tests\n")
        self.write("docs/plan.md", "# plan\n")
        self.write("sites/locust.farm/index.html", "site\n")
        self.write(".gitignore", "target/\noutput/\n")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture source")
        self.commit = self.git("rev-parse", "HEAD").strip()
        self.runner = FakeBuilder(self.repo)
        which = patch("build_t1.shutil.which", return_value=str(self.runner.tools / "rustup"))
        which.start()
        self.addCleanup(which.stop)

    def git(self, *arguments):
        return subprocess.check_output(["git", *arguments], cwd=self.repo, text=True)

    def write(self, path, content):
        destination = self.repo / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(content)

    def assert_state(self, state):
        with self.assertRaises(BuildError) as error:
            build(self.repo, self.runner)
        self.assertEqual(error.exception.state, state)
        return error.exception

    def test_identified_release_is_copied_verified_and_recorded(self):
        result = build(self.repo, self.runner)
        self.assertEqual(result["status"], "built")
        self.assertEqual(result["commit"], self.commit)
        self.assertFalse(result["dirty_build_inputs"])
        self.assertEqual(result["qualification"], "not_run")
        self.assertEqual(result["target"], TARGET)
        binary = Path(result["binary"])
        self.assertEqual(binary.parent, self.repo / "output/t1" / self.commit)
        self.assertTrue(os.access(binary, os.X_OK))
        self.assertEqual(result["sha256"], hashlib.sha256(binary.read_bytes()).hexdigest())
        metadata = json.loads(Path(result["metadata"]).read_text())
        self.assertEqual(metadata["sha256"], result["sha256"])
        self.assertEqual(Path(result["sha256_file"]).read_text(), f'{result["sha256"]}  locust\n')
        command = next(command for command in self.runner.commands if "build" in command)
        self.assertEqual(command[:3], [str(self.runner.tools / "cargo"), "build", "--locked"])
        self.assertEqual(result["source_snapshot"]["method"], "verified_git_archive")
        self.assertFalse(result["source_snapshot"]["checkout_bytes_used"])
        self.assertFalse(result["build_environment"]["hermetic"])
        self.assertEqual(result["build_environment"]["reproducibility"], "not_verified")
        self.assertEqual(result["build_environment"]["cargo_arguments"][:3], ["build", "--locked", "--release"])
        self.assertNotEqual(self.runner.build_source, self.repo)
        self.assertFalse(self.runner.build_source.exists(), "private source snapshot is removed")
        self.assertIn("--release", command)
        self.assertEqual(command[command.index("--target") + 1], TARGET)
        self.assertEqual(command[command.index("--bin") + 1], "locust")
        self.assertFalse(any("install" in command or "scp" in command for command in self.runner.commands))

    def test_unrelated_docs_and_site_changes_do_not_block(self):
        self.write("docs/plan.md", "new plan\n")
        self.write("docs/new.md", "untracked plan\n")
        self.write("sites/locust.farm/index.html", "new site\n")
        self.write("sites/locust.farm/new.html", "untracked site\n")
        self.assertEqual(build(self.repo, self.runner)["status"], "built")

    def test_dirty_staged_untracked_or_configuration_inputs_are_rejected(self):
        cases = [
            ("crates/locust/src/main.rs", False),
            ("Cargo.lock", True),
            ("rust-toolchain.toml", False),
            ("scripts/build_t1.py", False),
            ("crates/locust/build.rs", False),
            (".cargo/config.toml", False),
        ]
        for path, staged in cases:
            with self.subTest(path=path):
                original = (self.repo / path).read_bytes() if (self.repo / path).exists() else None
                self.write(path, "changed build input\n")
                if staged:
                    self.git("add", "--", path)
                error = self.assert_state("dirty_build_inputs")
                self.assertIn(path, error.detail)
                self.assertFalse(any("build" in command for command in self.runner.commands))
                if original is None:
                    (self.repo / path).unlink()
                else:
                    (self.repo / path).write_bytes(original)
                if staged:
                    self.git("add", "--", path)

    def test_ignored_untracked_build_inputs_are_not_hidden(self):
        self.write(".gitignore", "target/\noutput/\ncrates/locust/src/hidden.rs\n")
        self.write("crates/locust/src/hidden.rs", "untracked compiler input\n")
        error = self.assert_state("dirty_build_inputs")
        self.assertIn("crates/locust/src/hidden.rs", error.detail)

    def test_rename_from_build_input_to_unrelated_path_is_not_hidden(self):
        self.assertEqual(dirty_inputs("R  docs/main.rs\0crates/locust/src/main.rs\0"), ["crates/locust/src/main.rs"])
        self.assertEqual(dirty_inputs(" R crates/locust/src/renamed.rs\0docs/old.rs\0"), ["crates/locust/src/renamed.rs"])

    def test_name_only_scaffold_cannot_publish_an_artifact(self):
        self.runner.version_output = "locust\n"
        self.assert_state("version_contract_missing")
        self.assertFalse((self.repo / "output/t1" / self.commit).exists())

    def test_version_and_commit_must_both_identify_built_source(self):
        verify_version(f"locust 0.1.0 commit={self.commit[:7]}", "0.1.0", self.commit)
        verify_version(f"locust 0.1.0 ({self.commit})", "0.1.0", self.commit)
        for output, state in [
            ("locust 0.1.0", "version_contract_missing"),
            ("locust 0.1.0 (unknown) api 0 protocol 0", "version_contract_missing"),
            (f"locust 0.1.0 ({self.commit[:12]}-dirty) api 0 protocol 0", "version_dirty"),
            (f"locust 0.1.0 ({self.commit}-dirty)", "version_dirty"),
            ("locust 0.1.0 (fffffff)", "version_commit_mismatch"),
            (f"locust 0.2.0 ({self.commit})", "version_mismatch"),
            (f"locust 0.1.0 ({self.commit})\nextra output", "version_contract_missing"),
        ]:
            with self.subTest(output=output):
                self.runner.version_output = output
                self.assert_state(state)
                self.assertFalse((self.repo / "output/t1" / self.commit).exists())

    def test_target_and_pinned_compiler_are_verified(self):
        self.runner.installed = "x86_64-apple-darwin"
        self.assert_state("target_unavailable")
        self.runner.installed = TARGET
        self.runner.compiler = "release: 1.95.0"
        self.assert_state("toolchain_mismatch")
        self.runner.compiler = "release: 1.96.1"
        self.runner.wrong_target = True
        self.assert_state("wrong_binary_target")

    def test_failed_build_is_not_published(self):
        self.runner.build_failure = True
        self.assert_state("command_failed")
        self.assertFalse((self.repo / "output/t1" / self.commit).exists())

    def test_changes_during_build_are_rejected(self):
        self.runner.build_hook = lambda: self.write("crates/locust/src/main.rs", "modified while building\n")
        self.assert_state("dirty_build_inputs")
        self.assertFalse((self.repo / "output/t1" / self.commit).exists())

    def test_head_changes_during_build_are_rejected(self):
        def move_head():
            self.write("docs/plan.md", "new committed plan\n")
            self.git("add", "docs/plan.md")
            self.git("commit", "-qm", "new head")
        self.runner.build_hook = move_head
        self.assert_state("source_changed")
        self.assertFalse((self.repo / "output/t1" / self.commit).exists())

    def test_identical_artifact_is_reused_without_overwriting_metadata(self):
        first = build(self.repo, self.runner)
        before = Path(first["metadata"]).read_bytes()
        second = build(self.repo, self.runner)
        self.assertEqual(second["status"], "reused")
        self.assertEqual(Path(first["metadata"]).read_bytes(), before)
        self.assertEqual(second["created_utc"], first["created_utc"])

    def test_different_artifact_at_same_commit_is_never_overwritten(self):
        result = build(self.repo, self.runner)
        before = {path.name: path.read_bytes() for path in Path(result["binary"]).parent.iterdir()}
        self.runner.body = b"different compiled bytes"
        self.assert_state("artifact_collision")
        after = {path.name: path.read_bytes() for path in Path(result["binary"]).parent.iterdir()}
        self.assertEqual(before, after)

    def test_tampered_binary_checksum_or_metadata_is_rejected(self):
        result = build(self.repo, self.runner)
        directory = Path(result["binary"]).parent
        metadata = json.loads(Path(result["metadata"]).read_text())
        for filename in ("locust", "SHA256SUMS", "metadata.json"):
            with self.subTest(filename=filename):
                path = directory / filename
                original = path.read_bytes()
                path.write_bytes(original + b"tampered")
                with self.assertRaises(BuildError) as error:
                    verify_artifact(directory, metadata)
                self.assertEqual(error.exception.state, "artifact_collision")
                path.write_bytes(original)

    def test_non_object_metadata_is_rejected(self):
        result = build(self.repo, self.runner)
        directory = Path(result["binary"]).parent
        metadata = json.loads(Path(result["metadata"]).read_text())
        Path(result["metadata"]).write_text("[]")
        with self.assertRaises(BuildError) as error:
            verify_artifact(directory, metadata)
        self.assertEqual(error.exception.state, "artifact_collision")

    def test_hidden_index_edits_cannot_enter_the_committed_snapshot(self):
        path = "crates/locust/src/main.rs"
        original = (self.repo / path).read_bytes()
        for flag in ("skip-worktree", "assume-unchanged"):
            with self.subTest(flag=flag):
                self.git("update-index", f"--{flag}", path)
                self.write(path, "hidden source that must never compile\n")
                result = build(self.repo, self.runner)
                self.assertEqual(result["commit"], self.commit)
                self.assertEqual(self.runner.compiled_main, original)
                self.assertNotEqual(self.runner.compiled_main, (self.repo / path).read_bytes())
                self.git("update-index", f"--no-{flag}", path)
                (self.repo / path).write_bytes(original)

    def test_checkout_race_never_changes_the_snapshot_bytes(self):
        path = "crates/locust/src/main.rs"
        original = (self.repo / path).read_bytes()
        def mutate_checkout():
            self.write(path, "raced checkout edit\n")
            self.assertEqual((self.runner.build_source / path).read_bytes(), original)
        self.runner.build_hook = mutate_checkout
        self.assert_state("dirty_build_inputs")
        self.assertEqual(self.runner.compiled_main, original)

    def test_local_export_attributes_cannot_change_the_snapshot(self):
        self.write(".git/info/attributes", "Cargo.toml export-ignore\n")
        self.assert_state("archive_mismatch")
        self.assertIsNone(self.runner.build_source)

    def test_symlinks_are_rejected_before_extraction(self):
        (self.repo / "crates/locust/escape").symlink_to("/tmp")
        self.git("add", "crates/locust/escape")
        self.git("commit", "-qm", "fixture link")
        self.assert_state("unsupported_source_entry")

    def test_committed_cargo_config_cannot_install_a_wrapper(self):
        self.write(".cargo/config.toml", '[build]\nrustc-wrapper = "/not-the-compiler"\n')
        self.git("add", ".cargo/config.toml")
        self.git("commit", "-qm", "fixture config")
        self.assert_state("unsupported_cargo_config")
        self.assertIsNone(self.runner.build_source)

    def test_ancestor_cargo_configuration_is_also_refused(self):
        parent = self.repo / "private-parent"
        source = parent / "scratch/source"
        source.mkdir(parents=True)
        config = parent / ".cargo/config.toml"
        config.parent.mkdir()
        config.write_text('[build]\nrustc-wrapper = "/not-the-compiler"\n')
        with self.assertRaises(BuildError) as error:
            reject_cargo_configs(source, parent / "cargo-home")
        self.assertEqual(error.exception.state, "unsupported_cargo_config")

    def test_compiler_environment_is_controlled_and_recorded(self):
        poison = {key: "/injected/value" for key in (
            "PATH", "RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_HOME", "RUSTUP_HOME",
            "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_TARGET_DIR",
            "SDKROOT", "DEVELOPER_DIR", "CC", "CFLAGS", "LDFLAGS",
            "DYLD_INSERT_LIBRARIES", "GIT_DIR", "GIT_CONFIG_COUNT",
        )}
        with patch.dict(os.environ, poison):
            result = build(self.repo, self.runner)
        argv, cwd, environment = next(item for item in self.runner.environments if "build" in item[0])
        self.assertEqual(environment["RUSTC"], str(self.runner.tools / "rustc"))
        self.assertEqual(argv[0], str(self.runner.tools / "cargo"))
        self.assertEqual(environment["LOCUST_BUILD_COMMIT"], self.commit[:12])
        self.assertNotEqual(environment["HOME"], str(Path.home()))
        self.assertNotEqual(environment["CARGO_HOME"], str(Path.home() / ".cargo"))
        self.assertTrue(environment["CARGO_ENCODED_RUSTFLAGS"].startswith("--remap-path-prefix="))
        for key in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS", "DYLD_INSERT_LIBRARIES", "CFLAGS", "LDFLAGS"):
            self.assertNotIn(key, environment)
        for _, _, env in self.runner.environments:
            self.assertNotIn("/injected/value", env.values())
        recorded = result["build_environment"]
        self.assertEqual(recorded["variables"]["HOME"], "<build>/home")
        self.assertEqual(recorded["tools"]["rustc"]["sha256"], hashlib.sha256((self.runner.tools / "rustc").read_bytes()).hexdigest())

    def test_ds_store_metadata_does_not_block_an_archive_build(self):
        self.write("crates/locust/.DS_Store", "Finder metadata\n")
        result = build(self.repo, self.runner)
        self.assertEqual(result["status"], "built")

    def test_compiler_replaced_during_build_is_not_misreported(self):
        self.runner.build_hook = lambda: (self.runner.tools / "rustc").write_text("changed compiler bytes")
        self.assert_state("tool_changed")
        self.assertFalse((self.repo / "output/t1" / self.commit).exists())

    def test_old_python_has_an_actionable_structured_error(self):
        with patch("build_t1.sys.version_info", (3, 10, 0)):
            error = self.assert_state("python_version_unsupported")
        self.assertIn("Python 3.11", error.detail)
        self.assertEqual(self.runner.commands, [])


if __name__ == "__main__":
    unittest.main()
