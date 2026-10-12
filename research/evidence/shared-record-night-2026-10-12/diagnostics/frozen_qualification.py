#!/usr/bin/env python3
"""Summarize frozen evaluator records; no model calls or evaluation reruns."""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import statistics

from private_stage_diagnostics import digest, read, summarize as private_stage


def all_valid(score):
    return bool(score['instances']) and all(row['valid'] for row in score['instances'])


def noise(score):
    repeats, instances = defaultdict(list), defaultdict(list)
    for row in score['instances']:
        repeats[row['repetition']].append(row)
        instances[row['instance_index']].append(row)
    assert sorted(repeats) == list(range(score['repetitions']))
    reference = score['reference_total'] / score['repetitions']
    values = [math.fsum(row['cost'] for row in repeats[rep]) / reference for rep in sorted(repeats)]
    assert math.isclose(statistics.fmean(values), score['normalized'], abs_tol=1e-12)
    return dict(repetitions=score['repetitions'], reference_total_per_repeat=reference,
                repeat_normalized_scores=values, normalized_range=max(values) - min(values),
                normalized_sample_stddev=statistics.stdev(values) if len(values) > 1 else None,
                instances=len(instances),
                instances_with_cost_variation=sum(len({r['cost'] for r in rows}) > 1 for rows in instances.values()),
                instances_with_validity_flips=sum(len({r['valid'] for r in rows}) > 1 for rows in instances.values()),
                invalid_evaluation_rows=sum(not r['valid'] for r in score['instances']),
                evaluation_rows=len(score['instances']))


def qualify(study):
    study = Path(study).resolve()
    private = private_stage(study)  # Requires and verifies the completed live seal.
    manifest, hidden = read(study / 'manifest.json'), read(study / 'hidden.json')
    proof, analysis = read(study / 'hidden-seal.json'), read(study / 'analysis.json')
    assert proof['hidden_sha256'] == digest(study / 'hidden.json')
    assert proof['seal_sha256'] == digest(study / 'seal.json')
    assert hidden['manifest_sha256'] == digest(study / 'manifest.json')
    for name, expected in proof['journal_files'].items():
        assert digest(study / name) == expected, name
    assert analysis['manifest_sha256'] == hidden['manifest_sha256']
    assert analysis['hidden_sha256'] == proof['hidden_sha256']
    ledger = read(study / 'ledger.json')
    by_request = {row['id']: row for row in ledger}
    records, results = {'public': {}, 'hidden': {}}, {}
    for key, private_trial in private['trials'].items():
        trial = read(study / 'trials' / f'{key}.json')
        scores = hidden['trials'][key]['candidates']
        ties = defaultdict(set)
        selected = next((c for c in trial['candidates'] if c['id'] == trial['selected']), None)
        for candidate in trial['candidates']:
            if all_valid(candidate['public']):
                ties[candidate['public']['normalized']].add(candidate['sha256'])
            for split, score in [('public', candidate['public']), ('hidden', scores[candidate['id']])]:
                score_digest = hashlib.sha256(json.dumps(score, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
                identity = (trial['family'], trial['pack'], candidate['sha256'], score_digest if split == 'public' else '')
                if identity not in records[split]:
                    records[split][identity] = dict(family=trial['family'], pack=trial['pack'],
                        source_sha256=candidate['sha256'], occurrences=[], **noise(score))
                records[split][identity]['occurrences'].append(dict(trial=key, candidate_id=candidate['id']))
        coverage = []
        for agent in trial['agents']:
            first = min((c['time'] for c in agent['candidates']), default=None)
            requests = [by_request[id] for id in agent['request_ids']]
            coverage.append(dict(agent=agent['name'], requests=len(requests),
                requests_after_first_candidate=sum(r['reserved_at'] > first for r in requests) if first is not None else 0,
                candidates=len(agent['candidates']), candidates_after_first=max(0, len(agent['candidates']) - 1),
                final_trace_event=agent['trace'][-1]['event'] if agent['trace'] else None,
                response_statuses=dict(Counter(e.get('status') for e in agent['trace'] if e['event'] == 'response'))))
        row = analysis['trials'][key]
        results[key] = dict(family=trial['family'], arm=trial['arm'], selected=trial['selected'],
            selected_public_normalized=row['public'], selected_hidden_normalized=row['hidden'],
            fallback=row['fallback'], selector_regret=row['selector_regret'], oracle_hidden=row['oracle_hidden'],
            public_tie_groups=[dict(public_normalized_score=value, distinct_source_count=len(hashes), source_sha256=sorted(hashes))
                               for value, hashes in sorted(ties.items()) if len(hashes) > 1],
            public_tied_distinct_source_pairs=sum(len(v) * (len(v) - 1) // 2 for v in ties.values()),
            best_public_tied_distinct_sources=len(ties[min(ties)]) if ties else 0,
            selected_public_valid_hidden_invalid=bool(selected and all_valid(selected['public']) and not all_valid(scores[selected['id']])),
            public_valid_hidden_invalid_candidates=sum(all_valid(c['public']) and not all_valid(scores[c['id']]) for c in trial['candidates']),
            continuation=coverage, explicit_private_stage=private_trial['agents'], failures=row['failures'], spending=row['spending'])
    summaries = {}
    for split, items in records.items():
        values = list(items.values())
        ranges = [row['normalized_range'] for row in values]
        summaries[split] = dict(source_evaluation_records=len(values),
            unique_sources=len({row['source_sha256'] for row in values}),
            max_normalized_repeat_range=max(ranges, default=0),
            mean_normalized_repeat_range=statistics.fmean(ranges) if ranges else None,
            records_with_range_above_0_005=sum(x > .005 for x in ranges), records_with_range_above_0_01=sum(x > .01 for x in ranges),
            instance_source_pairs=sum(row['instances'] for row in values),
            **{name: sum(row[name] for row in values) for name in ('instances_with_cost_variation', 'instances_with_validity_flips', 'invalid_evaluation_rows', 'evaluation_rows')})
    failed = [dict(id=row['id'], label=row['label'], status=row['status'], reserved=row['reserved'],
                   cost=row.get('cost'), error=row.get('error')) for row in ledger if row['status'] != 'complete']
    return dict(schema='shared-record-qualification.v1', study_id=manifest['study_id'], source_directory=str(study),
        evidence=dict(manifest_sha256=hidden['manifest_sha256'], seal_sha256=hidden['seal_sha256'],
                      hidden_sha256=proof['hidden_sha256'], analysis_sha256=digest(study / 'analysis.json'),
                      diagnostic_sha256=digest(__file__), hidden_journal_files_verified=len(proof['journal_files'])),
        method=dict(noise='Sum costs within each frozen repetition and divide by reference_total/repetitions. Retain public score records separately when the same source has differing records; deduplicate hidden sources within family/pack.',
                    tie='Exact public-score equality among fully public-valid distinct source hashes.',
                    limitation='Two seeded repeats combine algorithmic randomness and execution variation. They do not establish a stable noise distribution or general selector performance. All invalid outcomes and failed requests remain retained.'),
        evaluation=manifest['evaluation'], noise_summary=summaries,
        noise_exceptions={split: [row for row in items.values() if row['normalized_range'] or row['instances_with_validity_flips']]
                          for split, items in records.items()}, trials=results,
        summary=dict(trials=len(results),
            selected_public_valid_hidden_invalid=sum(row['selected_public_valid_hidden_invalid'] for row in results.values()),
            public_valid_hidden_invalid_candidates=sum(row['public_valid_hidden_invalid_candidates'] for row in results.values()),
            public_tied_distinct_source_pairs=sum(row['public_tied_distinct_source_pairs'] for row in results.values()),
            max_selector_regret=max(row['selector_regret'] for row in results.values()),
            mean_selector_regret=statistics.fmean(row['selector_regret'] for row in results.values()),
            fallback_trials=sum(row['fallback'] for row in results.values()),
            trial_failures_retained=sum(bool(row['failures']) for row in results.values()),
            billing=analysis['billing'], failed_requests_retained=failed, private_stage=private['summary']))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('study', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if args.output.resolve().is_relative_to(args.study.resolve()):
        raise ValueError('write diagnostics outside the sealed study directory')
    result = qualify(args.study)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + '\n')
    print(json.dumps(dict(path=str(args.output.resolve()), noise_summary=result['noise_summary'], summary=result['summary']), indent=2))
