import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import problems
import scoring


class FrozenPacks(unittest.TestCase):
    def test_pack_count_and_split_are_explicit_and_reproducible(self):
        first = problems.instances('study', 'vertex_cover', 'public', pack='p1', count=1)
        self.assertEqual(first, problems.instances('study', 'vertex_cover', 'public', pack='p1', count=1))
        self.assertNotEqual(first, problems.instances('study', 'vertex_cover', 'public', pack='p2', count=1))
        self.assertNotEqual(first, problems.instances('study', 'vertex_cover', 'hidden', pack='p1', count=1))
        self.assertEqual(len(problems.instances('study', 'vertex_cover', 'public', pack='p1', count=2)), 2)
        with self.assertRaises(ValueError):
            problems.instances('study', 'vertex_cover', 'typo', pack='p1', count=1)
        with self.assertRaises(ValueError):
            problems.instances('study', 'vertex_cover', 'public', pack='p1', count=0)
        with self.assertRaises(TypeError):
            problems.instances('study', 'vertex_cover', 'public')

    def test_permutations_reject_float_and_boolean_aliases(self):
        instance = {'points': [[0, 0], [1, 0]]}
        for solution in ([0.0, 1.0], [False, True]):
            with self.subTest(solution=solution), self.assertRaises(ValueError):
                problems.FAMILIES['tsp'].validate(instance, solution)


class PublicScoring(unittest.TestCase):
    def setUp(self):
        self.items = [{'n': 4, 'edges': [[0, 1], [2, 3]]}, {'n': 3, 'edges': [[0, 1]]}]
        self.refs = problems.reference_costs('vertex_cover', self.items)
        self.source = 'def solve(i):\n    return list(range(i["n"]))'

    def test_repetition_schedule_keeps_all_outcomes_and_stable_seeds(self):
        seen = []

        def execute(_runner, payload, _wall):
            seen.append(payload['seed'])
            return SimpleNamespace(returncode=0, stdout=json.dumps({
                'solution': list(range(payload['instance']['n'])), 'cpu': 0.001, 'stdout': ''}), stderr='')
        with patch.object(scoring, '_sandboxed', side_effect=execute):
            result = scoring.score('vertex_cover', self.source, self.items, self.refs,
                                   repetitions=3, seed='frozen')
            repeat = scoring.score('vertex_cover', self.source, self.items, self.refs,
                                   repetitions=3, seed='frozen')
        self.assertEqual(result, repeat)
        self.assertEqual(seen[:6], seen[6:])
        self.assertEqual(len(set(seen[:6])), 6)
        self.assertEqual(result['total_instances'], 6)
        self.assertEqual(result['valid_instances'], 6)
        self.assertEqual([(row['instance_index'], row['repetition']) for row in result['instances']],
                         [(i, j) for i in range(2) for j in range(3)])
        self.assertEqual(result['total_cost'], 3 * 7)
        self.assertEqual(result['reference_total'], 3 * sum(self.refs))

    def test_one_failed_repetition_prevents_public_selection(self):
        calls = 0

        def execute(_runner, payload, _wall):
            nonlocal calls
            calls += 1
            value = {'error': 'bad repetition'} if calls == 2 else {
                'solution': list(range(payload['instance']['n'])), 'cpu': 0.001}
            return SimpleNamespace(returncode=0, stdout=json.dumps(value), stderr='')
        with patch.object(scoring, '_sandboxed', side_effect=execute):
            result = scoring.score('vertex_cover', self.source, self.items, self.refs,
                                   repetitions=2, seed='frozen')
        self.assertEqual(result['valid_instances'], 3)
        self.assertFalse(scoring.all_valid(result))
        self.assertIsNone(scoring.select_public([{'id': 'bad', 'public': result}]))

    def test_native_crash_and_invalid_outputs_use_fallback(self):
        for source in ('def solve(i):\n    raise RuntimeError("failure")',
                       'def solve(i):\n    return []'):
            with self.subTest(source=source):
                result = scoring.score('vertex_cover', source, self.items, self.refs,
                                       repetitions=1, seed='frozen')
                self.assertEqual(result['valid_instances'], 0)
                self.assertEqual(result['total_cost'], 7)
        good = scoring.score('vertex_cover', self.source, self.items, self.refs, repetitions=1, seed='frozen')
        self.assertTrue(scoring.all_valid(good))
        self.assertEqual(good['total_cost'], 7)

    def test_native_probe_output_is_not_arbitrarily_truncated(self):
        result = scoring.probe('print("x" * 12000)', self.items)
        self.assertEqual(result['stdout'], 'x' * 12000 + '\n')
        self.assertNotIn('truncated', result)

    def test_native_cpu_overrun_is_killed_and_retains_fallback(self):
        result = scoring.score('vertex_cover', 'def solve(i):\n    while True:\n        pass',
                               self.items[:1], self.refs[:1], repetitions=1, seed='frozen',
                               cpu_seconds=2, wall_seconds=10)
        self.assertEqual(result['valid_instances'], 0)
        self.assertEqual(result['total_cost'], 4)
        self.assertIn('CPU limit of 2 s exceeded', result['instances'][0]['error'])

    def test_public_view_keeps_unrounded_score_and_all_rows(self):
        result = scoring.fallback_score('vertex_cover', self.items, self.refs, repetitions=2, seed='frozen')
        result['normalized'] = 0.987654321987
        view = scoring.public_view(result, self.refs)
        self.assertEqual(view['public_normalized_score'], 0.987654321987)
        self.assertEqual(len(view['instances']), 4)
        self.assertNotIn('hidden', view)
        self.assertEqual([row['reference_cost'] for row in view['instances']],
                         [self.refs[0], self.refs[0], self.refs[1], self.refs[1]])


def evaluation(value, source_hash, *, valid=True):
    return dict(instances=[{'valid': valid, 'cost': value, 'instance_index': 0, 'repetition': 0}],
                normalized=value, valid_instances=int(valid), total_instances=1,
                source_sha256=source_hash)


class PublicSelection(unittest.TestCase):
    def test_valid_first_unrounded_score_then_hash_never_hidden_or_arrival(self):
        invalid = {'id': 'invalid', 'public': evaluation(0.01, 'a', valid=False), 'hidden': {'normalized': 0}}
        high_hash = {'id': 'high', 'time': 1, 'public': evaluation(0.80000009, 'z'), 'hidden': {'normalized': 0}}
        low_hash = {'id': 'low', 'time': 99, 'public': evaluation(0.80000009, 'a'), 'hidden': {'normalized': 100}}
        better = {'id': 'better', 'time': 100, 'public': evaluation(0.80000008, 'z'), 'hidden': {'normalized': 1000}}
        self.assertEqual(scoring.select_public([invalid, high_hash, low_hash])['id'], 'low')
        self.assertEqual(scoring.select_public([invalid, high_hash, low_hash, better])['id'], 'better')
        self.assertIsNone(scoring.select_public([invalid]))
        self.assertIsNone(scoring.select_public([]))

    def test_inconsistent_validity_records_are_rejected(self):
        result = evaluation(1, 'a')
        result['valid_instances'] = 0
        with self.assertRaisesRegex(ValueError, 'validity count'):
            scoring.all_valid(result)


if __name__ == '__main__':
    unittest.main()
