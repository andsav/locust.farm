#!/usr/bin/env python3
"""Select calibrated solo procedures only after all three studies are complete.

Reads archived evidence only. No API calls, evaluations, or harness imports.
"""
import argparse
from collections import defaultdict
import json
import math
from pathlib import Path
import statistics

from private_stage_diagnostics import digest, read, summarize as private_stage

ORDER = ('continue', 'verify', 'restart')
STUDIES = dict(zip(ORDER, ('calibration-continue', 'calibration-verify-min16', 'calibration-restart-min16')))


def choose(rows):
    """Exact frozen lexicographic rule; never turn uncertainty into a charge."""
    best = min(row['family_balanced_hidden'] for row in rows.values())
    tied = [name for name in ORDER if rows[name]['family_balanced_hidden'] == best]
    result = dict(selected=None, quality_tied_procedures=tied, family_balanced_hidden=best)
    if len(tied) == 1:
        result.update(selected=tied[0], reason='strictly lowest family-balanced hidden score')
    elif any(not rows[name]['actual_cost_fully_known'] for name in tied):
        result.update(reason='exact quality tie includes unknown actual cost; selection unresolved')
    else:
        cost = min(rows[name]['actual_settled_cost'] for name in tied)
        cost_ties = [name for name in tied if rows[name]['actual_settled_cost'] == cost]
        result.update(selected=cost_ties[0], actual_settled_cost=cost,
                      cost_tied_procedures=cost_ties,
                      reason='lower actual settled cost after exact quality tie' if len(cost_ties) == 1
                      else 'exact quality and actual-cost tie; fixed continue/verify/restart order')
    return result


def first_valid_evidence(trial, ledger, allowance):
    candidates = [c for c in trial['candidates'] if c['public']['instances']
                  and all(row['valid'] for row in c['public']['instances'])]
    if not candidates:
        return dict(candidate_id=None, observation='no fully public-valid candidate')
    candidate = min(candidates, key=lambda c: (c['time'], c['sequence'], c['id']))
    agent = next(a for a in trial['agents'] if any(c['id'] == candidate['id'] for c in a['candidates']))
    calls = {call for event in agent['trace'] if event['event'] == 'tool_results'
             for call, value in event['results'] if value.get('candidate_id') == candidate['id']}
    generating = [r for r in agent['requests'] if any(item.get('call_id') in calls
                  for item in r.get('response', {}).get('output', []) if item.get('type') == 'function_call')]
    if len(generating) != 1:
        raise ValueError('first valid candidate does not have exactly one archived generating response')
    request = generating[0]
    entries = [ledger[id] for id in agent['request_ids'] if ledger[id]['reserved_at'] <= candidate['time']]
    known = [r for r in entries if r.get('cost') is not None and r.get('settled_at') is not None
             and r['settled_at'] <= candidate['time']]
    unknown = [r for r in entries if r not in known]
    usage_complete = all(isinstance(r.get('usage', {}).get('output_tokens'), int) for r in entries)
    known_cost = math.fsum(r['cost'] for r in known)
    actual = known_cost if not unknown else None
    return dict(candidate_id=candidate['id'], source_sha256=candidate['sha256'],
        candidate_available_at=candidate['time'], generating_request_id=request['id'],
        generating_response_id=request['response']['id'],
        generating_response_output_tokens=request['response']['usage']['output_tokens'],
        cumulative_generation_requests=len(entries),
        cumulative_generation_output_tokens=sum(r['usage']['output_tokens'] for r in entries) if usage_complete else None,
        cumulative_actual_cost=actual, cumulative_known_cost=known_cost,
        cumulative_unresolved_reservations=math.fsum(r['reserved'] for r in unknown),
        cumulative_committed_cost_at_availability=known_cost + math.fsum(r['reserved'] for r in unknown),
        full_solo_allowance=allowance,
        nominal_early_luna_group_worker_allowance=allowance / 12 if trial['kind'] == 'S' else None,
        nominal_delayed_luna_group_worker_allowance=allowance / 6 if trial['kind'] == 'S' else None,
        observed_actual_cost_below_nominal_early_luna_slice=(actual <= allowance / 12
            if actual is not None and trial['kind'] == 'S' else None),
        scope='Observed in a full-B solo prompt and trajectory. Response token counts include reasoning and every call in that response. This is not proof of affordability or valid production under a group prefix prompt, B/12 or B/6 allowance, or its output limit.')


def load_study(root, procedure):
    study = root / STUDIES[procedure]
    private_path = root / f'private-stage-{procedure}.json'
    supplied_private = read(private_path)
    private = private_stage(study)
    for key in ('manifest_sha256', 'seal_sha256', 'trials', 'summary'):
        if supplied_private[key] != private[key]:
            raise ValueError('private-stage diagnostic does not match sealed evidence: ' + procedure)
    manifest = read(study / 'manifest.json')
    if any(manifest['continuation'][model] != procedure for model in ('luna', 'frontier')):
        raise ValueError('study continuation differs from declared procedure')
    hidden, proof, analysis = (read(study / name) for name in ('hidden.json', 'hidden-seal.json', 'analysis.json'))
    if proof['hidden_sha256'] != digest(study / 'hidden.json') or proof['seal_sha256'] != private['seal_sha256']:
        raise ValueError('hidden scores differ from completed evidence')
    if hidden['manifest_sha256'] != private['manifest_sha256']:
        raise ValueError('hidden scores belong to another manifest')
    for name, expected in proof['journal_files'].items():
        if digest(study / name) != expected:
            raise ValueError('hidden journal changed: ' + name)
    if analysis['hidden_sha256'] != proof['hidden_sha256'] or analysis['manifest_sha256'] != private['manifest_sha256']:
        raise ValueError('analysis belongs to another archive')
    ledger = {row['id']: row for row in read(study / 'ledger.json')}
    models, signatures = {}, {}
    for model_key, kind in (('luna', 'S'), ('frontier', 'R')):
        arms = [name for name, arm in manifest['arms'].items() if arm['kind'] == kind]
        if len(arms) != 1:
            raise ValueError('calibration must contain exactly one solo arm per model')
        arm = arms[0]
        values, trials, used, signature = defaultdict(lambda: defaultdict(list)), {}, set(), []
        for block in manifest['blocks']:
            key = f'{block["id"]}/{arm}'
            trial = read(study / 'trials' / f'{key}.json')
            row = analysis['trials'][key]
            observed = hidden['trials'][key]
            outcome = observed['fallback'] if trial['selected'] is None else observed['candidates'][trial['selected']]
            if observed['selected'] != trial['selected'] or outcome['normalized'] != row['hidden']:
                raise ValueError('analysis score differs from sealed deliverable')
            values[block['family']][block['pack']].append(row['hidden'])
            signature.append((block['family'], block['pack'], block['repetition'], block['public']['digest'], block['hidden']['digest']))
            used.update(trial['request_ids'])
            trials[key] = dict(family=block['family'], hidden=row['hidden'], fallback=row['fallback'],
                failures=row['failures'], spending=row['spending'],
                private_stage=private['trials'][key]['agents'],
                first_fully_public_valid_candidate=first_valid_evidence(trial, ledger, manifest['budget']['task_arm_usd']))
        entries = [ledger[id] for id in used]
        known = [r for r in entries if r.get('cost') is not None and r.get('settled_at') is not None]
        families = {family: statistics.fmean(statistics.fmean(v) for v in packs.values()) for family, packs in values.items()}
        models[model_key] = dict(model=manifest['models'][model_key]['model'], family_means=families,
            family_balanced_hidden=statistics.fmean(families.values()),
            actual_cost_fully_known=len(known) == len(entries),
            actual_settled_cost=math.fsum(r['cost'] for r in known) if len(known) == len(entries) else None,
            known_cost_subtotal=math.fsum(r['cost'] for r in known),
            unresolved_reservations=math.fsum(r['reserved'] for r in entries if r not in known), trials=trials)
        signatures[model_key] = sorted(signature)
    return dict(study_id=manifest['study_id'], directory=str(study),
                evidence=dict(manifest_sha256=private['manifest_sha256'], seal_sha256=private['seal_sha256'],
                              hidden_sha256=proof['hidden_sha256'], analysis_sha256=digest(study / 'analysis.json'),
                              private_stage_sha256=digest(private_path)),
                models=models, signatures=signatures)


def consolidate(root):
    root = Path(root).resolve()
    required = [root / name / file for name in STUDIES.values()
                for file in ('manifest.json', 'seal.json', 'hidden.json', 'hidden-seal.json', 'analysis.json')]
    required += [root / f'private-stage-{procedure}.json' for procedure in ORDER]
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        raise ValueError('calibration incomplete; no selection made. Missing: ' + ', '.join(missing))
    studies = {procedure: load_study(root, procedure) for procedure in ORDER}
    for model in ('luna', 'frontier'):
        if any(studies[p]['signatures'][model] != studies['continue']['signatures'][model] for p in ORDER):
            raise ValueError('calibration procedures do not have identical family/pack/repetition/instance coverage')
        if any(studies[p]['models'][model]['model'] != studies['continue']['models'][model]['model'] for p in ORDER):
            raise ValueError('calibration procedures use different model identities')
    return dict(schema='shared-record-calibration-selection.v1', diagnostic_sha256=digest(__file__),
        rule='Lowest family-balanced hidden mean; exact score ties use lower fully known actual settled cost; exact remaining tie uses continue, verify, restart. No tolerance threshold. Unknown actual cost among quality-tied procedures leaves selection unresolved.',
        selections={model: choose({p: studies[p]['models'][model] for p in ORDER}) for model in ('luna', 'frontier')},
        group_prefix_qualification=dict(qualified=False,
            reason='These are full-B solo observations. Eligibility and useful output under the actual early/delayed group prefix prompts and budgets require separate evidence.'),
        claim_limit='Sparse development selects observed comparators; it does not identify a generally optimal continuation or prove that private review caused a quality difference.',
        studies=studies)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if any(args.output.resolve().is_relative_to((args.root / name).resolve()) for name in STUDIES.values()):
        raise ValueError('write consolidation outside all sealed study directories')
    result = consolidate(args.root)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + '\n')
    print(json.dumps(dict(output=str(args.output.resolve()), selections=result['selections']), indent=2))
