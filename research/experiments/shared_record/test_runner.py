"""Runner boundary tests: no model network or daemon required here."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import luna
import protocol
import run
import scoring
from test_record import FakeGoal


def small_manifest(directory):
    path = Path(directory)
    binary = path / 'fixture-binary'
    binary.write_bytes(b'offline test fixture; never execute')
    spec = run.scripted_spec('test-study')
    spec['blocks'] = [dict(family='vertex_cover', pack='p0', repetition=0,
                           attempt='attempt-0', phase='scripted')]
    manifest = protocol.build_manifest(spec, binary, run.PROMPTS)
    output = path / 'study'
    protocol.freeze(output, manifest, binary)
    return output, manifest


def public_result(source, **kwargs):
    return dict(normalized=1., valid_instances=1, total_instances=1, total_cost=1., reference_total=1.,
                source_sha256=__import__('hashlib').sha256(source.encode()).hexdigest(),
                instances=[dict(valid=True, cost=1., instance_index=0, repetition=0)])


class Runner(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output, self.manifest = small_manifest(self.temp.name)
        self.block = self.manifest['blocks'][0]
        self.ledger = luna.Ledger(self.output / 'ledger.json', ceiling=100)

    def trial(self, arm='I', prefix_kind=None):
        return run.Trial(self.manifest, self.block, arm, self.output, self.ledger, None,
                         transport_factory=run.ScriptedTransport, prefix_kind=prefix_kind)

    def test_live_evaluation_has_only_public_data_and_no_hidden_call(self):
        trial = self.trial()
        agent = run.Agent(trial, 'agent-0', .2)
        source = 'def solve(i): return list(range(i["n"]))'
        with patch('run.scoring.score', return_value=public_result(source)) as score:
            result = agent.tool('evaluate', {'source': source, 'parent_id': None})
        self.assertEqual(score.call_count, 1)
        self.assertIs(score.call_args.args[2], self.block['public']['instances'])
        self.assertNotIn('hidden', agent.candidates[0])
        self.assertIn('candidate_id', result)

    def test_candidate_ids_are_unique_opaque_and_stable_in_archives(self):
        identities = set()
        arms = [(arm, None) for arm in self.manifest['arms']]
        arms += [('early', 'early'), ('delayed', 'delayed')]
        source = 'def solve(i): return list(range(i["n"]))'
        with patch('run.scoring.score', return_value=public_result(source)):
            for arm, prefix_kind in arms:
                for name in ('agent-0', 'agent-1'):
                    agent = run.Agent(self.trial(arm, prefix_kind), name, .2)
                    first = agent.tool('evaluate', {'source': source, 'parent_id': None})['candidate_id']
                    second = agent.tool('evaluate', {'source': source, 'parent_id': first})['candidate_id']
                    self.assertNotEqual(first, second, 'repeated source submissions remain distinct attempts')
                    for ident in (first, second):
                        self.assertRegex(ident, r'^candidate-[0-9a-f]{32}$')
                        self.assertNotIn(ident, identities)
                        identities.add(ident)
                    archived = json.loads(json.dumps(agent.summary()))
                    self.assertEqual(archived['id'], f'{self.block["id"]}/{arm}/{name}')
                    self.assertEqual([c['id'] for c in archived['candidates']], [first, second])
                    self.assertEqual(archived['candidates'][1]['parent_id'], first)
                    self.assertTrue(all(c['agent'] == name for c in archived['candidates']))

    def test_opaque_peer_sources_and_parent_links_keep_exact_authorization(self):
        source = 'def solve(i): return list(range(i["n"]))'
        prefix = self.trial('early', prefix_kind='early')
        original = [run.Agent(prefix, f'agent-{i}', .2) for i in range(3)]
        with patch('run.scoring.score', return_value=public_result(source)):
            for agent in original:
                agent.tool('evaluate', {'source': source, 'parent_id': None})
            donors = {a.name: a.candidates for a in original}
            peer_id = original[1].candidates[0]['id']
            branches = []
            for arm in ('C_E', 'P_E'):
                trial = self.trial(arm)
                agent = run.Agent(trial, 'agent-0', .2, prefix=original[0])
                agent.record = run.FrozenRecord(FakeGoal(arm, donors), f'{self.block["id"]}/{arm}',
                                                'E', donors).freeze()
                packet = agent.record.review(agent.name, peers=arm == 'P_E')
                self.assertTrue(all(row['candidate_id'] == row['source_reference']['candidate_id']
                                    for row in packet['candidates']))
                branches.append(agent)
            private, peer = branches
            self.assertEqual(peer.tool('get_source', {'candidate_id': peer_id}),
                             dict(candidate_id=peer_id, sha256=public_result(source)['source_sha256'],
                                  source=source))
            revised = peer.tool('evaluate', {'source': source, 'parent_id': peer_id})
            self.assertNotEqual(revised['candidate_id'], peer_id)
            self.assertEqual(peer.candidates[-1]['parent_id'], peer_id)
            for candidate_id in (peer_id, original[1].id + '/c0'):
                with self.assertRaises(PermissionError):
                    private.tool('get_source', {'candidate_id': candidate_id})
                with self.assertRaises(PermissionError):
                    private.tool('evaluate', {'source': source, 'parent_id': candidate_id})
            self.assertEqual(len(private.candidates), 1)

    def test_c_and_p_fork_exact_history_and_charge_prefix_once_each(self):
        prefix = self.trial('early', prefix_kind='early')
        agent = run.Agent(prefix, 'agent-0', .2)
        prefix.agents = [agent]
        with patch('run.scoring.score', side_effect=lambda f, s, *a, **k: public_result(s)):
            agent.run(stage_allowance=.05)
        left = run.Agent(self.trial('C_E'), 'agent-0', .2, prefix=agent)
        right = run.Agent(self.trial('P_E'), 'agent-0', .2, prefix=agent)
        self.assertEqual(left.session.history, right.session.history)
        self.assertEqual(left.session.request_ids, right.session.request_ids)
        self.assertEqual(left.session.spent(), agent.session.spent())
        self.assertEqual(right.session.spent(), agent.session.spent())
        self.assertEqual(left.candidates, agent.candidates)
        self.assertEqual(right.candidates, agent.candidates)
        original_id = agent.candidates[0]['id']
        self.assertEqual(left.prefix_candidate_ids, right.prefix_candidate_ids)
        source = agent.candidates[0]['source']
        with patch('run.scoring.score', return_value=public_result(source)):
            left_id = left.tool('evaluate', {'source': source, 'parent_id': original_id})['candidate_id']
            right_id = right.tool('evaluate', {'source': source, 'parent_id': original_id})['candidate_id']
        self.assertEqual(len({original_id, left_id, right_id}), 3)
        self.assertEqual(left.candidates[-1]['parent_id'], right.candidates[-1]['parent_id'])
        self.assertEqual(agent.candidates[0]['id'], original_id)
        left.session.user('only left')
        self.assertNotEqual(left.session.history, right.session.history)
        self.assertNotEqual(left.session.history, agent.session.history)
        self.assertEqual(len(self.ledger.entries), len(agent.session.request_ids))

    def test_independent_allowance_is_uninterrupted(self):
        observed = []
        original = luna.Session.request
        def capture(session):
            observed.append(session.allowance)
            return original(session)
        with patch.object(luna.Session, 'request', capture), patch('run.scoring.score',
                side_effect=lambda f, s, *a, **k: public_result(s)):
            trial = self.trial().run()
        self.assertTrue(observed)
        self.assertTrue(all(abs(value - .2) < 1e-12 for value in observed))
        self.assertTrue(all(a.record is None for a in trial.agents))
        self.assertTrue(all(t['name'] != 'get_source' for a in trial.agents for t in a.session.tools))

    def test_stage_allocation_does_not_change_total_allowance_in_prompt(self):
        trial = self.trial('early', prefix_kind='early')
        observed = []
        original = luna.Session.request
        def capture(session):
            observed.append(session.allowance)
            return original(session)
        with patch.object(luna.Session, 'request', capture), patch('run.scoring.score',
                side_effect=lambda f, s, *a, **k: public_result(s)):
            trial.run(fraction=.25)
        self.assertTrue(all(abs(value - .05) < 1e-12 for value in observed))
        self.assertTrue(all('allowance: $0.2' in a.initial_prompt['content'] for a in trial.agents))
        self.assertTrue(all(abs(a.session.allowance - .2) < 1e-12 for a in trial.agents))

    def test_transport_failure_does_not_cancel_other_workers(self):
        self.block['repetition'] = 1
        with patch('run.scoring.score', side_effect=lambda f, s, *a, **k: public_result(s)):
            trial = self.trial().run()
        result = trial.result(True)
        self.assertTrue(result['agents'][2]['failed'])
        self.assertTrue(result['agents'][0]['candidates'])
        self.assertTrue(result['agents'][1]['candidates'])
        self.assertGreater(self.ledger.snapshot()['unknown_reserved'], 0)

    def test_disallowed_source_returns_tool_error_without_hidden_read(self):
        trial = self.trial()
        agent = run.Agent(trial, 'agent-0', .2)
        self.assertEqual(agent.tool('get_source', {'candidate_id': 'guessed'}), {'error': 'tool is not available'})

    def test_execution_requires_explicit_paid_authority_before_network(self):
        with patch('urllib.request.urlopen', side_effect=AssertionError('network forbidden')):
            with self.assertRaisesRegex(ValueError, 'scripted studies'):
                run.run_study(self.output, paid=True)
        self.assertFalse((self.output / 'execution.json').exists())

    def test_interrupted_run_never_redispatches(self):
        protocol.save(self.output / 'execution.json', {'status': 'running'})
        with patch('urllib.request.urlopen', side_effect=AssertionError('network forbidden')):
            with self.assertRaisesRegex(ValueError, 'interrupted execution'):
                run.run_study(self.output, transport_factory=run.ScriptedTransport)

    def test_paid_execution_requires_campaign_funding_before_daemon_or_network(self):
        manifest = copy.deepcopy(self.manifest)
        manifest['stage'] = 'development'
        protocol.save(self.output / 'manifest.json', manifest)
        protocol.save(self.output / 'freeze.json', dict(schema=protocol.SCHEMA,
                      manifest_sha256=protocol.file_digest(self.output / 'manifest.json')))
        with patch('run.Daemon', side_effect=AssertionError('daemon must not launch')):
            with self.assertRaisesRegex(ValueError, 'campaign allocation receipt'):
                run.run_study(self.output, paid=True)
        self.assertFalse((self.output / 'execution.json').exists())

    def test_private_restart_keeps_own_sources_and_attributed_cost(self):
        self.manifest['continuation']['luna'] = 'restart'
        trial = self.trial()
        agent = run.Agent(trial, 'agent-0', .2)
        with patch('run.scoring.score', side_effect=lambda f, s, *a, **k: public_result(s)):
            agent.run()
        self.assertTrue(agent.private_review_done)
        self.assertEqual(len(agent.session.request_ids), 3)
        self.assertIn(agent.candidates[0]['source'], json.loads(agent.session.history[1]['content'].split('Own evaluated artifacts:\n')[1])[0]['source'])
        self.assertGreater(agent.session.spent(), 0)


class FrozenArchive(unittest.TestCase):
    def test_public_regression_fixtures_are_frozen_and_never_added_to_hidden(self):
        import selector_regression
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / 'binary'
            binary.write_bytes(b'test')
            spec = run.scripted_spec()
            spec['blocks'] = [dict(family='coloring', pack='p0', repetition=0,
                                   attempt='a0', phase='scripted')]
            original = protocol.build_manifest(spec, binary, run.PROMPTS)
            spec['public_fixtures'] = {'coloring': selector_regression.public_regression_pack()}
            qualified = protocol.build_manifest(spec, binary, run.PROMPTS)
            public = qualified['blocks'][0]['public']
            self.assertEqual(public['fixture_count'], 1)
            self.assertEqual(public['instances'][-1]['n'], 12)
            self.assertEqual(qualified['blocks'][0]['hidden'], original['blocks'][0]['hidden'])
            self.assertIn('outside the sampled generator: 1', qualified['blocks'][0]['prompts']['solo'])

    def test_scored_labels_cannot_hide_a_different_procedure(self):
        with tempfile.TemporaryDirectory() as directory:
            output, manifest = small_manifest(directory)
            manifest['stage'] = 'scored'
            manifest['arms'] = {name: {'kind': name} for name in ('S', 'I', 'C', 'P', 'R')}
            manifest['arms']['C']['policy'] = manifest['arms']['P']['policy'] = 'D'
            manifest['arms']['P']['kind'] = 'C'
            with self.assertRaisesRegex(ValueError, 'labels must match'):
                protocol.validate(manifest)

    def test_manifest_overwrite_refused_and_source_drift_detected(self):
        with tempfile.TemporaryDirectory() as directory:
            output, manifest = small_manifest(directory)
            before = (output / 'manifest.json').read_bytes()
            with self.assertRaisesRegex(ValueError, 'already exists'):
                protocol.freeze(output, manifest, Path(directory) / 'fixture-binary')
            self.assertEqual(before, (output / 'manifest.json').read_bytes())
            with patch('protocol.source_hashes', return_value={}):
                with self.assertRaisesRegex(ValueError, 'runtime differs'):
                    protocol.load_manifest(output)
            documentation_commit = copy.deepcopy(manifest['runtime'])
            documentation_commit['source_commit'] = 'later-commit-tracking-the-manifest'
            with patch('protocol.runtime', return_value=documentation_commit):
                self.assertEqual(protocol.load_manifest(output), manifest)
            changed = copy.deepcopy(manifest)
            changed['budget']['task_arm_usd'] = 9
            protocol.save(output / 'manifest.json', changed)
            with self.assertRaisesRegex(ValueError, 'frozen configuration receipt'):
                protocol.load_manifest(output)

    def test_repetition_ids_unique_while_task_pack_stays_identical(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / 'binary'
            binary.write_bytes(b'test')
            manifest = protocol.build_manifest(run.scripted_spec(), binary, run.PROMPTS)
            self.assertEqual(len({b['id'] for b in manifest['blocks']}), 4)
            self.assertEqual(manifest['blocks'][0]['public'], manifest['blocks'][1]['public'])
            self.assertNotEqual(manifest['blocks'][0]['public'], manifest['blocks'][2]['public'])

    def test_seal_requires_every_assigned_trial_and_detects_tampering(self):
        with tempfile.TemporaryDirectory() as directory:
            output, manifest = small_manifest(directory)
            protocol.save(output / 'ledger.json', [])
            with self.assertRaises(FileNotFoundError):
                protocol.seal(output)
            for path in protocol.trial_paths(manifest):
                protocol.save(output / path, {'final': True, 'manifest_sha256': protocol.file_digest(output / 'manifest.json')})
            protocol.seal(output)
            self.assertEqual(protocol.verify_seal(output), manifest)
            protocol.save(output / 'ledger.json', [{'tampered': True}])
            with self.assertRaisesRegex(ValueError, 'sealed file changed'):
                protocol.verify_seal(output)

    def test_finalization_keeps_missing_assignments_as_failed_observations(self):
        with tempfile.TemporaryDirectory() as directory:
            output, manifest = small_manifest(directory)
            protocol.save(output / 'ledger.json', [])
            protocol.save(output / 'execution.json', {'status': 'running'})
            run.finalize_interrupted(output)
            protocol.verify_seal(output)
            for path in protocol.trial_paths(manifest):
                result = json.loads((output / path).read_text())
                self.assertTrue(result['final'])
                self.assertIn('interrupted', result['error'])
                self.assertIsNone(result['selected'])


if __name__ == '__main__':
    unittest.main()
