import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import analyze
import problems
import protocol
import scoring


def result(value, source=None, *, valid=True):
    source_hash = hashlib.sha256(source.encode()).hexdigest() if source is not None else None
    return dict(instances=[dict(instance_index=0, repetition=0, seed=1, valid=valid,
                               cost=value, fallback_cost=10, solution_sha256=str(value))],
                total_cost=value, reference_total=1, normalized=value,
                valid_instances=int(valid), total_instances=1, instance_count=1,
                repetitions=1, source_sha256=source_hash)


def candidate(name, value, when, *, valid=True):
    source = f'def solve(i):\n    return list(range(i["n"]))  # {name}'
    public = result(value, source, valid=valid)
    return dict(id=name, parent_id=None, agent='agent-0', sequence=0,
                source=source, sha256=public['source_sha256'], time=when, public=public)


def ledger_entry(name, dispatched, reserved, *, settled=None, cost=None, status='complete'):
    row = dict(id=name, label=name, phase='scripted', reserved_at=dispatched,
               reserved=reserved, status=status)
    if settled is not None:
        row['settled_at'] = settled
    if cost is not None:
        row['cost'] = cost
    return row


def fixture(directory, *, failed_peer=False):
    root = Path(directory)
    public = [{'n': 4, 'edges': [[0, 1], [2, 3]]}]
    hidden = problems.instances('study', 'vertex_cover', 'hidden', pack='p0', count=1)
    block = dict(id='b0', family='vertex_cover', pack='p0', repetition=0, attempt='a0', phase='scripted',
                 public=dict(instances=public, reference=[2], digest=protocol.digest(public)),
                 hidden=dict(seed='study', pack='p0', count=1, digest=protocol.digest(hidden)),
                 launch_order=['I', 'C', 'P'])
    source_hashes = {str(Path(module.__file__).relative_to(protocol.ROOT)): protocol.file_digest(module.__file__)
                     for module in (problems, scoring)}
    model = dict(model='scripted-fixture', effort='high', service_tier='default',
                 rates=dict(input=1, cached_input=0, cache_write=1, output=1),
                 context_tokens=100000, max_input_tokens=90000, max_output_tokens=10000,
                 input_accounting=dict(method='context_bound'))
    manifest = dict(schema=protocol.SCHEMA, study_id='test', seed='study', stage='scripted',
                    budget=dict(task_arm_usd=1, study_usd=10, phase_usd=10),
                    arms={'I': {'kind': 'I'}, 'C': {'kind': 'C', 'policy': 'D'}, 'P': {'kind': 'P', 'policy': 'D'}},
                    models={'luna': model, 'frontier': model}, continuation={'luna': 'continue', 'frontier': 'continue'},
                    blocks=[block], evaluation=dict(repetitions=1, seed='e', cpu_seconds=2, wall_seconds=40),
                    analysis=dict(bootstrap_samples=100, seed=7, useful_effect=0.01),
                    runtime=dict(source_sha256=source_hashes))
    protocol.save(root / 'manifest.json', manifest)
    manifest_hash = protocol.file_digest(root / 'manifest.json')
    protocol.save(root / 'freeze.json', dict(schema=protocol.SCHEMA, manifest_sha256=manifest_hash))
    ledger = [ledger_entry('prefix', 0, .2, settled=1, cost=.1)]
    prefix = candidate('prefix', .99, 2)
    paths = ['freeze.json']
    for arm, value in (('I', .8), ('C', .85), ('P', .7)):
        entry = ledger_entry(arm, 3, .2, settled=4, cost=.1)
        ledger.append(entry)
        candidates = [] if arm == 'P' and failed_peer else [copy.deepcopy(prefix), candidate(arm, value, 5)]
        selected = scoring.select_public(candidates)
        trial = dict(block_id='b0', family='vertex_cover', pack='p0', repetition=0,
                     attempt='a0', phase='scripted', arm=arm, final=True,
                     manifest_sha256=manifest_hash, started=0, finished=6,
                     request_ids=['prefix', arm], candidates=candidates,
                     prefix_candidate_ids=[] if not candidates else ['prefix'],
                     selected=selected['id'] if selected else None,
                     agents=[dict(name='agent-0', failed='transport' if not candidates else None)])
        name = f'trials/b0/{arm}.json'
        protocol.save(root / name, trial)
        paths.append(name)
    protocol.save(root / 'ledger.json', ledger)
    paths.append('ledger.json')
    protocol.save(root / 'seal.json', dict(schema=protocol.SCHEMA, manifest_sha256=manifest_hash,
                                         files={name: protocol.file_digest(root / name) for name in paths}))
    return manifest


def fake_hidden(_family, source, _items, _reference, **_kwargs):
    name = source.rsplit('# ', 1)[1]
    return result({'prefix': .99, 'I': .8, 'C': .85, 'P': .7}[name], source)


class SealedOfflineScoring(unittest.TestCase):
    def test_hidden_scoring_refuses_unsealed_or_changed_live_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            (root / 'trials/b0/C.json').write_text('{}')
            with patch.object(scoring, 'score') as score, self.assertRaises(ValueError):
                analyze.offline_hidden(root)
            score.assert_not_called()
            self.assertFalse((root / 'hidden.json').exists())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            (root / 'seal.json').unlink()
            with patch.object(scoring, 'score') as score, self.assertRaises(FileNotFoundError):
                analyze.offline_hidden(root)
            score.assert_not_called()

    def test_separate_hidden_archive_duplicate_sources_cached_and_reused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            before = (root / 'trials/b0/P.json').read_bytes()
            with patch.object(scoring, 'score', side_effect=fake_hidden) as score:
                hidden = analyze.offline_hidden(root)
                self.assertEqual(score.call_count, 4, 'shared prefix source is evaluated once')
                self.assertEqual(analyze.offline_hidden(root), hidden)
                self.assertEqual(score.call_count, 4, 'completed evaluation is not rerun')
            self.assertEqual(before, (root / 'trials/b0/P.json').read_bytes())
            self.assertEqual(hidden['trials']['b0/P']['selected'], 'P')
            report = analyze.analyze(root)
            self.assertAlmostEqual(report['contrasts']['G_info']['mean'], .15)
            self.assertAlmostEqual(report['contrasts']['G_net']['mean'], .1)
            self.assertAlmostEqual(report['billing']['total'], .4)
            self.assertAlmostEqual(sum(row['spending']['attributed_cost'] for row in report['trials'].values()), .6)
            self.assertTrue(report['contrasts']['G_info']['descriptive'])
            self.assertTrue(report['billing']['synthetic'])

    def test_resume_reuses_each_completed_source_without_repeating_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            original = analyze._save_once
            pending_starts = 0

            def stop_between_sources(path, value):
                nonlocal pending_starts
                if path.name.endswith('.pending.json'):
                    pending_starts += 1
                    if pending_starts == 2:
                        raise RuntimeError('interrupted before another source starts')
                original(path, value)

            with patch.object(analyze, '_save_once', side_effect=stop_between_sources), \
                    patch.object(scoring, 'score', side_effect=fake_hidden) as first:
                with self.assertRaisesRegex(RuntimeError, 'interrupted'):
                    analyze.offline_hidden(root)
                self.assertEqual(first.call_count, 1)
            self.assertFalse((root / 'hidden.json').exists())
            with patch.object(scoring, 'score', side_effect=fake_hidden) as resumed:
                hidden = analyze.offline_hidden(root)
                self.assertEqual(resumed.call_count, 3)
                self.assertTrue(all('# prefix' not in call.args[1] for call in resumed.call_args_list))
            self.assertEqual(hidden['trials']['b0/P']['candidates']['prefix']['normalized'], .99)

    def test_unfinished_source_cannot_be_silently_evaluated_again(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            with patch.object(scoring, 'score', side_effect=KeyboardInterrupt):
                with self.assertRaises(KeyboardInterrupt):
                    analyze.offline_hidden(root)
            self.assertEqual(len(list((root / 'hidden-journal').glob('*.pending.json'))), 1)
            with patch.object(scoring, 'score') as score, \
                    self.assertRaisesRegex(ValueError, 'explicit recovery required, automatic rerun refused'):
                analyze.offline_hidden(root)
            score.assert_not_called()

    def test_completed_source_journal_is_part_of_archive_integrity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            with patch.object(scoring, 'score', side_effect=fake_hidden):
                analyze.offline_hidden(root)
            source = next((root / 'hidden-journal').glob('*.complete.json'))
            value = json.loads(source.read_text())
            value['score']['normalized'] = 0
            protocol.save(source, value)
            with self.assertRaisesRegex(ValueError, 'journal differs'):
                analyze.analyze(root)

    def test_elapsed_adds_prefix_critical_path_and_retains_trial_error(self):
        trial = dict(arm='P', selected=None, candidates=[], agents=[], request_ids=[],
                     started=100, finished=106, prefix_elapsed_seconds=4, error='interrupted')
        hidden = dict(selected=None, fallback=result(10, valid=False), candidates={})
        summary = analyze.trial_summary(trial, hidden, {}, 1)
        self.assertEqual(summary['elapsed_seconds'], 10)
        self.assertEqual(summary['branch_elapsed_seconds'], 6)
        self.assertEqual(summary['prefix_elapsed_seconds'], 4)
        self.assertEqual(summary['failures'], ['interrupted'])
        trial['started'] = None
        summary = analyze.trial_summary(trial, hidden, {}, 1)
        self.assertIsNone(summary['elapsed_seconds'])
        self.assertIsNone(summary['branch_elapsed_seconds'])
        self.assertEqual(summary['prefix_elapsed_seconds'], 4)

    def test_no_candidate_failure_remains_an_observation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root, failed_peer=True)
            with patch.object(scoring, 'score', side_effect=fake_hidden):
                analyze.offline_hidden(root)
            report = analyze.analyze(root)
            self.assertEqual(len(report['trials']), 3)
            failed = report['trials']['b0/P']
            self.assertTrue(failed['fallback'])
            self.assertFalse(failed['hidden_valid'])
            self.assertEqual(failed['failures'], ['transport'])
            self.assertGreater(failed['hidden'], 1)
            self.assertEqual(report['contrasts']['G_info']['blocks'], 1)
            self.assertLess(report['contrasts']['G_info']['mean'], 0)

    def test_archived_analysis_does_not_depend_on_current_scorer_runtime(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as archive:
            root = Path(directory)
            fixture(root)
            with patch.object(scoring, 'score', side_effect=fake_hidden):
                analyze.offline_hidden(root)
            original = analyze.analyze(root)
            analyze.package(root, archive)
            with patch.object(analyze, '_scorer_provenance', side_effect=AssertionError('must not inspect live scorer')):
                reproduced = analyze.analyze(archive)
            self.assertEqual(original, reproduced)

    def test_hidden_file_change_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            with patch.object(scoring, 'score', side_effect=fake_hidden):
                analyze.offline_hidden(root)
            hidden = json.loads((root / 'hidden.json').read_text())
            hidden['trials']['b0/P']['candidates']['P']['normalized'] = 0
            protocol.save(root / 'hidden.json', hidden)
            with self.assertRaisesRegex(ValueError, 'completion seal'):
                analyze.analyze(root)

    def test_missing_dependency_is_not_silently_free(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            ledger = json.loads((root / 'ledger.json').read_text())[1:]
            protocol.save(root / 'ledger.json', ledger)
            seal = json.loads((root / 'seal.json').read_text())
            seal['files']['ledger.json'] = protocol.file_digest(root / 'ledger.json')
            protocol.save(root / 'seal.json', seal)
            with patch.object(scoring, 'score') as score, self.assertRaisesRegex(ValueError, 'missing ledger dependency'):
                analyze.offline_hidden(root)
            score.assert_not_called()


class CommittedCostCheckpoints(unittest.TestCase):
    def test_pending_work_prefix_dedup_and_no_retroactive_admission(self):
        ledger = [ledger_entry('prefix', 0, .2, settled=1, cost=.1),
                  ledger_entry('parallel', 1, .4, settled=100, cost=.02),
                  ledger_entry('other-arm', 0, 4, settled=1, cost=3)]
        self.assertAlmostEqual(analyze.arm_spend_at(ledger, ['prefix', 'parallel', 'prefix'], 3), .5)
        self.assertAlmostEqual(analyze.arm_spend_at(ledger, ['prefix', 'parallel'], 101), .12)
        early, late = candidate('early', .1, 3), candidate('late', .8, 101)
        trial = dict(request_ids=['prefix', 'parallel'], candidates=[early, late])
        hidden = dict(fallback=result(10, valid=False), candidates={'early': result(.1), 'late': result(.8)})
        rows = analyze.matched(trial, analyze._entries(ledger), hidden, [.15, .5])
        self.assertEqual(rows[0]['selected'], 'late')
        self.assertEqual(rows[1]['selected'], 'early')

    def test_failed_uncertainty_is_not_a_zero_cost_request(self):
        ledger = [ledger_entry('failed', 1, .4, settled=2, status='failed')]
        self.assertEqual(analyze.arm_spend_at(ledger, ['failed'], 3), .4)
        spent = analyze.spending(dict(request_ids=['failed']), analyze._entries(ledger))
        self.assertEqual(spent['known_usage_cost'], 0)
        self.assertEqual(spent['uncertain_reservations'], .4)
        self.assertEqual(spent['failed'], 1)


class HierarchicalAnalysis(unittest.TestCase):
    def test_family_then_pack_weighting_and_all_repetitions_preserved(self):
        manifest = dict(stage='scored', arms={'I': {'kind': 'I'}, 'C': {'kind': 'C', 'policy': 'D'},
                                            'P': {'kind': 'P', 'policy': 'D'}},
                        analysis=dict(bootstrap_samples=500, seed=5), blocks=[])
        summaries = {}
        for index, (family, pack, delta) in enumerate([('a', 'x', .1), ('a', 'x', .3),
                                                      ('a', 'y', .8), ('b', 'x', .1)]):
            block = dict(id=str(index), family=family, pack=pack)
            manifest['blocks'].append(block)
            for arm, value in [('I', delta + .5), ('C', delta + .5), ('P', .5)]:
                summaries[f'{index}/{arm}'] = dict(hidden=value)
        out = analyze.contrasts(manifest, summaries)['G_info']
        self.assertAlmostEqual(out['mean'], .3)
        self.assertEqual(out['blocks'], 4)
        self.assertEqual(out['families']['a']['repetitions'], 3)
        self.assertEqual(out, analyze.contrasts(manifest, summaries)['G_info'])
        self.assertTrue(out['few_packs'])
        self.assertTrue(out['descriptive'], 'family b has only one independent task pack')
        self.assertIn('fewer than two independent task packs', out['interval_note'])

    def test_constant_paired_effect_has_correct_sign_and_coverage_label(self):
        data = {'a': {'p0': [.2, .2], 'p1': [.2, .2]}, 'b': {'p0': [.2, .2]}}
        interval = analyze.hierarchical_interval(data, samples=100, seed=2, coverage=.975)
        self.assertAlmostEqual(interval[0], .2)
        self.assertAlmostEqual(interval[1], .2)

    def test_exploratory_flag_prevents_positive_and_negative_threshold_claims(self):
        manifest = dict(analysis=dict(exploratory=True, useful_effect=.01),
                        blocks=[dict(family='a', pack='p0'), dict(family='a', pack='p1')])
        for interval in ([.2, .3], [-.3, -.2]):
            full = {name: dict(interval_97_5=interval) for name in ('G_info', 'G_net')}
            decision = analyze.scored_decision(manifest, full)
            self.assertEqual(decision['verdict'], 'descriptive exploratory pilot; no confirmatory conclusion')
            self.assertTrue(decision['descriptive'])
            self.assertIn('frozen analysis declares an exploratory study', decision['reasons'][0])

    def test_one_pack_guard_blocks_degenerate_interval_claim_without_opt_in(self):
        manifest = dict(analysis=dict(useful_effect=.01),
                        blocks=[dict(family='a', pack='p0'), dict(family='a', pack='p0')])
        full = {name: dict(interval_97_5=[.2, .2]) for name in ('G_info', 'G_net')}
        decision = analyze.scored_decision(manifest, full)
        self.assertTrue(decision['descriptive'])
        self.assertIn('fewer than two independent task packs', decision['reasons'][0])
        manifest['blocks'].append(dict(family='a', pack='p1'))
        decision = analyze.scored_decision(manifest, full)
        self.assertFalse(decision['descriptive'])
        self.assertIn('worthwhile peer-policy benefit', decision['verdict'])


class DevelopmentPolicySelection(unittest.TestCase):
    def example(self, delayed=.80, filtered=.81, delayed_cost=.2, filtered_cost=.1):
        arms = {'S': {'kind': 'S'}, 'I': {'kind': 'I'}, 'R': {'kind': 'R'}}
        for kind in ('C', 'P'):
            for policy in ('E', 'D', 'F'):
                arms[f'{kind}_{policy}'] = dict(kind=kind, policy=policy)
        manifest = dict(stage='development', arms=arms,
                        blocks=[dict(id='b0', family='a', pack='p0'),
                                dict(id='b1', family='b', pack='p0')])
        summaries = {}
        for block in manifest['blocks']:
            for arm in arms:
                value = delayed if arm == 'P_D' else filtered if arm == 'P_F' else 1
                cost = delayed_cost if arm == 'P_D' else filtered_cost if arm == 'P_F' else .5
                summaries[f'{block["id"]}/{arm}'] = dict(hidden=value,
                    spending=dict(known_usage_cost=cost, unknown_cost_requests=0))
        return manifest, summaries

    def test_larger_gain_selects_policy_and_preserves_early_comparator_and_controls(self):
        manifest, summaries = self.example()
        selection = analyze.development_selection(manifest, summaries)
        self.assertEqual(selection['selected_policy'], 'D')
        self.assertEqual(selection['selected_arm'], 'P_D')
        self.assertEqual(set(selection['arms']), set(manifest['arms']))
        self.assertAlmostEqual(selection['arms']['P_F']['family_balanced_gain_over_I'], .19)

    def test_near_gain_tie_uses_actual_cost_and_exact_cost_tie_uses_D(self):
        manifest, summaries = self.example(delayed=.8, filtered=.801)
        selection = analyze.development_selection(manifest, summaries)
        self.assertEqual(selection['selected_policy'], 'F')
        manifest, summaries = self.example(delayed=.8, filtered=.801, delayed_cost=.1)
        selection = analyze.development_selection(manifest, summaries)
        self.assertEqual(selection['selected_policy'], 'D')
        self.assertIn('exact actual-cost tie', selection['reason'])

    def test_unknown_actual_cost_does_not_substitute_reservations_or_zero(self):
        manifest, summaries = self.example(delayed=.8, filtered=.801)
        summaries['b0/P_F']['spending']['unknown_cost_requests'] = 1
        selection = analyze.development_selection(manifest, summaries)
        self.assertIsNone(selection['selected_policy'])
        self.assertEqual(selection['status'], 'incomplete')
        self.assertIsNone(selection['arms']['P_F']['mean_actual_cost'])
        self.assertIn('actual charged cost is unresolved', selection['reason'])

    def test_scored_and_scripted_stages_do_not_choose_a_development_policy(self):
        manifest, summaries = self.example()
        for stage in ('scored', 'scripted'):
            manifest['stage'] = stage
            self.assertIsNone(analyze.development_selection(manifest, summaries))


if __name__ == '__main__':
    unittest.main()
