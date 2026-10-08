import io
import json
from pathlib import Path
import random
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import analyze  # noqa: E402
import luna  # noqa: E402
import problems  # noqa: E402
import run  # noqa: E402
import scoring  # noqa: E402


class Problems(unittest.TestCase):
    def test_every_family_is_consistent(self):
        for name, problem in problems.FAMILIES.items():
            with self.subTest(family=name):
                a = problem.generate(random.Random(7))
                b = problem.generate(random.Random(7))
                self.assertEqual(a, b, 'generation must be deterministic in the seed')
                reference = problem.reference(a)
                problem.validate(a, reference)
                fallback = problem.fallback(a)
                problem.validate(a, fallback)
                self.assertLessEqual(problem.cost(a, reference), problem.cost(a, fallback))
                self.assertGreater(problem.cost(a, fallback), 0)
                with self.assertRaises((ValueError, TypeError, IndexError, KeyError)):
                    problem.validate(a, 'not a solution')
                json.dumps(a), json.dumps(reference)
                self.assertTrue(problem.shape.strip(), 'every family states its generator for the prompt')

    def test_qap_reference_delta_matches_full_recomputation(self):
        qap = problems.FAMILIES['qap']
        for seed in range(4):
            rng = random.Random(seed)
            n = rng.randint(6, 14)

            def symmetric():
                m = [[0] * n for _ in range(n)]
                for i in range(n):
                    for j in range(i + 1, n):
                        m[i][j] = m[j][i] = rng.randint(0, 20)
                return m
            inst = {'flow': symmetric(), 'distance': symmetric()}
            p = list(range(n))
            current = qap.cost(inst, p)
            improved = True
            while improved:
                improved = False
                for i in range(n):
                    for j in range(i + 1, n):
                        p[i], p[j] = p[j], p[i]
                        trial = qap.cost(inst, p)
                        if trial < current:
                            current, improved = trial, True
                        else:
                            p[i], p[j] = p[j], p[i]
            self.assertEqual(qap.reference(inst), p)

    def test_validators_reject_specific_violations(self):
        tsp = problems.FAMILIES['tsp']
        inst = tsp.generate(random.Random(1))
        with self.assertRaises(ValueError):
            tsp.validate(inst, [0] * len(inst['points']))
        col = problems.FAMILIES['coloring']
        inst = col.generate(random.Random(1))
        with self.assertRaises(ValueError):
            col.validate(inst, [0] * inst['n'])
        mkp = problems.FAMILIES['mkp']
        inst = mkp.generate(random.Random(1))
        with self.assertRaises(ValueError):
            mkp.validate(inst, list(range(len(inst['values']))))
        cover = problems.FAMILIES['setcover']
        inst = cover.generate(random.Random(1))
        with self.assertRaises(ValueError):
            cover.validate(inst, [])

    def test_instance_splits_differ_and_are_frozen(self):
        public = problems.instances('seed', 'tsp', 'public')
        hidden = problems.instances('seed', 'tsp', 'hidden')
        self.assertEqual(len(public), problems.PUBLIC_INSTANCES)
        self.assertEqual(len(hidden), problems.HIDDEN_INSTANCES)
        self.assertNotIn(public[0], hidden)
        self.assertEqual(public, problems.instances('seed', 'tsp', 'public'))
        self.assertNotEqual(public, problems.instances('other', 'tsp', 'public'))


class Scoring(unittest.TestCase):
    family = 'vertex_cover'

    def setUp(self):
        self.public = problems.instances('t', self.family, 'public')[:2]
        self.reference = problems.reference_costs(self.family, self.public)

    def test_crash_and_invalid_count_as_fallback(self):
        crash = scoring.score(self.family, 'def solve(i):\n    raise RuntimeError("x")', self.public, self.reference)
        self.assertEqual(crash['valid_instances'], 0)
        self.assertEqual(crash['total_cost'], sum(scoring.FAMILIES[self.family].cost(i, scoring.FAMILIES[self.family].fallback(i)) for i in self.public))
        invalid = scoring.score(self.family, 'def solve(i):\n    return []', self.public, self.reference)
        self.assertEqual(invalid['valid_instances'], 0)
        self.assertIn('invalid solution', invalid['instances'][0]['error'])

    def test_trivial_candidate_scores_worse_than_reference(self):
        trivial = scoring.score(self.family, 'def solve(i):\n    return list(range(i["n"]))', self.public, self.reference)
        self.assertEqual(trivial['valid_instances'], 2)
        self.assertGreater(trivial['normalized'], 1.0)

    def test_cpu_overrun_is_killed(self):
        busy = scoring.score(self.family, 'def solve(i):\n    while True:\n        pass', self.public[:1], self.reference[:1])
        self.assertFalse(busy['instances'][0]['valid'])
        self.assertIn('CPU', busy['instances'][0]['error'])

    def test_public_view_has_no_hidden_fields(self):
        result = scoring.score(self.family, 'def solve(i):\n    return list(range(i["n"]))', self.public, self.reference)
        view = scoring.public_view(result, self.reference)
        self.assertEqual(set(view), {'public_normalized_score', 'instances'})
        self.assertEqual(set(view['instances'][0]), {'cost', 'reference_cost', 'valid', 'cpu_seconds'})

    def test_probe_returns_output_and_blocks_network(self):
        out = scoring.probe('print(len(INSTANCES)); import urllib.request\ntry:\n    urllib.request.urlopen("http://example.com", timeout=3)\nexcept Exception as e:\n    print("blocked", type(e).__name__)', self.public)
        self.assertIn('2', out['stdout'])
        self.assertIn('blocked', out['stdout'])


class LedgerAndSession(unittest.TestCase):
    def test_ceilings_and_settlement(self):
        with tempfile.TemporaryDirectory() as d:
            ledger = luna.Ledger(Path(d) / 'ledger.json', ceiling=1.0)
            item = ledger.reserve('calibration/t/a/x', 'calibration', 0.5, phase_ceiling=0.6)
            with self.assertRaises(luna.BudgetStop):
                ledger.reserve('calibration/t/a/y', 'calibration', 0.2, phase_ceiling=0.6)  # phase
            with self.assertRaises(luna.BudgetStop):
                ledger.reserve('scored/t/a/y', 'scored', 0.6, phase_ceiling=5)  # study
            ledger.settle(item, {'usage': {'input_tokens': 1_000_000, 'output_tokens': 100_000}, 'model': luna.MODEL, 'id': 'r'})
            self.assertAlmostEqual(item['cost'], 0.125 + 0.05)
            self.assertAlmostEqual(ledger.spent(), 0.175)
            failed = ledger.reserve('scored/t/a/z', 'scored', 0.3, phase_ceiling=5)
            ledger.fail(failed, 'HTTP 503')
            self.assertAlmostEqual(ledger.spent(), 0.475)  # reservation kept
            self.assertEqual(ledger.failures, 1)
            reloaded = luna.Ledger(Path(d) / 'ledger.json', ceiling=1.0)
            self.assertAlmostEqual(reloaded.spent(), 0.475)

    def test_session_parses_calls_and_discards_incomplete(self):
        response = {'id': 'resp', 'model': luna.MODEL, 'status': 'completed',
                    'usage': {'input_tokens': 100, 'output_tokens': 50},
                    'output': [{'type': 'reasoning', 'encrypted_content': 'x'},
                               {'type': 'function_call', 'call_id': 'c1', 'name': 'evaluate', 'arguments': '{"source": "def solve(i): return []"}'},
                               {'type': 'function_call', 'call_id': 'c2', 'name': 'evaluate', 'arguments': '{"source": "def sol'},
                               {'type': 'message', 'content': [{'type': 'output_text', 'text': 'trying'}]}]}
        captured = {}

        class Opener:
            def __init__(self, request, timeout):
                captured['body'] = json.loads(request.data)

            def __enter__(self):
                return io.StringIO(json.dumps(response))

            def __exit__(self, *a):
                return False
        with tempfile.TemporaryDirectory() as d:
            import os
            os.environ.setdefault('OPENAI_API_KEY', 'test-key')
            ledger = luna.Ledger(Path(d) / 'l.json', ceiling=1.0)
            session = luna.Session('sys', run.BASE_TOOLS, ledger, 'scored/t/solo/agent-0', 'scored', 1.0, 0.5, opener=Opener)
            session.user('hello')
            calls, visible, status = session.request()
        self.assertEqual([c[1] for c in calls], ['evaluate'])
        self.assertIn('[incomplete tool call discarded: evaluate]', visible)
        self.assertEqual(captured['body']['model'], luna.MODEL)
        self.assertEqual(captured['body']['reasoning'], {'effort': 'high'})
        self.assertFalse(captured['body']['store'])
        self.assertTrue(all(t['strict'] for t in captured['body']['tools']))
        self.assertEqual(len(session.history), 4)  # user + reasoning + one call + message
        self.assertAlmostEqual(ledger.spent(), (100 * 0.125 + 50 * 0.5) / 1e6)


def candidate(agent, public, hidden, t):
    return dict(agent=agent, index=0, time=t, public={'normalized': public, 'valid_instances': 5},
                hidden={'normalized': hidden, 'valid_instances': 10}, sha256='', source='')


class Selection(unittest.TestCase):
    def test_selector_uses_public_only_and_earliest_tie(self):
        chosen = run.select([candidate('a', 0.9, 0.5, 1), candidate('b', 0.8, 0.99, 2), candidate('c', 0.8, 0.1, 3)])
        self.assertEqual(chosen['agent'], 'b')
        self.assertIsNone(run.select([]))


class Gate(unittest.TestCase):
    def trial(self, task, arm, finals, posts=0, reads=0, failed=0):
        agents = []
        for i, score in enumerate(finals):
            cands = [candidate(f'agent-{i}', score, score, i)]
            agents.append(dict(name=f'agent-{i}', failed='x' if i < failed else None, candidates=cands,
                               posts=[{}] * posts, reads=[dict(refused=False, returned=['e'])] * reads, trace=[], turns=1,
                               elapsed_seconds=1, nudged=False))
        return dict(task=task, arm=arm, agents=agents, k=len(finals), spent_usd=0.1, phase='calibration', goal=None)

    def test_passes_with_spread_delivery_and_improvement(self):
        trials = [self.trial('maxcut', 'independent', [0.98, 0.99, 1.01]), self.trial('maxcut', 'shared', [0.97, 0.99, 1.0], posts=2, reads=1),
                  self.trial('vertex_cover', 'independent', [0.95, 1.0, 1.02]), self.trial('vertex_cover', 'solo', [0.99])]
        passed, reasons, facts = run.gate(trials)
        self.assertTrue(passed, reasons)
        self.assertAlmostEqual(facts['maxcut/independent/hidden_range'], 0.03)
        self.assertEqual(facts['independent_trials_below_hidden_range'], [])

    def test_a2_passes_when_one_independent_trial_has_spread(self):
        trials = [self.trial('maxcut', 'independent', [0.95, 0.953, 0.963]), self.trial('maxcut', 'shared', [0.955, 0.956, 0.96], posts=2, reads=1),
                  self.trial('vertex_cover', 'independent', [0.9475, 0.948, 0.9498]), self.trial('vertex_cover', 'solo', [0.957])]
        passed, reasons, facts = run.gate(trials)
        self.assertTrue(passed, reasons)
        self.assertEqual(facts['independent_trials_with_hidden_range'], ['maxcut/independent'])
        self.assertEqual(facts['independent_trials_below_hidden_range'], ['vertex_cover/independent'])

    def test_fails_on_saturation_or_missing_delivery(self):
        saturated = [self.trial('maxcut', 'independent', [1.0, 1.0, 1.0]), self.trial('maxcut', 'shared', [1.0, 1.0, 1.0], posts=1, reads=1),
                     self.trial('vertex_cover', 'independent', [1.0, 1.0, 1.0]), self.trial('vertex_cover', 'solo', [1.0])]
        passed, reasons, _ = run.gate(saturated)
        self.assertFalse(passed)
        self.assertTrue(any('hidden range' in r for r in reasons))
        self.assertTrue(any('beat the reference' in r for r in reasons))
        narrow = [self.trial('maxcut', 'independent', [0.95, 0.951, 0.952]), self.trial('maxcut', 'shared', [0.95, 0.96, 0.97], posts=1, reads=1),
                  self.trial('vertex_cover', 'independent', [0.9475, 0.948, 0.9498]), self.trial('vertex_cover', 'solo', [0.957])]
        passed, reasons, _ = run.gate(narrow)
        self.assertFalse(passed)
        self.assertTrue(any('no independent trial reached hidden range' in r for r in reasons))
        undelivered = [self.trial('maxcut', 'independent', [0.9, 1.0, 1.1]), self.trial('maxcut', 'shared', [0.9, 1.0, 1.1], posts=0, reads=0),
                       self.trial('vertex_cover', 'independent', [0.9, 1.0, 1.1]), self.trial('vertex_cover', 'solo', [0.9])]
        passed, reasons, _ = run.gate(undelivered)
        self.assertFalse(passed)
        self.assertTrue(any('record delivery' in r for r in reasons))
        broken = [self.trial('maxcut', 'independent', [0.9, 1.0, 1.1], failed=2)] + undelivered[1:]
        passed, reasons, _ = run.gate(broken)
        self.assertTrue(any('transport failures' in r for r in reasons))


class MatchedSpend(unittest.TestCase):
    def test_arm_spend_and_matched_selection(self):
        ledger = [dict(label='scored/tsp/independent/agent-0', reserved=0.1, cost=0.05, reserved_at=0, settled_at=10),
                  dict(label='scored/tsp/independent/agent-1', reserved=0.1, cost=0.05, reserved_at=0, settled_at=20),
                  dict(label='scored/tsp/independent/agent-2', reserved=0.1, reserved_at=0),  # failed: reservation kept
                  dict(label='scored/tsp/solo/agent-0', reserved=0.1, cost=0.09, reserved_at=0, settled_at=5)]
        self.assertAlmostEqual(analyze.arm_spend_at(ledger, 'scored/tsp/independent/', 15), 0.15)
        self.assertAlmostEqual(analyze.arm_spend_at(ledger, 'scored/tsp/independent/', 25), 0.20)
        trial = dict(phase='scored', task='tsp', arm='independent', agents=[
            dict(candidates=[candidate('agent-0', 0.95, 0.96, 11)]),
            dict(candidates=[candidate('agent-1', 0.90, 0.97, 21)])])
        rows = analyze.matched(trial, ledger, [0.16, 0.30])
        self.assertEqual(rows['0.16']['agent'], 'agent-0')
        self.assertEqual(rows['0.30']['agent'], 'agent-1')


class FakeGoal:
    def __init__(self):
        self.store = []
        self.roles = {'agent-0': {}, 'agent-1': {}}

    def principal(self, agent):
        return 'principal-' + agent

    def publish(self, agent, finding):
        self.store.append(dict(contribution=f'event-{len(self.store)}', author=self.principal(agent), finding=finding))
        return {'recorded': {'event': f'event-{len(self.store) - 1}'}}

    def read(self, agent):
        return list(self.store)


class SharedArmTools(unittest.TestCase):
    def test_read_refused_until_first_candidate_then_filters_own(self):
        with tempfile.TemporaryDirectory() as d:
            import os
            os.environ.setdefault('OPENAI_API_KEY', 'test-key')
            ledger = luna.Ledger(Path(d) / 'l.json', ceiling=1.0)
            trial = run.Trial('calibration', 'vertex_cover', 'shared', ledger, Path(d))
            trial.public, trial.hidden = trial.public[:1], trial.hidden[:1]
            trial.public_reference, trial.hidden_reference = trial.public_reference[:1], trial.hidden_reference[:1]
            trial.started = time.time()
            goal = FakeGoal()
            goal.publish('agent-1', dict(agent='agent-1', seq=0, posted_at=time.time(), summary='peer finding', public_score=0.9))
            agent = run.Agent(trial, 'agent-0', 0.14, goal)
            trial.agents = [agent]
            refused = agent.tool('read_record', {})
            self.assertIn('Refused', refused['error'])
            view = agent.tool('evaluate', {'source': 'def solve(i):\n    return list(range(i["n"]))'})
            self.assertIn('public_normalized_score', view)
            self.assertEqual(len(agent.candidates), 1)
            self.assertIn('normalized', agent.candidates[0]['hidden'])
            posted = agent.tool('post_finding', {'summary': 'trivial cover', 'include_source': True})
            self.assertTrue(posted['posted'].startswith('event-'))
            self.assertEqual(goal.store[-1]['finding']['public_score'], round(agent.candidates[0]['public']['normalized'], 6))
            self.assertIn('return list(range', goal.store[-1]['finding']['source'])
            got = agent.tool('read_record', {})
            self.assertEqual(got['count'], 1)
            self.assertEqual(got['new_findings'][0]['summary'], 'peer finding')
            again = agent.tool('read_record', {})
            self.assertEqual(again['count'], 0)
            self.assertEqual(agent.candidates[0]['reads_before'], 1)  # the refused read counts as an attempt
            self.assertEqual(len([r for r in agent.reads if r['refused']]), 1)
            self.assertEqual(sum(len(r['returned']) for r in agent.reads), 1)

    def test_solo_arm_has_no_record_tools(self):
        with tempfile.TemporaryDirectory() as d:
            import os
            os.environ.setdefault('OPENAI_API_KEY', 'test-key')
            ledger = luna.Ledger(Path(d) / 'l.json', ceiling=1.0)
            trial = run.Trial('calibration', 'vertex_cover', 'solo', ledger, Path(d))
            agent = run.Agent(trial, 'agent-0', 0.42)
            self.assertEqual([t['name'] for t in agent.session.tools], ['run_python', 'evaluate'])
            self.assertEqual(agent.tool('read_record', {}), {'error': 'unknown tool'})
            self.assertIn('alone', agent.session.history[0]['content'])

    def test_prompt_states_generator_and_keeps_instances_out(self):
        public = problems.instances('t', 'tsp', 'public')
        prompt = run.task_prompt('tsp', 'solo', 0.42, public, problems.reference_costs('tsp', public))
        self.assertIn(problems.FAMILIES['tsp'].shape, prompt)
        self.assertIn('INSTANCES', prompt)
        self.assertNotIn(json.dumps(public[0]['points'][:3], separators=(',', ':'))[1:-1], prompt)
        self.assertLess(len(prompt), 4000)


class RecordNames(unittest.TestCase):
    def test_every_study_trial_yields_valid_distinct_principal_names(self):
        import record
        names = set()
        for phase in ('calibration', 'scored'):
            for task in problems.DEVELOPMENT + problems.HELD_OUT:
                for i in range(run.CONFIG['arms']['shared']):
                    name = record.principal_name(f'{phase}-{task}', f'agent-{i}')
                    self.assertRegex(name, record.PRINCIPAL_NAME)
                    self.assertNotIn(name, names)
                    names.add(name)
        self.assertEqual(record.principal_name('scored-unrelated_machines', 'agent-2'), 'scored-unrelated-machines-a2')
        self.assertEqual(record.principal_name('scored-coloring', 'agent-0'), 'scored-coloring-agent-0')
        with self.assertRaises(ValueError):
            record.principal_name('x' * 40, 'agent-0')


class Manifest(unittest.TestCase):
    def test_instance_digest_is_stable_and_order_sensitive(self):
        items = problems.instances('t', 'mkp', 'public')
        self.assertEqual(run.instances_digest(items), run.instances_digest(json.loads(json.dumps(items))))
        self.assertNotEqual(run.instances_digest(items), run.instances_digest(list(reversed(items))))


if __name__ == '__main__':
    unittest.main()
