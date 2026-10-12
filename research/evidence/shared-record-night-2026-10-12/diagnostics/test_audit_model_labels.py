#!/usr/bin/env python3
"""Offline diagnostic tests; only temporary JSON artifacts are created."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

from audit_model_labels import Audit, Scanner, PRIVATE_MARKER, PACKET_MARKER, canonical, digest

BLOCK = 'b-0123456789abcdef01234567'
OPAQUE = 'candidate-' + 'a' * 32
OLD = BLOCK + '/early/agent-0/c0'


def scanner():
    manifest = dict(study_id='study-fixture', blocks=[dict(id=BLOCK)],
                    arms={key: {} for key in ('S', 'C_E', 'P_E', 'C_D', 'P_D', 'C_F', 'P_F')})
    docs = [dict(candidates=[dict(id=OLD), dict(id=OPAQUE)],
                 agents=[dict(id=BLOCK + '/S/agent-0', candidates=[])])]
    return Scanner(manifest, docs, [dict(label=BLOCK + '/early/agent-0')])


class ScannerTests(unittest.TestCase):
    def test_exact_candidate_is_longest_match_and_carries_prefix_cue(self):
        hits = scanner().scan(dict(candidate_id=OLD))
        self.assertEqual(len(hits), 1)
        self.assertEqual(hits[0]['kinds'], ['internal_candidate_id'])
        self.assertEqual(hits[0]['cues'], ['early'])

    def test_internal_agent_and_trial_ids(self):
        hits = scanner().scan([BLOCK + '/S/agent-0', BLOCK + '/P_D'])
        self.assertEqual([h['kinds'] for h in hits], [['internal_agent_id'], ['internal_trial_id']])
        self.assertEqual(hits[1]['cues'], ['P_D'])

    def test_allowed_policy_authors_and_opaque_candidate_references(self):
        self.assertEqual(scanner().scan(dict(policy='D', author='agent-0', candidate_id=OPAQUE,
                                             parent_id=OPAQUE, source_reference=dict(candidate_id=OPAQUE))), [])

    def test_natural_early_and_single_letter_prose_do_not_flag(self):
        self.assertEqual(scanner().scan('Produce a valid candidate early. I prefer delayed work. S R E D F.'), [])

    def test_explicit_arm_and_prefix_assignment_flags(self):
        hits = scanner().scan(dict(arm='S', prefix_kind='early', nested=dict(condition='delayed')))
        self.assertEqual(len(hits), 3)
        self.assertTrue(all(h['kinds'] == ['structured_assignment_label'] for h in hits))

    def test_standalone_multicharacter_arm_tokens_flag(self):
        hits = scanner().scan('Your assigned arm: P_F; paired with C_F.')
        self.assertEqual([h['literal'] for h in hits], ['P_F', 'C_F'])

    def test_embedded_json_private_review_and_packet(self):
        for marker, part in [(PRIVATE_MARKER, 'private_review_serialization'),
                             (PACKET_MARKER, 'record_review_packet')]:
            hits = scanner().scan('Review these.\n' + marker + json.dumps(dict(candidates=[dict(id=OLD)])))
            self.assertEqual(len(hits), 1)
            self.assertEqual(hits[0]['surface_part'], part)
            self.assertTrue(hits[0]['path'].endswith('#json/candidates/0/id'))

    def test_json_escaped_block_id_is_decoded(self):
        payload = '{"candidate_id": "\\u0062' + OLD[1:] + '"}'
        hits = scanner().scan(dict(output=payload))
        self.assertEqual(hits[0]['literal'], OLD)

    def test_internal_id_in_dictionary_key_flags(self):
        hits = scanner().scan({OLD: 'value'})
        self.assertEqual(len(hits), 1)
        self.assertTrue(hits[0]['path'].endswith('#key'))

    def test_similar_longer_token_is_not_exact_match(self):
        self.assertEqual(scanner().scan('prefix' + BLOCK + 'suffix'), [])


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.manifest = dict(study_id='study-fixture', blocks=[dict(id=BLOCK)],
                             arms={'S': dict(kind='S')}, models={'luna': dict(model='scripted-luna')})
        self.manifest_hash = self.write('manifest.json', self.manifest)
        self.write('freeze.json', dict(manifest_sha256=self.manifest_hash))
        self.label = BLOCK + '/S/agent-0'
        self.payload = dict(instructions='Produce a valid candidate early.',
                            input=[dict(role='user', content='Solve the problem.')],
                            tools=[], model='scripted-luna')
        self.row = dict(id='request-1', label=self.label, status='complete',
                        payload=self.payload, response=dict(output=[], status='completed'))
        self.agent = dict(name='agent-0', id=self.label, candidates=[dict(id=OPAQUE)],
                          history=copy.deepcopy(self.payload['input']), trace=[],
                          request_ids=['request-1'], requests=[copy.deepcopy(self.row)])

    def tearDown(self):
        self.temp.cleanup()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        raw = canonical(value)
        path.write_bytes(raw)
        return digest(raw)

    def finish(self, sealed=True):
        ledger_hash = self.write('ledger.json', [self.row])
        trial_name = f'trials/{BLOCK}/S.json'
        trial_hash = self.write(trial_name, dict(final=sealed, manifest_sha256=self.manifest_hash,
                                                agents=[self.agent], candidates=self.agent['candidates']))
        if sealed:
            self.write('seal.json', dict(manifest_sha256=self.manifest_hash,
                                        files={'ledger.json': ledger_hash, trial_name: trial_hash,
                                               'freeze.json': digest((self.root / 'freeze.json').read_bytes())}))

    def test_internal_archive_labels_are_not_model_visible(self):
        self.finish()
        report = Audit(self.root).run()
        self.assertEqual(report['status'], 'pass')
        self.assertEqual(report['summary']['findings'], 0)
        self.assertEqual(report['coverage']['requests_response_confirmed'], 1)

    def test_submitted_tool_and_private_review_leaks_are_distinguished(self):
        result = dict(candidate_id=OLD)
        tool_item = dict(type='function_call_output', call_id='call-1', output=json.dumps(result))
        private = dict(role='user', content=PRIVATE_MARKER + json.dumps([dict(id=OLD)]))
        self.payload['input'].extend([tool_item, private])
        self.agent['candidates'] = [dict(id=OLD)]
        self.agent['history'] = copy.deepcopy(self.payload['input'])
        self.agent['trace'] = [dict(event='tool_results', results=[['call-1', result]])]
        self.agent['requests'] = [copy.deepcopy(self.row)]
        self.finish()
        report = Audit(self.root).run()
        self.assertEqual(report['status'], 'leaks_found')
        self.assertEqual(report['summary']['by_surface'], dict(request_payload=2, archived_history=2, tool_result=1))
        self.assertEqual(report['coverage']['private_review_history_items'], 1)
        self.assertEqual(report['coverage']['request_findings_response_confirmed'], 2)
        history = [s for s in report['archive_surfaces_with_findings'] if s['surface'] == 'archived_history']
        self.assertTrue(all(s['matching_payload_items'] for s in history))

    def test_unsubmitted_final_tool_result_does_not_claim_dispatch(self):
        self.agent['history'].append(dict(type='function_call_output', call_id='call-final',
                                          output=json.dumps(dict(candidate_id=OLD))))
        self.agent['candidates'] = [dict(id=OLD)]
        self.finish()
        report = Audit(self.root).run()
        self.assertEqual(report['summary']['by_surface'], dict(archived_history=1))
        self.assertEqual(report['archive_surfaces_with_findings'][0]['matching_payload_items'], [])
        self.assertEqual(report['coverage']['request_findings_response_confirmed'], 0)

    def test_unsealed_requires_opt_in_and_never_passes(self):
        self.finish(sealed=False)
        with self.assertRaisesRegex(ValueError, 'allow-partial'):
            Audit(self.root).run()
        self.assertEqual(Audit(self.root, allow_partial=True).run()['status'], 'no_leaks_found_in_partial_snapshot')

    def test_pending_reservation_is_not_confirmed_submission(self):
        self.row.update(status='pending', payload=dict(input=[dict(content=OLD)]))
        self.row.pop('response')
        self.agent['candidates'] = [dict(id=OLD)]
        self.agent['requests'] = [copy.deepcopy(self.row)]
        self.finish(sealed=False)
        report = Audit(self.root, allow_partial=True).run()
        self.assertEqual(report['coverage']['requests_reservation_or_archive_only'], 1)
        self.assertEqual(report['coverage']['request_findings_reservation_or_archive_only'], 1)

    def test_failed_dispatch_is_not_confirmed_response(self):
        self.row.update(status='failed')
        self.row.pop('response')
        self.agent['requests'] = [copy.deepcopy(self.row)]
        self.finish()
        self.assertEqual(Audit(self.root).run()['coverage']['requests_dispatch_attempt_failed_or_unknown'], 1)

    def test_conflicting_copied_payload_invalidates_clean_audit(self):
        self.agent['requests'][0]['payload']['input'][0]['content'] = 'Other problem.'
        self.finish()
        report = Audit(self.root).run()
        self.assertEqual(report['status'], 'invalid_evidence')
        self.assertIn('different payload copies for request: request-1', report['errors'])

    def test_changed_sealed_bytes_invalidate_clean_audit(self):
        self.finish()
        self.write('ledger.json', [dict(self.row, extra_metadata='unsealed')])
        report = Audit(self.root).run()
        self.assertEqual(report['status'], 'invalid_evidence')
        self.assertIn('read evidence not bound to seal: ledger.json', report['errors'])


if __name__ == '__main__':
    unittest.main()
