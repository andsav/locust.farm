"""Save original outputs and independently reconstruct the two-provider run."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

from runner import cost
from study import (analyze, digest, nt, prepare, read_attempts, selected,
                   valid_response, verify_requests)


def slim_manifest(manifest):
    cases = [{'id': c['id'], 'cluster': c['cluster'], 'outcome': c['outcome'],
              'public': {'evidence': [{'id': e['id']} for e in c['public']['evidence']]}}
             for c in manifest['base']['cases']]
    return {**manifest, 'base': {'cases': cases}}


def diagnostics(manifest, attempts, prior_attempts=()):
    from collections import Counter
    records = selected(attempts)
    cases = {c['id']: c for c in manifest['base']['cases']}
    valid, invalid = Counter(), []
    for name, r in records.items():
        q, profile, population, stage, agent = name.split('/')
        if valid_response(r, cases[q], stage, int(agent)) is not None:
            valid[r['provider']] += 1
        else:
            invalid.append({'label': name, 'provider': r['provider'], 'status': r['status']})
    providers = {}
    for family in manifest['models']:
        all_records = [r for r in attempts if r['provider'] == family]
        new = [r for r in [*prior_attempts, *attempts] if r['provider'] == family and not r.get('imported')]
        providers[family] = {'new_attempts': len(new), 'reused_unique_responses': sum(r.get('imported', False) for r in all_records),
                             'new_accounted_usd': sum(r.get('cost_usd', r['reserved_usd']) for r in new),
                             'valid_unique_responses': valid[family],
                             'returned_models': sorted({r['returned_model'] for r in all_records if r.get('returned_model')})}
    return {'providers': providers, 'invalid_unique_responses': invalid,
            'selected_statuses': dict(Counter(r['status'] for r in records.values())),
            'new_transport_failures': [{'label': r['label'], 'error_type': r.get('error_type'), 'http_status': r.get('http_status')}
                                       for r in [*prior_attempts, *attempts] if not r.get('imported') and r['status'] == 'transport_error'],
            'prior_unknown_after_interruption': sum(r['status'] == 'pending' for r in prior_attempts)}


def rebuild_prior(root, upstream, baseline, manifest, prior_attempts):
    """Audit archived requests with the exact historical Git code, offline."""
    amendment = manifest['amendment']
    if digest(prior_attempts) != amendment['prior_attempts_sha256']:
        raise ValueError('Prior attempts differ from amendment')
    repo = Path(__file__).resolve().parents[3]
    revision = amendment['prior_generation_revision']
    paths = ('nous_models/study.py', 'nous_models/runner.py', 'nous_transfer/protocol.py',
             'nous_transfer/experiment.py', 'luna_decision_pilot/run.py', 'luna_decision_pilot/study.py')
    code = root/'historical-code'
    for path in paths:
        target = code/path;target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(subprocess.check_output(['git', '-C', str(repo), 'show', f'{revision}:research/experiments/{path}']))
    archive = root/'prior-attempts.json';archive.write_text(json.dumps(prior_attempts))
    prior = root/'prior'
    script = '''import json,sys
from pathlib import Path
sys.path.insert(0,sys.argv[1])
import study
upstream,baseline,folder,archive=map(Path,sys.argv[2:6])
study.prepare(upstream,baseline,folder)
m=json.loads((folder/'manifest.json').read_text())
a=json.loads(archive.read_text())
study.verify_requests(m,a)
(folder/'attempts.jsonl').write_text(''.join(json.dumps(r)+'\\n' for r in a))
'''
    subprocess.run([sys.executable, '-c', script, str(code/'nous_models'), str(upstream.resolve()),
                    str(baseline.resolve()), str(prior), str(archive)], check=True, capture_output=True)
    old = json.loads((prior/'manifest.json').read_text())
    if digest(old) != amendment['prior_manifest_sha256']:
        raise ValueError('Historical manifest reconstruction mismatch')
    return prior


def verify_code(revision, hashes):
    root = Path(__file__).resolve().parents[3]
    revision = subprocess.check_output(['git', '-C', str(root), 'rev-parse', revision+'^{commit}'], text=True).strip()
    if set(hashes) != {'nous_models/study.py', 'nous_models/runner.py', 'nous_transfer/protocol.py'}:
        raise ValueError('Unexpected code provenance paths')
    for path, expected in hashes.items():
        value = subprocess.check_output(['git', '-C', str(root), 'show', f'{revision}:research/experiments/{path}'])
        if hashlib.sha256(value).hexdigest() != expected:
            raise ValueError('Code revision does not match generation hashes')
    return revision


def export(folder, destination, revision='HEAD'):
    manifest = json.loads((folder/'manifest.json').read_text())
    if digest(manifest) != (folder/'manifest.sha256').read_text().strip():
        raise ValueError('Manifest hash mismatch')
    attempts = read_attempts(folder/'attempts.jsonl')
    if any(r['status'] == 'pending' for r in attempts):
        raise ValueError('Wait until pending attempts finish')
    verify_requests(manifest, attempts)
    slim = slim_manifest(manifest)
    prior_attempts = json.loads((folder/'prior-attempts.json').read_text()) if 'amendment' in manifest else []
    bundle = {'version': 1, 'scope': 'Original model outputs; upstream prompts and brief bodies omitted',
              'manifest_sha256': digest(manifest), 'analysis_manifest': slim,
              'source_hashes': manifest['base']['source_hashes'], 'upstream_commit': manifest['base']['upstream_commit'],
              'generation_code_revision': verify_code(revision, manifest['code_hashes']),
              'analysis_python': sys.version.split()[0], 'attempts': attempts, 'prior_attempts': prior_attempts,
              'summary': analyze(slim, attempts), 'diagnostics': diagnostics(slim, attempts, prior_attempts)}
    destination.write_text(json.dumps(bundle, indent=2, allow_nan=False)+'\n')
    return {'path': str(destination), 'attempts': len(attempts), 'summary_sha256': digest(bundle['summary'])}


def verify(path, upstream=None, baseline=None):
    bundle = json.loads(path.read_text())
    manifest, attempts = bundle['analysis_manifest'], bundle['attempts']
    prior_attempts = bundle.get('prior_attempts', [])
    if any(r['status'] == 'pending' or r.get('simulated') for r in attempts):
        raise ValueError('Evidence is pending or simulated')
    if analyze(manifest, attempts) != bundle['summary']:
        raise ValueError('Summary mismatch')
    if diagnostics(manifest, attempts, prior_attempts) != bundle['diagnostics']:
        raise ValueError('Diagnostics mismatch')
    if 'amendment' in manifest:
        amendment = manifest['amendment']
        carry = sum(r.get('cost_usd', r['reserved_usd']) for r in prior_attempts if not r.get('imported'))
        if (digest(prior_attempts) != amendment['prior_attempts_sha256']
                or carry != manifest['carry_forward_accounted_usd']
                or carry != amendment['prior_new_accounted_usd']):
            raise ValueError('Prior attempt accounting/provenance mismatch')
    for r in [*prior_attempts, *attempts]:
        if 'usage' in r:
            charge, incoming = cost(r['provider'], r['usage'], manifest['models'][r['provider']])
            if abs(charge-r['cost_usd']) > 1e-12 or charge > r['reserved_usd'] or incoming > r['input_bound']:
                raise ValueError('Cost/reservation mismatch')
    if bundle['summary']['new_accounted_usd'] > manifest['new_spend_ceiling_usd']:
        raise ValueError('Spending ceiling exceeded')
    rebuilt_requests = False
    if upstream is not None or baseline is not None:
        if upstream is None or baseline is None:
            raise ValueError('Request reconstruction requires upstream and baseline')
        verify_code(bundle['generation_code_revision'], manifest['code_hashes'])
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)/'rebuilt'
            prior = rebuild_prior(Path(tmp), upstream, baseline, manifest, prior_attempts) if 'amendment' in manifest else None
            prepare(upstream, baseline, folder, manifest['new_spend_ceiling_usd'], prior=prior)
            rebuilt = json.loads((folder/'manifest.json').read_text())
            if digest(rebuilt) != bundle['manifest_sha256'] or slim_manifest(rebuilt) != manifest:
                raise ValueError('Reconstructed manifest mismatch; use the original generation code')
            if [r for r in attempts if r.get('imported')] != read_attempts(folder/'attempts.jsonl'):
                raise ValueError('Imported Luna records changed')
            verify_requests(rebuilt, attempts)
            rebuilt_requests = True
    return {'verified_attempts': len(attempts), 'verified_unique_requests': len(selected(attempts)),
            'verified_archived_attempts': len(prior_attempts),
            'scores_and_costs_recomputed': True, 'requests_reconstructed_from_sources': rebuilt_requests}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('export');p.add_argument('folder', type=Path);p.add_argument('destination', type=Path)
    p.add_argument('--code-revision', default='HEAD')
    p = sub.add_parser('verify');p.add_argument('path', type=Path)
    p.add_argument('--upstream', type=Path);p.add_argument('--baseline', type=Path)
    args = parser.parse_args()
    result = (export(args.folder, args.destination, args.code_revision) if args.command == 'export'
              else verify(args.path, args.upstream, args.baseline))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
