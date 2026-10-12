"""Provider and ledger qualification without model network calls."""
import hashlib
from io import BytesIO
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import luna


def config(method='context_bound'):
    return dict(model='scripted-model', rates=dict(input=1.0, cached_input=0.25, cache_write=1.0, output=2.0),
                context_tokens=1000, max_input_tokens=1000, max_output_tokens=500, service_tier='default',
                input_accounting=dict(method=method, qualification='scripted fixture; not live qualification'),
                qualification=dict(model='fixture identity', rates='fixture prices', limits='fixture limits'))


def response(payload, incoming=20, outgoing=10, cached=0, cache_write=0, output=None):
    return dict(id='scripted-response', model=payload['model'], status='completed',
                service_tier='default',
                usage=dict(input_tokens=incoming, output_tokens=outgoing,
                           input_tokens_details=dict(cached_tokens=cached, cache_write_tokens=cache_write)),
                output=output or [])


def long_config(method='qualified_counter'):
    result = config(method)
    result.update(rates=dict(input=0.10, cached_input=0.01, cache_write=0.125, output=0.50),
                  context_tokens=1050000, max_input_tokens=922000, max_output_tokens=128000,
                  long_context_pricing=dict(threshold_tokens=272000, input_multiplier=2.0,
                                            output_multiplier=1.5))
    return result


class MeteredSessions(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.ledger = luna.Ledger(self.root / 'ledger.json', ceiling=1.0)
        blocker = patch('urllib.request.urlopen', side_effect=AssertionError('network forbidden in tests'))
        blocker.start()
        self.addCleanup(blocker.stop)
        credential = patch.dict('os.environ', {'OPENAI_API_KEY': 'test-not-a-real-key'})
        credential.start()
        self.addCleanup(credential.stop)

    def session(self, **options):
        defaults = dict(model_config=config(), transport=response)
        defaults.update(options)
        return luna.Session('system', [], self.ledger, 'scored/block/prefix/worker',
                            'scored', 1.0, defaults.pop('allowance', 0.1), **defaults)

    def test_default_cannot_dispatch_or_reserve_even_with_a_credential(self):
        session = self.session(transport=None)
        with patch.dict('os.environ', {'OPENAI_API_KEY': 'test-not-a-real-key'}):
            with self.assertRaises(luna.PaidCallsDisabled):
                session.request()
        self.assertEqual(self.ledger.entries, [])

    def test_live_requires_unchanged_manifest_and_matching_configuration(self):
        frozen = self.root / 'manifest.json'
        frozen.write_text(json.dumps({'models': {'luna': config()}}))
        proof = dict(path=str(frozen), sha256=hashlib.sha256(frozen.read_bytes()).hexdigest(), model_key='luna')
        with patch.dict('os.environ', {'OPENAI_API_KEY': 'test-not-a-real-key'}):
            with self.assertRaises(luna.PaidCallsDisabled):
                self.session(transport=None, paid_opt_in=True).request()
            changed_config = config()
            changed_config['rates']['input'] = 2.0
            with self.assertRaises(luna.PaidCallsDisabled):
                self.session(model_config=changed_config, transport=None,
                             paid_opt_in=True, live_manifest=proof).request()
            session = self.session(transport=None, paid_opt_in=True, live_manifest=proof)
            with patch('luna._http', side_effect=response) as fake:
                session.request()
                self.assertEqual(fake.call_count, 1)
                frozen.write_text(frozen.read_text() + '\n')
                with self.assertRaises(luna.PaidCallsDisabled):
                    session.request()
                self.assertEqual(fake.call_count, 1)
        self.assertEqual(len(self.ledger.entries), 1)

    def test_missing_live_qualification_never_reserves(self):
        unqualified = config()
        unqualified['qualification']['rates'] = ''
        frozen = self.root / 'manifest.json'
        frozen.write_text(json.dumps({'models': {'luna': unqualified}}))
        proof = dict(path=str(frozen), sha256=hashlib.sha256(frozen.read_bytes()).hexdigest(), model_key='luna')
        with self.assertRaises(luna.PaidCallsDisabled):
            self.session(model_config=unqualified, transport=None, paid_opt_in=True, live_manifest=proof).request()
        self.assertEqual(self.ledger.entries, [])

    def test_context_bound_reserves_input_and_budget_derived_output_before_dispatch(self):
        observed = []
        def transport(payload):
            observed.append(payload)
            pending = json.loads(self.ledger.path.read_text())[-1]
            self.assertEqual(pending['status'], 'pending')
            self.assertEqual(pending['payload'], payload)
            self.assertEqual(pending['input_bound'], 1000)
            self.assertAlmostEqual(pending['reserved'], 0.0011)
            return response(payload)
        self.session(transport=transport, allowance=0.0011).request()
        self.assertEqual(observed[0]['max_output_tokens'], 50)
        self.assertEqual(len(self.ledger.entries), 1)

    def test_counter_receives_all_input_components_and_respects_context_room(self):
        captured = []
        def counter(payload):
            captured.append(payload)
            return 980
        session = self.session(model_config=config('qualified_counter'), input_counter=counter,
                               transport=lambda p: response(p, incoming=980, outgoing=8))
        session.tools = [dict(name='evaluate', description='full tool definition',
                             input_schema=dict(type='object', properties={}))]
        session.user('all input text')
        session.history.append({'type': 'reasoning', 'encrypted_content': 'opaque-state'})
        session.request()
        self.assertEqual(captured[0]['instructions'], 'system')
        self.assertEqual(captured[0]['input'], session.archive[0]['payload']['input'])
        self.assertEqual(captured[0]['tools'][0]['description'], 'full tool definition')
        self.assertEqual(session.archive[0]['payload']['max_output_tokens'], 20)
        self.assertEqual(session.archive[0]['response']['usage']['input_tokens'], 980)

    def test_counter_required_and_unfunded_requests_never_dispatch(self):
        with self.assertRaises(ValueError):
            self.session(model_config=config('qualified_counter')).request()
        with self.assertRaises(luna.BudgetStop):
            self.session(allowance=0.0001).request()
        self.assertEqual(self.ledger.entries, [])

    def test_cache_charges_complete_archive_and_durable_reload(self):
        session = self.session(transport=lambda p: response(p, incoming=100, outgoing=20, cached=80))
        session.user('hello')
        session.request()
        expected = (20 * 1.0 + 80 * 0.25 + 20 * 2.0) / 1e6
        self.assertAlmostEqual(session.spent(), expected)
        entry = self.ledger.entries[0]
        self.assertEqual(entry['rates'], config()['rates'])
        self.assertEqual(entry['payload'], session.archive[0]['payload'])
        self.assertEqual(entry['response'], session.archive[0]['response'])
        reloaded = luna.Ledger(self.ledger.path, 1.0)
        self.assertEqual(reloaded.entries, self.ledger.entries)
        self.assertAlmostEqual(reloaded.snapshot(session.request_ids)['known_cost'], expected)

    def test_forks_preserve_prefix_but_bill_it_once(self):
        prefix = self.session()
        prefix.user('independent work')
        prefix.request()
        prefix_cost = prefix.spent()
        private = prefix.fork('scored/block/C/worker', transport=lambda p: response(p, outgoing=20))
        peer = prefix.fork('scored/block/P/worker', transport=lambda p: response(p, outgoing=30))
        self.assertEqual(private.history, peer.history)
        self.assertEqual(private.request_ids, peer.request_ids)
        self.assertEqual(private.billing_spent(), 0.0)
        private.user('private packet')
        peer.user('peer packet')
        self.assertNotEqual(private.history, peer.history)
        self.assertEqual(prefix.history, [{'role': 'user', 'content': 'independent work'}])
        private.request()
        peer.request()
        self.assertAlmostEqual(private.spent(), prefix_cost + private.billing_spent())
        self.assertAlmostEqual(peer.spent(), prefix_cost + peer.billing_spent())
        ids = private.request_ids + peer.request_ids + prefix.request_ids
        snapshot = self.ledger.snapshot(ids)
        self.assertEqual(len(snapshot['request_ids']), 3)
        self.assertAlmostEqual(snapshot['committed_cost'], self.ledger.spent())
        self.assertLess(self.ledger.spent(), private.spent() + peer.spent())
        self.assertEqual(len(private.fork('scored/block/C2/worker').request_ids), 2)

    def test_failure_keeps_reservation_and_stops_without_retry(self):
        calls = []
        def transport(payload):
            calls.append(payload)
            raise TimeoutError('unknown provider outcome')
        session = self.session(transport=transport)
        with self.assertRaises(luna.TransportFailure):
            session.request()
        entry = self.ledger.entries[0]
        snapshot = self.ledger.snapshot(session.request_ids)
        self.assertEqual(snapshot['known_cost'], 0.0)
        self.assertEqual(snapshot['failed_ids'], [entry['id']])
        self.assertEqual(snapshot['unknown_reserved'], entry['reserved'])
        with self.assertRaises(luna.TransportFailure):
            session.request()
        with self.assertRaises(luna.TransportFailure):
            session.fork('scored/block/failed-continuation').request()
        self.assertEqual(len(calls), 1)

    def test_wrong_model_is_not_priced_using_requested_model_rates(self):
        def wrong(payload):
            return dict(response(payload), model='different-model')
        session = self.session(transport=wrong)
        with self.assertRaises(luna.AccountingFailure):
            session.request()
        entry = self.ledger.entries[0]
        self.assertNotIn('cost', entry)
        self.assertEqual(entry['returned_model'], 'different-model')
        self.assertEqual(session.spent(), entry['reserved'])
        self.assertEqual(session.archive[0]['status'], 'accounting_error')

    def test_invalid_bound_fails_closed_and_preserves_usage(self):
        session = self.session(model_config=config('qualified_counter'), input_counter=lambda _: 5)
        with self.assertRaises(luna.AccountingFailure):
            session.request()
        self.assertEqual(self.ledger.entries[0]['usage']['input_tokens'], 20)
        self.assertIn('input usage exceeded', self.ledger.entries[0]['error'])
        with self.assertRaises(luna.TransportFailure):
            session.request()

    def test_accounting_error_halts_other_sessions_and_survives_reload(self):
        broken = self.session(model_config=config('qualified_counter'), input_counter=lambda _: 5)
        with self.assertRaises(luna.AccountingFailure):
            broken.request()
        unrelated = self.session(transport=lambda _: self.fail('study must not dispatch again'))
        with self.assertRaises(luna.AccountingFailure):
            unrelated.request()
        reloaded = luna.Ledger(self.ledger.path, 1.0)
        unrelated.ledger = reloaded
        with self.assertRaises(luna.AccountingFailure):
            unrelated.request()
        self.assertEqual(len(reloaded.entries), 1)
        self.assertEqual(reloaded.entries[0]['usage']['input_tokens'], 20)

    def test_missing_usage_keeps_full_unknown_reservation(self):
        session = self.session(transport=lambda payload: dict(model=payload['model'], output=[]))
        with self.assertRaises(luna.AccountingFailure):
            session.request()
        entry = self.ledger.entries[0]
        self.assertNotIn('cost', entry)
        self.assertEqual(entry['status'], 'accounting_error')
        snapshot = self.ledger.snapshot(session.request_ids)
        self.assertEqual(snapshot['known_cost'], 0.0)
        self.assertEqual(snapshot['unknown_reserved'], entry['reserved'])
        self.assertEqual(snapshot['failed_ids'], [entry['id']])
        self.assertEqual(entry['response']['output'], [])

    def test_missing_cache_usage_is_not_labeled_as_a_known_charge(self):
        def no_cache_details(payload):
            result = response(payload)
            del result['usage']['input_tokens_details']
            return result
        session = self.session(transport=no_cache_details)
        with self.assertRaises(luna.AccountingFailure):
            session.request()
        self.assertEqual(self.ledger.snapshot(session.request_ids)['known_cost'], 0.0)
        self.assertGreater(self.ledger.snapshot(session.request_ids)['unknown_reserved'], 0.0)

    def test_transport_error_is_retained_without_credential(self):
        def fail(_):
            raise RuntimeError('service refused token test-secret-value with diagnostic detail')
        session = self.session(transport=fail)
        with patch.dict('os.environ', {'OPENAI_API_KEY': 'test-secret-value'}):
            with self.assertRaises(luna.TransportFailure):
                session.request()
        error = self.ledger.entries[0]['error']
        self.assertIn('diagnostic detail', error)
        self.assertIn('<redacted>', error)
        self.assertNotIn('test-secret-value', json.dumps(session.archive))

    def test_historical_cut_counts_inflight_work_and_deduplicates_dependencies(self):
        session = self.session()
        session.request()
        entry = self.ledger.entries[0]
        entry['reserved_at'], entry['settled_at'] = 10.0, 30.0
        ident = entry['id']
        self.assertEqual(self.ledger.snapshot([ident], at=9)['committed_cost'], 0.0)
        at_twenty = self.ledger.snapshot([ident, ident], at=20)
        self.assertEqual(at_twenty['known_cost'], 0.0)
        self.assertEqual(at_twenty['committed_cost'], entry['reserved'])
        self.assertEqual(at_twenty['pending_ids'], [ident])
        self.assertEqual(self.ledger.snapshot([ident], at=30)['known_cost'], entry['cost'])
        with self.assertRaises(ValueError):
            self.ledger.snapshot(['missing-prefix'])

    def test_phase_and_study_ceilings_include_pending_reservations(self):
        session = self.session()
        arguments = dict(model_config=config(), payload=session.payload(), input_bound=1000,
                         max_output_tokens=500, session_id='pending-session')
        self.ledger.reserve('first', 'scored', 0.7, 0.8, **arguments)
        with self.assertRaises(luna.BudgetStop):
            self.ledger.reserve('second', 'scored', 0.2, 0.8, **arguments)
        with self.assertRaises(luna.BudgetStop):
            self.ledger.reserve('third', 'development', 0.4, 1.0, **arguments)
        session.request_ids = [self.ledger.entries[0]['id']]
        with self.assertRaises(ValueError):
            session.fork('pending-fork')

    def test_incomplete_tool_call_is_archived_but_not_replayed(self):
        outputs = [dict(type='reasoning', encrypted_content='opaque-state'),
                   dict(type='function_call', call_id='good', name='evaluate', arguments='{"source":"complete"}'),
                   dict(type='function_call', call_id='bad', name='evaluate', arguments='{"source":'),
                   dict(type='message', content=[dict(type='output_text', text='visible response')])]
        session = self.session(transport=lambda p: response(p, output=outputs))
        calls, visible, status = session.request()
        self.assertEqual([item[0] for item in calls], ['good'])
        self.assertTrue(any('discarded' in item for item in visible))
        self.assertEqual(status, 'completed')
        self.assertEqual(len(session.archive[0]['response']['output']), 4)
        self.assertEqual(len(session.history), 3)
        session.results([('good', {'answer': 1})])
        self.assertEqual(json.loads(session.history[-1]['output']), {'answer': 1})

    def test_reused_label_has_fresh_session_accounting(self):
        first = self.session()
        first.request()
        second = self.session()
        self.assertEqual(first.label, second.label)
        self.assertEqual(second.spent(), 0.0)
        second.request()
        self.assertNotEqual(first.request_ids, second.request_ids)
        self.assertAlmostEqual(self.ledger.spent(), first.spent() + second.spent())

    def test_cache_writes_are_exclusive_and_reserve_the_highest_input_rate(self):
        model = config('qualified_counter')
        model['rates']['cache_write'] = 1.25
        session = self.session(model_config=model, input_counter=lambda _: 100,
                               transport=lambda p: response(p, incoming=100, cached=30,
                                                            cache_write=50, outgoing=20))
        session.request()
        entry = self.ledger.entries[-1]
        expected = (20 * 1.0 + 30 * 0.25 + 50 * 1.25 + 20 * 2.0) / 1e6
        self.assertAlmostEqual(entry['cost'], expected)
        self.assertAlmostEqual(entry['reserved'], (100 * 1.25 + 500 * 2.0) / 1e6)
        self.assertEqual(entry['usage']['input_tokens_details']['cache_write_tokens'], 50)

    def test_invalid_cache_write_partition_keeps_unknown_reservation(self):
        session = self.session(transport=lambda p: response(p, incoming=20, cached=15, cache_write=10))
        with self.assertRaisesRegex(luna.AccountingFailure, 'exceed input'):
            session.request()
        self.assertEqual(self.ledger.snapshot()['known_cost'], 0)
        self.assertEqual(self.ledger.snapshot()['unknown_reserved'], self.ledger.entries[-1]['reserved'])

    def test_distinct_cache_write_price_requires_usage_detail(self):
        model = config()
        model['rates']['cache_write'] = 1.25
        def absent(payload):
            result = response(payload)
            del result['usage']['input_tokens_details']['cache_write_tokens']
            return result
        with self.assertRaisesRegex(luna.AccountingFailure, 'missing cache-write'):
            self.session(model_config=model, transport=absent).request()
        self.assertNotIn('cost', self.ledger.entries[-1])

    def test_equal_cache_write_price_can_omit_its_detail(self):
        def absent(payload):
            result = response(payload)
            del result['usage']['input_tokens_details']['cache_write_tokens']
            return result
        self.session(transport=absent).request()
        self.assertAlmostEqual(self.ledger.entries[-1]['cost'], (20 * 1.0 + 10 * 2.0) / 1e6)

    def test_long_context_threshold_applies_to_all_tokens_and_output(self):
        for incoming, multiplier, output_multiplier in ((272000, 1, 1), (272001, 2, 1.5)):
            with self.subTest(incoming=incoming):
                session = self.session(model_config=long_config(), input_counter=lambda _: incoming,
                                       allowance=0.3,
                                       transport=lambda p: response(p, incoming=incoming, outgoing=1000,
                                                                    cached=20000, cache_write=50000))
                session.request()
                entry = self.ledger.entries[-1]
                expected = (((incoming - 70000) * .1 + 20000 * .01 + 50000 * .125) * multiplier
                            + 1000 * .5 * output_multiplier) / 1e6
                reserved = (incoming * .125 * multiplier + 128000 * .5 * output_multiplier) / 1e6
                self.assertAlmostEqual(entry['cost'], expected)
                self.assertAlmostEqual(entry['reserved'], reserved)
                self.assertEqual(entry['charged_rates']['cache_write'], .125 * multiplier)
                self.assertEqual(entry['charged_rates']['cached_input'], .01 * multiplier)

    def test_context_bound_reserves_long_rates_but_settles_actual_short_usage(self):
        session = self.session(model_config=long_config('context_bound'), allowance=0.5)
        session.request()
        entry = self.ledger.entries[-1]
        self.assertEqual(entry['input_bound'], 922000)
        self.assertAlmostEqual(entry['reserved'], (922000 * .25 + 128000 * .75) / 1e6)
        self.assertAlmostEqual(entry['cost'], (20 * .1 + 10 * .5) / 1e6)
        self.assertEqual(entry['reservation_rates']['output'], .75)
        self.assertEqual(entry['charged_rates']['output'], .5)

    def test_provider_count_projects_exact_inputs_and_is_durable_before_generation(self):
        observed = []
        def count(payload):
            observed.append(payload)
            audit = json.loads(self.ledger.count_path.read_text())[-1]
            self.assertEqual(audit['status'], 'pending')
            self.assertEqual(audit['payload'], payload)
            return dict(object='response.input_tokens', input_tokens=980)
        def generate(payload):
            audit = json.loads(self.ledger.count_path.read_text())[-1]
            self.assertEqual(audit['status'], 'complete')
            self.assertEqual(self.ledger.entries[-1]['input_count_id'], audit['id'])
            return response(payload, incoming=980, outgoing=8)
        session = self.session(model_config=config('provider_count'), count_transport=count, transport=generate)
        session.tools = [dict(name='evaluate', description='full tool definition',
                             input_schema=dict(type='object', properties={}))]
        session.user('all input text')
        session.history.extend([{'type': 'reasoning', 'encrypted_content': 'opaque-state'},
                                {'type': 'function_call_output', 'call_id': 'previous', 'output': 'full result'}])
        expected = session.payload()
        session.request()
        self.assertEqual(set(observed[0]), set(luna.COUNT_INPUT_FIELDS))
        for field in luna.COUNT_INPUT_FIELDS:
            self.assertEqual(observed[0][field], expected[field])
        self.assertEqual(session.archive[0]['payload']['max_output_tokens'], 20)
        self.assertEqual(session.archive[0]['input_count'], session.count_archive[0])
        reloaded = luna.Ledger(self.ledger.path, 1.0)
        self.assertEqual(reloaded.counts, self.ledger.counts)
        self.assertEqual(len(reloaded.entries), 1)

    def test_provider_count_live_call_requires_opt_in_and_unchanged_manifest(self):
        model = config('provider_count')
        session = self.session(model_config=model)
        with patch('luna._http_count') as count:
            with self.assertRaises(luna.PaidCallsDisabled):
                session.request()
            self.assertEqual(count.call_count, 0)
        frozen = self.root / 'manifest.json'
        frozen.write_text(json.dumps({'models': {'luna': model}}))
        proof = dict(path=str(frozen), sha256=hashlib.sha256(frozen.read_bytes()).hexdigest(), model_key='luna')
        live = self.session(model_config=model, transport=None, paid_opt_in=True, live_manifest=proof)
        with patch.dict('os.environ', {'OPENAI_API_KEY': 'test-not-a-real-key'}):
            with patch('luna._http_count', return_value=dict(object='response.input_tokens', input_tokens=20)) as count:
                with patch('luna._http', side_effect=response) as generate:
                    live.request()
                    self.assertEqual(count.call_count, 1)
                    self.assertEqual(generate.call_count, 1)
                    frozen.write_text(frozen.read_text() + '\n')
                    with self.assertRaises(luna.PaidCallsDisabled):
                        live.request()
                    self.assertEqual(count.call_count, 1)
                    self.assertEqual(generate.call_count, 1)

    def test_provider_count_forks_archive_prefix_and_count_each_new_payload(self):
        counted = []
        def count(payload):
            counted.append(payload)
            return dict(object='response.input_tokens', input_tokens=20)
        prefix = self.session(model_config=config('provider_count'), count_transport=count)
        prefix.user('prefix')
        prefix.request()
        child = prefix.fork('scored/block/C/worker')
        self.assertEqual(child.count_archive, prefix.count_archive)
        self.assertEqual(child.billing_spent(), 0)
        child.user('new packet')
        child.request()
        self.assertEqual(len(counted), 2)
        self.assertNotEqual(counted[0]['input'], counted[1]['input'])
        self.assertEqual(len(prefix.count_archive), 1)
        self.assertEqual(len(child.count_archive), 2)
        self.assertEqual(len(self.ledger.counts), 2)
        self.assertEqual(len(self.ledger.entries), 2)

    def test_provider_count_failure_never_dispatches_generation_or_falls_back(self):
        def fail(_):
            raise TimeoutError('counter unavailable')
        session = self.session(model_config=config('provider_count'), count_transport=fail,
                               transport=lambda _: self.fail('must not generate'))
        with self.assertRaises(luna.TransportFailure):
            session.request()
        with self.assertRaises(luna.TransportFailure):
            session.request()
        self.assertEqual(self.ledger.entries, [])
        self.assertEqual(len(self.ledger.counts), 1)
        self.assertEqual(self.ledger.counts[0]['status'], 'failed')

    def test_invalid_provider_count_halts_accounting_and_survives_reload(self):
        for index, result in enumerate(({}, {'object': 'response.input_tokens', 'input_tokens': True},
                                        {'object': 'response.input_tokens', 'input_tokens': -1},
                                        {'object': 'unexpected', 'input_tokens': 20})):
            with self.subTest(result=result):
                self.ledger = luna.Ledger(self.root / f'case-{index}.json', 1)
                session = self.session(model_config=config('provider_count'), count_transport=lambda _: result,
                                       transport=lambda _: self.fail('must not generate'))
                with self.assertRaises(luna.AccountingFailure):
                    session.request()
                self.assertEqual(self.ledger.entries, [])
                self.assertEqual(self.ledger.counts[0]['status'], 'accounting_error')
                self.assertEqual(self.ledger.counts[0]['response'], result)
                self.ledger = luna.Ledger(self.ledger.path, 1)
                with self.assertRaises(luna.AccountingFailure):
                    self.session().request()

    def test_counted_input_over_provider_limit_never_generates(self):
        model = config('provider_count')
        model['max_input_tokens'] = 900
        session = self.session(model_config=model, count_transport=lambda _: dict(
            object='response.input_tokens', input_tokens=901),
            transport=lambda _: self.fail('must not generate'))
        with self.assertRaisesRegex(luna.BudgetStop, 'provider input limit'):
            session.request()
        self.assertEqual(self.ledger.entries, [])
        self.assertEqual(self.ledger.counts[0]['status'], 'complete')

    def test_unqualified_counter_projection_field_never_counts(self):
        session = self.session(model_config=config('provider_count'),
                               count_transport=lambda _: self.fail('must not count'))
        payload = dict(session.payload(), new_input_field='cannot silently drop input')
        with self.assertRaisesRegex(ValueError, 'unqualified payload fields'):
            session.provider_count(payload)
        self.assertEqual(self.ledger.counts, [])

    def test_returned_service_tier_mismatch_is_not_priced_as_default(self):
        session = self.session(transport=lambda p: dict(response(p), service_tier='priority'))
        with self.assertRaisesRegex(luna.AccountingFailure, 'service tier'):
            session.request()
        self.assertNotIn('cost', self.ledger.entries[-1])
        self.assertEqual(self.ledger.entries[-1]['returned_service_tier'], 'priority')

    def test_live_response_without_service_tier_is_unknown_cost(self):
        model = config()
        frozen = self.root / 'manifest.json'
        frozen.write_text(json.dumps({'models': {'luna': model}}))
        proof = dict(path=str(frozen), sha256=hashlib.sha256(frozen.read_bytes()).hexdigest(), model_key='luna')
        session = self.session(model_config=model, transport=None, paid_opt_in=True, live_manifest=proof)
        def absent(payload):
            value = response(payload)
            del value['service_tier']
            return value
        with patch.dict('os.environ', {'OPENAI_API_KEY': 'test-not-a-real-key'}):
            with patch('luna._http', side_effect=absent):
                with self.assertRaisesRegex(luna.AccountingFailure, 'missing provider service tier'):
                    session.request()
        self.assertNotIn('cost', self.ledger.entries[-1])

    def test_http_count_uses_documented_post_endpoint_without_generation(self):
        model = config('provider_count')
        frozen = self.root / 'manifest.json'
        frozen.write_text(json.dumps({'models': {'luna': model}}))
        proof = dict(path=str(frozen), sha256=hashlib.sha256(frozen.read_bytes()).hexdigest(), model_key='luna')
        session = self.session(model_config=model, paid_opt_in=True, live_manifest=proof)
        session.user('count this complete message')
        value = dict(object='response.input_tokens', input_tokens=20)
        def local_http(request):
            self.assertEqual(request.full_url, 'https://api.openai.com/v1/responses/input_tokens')
            self.assertEqual(request.get_method(), 'POST')
            self.assertEqual(json.loads(request.data)['input'], session.history)
            self.assertEqual(request.headers['Authorization'], 'Bearer test-not-a-real-key')
            return BytesIO(json.dumps(value).encode())
        with patch('urllib.request.urlopen', side_effect=local_http) as http:
            session.request()
        self.assertEqual(http.call_count, 1)
        self.assertEqual(len(self.ledger.entries), 1)
        self.assertEqual(self.ledger.counts[-1]['response'], value)

    def test_manifest_mutation_during_provider_count_prevents_generation(self):
        model = config('provider_count')
        frozen = self.root / 'manifest.json'
        frozen.write_text(json.dumps({'models': {'luna': model}}))
        proof = dict(path=str(frozen), sha256=hashlib.sha256(frozen.read_bytes()).hexdigest(), model_key='luna')
        session = self.session(model_config=model, transport=None, paid_opt_in=True, live_manifest=proof)
        def count(_):
            frozen.write_text(frozen.read_text() + '\n')
            return dict(object='response.input_tokens', input_tokens=20)
        with patch('luna._http_count', side_effect=count):
            with patch('luna._http', side_effect=AssertionError('must not generate')):
                with self.assertRaisesRegex(luna.PaidCallsDisabled, 'manifest hash changed'):
                    session.request()
        self.assertEqual(self.ledger.entries, [])
        self.assertEqual(self.ledger.counts[-1]['status'], 'complete')

    def test_pricing_and_limits_must_be_explicit(self):
        for field in ('cache_write',):
            model = config()
            del model['rates'][field]
            with self.assertRaises(KeyError):
                luna.validate_model_config(model)
        for field in ('service_tier', 'max_input_tokens'):
            model = config()
            del model[field]
            with self.assertRaises(KeyError):
                luna.validate_model_config(model)
        model = config()
        model['service_tier'] = 'auto'
        with self.assertRaisesRegex(ValueError, 'service tier'):
            luna.validate_model_config(model)

    def test_budget_below_provider_output_minimum_never_reserves_or_dispatches(self):
        model = config('qualified_counter')
        model['rates']['output'] = 1.0
        session = self.session(model_config=model, input_counter=lambda _: 0, allowance=15 / 1e6,
                               transport=lambda _: self.fail('must not dispatch below provider minimum'))
        with self.assertRaisesRegex(luna.BudgetStop, 'provider minimum of 16'):
            session.request()
        self.assertEqual(self.ledger.entries, [])
        self.assertEqual(session.archive, [])

    def test_context_below_provider_output_minimum_retains_count_without_generation(self):
        session = self.session(model_config=config('provider_count'), count_transport=lambda _: dict(
            object='response.input_tokens', input_tokens=985),
            transport=lambda _: self.fail('must not dispatch below provider minimum'))
        with self.assertRaisesRegex(luna.BudgetStop, 'provider minimum of 16'):
            session.request()
        self.assertEqual(self.ledger.entries, [])
        self.assertEqual(session.archive, [])
        self.assertEqual(self.ledger.counts[-1]['status'], 'complete')
        self.assertEqual(self.ledger.counts[-1]['response']['input_tokens'], 985)

    def test_exact_provider_minimum_context_room_dispatches_sixteen(self):
        session = self.session(model_config=config('qualified_counter'), input_counter=lambda _: 984,
                               transport=lambda p: response(p, incoming=984, outgoing=10))
        session.request()
        self.assertEqual(session.archive[-1]['payload']['max_output_tokens'], 16)
        self.assertEqual(self.ledger.entries[-1]['status'], 'complete')

    def test_configured_output_limit_cannot_be_below_provider_minimum(self):
        model = config()
        model['max_output_tokens'] = 15
        with self.assertRaisesRegex(ValueError, 'Responses API minimum of 16'):
            luna.validate_model_config(model)


if __name__ == '__main__':
    unittest.main()
