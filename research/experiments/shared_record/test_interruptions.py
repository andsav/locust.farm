"""Interrupted-study retention and C/P request-isolation checks, entirely offline."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import analyze
import luna
import protocol
import run
from test_record import FakeGoal
from test_runner import public_result, small_manifest


def fake_score(family, source, *args, **kwargs):
    return public_result(source)


class InterruptionRetention(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output, self.manifest = small_manifest(self.temp.name)
        self.block = self.manifest['blocks'][0]
        self.ledger = luna.Ledger(self.output / 'ledger.json', ceiling=100)
        self.ledger.save()
        protocol.save(self.output / 'execution.json', {'status': 'running'})
        self.forbid_network = patch('urllib.request.urlopen', side_effect=AssertionError('network forbidden'))
        self.forbid_network.start()
        self.addCleanup(self.forbid_network.stop)

    def prefix(self, name):
        trial = run.Trial(self.manifest, self.block, name, self.output, self.ledger, None,
                          transport_factory=run.ScriptedTransport, prefix_kind=name)
        with patch('run.scoring.score', side_effect=fake_score):
            trial.run(fraction=.5 if name == 'delayed' else .25)
        self.assertFalse(trial.error)
        self.assertTrue(trial.result()['candidates'])
        return trial

    def load(self, arm):
        return json.loads((self.output / f'trials/{self.block["id"]}/{arm}.json').read_text())

    def finish_and_analyze(self):
        before = copy.deepcopy(self.ledger.entries)
        run.finalize_interrupted(self.output)
        self.assertEqual(json.loads((self.output / 'ledger.json').read_text()), before)
        self.assertEqual(protocol.verify_seal(self.output), self.manifest)
        with patch('analyze.scoring.score', side_effect=fake_score):
            analyze.offline_hidden(self.output)
        report = analyze.analyze(self.output)
        self.assertEqual(len(report['trials']), len(self.manifest['arms']))
        return report

    def test_missing_matched_branches_retain_correct_completed_prefix_and_cost(self):
        prefixes = {name: self.prefix(name).result(True) for name in ('early', 'delayed')}
        report = self.finish_and_analyze()
        for policy in ('E', 'D', 'F'):
            prefix = prefixes['delayed' if policy == 'D' else 'early']
            expected_cost = self.ledger.snapshot(prefix['request_ids'])['committed_cost']
            self.assertGreater(expected_cost, 0)
            for kind in ('C', 'P'):
                arm = kind + '_' + policy
                trial = self.load(arm)
                self.assertEqual(trial['candidates'], prefix['candidates'])
                self.assertEqual(trial['selected'], prefix['selected'])
                self.assertEqual(set(trial['request_ids']), set(prefix['request_ids']))
                self.assertEqual(len(trial['request_ids']), len(set(trial['request_ids'])))
                self.assertEqual(set(trial['prefix_candidate_ids']), {c['id'] for c in prefix['candidates']})
                summary = report['trials'][self.block['id'] + '/' + arm]
                self.assertAlmostEqual(summary['spending']['attributed_cost'], expected_cost)
                self.assertFalse(summary['fallback'])
        independent = self.load('I')
        self.assertEqual(independent['candidates'], [])
        self.assertEqual(independent['request_ids'], [])
        self.assertIsNone(independent['selected'])
        summary = report['trials'][self.block['id'] + '/I']
        self.assertTrue(summary['fallback'])
        self.assertEqual(summary['spending']['attributed_cost'], 0)
        self.assertAlmostEqual(report['billing']['total'], self.ledger.snapshot()['committed_cost'])

    def test_partial_branch_retains_prefix_new_candidate_and_post_checkpoint_reservation_once(self):
        prefix = self.prefix('early')
        trial = run.Trial(self.manifest, self.block, 'C_E', self.output, self.ledger, None,
                          transport_factory=run.ScriptedTransport)
        trial.agents = [run.Agent(trial, f'agent-{i}', .2, prefix=agent)
                        for i, agent in enumerate(prefix.agents)]
        source = '# completed continuation\ndef solve(i): return list(range(i["n"]))'
        with patch('run.scoring.score', side_effect=fake_score):
            trial.agents[0].tool('evaluate', {'source': source, 'parent_id': None})
        trial.checkpoint()
        saved = self.load('C_E')
        # A dispatch is durable in the ledger before the next trial checkpoint.
        pending = self.ledger.reserve(f'{self.block["id"]}/C_E/agent-0', 'scripted', .012, 100,
                                      model_config=self.manifest['models']['luna'], payload={},
                                      input_bound=10, max_output_tokens=10, session_id='pending-fixture')
        report = self.finish_and_analyze()
        resumed = self.load('C_E')
        self.assertEqual(resumed['candidates'], saved['candidates'])
        self.assertEqual(resumed['selected'], saved['selected'])
        expected_ids = set(saved['request_ids']) | {pending['id']}
        self.assertEqual(set(resumed['request_ids']), expected_ids)
        self.assertEqual(len(resumed['request_ids']), len(expected_ids))
        prefix_ids = set(prefix.result()['request_ids'])
        self.assertEqual(set(self.load('P_E')['request_ids']), prefix_ids)
        self.assertEqual(self.load('P_E')['candidates'], prefix.result()['candidates'])
        summary = report['trials'][self.block['id'] + '/C_E']
        self.assertAlmostEqual(summary['spending']['attributed_cost'],
                               self.ledger.snapshot(expected_ids)['committed_cost'])
        self.assertAlmostEqual(summary['spending']['uncertain_reservations'], .012)
        self.assertEqual(summary['spending']['pending'], 1)
        first_seal = (self.output / 'seal.json').read_bytes()
        run.finalize_interrupted(self.output)
        self.assertEqual((self.output / 'seal.json').read_bytes(), first_seal)

    def test_prefix_interruption_keeps_its_late_pending_dispatch_in_each_assigned_branch(self):
        prefix = self.prefix('early')
        checkpoint = prefix.result()
        protocol.save(prefix.path, checkpoint)
        pending = self.ledger.reserve(f'{self.block["id"]}/early/agent-0', 'scripted', .013, 100,
                                      model_config=self.manifest['models']['luna'], payload={},
                                      input_bound=10, max_output_tokens=10, session_id='prefix-pending-fixture')
        report = self.finish_and_analyze()
        expected_ids = set(checkpoint['request_ids']) | {pending['id']}
        for arm in ('C_E', 'P_E', 'C_F', 'P_F'):
            trial = self.load(arm)
            self.assertEqual(trial['candidates'], checkpoint['candidates'])
            self.assertEqual(set(trial['request_ids']), expected_ids)
            self.assertEqual(len(trial['request_ids']), len(expected_ids))
            spending = report['trials'][self.block['id'] + '/' + arm]['spending']
            self.assertAlmostEqual(spending['attributed_cost'], self.ledger.snapshot(expected_ids)['committed_cost'])
            self.assertAlmostEqual(spending['uncertain_reservations'], .013)
        self.assertAlmostEqual(report['billing']['total'], self.ledger.snapshot()['committed_cost'])


class ReviewRequestIsolation(unittest.TestCase):
    def test_scripted_model_payloads_keep_execution_labels_out_of_every_continuation(self):
        class Daemon:
            def goal(self, title, agents):
                return FakeGoal(title, agents)
        for strategy in ('continue', 'verify', 'restart'):
            with self.subTest(strategy=strategy), tempfile.TemporaryDirectory() as directory:
                output, manifest = small_manifest(directory)
                manifest['continuation'] = dict(luna=strategy, frontier=strategy)
                block = manifest['blocks'][0]
                ledger = luna.Ledger(output / 'ledger.json', ceiling=100)
                prefixes = {}
                with patch('urllib.request.urlopen', side_effect=AssertionError('network forbidden')):
                    with patch('run.scoring.score', side_effect=fake_score):
                        for name, fraction in (('early', .25), ('delayed', .5)):
                            prefixes[name] = run.Trial(manifest, block, name, output, ledger, None,
                                transport_factory=run.ScriptedTransport, prefix_kind=name).run(fraction=fraction)
                        for arm, settings in manifest['arms'].items():
                            prefix = (prefixes['delayed' if settings['policy'] == 'D' else 'early']
                                      if settings['kind'] in ('C', 'P') else None)
                            trial = run.Trial(manifest, block, arm, output, ledger, Daemon(),
                                              transport_factory=run.ScriptedTransport).run(prefix=prefix)
                            self.assertIsNone(trial.error)
                            self.assertTrue(all(a.failed is None for a in trial.agents))
                source_results = private_reviews = review_packets = 0
                for entry in ledger.entries:
                    # Inspect the actual dispatched payload, including tool results,
                    # source retrieval and private-review candidate serialization.
                    text = json.dumps(entry['payload'])
                    self.assertNotIn(block['id'], text)
                    self.assertNotIn(entry['label'], text)
                    for label in [*manifest['arms'], 'early', 'delayed']:
                        self.assertNotIn(f'/{label}/', text)
                    for item in entry['payload']['input']:
                        if item.get('type') == 'function_call_output':
                            value = json.loads(item['output'])
                            if 'candidate_id' in value:
                                self.assertRegex(value['candidate_id'], r'^candidate-[0-9a-f]{32}$')
                                source_results += 'source' in value
                        elif item.get('role') == 'user':
                            content = item['content']
                            if '\nOwn evaluated artifacts:\n' in content:
                                private_reviews += 1
                                candidates = json.loads(content.split('\nOwn evaluated artifacts:\n')[1])
                                for candidate in candidates:
                                    self.assertRegex(candidate['id'], r'^candidate-[0-9a-f]{32}$')
                                    self.assertIn(candidate['agent'], ('agent-0', 'agent-1', 'agent-2'))
                            if '\nFrozen evidence:\n' in content:
                                review_packets += 1
                                packet = json.loads(content.split('\nFrozen evidence:\n')[1])
                                self.assertEqual(set(packet), {'schema', 'policy', 'candidates'})
                                self.assertIn(packet['policy'], ('E', 'D', 'F'))
                self.assertGreater(source_results, 0)
                self.assertGreater(review_packets, 0)
                self.assertEqual(private_reviews > 0, strategy != 'continue')

    def test_c_and_p_first_continuation_requests_only_change_review_visibility(self):
        with tempfile.TemporaryDirectory() as directory:
            output, manifest = small_manifest(directory)
            block = manifest['blocks'][0]
            ledger = luna.Ledger(output / 'ledger.json', ceiling=100)
            prefix = run.Trial(manifest, block, 'early', output, ledger, None,
                               transport_factory=run.ScriptedTransport, prefix_kind='early')
            captured = {}
            class Transport:
                def __init__(self, trial, agent):
                    self.key = (trial.arm, agent)
                def __call__(self, payload):
                    captured[self.key] = copy.deepcopy(payload)
                    return dict(id='scripted-review-' + '/'.join(self.key), model=payload['model'],
                                status='completed', usage={'input_tokens': 100, 'output_tokens': 1,
                                                           'input_tokens_details': {'cached_tokens': 0}}, output=[])
            class Daemon:
                def goal(self, title, agents):
                    return FakeGoal(title, agents)
            with patch('urllib.request.urlopen', side_effect=AssertionError('network forbidden')):
                with patch('run.scoring.score', side_effect=fake_score):
                    prefix.run(fraction=.25)
                    history_before = [copy.deepcopy(a.session.history) for a in prefix.agents]
                    private = run.Trial(manifest, block, 'C_E', output, ledger, Daemon(),
                                        transport_factory=Transport).run(prefix=prefix)
                    peer = run.Trial(manifest, block, 'P_E', output, ledger, Daemon(),
                                     transport_factory=Transport).run(prefix=prefix)
            self.assertIsNone(private.error)
            self.assertIsNone(peer.error)
            self.assertFalse([a.failed for a in private.agents if a.failed])
            self.assertFalse([a.failed for a in peer.agents if a.failed])
            self.assertEqual([a.session.history for a in prefix.agents], history_before)
            for index in range(3):
                name = f'agent-{index}'
                left = captured[('C_E', name)]
                right = captured[('P_E', name)]
                self.assertEqual(left['input'][:-1], history_before[index])
                self.assertEqual(right['input'][:-1], history_before[index])
                left_review = left['input'].pop()
                right_review = right['input'].pop()
                self.assertEqual(left, right)  # Includes instructions, model ID, limits and tools.
                left_prefix, left_data = left_review['content'].split('Frozen evidence:\n')
                right_prefix, right_data = right_review['content'].split('Frozen evidence:\n')
                self.assertEqual(left_prefix, right_prefix)
                self.assertEqual(left_prefix.rstrip(), run.REVIEW)
                own, shared = json.loads(left_data), json.loads(right_data)
                self.assertTrue(all(row['author'] == name for row in own['candidates']))
                self.assertEqual(own['candidates'], [row for row in shared['candidates'] if row['author'] == name])
                self.assertTrue(any(row['author'] != name for row in shared['candidates']))
                self.assertFalse(any('hidden' in row for row in shared['candidates']))


if __name__ == '__main__':
    unittest.main()
