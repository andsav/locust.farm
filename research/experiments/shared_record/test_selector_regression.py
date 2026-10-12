import gzip
import hashlib
import json
from pathlib import Path
import unittest

import scoring
import selector_regression as regression


class HistoricalColoringSelectorRegression(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Execute the actual current sandbox once, including the archived
        # repair's own 1.35 CPU second search. No evaluator or selector mocks.
        cls.report = regression.replay()

    def test_sources_are_exact_archived_candidates(self):
        root = Path(__file__).resolve().parents[3]
        archive = (root / regression.ARCHIVE_PATH).read_bytes()
        self.assertEqual(hashlib.sha256(archive).hexdigest(), regression.ARCHIVE_SHA256)
        data = json.loads(gzip.decompress(archive))
        agent = next(agent for agent in data['agents'] if agent['name'] == 'agent-1')
        for index, source, expected_hash in (
            (2, regression.BAD_SOURCE, regression.BAD_SOURCE_SHA256),
            (5, regression.REPAIRED_SOURCE, regression.REPAIRED_SOURCE_SHA256),
        ):
            with self.subTest(candidate_index=index):
                self.assertEqual(source, agent['candidates'][index]['source'])
                self.assertEqual(hashlib.sha256(source.encode()).hexdigest(), expected_hash)
                self.assertEqual(expected_hash, agent['candidates'][index]['sha256'])

    def test_public_fixture_exposes_defect_and_repair_survives(self):
        bad, repaired = self.report['candidates']
        self.assertEqual(bad['public']['valid_instances'], 0)
        self.assertEqual(bad['public']['total_instances'], 1)
        self.assertTrue(bad['public']['instances'][0]['error'].startswith('IndexError:'))
        self.assertTrue(scoring.all_valid(repaired['public']))
        self.assertEqual(repaired['public']['total_cost'], 12)
        self.assertEqual(self.report['fixture']['reference_costs'], [12.0])

    def test_validity_gate_beats_cost_tie_and_is_order_invariant(self):
        self.assertEqual(self.report['status'], 'passed', self.report['checks'])
        self.assertEqual(self.report['selected'], 'archived-repair')
        self.assertEqual(self.report['cost_only_control_selected'], 'archived-defect')
        self.assertTrue(all(self.report['checks'].values()), self.report['checks'])

    def test_report_binds_fixture_sources_and_current_evaluator(self):
        fixture = self.report['fixture']['instances']
        fixture_bytes = json.dumps(fixture, sort_keys=True, separators=(',', ':')).encode()
        self.assertEqual(self.report['fixture']['sha256'], hashlib.sha256(fixture_bytes).hexdigest())
        self.assertEqual(len(fixture[0]['edges']), 66)
        root = Path(__file__).resolve().parent
        for name, digest in self.report['evaluator']['code_sha256'].items():
            self.assertEqual(digest, hashlib.sha256((root / name).read_bytes()).hexdigest())
        self.assertIn('not held-out benchmark evidence', self.report['scope'])
        self.assertNotIn('hidden', self.report['candidates'][0])
        for candidate in self.report['candidates']:
            self.assertEqual(candidate['source_sha256'], candidate['public']['source_sha256'])


if __name__ == '__main__':
    unittest.main()
