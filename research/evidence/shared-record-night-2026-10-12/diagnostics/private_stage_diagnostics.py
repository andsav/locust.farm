#!/usr/bin/env python3
"""Read sealed studies; distinguish a queued private prompt from actual execution.

No harness imports, API calls, hidden-score reads, or candidate evaluation.
Usage: python3 private_stage_diagnostics.py STUDY --output REPORT.json
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path


def read(path):
    return json.loads(Path(path).read_text())


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def private_messages(items, prompt):
    marker = prompt + '\nOwn evaluated artifacts:\n'
    found = []
    for index, item in enumerate(items):
        if item.get('role') != 'user':
            continue
        content = item.get('content', '')
        if isinstance(content, list):
            content = ''.join(part.get('text', '') for part in content if isinstance(part, dict))
        if isinstance(content, str) and content.startswith(marker):
            found.append(index)
    return found


def active_private_prompt(items, prompt):
    user_indices = [index for index, item in enumerate(items) if item.get('role') == 'user']
    return bool(user_indices and user_indices[-1] in private_messages(items, prompt))


def agent_summary(manifest, trial, agent, ledger, counts):
    prompt = manifest['prompts']['private']
    strategy = manifest['continuation']['frontier' if trial['kind'] == 'R' else 'luna']
    archived = {row['id']: row for row in agent['requests']}
    candidates = {row['id'] for row in agent['candidates']}
    tool_candidates = {}
    for event in agent['trace']:
        if event['event'] == 'tool_results':
            for call_id, value in event['results']:
                if value.get('candidate_id') in candidates:
                    tool_candidates[call_id] = value['candidate_id']
    requests = []
    for request_id in agent['request_ids']:
        entry = ledger[request_id]
        if not active_private_prompt(entry['payload']['input'], prompt):
            continue
        record = archived.get(request_id, {})
        response = record.get('response')
        if response is not None and entry.get('response_id') not in (None, response.get('id')):
            raise ValueError('ledger and archived response identity differ: ' + request_id)
        created = []
        evaluate_calls = []
        for item in (response or {}).get('output', []):
            if item.get('type') == 'function_call' and item.get('name') == 'evaluate':
                evaluate_calls.append(item['call_id'])
                if item['call_id'] in tool_candidates:
                    created.append(tool_candidates[item['call_id']])
        requests.append(dict(id=request_id, inherited=entry['label'] != agent['id'],
                             ledger_status=entry['status'], reserved_at=entry['reserved_at'],
                             response_received=response is not None,
                             response_status=(response or {}).get('status'),
                             evaluate_call_ids=evaluate_calls, created_candidate_ids=sorted(set(created)),
                             cost=entry.get('cost'), reserved=entry['reserved']))
    received = sum(row['response_received'] for row in requests)
    completed = sum(row['response_status'] == 'completed' for row in requests)
    incomplete = sum(row['response_status'] == 'incomplete' for row in requests)
    created = sorted({candidate for row in requests for candidate in row['created_candidate_ids']})
    completed_created = {candidate for row in requests if row['response_status'] == 'completed'
                         for candidate in row['created_candidate_ids']}
    queued = bool(agent.get('private_review_done'))
    if not queued and not requests:
        status = 'not_applicable' if strategy == 'continue' else 'not_queued'
    elif not requests:
        status = 'queued_no_generation_dispatch'
    elif not received:
        status = 'dispatched_no_archived_response'
    elif completed:
        status = 'completed_response_with_candidate' if completed_created else 'completed_response_no_candidate'
    else:
        status = 'response_received_with_candidate' if created else 'response_received_no_candidate'
    own_counts = [row for row in counts if row['label'] == agent['id']
                  and active_private_prompt(row['payload']['input'], prompt)]
    return dict(agent=agent['name'], agent_id=agent['id'], strategy=strategy, status=status,
                prompt_queued_flag=queued,
                private_prompt_present_in_final_history=bool(private_messages(agent['history'], prompt)),
                own_private_input_count_attempts=len(own_counts),
                own_private_input_count_statuses=dict(Counter(row['status'] for row in own_counts)),
                private_generation_dispatches=len(requests),
                own_private_generation_dispatches=sum(not row['inherited'] for row in requests),
                private_responses_received=received, private_responses_completed=completed,
                private_responses_incomplete=incomplete,
                private_created_candidate_ids=created,
                private_created_candidates=len(created),
                private_completed_response_created_candidates=len(completed_created),
                own_private_created_candidates=len({candidate for row in requests if not row['inherited']
                                                    for candidate in row['created_candidate_ids']}),
                private_generation_requests=requests,
                budget_stop_reasons=[row['reason'] for row in agent['trace'] if row['event'] == 'budget_stop'],
                failed=agent.get('failed'))


def summarize(directory):
    directory = Path(directory).resolve()
    manifest = read(directory / 'manifest.json')
    seal = read(directory / 'seal.json')  # Required: an unfinished live study is refused.
    if seal['manifest_sha256'] != digest(directory / 'manifest.json'):
        raise ValueError('manifest differs from its completion seal')
    required = {'ledger.json', 'freeze.json'} | {
        f'trials/{block["id"]}/{arm}.json' for block in manifest['blocks'] for arm in manifest['arms']}
    if not required.issubset(seal['files']):
        raise ValueError('seal omits assigned trials or accounting records')
    for name, expected in seal['files'].items():
        if digest(directory / name) != expected:
            raise ValueError('sealed file changed: ' + name)
    ledger = {row['id']: row for row in read(directory / 'ledger.json')}
    counts_path = directory / 'ledger.counts.json'
    if counts_path.exists() and 'ledger.counts.json' not in seal['files']:
        raise ValueError('input-count records are not sealed')
    counts = read(counts_path) if counts_path.exists() else []
    trials = {}
    for block in manifest['blocks']:
        for arm in manifest['arms']:
            key = f'{block["id"]}/{arm}'
            trial = read(directory / 'trials' / f'{key}.json')
            if not trial['final'] or trial['manifest_sha256'] != seal['manifest_sha256']:
                raise ValueError('trial is incomplete or belongs to another manifest')
            trials[key] = dict(family=block['family'], arm=arm,
                               agents=[agent_summary(manifest, trial, agent, ledger, counts)
                                       for agent in trial['agents']])
    agents = [agent for trial in trials.values() for agent in trial['agents']]
    return dict(schema='shared-record-private-stage-diagnostic.v1', study_id=manifest['study_id'],
                source_directory=str(directory), manifest_sha256=seal['manifest_sha256'],
                seal_sha256=digest(directory / 'seal.json'), diagnostic_sha256=digest(__file__),
                interpretation='The flag proves only that a private prompt was queued. Dispatch requires that exact frozen private prompt as the latest user instruction in a sealed generation payload; later peer-review instructions end this private stage. A completed response requires its archived provider response. Candidate attribution joins evaluate call IDs in that response to candidate IDs in the agent tool-result trace. These establish execution and creation, not semantic use or benefit.',
                inheritance_note='Copied prefixes can repeat request evidence across arm views. Own dispatches and candidates exclude inherited request labels; totals are diagnostics, not additional billing.',
                summary=dict(agents=len(agents), statuses=dict(Counter(row['status'] for row in agents)),
                             queued_without_generation=sum(row['status'] == 'queued_no_generation_dispatch' for row in agents),
                             agents_with_completed_private_response=sum(row['private_responses_completed'] > 0 for row in agents),
                             agents_with_private_candidates=sum(row['private_created_candidates'] > 0 for row in agents)),
                trials=trials)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('study', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = summarize(args.study)
    text = json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + '\n'
    if args.output:
        if args.output.resolve().is_relative_to(args.study.resolve()):
            raise ValueError('write diagnostics outside the sealed study directory')
        args.output.write_text(text)
        print(json.dumps(dict(output=str(args.output.resolve()), summary=result['summary']), sort_keys=True))
    else:
        print(text, end='')


if __name__ == '__main__':
    main()
