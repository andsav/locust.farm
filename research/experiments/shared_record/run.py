#!/usr/bin/env python3
"""Frozen information-policy runner. Defaults to zero-network scripted verification."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import copy
import json
from pathlib import Path
import re
import shutil
import sys
import threading
import time
import uuid

from luna import Session, Ledger, BudgetStop, TransportFailure
from protocol import (ROOT, SCHEMA, build_manifest, digest, file_digest, freeze,
                      load_manifest, save, seal, trial_paths, verify_seal)
from record import Daemon, FrozenRecord
import scoring

SYSTEM = '''Design a Python 3 standard-library heuristic defining solve(instance). Lower cost is better. The public evaluator records complete candidates. The result selector first requires validity on every public evaluation, then minimizes unrounded normalized public cost, then source hash. If no candidate is fully valid the fixed task fallback is used. Hidden evaluation happens only after the entire study's outputs are sealed, and is never available to you.
Public instances are available as INSTANCES inside run_python. Use deterministic randomness. The task prompt states the evaluator CPU budget. Candidate errors and overruns receive fallback cost. Produce a valid candidate early, verify changes, and stop when additional work is not useful. Every request, including review and source retrieval, uses your API allowance. Do not inspect files, environments or credentials.'''
REVIEW = ('Review the supplied evaluated attempts. Use the public evidence to decide whether to retain your '
          'approach, test a specific alternative, or reject a suggested change. Verify changes with the public '
          'evaluator. You may ignore unhelpful evidence. Continue using your remaining allowance.')
PRIVATE = ('Privately verify your own evaluated attempts and, if useful, try a materially different approach. '
           'You may retain your best candidate and stop when further work is not useful.')


def schema(properties):
    return dict(type='object', properties=properties, required=list(properties), additionalProperties=False)


BASE_TOOLS = [
    dict(name='run_python', description='Run standard-library Python with public INSTANCES; no candidate is recorded.',
         input_schema=schema({'source': {'type': 'string'}})),
    dict(name='evaluate', description='Evaluate and record a complete solve(instance) candidate on public instances only.',
         input_schema=schema({'source': {'type': 'string'}, 'parent_id': {'type': ['string', 'null']}})),
]
SOURCE_TOOL = dict(name='get_source', description='Retrieve complete source of an explicitly eligible candidate from the frozen record.',
                   input_schema=schema({'candidate_id': {'type': 'string'}}))
PROMPTS = dict(system=SYSTEM, review=REVIEW, private=PRIVATE, tools=BASE_TOOLS, source_tool=SOURCE_TOOL)



class Agent:
    def __init__(self, trial, name, allowance, model_key='luna', prefix=None):
        self.trial, self.name = trial, name
        self.id = f'{trial.block["id"]}/{trial.arm}/{name}'
        self.record = None
        self.review_at = None
        self.prefix_candidate_ids = []
        self.failed = None
        self.candidates, self.trace = [], []
        self.private_review_done = False
        self.model_key = model_key
        transport = trial.transport_factory(trial, name) if trial.transport_factory else None
        if prefix is None:
            self.session = Session(trial.manifest['prompts']['system'], copy.deepcopy(BASE_TOOLS),
                                   trial.ledger, self.id, trial.block['phase'],
                                   trial.manifest['budget']['phase_usd'], allowance,
                                   effort=trial.manifest.get('effort', 'high'),
                                   model_config=trial.manifest['models'][model_key], transport=transport,
                                   paid_opt_in=trial.paid,
                                   live_manifest={'path': str(trial.output / 'manifest.json'),
                                                  'sha256': trial.manifest_hash, 'model_key': model_key})
            self.session.user(trial.block['prompts']['solo' if trial.kind in ('S', 'R') else 'group'])
        else:
            self.session = prefix.session.fork(self.id, allowance=allowance)
            self.session.transport = transport
            self.candidates = copy.deepcopy(prefix.candidates)
            self.prefix_candidate_ids = [c['id'] for c in self.candidates]
            self.trace = copy.deepcopy(prefix.trace)
            self.failed = prefix.failed
            self.private_review_done = prefix.private_review_done
        self.initial_prompt = self.session.history[0]

    def tool(self, name, args):
        if name == 'run_python':
            return scoring.probe(args['source'], self.trial.block['public']['instances'])
        if name == 'evaluate':
            parent = args.get('parent_id')
            eligible = {c['id'] for c in self.candidates}
            if parent is not None and parent not in eligible:
                if self.record is None:
                    raise ValueError('parent_id is not an available candidate')
                self.record.source(self.name, parent)  # Enforce the same authority as source retrieval.
            ev = self.trial.manifest['evaluation']
            result = scoring.score(self.trial.block['family'], args['source'],
                                   self.trial.block['public']['instances'],
                                   self.trial.block['public']['reference'],
                                   repetitions=ev['repetitions'], seed=ev['seed'],
                                   cpu_seconds=ev['cpu_seconds'], wall_seconds=ev['wall_seconds'])
            # Generate once and retain through forks, parent links and source retrieval.
            # Internal accounting labels must not become model-visible arm cues.
            item = dict(id='candidate-' + uuid.uuid4().hex, parent_id=parent, agent=self.name,
                        sequence=len(self.candidates), source=args['source'], sha256=result['source_sha256'],
                        time=time.time(), public=result)
            self.candidates.append(item)
            return dict(candidate_id=item['id'], **scoring.public_view(result, self.trial.block['public']['reference']))
        if name == 'get_source' and self.record is not None:
            return self.record.source(self.name, args['candidate_id'])
        return {'error': 'tool is not available'}

    def private_review(self):
        strategy = self.trial.manifest['continuation'][self.model_key]
        if self.private_review_done or strategy == 'continue':
            return False
        self.private_review_done = True
        if strategy == 'restart':
            self.session.history = [copy.deepcopy(self.initial_prompt)]
        self.session.user(self.trial.manifest['prompts']['private'] + '\nOwn evaluated artifacts:\n' +
                          json.dumps(self.candidates, sort_keys=True))
        return True

    def run(self, stage_allowance=None):
        total = self.session.allowance
        if stage_allowance is not None:
            self.session.allowance = stage_allowance
        started = time.time()
        try:
            while self.failed is None:
                self.trial.checkpoint()
                try:
                    calls, visible, status = self.session.request()
                except BudgetStop as error:
                    self.trace.append(dict(event='budget_stop', reason=str(error), time=time.time()))
                    break
                except (TransportFailure, RuntimeError, ValueError) as error:
                    self.failed = type(error).__name__ + ': ' + str(error)
                    self.trace.append(dict(event='request_failure', error=self.failed, time=time.time()))
                    break
                self.trace.append(dict(event='response', text=visible, status=status, time=time.time()))
                if not calls:
                    if status == 'incomplete':
                        # An incomplete output is an observed provider/resource stop, not an instruction to spin.
                        self.trace.append(dict(event='incomplete_stop', time=time.time()))
                        break
                    if self.private_review():
                        continue
                    break
                results = []
                for ident, tool, args in calls:
                    try:
                        value = self.tool(tool, args)
                    except (KeyError, TypeError, ValueError, PermissionError) as error:
                        value = {'error': type(error).__name__ + ': ' + str(error)}
                    value['spend'] = dict(committed_usd=self.session.spent(), allowance_usd=total)
                    results.append((ident, value))
                self.session.results(results)
                self.trace.append(dict(event='tool_results', results=results, time=time.time()))
                self.trial.checkpoint()
        finally:
            self.session.allowance = total
            self.elapsed = time.time() - started
        return self

    def summary(self):
        return dict(name=self.name, id=self.id, request_ids=self.session.request_ids,
                    allowance_usd=self.session.allowance, spent_usd=self.session.spent(),
                    failed=self.failed, candidates=self.candidates, history=self.session.history,
                    requests=self.session.requests, trace=self.trace,
                    private_review_done=self.private_review_done,
                    review_at=self.review_at, prefix_candidate_ids=self.prefix_candidate_ids,
                    elapsed_seconds=getattr(self, 'elapsed', 0))


class Trial:
    def __init__(self, manifest, block, arm, output, ledger, daemon,
                 transport_factory=None, paid=False, prefix_kind=None):
        self.manifest, self.block, self.arm = manifest, block, arm
        self.output, self.ledger, self.daemon = Path(output), ledger, daemon
        self.transport_factory, self.paid = transport_factory, paid
        self.kind = manifest['arms'][arm]['kind'] if prefix_kind is None else 'prefix'
        self.manifest_hash = file_digest(self.output / 'manifest.json')
        self.path = (self.output / f'prefixes/{block["id"]}-{arm}.json' if prefix_kind else
                     self.output / f'trials/{block["id"]}/{arm}.json')
        self.agents = []
        self.record = None
        self.error = None
        self.started = time.time()
        self.finished = None
        self.prefix_elapsed_seconds = 0
        self.lock = threading.RLock()

    def result(self, final=False):
        agents = [a.summary() for a in self.agents]
        candidates = [c for a in agents for c in a['candidates']]
        selected = scoring.select_public(candidates)
        ids = list(dict.fromkeys(rid for agent in agents for rid in agent['request_ids']))
        return dict(schema=SCHEMA, manifest_sha256=self.manifest_hash,
                    block_id=self.block['id'], family=self.block['family'], pack=self.block['pack'],
                    repetition=self.block['repetition'], attempt=self.block['attempt'], phase=self.block['phase'],
                    arm=self.arm, kind=self.kind, final=final, started=self.started,
                    finished=self.finished if final else None, prefix_elapsed_seconds=self.prefix_elapsed_seconds,
                    error=self.error, agents=agents,
                    request_ids=ids, candidates=candidates, selected=None if selected is None else selected['id'],
                    prefix_candidate_ids=[c for a in self.agents for c in a.prefix_candidate_ids],
                    record=None if self.record is None else self.record.archive())

    def checkpoint(self):
        with self.lock:
            save(self.path, self.result())

    def run(self, prefix=None, fraction=None):
        if prefix is not None:
            self.prefix_elapsed_seconds = prefix.finished - prefix.started
        count = 1 if self.kind in ('S', 'R') else 3
        allowance = self.manifest['budget']['task_arm_usd'] / count
        model = 'frontier' if self.kind == 'R' else 'luna'
        self.agents = [Agent(self, f'agent-{i}', allowance, model,
                             prefix=prefix.agents[i] if prefix is not None else None) for i in range(count)]
        self.checkpoint()
        try:
            if self.kind in ('C', 'P'):
                policy = self.manifest['arms'][self.arm]['policy']
                goal = self.daemon.goal(f'{self.manifest["study_id"]}/{self.block["id"]}/{self.arm}',
                                        [a.name for a in self.agents])
                self.record = FrozenRecord(goal, f'{self.block["id"]}/{self.arm}', policy,
                                           {a.name: a.candidates for a in self.agents})
                self.record.freeze()
                for agent in self.agents:
                    try:
                        packet = self.record.review(agent.name, peers=self.kind == 'P')
                        agent.record = self.record
                        agent.review_at = time.time()
                        agent.session.tools = copy.deepcopy(BASE_TOOLS) + [copy.deepcopy(SOURCE_TOOL)]
                        agent.session.user(self.manifest['prompts']['review'] + '\nFrozen evidence:\n' +
                                           json.dumps(packet, sort_keys=True))
                    except Exception as error:
                        agent.failed = 'record delivery failed: ' + type(error).__name__ + ': ' + str(error)
            with ThreadPoolExecutor(max_workers=count) as pool:
                futures = [pool.submit(a.run, allowance * fraction if fraction else None) for a in self.agents]
                # Each failed worker is retained; one exception cannot cancel its peers.
                for agent, future in zip(self.agents, futures):
                    try:
                        future.result()
                    except Exception as error:
                        agent.failed = type(error).__name__ + ': ' + str(error)
        except Exception as error:
            self.error = type(error).__name__ + ': ' + str(error)
        self.finished = time.time()
        save(self.path, self.result(final=True))
        return self


def run_study(output, *, transport_factory=None, paid=False):
    output = Path(output)
    manifest = load_manifest(output)
    if manifest['prompts'] != PROMPTS:
        raise ValueError('rendered prompt/tool configuration differs from current runner')
    if (output / 'seal.json').exists():
        verify_seal(output)
        return {'already_complete': True}
    if (output / 'execution.json').exists():
        raise ValueError('interrupted execution: preserve it and use finalize-interrupted; never silently redispatch')
    if manifest['stage'] == 'scripted' and (transport_factory is None or paid):
        raise ValueError('scripted studies require an offline transport and prohibit paid calls')
    if manifest['stage'] != 'scripted' and (not paid or transport_factory is not None):
        raise ValueError('development/scored execution requires explicit paid authorization and live transport')
    if paid:
        from campaign import verify_allocation
        verify_allocation(output, manifest)
    save(output / 'execution.json', dict(status='running', manifest_sha256=file_digest(output / 'manifest.json')))
    ledger = Ledger(output / 'ledger.json', manifest['budget']['study_usd'])
    ledger.save()
    with Daemon(output, output / 'bin/locust') as daemon:
        try:
            for block in manifest['blocks']:
                prefixes = {}
                policies = {arm['policy'] for arm in manifest['arms'].values() if arm['kind'] in ('C', 'P')}
                for key, fraction in [('early', .25), ('delayed', .5)]:
                    if (key == 'early' and policies & {'E', 'F'}) or (key == 'delayed' and 'D' in policies):
                        prefixes[key] = Trial(manifest, block, key, output, ledger, daemon,
                                              transport_factory, paid, prefix_kind=key).run(fraction=fraction)
                for arm in block['launch_order']:
                    settings = manifest['arms'][arm]
                    policy = settings.get('policy')
                    prefix = prefixes.get('delayed' if policy == 'D' else 'early') if policy else None
                    trial = Trial(manifest, block, arm, output, ledger, daemon, transport_factory, paid)
                    trial.run(prefix=prefix)
        finally:
            events = Path(daemon.daemon.events)
            if events.exists():
                shutil.copy2(events, output / 'daemon-events.jsonl')
    value = seal(output)
    save(output / 'execution.json', dict(status='sealed', seal_sha256=file_digest(output / 'seal.json')))
    return value


def finalize_interrupted(output):
    """Explicitly retain partial work and failures; never retry or pay for missing work."""
    output = Path(output)
    manifest = load_manifest(output)
    if (output / 'seal.json').exists():
        return verify_seal(output)
    if not (output / 'execution.json').exists():
        raise ValueError('study never started')
    ledger = json.loads((output / 'ledger.json').read_text())
    for block in manifest['blocks']:
        for arm in manifest['arms']:
            setting = manifest['arms'][arm]
            prefix_name = ('delayed' if setting.get('policy') == 'D' else 'early')
            prefix_path = output / f'prefixes/{block["id"]}-{prefix_name}.json'
            prefix = None
            if setting['kind'] in ('C', 'P') and prefix_path.exists():
                prefix = json.loads(prefix_path.read_text())
            path = output / f'trials/{block["id"]}/{arm}.json'
            if path.exists():
                value = json.loads(path.read_text())
                if value['final']:
                    continue
            else:
                value = dict(schema=SCHEMA, manifest_sha256=file_digest(output / 'manifest.json'),
                             block_id=block['id'], family=block['family'], pack=block['pack'],
                             repetition=block['repetition'], attempt=block['attempt'], phase=block['phase'],
                             arm=arm, kind=manifest['arms'][arm]['kind'], agents=[], candidates=[],
                             selected=None, request_ids=[], started=None, record=None)
                if prefix:
                    value.update(agents=copy.deepcopy(prefix['agents']), candidates=prefix['candidates'],
                                 selected=prefix['selected'], request_ids=prefix['request_ids'],
                                 prefix_elapsed_seconds=(prefix['finished'] - prefix['started']
                                                         if prefix.get('finished') is not None else None),
                                 prefix_candidate_ids=[c['id'] for c in prefix['candidates']])
            value.update(final=True, finished=time.time(), error='interrupted; no automatic replacement')
            # Dispatches persisted after the last trial checkpoint must remain attributed.
            labels = [f'{block["id"]}/{arm}/']
            if prefix:
                labels.append(f'{block["id"]}/{prefix_name}/')
            value['request_ids'] = list(dict.fromkeys(value['request_ids'] +
                [entry['id'] for entry in ledger if any(entry['label'].startswith(label) for label in labels)]))
            save(path, value)
    return seal(output)


def scripted_spec(study_id='offline-information-policy-replay'):
    model = dict(model='scripted-luna', rates=dict(input=1., cached_input=.5, cache_write=1., output=2.),
                 context_tokens=4096, max_input_tokens=4096, max_output_tokens=1024, service_tier='default',
                 input_accounting=dict(method='context_bound', qualification='script fixture; not a provider bound'),
                 qualification=dict(model='script fixture', rates='synthetic dollars', limits='script fixture'))
    arms = {'S': {'kind': 'S'}, 'I': {'kind': 'I'}, 'R': {'kind': 'R'}}
    for policy in ('E', 'D', 'F'):
        for kind in ('C', 'P'):
            arms[f'{kind}_{policy}'] = dict(kind=kind, policy=policy)
    return dict(study_id=study_id, seed='information-policy-offline-2026-10-12', stage='scripted',
                models={'luna': model, 'frontier': dict(model, model='scripted-frontier')},
                budget=dict(task_arm_usd=.6, phase_usd=100., study_usd=100.), effort='high',
                arms=arms, continuation={'luna': 'continue', 'frontier': 'verify'},
                blocks=[dict(family='vertex_cover', pack=f'pack-{pack}', repetition=rep,
                             attempt='attempt-0', phase='scripted') for pack in range(2) for rep in range(2)],
                public_instances=1, hidden_instances=1,
                evaluation=dict(repetitions=1, seed='offline-evaluation', cpu_seconds=2, wall_seconds=40),
                analysis=dict(bootstrap_samples=1000, seed='offline-bootstrap', useful_effect=.01))


class ScriptedTransport:
    """A finite test script, never an external model or a forecasting-quality result."""
    def __init__(self, trial, name):
        self.trial, self.name = trial, name
        self.step = 0

    def __call__(self, payload):
        step = self.step
        self.step += 1
        # Verify failure preservation in an otherwise completed block.
        if self.trial.arm == 'I' and self.name == 'agent-2' and self.trial.block['repetition'] == 1:
            raise TransportFailure('scripted uncertain transport failure')
        source = ('def solve(i):\n    return []\n' if self.name == 'agent-2' else
                  f'# {self.name}\ndef solve(i):\n    return list(range(i["n"]))\n')
        calls = []
        if self.trial.kind in ('C', 'P'):
            packet = json.loads(payload['input'][-1]['content'].split('Frozen evidence:\n', 1)[1]) if step == 0 else None
            if step == 0 and packet['candidates']:
                calls = [('get_source', {'candidate_id': packet['candidates'][-1]['candidate_id']})]
            elif step <= 1:
                calls = [('evaluate', {'source': source, 'parent_id': None})]
        elif step == 0:
            calls = [('evaluate', {'source': source, 'parent_id': None})]
            if self.name != 'agent-1':
                calls.append(('evaluate', {'source': source + '# repeated attempt\n', 'parent_id': None}))
        output = [dict(type='function_call', call_id=f'call-{step}-{i}', name=name,
                       arguments=json.dumps(args)) for i, (name, args) in enumerate(calls)]
        if not output:
            output = [dict(type='message', role='assistant', content=[dict(type='output_text', text='done')])]
        return dict(id=f'script-{digest([self.trial.block["id"], self.trial.arm, self.name, step])}',
                    model=payload['model'], status='completed',
                    usage=dict(input_tokens=100, output_tokens=80,
                               input_tokens_details={'cached_tokens': 0}), output=output)


def replay(output, binary):
    manifest = build_manifest(scripted_spec(), binary, PROMPTS)
    freeze(output, manifest, binary)
    run_study(output, transport_factory=ScriptedTransport)
    from analyze import offline_hidden, analyze
    offline_hidden(output)
    report = analyze(output)
    save(Path(output) / 'analysis.json', report)
    save(Path(output) / 'replay-verification.json', verify_scripted_replay(output))
    return report


def verify_scripted_replay(output):
    """Assert delivery, failure and attribution facts; synthetic scores prove no quality claim."""
    output = Path(output)
    manifest = verify_seal(output)
    if manifest['stage'] != 'scripted':
        raise ValueError('scripted verification cannot qualify a model study')
    ledger = json.loads((output / 'ledger.json').read_text())
    goals, receipts, empty_filtered, invalid_recipients, failures = set(), 0, 0, 0, 0
    candidates_by_id = {}
    for block in manifest['blocks']:
        trials = {arm: json.loads((output / f'trials/{block["id"]}/{arm}.json').read_text())
                  for arm in manifest['arms']}
        for arm, trial in trials.items():
            if trial['error']:
                raise AssertionError(f'replay branch failed: {block["id"]}/{arm}: {trial["error"]}')
            for agent in trial['agents']:
                expected_failure = arm == 'I' and agent['name'] == 'agent-2' and block['repetition'] == 1
                if bool(agent['failed']) != expected_failure:
                    raise AssertionError(f'unexpected worker outcome: {block["id"]}/{arm}/{agent["name"]}')
                failures += bool(agent['failed'])
            if any('hidden' in candidate for candidate in trial['candidates']):
                raise AssertionError('hidden result leaked into live candidate archive')
            for candidate in trial['candidates']:
                ident = candidate['id']
                if not re.fullmatch(r'candidate-[0-9a-f]{32}', ident):
                    raise AssertionError('candidate identity exposes execution labels')
                if ident in candidates_by_id and candidates_by_id[ident] != candidate:
                    raise AssertionError('candidate identity changed across archived copies')
                candidates_by_id[ident] = candidate
            record = trial['record']
            if record:
                if record['state'] != 'frozen' or record['goal'] in goals:
                    raise AssertionError('unfrozen or reused branch goal')
                goals.add(record['goal'])
                receipts += len(record['receipts'])
                for delivery in record['deliveries']:
                    if delivery['status'] != 'delivered':
                        raise AssertionError('scripted daemon delivery incomplete')
                    if trial['kind'] == 'C' and delivery['peer_access']:
                        raise AssertionError('private review received peer records')
                    if trial['kind'] == 'P' and delivery['recipient'] == 'agent-2':
                        if delivery['peer_access']:
                            raise AssertionError('invalid recipient received peers')
                        invalid_recipients += 1
                    if record['policy'] == 'F' and not delivery['delivered_candidate_ids']:
                        empty_filtered += 1
        for policy in ('E', 'D', 'F'):
            prefix_name = 'delayed' if policy == 'D' else 'early'
            prefix = json.loads((output / f'prefixes/{block["id"]}-{prefix_name}.json').read_text())
            for arm in (f'C_{policy}', f'P_{policy}'):
                if not set(prefix['request_ids']).issubset(trials[arm]['request_ids']):
                    raise AssertionError('common prefix was not fully attributed')
            for control, peer, original in zip(trials[f'C_{policy}']['agents'], trials[f'P_{policy}']['agents'], prefix['agents']):
                count = len(original['requests'])
                left, right = control['requests'][count]['payload'], peer['requests'][count]['payload']
                before = copy.deepcopy(left)
                after = copy.deepcopy(right)
                before['input'] = before['input'][:-1]
                after['input'] = after['input'][:-1]
                if before != after:
                    raise AssertionError('C/P first continuations differ beyond review evidence')
                if original['name'] == 'agent-2' and left != right:
                    raise AssertionError('invalid P recipient differs from private control')
        if set(trials['I']['request_ids']) & set(trials['C_E']['request_ids']):
            raise AssertionError('independent arm reused staged requests')
    if not (receipts and empty_filtered and invalid_recipients and failures):
        raise AssertionError('scripted replay did not exercise all required cases')
    if any(not item['model'].startswith('scripted-') for item in ledger):
        raise AssertionError('replay contains a non-scripted model')
    for item in ledger:
        visible = json.dumps(item['payload'], sort_keys=True)
        if any(block['id'] in visible for block in manifest['blocks']):
            raise AssertionError('internal execution identity leaked into a model request')
        if any(f'/{label}/' in visible for label in [*manifest['arms'], 'early', 'delayed']):
            raise AssertionError('arm or prefix label leaked into a model request')
    return dict(status='passed', paid_model_calls=0, blocks=len(manifest['blocks']),
                trials=len(trial_paths(manifest)), isolated_goals=len(goals), signed_receipts=receipts,
                empty_filtered_deliveries=empty_filtered, invalid_peer_recipients=invalid_recipients,
                expected_transport_failures=failures, scripted_requests=len(ledger),
                opaque_candidate_ids=len(candidates_by_id), label_free_model_requests=len(ledger),
                binary_sha256=manifest['runtime']['binary_sha256'],
                manifest_sha256=file_digest(output / 'manifest.json'),
                seal_sha256=file_digest(output / 'seal.json'),
                hidden_sha256=file_digest(output / 'hidden.json'),
                source_sha256=manifest['runtime']['source_sha256'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    for command in ('freeze', 'execute', 'replay', 'finalize-interrupted'):
        part = sub.add_parser(command)
        part.add_argument('--output', type=Path, required=True)
        if command in ('freeze', 'replay'):
            part.add_argument('--binary', type=Path, default=ROOT / 'target/debug/locust')
        if command == 'freeze':
            part.add_argument('--spec', type=Path, required=True)
        if command == 'execute':
            part.add_argument('--allow-paid', action='store_true', help='explicit authorization; never used by offline verification')
    args = parser.parse_args()
    if args.command == 'freeze':
        spec = json.loads(args.spec.read_text())
        freeze(args.output, build_manifest(spec, args.binary, PROMPTS), args.binary)
    elif args.command == 'execute':
        run_study(args.output, paid=args.allow_paid)
    elif args.command == 'replay':
        replay(args.output, args.binary)
    else:
        finalize_interrupted(args.output)
    print(json.dumps({'command': args.command, 'output': str(args.output.resolve()), 'complete': True}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
