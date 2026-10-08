"""Recompute retained calibration outcomes without network access."""
import json
from pathlib import Path
import tempfile
from calibrate import Calibration, public
from study import digest

BUNDLE = Path(__file__).resolve().parents[2] / 'evidence' / 'evidence-calibration-2026-10-08.json'

def verify(path=BUNDLE):
    bundle = json.loads(path.read_text())
    for run in bundle['runs']:
        design, records = run['design'], run['records']
        assert digest(design) == run['design_sha256']
        assert len({r['label'] for r in records}) == len(records)
        assert {r['label'] for r in records} == {j['label'] for j in design['jobs']}
        for job in design['jobs']:
            expected = public(job['row'])
            if job['condition'] == 'no_context':expected['documents'] = []
            assert job['public'] == expected
        for r in records:
            assert r['status'] in ('completed', 'incomplete', 'transport_error')
            if 'usage' in r:
                assert r['returned_model'] == design['model']
                assert r['cost_usd'] <= r['reserved_usd']
                assert abs(r['cost_usd'] - (r['usage']['input_tokens']*.125+r['usage']['output_tokens']*.5)/1e6) < 1e-12
        with tempfile.TemporaryDirectory() as tmp:
            folder=Path(tmp)
            (folder/'design.json').write_text(json.dumps(design))
            (folder/'records.json').write_text(json.dumps(records))
            assert Calibration(folder).analyze() == run['summary']
        print(json.dumps({'run':run['name'], 'verified_calls':len(records), 'groups':run['summary']['groups']}))
    return bundle

if __name__ == '__main__': verify()
