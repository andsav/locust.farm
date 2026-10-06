"""Owner confirmation argv must stay bound to the displayed plan."""

import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from owner_plans import confirmation_arguments


class OwnerPlanTests(unittest.TestCase):
    def test_goal_confirmation_repeats_exact_selection(self):
        original = ["--agent", "alice", "goal", "create", "--title", "demo"]
        planned = {"action": "review_required", "plan_id": "plan-0123456789abcdef"}
        self.assertEqual(confirmation_arguments(original, planned),
                         [*original, "--confirm", "plan-0123456789abcdef"])
        self.assertEqual(original, ["--agent", "alice", "goal", "create", "--title", "demo"])

    def test_onboarding_repeats_printed_name(self):
        planned = {"action": "review_required", "plan_id": "plan-0123456789abcdef",
                   "plan": {"clients": [{"plan": {"spec": {"name": "codex-maple"}}}]}}
        self.assertEqual(confirmation_arguments(["up", "--client", "codex", "--plan"], planned),
                         ["up", "--client", "codex", "--name", "codex-maple",
                          "--confirm", "plan-0123456789abcdef"])

    def test_immediate_result_needs_no_second_run(self):
        self.assertIsNone(confirmation_arguments(["invitation", "revoke"], {"changed": True}))
        self.assertIsNone(confirmation_arguments(["agent", "enroll"], "done"))
        self.assertIsNone(confirmation_arguments(["agent", "enroll"], None))

    def test_invalid_plan_is_not_confirmed(self):
        with self.assertRaisesRegex(ValueError, "invalid plan id"):
            confirmation_arguments(["goal", "invite"],
                                   {"action": "review_required", "plan_id": "wrong"})


if __name__ == "__main__":
    unittest.main()
