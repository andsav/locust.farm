"""Campaign funding safety, including separate processes and uncertain charges."""
import copy
import json
import multiprocessing
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import campaign
import protocol
import run


def competing_allocation(path, name, output, ready, start, results):
    ready.put(name)
    start.wait()
    try:
        receipt = campaign.allocate(path, name, output, 20)
        results.put(('allocated', receipt['name']))
    except ValueError as error:
        results.put(('refused', str(error)))


class Campaign(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.path = self.root / 'campaign.json'
        campaign.create(self.path, 'tonight', 30)
        self.network = patch('urllib.request.urlopen', side_effect=AssertionError('no network in funding tests'))
        self.network.start()
        self.addCleanup(self.network.stop)

    def frozen_study(self, name, ceiling):
        output = self.root / name
        parameters = {'stage': 'scripted', 'budget': {'task_arm_usd': .6},
                      'continuation': {'luna': 'continue'}}
        receipt = campaign.allocate(self.path, name, output, ceiling, parameters)
        binary = self.root / 'fixture-binary'
        binary.write_bytes(b'offline campaign fixture; never execute')
        spec = run.scripted_spec('campaign-' + name)
        spec['blocks'] = spec['blocks'][:1]
        spec['budget']['study_usd'] = ceiling
        spec['campaign'] = receipt
        manifest = protocol.build_manifest(spec, binary, run.PROMPTS)
        protocol.freeze(output, manifest, binary)
        return output, manifest

    def sealed_study(self, name, ceiling, ledger):
        output, manifest = self.frozen_study(name, ceiling)
        for path in protocol.trial_paths(manifest):
            protocol.save(output / path, dict(final=True, manifest_sha256=protocol.file_digest(output / 'manifest.json')))
        protocol.save(output / 'ledger.json', ledger)
        protocol.seal(output)
        return output, manifest

    def test_cancel_unstarted_freeze_releases_cap_but_keeps_identity_and_proof(self):
        output, manifest = self.frozen_study('unstarted', 1.68)
        original_manifest = (output / 'manifest.json').read_bytes()
        original_freeze = (output / 'freeze.json').read_bytes()
        self.assertAlmostEqual(campaign.remaining(self.path), 28.32)
        with patch('protocol.runtime', side_effect=AssertionError('old source no longer runs')):
            cancelled = campaign.cancel(self.path, 'unstarted', 'provider minimum requires a new freeze')
        self.assertEqual(campaign.remaining(self.path), 30)
        self.assertEqual(cancelled['committed_decimal'], '0')
        self.assertEqual((output / 'manifest.json').read_bytes(), original_manifest)
        self.assertEqual((output / 'freeze.json').read_bytes(), original_freeze)
        self.assertEqual(campaign.cancel(self.path, 'unstarted', cancelled['reason']), cancelled)
        with self.assertRaisesRegex(ValueError, 'original reason'):
            campaign.cancel(self.path, 'unstarted', 'different explanation')
        with self.assertRaisesRegex(ValueError, 'cancelled'):
            campaign.verify_allocation(output, manifest)
        with self.assertRaisesRegex(ValueError, 'cancelled'):
            campaign.settle(self.path, 'unstarted')
        with self.assertRaises(ValueError):
            campaign.allocate(self.path, 'unstarted', self.root / 'new', 1.68)
        with self.assertRaisesRegex(ValueError, 'already allocated'):
            campaign.allocate(self.path, 'replacement', output, 1.68)
        replacement = campaign.allocate(self.path, 'replacement', self.root / 'replacement', 1.68)
        self.assertNotEqual(replacement['allocation_id'], manifest['campaign']['allocation_id'])
        self.assertAlmostEqual(campaign.remaining(self.path), 28.32)

    def test_any_execution_or_request_evidence_prevents_cancellation(self):
        for index, filename in enumerate(('execution.json', 'ledger.json', 'ledger.counts.json', 'seal.json')):
            name = 'marker-' + str(index)
            output, _ = self.frozen_study(name, 1)
            (output / filename).write_text('{}')
            with self.subTest(filename=filename), self.assertRaisesRegex(ValueError, 'may have started'):
                campaign.cancel(self.path, name, 'cannot discard execution')
            self.assertEqual(campaign.read(self.path)['allocations'][name]['status'], 'allocated')
        self.assertEqual(campaign.remaining(self.path), 26)
        output, _ = self.frozen_study('broken-marker', 1)
        (output / 'execution.json').symlink_to(output / 'missing-target')
        with self.assertRaisesRegex(ValueError, 'may have started'):
            campaign.cancel(self.path, 'broken-marker', 'broken marker still blocks')

    def test_cancel_requires_unchanged_manifest_freeze_and_retained_cancellation_proof(self):
        output, _ = self.frozen_study('changed', 1)
        manifest = output / 'manifest.json'
        manifest.write_text(manifest.read_text() + '\n')
        with self.assertRaisesRegex(ValueError, 'frozen configuration'):
            campaign.cancel(self.path, 'changed', 'invalid preflight proof')
        self.assertEqual(campaign.remaining(self.path), 29)
        output, _ = self.frozen_study('cancelled', 1)
        campaign.cancel(self.path, 'cancelled', 'unused configuration')
        # Even a semantically equivalent replacement freeze receipt is distinct
        # from the exact retained cancellation proof.
        frozen = output / 'freeze.json'
        frozen.write_text(frozen.read_text() + '\n')
        with self.assertRaisesRegex(ValueError, 'original reason or proof'):
            campaign.cancel(self.path, 'cancelled', 'unused configuration')
        value = json.loads(self.path.read_text())
        value['allocations']['cancelled']['cancellation']['reason'] = 'edited reason'
        self.path.write_text(json.dumps(value))
        with self.assertRaisesRegex(ValueError, 'retained proof'):
            campaign.read(self.path)

    def test_disjoint_stage_caps_cannot_exceed_whole_campaign(self):
        for name in ('cal-continue', 'cal-verify', 'cal-restart'):
            campaign.allocate(self.path, name, self.root / name, 1.68)
        campaign.allocate(self.path, 'development', self.root / 'development', 5.88)
        self.assertAlmostEqual(campaign.remaining(self.path), 19.08)
        receipt = campaign.allocate(self.path, 'scored', self.root / 'scored', campaign.remaining(self.path))
        self.assertAlmostEqual(receipt['ceiling_usd'], 19.08)
        self.assertEqual(campaign.remaining(self.path), 0)
        with self.assertRaisesRegex(ValueError, 'remaining campaign'):
            campaign.allocate(self.path, 'excess', self.root / 'excess', .01)
        self.assertAlmostEqual(campaign.read(self.path)['committed_usd'], 30)

    def test_allocation_idempotence_and_manifest_binding(self):
        output, manifest = self.sealed_study('phase', 2, [])
        receipt = manifest['campaign']
        self.assertEqual(campaign.verify_allocation(output, manifest), receipt)
        self.assertEqual(campaign.allocate(self.path, 'phase', output, 2,
                                           {'stage': 'scripted', 'budget': {'task_arm_usd': .6},
                                            'continuation': {'luna': 'continue'}}), receipt)
        for mutation in ('cap', 'stage', 'receipt', 'output', 'continuation'):
            changed = copy.deepcopy(manifest)
            location = output
            if mutation == 'cap':
                changed['budget']['study_usd'] = 3
            elif mutation == 'stage':
                changed['stage'] = 'development'
            elif mutation == 'receipt':
                changed['campaign']['allocation_id'] = 'other'
            elif mutation == 'continuation':
                changed['continuation']['luna'] = 'restart'
            else:
                location = self.root / 'another'
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                campaign.verify_allocation(location, changed)
        with self.assertRaises(ValueError):
            campaign.allocate(self.path, 'another-name', output, 2)
        with self.assertRaises(ValueError):
            campaign.allocate(self.path, 'phase', self.root / 'different-output', 2)
        self.assertEqual(len(campaign.read(self.path)['allocations']), 1)

    def test_settlement_releases_only_unused_cap_and_keeps_uncertain_reservations(self):
        entries = [dict(id='known', reserved=.5, cost=.1, status='complete', settled_at=3),
                   dict(id='failed', reserved=.2, status='failed'),
                   dict(id='pending', reserved=.3, status='pending')]
        output, manifest = self.sealed_study('calibration', 1.68, entries)
        self.assertAlmostEqual(campaign.remaining(self.path), 28.32)
        settled = campaign.settle(self.path, 'calibration')
        self.assertAlmostEqual(settled['known_usage_cost_usd'], .1)
        self.assertAlmostEqual(settled['uncertain_reserved_usd'], .5)
        self.assertAlmostEqual(settled['committed_cost_usd'], .6)
        self.assertEqual(settled['committed_decimal'], '0.6')
        self.assertAlmostEqual(campaign.remaining(self.path), 29.4)
        self.assertEqual(campaign.settle(self.path, 'calibration'), settled)
        self.assertAlmostEqual(campaign.remaining(self.path), 29.4)
        with self.assertRaisesRegex(ValueError, 'settled'):
            campaign.verify_allocation(output, manifest)
        self.assertEqual(campaign.verify_allocation(output, manifest, require_active=False), manifest['campaign'])
        campaign.allocate(self.path, 'scored', self.root / 'scored', campaign.remaining(self.path))
        self.assertEqual(campaign.remaining(self.path), 0)

    def test_unsealed_or_changed_evidence_cannot_release_capacity(self):
        campaign.allocate(self.path, 'unfinished', self.root / 'unfinished', 4)
        with self.assertRaises(FileNotFoundError):
            campaign.settle(self.path, 'unfinished')
        self.assertEqual(campaign.remaining(self.path), 26)
        output, _ = self.sealed_study('complete', 2, [])
        protocol.save(output / 'ledger.json', [{'tampered': True}])
        with self.assertRaises(ValueError):
            campaign.settle(self.path, 'complete')
        self.assertEqual(campaign.remaining(self.path), 24)

    def test_accounting_error_retains_reservation_and_halts_campaign(self):
        entries = [dict(id='bad-model', status='accounting_error', reserved=.5,
                        cost=.1, settled_at=1)]
        output, manifest = self.sealed_study('broken', 2, entries)
        settled = campaign.settle(self.path, 'broken')
        self.assertEqual(settled['committed_cost_usd'], .5)
        self.assertTrue(campaign.read(self.path)['halted'])
        with self.assertRaisesRegex(ValueError, 'halted'):
            campaign.allocate(self.path, 'retry', self.root / 'retry', 1)
        with self.assertRaisesRegex(ValueError, 'halted'):
            campaign.verify_allocation(output, manifest)

    def test_observed_overspend_is_recorded_and_cannot_release_more_capacity(self):
        entries = [dict(id='bound-failed', status='accounting_error', reserved=1, cost=3, settled_at=1)]
        self.sealed_study('overrun', 2, entries)
        settled = campaign.settle(self.path, 'overrun')
        self.assertEqual(settled['committed_cost_usd'], 3)
        self.assertEqual(campaign.read(self.path)['committed_usd'], 3)
        self.assertTrue(any('allocated cap' in error for error in settled['accounting_errors']))
        with self.assertRaisesRegex(ValueError, 'halted'):
            campaign.allocate(self.path, 'new', self.root / 'new', 1)

    def test_mutating_campaign_or_allocation_cap_invalidates_receipt(self):
        campaign.allocate(self.path, 'one', self.root / 'one', 1)
        original = json.loads(self.path.read_text())
        changed = copy.deepcopy(original)
        changed['ceiling_usd'] = 300
        self.path.write_text(json.dumps(changed))
        with self.assertRaisesRegex(ValueError, 'immutable receipt'):
            campaign.read(self.path)
        changed = copy.deepcopy(original)
        changed['allocations']['one']['ceiling_usd'] = .1
        self.path.write_text(json.dumps(changed))
        with self.assertRaisesRegex(ValueError, 'immutable receipt'):
            campaign.read(self.path)

    def test_two_processes_cannot_overallocate_from_stale_balance(self):
        context = multiprocessing.get_context('spawn')
        ready, results = context.Queue(), context.Queue()
        start = context.Event()
        processes = [context.Process(target=competing_allocation,
                                     args=(self.path, name, self.root / name, ready, start, results))
                     for name in ('one', 'two')]
        for process in processes:
            process.start()
        try:
            for _ in processes:
                ready.get(timeout=10)
            start.set()
            outcomes = [results.get(timeout=10)[0] for _ in processes]
            self.assertEqual(sorted(outcomes), ['allocated', 'refused'])
            self.assertEqual(campaign.read(self.path)['committed_usd'], 20)
            self.assertEqual(campaign.remaining(self.path), 10)
        finally:
            for process in processes:
                process.join(timeout=10)
                if process.is_alive():
                    process.terminate()
                    process.join()
            ready.close()
            results.close()

    def test_fractional_remaining_never_rounds_up_past_exact_funding(self):
        # Dollar calculations retain exact decimal strings from recorded numbers.
        entries = [dict(id='x', status='complete', reserved=1, cost=.1234567891234567, settled_at=1),
                   dict(id='y', status='complete', reserved=1, cost=.2345678912345678, settled_at=1)]
        self.sealed_study('precision', 2, entries)
        campaign.settle(self.path, 'precision')
        balance = campaign.remaining(self.path)
        campaign.allocate(self.path, 'all-rest', self.root / 'all-rest', balance)
        self.assertLessEqual(campaign.read(self.path)['committed_usd'], 30)
        self.assertGreaterEqual(campaign.remaining(self.path), 0)


if __name__ == '__main__':
    unittest.main()
