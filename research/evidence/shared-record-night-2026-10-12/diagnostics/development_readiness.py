#!/usr/bin/env python3
"""Read archived development evidence and propose, but never run, a diagnostic.

Only Python standard-library reads and output writes. No harness imports,
candidate execution, model calls, campaign allocation, or freeze operations.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import statistics


def read(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def objsha(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def summarize(score):
    rows = score['instances']
    repeats = sorted({row['repetition'] for row in rows})
    denominator = score['reference_total'] / score['repetitions']
    values = [math.fsum(row['cost'] for row in rows if row['repetition'] == rep) / denominator
              for rep in repeats]
    cpus = [row['cpu_seconds'] for row in rows if 'cpu_seconds' in row]
    flips, valid_cost_changes, valid_hash_changes = [], [], []
    for index in sorted({row['instance_index'] for row in rows}):
        group = [row for row in rows if row['instance_index'] == index]
        if len({row['valid'] for row in group}) > 1:
            flips.append(index)
        good = [row for row in group if row['valid']]
        if len({row['cost'] for row in good}) > 1:
            valid_cost_changes.append(index)
        if len({row['solution_sha256'] for row in good}) > 1:
            valid_hash_changes.append(index)
    return dict(normalized=score['normalized'], valid_rows=score['valid_instances'],
                total_rows=score['total_instances'], repeat_normalized=values,
                repeat_range=max(values)-min(values),
                sample_stddev=statistics.stdev(values),
                cpu_range=[min(cpus), max(cpus)] if cpus else None,
                validity_flip_instances=flips, valid_cost_change_instances=valid_cost_changes,
                valid_solution_hash_change_instances=valid_hash_changes,
                errors=dict(Counter(row['error'] for row in rows if not row['valid'])))


def build(study, qualification):
    manifest, analysis, hidden, seal = (read(study/name) for name in
                                      ('manifest.json', 'analysis.json', 'hidden.json', 'seal.json'))
    q = read(qualification)
    for name in ('manifest', 'seal', 'hidden', 'analysis'):
        if q['evidence'][name+'_sha256'] != sha(study/(name+'.json')):
            raise ValueError('qualification evidence binding mismatch: '+name)
    for relative, digest in seal['files'].items():
        if sha(study/relative) != digest:
            raise ValueError('live seal mismatch: '+relative)
    # The full qualification helper already checked every hidden journal hash;
    # bind this report to that exact qualification and the hidden completion seal.
    evidence = dict(q['evidence'], qualification_sha256=sha(qualification),
                    hidden_seal_sha256=sha(study/'hidden-seal.json'),
                    report_builder_sha256=sha(Path(__file__)))
    blocks = {block['id']: block for block in manifest['blocks']}
    sources, reasons, endpoints, source_hidden, source_public = {}, defaultdict(set), {}, {}, {}
    for key, result in sorted(analysis['trials'].items()):
        trial_path = study/'trials'/(key+'.json')
        trial = read(trial_path)
        block = blocks[trial['block_id']]
        for candidate in trial['candidates']:
            digest = candidate['sha256']
            identity = (trial['family'], trial['pack'], digest)
            if hashlib.sha256(candidate['source'].encode()).hexdigest() != digest:
                raise ValueError('source hash mismatch')
            source_ref = dict(trial_file=str(trial_path.relative_to(study)),
                              trial_file_sha256=sha(trial_path), candidate_id=candidate['id'])
            if identity not in sources:
                sources[identity] = dict(family=trial['family'], pack=trial['pack'],
                    source_sha256=digest, block_id=block['id'], source_reference=source_ref,
                    all_candidate_references=[])
            sources[identity]['all_candidate_references'].append(dict(trial=key, candidate_id=candidate['id']))
            public = candidate['public']
            score = hidden['trials'][key]['candidates'][candidate['id']]
            if identity in source_hidden and score != source_hidden[identity]:
                raise ValueError('hidden cached score mismatch')
            source_hidden[identity] = score
            source_public.setdefault(identity, {})[candidate['id']] = public
            fully_public_valid = public['valid_instances'] == public['total_instances']
            if fully_public_valid and score['valid_instances'] < score['total_instances']:
                reasons[identity].add('public_valid_hidden_invalid')
            if fully_public_valid and public['normalized'] == result['public']:
                reasons[identity].add('best_public_tie')
            if candidate['id'] == trial['selected']:
                reasons[identity].add('selected_endpoint')
                endpoints[key] = dict(family=trial['family'], arm=trial['arm'],
                    candidate_id=candidate['id'], source_sha256=digest,
                    public=summarize(public), hidden=summarize(score),
                    selector_regret=result['selector_regret'],
                    branch_elapsed_seconds=result['branch_elapsed_seconds'],
                    prefix_elapsed_seconds=result.get('prefix_elapsed_seconds'),
                    elapsed_seconds=result['elapsed_seconds'])
    variability = {}
    for split, records in q['noise_exceptions'].items():
        rows = []
        for record in records:
            identity = (record['family'], record['pack'], record['source_sha256'])
            reasons[identity].add('repeat_variation')
            scores = [source_hidden[identity]] if split == 'hidden' else list(source_public[identity].values())
            details = [summarize(score) for score in scores]
            rows.append(dict(record, observed_evaluations=details,
                             source_reference=sources[identity]['source_reference']))
        variability[split] = rows
    focus = {key: row for key, row in endpoints.items() if row['arm'] in ('I', 'C_F', 'P_F')}
    target_key = 'b-e4b7a4ed895a0da5ef835156/P_D'
    target = endpoints[target_key]
    target_identity = ('vertex_cover', 'development-pack-0', target['source_sha256'])
    target_trial = read(study/'trials'/(target_key+'.json'))
    target_candidate = next(c for c in target_trial['candidates'] if c['id'] == target['candidate_id'])
    target['hidden_rows'] = source_hidden[target_identity]['instances']
    target['source_static_observation'] = {
        'imports': ['random', 'heapq'], 'uses_own_seeded_random': True,
        'clock_based_stopping': False,
        'fixed_search_calls': ['search(4451, 50, 200)', 'search(23173, 20, 80)'],
        'interpretation': 'Recorded rejections are CPU-budget failures, not invalid-solution errors; validation is skipped for over-budget runs, so semantic validity of their returned solutions was not established.'}
    assert 'import random, heapq' in target_candidate['source']
    assert 'best=search(4451,50,200)' in target_candidate['source']
    assert 'extra=search(23173,20,80)' in target_candidate['source']
    illustrative = []
    for label, sd in [('selected_P_D_vertex_cover', target['hidden']['sample_stddev']),
                      ('largest_hidden_repeat_sd', max(x['normalized_sample_stddev'] for x in q['noise_exceptions']['hidden']))]:
        illustrative.append(dict(source=label, estimated_single_pass_sd=sd,
            nominal_standard_error_at_repetitions={str(n):sd/math.sqrt(n) for n in (2, 20, 100, 1000)},
            repetitions_for_nominal_SE_below_0_01=math.floor((sd/.01)**2)+1,
            repetitions_for_nominal_SE_below_0_0025=math.floor((sd/.0025)**2)+1))
    diagnostic_id = 'proposed-evaluator-noise-development-2026-10-12'
    cohort = []
    for identity in sorted(reasons):
        row = dict(sources[identity], selection_reasons=sorted(reasons[identity]))
        block = blocks[row['block_id']]
        hid = source_hidden[identity]
        pub = next(iter(source_public[identity].values()))
        row['public_pack'] = dict(instance_count=len(block['public']['instances']),
            instance_digest=block['public']['digest'], reference=block['public']['reference'],
            manifest_pointer='/blocks/'+str(manifest['blocks'].index(block))+'/public/instances')
        row['hidden_pack'] = block['hidden']
        row['recorded_seeds'] = {split:[dict(instance_index=x['instance_index'],
            original_repetition=x['repetition'], seed=x['seed']) for x in score['instances']]
            for split, score in (('public',pub),('hidden',hid))}
        cohort.append(row)
    proposed = dict(schema='shared-record-proposed-evaluator-noise-diagnostic.v1',
        status='proposed_not_preregistered_not_executed', diagnostic_id=diagnostic_id,
        evidence=evidence,
        purpose='Diagnose CPU-boundary instability without replacing original outcomes or changing the D/F selection. This proposal is not an acceptance criterion and is not scheduled as part of this pilot.',
        cohort_rule='Union of every selected endpoint, every fully public-valid source tied for an arm public minimum, every fully public-valid but hidden-invalid source, and every source with recorded repeat variation. Deduplicate by family/pack/source hash.',
        unique_sources=len(cohort), cohort=cohort,
        schedule=dict(complete_passes=20, solve_executions=len(cohort)*8*20,
            source_order='For pass j=0..19 sort by SHA256 of compact sorted-key JSON [diagnostic_id,j,family,pack,source_sha256].',
            within_source_order='public then hidden; ascending instance_index',
            seed_schedule='Pass j uses exactly the archived seed for original_repetition=j%2 on that split/instance. This repeats each original seed ten times.',
            schedule_rationale='Ten fresh pairs of original-seed repetitions describe two-repeat variation and timing drift; twenty passes are a diagnostic sample, not a claim of adequate qualification precision.',
            process='One fresh sandboxed process per source/instance; serial execution; same archived interpreter and scoring/problem module bytes.',
            limits=manifest['evaluation'], runtime=hidden['runtime'],
            observations='Retain every cost, validity, error, CPU measurement and solution hash. Record UTC start/end for every execution and host power/sleep state. Do not substitute interrupted rows.',
            output_isolation='New diagnostic directory and new pending/complete journal; preserve original study, hidden journal, analysis, selection and billing byte-for-byte. No automatic rerun of incomplete rows.',
            analysis='Report all source/split/seed distributions, CPU-threshold crossing rates, ten fresh two-pass means, and the all-20-pass mean. Original results remain primary development data. No best-run selection or cohort replacement.',
            stop_policy='An incomplete execution remains incomplete and stops automatic diagnostic continuation; explicit recovery requires a separately named continuation with original evidence retained.'),
        execution_scheduled=False, campaign_allocation_created=False)
    report = dict(schema='shared-record-development-readiness-review.v1',
        status='stop_before_scored_freeze', evidence=evidence,
        original_requirement='docs/shared-record-information-policy-plan.md:213-218: qualify selector regret/ties on development; choose repetitions so unchanged-candidate evaluator noise is small relative to 0.01 before freeze.',
        policy_selection=q['policy_selection'], prefix_summary=q['prefix_summary'],
        noise_summary=q['noise_summary'], noise_exceptions=variability,
        selected_endpoints=endpoints, chosen_F_comparisons=dict(endpoint_references=len(focus),
            unique_sources=len({x['source_sha256'] for x in focus.values()}), endpoints=focus,
            all_observed_repeats_identical=all(x['hidden']['repeat_range']==0 for x in focus.values()),
            all_hidden_valid=all(x['hidden']['valid_rows']==x['hidden']['total_rows'] for x in focus.values()),
            boundary='Two repeats per frozen source/pack do not establish low failure probability. Shared C/P outputs are one evaluation, not independent stability evidence.'),
        P_D_vertex_cover=target,
        repetition_scale=dict(illustrations=illustrative,
            warning='Illustrative SE=observed two-repeat sample SD/sqrt(R), assuming independent stationary executions. These assumptions and sparse SD estimates are not established. 0.0025 here only illustrates one quarter of the useful-effect scale; it is not a new pass threshold or recommended R.'),
        recommendation='Stop the current paid pilot before scored allocation or calls. Keep the frozen F choice as descriptive development selection. Repetition alone is not a credible immediate fix for CPU-boundary instability at this useful-effect scale. A later separately frozen study should qualify a revised task/evaluator runtime procedure and recalibrate it with new identities; preserve all current data.',
        proposed_diagnostic_sha256=objsha(proposed), proposed_diagnostic_executed=False,
        source_scope='Archive-only review. No API calls, evaluator execution, campaign mutation or source repair.',
        host_sleep_boundary='One previously diagnosed maxcut source spans host sleep. It is neither a selected endpoint nor a noisy source. Raw hidden elapsed UTC is not active evaluator runtime; source-specific evidence remains in hidden-host-pause-diagnostic.json.')
    return report, proposed


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('study',type=Path)
    parser.add_argument('qualification',type=Path)
    parser.add_argument('output',type=Path)
    args=parser.parse_args()
    if args.output.resolve().is_relative_to(args.study.resolve()):
        raise ValueError('output must be outside sealed study')
    report, proposed=build(args.study,args.qualification)
    for name,value in [('qualification-development-readiness.json',report),
                       ('proposed-evaluator-noise-diagnostic.json',proposed)]:
        with (args.output/name).open('x') as stream:
            stream.write(json.dumps(value,indent=2,sort_keys=True,allow_nan=False)+'\n')
    print(json.dumps({'status':report['status'],'diagnostic_status':proposed['status'],
                      'proposed_sources':proposed['unique_sources'],
                      'proposed_solve_executions':proposed['schedule']['solve_executions']}))
