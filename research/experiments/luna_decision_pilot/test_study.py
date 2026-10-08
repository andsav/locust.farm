import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from study import ARMS, STYLES, analyze, cases, design, instructions, oracle, score
from run import Runner


class StudyTests(unittest.TestCase):
    def test_known_bayes_and_duplicate_invariance(self):
        case = dict(prior_success=.1, reward_if_success=10, loss_if_failure=5,
                    reports=[dict(underlying_observation_id="x", likelihood_if_success=.8, likelihood_if_failure=.2)])
        truth = oracle(case)
        self.assertEqual(truth["exact_probability"], "4/13")
        self.assertEqual(truth["action"], "WAIT")
        case["reports"] *= 3
        self.assertEqual(oracle(case), truth)

    def test_independent_observations_multiply(self):
        case = dict(prior_success=.5, reward_if_success=10, loss_if_failure=10,
                    reports=[dict(underlying_observation_id=x, likelihood_if_success=.8, likelihood_if_failure=.2)
                             for x in ["a", "b"]])
        self.assertEqual(oracle(case)["exact_probability"], "16/17")

    def test_conflicting_duplicate_rejected(self):
        case = cases()[0]["public_case"]
        case["reports"].append(dict(case["reports"][0], likelihood_if_success=.01))
        with self.assertRaises(ValueError):
            oracle(case)

    def test_design_and_pair_interventions(self):
        values = cases()
        self.assertEqual(len(values), 20)
        self.assertEqual(len({v["pair_id"] for v in values}), 10)
        for first, second in zip(values[::2], values[1::2]):
            self.assertEqual(first["pair_id"], second["pair_id"])
            if first["family"] == "utility":
                self.assertEqual(first["truth"]["probability"], second["truth"]["probability"])
                self.assertEqual((first["truth"]["action"], second["truth"]["action"]), ("ACT", "WAIT"))
            else:
                self.assertNotEqual(first["truth"]["probability"], second["truth"]["probability"])
        self.assertEqual(len({len(instructions(a, s)) for a in ARMS[1:] for s in STYLES}), 1)

    def test_grader_and_failure_penalty(self):
        truth = dict(probability=.75, action="ACT", act_value=50, evidence_ids=["x"])
        perfect = json.dumps(dict(probability_success=.75, action="ACT", evidence_ids_used=["x"]))
        self.assertEqual(score(perfect, truth)["squared_error"], 0)
        for bad in ['{}', '[]', 'null', 'not json', perfect.replace('0.75', 'NaN')]:
            self.assertFalse(score(bad, truth)["valid"])
            self.assertEqual(score(bad, truth)["regret"], 50)
        self.assertFalse(score(perfect, truth, "incomplete")["valid"])

    def test_perfect_records_and_ensemble(self):
        frozen = design()
        records = []
        for c in frozen["cases"]:
            t = c["truth"]
            for arm in ARMS:
                for style in STYLES:
                    records.append(dict(kind="scored", case_id=c["case_id"], arm=arm, style=style,
                        status="completed", reserved_usd=.01, cost_usd=.001,
                        text=json.dumps(dict(probability_success=t["probability"], action=t["action"], evidence_ids_used=t["evidence_ids"]))))
        summary = analyze(records, frozen)
        for values in summary["arms"].values():
            self.assertEqual(values["valid"], 60)
            self.assertEqual(values["squared_error"], 0)
            self.assertAlmostEqual(values["ensemble_squared_error_complete_cases"], 0)
            self.assertEqual(values["directional_pair_success"], 1)
        for values in summary["contrasts"].values():
            self.assertEqual(values["pair_cluster_bootstrap_95"], [0, 0])


class RunnerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.folder = Path(temporary.name)
        (self.folder / "design.json").write_text(json.dumps(design()))
        self.runner = Runner(self.folder)

    def test_reservation_saved_before_network_and_failure_not_retried(self):
        def fail(*args, **kwargs):
            saved = json.loads((self.folder / "records.json").read_text())
            self.assertEqual(saved[0]["status"], "pending")
            self.assertGreater(saved[0]["reserved_usd"], 0)
            raise TimeoutError()
        with patch.dict("os.environ", {"OPENAI_API_KEY": "test-only"}), patch("run.urllib.request.urlopen", side_effect=fail) as network:
            self.runner.invoke("test", "Test", {}, kind="smoke")
            self.runner.invoke("test", "Test", {}, kind="smoke")
            self.assertEqual(network.call_count, 1)
        saved = self.runner.records[0]
        self.assertEqual(saved["status"], "transport_error")
        self.assertNotIn("cost_usd", saved)
        self.assertNotIn("test-only", json.dumps(saved))

    def test_budget_prevents_dispatch(self):
        self.runner.records.append(dict(label="previous", reserved_usd=2))
        with patch("run.urllib.request.urlopen") as network:
            self.runner.invoke("test", "Test", {}, kind="smoke")
            network.assert_not_called()

    def test_frozen_design_mismatch_rejected(self):
        (self.folder / "design.json").write_text("{}")
        with self.assertRaises(ValueError):
            Runner(self.folder)


if __name__ == "__main__":
    unittest.main()
