"""Frozen Luna/Sonnet team comparison, sharing identical independent calls."""
from datetime import date
import hashlib
import json
from pathlib import Path
import random
import statistics
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from nous_transfer import protocol as nt

TEAMS = ('luna', 'sonnet', 'mixed')
STAGES = ('initial', 'private', 'exchange')
MODELS = {
    'luna': {'id': 'gpt-6-luna', 'input_rate': .125, 'output_rate': .5,
             'reservation_input_rate': .125, 'effort': 'high', 'max_tokens': 4096},
    'sonnet': {'id': 'claude-sonnet-5-5', 'input_rate': 2, 'output_rate': 10,
               'reservation_input_rate': 4, 'effort': 'high', 'max_tokens': 4096},
}


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()).hexdigest()


def code_hashes():
    here = Path(__file__).parent
    return {str(p.relative_to(here.parent)): nt.file_hash(p) for p in
            (here/'study.py', here/'runner.py', here.parent/'nous_transfer/protocol.py')}


def label(qid, profile, population, stage, agent):
    return f'{qid}/{profile}/{population}/{stage}/{agent}'


def selected(attempts):
    result = {}
    for r in attempts:
        old = result.get(r['label'])
        if old is None or old['status'] == 'transport_error':
            result[r['label']] = r
    return result


def provider(manifest, case, team, agent):
    return manifest['mixed_models'][case['id']][agent] if team == 'mixed' else team


def valid_response(record, case, stage, agent):
    allowed = {e['id'] for e in case['public']['evidence']}
    obj = nt.parse(record, allowed)
    if obj is None and record and record.get('status') == 'completed':
        # Accept one unambiguous forecast object surrounded by prose/fences.
        # Do not repair values, select among multiple answers, or retry the model.
        text = record.get('text', '')
        candidates = []
        for start, character in enumerate(text):
            if character != '{':
                continue
            try:
                candidate, _ = json.JSONDecoder().raw_decode(text[start:])
            except ValueError:
                continue
            if isinstance(candidate, dict) and {'probability', 'evidence_ids', 'explanation', 'used_peer_ids'} <= set(candidate):
                candidates.append(candidate)
        if len(candidates) == 1:
            obj = nt.parse({'status': 'completed', 'text': json.dumps(candidates[0])}, allowed)
    if obj and (agent in obj['used_peer_ids'] or (stage != 'exchange' and obj['used_peer_ids'])):
        return None
    return obj


def request_body(manifest, records, case, profile, population, stage, agent):
    base = manifest['base']
    ids = list(range(10))
    random.Random(nt.digest([base['seed'], case['id']])).shuffle(ids)
    instructions = (base['base_prompt']+'\n\n'+base['profile_blocks'][profile][ids[agent]]+
                    '\n\n'+base['initial_prompt' if stage == 'initial' else 'review_prompt'])
    public = dict(case['public'], participant=agent)
    model = provider(manifest, case, population, agent)
    if stage != 'initial':
        def initial(i, family):
            record = records.get(label(case['id'], profile, family, 'initial', i))
            return valid_response(record, case, 'initial', i) or {'status': 'unavailable_or_invalid'}
        public['initial_forecast'] = initial(agent, model)
        public['peer_forecasts'] = ([{'peer_id': i, **initial(i, provider(manifest, case, population, i))}
                                     for i in range(10) if i != agent] if stage == 'exchange' else [])
    settings = manifest['models'][model]
    message = json.dumps(public, sort_keys=True)
    if model == 'luna':
        body = dict(model=settings['id'], instructions=instructions, input=message,
                    reasoning={'effort': settings['effort']}, max_output_tokens=settings['max_tokens'],
                    service_tier='default', store=False)
    else:
        body = dict(model=settings['id'], system=instructions,
                    messages=[{'role': 'user', 'content': message}], max_tokens=settings['max_tokens'],
                    thinking={'type': 'adaptive'}, output_config={'effort': settings['effort']},
                    service_tier='standard_only')
    return model, body


def planned_labels(manifest):
    return {label(c['id'], p, population, stage, i)
            for c in manifest['base']['cases'] for p in nt.CONDITIONS for stage in STAGES
            for population in (TEAMS if stage == 'exchange' else TEAMS[:2]) for i in range(10)}


def verify_requests(manifest, attempts):
    records = selected(attempts)
    cases = {c['id']: c for c in manifest['base']['cases']}
    expected = planned_labels(manifest)
    if len({r['attempt_id'] for r in attempts}) != len(attempts):
        raise ValueError('Duplicate attempt identity')
    for r in attempts:
        if r['label'] not in expected:
            raise ValueError('Unexpected request identity')
        qid, profile, population, stage, agent = r['label'].split('/')
        model, body = request_body(manifest, records, cases[qid], profile, population, stage, int(agent))
        if model != r['provider'] or digest(body) != r['request_sha256']:
            raise ValueError('Request differs from frozen inputs: '+r['label'])


def prepare(upstream, baseline, folder, ceiling=100, prior=None):
    with tempfile.TemporaryDirectory() as tmp:
        manifests = []
        for phase in ('smoke', 'replay'):
            dest = Path(tmp)/phase
            nt.prepare(upstream, dest, phase, date(2026, 10, 8), 10)
            manifests.append(json.loads((dest/'manifest.json').read_text()))
    base = manifests[0]
    cohorts = {c['id']: name for m, name in zip(manifests, ('previous', 'additional')) for c in m['cases']}
    mixed = {}
    for m in manifests:
        for index, case in enumerate(m['cases']):
            ids = list(range(10))
            random.Random(nt.digest([base['seed'], case['id']])).shuffle(ids)
            mixed[case['id']] = ['sonnet' if (p+index) % 2 == 0 else 'luna' for p in ids]
    # Each persona uses each provider five times within each ten-question cohort.
    base['cases'] = [c for m in manifests for c in m['cases']]
    previous = json.loads(baseline.read_text())
    if previous['summary']['completed_calls'] != 900 or len(previous['records']) != 900:
        raise ValueError('Expected the completed original Luna evidence')
    manifest = {'version': 2, 'scope': 'Historical Luna/Sonnet team comparison; exploratory',
                'base': base, 'models': MODELS, 'cohorts': cohorts, 'mixed_models': mixed,
                'baseline_sha256': nt.file_hash(baseline), 'new_spend_ceiling_usd': ceiling,
                'baseline_accounted_usd': previous['recovery']['accounted_usd_including_all_attempts'],
                'maximum_attempts': 6, 'retry_delays_seconds': [5, 15, 30, 60, 60],
                'max_concurrency': 8, 'code_hashes': code_hashes(),
                'response_parsing': 'Accept a strict JSON response or exactly one forecast-schema JSON object surrounded by prose/fences; no value coercion, no choice among multiple answers',
                'primary_contrasts': ['mixed initial versus Luna initial', 'mixed initial versus Sonnet initial'],
                'secondary_contrasts': ['exchange minus private within each team', 'profile effects within each team'],
                'invalid_policy': 'Score 0.5; retain first non-transport result; invalid initial becomes explicit unavailable marker in both revision branches',
                'sharing': 'Initial/private calls shared exactly; exchange always uses the selected team initial messages; anonymous provider identities'}
    imported = []
    for r in previous['records']:
        qid, profile, stage, agent = r['label'].split('/')
        name = label(qid, profile, 'luna', stage, int(agent))
        imported.append({**r, 'label': name, 'attempt_id': name+'#imported', 'provider': 'luna',
                         'imported': True, 'source_label': r['label']})
    verify_requests(manifest, imported)
    prior_attempts = None
    if prior is not None:
        prior_manifest = json.loads((prior/'manifest.json').read_text())
        prior_attempts = read_attempts(prior/'attempts.jsonl')
        if digest(prior_manifest) != (prior/'manifest.sha256').read_text().strip():
            raise ValueError('Prior manifest hash mismatch')
        old_new = [r for r in prior_attempts if not r.get('imported')]
        manifest['amendment'] = {'reason': 'Normalize an unambiguous JSON forecast surrounded by prose equally for both providers',
                                 'prior_generation_revision': '4d52058',
                                 'prior_manifest_sha256': digest(prior_manifest),
                                 'prior_attempts_sha256': digest(prior_attempts),
                                 'prior_new_attempts': len(old_new),
                                 'prior_unknown_after_interruption': sum(r['status'] == 'pending' for r in old_new),
                                 'prior_new_accounted_usd': sum(r.get('cost_usd', r['reserved_usd']) for r in old_new)}
        manifest['carry_forward_accounted_usd'] = manifest['amendment']['prior_new_accounted_usd']
        # Initial bodies are unchanged. Import them first, then compare every
        # revision body against the corrected initial-message set before reuse.
        cases = {c['id']: c for c in base['cases']}
        for stage in STAGES:
            records = selected(imported)
            for r in prior_attempts:
                if r.get('imported') or r['status'] in ('pending', 'transport_error'):
                    continue
                qid, profile, population, recorded_stage, agent = r['label'].split('/')
                if stage != recorded_stage or r['label'] in records:
                    continue
                _, body = request_body(manifest, records, cases[qid], profile, population, stage, int(agent))
                if digest(body) == r['request_sha256']:
                    copied = {**r, 'attempt_id': r['label']+'#reused-v1', 'imported': True,
                              'reuse_origin': 'formatting-amendment', 'prior_attempt_id': r['attempt_id']}
                    imported.append(copied)
                    records[r['label']] = copied
        verify_requests(manifest, imported)
    folder.mkdir(parents=True, exist_ok=False)
    (folder/'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
    (folder/'manifest.sha256').write_text(digest(manifest)+'\n')
    (folder/'attempts.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in imported))
    if prior_attempts is not None:
        (folder/'prior-attempts.json').write_text(json.dumps(prior_attempts, indent=2)+'\n')
    return {'questions': len(base['cases']), 'clusters': len({c['cluster'] for c in base['cases']}),
            'logical_forecasts': len(base['cases'])*270, 'unique_requests': len(planned_labels(manifest)),
            'imported': len(imported), 'new_requests': len(planned_labels(manifest))-len(imported),
            'manifest_sha256': digest(manifest)}


def read_attempts(path):
    records = {}
    for line in path.read_text().splitlines():
        r = json.loads(line)
        records[r['attempt_id']] = r
    return list(records.values())


def analyze(manifest, attempts):
    records = selected(attempts)
    rows = []
    for case in manifest['base']['cases']:
        for profile in nt.CONDITIONS:
            for team in TEAMS:
                for stage in STAGES:
                    objects = []
                    attributed = set()
                    for i in range(10):
                        family = provider(manifest, case, team, i)
                        pop = team if stage == 'exchange' else family
                        name = label(case['id'], profile, pop, stage, i)
                        objects.append(valid_response(records.get(name), case, stage, i))
                        attributed.add(name)
                        if stage != 'initial':
                            attributed.add(label(case['id'], profile, family, 'initial', i))
                    values = [o['probability'] if o else .5 for o in objects]
                    metrics = nt.group_metrics(values, case['outcome'])
                    rows.append(dict(id=case['id'], cluster=case['cluster'], cohort=manifest['cohorts'][case['id']],
                                     profile=profile, team=team, stage=stage,
                                     valid_agents=sum(o is not None for o in objects),
                                     attributed_usd=sum(r.get('cost_usd', r['reserved_usd']) for r in attempts if r['label'] in attributed),
                                     **metrics))
    groups, contrasts = {}, {}
    for cohort in ('all', 'previous', 'additional'):
        cohort_rows = [r for r in rows if cohort == 'all' or r['cohort'] == cohort]
        for profile in nt.CONDITIONS:
            for team in TEAMS:
                for stage in STAGES:
                    group = [r for r in cohort_rows if (r['profile'], r['team'], r['stage']) == (profile, team, stage)]
                    clusters = sorted({r['cluster'] for r in group})
                    groups[f'{cohort}/{profile}/{team}/{stage}'] = {
                        'questions': len(group), 'clusters': len(clusters),
                        'valid_agents': sum(r['valid_agents'] for r in group),
                        'cluster_equal_brier': statistics.mean(statistics.mean(r['ensemble_brier'] for r in group if r['cluster'] == cl) for cl in clusters),
                        'mean_jsd_bits': statistics.mean(r['jsd_bits'] for r in group),
                        'attributed_usd': sum(r['attributed_usd'] for r in group)}
            def loss(team, stage):
                return groups[f'{cohort}/{profile}/{team}/{stage}']['cluster_equal_brier']
            contrasts[f'{cohort}/{profile}'] = {
                'mixed_minus_luna_initial': loss('mixed', 'initial')-loss('luna', 'initial'),
                'mixed_minus_sonnet_initial': loss('mixed', 'initial')-loss('sonnet', 'initial'),
                **{team+'_exchange_minus_private': loss(team, 'exchange')-loss(team, 'private') for team in TEAMS}}
    current = [r for r in attempts if not r.get('imported')]
    new_cost = manifest.get('carry_forward_accounted_usd', 0)+sum(r.get('cost_usd', r['reserved_usd']) for r in current)
    return {'planned_unique_requests': len(planned_labels(manifest)), 'selected_records': len(records),
            'completed_unique_requests': sum(r['status'] == 'completed' for r in records.values()),
            'missing_requests': len(planned_labels(manifest)-set(records)),
            'new_attempts': len(current)+manifest.get('amendment', {}).get('prior_new_attempts', 0),
            'post_amendment_attempts': len(current), 'transport_errors': sum(r['status'] == 'transport_error' for r in current),
            'new_accounted_usd': new_cost, 'total_accounted_usd_with_prior_run': new_cost+manifest['baseline_accounted_usd'],
            'groups': groups, 'contrasts': contrasts, 'rows': rows,
            'intervals': None, 'interpretation': 'Descriptive historical comparison; only five provisional clusters, two previously inspected; no confirmatory intervals or equal-dollar claim'}
