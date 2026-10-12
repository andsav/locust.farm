#!/usr/bin/env python3
"""Audit a sealed information-policy study without importing its live harness.

Reads JSON and hashes sealed bytes only. It never executes candidates, imports
the runner, reads API credentials, performs network calls, changes study files,
or settles campaign funding. The campaign is a separately identified snapshot.

Usage:
  python3 audit_accounting.py STUDY --output REPORT.json
  python3 audit_accounting.py STUDY --campaign COPIED_CAMPAIGN.json --output REPORT.json

Reports mirror existing ledger semantics: each generation is billed once;
consuming C/P arms each attribute their shared prefix; pending and failed calls
retain reservations. An accounting_error prevents a passing accounting audit
and campaign accounting retains max(reservation, any known cost). Counter calls
are audited separately, using the campaign's documented no-separate-fee basis.
"""
import argparse
from collections import Counter
from datetime import datetime, timezone
from decimal import Decimal
import hashlib
import json
from pathlib import Path


SCHEMA = 'shared-record-independent-accounting-audit.v1'
TOLERANCE = Decimal('0.000000000001')  # Same dollar comparison tolerance as Luna's ledger.
INPUT_FIELDS = ('model', 'instructions', 'input', 'tools', 'reasoning')
PREFIX_FRACTIONS = {'early': Decimal('.25'), 'delayed': Decimal('.5')}


def decimal(value):
    if isinstance(value, bool):
        raise ValueError('boolean is not a numeric accounting value')
    result = Decimal(str(value))
    if not result.is_finite() or result < 0:
        raise ValueError('accounting values must be finite and nonnegative')
    return result


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def effective_rates(config, input_tokens):
    rates = {key: decimal(value) for key, value in config['rates'].items()}
    tier = config.get('long_context_pricing')
    if tier and input_tokens > tier['threshold_tokens']:
        for key in ('input', 'cached_input', 'cache_write'):
            rates[key] *= decimal(tier['input_multiplier'])
        rates['output'] *= decimal(tier['output_multiplier'])
    return rates


def charge_at(entry, when=None):
    """Exactly the existing runtime/analyzer's dependency charge at a cut."""
    if when is not None and entry['reserved_at'] > when:
        return Decimal(0)
    settled = entry.get('settled_at')
    if entry.get('cost') is not None and settled is not None and (when is None or settled <= when):
        return decimal(entry['cost'])
    return decimal(entry['reserved'])


def campaign_charge(entry):
    """The campaign retains failed reserves and never releases accounting errors."""
    if entry['status'] == 'complete':
        return decimal(entry['cost'])
    if entry['status'] == 'accounting_error':
        return max(decimal(entry['reserved']), decimal(entry.get('cost', 0)))
    return decimal(entry['reserved'])


def maximum_committed(entries):
    """Maximum commitment immediately after any dispatch, before future refunds."""
    return max((sum((charge_at(entry, event['reserved_at']) for entry in entries), Decimal(0))
                for event in entries), default=Decimal(0))


def spending(entries):
    known = sum((decimal(entry['cost']) for entry in entries
                 if entry.get('cost') is not None and entry.get('settled_at') is not None), Decimal(0))
    uncertain = sum((decimal(entry['reserved']) for entry in entries
                     if entry.get('cost') is None or entry.get('settled_at') is None), Decimal(0))
    return dict(requests=len(entries), known_usage_cost_usd=str(known),
                unknown_reservation_usd=str(uncertain), attributed_cost_usd=str(known + uncertain),
                maximum_committed_at_dispatch_usd=str(maximum_committed(entries)))


class Audit:
    def __init__(self, study, campaign_path=None):
        self.study = Path(study).resolve()
        self.campaign_path = None if campaign_path is None else Path(campaign_path).resolve()
        self.checks, self.failures, self.warnings = Counter(), [], []
        self.raw = {}
        self.exact_matches = 0
        self.usage_totals = Counter()
        self.recomputed = Decimal(0)
        self.max_cost_difference = self.max_reservation_difference = Decimal(0)

    def check(self, name, okay, context=None):
        self.checks[name] += 1
        if not okay:
            self.failures.append(dict(check=name, context=context))

    def warn(self, name, context=None):
        self.warnings.append(dict(note=name, context=context))

    def read(self, name):
        path = (self.study / name).resolve()
        if not path.is_relative_to(self.study):
            raise ValueError('archive path escapes study')
        self.raw[name] = path.read_bytes()
        return json.loads(self.raw[name])

    def entries_for(self, request_ids, context):
        self.check('request_dependencies_unique', len(request_ids) == len(set(request_ids)), context)
        missing = set(request_ids) - self.by_id.keys()
        self.check('request_dependencies_exist', not missing, dict(scope=context, missing=sorted(missing)))
        return [self.by_id[ident] for ident in dict.fromkeys(request_ids) if ident in self.by_id]

    def inspect_count(self, count):
        ident = count['id']
        status = count['status']
        self.check('count_status', status in ('complete', 'pending', 'failed', 'accounting_error'), ident)
        self.check('count_endpoint', count['endpoint'] == 'responses/input_tokens', ident)
        if status == 'complete':
            response = count['response']
            self.check('count_response_shape', response.get('object') == 'response.input_tokens'
                       and type(response.get('input_tokens')) is int and response['input_tokens'] >= 0, ident)
            self.check('count_timestamp_order', count['started_at'] <= count['settled_at'], ident)
        elif status == 'accounting_error':
            self.check('count_accounting_qualification', False, ident)

    def inspect_request(self, entry):
        ident, status = entry['id'], entry['status']
        self.check('generation_status', status in ('complete', 'pending', 'failed', 'accounting_error'), ident)
        self.check('generation_accounting_qualification', status != 'accounting_error', ident)
        cfg = self.configs[entry['model']]
        payload = entry['payload']
        self.check('requested_model', payload['model'] == cfg['model'] == entry['model'], ident)
        self.check('requested_service_tier', payload['service_tier'] == cfg['service_tier']
                   == entry['service_tier'] == 'default', ident)
        self.check('frozen_rates_and_tier', entry['rates'] == cfg['rates']
                   and entry.get('long_context_pricing') == cfg.get('long_context_pricing'), ident)
        self.check('frozen_input_accounting', entry['input_accounting'] == cfg['input_accounting'], ident)
        if entry.get('settled_at') is not None:
            self.check('request_timestamp_order', entry['reserved_at'] <= entry['settled_at'], ident)
        incoming_bound = entry['input_bound']
        maximum = entry['max_output_tokens']
        self.check('request_output_limit_matches_payload', maximum == payload['max_output_tokens'], ident)
        self.check('documented_input_bound_limit', 0 <= incoming_bound <= cfg['max_input_tokens'], ident)
        self.check('documented_output_upper_limit', 0 < maximum <= cfg['max_output_tokens'], ident)
        if cfg['input_accounting']['method'] != 'context_bound':
            self.check('counted_context_room', incoming_bound + maximum <= cfg['context_tokens'], ident)
        # The initial sealed calibration's rejected one-token request is retained;
        # its unknown reservation is valid accounting, not a successful API call.
        if maximum < 16:
            self.check('completed_request_meets_provider_minimum', status != 'complete', ident)
            self.warn('request_below_current_provider_minimum_retained_as_failed_observation', ident)

        count = None
        if cfg['input_accounting']['method'] == 'provider_count':
            count = self.by_count.get(entry.get('input_count_id'))
            self.check('generation_has_counter', count is not None, ident)
            if count is not None:
                self.linked_counts.append(count['id'])
                self.check('generation_counter_completed', count['status'] == 'complete', ident)
                self.check('count_session_label_phase', all(count[key] == entry[key]
                           for key in ('session_id', 'label', 'phase')), ident)
                self.check('count_precedes_generation', count['settled_at'] <= entry['reserved_at'], ident)
                self.check('exact_count_payload_projection', count['payload'] ==
                           {key: payload[key] for key in INPUT_FIELDS if key in payload}, ident)
                self.check('reservation_input_equals_provider_count', incoming_bound == count['response']['input_tokens'], ident)

        reserved_rates = effective_rates(cfg, incoming_bound)
        expected_reserve = (decimal(incoming_bound) * max(reserved_rates[key] for key in
                           ('input', 'cached_input', 'cache_write')) + decimal(maximum) * reserved_rates['output']) / Decimal(1000000)
        difference = abs(expected_reserve - decimal(entry['reserved']))
        self.max_reservation_difference = max(self.max_reservation_difference, difference)
        self.check('reservation_recomputed_at_highest_input_rate', difference <= TOLERANCE, ident)
        self.check('recorded_reservation_rates', all(decimal(entry['reservation_rates'][key]) == rate
                   for key, rate in reserved_rates.items()), ident)

        if status in ('pending', 'failed'):
            self.check('uncertain_request_has_no_settled_cost', entry.get('cost') is None, ident)
            return
        if status == 'complete':
            self.check('completed_request_has_cost_and_settlement', entry.get('cost') is not None
                       and entry.get('settled_at') is not None, ident)
        if 'usage' not in entry or 'response' not in entry:
            self.check('settled_usage_available', False, ident)
            return
        usage, response = entry['usage'], entry['response']
        self.check('usage_matches_response', usage == response['usage'], ident)
        self.check('requested_returned_model', response.get('model') == entry.get('returned_model') == cfg['model'], ident)
        if entry.get('require_service_tier') or 'service_tier' in response:
            self.check('requested_returned_service_tier', response.get('service_tier')
                       == entry.get('returned_service_tier') == cfg['service_tier'], ident)
        incoming, outgoing = usage['input_tokens'], usage['output_tokens']
        details = usage.get('input_tokens_details', {})
        for field, rate in (('cached_tokens', 'cached_input'), ('cache_write_tokens', 'cache_write')):
            self.check('required_cache_usage_category', cfg['rates'][rate] == cfg['rates']['input']
                       or field in details, dict(request=ident, category=field))
        cached, written = details.get('cached_tokens', 0), details.get('cache_write_tokens', 0)
        self.check('nonnegative_integer_usage', all(type(value) is int and value >= 0
                   for value in (incoming, outgoing, cached, written)), ident)
        self.check('exclusive_input_usage_categories', cached + written <= incoming, ident)
        if 'total_tokens' in usage:
            self.check('total_usage_sum', usage['total_tokens'] == incoming + outgoing, ident)
        self.check('actual_input_within_qualified_bound', incoming <= incoming_bound, ident)
        self.check('generated_output_within_request', outgoing <= maximum, ident)
        if count is not None:
            exact = count['response']['input_tokens'] == incoming
            self.check('exact_count_matches_usage', exact, ident)
            self.exact_matches += exact
        rates = effective_rates(cfg, incoming)
        expected = (decimal(incoming - cached - written) * rates['input'] + decimal(cached) * rates['cached_input']
                    + decimal(written) * rates['cache_write'] + decimal(outgoing) * rates['output']) / Decimal(1000000)
        self.recomputed += expected
        if entry.get('cost') is not None:
            difference = abs(expected - decimal(entry['cost']))
            self.max_cost_difference = max(self.max_cost_difference, difference)
            self.check('cost_recomputed_from_categories', difference <= TOLERANCE, ident)
            self.check('cost_within_reservation', decimal(entry['cost']) <= decimal(entry['reserved']) + TOLERANCE, ident)
        self.check('recorded_charged_rates', all(decimal(entry.get('charged_rates', {}).get(key, -1)) == rate
                   for key, rate in rates.items()), ident)
        self.usage_totals.update(input_tokens=incoming, ordinary_input_tokens=incoming - cached - written,
                                 cached_tokens=cached, cache_write_tokens=written, output_tokens=outgoing)

    def inspect_agent(self, agent, ceiling, context):
        selected = self.entries_for(agent['request_ids'], context)
        totals = spending(selected)
        self.check('agent_spending_matches_ledger', abs(decimal(agent['spent_usd'])
                   - decimal(totals['attributed_cost_usd'])) <= TOLERANCE, context)
        self.check('session_actual_and_dispatch_ceiling', max(decimal(totals['attributed_cost_usd']),
                   decimal(totals['maximum_committed_at_dispatch_usd'])) <= ceiling + TOLERANCE, context)
        for request in agent['requests']:
            entry = self.by_id.get(request['id'])
            self.check('session_request_exists_in_ledger', entry is not None, request['id'])
            if entry is None:
                continue
            self.check('session_request_matches_ledger', request['payload'] == entry['payload']
                       and request.get('response') == entry.get('response'), request['id'])
            if entry.get('input_count_id') in self.by_count:
                self.check('session_embedded_count_matches_audit', request.get('input_count')
                           == self.by_count[entry['input_count_id']], request['id'])
        return dict(label=agent['id'], source_session_ids=sorted({entry['session_id'] for entry in selected}),
                    ceiling_usd=str(ceiling), **totals)

    def inspect_trials(self):
        self.trials, self.prefixes = {}, {}
        self.agent_rows, self.arm_rows, self.prefix_rows = [], [], []
        attribution = Counter()
        request_blocks = {}
        expected_paths = []
        budget = decimal(self.manifest['budget']['task_arm_usd'])
        for block in self.manifest['blocks']:
            for arm, definition in self.manifest['arms'].items():
                name = f'trials/{block["id"]}/{arm}.json'
                expected_paths.append(name)
                trial = self.read(name)
                self.trials[(block['id'], arm)] = trial
                self.check('trial_final_and_sealed', trial['final'] is True and name in self.seal['files'], name)
                self.check('trial_manifest_binding', trial['manifest_sha256'] == self.seal['manifest_sha256'], name)
                selected = self.entries_for(trial['request_ids'], name)
                attribution.update(set(trial['request_ids']))
                for entry in selected:
                    previous = request_blocks.setdefault(entry['id'], block['id'])
                    self.check('no_request_reuse_across_blocks', previous == block['id'], entry['id'])
                totals = spending(selected)
                self.check('task_arm_actual_and_dispatch_ceiling', max(decimal(totals['attributed_cost_usd']),
                           decimal(totals['maximum_committed_at_dispatch_usd'])) <= budget + TOLERANCE, name)
                self.arm_rows.append(dict(block=block['id'], family=block['family'], arm=arm,
                                          ceiling_usd=str(budget), **totals))
                per_worker = budget / (1 if definition['kind'] in ('S', 'R') else 3)
                for agent in trial['agents']:
                    self.check('session_declared_allowance', abs(decimal(agent['allowance_usd'])
                               - per_worker) <= TOLERANCE, agent['id'])
                    self.agent_rows.append(self.inspect_agent(agent, per_worker, agent['id']))
        actual_paths = {str(path.relative_to(self.study)) for path in self.study.glob('trials/*/*.json')}
        self.check('assigned_trial_set', actual_paths == set(expected_paths))
        for path in sorted((self.study / 'prefixes').glob('*.json')):
            name = str(path.relative_to(self.study))
            prefix = self.read(name)
            self.prefixes[(prefix['block_id'], prefix['arm'])] = prefix
            self.check('prefix_in_seal', name in self.seal['files'], name)
            fraction = PREFIX_FRACTIONS[prefix['arm']]
            for agent in prefix['agents']:
                self.prefix_rows.append(self.inspect_agent(agent, budget / 3 * fraction, agent['id']))
        for (block_id, arm), trial in self.trials.items():
            definition = self.manifest['arms'][arm]
            if definition['kind'] in ('C', 'P'):
                prefix_name = 'delayed' if definition['policy'] == 'D' else 'early'
                prefix = self.prefixes.get((block_id, prefix_name))
                # An interruption before prefix creation legitimately leaves no
                # prefix work or attribution for the still-assigned fallback arm.
                if prefix is None:
                    self.check('missing_prefix_has_no_branch_dispatch', not trial['request_ids'], trial['arm'])
                    continue
                self.check('full_group_prefix_attributed_to_consuming_arm', set(prefix['request_ids'])
                           .issubset(trial['request_ids']), f'{block_id}/{arm}')
                original = {agent['name']: agent for agent in prefix['agents']}
                for agent in trial['agents']:
                    source = original[agent['name']]
                    self.check('full_worker_prefix_attributed_to_continuation', set(source['request_ids'])
                               .issubset(agent['request_ids']), agent['id'])
                    self.check('prefix_request_archives_copied_exactly', agent['requests'][:len(source['requests'])]
                               == source['requests'], agent['id'])
            else:
                for (prefix_block, _), prefix in self.prefixes.items():
                    if block_id == prefix_block:
                        self.check('independent_solo_not_reusing_staged_prefix', not set(prefix['request_ids'])
                                   .intersection(trial['request_ids']), f'{block_id}/{arm}')
        covered = set(attribution)
        if set(self.by_id) - covered:
            self.warn('billed_requests_without_assigned_arm_reference', sorted(set(self.by_id) - covered))
        reused = {ident: times for ident, times in attribution.items() if times > 1}
        return dict(unique_billed_requests=len(self.entries), arm_dependency_references=sum(attribution.values()),
                    requests_attributed_to_multiple_arms=len(reused), consuming_arms_per_reused_request=reused,
                    total_attributed_across_arms_usd=str(sum((decimal(row['attributed_cost_usd'])
                                                             for row in self.arm_rows), Decimal(0))),
                    note='Common prefix requests are billed once in ledger.json and attributed in full to every consuming arm.')

    def inspect_campaign(self):
        receipt = self.manifest.get('campaign')
        if receipt is None:
            self.check('paid_study_has_campaign', self.manifest['stage'] == 'scripted')
            return None
        path = self.campaign_path or Path(receipt['path'])
        raw = path.read_bytes()
        campaign = json.loads(raw)
        header = {key: campaign[key] for key in ('schema', 'campaign_id', 'ceiling_usd', 'created_at')}
        self.check('campaign_identity_hash', sha256(canonical(header)) == campaign['campaign_sha256']
                   == receipt['campaign_sha256'])
        committed = Decimal(0)
        allocations = []
        for name, allocation in campaign['allocations'].items():
            binding = {key: allocation[key] for key in ('allocation_id', 'name', 'output', 'ceiling_usd', 'parameters', 'created_at')}
            self.check('campaign_allocation_binding', sha256(canonical(binding)) == allocation['binding_sha256'], name)
            status = allocation['status']
            self.check('campaign_allocation_status', status in ('allocated', 'settled', 'cancelled'), name)
            amount = (decimal(allocation['ceiling_usd']) if status == 'allocated' else Decimal(0)
                      if status == 'cancelled' else decimal(allocation['settlement']['committed_decimal']))
            committed += amount
            allocations.append(dict(name=name, status=status, committed_usd=str(amount)))
        allocation = campaign['allocations'][receipt['name']]
        self.check('study_campaign_allocation_identity', allocation['allocation_id'] == receipt['allocation_id']
                   and allocation['binding_sha256'] == receipt['binding_sha256']
                   and allocation['output'] == receipt['output'])
        self.check('executed_study_not_cancelled', allocation['status'] != 'cancelled')
        ceiling = decimal(allocation['ceiling_usd'])
        self.check('study_campaign_ceiling_match', ceiling == decimal(self.manifest['budget']['study_usd'])
                   == decimal(receipt['ceiling_usd']))
        charged = sum((campaign_charge(entry) for entry in self.entries), Decimal(0))
        self.check('campaign_study_commitment_within_allocation', charged <= ceiling + TOLERANCE)
        if allocation['status'] == 'settled':
            settlement = allocation['settlement']
            expected_proof = dict(manifest_sha256=self.seal['manifest_sha256'],
                                  seal_sha256=sha256(self.raw['seal.json']), ledger_sha256=sha256(self.raw['ledger.json']))
            self.check('campaign_settlement_binds_sealed_evidence', settlement['proof'] == expected_proof)
            self.check('campaign_settlement_matches_study', abs(decimal(settlement['committed_decimal']) - charged) <= TOLERANCE)
            settled_requests = {item['id']: item for item in settlement['requests']}
            self.check('campaign_settlement_request_set', set(settled_requests) == set(self.by_id))
            for entry in self.entries:
                saved = settled_requests.get(entry['id'])
                self.check('campaign_settlement_retains_each_charge', saved is not None and saved['status'] == entry['status']
                           and abs(decimal(saved['committed_decimal']) - campaign_charge(entry)) <= TOLERANCE, entry['id'])
        self.check('campaign_commitments_within_hard_ceiling', committed <= decimal(campaign['ceiling_usd']))
        self.check('campaign_not_halted', not campaign.get('halted'))
        return dict(path=str(path.resolve()), sha256=sha256(raw), ceiling_usd=str(campaign['ceiling_usd']),
                    committed_usd=str(committed), remaining_usd=str(decimal(campaign['ceiling_usd']) - committed),
                    study_allocation_status=allocation['status'], study_committed_usd=str(charged), allocations=allocations,
                    note='Snapshot of allocation commitments; other studies are not recursively re-audited here.')

    def run(self):
        self.manifest, freeze, self.seal = (self.read(name) for name in ('manifest.json', 'freeze.json', 'seal.json'))
        self.check('manifest_matches_freeze_and_seal', sha256(self.raw['manifest.json'])
                   == freeze['manifest_sha256'] == self.seal['manifest_sha256'])
        for name, expected in self.seal['files'].items():
            path = (self.study / name).resolve()
            self.check('sealed_file_path_stays_inside_study', path.is_relative_to(self.study), name)
            if path.is_relative_to(self.study):
                self.check('sealed_file_hash', sha256(path.read_bytes()) == expected, name)
        self.entries = self.read('ledger.json')
        self.counts = self.read('ledger.counts.json') if (self.study / 'ledger.counts.json').exists() else []
        self.check('generation_ledger_is_sealed', 'ledger.json' in self.seal['files'])
        if self.counts:
            self.check('count_audit_is_sealed', 'ledger.counts.json' in self.seal['files'])
        self.by_id = {entry['id']: entry for entry in self.entries}
        self.by_count = {entry['id']: entry for entry in self.counts}
        self.check('unique_generation_ids', len(self.by_id) == len(self.entries))
        self.check('unique_count_ids', len(self.by_count) == len(self.counts))
        self.configs = {config['model']: config for config in self.manifest['models'].values()}
        self.linked_counts = []
        for count in self.counts:
            self.inspect_count(count)
        for entry in self.entries:
            self.inspect_request(entry)
        self.check('count_not_reused_across_generations', len(self.linked_counts) == len(set(self.linked_counts)))
        attribution = self.inspect_trials()
        overall = spending(self.entries)
        ceiling = decimal(self.manifest['budget']['study_usd'])
        self.check('study_actual_and_dispatch_ceiling', max(decimal(overall['attributed_cost_usd']),
                   decimal(overall['maximum_committed_at_dispatch_usd'])) <= ceiling + TOLERANCE)
        phase_rows = []
        for phase in sorted({entry['phase'] for entry in self.entries}):
            rows = [entry for entry in self.entries if entry['phase'] == phase]
            totals = spending(rows)
            phase_ceiling = decimal(self.manifest['budget']['phase_usd'])
            self.check('phase_actual_and_dispatch_ceiling', max(decimal(totals['attributed_cost_usd']),
                       decimal(totals['maximum_committed_at_dispatch_usd'])) <= phase_ceiling + TOLERANCE, phase)
            phase_rows.append(dict(phase=phase, ceiling_usd=str(phase_ceiling), **totals))
        campaign = self.inspect_campaign()
        unpaired = []
        for count in self.counts:
            if count['id'] in set(self.linked_counts):
                continue
            agents = [agent for trial in list(self.trials.values()) + list(self.prefixes.values())
                      for agent in trial['agents'] if agent['id'] == count['label']]
            stops = [trace for agent in agents for trace in agent['trace']
                     if trace['event'] in ('budget_stop', 'request_failure')
                     and trace['time'] >= count.get('settled_at', count['started_at'])]
            unpaired.append(dict(id=count['id'], label=count['label'], status=count['status'],
                                 input_tokens=(count.get('response') or {}).get('input_tokens'),
                                 following_stop=stops[0] if stops else None))
            if not stops and count['status'] == 'complete':
                self.warn('completed_counter_without_generation_or_recorded_stop', count['id'])
        report = dict(schema=SCHEMA, status='pass' if not self.failures else 'fail',
                      audited_at=datetime.now(timezone.utc).isoformat(), study_id=self.manifest['study_id'],
                      study_path=str(self.study), helper_sha256=sha256(Path(__file__).read_bytes()),
                      scope='Read-only sealed accounting audit; no candidate execution or provider calls.',
                      monetary_basis='Recomputed provider token usage, not an independently reconciled invoice. Count calls audited separately.',
                      source_sha256={name: sha256(raw) for name, raw in self.raw.items()},
                      checks=dict(self.checks), failures=self.failures, warnings=self.warnings,
                      generation_requests=len(self.entries), provider_counts=len(self.counts),
                      generation_statuses=dict(Counter(entry['status'] for entry in self.entries)),
                      count_statuses=dict(Counter(count['status'] for count in self.counts)),
                      exact_input_matches=self.exact_matches, usage_totals=dict(self.usage_totals),
                      actual_usage_cost_usd=overall['known_usage_cost_usd'],
                      recomputed_usage_cost_usd=str(self.recomputed),
                      unknown_reservation_usd=overall['unknown_reservation_usd'],
                      maximum_study_committed_at_dispatch_usd=overall['maximum_committed_at_dispatch_usd'],
                      study_ceiling_usd=str(ceiling),
                      maximum_cost_recomputation_difference_usd=str(self.max_cost_difference),
                      maximum_reservation_recomputation_difference_usd=str(self.max_reservation_difference),
                      session_ceiling_checks=self.agent_rows, task_arm_ceiling_checks=self.arm_rows,
                      prefix_stage_ceiling_checks=self.prefix_rows, phase_ceiling_checks=phase_rows,
                      prefix_billing_and_attribution=attribution, counts_without_generation=unpaired,
                      unresolved_requests=[dict(id=entry['id'], label=entry['label'], status=entry['status'],
                                                reserved_usd=str(entry['reserved']), error=entry.get('error'))
                                           for entry in self.entries if entry['status'] != 'complete'],
                      campaign_snapshot=campaign)
        return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('study', type=Path)
    parser.add_argument('--campaign', type=Path, help='Use a copied campaign snapshot instead of its recorded absolute path')
    parser.add_argument('--output', type=Path, help='Write audit JSON outside the study directory')
    args = parser.parse_args(argv)
    if args.output is not None and args.output.resolve().is_relative_to(args.study.resolve()):
        parser.error('audit output must be outside the immutable study directory')
    audit = Audit(args.study, args.campaign)
    try:
        report = audit.run()
    except (KeyError, TypeError, ValueError, OSError) as error:
        report = dict(schema=SCHEMA, status='fail', study_path=str(args.study.resolve()),
                      failures=audit.failures + [dict(check='archive_or_schema_error', error=f'{type(error).__name__}: {error}')],
                      checks=dict(audit.checks), warnings=audit.warnings)
    rendered = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + '\n'
    if args.output is None:
        print(rendered, end='')
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered)
        print(json.dumps({key: report[key] for key in ('status', 'study_path')}, sort_keys=True))
    return 0 if report['status'] == 'pass' else 1


if __name__ == '__main__':
    raise SystemExit(main())
