"""Offline policy and exact transport-boundary tests; no model/network calls."""
import copy
import hashlib
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import record


def candidate(author, sequence, score=1.0, valid=True, source=None):
    source = source if source is not None else f'# {author} attempt {sequence}\ndef solve(i): return []\n'
    return dict(id=f'block/prefix/{author}/{sequence}', parent_id=None, agent=author,
                sequence=sequence, source=source, sha256=hashlib.sha256(source.encode()).hexdigest(),
                public=dict(normalized=score, valid_instances=int(valid), total_instances=1,
                            source_sha256=hashlib.sha256(source.encode()).hexdigest(),
                            instances=[dict(valid=valid, cost=score, reference_cost=1.0)]))


class FakeGoal:
    def __init__(self, name='goal', agents=('a', 'b', 'c')):
        self.id = name
        self.roles = {agent: dict(principal=name + '-principal-' + agent) for agent in agents}
        self.identity_map = {agent: dict(agent=agent, goal=name, principal=self.principal(agent))
                             for agent in agents}
        self.receipts, self.store, self.reads = [], {}, []

    def principal(self, agent):
        return self.roles[agent]['principal']

    def publish(self, agent, finding):
        if len(record.canonical(finding).encode()) + record.SEALED_OVERHEAD_BYTES > record.MAX_PAYLOAD_BYTES:
            raise ValueError('oversized fake daemon publication')
        event = self.id + '-event-' + str(len(self.store))
        self.store[event] = dict(contribution=event, author=self.principal(agent), finding=copy.deepcopy(finding))
        receipt = {'recorded': {'event': event}}
        self.receipts.append(dict(agent=agent, receipt=receipt))
        return receipt

    def read(self, agent, receipt_ids):
        self.reads.append((agent, list(receipt_ids)))
        return [copy.deepcopy(self.store[receipt_id]) for receipt_id in receipt_ids]


class Eligibility(unittest.TestCase):
    def test_filtered_chronology_baseline_strict_improvement_ties_and_invalid(self):
        candidates = [candidate('a', 0, 0.7, False), candidate('a', 1, 1.0),
                      candidate('a', 2, 1.0 - 1e-12), candidate('a', 3, 1.0 - 1e-12),
                      candidate('a', 4, 1.1), candidate('a', 5, 0.1, False)]
        kept, withheld = record.eligible_candidates(list(reversed(candidates)), 'F')
        self.assertEqual([row['sequence'] for row in kept], [0, 3, 4, 5])
        self.assertEqual([row['reason'] for row in withheld], ['first_valid_baseline', 'strict_public_improvement'])
        self.assertIsNone(kept[0]['public_best_before'])
        self.assertEqual(kept[1]['public_best_delta'], 0)
        self.assertLess(kept[2]['public_best_delta'], 0)
        self.assertEqual(kept[3]['public_best_before'], 1.0 - 1e-12)
        self.assertFalse(kept[3]['fully_public_valid'])

    def test_full_policies_keep_every_evaluated_candidate_and_empty_filter_stays_empty(self):
        rows = [candidate('a', 0), candidate('a', 1, .9)]
        for policy in ('E', 'D'):
            kept, withheld = record.eligible_candidates(rows, policy)
            self.assertEqual(len(kept), 2)
            self.assertEqual(withheld, [])
        self.assertEqual(record.eligible_candidates(rows, 'F')[0], [])
        self.assertEqual(record.eligible_candidates([], 'F'), ([], []))

    def test_duplicate_sequence_nonfinite_and_incomplete_public_validation(self):
        with self.assertRaises(ValueError):
            record.eligible_candidates([candidate('a', 0), candidate('a', 0)], 'F')
        with self.assertRaises(ValueError):
            record.eligible_candidates([candidate('a', 0, float('nan'))], 'F')
        item = candidate('a', 0)
        item['public']['total_instances'] = 2
        with self.assertRaises(ValueError):
            record.fully_valid(item)


class FrozenPolicies(unittest.TestCase):
    def prefixes(self):
        return {'a': [candidate('a', 0)], 'b': [candidate('b', 0), candidate('b', 1, 1.1)],
                'c': [candidate('c', 0, .1, False)]}

    def test_private_and_peer_review_and_exact_candidate_id_source_authorization(self):
        prefixes = self.prefixes()
        private = record.FrozenRecord(FakeGoal('C'), 'block/C', 'D', prefixes).freeze()
        peer = record.FrozenRecord(FakeGoal('P'), 'block/P', 'D', prefixes).freeze()
        a_id, b_id = prefixes['a'][0]['id'], prefixes['b'][1]['id']
        c_view, p_view = private.review('a', peers=False), peer.review('a', peers=True)
        self.assertEqual([row['candidate_id'] for row in c_view['candidates']], [a_id])
        self.assertEqual(len(p_view['candidates']), 4)
        self.assertEqual(c_view['candidates'][0], p_view['candidates'][0])
        reads_before = len(private.goal.reads)
        with self.assertRaises(PermissionError):
            private.source('a', b_id)
        self.assertEqual(len(private.goal.reads), reads_before)
        self.assertEqual(peer.source('a', b_id)['source'], prefixes['b'][1]['source'])
        with self.assertRaises(PermissionError):
            peer.source('b', b_id)
        with self.assertRaises(PermissionError):
            peer.source('a', 'block/another-prefix/b/1')
        with self.assertRaises(RuntimeError):
            private.review('a', peers=True)
        audit = private.archive()
        self.assertEqual(audit['deliveries'][0]['recipient_id'], 'block/C/a')
        self.assertEqual(len(audit['deliveries'][0]['withheld']), 3)
        self.assertTrue(all(row['reason'] == 'private_review' for row in audit['deliveries'][0]['withheld']))
        self.assertTrue(set(private.goal.store).isdisjoint(peer.goal.store))

    def test_invalid_recipient_gets_identical_private_content_and_never_later_exposes(self):
        prefixes = self.prefixes()
        private = record.FrozenRecord(FakeGoal('C'), 'C', 'D', prefixes).freeze()
        peer = record.FrozenRecord(FakeGoal('P'), 'P', 'D', prefixes).freeze()
        self.assertEqual(private.review('c', peers=False), peer.review('c', peers=True))
        prefixes['c'].append(candidate('c', 1, 0.9))
        with self.assertRaises(PermissionError):
            peer.source('c', prefixes['a'][0]['id'])
        with self.assertRaises(RuntimeError):
            peer.review('c', peers=True)
        delivery = peer.archive()['deliveries'][0]
        self.assertFalse(delivery['peer_access'])
        self.assertTrue(all(row['reason'] == 'recipient_has_no_valid_prefix' for row in delivery['withheld']))

    def test_filtered_packets_withhold_success_and_preserve_empty_treatment(self):
        prefixes = self.prefixes()
        peer = record.FrozenRecord(FakeGoal(), 'F', 'F', prefixes).freeze()
        got = peer.review('a', peers=True)
        self.assertEqual([row['candidate_id'] for row in got['candidates']],
                         [prefixes['b'][1]['id'], prefixes['c'][0]['id']])
        with self.assertRaises(PermissionError):
            peer.source('a', prefixes['b'][0]['id'])
        self.assertEqual(len(peer.goal.store), 4)
        empty = {agent: [candidate(agent, 0)] for agent in ('a', 'b', 'c')}
        frozen = record.FrozenRecord(FakeGoal(), 'empty', 'F', empty).freeze()
        self.assertEqual(frozen.review('a', peers=True)['candidates'], [])
        self.assertEqual(frozen.goal.store, {})
        self.assertTrue(frozen.archive()['deliveries'][0]['peer_access'])

    def test_snapshot_immutable_and_one_publication_only(self):
        prefixes = self.prefixes()
        frozen = record.FrozenRecord(FakeGoal(), 'branch', 'D', prefixes)
        old_source = prefixes['a'][0]['source']
        prefixes['a'][0]['source'] = 'changed'
        frozen.freeze()
        frozen.review('a')
        self.assertEqual(frozen.source('a', prefixes['a'][0]['id'])['source'], old_source)
        count = len(frozen.goal.store)
        with self.assertRaises(RuntimeError):
            frozen.freeze()
        self.assertEqual(len(frozen.goal.store), count)
        audit = frozen.archive()
        audit['valid_prefix']['a'] = False
        self.assertTrue(frozen.archive()['valid_prefix']['a'])

    def test_wrong_hash_duplicate_id_and_wrong_author_refuse_before_publication(self):
        for mutation in ('hash', 'id', 'author'):
            with self.subTest(mutation=mutation):
                prefixes = self.prefixes()
                if mutation == 'hash':
                    prefixes['a'][0]['sha256'] = 'bad'
                elif mutation == 'id':
                    prefixes['b'][0]['id'] = prefixes['a'][0]['id']
                else:
                    prefixes['a'][0]['agent'] = 'b'
                goal = FakeGoal()
                with self.assertRaises(ValueError):
                    record.FrozenRecord(goal, 'branch', 'E', prefixes)
                self.assertEqual(goal.store, {})

    def test_transport_failure_preserves_partial_publications_and_prevents_retry(self):
        goal = FakeGoal()
        original = goal.publish
        def failing(agent, finding):
            if goal.store:
                raise RuntimeError('transport stopped')
            return original(agent, finding)
        goal.publish = failing
        frozen = record.FrozenRecord(goal, 'branch', 'D', self.prefixes())
        with self.assertRaisesRegex(RuntimeError, 'transport stopped'):
            frozen.freeze()
        self.assertEqual(frozen.archive()['state'], 'failed')
        self.assertEqual(len(frozen.archive()['receipts']), 1)
        with self.assertRaises(RuntimeError):
            frozen.freeze()
        with self.assertRaises(RuntimeError):
            frozen.review('a', peers=True)

    def test_forged_author_cross_branch_and_corrupt_bytes_fail_closed(self):
        for mutation in ('author', 'branch', 'bytes', 'missing'):
            with self.subTest(mutation=mutation):
                goal = FakeGoal()
                frozen = record.FrozenRecord(goal, 'branch', 'D', self.prefixes()).freeze()
                first = next(row for row in goal.store.values() if row['finding']['kind'] == 'metadata')
                if mutation == 'author':
                    first['author'] = goal.principal('b')
                elif mutation == 'branch':
                    first['finding']['branch_id'] = 'another-branch'
                elif mutation == 'bytes':
                    first['finding']['data'] = 'e30='
                else:
                    goal.read = lambda agent, receipt_ids: []
                with self.assertRaises(RuntimeError):
                    frozen.review('a')
                self.assertEqual(frozen.archive()['deliveries'][0]['status'], 'failed')
                with self.assertRaises(PermissionError):
                    frozen.source('a', self.prefixes()['a'][0]['id'])
                with self.assertRaises(RuntimeError):
                    frozen.review('a')


class TransportConstraints(unittest.TestCase):
    def test_real_limit_constants_match_pinned_rust_sources(self):
        limits = (record.ROOT / 'crates/locust-proto/src/limits.rs').read_text()
        seal = (record.ROOT / 'crates/locust-proto/src/seal.rs').read_text()
        self.assertIn('MAX_PAYLOAD_BYTES: usize = 1024 * 1024;', limits)
        self.assertIn('NONCE_OFFSET: usize = 5;', seal)
        self.assertIn('NONCE_BYTES: usize = 24;', seal)
        self.assertIn('TAG_BYTES: usize = 16;', seal)
        self.assertEqual(record.SEALED_OVERHEAD_BYTES, 5 + 24 + 16)

    def test_goal_checks_sealed_size_not_raw_payload_and_inspects_explicit_ids(self):
        class Transport:
            def __init__(self):
                self.calls = []
            def call(self, args, role, stdin=None):
                self.calls.append((args, role, stdin))
                if args[:2] == ['contribution', 'publish']:
                    return {'recorded': {'event': 'e'}}
                return {'contribution_inspected': {'contribution': {
                    'view': {'event': 'e', 'author': 'p', 'standing': 'effective'}, 'text': '{"x":1}'}}}
        transport = Transport()
        goal = record.Goal(transport, 'g', {'a': {'principal': 'p'}})
        goal.publish('a', 'x' * (record.MAX_TEXT_BYTES - 2))
        self.assertEqual(goal.receipts[0]['sealed_bytes'], record.MAX_PAYLOAD_BYTES)
        with self.assertRaises(ValueError):
            goal.publish('a', 'x' * (record.MAX_TEXT_BYTES - 1))
        self.assertEqual(goal.read('a', ['e']), [{'contribution': 'e', 'author': 'p', 'finding': {'x': 1}}])
        self.assertIn('--contribution', transport.calls[-1][0])

    def test_multi_payload_source_and_large_metadata_are_lossless(self):
        source = '# λ☃"\\\n' * 160000
        item = candidate('a', 0, source=source)
        item['public']['instances'][0]['error_detail'] = 'large public evidence ' * 80000
        frozen = record.FrozenRecord(FakeGoal(), 'large', 'D', {'a': [item], 'b': [], 'c': []}).freeze()
        chunks = list(frozen.goal.store.values())
        self.assertGreater(len(chunks), 4)
        self.assertTrue(all(len(record.canonical(row['finding']).encode()) <= record.MAX_TEXT_BYTES for row in chunks))
        view = frozen.review('a')
        self.assertEqual(view['candidates'][0]['public'], item['public'])
        self.assertEqual(frozen.source('a', item['id'])['source'], source)
        self.assertTrue(all(len(ids) >= 2 for _, ids in frozen.goal.reads))

    def test_hashed_identity_preserves_long_unicode_names_without_collisions(self):
        names = set()
        for arm in ('C', 'P'):
            for pack in range(4):
                for repetition in range(3):
                    for attempt in range(2):
                        for agent in ('a', 'b', 'c'):
                            title = f'study/very-long-frozen-study-identity/☃/{arm}/{pack}/{repetition}/{attempt}'
                            name = record.principal_name(title, agent)
                            self.assertRegex(name, record.PRINCIPAL_NAME)
                            self.assertNotIn(name, names)
                            self.assertEqual(name, record.principal_name(title, agent))
                            names.add(name)
        self.assertNotEqual(record.principal_name('a/b', 'c'), record.principal_name('a', 'b/c'))


if __name__ == '__main__':
    unittest.main()
