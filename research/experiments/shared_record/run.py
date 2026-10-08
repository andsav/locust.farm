#!/usr/bin/env python3
"""Run the preregistered shared-record study. See README.md for the protocol."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import random
import shutil
import sys
import threading
import time

from luna import Session, Ledger, BudgetStop, TransportFailure, MODEL, RATES
from problems import FAMILIES, DEVELOPMENT, HELD_OUT, PUBLIC_INSTANCES, HIDDEN_INSTANCES, instances, reference_costs
from record import Daemon, copy_binary
import scoring

HERE = Path(__file__).resolve().parent

CONFIG = dict(
    study_seed='shared-record-2026-10-08',
    amendment='A1: instance sizes raised about tenfold and instances delivered through run_python rather than the prompt, '
              'after the first calibration saturated (README, Amendment A1). '
              'A2 (post hoc): gate criterion 3 requires the hidden range in at least one independent calibration trial, '
              'not each, after calibration 2 failed only on vertex cover at 0.0024 (README, Amendment A2)',
    model=MODEL, effort='high', rates=RATES,
    k=3, task_arm_allowance_usd=0.42,
    arms={'solo': 1, 'independent': 3, 'shared': 3},
    max_turns=150, nudge_fraction=0.4, history_token_soft_limit=150000,
    public_instances=PUBLIC_INSTANCES, hidden_instances=HIDDEN_INSTANCES,
    cpu_seconds_per_instance=scoring.CPU_SECONDS_PER_INSTANCE,
    calibration=dict(tasks=[['maxcut', ['independent', 'shared']], ['vertex_cover', ['independent', 'solo']]],
                     ceiling_usd=2.00),
    scored=dict(tasks=list(HELD_OUT), arms=['solo', 'independent', 'shared'], ceiling_usd=8.50, concurrent_tasks=2),
    study_ceiling_usd=15.00,
    gate=dict(max_transport_failures=1, min_hidden_range=0.005, min_agents_with_valid_candidate=2,
              independent_trials_with_range='at least one (A2); the preregistered rule was each'),
)

SYSTEM = '''You are designing a heuristic for a combinatorial optimization problem. Write Python 3 using only the standard library, defining solve(instance) that returns a solution in the stated format. Lower cost is better.

Scoring: a fixed evaluator runs your solve on instances drawn from the stated generator. You have 5 public instances, available inside run_python. Hidden instances from the same distribution, which you never see, decide your real score. Score = your total cost / the reference heuristic's total cost on the same instances; 1.0 equals the reference, lower is better. An instance where solve raises, returns an invalid solution, or exceeds the CPU budget counts as the trivial fallback's cost, which is much worse than the reference. Do not overfit the public instances.

Compute budget per instance: 2 CPU seconds, enforced by the evaluator. Design solve to stop its own search after about 1.5 s of time.process_time(). Seed any randomness deterministically.

Tools:
- run_python(source): free experimentation. Runs your source with the public instances in a global INSTANCES list and returns stdout/stderr. Nothing is recorded.
- evaluate(source): scores complete candidate source on the public instances and records it as a candidate. Your result is your best-public-score candidate, so evaluate every candidate you consider an improvement.

You have an API-dollar allowance for this task; every tool result reports your spend so far. Produce a valid candidate early. Then keep improving until the allowance is nearly used; stop earlier only if you are confident more work will not help. Tool execution is free. Do not inspect files, environments or credentials. Keep explanations brief; spend effort on the heuristic.'''

ARM_NOTES = {
    'solo': 'You work alone on this task.',
    'independent': 'Two other agents (the same model) work independently on this same task with the same tools and allowance. You cannot communicate. The task result is the best-public-score candidate among all three agents.',
    'shared': '''Two other agents (the same model) work on this same task with the same tools and allowance. The task result is the best-public-score candidate among all three agents.

A shared record (a Locust goal) holds findings posted by all three agents. Two extra tools:
- read_record(): returns findings posted by the other agents since your last read, oldest first. It is refused until you have evaluated your own first candidate, so initial attempts stay independent.
- post_finding(summary, include_source): publishes a finding attached to your latest evaluated candidate (its public score and source hash; its full source if include_source is true).

Use the record. After your first candidate, read it before deciding what to try next, and read again before major changes of approach. Post a finding after each meaningful result, including approaches that did not help: what you tried, the public score, and why it did or did not work. Include your source when a peer could build on it. Peer findings are claims scored by the same public evaluator; verify before relying on them.''',
}


def schema(properties):
    return dict(type='object', properties=properties, required=list(properties), additionalProperties=False)


BASE_TOOLS = [
    dict(name='run_python', description='Run Python source for experimentation with the public instances available as INSTANCES. Returns stdout and stderr. Records nothing.',
         input_schema=schema({'source': {'type': 'string'}})),
    dict(name='evaluate', description='Score complete candidate source (defining solve) on the public instances and record it as a candidate.',
         input_schema=schema({'source': {'type': 'string'}})),
]
RECORD_TOOLS = [
    dict(name='read_record', description='Return findings posted by the other agents since your last read, oldest first. Refused before your first evaluated candidate.',
         input_schema=schema({})),
    dict(name='post_finding', description='Publish a finding to the shared record, attached to your latest evaluated candidate.',
         input_schema=schema({'summary': {'type': 'string'}, 'include_source': {'type': 'boolean'}})),
]


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix('.tmp')
    temp.write_text(json.dumps(value, indent=1, sort_keys=True) + '\n')
    temp.replace(path)


def task_prompt(task, arm, allowance, public, public_reference):
    problem = FAMILIES[task]
    fallback = [problem.cost(i, problem.fallback(i)) for i in public]
    return (f'Problem: {problem.title}\n{problem.description}\n'
            f'Instance generator: {problem.shape}\n\n'
            f'{ARM_NOTES[arm]}\n\n'
            f'Your API allowance for this task: ${allowance:.2f}.\n\n'
            f'The {len(public)} public instances are not printed here. They are the global list INSTANCES inside '
            f'run_python, and evaluate scores exactly these {len(public)} plus hidden instances from the same generator. '
            f'Inspect them with run_python before designing.\n\n'
            f'Reference heuristic cost on each public instance: {[round(c, 3) for c in public_reference]}\n'
            f'Trivial fallback cost on each public instance: {[round(c, 3) for c in fallback]}')


class Agent:
    def __init__(self, trial, name, allowance, goal=None):
        self.trial, self.name, self.allowance, self.goal = trial, name, allowance, goal
        tools = BASE_TOOLS + (RECORD_TOOLS if trial.arm == 'shared' else [])
        self.label = f'{trial.phase}/{trial.task}/{trial.arm}/{name}'
        self.session = Session(SYSTEM, tools, trial.ledger, self.label, trial.phase,
                               trial.phase_ceiling, allowance, effort=CONFIG['effort'])
        self.session.user(task_prompt(trial.task, trial.arm, allowance, trial.public, trial.public_reference))
        self.candidates, self.trace, self.reads, self.posts = [], [], [], []
        self.seen = set()
        self.failed = None
        self.nudged = False
        self.turns = 0

    def tool(self, name, args):
        trial = self.trial
        if name == 'run_python':
            return scoring.probe(args['source'], trial.public)
        if name == 'evaluate':
            source = args['source']
            public = scoring.score(trial.task, source, trial.public, trial.public_reference)
            hidden = scoring.score(trial.task, source, trial.hidden, trial.hidden_reference)
            self.candidates.append(dict(
                index=len(self.candidates), agent=self.name, source=source, sha256=public['source_sha256'],
                time=time.time(), turn=self.turns, agent_spent_usd=self.session.spent(),
                public=dict(normalized=public['normalized'], valid_instances=public['valid_instances'],
                            costs=[r['cost'] for r in public['instances']]),
                hidden=dict(normalized=hidden['normalized'], valid_instances=hidden['valid_instances'],
                            costs=[r['cost'] for r in hidden['instances']],
                            errors=[r.get('error') for r in hidden['instances']]),
                reads_before=len(self.reads), posts_before=len(self.posts)))
            return scoring.public_view(public, trial.public_reference)
        if name == 'read_record' and self.goal is not None:
            if not self.candidates:
                self.reads.append(dict(time=time.time(), refused=True, returned=[]))
                return {'error': 'Refused: evaluate your own first candidate before reading the record.'}
            own = self.goal.principal(self.name)
            found = [item for item in self.goal.read(self.name)
                     if item['author'] != own and item['contribution'] not in self.seen]
            found.sort(key=lambda item: item['finding'].get('posted_at', 0))
            out = []
            for item in found:
                self.seen.add(item['contribution'])
                finding = dict(item['finding'])
                finding['posted_at'] = round(finding.get('posted_at', 0) - trial.started, 1)
                out.append(finding)
            self.reads.append(dict(time=time.time(), refused=False, returned=[i['contribution'] for i in found],
                                   candidates_before=len(self.candidates)))
            return {'new_findings': out, 'count': len(out)}
        if name == 'post_finding' and self.goal is not None:
            latest = self.candidates[-1] if self.candidates else None
            finding = dict(agent=self.name, seq=len(self.posts), posted_at=time.time(),
                           summary=str(args.get('summary', ''))[:3000],
                           public_score=round(latest['public']['normalized'], 6) if latest else None,
                           source_sha256=latest['sha256'] if latest else None)
            if args.get('include_source') and latest is not None:
                finding['source'] = latest['source'][:8000]
                finding['source_truncated'] = len(latest['source']) > 8000
            try:
                receipt = self.goal.publish(self.name, finding)
            except (ValueError, RuntimeError) as error:
                return {'error': 'publish failed: ' + str(error)[:200]}
            event = receipt.get('recorded', {}).get('event')
            self.posts.append(dict(time=time.time(), event=event, finding=finding))
            return {'posted': event}
        return {'error': 'unknown tool'}

    def compact_history(self):
        """Replace the oldest tool outputs once the conversation is very long; same rule in every arm."""
        from luna import input_token_bound
        history = self.session.history
        while input_token_bound(self.session.payload()) > CONFIG['history_token_soft_limit']:
            outputs = [i for i, item in enumerate(history) if item.get('type') == 'function_call_output'
                       and not item['output'].startswith('"[earlier tool output omitted')]
            if len(outputs) <= 8:
                break
            history[outputs[0]]['output'] = json.dumps('[earlier tool output omitted to bound context]')
            self.trace.append(dict(event='compacted', index=outputs[0]))

    def run(self):
        started = time.time()
        while self.turns < CONFIG['max_turns']:
            self.compact_history()
            try:
                calls, visible, status = self.session.request()
            except BudgetStop as error:
                self.trace.append(dict(event='budget_stop', reason=str(error), time=time.time()))
                break
            except TransportFailure as error:
                self.failed = str(error)
                self.trace.append(dict(event='transport_failure', error=str(error), time=time.time()))
                break
            self.turns += 1
            self.trace.append(dict(event='response', turn=self.turns, text=visible, status=status, time=time.time(),
                                   calls=[dict(name=n, arguments=a) for _, n, a in calls],
                                   cost=self.session.requests[-1]['cost'], usage=self.session.requests[-1]['usage']))
            if not calls:
                if status == 'incomplete':
                    self.session.user('Your response was cut off at the output limit. Continue, using a tool call.')
                    continue
                remaining = self.allowance - self.session.spent()
                if not self.nudged and remaining > CONFIG['nudge_fraction'] * self.allowance:
                    self.nudged = True
                    self.session.user(f'You still have ${remaining:.2f} of your ${self.allowance:.2f} allowance. '
                                      'You may continue improving your candidate, or reply "done" to stop.')
                    self.trace.append(dict(event='nudge', remaining=remaining, time=time.time()))
                    continue
                break
            results = []
            for ident, name, args in calls:
                try:
                    value = self.tool(name, args)
                except (ValueError, KeyError, TypeError) as error:
                    value = {'error': str(error)[:300]}
                value['spend'] = {'spent_usd': round(self.session.spent(), 4), 'allowance_usd': self.allowance}
                results.append((ident, value))
            self.session.results(results)
            self.trace.append(dict(event='tool_results', results=[v for _, v in results], time=time.time()))
            self.trial.checkpoint()
        self.elapsed = time.time() - started
        return self

    def summary(self):
        return dict(name=self.name, label=self.label, allowance_usd=self.allowance, spent_usd=self.session.spent(),
                    turns=self.turns, elapsed_seconds=getattr(self, 'elapsed', None), failed=self.failed,
                    nudged=self.nudged, requests=self.session.requests, candidates=self.candidates,
                    reads=self.reads, posts=self.posts, trace=self.trace)


def select(candidates):
    """The frozen selector: best public score, earliest on ties. Hidden scores never enter."""
    if not candidates:
        return None
    return min(candidates, key=lambda c: (c['public']['normalized'], c['time']))


class Trial:
    def __init__(self, phase, task, arm, ledger, output, daemon=None):
        self.phase, self.task, self.arm, self.ledger, self.daemon = phase, task, arm, ledger, daemon
        self.phase_ceiling = CONFIG[phase]['ceiling_usd']
        self.path = output / 'trials' / phase / f'{task}-{arm}.json'
        seed = CONFIG['study_seed']
        self.public, self.hidden = instances(seed, task, 'public'), instances(seed, task, 'hidden')
        self.public_reference, self.hidden_reference = reference_costs(task, self.public), reference_costs(task, self.hidden)
        self.agents = []
        self.lock = threading.Lock()
        self.goal = None
        self.started = None

    def checkpoint(self):
        with self.lock:
            save(self.path, self.result(final=False))

    def result(self, final):
        agents = [a.summary() for a in self.agents]
        candidates = [c for a in self.agents for c in a.candidates]
        selected = select(candidates)
        oracle = min(candidates, key=lambda c: (c['hidden']['normalized'], c['time'])) if candidates else None
        return dict(phase=self.phase, task=self.task, arm=self.arm, final=final, started=self.started,
                    finished=time.time() if final else None, k=len(self.agents),
                    allowance_per_agent_usd=self.agents[0].allowance if self.agents else None,
                    spent_usd=sum(a.session.spent() for a in self.agents),
                    selected=None if selected is None else dict(agent=selected['agent'], index=selected['index'],
                                                                 public=selected['public']['normalized'],
                                                                 hidden=selected['hidden']['normalized']),
                    oracle_hidden=None if oracle is None else oracle['hidden']['normalized'],
                    goal=None if self.goal is None else dict(id=self.goal.id, principals=
                                                            {n: self.goal.principal(n) for n in self.goal.roles},
                                                            receipts=self.goal.receipts),
                    agents=agents)

    def run(self):
        count = CONFIG['arms'][self.arm]
        allowance = CONFIG['task_arm_allowance_usd'] / count
        names = [f'agent-{i}' for i in range(count)]
        if self.arm == 'shared':
            self.goal = self.daemon.goal(f'{self.phase}-{self.task}', names)
        self.started = time.time()
        self.agents = [Agent(self, name, allowance, self.goal) for name in names]
        print(json.dumps(dict(started=f'{self.phase}/{self.task}/{self.arm}', agents=count, allowance=allowance)), flush=True)
        with ThreadPoolExecutor(max_workers=count) as pool:
            list(pool.map(lambda a: a.run(), self.agents))
        value = self.result(final=True)
        save(self.path, value)
        print(json.dumps(dict(finished=f'{self.phase}/{self.task}/{self.arm}', spent=round(value['spent_usd'], 4),
                              selected=value['selected'], oracle=value['oracle_hidden'])), flush=True)
        return value


def gate(trials):
    """Calibration gate, as relaxed post hoc by Amendment A2. Returns (passed, reasons, facts)."""
    reasons, facts = [], {}
    failures = sum(1 for t in trials for a in t['agents'] if a['failed'])
    facts['transport_failures'] = failures
    if failures > CONFIG['gate']['max_transport_failures']:
        reasons.append(f'{failures} transport failures')
    spreads = {}
    for t in trials:
        key = f"{t['task']}/{t['arm']}"
        valid = sum(1 for a in t['agents'] if any(c['public']['valid_instances'] == PUBLIC_INSTANCES for c in a['candidates']))
        facts[key + '/agents_with_valid_candidate'] = valid
        needed = 1 if t['arm'] == 'solo' else CONFIG['gate']['min_agents_with_valid_candidate']
        if valid < needed:
            reasons.append(f'{key}: only {valid} agents produced a fully valid candidate')
        if t['arm'] == 'independent':
            finals = [select(a['candidates'])['hidden']['normalized'] for a in t['agents'] if a['candidates']]
            spread = (max(finals) - min(finals)) if len(finals) >= 2 else 0.0
            facts[key + '/final_hidden_scores'] = finals
            facts[key + '/hidden_range'] = spread
            spreads[key] = spread
        if t['arm'] == 'shared':
            posts = sum(len(a['posts']) for a in t['agents'])
            delivered = sum(1 for a in t['agents'] for r in a['reads'] if not r['refused'] and r['returned'])
            facts[key + '/findings_posted'] = posts
            facts[key + '/reads_returning_peer_findings'] = delivered
            if posts < 1 or delivered < 1:
                reasons.append(f'{key}: record delivery not demonstrated (posts={posts}, delivered reads={delivered})')
    minimum = CONFIG['gate']['min_hidden_range']
    facts['independent_trials_with_hidden_range'] = sorted(k for k, s in spreads.items() if s >= minimum)
    facts['independent_trials_below_hidden_range'] = sorted(k for k, s in spreads.items() if s < minimum)
    if spreads and not facts['independent_trials_with_hidden_range']:
        reasons.append('no independent trial reached hidden range ' + str(minimum) + ': '
                       + ', '.join(f'{k} {s:.4f}' for k, s in sorted(spreads.items())))
    best = min((c['hidden']['normalized'] for t in trials for a in t['agents'] for c in a['candidates']), default=None)
    facts['best_hidden_any'] = best
    if best is None or best >= 1.0:
        reasons.append('no calibration candidate beat the reference on hidden instances')
    return not reasons, reasons, facts


def source_hashes():
    return {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(HERE.glob('*.py'))}


def instances_digest(items):
    return hashlib.sha256(json.dumps(items, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=HERE.parents[2] / 'target/debug/locust')
    parser.add_argument('--phase', choices=['calibration', 'scored', 'all'], default='all')
    args = parser.parse_args()
    output = args.output
    output.mkdir(parents=True, exist_ok=True)
    ledger = Ledger(output / 'ledger.json', CONFIG['study_ceiling_usd'])
    binary = copy_binary(args.binary, output / 'bin' / 'locust')
    # Instances regenerate exactly from the study seed and problems.py (its hash is in source_sha256),
    # so the manifest records their digests and reference costs rather than the instances themselves.
    frozen = {t: {split: instances(CONFIG['study_seed'], t, split) for split in ('public', 'hidden')}
              for t in DEVELOPMENT + HELD_OUT}
    manifest = dict(config=CONFIG, source_sha256=source_hashes(), python=sys.version, started=time.time(),
                    instances={t: {split: dict(count=len(items), sha256=instances_digest(items))
                                   for split, items in splits.items()} for t, splits in frozen.items()},
                    reference_costs={t: {split: reference_costs(t, items) for split, items in splits.items()}
                                     for t, splits in frozen.items()})
    save(output / 'manifest.json', manifest)

    with Daemon(output, binary) as daemon:
        manifest['binary'] = daemon.binary_metadata()
        save(output / 'manifest.json', manifest)
        try:
            if args.phase in ('calibration', 'all'):
                trials = []
                for task, arms in CONFIG['calibration']['tasks']:
                    with ThreadPoolExecutor(max_workers=len(arms)) as pool:
                        trials.extend(pool.map(lambda arm: run_or_load('calibration', task, arm, ledger, output, daemon), arms))
                passed, reasons, facts = gate(trials)
                save(output / 'calibration-gate.json', dict(passed=passed, reasons=reasons, facts=facts,
                                                            spent_usd=ledger.spent(prefix='calibration/'), time=time.time()))
                print(json.dumps(dict(gate='passed' if passed else 'FAILED', reasons=reasons, facts=facts)), flush=True)
                if not passed:
                    return 1
            if args.phase in ('scored', 'all'):
                verdict = json.loads((output / 'calibration-gate.json').read_text())
                if not verdict['passed']:
                    print('calibration gate did not pass; refusing scored phase', flush=True)
                    return 1
                tasks = list(CONFIG['scored']['tasks'])
                random.Random(CONFIG['study_seed']).shuffle(tasks)
                arms = CONFIG['scored']['arms']

                def run_task(task):
                    with ThreadPoolExecutor(max_workers=len(arms)) as pool:
                        return list(pool.map(lambda arm: run_or_load('scored', task, arm, ledger, output, daemon), arms))
                with ThreadPoolExecutor(max_workers=CONFIG['scored']['concurrent_tasks']) as pool:
                    list(pool.map(run_task, tasks))
        finally:
            manifest['finished'] = time.time()
            manifest['spent_usd'] = ledger.spent()
            save(output / 'manifest.json', manifest)
            events = Path(daemon.daemon.events)
            if events.exists():
                shutil.copy2(events, output / 'daemon-events.jsonl')
    print(json.dumps(dict(done=True, spent=round(ledger.spent(), 4))), flush=True)
    return 0


def run_or_load(phase, task, arm, ledger, output, daemon):
    trial = Trial(phase, task, arm, ledger, output, daemon)
    if trial.path.exists():
        existing = json.loads(trial.path.read_text())
        if existing.get('final'):
            print(json.dumps(dict(loaded=f'{phase}/{task}/{arm}')), flush=True)
            return existing
    return trial.run()


if __name__ == '__main__':
    sys.exit(main())
