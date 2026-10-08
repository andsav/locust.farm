import io
import json
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import patch
import urllib.error

from study import (MODELS, analyze, code_hashes, digest, label, nt, read_attempts,
                   request_body, selected, verify_requests)
from runner import Run, cost


def fixture():
    cases = [{'id': q, 'cluster': q, 'outcome': 1, 'public': {'question': 'Will event occur?',
               'information_cutoff': '2026-01-01', 'evidence': [{'id': 'brief', 'text': 'Known evidence'}]}}
             for q in ('a', 'b')]
    return {'base': {'seed': 1, 'base_prompt': nt.BASE, 'initial_prompt': nt.INITIAL,
                     'review_prompt': nt.REVIEW, 'profile_blocks': {p: ['Profile '+str(i) for i in range(10)] for p in nt.CONDITIONS},
                     'cases': cases}, 'mixed_models': {q: ['luna', 'sonnet']*5 for q in ('a', 'b')},
            'models': MODELS, 'cohorts': {'a': 'previous', 'b': 'additional'}, 'baseline_accounted_usd': 0,
            'maximum_attempts': 6, 'retry_delays_seconds': [5, 15, 30, 60, 60], 'new_spend_ceiling_usd': 100,
            'code_hashes': code_hashes(), 'max_concurrency': 2}


def starts(manifest):
    attempts = []
    for c in manifest['base']['cases']:
        for family, probability in (('luna', .2), ('sonnet', .8)):
            for i in range(10):
                name = label(c['id'], 'neutral', family, 'initial', i)
                _, body = request_body(manifest, {}, c, 'neutral', family, 'initial', i)
                attempts.append({'label': name, 'attempt_id': name+'#1', 'provider': family, 'status': 'completed',
                                 'text': json.dumps({'probability': probability, 'evidence_ids': ['brief'],
                                                     'explanation': family+' fixture', 'used_peer_ids': []}),
                                 'request_sha256': digest(body), 'reserved_usd': 0, 'cost_usd': 0})
    return attempts


def public(body):
    return json.loads(body['input'] if 'input' in body else body['messages'][0]['content'])


class StudyTests(unittest.TestCase):
    def test_mixed_peers_use_the_correct_models_without_names(self):
        m = fixture(); c = m['base']['cases'][0]; records = selected(starts(m))
        family, body = request_body(m, records, c, 'neutral', 'mixed', 'exchange', 0)
        data = public(body)
        self.assertEqual(family, 'luna')
        self.assertEqual(len(data['peer_forecasts']), 9)
        self.assertNotIn(0, [p['peer_id'] for p in data['peer_forecasts']])
        for p in data['peer_forecasts']:
            self.assertEqual(p['probability'], .8 if p['peer_id'] % 2 else .2)
            self.assertNotIn('provider', p)
        self.assertNotIn('outcome', data);self.assertNotIn('cluster', data)

    def test_private_and_mixed_exchange_share_self_and_instruction(self):
        m = fixture();c = m['base']['cases'][0];r = selected(starts(m))
        _, a = request_body(m, r, c, 'neutral', 'sonnet', 'private', 1)
        _, b = request_body(m, r, c, 'neutral', 'mixed', 'exchange', 1)
        self.assertEqual(a['system'], b['system'])
        x, y = public(a), public(b);y['peer_forecasts'] = []
        self.assertEqual(x, y)

    def test_invalid_initial_remains_explicit_in_both_revision_branches(self):
        m = fixture();c = m['base']['cases'][0]
        for pop, stage in (('luna', 'private'), ('mixed', 'exchange')):
            _, body = request_body(m, {}, c, 'neutral', pop, stage, 0)
            self.assertEqual(public(body)['initial_forecast'], {'status': 'unavailable_or_invalid'})

    def test_mixed_initial_is_the_actual_five_plus_five_mean(self):
        m = fixture();s = analyze(m, starts(m))
        self.assertAlmostEqual(s['groups']['all/neutral/mixed/initial']['cluster_equal_brier'], .25)
        self.assertAlmostEqual(s['groups']['all/neutral/sonnet/initial']['cluster_equal_brier'], .04)
        self.assertAlmostEqual(s['groups']['all/neutral/luna/initial']['cluster_equal_brier'], .64)

    def test_request_tampering_detected(self):
        m = fixture();attempts = starts(m);verify_requests(m, attempts)
        attempts[0]['request_sha256'] = 'changed'
        with self.assertRaises(ValueError):verify_requests(m, attempts)

    def test_first_non_transport_result_retained_even_when_incomplete(self):
        a = [{'label': 'a', 'status': 'transport_error'}, {'label': 'a', 'status': 'incomplete'},
             {'label': 'a', 'status': 'completed'}]
        self.assertEqual(selected(a)['a']['status'], 'incomplete')

    def test_provider_usage_accounts_for_cache_classes(self):
        usage = {'input_tokens': 100, 'output_tokens': 20, 'cache_creation_input_tokens': 50,
                 'cache_creation': {'ephemeral_1h_input_tokens': 10}, 'cache_read_input_tokens': 100}
        self.assertEqual(cost('sonnet', usage, MODELS['sonnet']), (.00055, 250))
        self.assertEqual(cost('luna', {'input_tokens': 100, 'output_tokens': 20}, MODELS['luna']), (.0000225, 100))

    def new_runner(self, folder, m):
        (folder/'manifest.json').write_text(json.dumps(m))
        (folder/'manifest.sha256').write_text(digest(m));(folder/'attempts.jsonl').write_text('')
        return Run(folder)

    def test_retry_journal_keeps_failure_and_does_not_store_thinking(self):
        m = fixture();c = m['base']['cases'][0]
        value = {'model': MODELS['sonnet']['id'], 'stop_reason': 'end_turn',
                 'usage': {'input_tokens': 100, 'output_tokens': 20},
                 'content': [{'type': 'thinking', 'thinking': 'DO NOT SAVE PRIVATE CONTENT'},
                             {'type': 'text', 'text': '{}'}]}
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp);r = self.new_runner(folder, m)
            with patch.dict('os.environ', {'ANTHROPIC_API_KEY': 'unit-placeholder'}), \
                 patch('urllib.request.urlopen', side_effect=[urllib.error.URLError('fixture'), io.StringIO(json.dumps(value))]) as api, \
                 patch.object(r.stop, 'wait', return_value=False):
                r.invoke(c, 'neutral', 'sonnet', 'initial', 0)
                r.invoke(c, 'neutral', 'sonnet', 'initial', 0)
            self.assertEqual(api.call_count, 2)
            self.assertNotIn('DO NOT SAVE PRIVATE', (folder/'attempts.jsonl').read_text())
            attempts = read_attempts(folder/'attempts.jsonl')
            self.assertEqual(len(attempts), 2);self.assertEqual(attempts[0]['status'], 'transport_error')
            self.assertAlmostEqual(r.spent, attempts[0]['reserved_usd']+attempts[1]['cost_usd'])
            self.assertEqual(len(Run(folder).attempts), 2)

    def test_failed_reservation_prevents_retry_over_ceiling(self):
        m = fixture();c = m['base']['cases'][0]
        _, body = request_body(m, {}, c, 'neutral', 'sonnet', 'initial', 0)
        reserve = ((len(json.dumps(body).encode())+4096)*4+4096*10)/1e6
        m['new_spend_ceiling_usd'] = reserve*1.5
        with tempfile.TemporaryDirectory() as tmp:
            r = self.new_runner(Path(tmp), m)
            with patch.dict('os.environ', {'ANTHROPIC_API_KEY': 'unit-placeholder'}), \
                 patch('urllib.request.urlopen', side_effect=urllib.error.URLError('fixture')) as api, \
                 patch.object(r.stop, 'wait', return_value=False):
                r.invoke(c, 'neutral', 'sonnet', 'initial', 0)
            self.assertEqual(api.call_count, 1);self.assertTrue(r.stop.is_set())


if __name__ == '__main__':
    unittest.main()
