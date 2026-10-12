#!/usr/bin/env python3
"""Offline group-study qualification from sealed live and hidden records only.

No API calls, evaluator execution, or harness imports. Scripted input proves
report structure only. Exact delivery/receipt/source auditing is separate.
"""
import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import statistics

from frozen_qualification import all_valid, noise, qualify
from private_stage_diagnostics import digest, read


def object_digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def balanced(manifest, rows, field):
    families = defaultdict(lambda: defaultdict(list))
    for block in manifest['blocks']:
        families[block['family']][block['pack']].append(field(rows[block['id']]))
    return statistics.fmean(statistics.fmean(statistics.fmean(v) for v in packs.values()) for packs in families.values())


def accounted(entries, when=None):
    rows = [row for row in entries if when is None or row['reserved_at'] <= when]
    known = [row for row in rows if row.get('cost') is not None and row.get('settled_at') is not None
             and (when is None or row['settled_at'] <= when)]
    uncertain = [row for row in rows if row not in known]
    actual = math.fsum(row['cost'] for row in known)
    reserved = math.fsum(row['reserved'] for row in uncertain)
    return dict(requests=len(rows), known_actual_cost=actual, unresolved_reservations=reserved,
                actual_cost=actual if not uncertain else None, committed_cost=actual + reserved,
                unknown_cost_requests=len(uncertain),
                output_tokens=sum(row['usage']['output_tokens'] for row in rows)
                if all(isinstance(row.get('usage', {}).get('output_tokens'), int) for row in rows) else None)


def prefix_agent(prefix, agent, ledger, allowance):
    valid = [candidate for candidate in agent['candidates'] if all_valid(candidate['public'])]
    own_ids = set(agent['request_ids']) | {id for id, row in ledger.items() if row['label'] == agent['id']}
    own_entries = [ledger[id] for id in own_ids]
    result = dict(agent=agent['name'], candidates=len(agent['candidates']), fully_public_valid_candidates=len(valid),
                  eligible_to_receive_peer_records=bool(valid), assigned_prefix_allowance=allowance,
                  failures=[agent['failed']] if agent.get('failed') else [],
                  prefix_spending=accounted(own_entries), first_valid=None)
    if valid:
        first = min(valid, key=lambda candidate: (candidate['time'], candidate['sequence'], candidate['id']))
        call_ids = {call for event in agent['trace'] if event['event'] == 'tool_results'
                    for call, value in event['results'] if value.get('candidate_id') == first['id']}
        generation = [request for request in agent['requests'] if any(
            item.get('call_id') in call_ids for item in request.get('response', {}).get('output', [])
            if item.get('type') == 'function_call')]
        if len(generation) != 1 and prefix['final']:
            raise ValueError('prefix first valid candidate lacks a unique generating response')
        response = generation[0]['response'] if len(generation) == 1 else None
        own = accounted(own_entries, first['time'])
        prefix_label = f'{prefix["block_id"]}/{prefix["arm"]}/'
        prefix_ids = set(prefix['request_ids']) | {id for id, row in ledger.items() if row['label'].startswith(prefix_label)}
        group = accounted([ledger[id] for id in prefix_ids], first['time'])
        result['first_valid'] = dict(candidate_id=first['id'], source_sha256=first['sha256'], available_at=first['time'],
            generating_request_id=generation[0]['id'] if response is not None else None,
            generating_response_id=response['id'] if response is not None else None,
            generating_response_output_tokens=response['usage']['output_tokens'] if response is not None else None,
            generation_link_status='observed' if response is not None else 'unavailable_in_partial_checkpoint',
            own_spending_at_availability=own, whole_prefix_spending_at_availability=group,
            committed_within_assigned_worker_slice=own['committed_cost'] <= allowance + 1e-12,
            scope='Observed under the actual frozen group prefix prompt and assigned prefix allowance. Whole-prefix cost includes all peers already dispatched, including pending reservations.')
    return result


def policy_selection(manifest, analyses):
    independent = [name for name, arm in manifest['arms'].items() if arm['kind'] == 'I']
    if len(independent) != 1:
        raise ValueError('development needs one independent baseline')
    profiles = {}
    for arm, definition in manifest['arms'].items():
        rows = {block['id']: analyses[f'{block["id"]}/{arm}'] for block in manifest['blocks']}
        baseline = {block['id']: analyses[f'{block["id"]}/{independent[0]}']['hidden'] for block in manifest['blocks']}
        gains = {id: dict(gain=baseline[id] - row['hidden']) for id, row in rows.items()}
        unknown = sum(row['spending']['unknown_cost_requests'] for row in rows.values())
        profiles[arm] = dict(definition, family_balanced_hidden=balanced(manifest, rows, lambda row: row['hidden']),
            family_balanced_gain_over_I=balanced(manifest, gains, lambda row: row['gain']),
            mean_known_usage_cost=statistics.fmean(row['spending']['known_usage_cost'] for row in rows.values()),
            mean_actual_cost=statistics.fmean(row['spending']['known_usage_cost'] for row in rows.values()) if not unknown else None,
            unknown_cost_requests=unknown)
    names = {policy: [name for name, arm in manifest['arms'].items() if arm['kind'] == 'P' and arm['policy'] == policy]
             for policy in ('D', 'F')}
    if any(len(arms) != 1 for arms in names.values()):
        raise ValueError('development needs one peer arm for each D/F policy')
    delayed, filtered = (profiles[names[policy][0]] for policy in ('D', 'F'))
    delta = delayed['family_balanced_gain_over_I'] - filtered['family_balanced_gain_over_I']
    selected, reason = None, 'near quality tie has unresolved actual cost'
    if abs(delta) >= .0025:
        selected, reason = ('D' if delta > 0 else 'F'), 'larger family-balanced gain over I'
    elif delayed['mean_actual_cost'] is not None and filtered['mean_actual_cost'] is not None:
        selected = 'D' if delayed['mean_actual_cost'] <= filtered['mean_actual_cost'] else 'F'
        reason = 'gain difference below .0025; lower actual mean cost; exact remaining tie D'
    return dict(selected_policy=selected, selected_arm=names[selected][0] if selected else None,
                D_minus_F_gain=delta, arms=profiles, reason=reason,
                synthetic_rule_replay=manifest['stage'] == 'scripted')


def qualify_development(study, delivery_path=None):
    study = Path(study).resolve()
    base = qualify(study)  # Verifies all live/hidden seals without rerunning evaluators.
    manifest, analysis, seal, hidden = (read(study / name) for name in
                                       ('manifest.json', 'analysis.json', 'seal.json', 'hidden.json'))
    if manifest['stage'] not in ('development', 'scripted'):
        raise ValueError('this diagnostic applies only to development or scripted structure checks')
    ledger = {row['id']: row for row in read(study / 'ledger.json')}
    public, hidden_records, reference_occurrences, prefixes, filtered = {}, {}, 0, {}, {}
    policies = {arm.get('policy') for arm in manifest['arms'].values()}

    def add_public(candidate, family, pack):
        identity = candidate['id']
        record = dict(family=family, pack=pack, source_sha256=candidate['sha256'], score=candidate['public'])
        if identity in public and public[identity] != record:
            raise ValueError('shared candidate ID has inconsistent public evidence')
        public[identity] = record

    for block in manifest['blocks']:
        for name, fraction in (('early', .25), ('delayed', .5)):
            if not (policies & {'E', 'F'} if name == 'early' else 'D' in policies):
                continue
            relative = f'prefixes/{block["id"]}-{name}.json'
            key = f'{block["id"]}/{name}'
            if relative not in seal['files']:
                prefixes[key] = dict(family=block['family'], timing=name, status='missing_prefix_archive', agents=[])
                continue
            prefix = read(study / relative)
            if prefix['manifest_sha256'] != seal['manifest_sha256']:
                raise ValueError('prefix belongs to another study')
            for candidate in prefix['candidates']:
                add_public(candidate, block['family'], block['pack'])
            agents = [prefix_agent(prefix, agent, ledger, manifest['budget']['task_arm_usd'] * fraction / 3)
                      for agent in prefix['agents']]
            prefix_label = f'{block["id"]}/{name}/'
            prefix_ids = set(prefix['request_ids']) | {id for id, row in ledger.items() if row['label'].startswith(prefix_label)}
            prefixes[key] = dict(family=block['family'], timing=name,
                status='recorded' if prefix['final'] else 'recorded_incomplete_prefix',
                group_allowance=manifest['budget']['task_arm_usd'] * fraction,
                group_spending=accounted([ledger[id] for id in prefix_ids]),
                worker_count=len(agents), workers_with_valid_candidate=sum(a['eligible_to_receive_peer_records'] for a in agents),
                error=prefix.get('error'), agents=agents)
        for arm, definition in manifest['arms'].items():
            key = f'{block["id"]}/{arm}'
            trial = read(study / 'trials' / f'{key}.json')
            for candidate in trial['candidates']:
                reference_occurrences += 1
                add_public(candidate, block['family'], block['pack'])
                identity = (block['family'], block['pack'], candidate['sha256'])
                score = hidden['trials'][key]['candidates'][candidate['id']]
                if identity in hidden_records and hidden_records[identity]['score'] != score:
                    raise ValueError('cached hidden source has inconsistent outcomes')
                hidden_records[identity] = dict(family=block['family'], pack=block['pack'],
                                               source_sha256=candidate['sha256'], score=score)
            if definition.get('policy') == 'F':
                deliveries = {d['recipient']: d for d in (trial.get('record') or {}).get('deliveries', [])}
                filtered[key] = {}
                for agent in trial['agents']:
                    delivery = deliveries.get(agent['name'])
                    status = delivery['status'] if delivery else 'not_recorded'
                    ids = delivery['delivered_candidate_ids'] if delivery else []
                    filtered[key][agent['name']] = dict(delivery_status=status,
                        delivered_candidate_count=len(ids), empty_delivered_packet=(not ids) if status == 'delivered' else None,
                        note='Archived delivery status only. Actual model exposure is reported by the independent delivery audit.')
    noise_summary, exceptions = {}, {}
    for split, records in (('public', public), ('hidden', hidden_records)):
        rows = [dict(family=r['family'], pack=r['pack'], source_sha256=r['source_sha256'], **noise(r['score']))
                for r in records.values()]
        ranges = [row['normalized_range'] for row in rows]
        noise_summary[split] = dict(evaluation_records=len(rows),
            repeated_evaluation_available=bool(rows) and all(row['repetitions'] >= 2 for row in rows),
            max_normalized_repeat_range=max(ranges, default=0),
            records_with_range_above_0_01=sum(value > .01 for value in ranges),
            **{name: sum(row[name] for row in rows) for name in ('evaluation_rows', 'invalid_evaluation_rows', 'instances_with_cost_variation', 'instances_with_validity_flips')})
        exceptions[split] = [row for row in rows if row['normalized_range'] or row['instances_with_validity_flips']]
    selection = policy_selection(manifest, analysis['trials'])
    if manifest['stage'] == 'development':
        saved = analysis['development_selection']
        for key in ('selected_policy', 'selected_arm', 'D_minus_F_gain'):
            if saved[key] != selection[key]:
                raise ValueError('independent policy rule differs from archived analysis: ' + key)
    external = None
    if delivery_path is not None:
        audit = read(delivery_path)
        if audit['manifest_sha256'] != seal['manifest_sha256'] or audit['seal_sha256'] != digest(study / 'seal.json'):
            raise ValueError('delivery audit belongs to another sealed study')
        if audit['schema'] != 'shared-record-independent-delivery-audit.v1':
            raise ValueError('unexpected delivery audit schema')
        external = dict(path=str(Path(delivery_path).resolve()), sha256=digest(delivery_path),
                        schema=audit['schema'], status=audit['status'],
                        failures=audit['failures'], warnings=audit['warnings'],
                        per_recipient_exposure={key: row['recipients'] for key, row in audit['trials'].items()})
    packet_rows = [row for trial in filtered.values() for row in trial.values()]
    prefix_summary = {}
    for timing in ('early', 'delayed'):
        rows = [row for row in prefixes.values() if row['timing'] == timing]
        agents = [agent for row in rows for agent in row['agents']]
        known_costs = [agent['first_valid']['own_spending_at_availability']['actual_cost'] for agent in agents
                       if agent['first_valid'] and agent['first_valid']['own_spending_at_availability']['actual_cost'] is not None]
        prefix_summary[timing] = dict(assigned_prefixes=len(rows),
            missing_archives=sum(row['status'] == 'missing_prefix_archive' for row in rows),
            incomplete_prefixes=sum(row['status'] == 'recorded_incomplete_prefix' for row in rows),
            observed_workers=len(agents), workers_with_valid_candidate=sum(a['eligible_to_receive_peer_records'] for a in agents),
            all_observed_workers_valid=bool(agents) and all(a['eligible_to_receive_peer_records'] for a in agents),
            first_valid_known_actual_cost_range=[min(known_costs), max(known_costs)] if known_costs else None,
            prefixes_with_no_valid_worker=sum(not any(a['eligible_to_receive_peer_records'] for a in row['agents']) for row in rows))
    return dict(schema='shared-record-development-qualification.v1', study_id=manifest['study_id'],
        synthetic=manifest['stage'] == 'scripted', evidence=base['evidence'], diagnostic_sha256=digest(__file__),
        policy_selection=selection, prefix_summary=prefix_summary, prefixes=prefixes, filtered_packets=filtered,
        filtered_packet_summary=dict(recipient_observations=len(packet_rows),
            delivered_empty=sum(row['empty_delivered_packet'] is True for row in packet_rows),
            delivered_nonempty=sum(row['empty_delivered_packet'] is False for row in packet_rows),
            incomplete_or_unrecorded=sum(row['empty_delivered_packet'] is None for row in packet_rows)),
        delivery_audit=external, delivery_exposure_status='external audit attached' if external else 'awaiting independent delivery audit',
        noise_summary=noise_summary, noise_exceptions=exceptions,
        evaluation_reuse=dict(candidate_references_across_assigned_arms=reference_occurrences,
            unique_public_candidate_evaluations=len(public), unique_hidden_family_pack_source_evaluations=len(hidden_records),
            note='A copied prefix candidate is one public evaluation identified by candidate ID; different candidate IDs remain separate even for identical sources/results. Hidden evaluation is deduplicated by family/pack/source. Workers, copied candidates and branches are not independent replicates.'),
        assigned_arm_selector_and_failures=base['trials'], summary=base['summary'],
        claim_limit='Known-family exploratory development. Repeated-score stability does not prove calibration. Public-validity eligibility, archived delivery, model exposure, source reads and semantic use are distinct. Shared-prefix arm attribution is not additional study billing; failures remain assigned observations.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('study', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--delivery-audit', type=Path)
    args = parser.parse_args()
    if args.output.resolve().is_relative_to(args.study.resolve()):
        raise ValueError('write qualification outside the sealed study directory')
    result = qualify_development(args.study, args.delivery_audit)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + '\n')
    print(json.dumps(dict(output=str(args.output.resolve()), synthetic=result['synthetic'],
                         policy_selection=result['policy_selection'], evaluation_reuse=result['evaluation_reuse'],
                         noise_summary=result['noise_summary'], filtered_packet_summary=result['filtered_packet_summary']), indent=2))
