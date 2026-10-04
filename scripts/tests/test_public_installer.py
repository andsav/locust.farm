import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "install.sh"


class PublicInstallerTests(unittest.TestCase):
    def run_script(self, *arguments):
        return subprocess.run(
            ["/bin/sh", str(SCRIPT), *arguments], capture_output=True, text=True
        )

    def test_help_and_shell_syntax_need_no_installation(self):
        syntax = subprocess.run(["/bin/sh", "-n", str(SCRIPT)], capture_output=True)
        self.assertEqual(syntax.returncode, 0)
        result = self.run_script("--help")
        self.assertEqual(result.returncode, 0)
        self.assertIn("Does not configure clients or start a daemon", result.stdout)

    def test_unknown_options_and_missing_destinations_fail(self):
        for arguments in (("--unknown",), ("--prefix",), ("--bin-dir",)):
            with self.subTest(arguments=arguments):
                result = self.run_script(*arguments)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("Locust install:", result.stderr)

    def test_relative_and_traversal_destinations_fail_before_installation(self):
        for destination in ("relative", "/tmp/locust/../other", "/tmp/locust/./other"):
            for option in ("--prefix", "--bin-dir"):
                with self.subTest(destination=destination, option=option):
                    result = self.run_script(option, destination)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("installation paths", result.stderr)

    @unittest.skipUnless(sys.platform == "darwin", "macOS installer preflight")
    def test_existing_foreign_cli_is_preserved_before_any_activation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary_directory = root / "bin"
            binary_directory.mkdir()
            foreign = binary_directory / "locust"
            foreign.write_bytes(b"existing unrelated executable")
            prefix = root / "software"
            result = self.run_script("--prefix", str(prefix), "--bin-dir", str(binary_directory))
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("preserving existing", result.stderr)
            self.assertEqual(foreign.read_bytes(), b"existing unrelated executable")
            self.assertFalse(prefix.exists())

    @unittest.skipUnless(sys.platform == "darwin", "macOS installer preflight")
    def test_symlinked_bin_directory_is_preserved(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            other = root / "other"
            other.mkdir()
            binary_directory = root / "bin"
            binary_directory.symlink_to(other, target_is_directory=True)
            prefix = root / "software"
            result = self.run_script("--prefix", str(prefix), "--bin-dir", str(binary_directory))
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("bin directory is a symlink", result.stderr)
            self.assertEqual(os.readlink(binary_directory), str(other))
            self.assertEqual(list(other.iterdir()), [])
            self.assertFalse(prefix.exists())


if __name__ == "__main__":
    unittest.main()
