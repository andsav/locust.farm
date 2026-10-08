"""Closed-form fixtures and malformed-input checks for the frozen-output audit."""

import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from audit_nous_frozen_outputs import AGENT_IDS, audit, jsd, pearson


class AuditTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.folder = self.root / "artifacts/usefulness/heterogeneity"
        (self.folder / "data").mkdir(parents=True)
        self.questions = [
            {"market_id": "a", "resolved_outcome": "Yes"},
            {"market_id": "b", "resolved_outcome": "No"},
            {"market_id": "missing", "resolved_outcome": "Yes"},
        ]
        self.records = [
            {"market_id": qid, "group": group, "agent_idx": agent,
             "prob": probability, "parse_failed": False}
            for qid, probability in [("a", 0.75), ("b", 0.25), ("missing", 0.5)]
            for group, agents in AGENT_IDS.items()
            for agent in sorted(agents)
        ]
        self.records[-1]["parse_failed"] = True
        self.records[-1]["prob"] = None

    def run_audit(self):
        (self.folder / "results_heterogeneity.json").write_text(json.dumps(self.records))
        for name in ["results_heterogeneity_controlb.json", "results_heterogeneity_placebo.json"]:
            (self.folder / name).write_text("[]")
        (self.folder / "data/eval_set_mantic_baseline.jsonl").write_text(
            "\n".join(json.dumps(q) for q in self.questions)
        )
        # Provenance is not part of the numerical fixture.
        with patch("audit_nous_frozen_outputs.subprocess.check_output", return_value="fixture\n"):
            return audit(self.root)

    def test_closed_form_and_common_complete_selection(self):
        result = self.run_audit()
        self.assertEqual(result["complete_question_ids"], ["a", "b"])
        self.assertEqual(result["omitted_question_ids"], ["missing"])
        self.assertEqual(result["records_by_group"]["placebo"]["failed"], 1)
        for metrics in result["group_metrics_on_common_complete_set"].values():
            self.assertAlmostEqual(metrics["ensemble_brier"], 0.0625)
            self.assertAlmostEqual(metrics["pairwise_error_product"], 0.0625)
            self.assertAlmostEqual(metrics["mean_pairwise_jsd_bits"], 0)
            self.assertLess(metrics["max_identity_gap"], 1e-12)
        for contrast in result["paired_contrasts"].values():
            self.assertEqual(contrast["ensemble_brier"]["question_paired_bootstrap_95_percentile"], [0, 0])

    def test_jsd_extremes_and_correlation(self):
        self.assertAlmostEqual(jsd(0, 1), 1)
        self.assertAlmostEqual(jsd(0.5, 0.5), 0)
        self.assertAlmostEqual(pearson([0, 1, 2], [2, 1, 0]), -1)
        self.assertIsNone(pearson([1, 1], [0, 1]))

    def test_disagreeing_forecasts_have_known_decomposition(self):
        for record in self.records:
            if not record["parse_failed"]:
                record["prob"] = record["agent_idx"] % 2
        result = self.run_audit()
        for metrics in result["group_metrics_on_common_complete_set"].values():
            self.assertAlmostEqual(metrics["ensemble_brier"], 0.25)
            self.assertAlmostEqual(metrics["individual_brier"], 0.5)
            self.assertAlmostEqual(metrics["pairwise_error_product"], 2 / 9)
            self.assertAlmostEqual(metrics["mean_pairwise_jsd_bits"], 5 / 9)
            self.assertLess(metrics["max_identity_gap"], 1e-12)

    def test_duplicate_record_is_rejected(self):
        self.records.append(dict(self.records[0]))
        with self.assertRaisesRegex(ValueError, "Duplicate agent record"):
            self.run_audit()

    def test_duplicate_question_is_rejected(self):
        self.questions.append(dict(self.questions[0]))
        with self.assertRaisesRegex(ValueError, "Duplicate question"):
            self.run_audit()

    def test_wrong_agent_group_is_rejected(self):
        self.records[0]["agent_idx"] = 20
        with self.assertRaisesRegex(ValueError, "Unexpected agent"):
            self.run_audit()

    def test_invalid_probability_is_rejected(self):
        for value in [float("nan"), -0.1, 1.1, "0.5"]:
            with self.subTest(value=value):
                self.records[0]["prob"] = value
                with self.assertRaisesRegex(ValueError, "Invalid probability"):
                    self.run_audit()


if __name__ == "__main__":
    unittest.main()
