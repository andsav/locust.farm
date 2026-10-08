import json
from pathlib import Path
import tempfile
import unittest

from evidence import diagnostics, slim_manifest, verify
from study import analyze
from test_study import fixture, starts


class EvidenceTests(unittest.TestCase):
    def bundle(self):
        m = slim_manifest(fixture());attempts = starts(fixture())
        return {'analysis_manifest': m, 'attempts': attempts, 'summary': analyze(m, attempts),
                'diagnostics': diagnostics(m, attempts)}

    def test_slim_manifest_omits_source_content(self):
        m = slim_manifest(fixture())
        self.assertEqual(set(m['base']), {'cases'})
        self.assertEqual(m['base']['cases'][0]['public'], {'evidence': [{'id': 'brief'}]})

    def test_summary_and_diagnostics_are_recomputed(self):
        bundle = self.bundle()
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'evidence.json';path.write_text(json.dumps(bundle))
            self.assertEqual(verify(path)['verified_unique_requests'], 40)
            bundle['diagnostics']['providers']['luna']['valid_unique_responses'] = 999
            path.write_text(json.dumps(bundle))
            with self.assertRaises(ValueError):verify(path)

    def test_cost_tampering_rejected_even_when_summary_updated(self):
        bundle = self.bundle();r = bundle['attempts'][0]
        r.update(usage={'input_tokens': 100, 'output_tokens': 20}, input_bound=5000,
                 reserved_usd=.01, cost_usd=.001)
        m = bundle['analysis_manifest'];a = bundle['attempts']
        bundle['summary'] = analyze(m, a);bundle['diagnostics'] = diagnostics(m, a)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'evidence.json';path.write_text(json.dumps(bundle))
            with self.assertRaises(ValueError):verify(path)


if __name__ == '__main__':
    unittest.main()
