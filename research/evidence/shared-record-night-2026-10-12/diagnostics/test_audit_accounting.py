"""Read-only accounting replay checks; no harness imports or evaluator calls."""
from decimal import Decimal
import json
from pathlib import Path
import unittest

from audit_accounting import Audit, campaign_charge, charge_at, maximum_committed


ROOT = Path(__file__).resolve().parent


class AccountingAudit(unittest.TestCase):
    def test_sealed_verify_reproduces_initial_independent_audit(self):
        original = json.loads((ROOT / 'accounting-verify.json').read_text())
        report = Audit(ROOT / 'calibration-verify-min16').run()
        self.assertEqual(report['status'], 'pass', report['failures'])
        for field in ('generation_requests', 'provider_counts', 'generation_statuses',
                      'count_statuses', 'exact_input_matches', 'usage_totals',
                      'recomputed_usage_cost_usd', 'unknown_reservation_usd'):
            self.assertEqual(report[field], original[field], field)
        self.assertEqual(report['exact_input_matches'], 40)
        self.assertEqual(report['recomputed_usage_cost_usd'], '0.756791520')
        self.assertEqual(len(report['counts_without_generation']), 1)
        self.assertEqual(report['counts_without_generation'][0]['following_stop']['event'], 'budget_stop')

    def test_sealed_continue_retains_failed_call_as_unknown_reservation(self):
        report = Audit(ROOT / 'calibration-continue').run()
        self.assertEqual(report['status'], 'pass', report['failures'])
        self.assertEqual(report['generation_statuses'], {'complete': 30, 'failed': 1})
        self.assertEqual(report['exact_input_matches'], 30)
        self.assertEqual(report['recomputed_usage_cost_usd'], '0.710231735')
        self.assertLess(abs(Decimal(report['unknown_reservation_usd']) - Decimal('.089425')), Decimal('1e-12'))
        self.assertEqual(len(report['unresolved_requests']), 1)
        self.assertIn('minimum', report['unresolved_requests'][0]['error'].lower())

    def test_existing_scripted_archive_checks_shared_prefixes_without_replaying_them(self):
        study = ROOT.parent / 'shared-record-policy-paid-preflight-2026-10-12'
        report = Audit(study).run()
        self.assertEqual(report['status'], 'pass', report['failures'])
        attribution = report['prefix_billing_and_attribution']
        self.assertEqual(attribution['unique_billed_requests'], 306)
        self.assertEqual(attribution['arm_dependency_references'], 402)
        self.assertEqual(attribution['requests_attributed_to_multiple_arms'], 48)
        self.assertGreater(report['checks']['full_group_prefix_attributed_to_consuming_arm'], 0)
        self.assertGreater(report['checks']['prefix_request_archives_copied_exactly'], 0)
        billed = Decimal(report['actual_usage_cost_usd']) + Decimal(report['unknown_reservation_usd'])
        self.assertGreater(Decimal(attribution['total_attributed_across_arms_usd']), billed)

    def test_dispatch_cut_cannot_use_a_later_settlement_discount(self):
        first = dict(id='prefix', status='complete', reserved=.7, cost=.1, reserved_at=10, settled_at=20)
        failed = dict(id='continuation', status='failed', reserved=.2, reserved_at=12, settled_at=30)
        self.assertEqual(charge_at(first, 15), Decimal('.7'))
        self.assertEqual(charge_at(first, 25), Decimal('.1'))
        self.assertEqual(charge_at(failed, 40), Decimal('.2'))
        self.assertEqual(maximum_committed([first, failed]), Decimal('.9'))
        self.assertEqual(charge_at(first) + charge_at(failed), Decimal('.3'))

    def test_campaign_accounting_error_cannot_release_reserved_funds(self):
        failure = dict(status='accounting_error', reserved=.7, cost=.1)
        self.assertEqual(campaign_charge(failure), Decimal('.7'))
        failure['cost'] = .9
        self.assertEqual(campaign_charge(failure), Decimal('.9'))


if __name__ == '__main__':
    unittest.main()
