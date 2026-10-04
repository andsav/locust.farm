import copy
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import tarfile
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_tla as tla


PASS = """TLC2 Version 2.19 of 08 August 2024 (rev: 5a47802)
Model checking completed. No error has been found.
5 states generated, 4 distinct states found, 0 states left on queue.
Finished in 00s at (2026-10-03 00:00:00)
"""
FAIL = """Error: Invariant Fence is violated.
Error: The behavior up to this point is:
State 1: <Initial predicate>
/\\ x = 0
/\\ found = FALSE
/\\ record = [generation |-> 1, values |-> <<1, 2>>]

State 2: <Write line 12, col 1 to line 14, col 2 of module Fixture>
/\\ x = 2
/\\ found = TRUE
/\\ record = [generation |-> 3,
  values |-> <<1, 2, 3>>]

The coverage statistics at 2026-10-03 00:00:00
<Write line 12, col 1 to line 14, col 2 of module Fixture>: 1:1
End of statistics.
3 states generated, 3 distinct states found, 0 states left on queue.
Finished in 00s at (2026-10-03 00:00:00)
"""
GOOD = {"expect": "pass"}
BAD = {"expect": "violation", "violation": "Fence", "trace_require": [{"found": "TRUE", "x": "2"}]}


class TlaRunnerTests(unittest.TestCase):
    def test_completed_check_requires_success_finish_and_empty_queue(self):
        self.assertTrue(tla.classify(PASS, 0, GOOD)["matched_expectation"])
        for output, code in [(PASS.split("Finished")[0], 0), (PASS, 1),
                             (PASS.replace("0 states left", "1 states left"), 0),
                             ("Model checking completed. No error has been found.", 0)]:
            with self.subTest(output=output, code=code):
                self.assertFalse(tla.classify(output, code, GOOD)["matched_expectation"])

    def test_expected_counterexample_matches_property_and_state_scenario(self):
        result = tla.classify(FAIL, 12, BAD)
        self.assertTrue(result["matched_expectation"])
        self.assertEqual(result["violation"], "Fence")
        self.assertEqual(result["trace"][-1]["variables_tla"]["record"],
                         "[generation |-> 3,\n  values |-> <<1, 2, 3>>]")
        for case in [dict(BAD, violation="Other"), dict(BAD, trace_require=[{"x": "99"}]),
                     dict(BAD, trace_require=[]), GOOD]:
            self.assertFalse(tla.classify(FAIL, 12, case)["matched_expectation"])

    def test_parser_error_deadlock_truncation_and_wrong_exit_do_not_reproduce(self):
        for output, code in [("Error: Parsing or semantic analysis failed.", 150),
                             ("Error: Deadlock reached.", 11), (FAIL, 1),
                             (FAIL.split("State 1:")[0], 12),
                             (FAIL.replace("State 2:", "State 3:"), 12),
                             (FAIL.split("Finished")[0], 12)]:
            self.assertFalse(tla.classify(output, code, BAD)["matched_expectation"])

    def test_timeout_overrides_even_a_successful_partial_log(self):
        result = tla.classify(PASS, 0, GOOD, timed_out=True)
        self.assertEqual(result["status"], "incomplete")
        self.assertFalse(result["matched_expectation"])

    def test_completed_graph_records_fingerprint_estimates_and_depth(self):
        output = PASS + """  calculated (optimistic):  val = 3.8E-6
  based on the actual fingerprints:  val = 1.6E-6
The depth of the complete state graph search is 17.
"""
        result = tla.classify(output, 0, GOOD)
        self.assertEqual(result["depth"], 17)
        self.assertEqual(result["fingerprint_collision_estimates"]["based on the actual fingerprints"], "1.6E-6")

    def test_deadlock_is_an_unexpected_model_violation(self):
        result = tla.classify("Error: Deadlock reached.", 11, BAD)
        self.assertEqual(result["status"], "unexpected_violation")
        self.assertFalse(result["matched_expectation"])

    def test_scenario_must_match_final_violation_state(self):
        output = FAIL.replace("The coverage statistics", "State 3: <OtherBug>\n/\\ x = 99\n/\\ found = FALSE\n\nThe coverage statistics")
        self.assertFalse(tla.classify(output, 12, BAD)["matched_expectation"])
        case = dict(BAD, trace_prefix=[{"x": "0"}])
        self.assertTrue(tla.classify(FAIL, 12, case)["matched_expectation"])
        case["trace_prefix"] = [{"x": "99"}]
        self.assertFalse(tla.classify(FAIL, 12, case)["matched_expectation"])

    def test_single_variable_trace_is_supported(self):
        text = FAIL.replace("/\\ x", "x").replace("/\\ found = FALSE\n", "").replace("/\\ found = TRUE\n", "")
        self.assertEqual(tla.trace_states(text)[-1]["variables_tla"]["x"], "2")

    def test_bad_checksum_refuses_cache_without_network(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tool.jar"
            path.write_bytes(b"wrong bytes")
            with patch("urllib.request.urlopen") as network:
                with self.assertRaises(tla.CheckError):
                    tla.download("https://example.invalid/tool", path, "0" * 64)
                network.assert_not_called()

    def test_download_verifies_before_promotion_and_removes_failed_temporary(self):
        from io import BytesIO
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tool.jar"
            with patch("urllib.request.urlopen", return_value=BytesIO(b"wrong")):
                with self.assertRaises(tla.CheckError):
                    tla.download("https://example.invalid/tool", path, "0" * 64)
            self.assertFalse(path.exists())
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_java_option_injection_is_removed_without_changing_environment(self):
        with patch.dict("os.environ", {"JAVA_TOOL_OPTIONS": "-javaagent:bad", "CLASSPATH": "bad"}):
            clean = tla.clean_java_env()
            self.assertNotIn("JAVA_TOOL_OPTIONS", clean)
            self.assertNotIn("CLASSPATH", clean)

    def test_extracted_runtime_changes_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "jdk.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                for name, data in [("jdk/bin/java", b"java binary"), ("jdk/lib/modules", b"modules")]:
                    member = tarfile.TarInfo(name)
                    member.size = len(data)
                    output.addfile(member, io.BytesIO(data))
            tla.extract_jdk(archive, root)
            tla.verify_runtime(archive, root)
            for name in ("jdk/bin/java", "jdk/lib/modules"):
                path = root / name
                original = path.read_bytes()
                path.write_bytes(b"changed")
                with self.assertRaises(tla.CheckError):
                    tla.verify_runtime(archive, root)
                path.write_bytes(original)

    def test_registry_rejects_mismatched_properties_unknown_outcomes_and_bad_ids(self):
        import json
        registry = json.loads(tla.CASES.read_text())
        tla.validate_cases(registry)
        for update in [{"properties": ["Missing"]}, {"expect": "unknown"},
                       {"id": "../escape"}, {"module": "../../AGENTS.md"}]:
            changed = copy.deepcopy(registry)
            changed["cases"][0].update(update)
            with self.assertRaises(tla.CheckError):
                tla.validate_cases(changed)
        duplicate = copy.deepcopy(registry)
        duplicate["cases"].append(duplicate["cases"][0])
        with self.assertRaises(tla.CheckError):
            tla.validate_cases(duplicate)
        empty = copy.deepcopy(registry)
        empty["cases"][1]["trace_require"] = [{}]
        with self.assertRaises(tla.CheckError):
            tla.validate_cases(empty)

    def test_validation_uses_frozen_config(self):
        import json
        registry = json.loads(tla.CASES.read_text())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tla.snapshot_inputs(root)
            (root / "fixtures/pass.cfg").write_text("INIT Init\nNEXT Next\nINVARIANT TypeOK\n")
            with self.assertRaises(tla.CheckError):
                tla.validate_cases(registry, root)

    def test_process_timeout_kills_and_records_incomplete_run(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            java = root / "fake-java"
            java.write_text(f"#!{sys.executable}\nimport time\nprint('partial output', flush=True)\ntime.sleep(20)\n")
            java.chmod(0o755)
            case = {"id": "timeout", "module": "fixtures/RunnerFixture.tla",
                    "config": "fixtures/pass.cfg", "expect": "pass"}
            start = time.monotonic()
            result = tla.run_case(case, java, root / "unused.jar", root, timeout=0.1)
            self.assertLess(time.monotonic() - start, 3)
            self.assertEqual(result["status"], "incomplete")
            self.assertEqual(result["reason"], "timeout")
            self.assertFalse(result["matched_expectation"])
            self.assertTrue((root / "timeout/tlc.log").exists())
            self.assertNotIn("-coverage", result["command"])
            self.assertFalse(result["coverage_instrumentation"])

    def test_model_snapshot_records_the_bytes_it_executes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            hashes = tla.snapshot_inputs(root)
            fixture = root / "fixtures/RunnerFixture.tla"
            self.assertEqual(hashes["research/tla/fixtures/RunnerFixture.tla"], tla.sha256(fixture))

    def test_protocol_model_identity_stays_separate_from_current_checkout(self):
        registry = {"model_baseline": {"status": "historical", "source_commit": "a" * 40,
                                       "protocol_version": 0, "api_version": 0}}
        result = tla.model_scope(registry, [{"kind": "current-safety"}])
        self.assertEqual(result["modeled_baseline"]["protocol_version"], 0)
        self.assertFalse(result["runtime_conformance_claimed"])
        with self.assertRaises(tla.CheckError):
            tla.model_scope({}, [{"kind": "current-safety"}])

    def test_runner_fixtures_do_not_claim_a_protocol_model_baseline(self):
        result = tla.model_scope({}, [{"kind": "runner-fixture"}])
        self.assertEqual(result["kind"], "runner-fixtures")
        self.assertNotIn("modeled_baseline", result)

    def test_mixed_versions_preserve_both_baselines(self):
        old = {"source_commit": "a" * 40, "protocol_version": 0}
        new = {"source_commit": "b" * 40, "protocol_version": 1}
        cases = [{"kind": "runner-fixture"}, {"kind": "current-safety"},
                 {"kind": "current-safety", "model_baseline": new}]
        result = tla.model_scope({"model_baseline": old}, cases)
        self.assertEqual(result["modeled_baselines"], [old, new])
        self.assertNotIn("modeled_baseline", result)
        self.assertFalse(result["runtime_conformance_claimed"])
        result = tla.model_scope({"model_baseline": old}, cases[2:])
        self.assertEqual(result["modeled_baseline"], new)

    def test_invalid_case_baseline_cannot_fall_back_to_historical(self):
        with self.assertRaises(tla.CheckError):
            tla.model_scope({"model_baseline": {"source_commit": "a" * 40}},
                            [{"kind": "current-safety", "model_baseline": {}}])


if __name__ == "__main__":
    unittest.main()
