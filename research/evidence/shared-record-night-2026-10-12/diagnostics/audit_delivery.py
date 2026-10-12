#!/usr/bin/env python3
"""Audit only completed shared-record delivery archives, without live-harness imports.

JSON/hash inspection only: no network, daemon calls, candidate evaluation, hidden
score reads, or study writes. Refuses an unsealed study. A passing audit verifies
recorded ancestry, policy, receipts and payload evidence; it does not prove that
a model attended to, adopted, or benefited from any delivered information.

Usage: python3 audit_delivery.py STUDY --output REPORT.json
"""
import argparse
import base64
from collections import Counter, defaultdict
import copy
import hashlib
import json
import math
from pathlib import Path

SCHEMA = 'shared-record-independent-delivery-audit.v1'
PACKET_SCHEMA = 'shared-record-packet-v1'
CHUNK_SCHEMA = 'shared-record-chunk-v1'
MAX_PAYLOAD_BYTES = 1024 * 1024
SEALED_OVERHEAD_BYTES = 45


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False, allow_nan=False).encode()


def digest(value):
    return hashlib.sha256(value).hexdigest()


def fully_valid(candidate):
    score = candidate['public']
    rows = score['instances']
    total = score['total_instances']
    if total != len(rows) or total <= 0:
        raise ValueError('invalid frozen public evaluation row count')
    if score['valid_instances'] != sum(bool(row['valid']) for row in rows):
        raise ValueError('invalid frozen public evaluation validity count')
    if not math.isfinite(score['normalized']):
        raise ValueError('nonfinite frozen public score')
    return score['valid_instances'] == total


def policy_metadata(candidates, policy):
    """Reconstruct the current E/D/F rule from frozen public records only."""
    if policy not in ('E', 'D', 'F'):
        raise ValueError('unknown policy')
    eligible, withheld, sequences = [], [], set()
    best = None
    for candidate in sorted(candidates, key=lambda row: (row['sequence'], row['id'])):
        if candidate['sequence'] in sequences:
            raise ValueError('duplicate author sequence')
        sequences.add(candidate['sequence'])
        valid = fully_valid(candidate)
        score = candidate['public']['normalized']
        before = best
        improving = valid and (best is None or score < best)
        if improving:
            best = score
        reason = ('failed_public_validation' if not valid else
                  'first_valid_baseline' if before is None else
                  'strict_public_improvement' if improving else 'non_improving')
        item = dict(schema=PACKET_SCHEMA, candidate_id=candidate['id'], parent_id=candidate.get('parent_id'),
                    author=candidate['agent'], sequence=candidate['sequence'], source_sha256=candidate['sha256'],
                    source_reference=dict(candidate_id=candidate['id'], sha256=candidate['sha256'],
                                          utf8_bytes=len(candidate['source'].encode())),
                    public=copy.deepcopy(candidate['public']), fully_public_valid=valid,
                    public_best_before=before, public_best_delta=None if before is None else before-score,
                    eligibility_reason=reason)
        if policy == 'F' and improving:
            withheld.append(dict(candidate_id=candidate['id'], author=candidate['agent'], reason=reason))
        else:
            eligible.append(item)
    return eligible, withheld


def document_chunks(branch, author, candidate_id, kind, document):
    """Reconstruct lossless chunk bytes at the source's actual payload boundary."""
    payload = canonical(document)
    envelope = dict(schema=CHUNK_SCHEMA, branch_id=branch, author=author, candidate_id=candidate_id,
                    kind=kind, sha256=digest(payload), encoding='base64-json-utf8', part=0, parts=1, data='')
    parts = 1
    while True:
        envelope.update(part=parts-1, parts=parts)
        capacity = (MAX_PAYLOAD_BYTES-SEALED_OVERHEAD_BYTES-len(canonical(envelope))) // 4 * 3
        if capacity <= 0:
            raise ValueError('chunk identity exceeds payload boundary')
        needed = max(1, (len(payload)+capacity-1)//capacity)
        if needed == parts:
            break
        parts = needed
    return [dict(envelope, part=part,
                 data=base64.b64encode(payload[part*capacity:(part+1)*capacity]).decode('ascii'))
            for part in range(parts)]


def contains_output(history, call_id, result):
    for item in history:
        if item.get('type') != 'function_call_output' or item.get('call_id') != call_id:
            continue
        try:
            if json.loads(item['output']) == result:
                return True
        except (KeyError, TypeError, json.JSONDecodeError):
            continue
    return False


class Audit:
    def __init__(self, study):
        self.study = Path(study).resolve()
        self.checks = Counter()
        self.failures, self.warnings = [], []
        self.hashes, self.prefixes, self.results = {}, {}, {}
        self.goals, self.principals = set(), set()
        self.published, self.inspected = defaultdict(list), defaultdict(list)
        self.created_goals, self.memberships, self.enrolled = set(), set(), set()

    def check(self, name, okay, context=None):
        self.checks[name] += 1
        if not okay:
            self.failures.append(dict(check=name, context=context))
        return bool(okay)

    def warn(self, name, context=None):
        self.warnings.append(dict(note=name, context=context))

    def raw(self, name):
        path = (self.study/name).resolve()
        if not path.is_relative_to(self.study):
            raise ValueError('archive path escapes study')
        content = path.read_bytes()
        self.hashes[name] = digest(content)
        return content

    def read(self, name):
        return json.loads(self.raw(name))

    def load(self):
        if not (self.study/'seal.json').is_file():
            raise ValueError('delivery audit requires a completed study seal; live study refused')
        self.seal = self.read('seal.json')
        self.manifest = self.read('manifest.json')
        self.manifest_hash = self.hashes['manifest.json']
        if self.seal['manifest_sha256'] != self.manifest_hash:
            raise ValueError('manifest differs from study seal')
        if self.read('freeze.json')['manifest_sha256'] != self.manifest_hash:
            raise ValueError('manifest differs from freeze receipt')
        required = {'freeze.json', 'ledger.json'} | {
            f'trials/{block["id"]}/{arm}.json' for block in self.manifest['blocks'] for arm in self.manifest['arms']}
        if not required.issubset(self.seal['files']):
            raise ValueError('seal omits assigned trials or accounting records')
        actual_trials = {str(path.relative_to(self.study)) for path in self.study.glob('trials/*/*.json')}
        if actual_trials != required-{'freeze.json', 'ledger.json'}:
            raise ValueError('trial files differ from frozen assignment')
        for name, expected in self.seal['files'].items():
            if digest(self.raw(name)) != expected:
                raise ValueError('sealed bytes changed: '+name)
        ledger = self.read('ledger.json')
        self.ledger = {row['id']: row for row in ledger}
        if len(self.ledger) != len(ledger):
            raise ValueError('duplicate request identity')
        self.events = []
        if 'daemon-events.jsonl' in self.seal['files']:
            self.events = [json.loads(line) for line in self.raw('daemon-events.jsonl').splitlines() if line]
        has_records = any(arm['kind'] in ('C', 'P') for arm in self.manifest['arms'].values())
        self.check('daemon_events_sealed', not has_records or bool(self.events))
        self.check('frozen_binary_event', not has_records or any(
            row.get('event') == 'binary' and row.get('sha256') == self.manifest['runtime']['binary_sha256']
            for row in self.events))
        self.check('daemon_start_and_ready_recorded', not has_records or
                   {'daemon_started', 'daemon_ready'}.issubset({row.get('event') for row in self.events}))
        for index, event in enumerate(self.events):
            response = event.get('response', {})
            if not response.get('ok'):
                continue
            result = response.get('result', {})
            if 'goal_created' in result:
                self.created_goals.add(result['goal_created']['goal'])
            if result.get('membership') == 'member':
                self.memberships.add((result['goal'], result['agent']))
            if 'agent_enrolled' in result:
                self.enrolled.add(result['agent_enrolled']['agent'])
            if event.get('operation') == 'contribution publish' and 'recorded' in result:
                self.published[(event['role'], result['recorded']['event'])].append((index, event))
            if event.get('operation') == 'contribution inspect' and 'contribution_inspected' in result:
                contribution = result['contribution_inspected']['contribution']
                self.inspected[(event['role'], contribution['view']['event'])].append((index, contribution))

    def candidate(self, candidate, author, context):
        source_hash = digest(candidate['source'].encode())
        self.check('candidate_source_identity', candidate['agent'] == author and
                   candidate['sha256'] == source_hash == candidate['public']['source_sha256'], context)
        fully_valid(candidate)
        self.check('live_candidate_has_no_hidden_score', 'hidden' not in candidate, context)

    def prefix(self, block, policy):
        kind = 'delayed' if policy == 'D' else 'early'
        key = f'{block["id"]}-{kind}'
        if key not in self.prefixes:
            name = f'prefixes/{key}.json'
            self.check('prefix_is_sealed', name in self.seal['files'], key)
            original = self.read(name)
            self.check('prefix_identity', original['final'] and original['kind'] == 'prefix'
                       and original['arm'] == kind and original['block_id'] == block['id']
                       and original['manifest_sha256'] == self.manifest_hash, key)
            self.check('prefix_candidate_inventory', original['candidates'] ==
                       [candidate for agent in original['agents'] for candidate in agent['candidates']], key)
            for agent in original['agents']:
                self.check('prefix_agent_identity', agent['id'] == f'{block["id"]}/{kind}/{agent["name"]}', key)
                self.check('prefix_request_inventory_exact', [row['id'] for row in agent['requests']] == agent['request_ids'], agent['id'])
                for request in agent['requests']:
                    entry = self.ledger[request['id']]
                    self.check('prefix_request_ledger_binding', entry['label'] == agent['id'] and request['payload'] == entry['payload'], request['id'])
                for candidate in agent['candidates']:
                    self.candidate(candidate, agent['name'], candidate['id'])
            self.prefixes[key] = original
        return self.prefixes[key]

    def ancestry(self, trial, original, agent, prior, context):
        count = len(prior['requests'])
        self.check('copied_prefix_requests_exact', agent['requests'][:count] == prior['requests'], context)
        self.check('copied_prefix_request_ids_exact', agent['request_ids'][:len(prior['request_ids'])] == prior['request_ids'], context)
        self.check('copied_prefix_trace_exact', agent['trace'][:len(prior['trace'])] == prior['trace'], context)
        self.check('copied_prefix_candidates_exact', agent['candidates'][:len(prior['candidates'])] == prior['candidates'], context)
        expected_ids = [row['id'] for row in prior['candidates']]
        self.check('prefix_candidate_ids_exact', agent['prefix_candidate_ids'] == expected_ids, context)
        own = agent['requests'][count:]
        for request in own:
            entry = self.ledger[request['id']]
            self.check('own_request_identity_and_payload', entry['label'] == agent['id'] and
                       request['payload'] == entry['payload'], request['id'])
            self.check('own_request_after_prefix', request['reserved_at'] >= original['finished'], request['id'])
        self.check('agent_request_inventory_exact', [row['id'] for row in agent['requests']] == agent['request_ids'], context)
        return own

    def document(self, record, author, candidate_id, kind, document, receipts, expected_receipts):
        context = f'{record["branch_id"]}/{candidate_id}/{kind}'
        reference = record['references'][candidate_id][kind]
        chunks = document_chunks(record['branch_id'], author, candidate_id, kind, document)
        self.check('document_reference_identity', reference['candidate_id'] == candidate_id and
                   reference['author'] == author and reference['kind'] == kind and
                   reference['sha256'] == digest(canonical(document)) and
                   reference['parts'] == len(chunks) == len(reference['receipt_ids']), context)
        identity = record['identity_map'][author]
        for ident, chunk in zip(reference['receipt_ids'], chunks):
            self.check('receipt_not_reused_across_documents', ident not in expected_receipts, ident)
            expected_receipts.add(ident)
            receipt = receipts[ident]
            text = canonical(chunk)
            self.check('publication_text_hash_and_size', receipt['text_sha256'] == digest(text) and
                       receipt['plaintext_bytes'] == len(text) and
                       receipt['sealed_bytes'] == len(text)+SEALED_OVERHEAD_BYTES <= MAX_PAYLOAD_BYTES, ident)
            self.check('publication_author_identity', receipt['agent'] == author and
                       receipt['principal'] == identity['principal'], ident)
            published = self.published[(identity['name'], ident)]
            self.check('daemon_publication_receipt', len(published) == 1 and
                       published[0][1]['response']['result'] == receipt['receipt'], ident)
            self.check('publication_within_freeze', record['freeze_started_at'] <= receipt['started_at']
                       <= receipt['finished_at'] <= record['freeze_finished_at'], ident)
            for (role, event_id), inspected in self.inspected.items():
                if event_id != ident:
                    continue
                self.check('inspector_belongs_to_branch', role in {v['name'] for v in record['identity_map'].values()}, ident)
                for index, value in inspected:
                    self.check('daemon_effective_author_and_event', value['view']['event'] == ident and
                               value['view']['standing'] == 'effective' and
                               value['view']['author'] == identity['principal'], ident)
                    self.check('daemon_inspected_exact_chunk', value['text'].encode() == text and
                               value['payload']['len'] == receipt['sealed_bytes'], ident)
                    self.check('inspection_after_publication', bool(published) and index > published[0][0], ident)
        return reference['receipt_ids']

    def source_tools(self, agent, prior, own, allowed, frozen_candidates, source_reads):
        context = agent['id']
        result_groups = [event for event in agent['trace'][len(prior['trace']):] if event['event'] == 'tool_results']
        call_groups = []
        for request in own:
            calls = []
            if request.get('status') not in ('completed', 'incomplete'):
                continue
            for item in request.get('response', {}).get('output', []):
                if item.get('type') != 'function_call':
                    continue
                try:
                    calls.append((item, json.loads(item['arguments'])))
                except (KeyError, TypeError, json.JSONDecodeError):
                    continue
            if calls:
                call_groups.append((request, calls))
        self.check('tool_result_groups_match_responses', len(call_groups) == len(result_groups), context)
        own_candidates = {row['id'] for row in prior['candidates']}
        read_operations, returns, denied, parents = [], [], [], []
        for (request, calls), group in zip(call_groups, result_groups):
            self.check('tool_call_result_identity', [item['call_id'] for item, _ in calls] ==
                       [ident for ident, _ in group['results']], request['id'])
            for (item, args), (_, value) in zip(calls, group['results']):
                if item['name'] == 'get_source':
                    candidate_id = args.get('candidate_id')
                    if 'error' in value:
                        denied.append(dict(request_id=request['id'], call_id=item['call_id'],
                                           candidate_id=candidate_id, error=value['error']))
                        if candidate_id in allowed:
                            read_operations.append(candidate_id)
                            self.warn('eligible_source_call_failed', dict(agent=context, candidate_id=candidate_id))
                        continue
                    self.check('returned_source_was_eligible', candidate_id in allowed, dict(agent=context, candidate_id=candidate_id))
                    candidate = frozen_candidates.get(candidate_id)
                    expected = None if candidate is None else dict(candidate_id=candidate_id,
                                                                   sha256=candidate['sha256'], source=candidate['source'])
                    self.check('full_source_return_exact', expected is not None and
                               {key:value.get(key) for key in expected} == expected,
                               dict(agent=context, request_id=request['id'], candidate_id=candidate_id))
                    read_operations.append(candidate_id)
                    submitted = [row['id'] for row in own if row['reserved_at'] >= group['time']
                                 and contains_output(row['payload']['input'], item['call_id'], value)]
                    responses = [row['id'] for row in own if row['id'] in submitted and row.get('response') is not None]
                    returns.append(dict(candidate_id=candidate_id, author=(candidate or {}).get('agent'),
                                        peer=(candidate or {}).get('agent') != agent['name'], request_id=request['id'],
                                        call_id=item['call_id'], source_sha256=value.get('sha256'),
                                        subsequent_payload_request_ids=submitted, subsequent_response_request_ids=responses))
                elif item['name'] == 'evaluate':
                    parent = args.get('parent_id')
                    if parent is not None and parent not in own_candidates:
                        if parent in allowed:
                            read_operations.append(parent)
                            parents.append(parent)
                        else:
                            self.check('ineligible_parent_rejected', 'error' in value, dict(agent=context, parent_id=parent))
                    candidate_id = value.get('candidate_id')
                    if candidate_id is not None:
                        own_candidates.add(candidate_id)
        self.check('source_reads_match_explicit_or_parent_operations', Counter(read_operations) ==
                   Counter(row['candidate_id'] for row in source_reads), context)
        return dict(full_source_returns=returns, denied_or_failed_source_calls=denied,
                    non_own_parent_ids_declared=parents,
                    peer_full_source_return_count=sum(row['peer'] for row in returns),
                    peer_source_submitted_count=sum(row['peer'] and bool(row['subsequent_payload_request_ids']) for row in returns))

    def trial(self, block, arm, settings):
        key = f'{block["id"]}/{arm}'
        trial = self.read(f'trials/{key}.json')
        self.check('trial_identity', trial['final'] and trial['manifest_sha256'] == self.manifest_hash and
                   trial['kind'] == settings['kind'] and trial['arm'] == arm and trial['block_id'] == block['id'] and
                   all(trial[field] == block[field] for field in ('family','pack','repetition','attempt','phase')), key)
        if settings['kind'] not in ('C', 'P'):
            self.check('unstaged_arm_has_no_record', trial['record'] is None, key)
            return
        policy = settings['policy']
        original = self.prefix(block, policy)
        prior = {agent['name']:agent for agent in original['agents']}
        agents = {agent['name']:agent for agent in trial['agents']}
        self.check('matched_worker_set', set(agents) == set(prior), key)
        self.check('matched_worker_order', [row['name'] for row in trial['agents']] == [row['name'] for row in original['agents']], key)
        self.check('trial_prefix_inventory_exact', trial['prefix_candidate_ids'] ==
                   [row['id'] for agent in original['agents'] for row in agent['candidates']], key)
        self.check('full_prefix_request_ancestry', set(original['request_ids']).issubset(trial['request_ids']), key)
        frozen_candidates = {row['id']:row for agent in original['agents'] for row in agent['candidates']}
        self.check('frozen_candidate_ids_unique', len(frozen_candidates) == len(original['candidates']), key)
        metadata, filtered = {}, {}
        for author, agent in prior.items():
            metadata[author], filtered[author] = policy_metadata(agent['candidates'], policy)
        expected_valid = {name:any(fully_valid(row) for row in agent['candidates']) for name,agent in prior.items()}
        record = trial['record']
        if not self.check('record_frozen', record is not None and record.get('state') == 'frozen', key):
            self.results[key] = dict(arm=arm, policy=policy, family=block['family'], record_complete=False, recipients={})
            return
        self.check('record_identity', record['schema'] == PACKET_SCHEMA and record['branch_id'] == key and record['policy'] == policy, key)
        self.check('record_policy_metadata_exact', record['candidates'] == metadata and record['filtered_out'] == filtered, key)
        self.check('own_valid_prefix_gate_exact', record['valid_prefix'] == expected_valid, key)
        self.check('branch_goal_unique', record['goal'] not in self.goals, key)
        self.goals.add(record['goal'])
        self.check('daemon_goal_creation_recorded', record['goal'] in self.created_goals, key)
        self.check('branch_identity_inventory', set(record['identity_map']) == set(prior), key)
        for name, identity in record['identity_map'].items():
            title = f'{self.manifest["study_id"]}/{key}'
            expected_name = 'sr-'+digest(canonical([title,name]))[:29]
            self.check('principal_identity_binding', identity['agent'] == name and identity['title'] == title
                       and identity['name'] == expected_name and identity['goal'] == record['goal'], key+'/'+name)
            self.check('principal_unique_to_branch', identity['principal'] not in self.principals, key+'/'+name)
            self.principals.add(identity['principal'])
            self.check('daemon_enrollment_and_membership', identity['principal'] in self.enrolled and
                       (record['goal'],identity['principal']) in self.memberships, key+'/'+name)
        receipts = {row['receipt']['recorded']['event']:row for row in record['receipts']}
        self.check('publication_receipt_ids_unique', len(receipts) == len(record['receipts']), key)
        selected_ids = {row['candidate_id'] for rows in metadata.values() for row in rows}
        self.check('reference_inventory_exact', set(record['references']) == selected_ids, key)
        expected_receipts = set()
        for author, rows in metadata.items():
            for item in rows:
                candidate_id = item['candidate_id']
                candidate = frozen_candidates[candidate_id]
                self.check('source_and_metadata_references', set(record['references'][candidate_id]) == {'source','metadata'}, candidate_id)
                self.document(record,author,candidate_id,'source',dict(candidate_id=candidate_id,
                              sha256=candidate['sha256'],source=candidate['source']),receipts,expected_receipts)
                self.document(record,author,candidate_id,'metadata',item,receipts,expected_receipts)
        self.check('all_receipts_belong_to_declared_documents', set(receipts) == expected_receipts, key)
        deliveries = {row['recipient']:row for row in record['deliveries']}
        self.check('one_delivery_per_recipient', len(deliveries) == len(record['deliveries']) and set(deliveries) == set(prior), key)
        inspect_demand = Counter()
        recipient_rows = {}
        for name, agent in agents.items():
            context = key+'/'+name
            self.check('continuation_agent_identity', agent['id'] == context, context)
            own = self.ancestry(trial,original,agent,prior[name],context)
            delivery = deliveries[name]
            peer_access = settings['kind'] == 'P' and expected_valid[name]
            donors = [name]+([author for author in sorted(prior) if author != name] if peer_access else [])
            eligible = [row['candidate_id'] for author in donors for row in metadata[author]]
            withheld = []
            for author in sorted(prior):
                if author in donors:
                    withheld.extend(filtered[author])
                else:
                    reason = 'recipient_has_no_valid_prefix' if settings['kind'] == 'P' else 'private_review'
                    withheld.extend(dict(candidate_id=row['id'],author=author,reason=reason) for row in prior[author]['candidates'])
            identity = record['identity_map'][name]
            self.check('delivery_recipient_identity', delivery['recipient_id'] == context and
                       delivery['principal'] == identity['principal'], context)
            self.check('delivery_eligibility_and_withheld_exact', delivery['peers_requested'] == (settings['kind']=='P') and
                       delivery['peer_access'] == peer_access and delivery['eligible_candidate_ids'] == eligible and
                       delivery['withheld'] == withheld, context)
            self.check('complete_delivery_exact', delivery['status']=='delivered' and delivery['delivered_candidate_ids']==eligible, context)
            wanted_receipts = [ident for candidate_id in eligible for ident in record['references'][candidate_id]['metadata']['receipt_ids']]
            self.check('metadata_delivery_receipts_exact', delivery['receipt_ids']==wanted_receipts, context)
            for ident in wanted_receipts:
                inspect_demand[(identity['name'],ident)] += 1
            self.check('delivery_after_frozen_prefix', original['finished'] <= record['freeze_started_at'] <=
                       record['freeze_finished_at'] <= delivery['started_at'] <= delivery['finished_at'], context)
            packet = dict(schema=PACKET_SCHEMA,policy=policy,candidates=[row for author in donors for row in metadata[author]])
            message = dict(role='user',content=self.manifest['prompts']['review']+'\nFrozen evidence:\n'+json.dumps(packet,sort_keys=True))
            expected_history = prior[name]['history']+[message]
            history = own[0]['payload']['input'] if own else agent['history']
            self.check('continuation_starts_from_exact_prefix_and_packet', history == expected_history, context)
            self.check('review_timestamp_matches_delivery', agent['review_at'] is not None and
                       agent['review_at'] >= delivery['finished_at'], context)
            dispatched = [row['id'] for row in own if message in row['payload']['input']]
            responses = [row['id'] for row in own if row['id'] in dispatched and row.get('response') is not None]
            reads = [row for row in record['source_reads'] if row['recipient']==name]
            for row in reads:
                candidate_id = row['candidate_id']
                self.check('source_read_requires_delivered_eligibility', candidate_id in eligible and
                           row['recipient_id']==context and row['principal']==identity['principal'], context)
                reference = record['references'][candidate_id]['source']
                self.check('source_read_complete_receipts', row['status']=='delivered' and row['receipt_ids']==reference['receipt_ids'], context)
                self.check('source_read_after_delivery', row['started_at'] >= delivery['finished_at'] and
                           row['finished_at'] >= row['started_at'], context)
                for ident in row['receipt_ids']:
                    inspect_demand[(identity['name'],ident)] += 1
            peer_ids = [ident for ident in delivery['delivered_candidate_ids'] if frozen_candidates.get(ident,{}).get('agent') != name]
            self.check('control_has_no_peer_delivery', settings['kind']!='C' or not peer_ids, context)
            self.check('invalid_prefix_has_no_peer_delivery', expected_valid[name] or not peer_ids, context)
            tools = self.source_tools(agent,prior[name],own,set(eligible),frozen_candidates,reads)
            recipient_rows[name] = dict(own_fully_public_valid_prefix=expected_valid[name],peer_access=peer_access,
                delivery_status=delivery['status'],eligible_candidate_ids=eligible,
                eligible_peer_candidate_ids=[ident for ident in eligible if frozen_candidates[ident]['agent'] != name],
                delivered_candidate_ids=delivery['delivered_candidate_ids'],
                delivered_peer_candidate_ids=peer_ids,withheld=withheld,packet_queued=True,
                packet_dispatched_request_ids=dispatched,packet_response_request_ids=responses,
                full_source_read_count=len(reads),full_source_read_candidate_ids=[row['candidate_id'] for row in reads],**tools)
        for role in [identity['name'] for identity in record['identity_map'].values()]:
            actual = Counter({key:len(rows) for key,rows in self.inspected.items() if key[0]==role})
            expected = Counter({key:count for key,count in inspect_demand.items() if key[0]==role})
            self.check('daemon_inspections_exactly_accounted', actual==expected, dict(branch=key,role=role))
        self.results[key] = dict(arm=arm,kind=settings['kind'],policy=policy,family=block['family'],
                                record_complete=True,goal=record['goal'],prefix_arm=original['arm'],
                                prefix_sha256=self.hashes[f'prefixes/{block["id"]}-{original["arm"]}.json'],
                                publication_receipts=len(receipts),recipients=recipient_rows)

    def matched(self):
        for block in self.manifest['blocks']:
            for policy in ('E','D','F'):
                arms = {settings['kind']:arm for arm,settings in self.manifest['arms'].items()
                        if settings.get('policy')==policy and settings['kind'] in ('C','P')}
                if set(arms) != {'C','P'}:
                    continue
                control = self.read(f'trials/{block["id"]}/{arms["C"]}.json')
                peer = self.read(f'trials/{block["id"]}/{arms["P"]}.json')
                original = self.prefix(block,policy)
                for left,right,prior in zip(control['agents'],peer['agents'],original['agents']):
                    context = f'{block["id"]}/{policy}/{prior["name"]}'
                    count = len(prior['requests'])
                    self.check('matched_C_P_common_request_ancestry', left['requests'][:count]==right['requests'][:count]==prior['requests'], context)
                    left_own,right_own = left['requests'][count:],right['requests'][count:]
                    if not left_own or not right_own:
                        self.warn('matched_first_dispatch_unavailable',context)
                        continue
                    before,after = copy.deepcopy(left_own[0]['payload']),copy.deepcopy(right_own[0]['payload'])
                    before['input'],after['input'] = before['input'][:-1],after['input'][:-1]
                    # Full peer packets change exact input count and thus the funded output maximum.
                    before.pop('max_output_tokens',None)
                    after.pop('max_output_tokens',None)
                    self.check('matched_C_P_first_dispatch_context',before==after,context)
                    if not any(fully_valid(row) for row in prior['candidates']):
                        self.check('ineligible_P_matches_C_packet',left_own[0]['payload']['input']==right_own[0]['payload']['input'],context)

    def run(self):
        self.load()
        for block in self.manifest['blocks']:
            for arm,settings in self.manifest['arms'].items():
                try:
                    self.trial(block,arm,settings)
                except (KeyError,TypeError,ValueError,IndexError) as error:
                    self.check('archive_structure_auditable',False,dict(trial=f'{block["id"]}/{arm}',error=str(error)))
        try:
            self.matched()
        except (KeyError,TypeError,ValueError,IndexError) as error:
            self.check('matched_structure_auditable',False,str(error))
        for name,expected in self.hashes.items():
            if digest((self.study/name).read_bytes()) != expected:
                raise ValueError('study changed during read-only audit: '+name)
        recipients = [row for trial in self.results.values() for row in trial['recipients'].values()]
        return dict(schema=SCHEMA,status='fail' if self.failures else 'pass',study_id=self.manifest['study_id'],
            study_path=str(self.study),synthetic=self.manifest['stage']=='scripted',
            manifest_sha256=self.manifest_hash,seal_sha256=self.hashes['seal.json'],
            diagnostic_sha256=digest(Path(__file__).read_bytes()),source_sha256=dict(sorted(self.hashes.items())),
            frozen_runtime_source_sha256=self.manifest['runtime']['source_sha256'],
            checks=dict(sorted(self.checks.items())),failures=self.failures,warnings=self.warnings,
            summary=dict(record_trials=len(self.results),isolated_goals=len(self.goals),recipients=len(recipients),
                publication_receipts=sum(row.get('publication_receipts',0) for row in self.results.values()),
                peer_eligible_recipients=sum(row['peer_access'] for row in recipients),
                recipients_with_peer_metadata=sum(bool(row['delivered_peer_candidate_ids']) for row in recipients),
                recipients_with_peer_metadata_dispatched=sum(bool(row['delivered_peer_candidate_ids']) and bool(row['packet_dispatched_request_ids']) for row in recipients),
                recipients_with_peer_source_returns=sum(bool(row['peer_full_source_return_count']) for row in recipients),
                recipients_with_peer_source_submitted=sum(bool(row['peer_source_submitted_count']) for row in recipients),
                empty_deliveries=sum(not row['delivered_candidate_ids'] for row in recipients)),
            boundaries=dict(method='Independent JSON/hash reconstruction of current E/D/F metadata, prefix ancestry, daemon receipt/view logs and actual archived request payloads. No hidden results are read.',
                daemon='Matches the real-daemon binary/start/goal/member/publish/inspect evidence and effective author views. The selected archive does not contain detached event signatures for independent cryptographic re-verification.',
                access='Own-versus-peer eligibility is a harness tool boundary, not a per-recipient daemon ACL.',
                use='Metadata delivery, request submission, returned full source, later source submission, and declared candidate parentage are separate observations. None establishes attention, semantic use, causal benefit, or improved quality.',
                matching='C/P inherit identical prefix requests, candidates and traces. Their first request contexts match after excluding the review packet and the output maximum funded from different exact input counts.',
                no_side_effects='No provider calls, daemon calls, candidate evaluations, hidden scoring or study writes.'),
            trials=self.results)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('study',type=Path)
    parser.add_argument('--output',required=True,type=Path)
    args=parser.parse_args()
    if args.output.resolve().is_relative_to(args.study.resolve()):
        raise ValueError('write audit reports outside the sealed study directory')
    result=Audit(args.study).run()
    args.output.write_text(json.dumps(result,indent=2,sort_keys=True,allow_nan=False)+'\n')
    print(json.dumps(dict(output=str(args.output.resolve()),status=result['status'],summary=result['summary'],
                          failures=result['failures'],warnings=result['warnings']),indent=2))
    return 0 if result['status']=='pass' else 1


if __name__=='__main__':
    raise SystemExit(main())
