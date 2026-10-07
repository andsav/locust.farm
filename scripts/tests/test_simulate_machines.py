"""The multi-process scenario runner's command line, without starting daemons."""

import contextlib
import io
import itertools
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

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

    def test_a_home_is_fresh_with_its_marks_and_both_are_removed(self):
        simlib = run.simlib
        with tempfile.TemporaryDirectory(dir="/tmp") as directory, \
                patch.object(simlib, "HOME_PREFIX", f"{directory}/sim-"), \
                patch.object(simlib, "_homes", itertools.count(1)), \
                patch.dict(simlib.BINARIES, {"candidate": Path(sys.executable)}), \
                patch.object(simlib.base, "binary_facts", return_value={}):
            # An earlier daemon's marks beside an unused home number.
            Path(f"{directory}/sim-1.marks").mkdir()
            cluster = simlib.Cluster("fixture", Path(directory) / "artifacts", profile="isolated")

            def body(c):
                machine = c.add()
                self.assertEqual(machine.home, Path(f"{directory}/sim-2"))
                self.assertFalse(Path(f"{directory}/sim-1").exists())
                Path(f"{machine.home}.marks").mkdir()

            self.assertTrue(cluster.run_scenario(body))
            self.assertFalse(Path(f"{directory}/sim-2").exists())
            self.assertFalse(Path(f"{directory}/sim-2.marks").exists())

    def test_a_member_signs_only_once_the_restore_guard_holds_nothing(self):
        import flows

        class Cluster:
            def __init__(self, state):
                self.state = state

            def goal_status(self, machine, goal):
                return self.state

        admitted = {"key": "k", "by_host": False, "reason": "admitted", "heard": [], "waiting": []}
        self.assertFalse(flows.signs(Cluster({"guard": [admitted]}), None, "g"))
        self.assertFalse(flows.signs(Cluster(None), None, "g"))
        self.assertFalse(flows.signs(Cluster({}), None, "g"))
        self.assertTrue(flows.signs(Cluster({"guard": []}), None, "g"))

    def test_mixed_build_without_a_second_binary_is_a_usage_error(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as stop:
            run.main(["--binary", "/nonexistent/locust", "mixed-build"])
        self.assertEqual(stop.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
