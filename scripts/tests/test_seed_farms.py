from datetime import datetime, timezone
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import seed_farms


class FarmSeedTests(unittest.TestCase):
    def test_signing_matches_frozen_rust_vector(self):
        vector = json.loads((seed_farms.ROOT / "crates/locust-proto/fixtures/farm-signature-vector.json").read_text())
        request = seed_farms.sign(bytes.fromhex(vector["test_seed_hex"]), "check_in", "{}")
        self.assertEqual(request, vector["request"])
        self.assertEqual(seed_farms.request_digest(request).hex(), vector["envelope_digest"])

    def test_preparation_freezes_identity_payload_and_keeps_keys_private(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            first = seed_farms.prepare(state)
            # Content or date edits must never silently change a sequence-1 retry.
            with patch.object(seed_farms, "build_snapshot", side_effect=AssertionError("regenerated")):
                self.assertEqual(seed_farms.prepare(state), first)
            self.assertEqual(len({request["farm_id"] for _, request in first}), 2)
            self.assertEqual(state.stat().st_mode & 0o777, 0o700)
            for path in state.iterdir():
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            request_path = next(state.glob("*.request.json"))
            request = json.loads(request_path.read_text())
            request["body"] = "{}"
            request_path.write_text(json.dumps(request))
            with self.assertRaisesRegex(ValueError, "does not match"):
                seed_farms.prepare(state)

    def test_demo_histories_are_explicit_complete_and_consistent(self):
        for scenario in json.loads(seed_farms.SCENARIOS.read_text()):
            with self.subTest(scenario=scenario["slug"]):
                snapshot = seed_farms.build_snapshot(scenario, "a" * 32)
                self.assertIn("Synthetic demo", snapshot["formation"])
                self.assertEqual(snapshot["goal_state"], "ended")
                self.assertEqual([agent["name"] for agent in snapshot["agents"]], ["Codex", "Claude", "Pi", "Kimi"])
                self.assertEqual(snapshot["omitted_changes"], 0)
                self.assertLess(snapshot["observed_at_ms"], datetime.now(timezone.utc).timestamp() * 1000)
                changes = snapshot["changes"]
                self.assertIn("Synthetic demo", changes[0]["text"])
                self.assertIn("synthetic", changes[-1]["text"])
                self.assertEqual(changes[-1]["kind"], "closure")
                self.assertEqual([c["id"] for c in changes], list(range(1, len(changes) + 1)))
                self.assertEqual([c["observed_at_ms"] for c in changes], sorted(c["observed_at_ms"] for c in changes))
                self.assertTrue(all(len(c["text"]) <= 240 for c in changes))
                self.assertTrue(all(len(c["requirement"]) <= 160 for c in snapshot["candidates"]))
                for i, task in enumerate(snapshot["tasks"]):
                    candidate = next(c for c in snapshot["candidates"] if c["id"] == task["selected_candidate"])
                    self.assertTrue(task["completed"] and candidate["completed"] and candidate["selected"])
                    self.assertEqual(task["round"], candidate["round"])
                    self.assertEqual(task["id"], candidate["task"])
                    self.assertNotEqual(scenario["tasks"][i]["agent"], scenario["tasks"][i]["reviewer"])
                    stage = next(s for s in scenario["stages"] if s["id"] == task["stage"])
                    for earlier in scenario["tasks"]:
                        if earlier["stage"] in stage["prerequisites"]:
                            self.assertLess(earlier["end"], scenario["tasks"][i]["start"])
                self.assertEqual(sum(a["state"] == "failed" for a in snapshot["attempts"]), 1)
                self.assertEqual(sum(t["round"] == 2 for t in snapshot["tasks"]), 1)

    def test_publish_requires_matching_receipt_and_read_back(self):
        request = seed_farms.sign(bytes([7]) * 32, "upload", json.dumps({"snapshot": {"goal_state": "ended"}}))
        receipt = {"farm_id": request["farm_id"], "sequence": 1,
                   "request_digest": seed_farms.request_digest(request).hex()}
        view = {"status": "available", "visibility": "listed", "snapshot": {"goal_state": "ended"}}
        with patch.object(seed_farms, "http_json", side_effect=[receipt, view]):
            self.assertEqual(seed_farms.publish("http://localhost:4319", request), receipt)
        with patch.object(seed_farms, "http_json", return_value={**receipt, "sequence": 2}):
            with self.assertRaisesRegex(ValueError, "Receipt sequence"):
                seed_farms.publish("http://localhost:4319", request)
        with patch.object(seed_farms, "http_json", side_effect=[receipt, {**view, "snapshot": None}]):
            with self.assertRaisesRegex(ValueError, "Read-back"):
                seed_farms.publish("http://localhost:4319", request)


if __name__ == "__main__":
    unittest.main()
