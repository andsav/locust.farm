"""Public labels and receipt boundaries for real-client farm publication."""

import json
import io
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from client_qualification.live_farm import configure_farm, finish_farm, public_ended, report_session


class LiveFarmTests(unittest.TestCase):
    def setUp(self):
        self.daemon = types.SimpleNamespace(goal="goal", timeout_seconds=1)

    def test_missing_member_label_refuses_consent(self):
        calls = []
        def call(_daemon, args, **_kwargs):
            calls.append(args)
            if args[:2] == ["farm", "on"]:
                return {"farm_preview": {"status": {"farm_id": "farm"}}}
            return {"goal_status": {"members": [{"member": "one"}, {"member": "two"}]}}
        with patch("client_qualification.live_farm._call", side_effect=call):
            with self.assertRaisesRegex(RuntimeError, "Every goal member"):
                configure_farm(self.daemon, [{"principal": "one", "name": "Approved"}],
                               "https://locust.farm", "Live test")
        self.assertFalse(any(args[:2] == ["farm", "consent"] for args in calls))

    def test_lifecycle_records_observed_state_without_claiming_confinement(self):
        role = {"client": "claude-code"}
        with patch("client_qualification.live_farm._call") as call:
            report_session(self.daemon, role, "exited", "Observed process exit")
        args = call.call_args.args[1]
        self.assertEqual(args[:2], ["call", "session.report"])
        record = json.loads(args[2])["record"]
        self.assertEqual(record["harness"], "claude_code")
        self.assertEqual(record["state"], "exited")
        self.assertFalse(record["capabilities"]["confinement"])

    def test_finish_waits_for_new_receipt_not_old_success(self):
        def preview(sequence, state="ended"):
            return {"farm_preview": {"snapshot": {"goal_state": state},
                "status": {"receipt": {"sequence": sequence, "stream_version": sequence}, "pending": None}}}
        with patch("client_qualification.live_farm._call", side_effect=[
                preview(4, "running"), {}, preview(4), preview(5), preview(6)]) as call, \
                patch("client_qualification.live_farm.public_ended", side_effect=[None, {"snapshot": {"goal_state": "ended"}}]) as public, \
                patch("client_qualification.live_farm.time.sleep"):
            result = finish_farm(self.daemon)
        self.assertEqual(result["local"]["status"]["receipt"]["sequence"], 6)
        self.assertEqual(call.call_count, 5)
        self.assertEqual(public.call_count, 2)

    def test_started_observation_refreshes_consented_harness_with_exact_label(self):
        self.daemon.public_farm = {"labels": {"principal": "Codex / Luna"}}
        with patch("client_qualification.live_farm._call") as call:
            report_session(self.daemon, {"client": "codex", "principal": "principal"},
                           "started", "Observed native process")
        first, second = call.call_args_list
        self.assertEqual(first.args[1][:2], ["call", "session.report"])
        self.assertEqual(second.args[1][:2], ["farm", "consent"])
        self.assertIn("Codex / Luna", second.args[1])
        self.assertTrue(second.kwargs["owner"])

    def test_public_open_snapshot_cannot_qualify_ended_receipt(self):
        self.daemon.public_farm = {"service": "https://locust.farm", "farm_id": "farm"}
        value = {"farm_id": "farm", "status": "available", "stream_version": 5,
                 "snapshot": {"farm_id": "farm", "goal_state": "open"}}
        with patch("client_qualification.live_farm.urllib.request.urlopen",
                   return_value=io.StringIO(json.dumps(value))):
            self.assertIsNone(public_ended(self.daemon, {"stream_version": 5}))


if __name__ == "__main__":
    unittest.main()
