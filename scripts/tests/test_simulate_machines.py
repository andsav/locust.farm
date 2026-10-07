"""The multi-process scenario runner's command line, without starting daemons."""

import contextlib
import io
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "simulate_machines"))
import run  # noqa: E402


class CommandLine(unittest.TestCase):
    def test_list_names_every_scenario_and_needs_no_binary(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(run.main(["--list"]), 0)
        names = out.getvalue().split()
        self.assertEqual(names[:3], ["two-machines-complete", "three-machines", "sleep"])
        self.assertEqual(sorted(names), sorted(run.SCENARIOS))

    def test_the_quick_set_is_made_of_known_scenarios(self):
        self.assertTrue(set(run.QUICK) <= set(run.SCENARIOS))

    def test_running_without_a_binary_is_a_usage_error(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as stop:
            run.main(["--quick"])
        self.assertEqual(stop.exception.code, 2)

    def test_only_the_relay_free_local_profile_needs_multicast(self):
        profiles = run.simlib.PROFILES
        self.assertTrue(run.simlib.multicast_only(profiles["lan"]))
        self.assertFalse(run.simlib.multicast_only(profiles["isolated"]))
        self.assertFalse(run.simlib.multicast_only(profiles["defaults"]))

    def test_mixed_build_without_a_second_binary_is_a_usage_error(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as stop:
            run.main(["--binary", "/nonexistent/locust", "mixed-build"])
        self.assertEqual(stop.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
