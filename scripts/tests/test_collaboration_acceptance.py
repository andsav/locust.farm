"""False-pass guards for late findings, provenance, recovery and application."""
import copy
import difflib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from client_qualification import acceptance_case as case
from client_qualification import collaboration_case as pilot
from client_qualification.acceptance_evidence import evaluate, usage
from client_qualification.runtime import Profile
from check_shared_context_models import retain_codex_sessions

PORTABLE = '''import ntpath
from pathlib import PurePosixPath

def normalize_member_path(name):
    if not isinstance(name, str) or not name or "\\x00" in name:
        return None
    normalized = name.replace("\\\\", "/")
    path = PurePosixPath(normalized)
    if ntpath.splitdrive(normalized)[0] or path.is_absolute() or ".." in path.parts:
        return None
    return path.as_posix()
'''
REVISED = PORTABLE.replace('    return path.as_posix()', '''    if path.parts and path.parts[0].casefold() == ".archive-index":
        return None
    return path.as_posix()''')


class AcceptanceCaseTests(unittest.TestCase):
    def test_private_deployment_convention_requires_revision_of_correct_portable_helper(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            fixture = case.seed(root / 'researcher', root / 'builder')
            public = fixture['prompts']['builder'] + ''.join((root / 'builder' / name).read_text()
                                                            for name in fixture['builder_files'])
            self.assertNotIn('.archive-index', public)
            self.assertIn('Python 3.12', public)
            self.assertIn('.archive-index', Path(fixture['researcher_only']['private_fixture']).read_text())
            artifact = root / 'builder' / 'safe_member.py'
            artifact.write_text(PORTABLE)
            self.assertTrue(pilot.verify(artifact.parent)['passed'])
            self.assertFalse(case.verify(artifact.parent)['passed'])
            artifact.write_text(REVISED)
            self.assertTrue(case.verify(artifact.parent)['passed'])

    def test_overbroad_reserved_name_rejection_fails_oracle(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            (root / 'safe_member.py').write_text(REVISED.replace(
                'path.parts[0].casefold() == ".archive-index"',
                'any(part.casefold().startswith(".archive-index") for part in path.parts)'))
            self.assertFalse(case.verify(root)['passed'])


class NativeEvidenceTests(unittest.TestCase):
    def test_profile_uses_short_owned_system_temp_root(self):
        with tempfile.TemporaryDirectory() as output:
            profile = Profile(output, 'codex')
            try:
                self.assertEqual(profile.root.parent, Path('/tmp').resolve())
                self.assertEqual(profile.root.stat().st_mode & 0o777, 0o700)
                self.assertEqual(profile.home.stat().st_mode & 0o777, 0o700)
            finally:
                profile.close()

    def test_native_session_retains_tool_failure_and_model_without_provider_key(self):
        with tempfile.TemporaryDirectory() as output:
            profile = Profile(output, 'codex')
            self.addCleanup(profile.close)
            source = profile.home / '.codex/sessions/session.jsonl'
            source.parent.mkdir(parents=True)
            rows = [{'type': 'turn_context', 'payload': {'model': 'gpt-6-luna'}},
                    {'type': 'response_item', 'payload': {'type': 'function_call_output',
                                                        'output': 'failed: sensitive-provider-key'}}]
            source.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            outside = Path(output) / 'outside.jsonl'
            outside.write_text('must not retain unrelated file')
            (source.parent / 'substituted.jsonl').symlink_to(outside)
            result = retain_codex_sessions(profile, 'first', 'sensitive-provider-key')
            self.assertEqual(result['native_selected_models'], ['gpt-6-luna'])
            self.assertEqual(len(result['native_sessions']), 1)
            retained = Path(result['native_sessions'][0]['path']).read_text()
            self.assertIn('failed: <redacted-provider-key>', retained)
            self.assertNotIn('sensitive-provider-key', retained)
            self.assertNotIn('must not retain', retained)
            second = retain_codex_sessions(profile, 'second', 'sensitive-provider-key')
            self.assertNotEqual(result['native_sessions'][0]['path'], second['native_sessions'][0]['path'])


class AcceptanceEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for phase, code in (('initial', PORTABLE), ('revised', REVISED), ('applied', REVISED)):
            folder = self.root / phase
            folder.mkdir()
            (folder / 'safe_member.py').write_text(code)
        self.finding = {'event': {'view': {'event': 'finding', 'kind': 'contribution_published',
            'author': 'researcher', 'standing': 'effective'}, 'task': None,
            'body': {'contribution_published': {'sources': [], 'attempt': None}},
            'text': 'Windows backslash on Linux: normalize ..; reject root .archive-index case-insensitively'},
            'text_complete': True}
        self.final = {'view': {'event': 'final', 'author': 'builder', 'kind': 'contribution_published',
                              'standing': 'effective'}, 'task': 'task:task-id', 'text': 'Final helper',
                      'body': {'contribution_published': {'sources': ['finding'], 'attempt': 'attempt',
                               'base': 'base', 'patch': 'patch', 'artifacts': ['head']}}}
        self.events = {}
        self.runs = []
        for index, phase in enumerate(('permission', 'initial', 'research', 'revision', 'review')):
            stdout = self.root / (phase + '.stdout')
            stdout.write_text(json.dumps({'type': 'thread.started', 'thread_id': 'same-thread'}) + '\n' +
                              json.dumps({'type': 'turn.completed', 'usage': {'input_tokens': 10,
                                          'cached_input_tokens': 4, 'output_tokens': 2}}) + '\n')
            self.events[phase] = []
            self.runs.append({'phase': phase, 'client': 'codex' if phase in ('permission', 'initial', 'revision') else 'merak',
                'stdout': str(stdout), 'mcp_events': str(self.root / (phase + '.mcp.jsonl')),
                'native_calls': [], 'started_sequence': index * 2 + 1, 'completed_sequence': index * 2 + 2,
                'exit_code': 0, 'natural_cleanup': True, 'cleanup_verified': True,
                'accounting': {'output_tokens': 2}})
        self.call('permission', 'locust_attempt_start', {'goal': 'goal', 'task': 'task:task-id'},
                  {'ok': False, 'error': {'code': 'authorization_required'}}, error=True)
        self.call('permission', 'locust_permission_inspect', {'goal': 'goal', 'agent': 'builder'},
                  {'ok': True, 'result': {'permissions': {'goal': 'goal', 'agent': 'builder', 'grants': {'execute': False}}}})
        self.call('initial', 'locust_attempt_start', {'goal': 'goal', 'task': 'task:task-id'},
                  {'ok': True, 'result': {'claimed': {'instance': 'instance-b', 'attempt': 'attempt'}}})
        self.call('revision', 'locust_context_read', {'goal': 'goal', 'view': 'compact'},
                  {'ok': True, 'result': {'context': {'items': [copy.deepcopy(self.finding)], 'receipt': 'short-reference'}}})
        self.call('revision', 'locust_context_acknowledge', {'goal': 'goal', 'receipt': 'short-reference'},
                  {'ok': True, 'result': {'context_acknowledged': {'goal': 'goal', 'principal': 'builder',
                    'session': 'instance-b', 'entries': [{'event': 'finding', 'version': 'exact-content'}]}}})
        transcript = self.root / 'review.transcript.json'
        transcript.write_text(json.dumps({'truncated': False, 'turns': [{'effects': [{'tool': 'exec.run',
            'status': 'completed', 'tool_truncated': False,
            'outcome': 'cmd: ./locust-scoped patch review --patch patch, exit_code: 0, success: true, stdout_truncated: false, unified_diff: safe_member.py'}]}]}))
        self.runs[-1]['transcript'] = str(transcript)
        diff = ''.join(difflib.unified_diff(pilot.STARTER.splitlines(True), REVISED.splitlines(True),
                                          fromfile='a/safe_member.py', tofile='b/safe_member.py'))
        self.report = {'runs': self.runs, 'goal': 'goal', 'base': 'base',
            'principals': {'builder': {'principal': 'builder', 'instance': 'instance-b'},
                           'researcher': {'principal': 'researcher', 'instance': 'instance-r'}},
            'permission_grant': {'actor': 'person-harness', 'principal': 'builder', 'instance': 'instance-b',
                'result': {'permissions': {'goal': 'goal', 'agent': 'builder', 'grants': {'execute': True}}}},
            'permission_artifact_unchanged': True,
            'initial_context_has_research_finding': False,
            'research_findings': [copy.deepcopy(self.finding)], 'final_contribution': {'event': self.final},
            'source_inspection': {'contribution_inspected': {'contribution': copy.deepcopy(self.final),
                'declared_sources': [{'event': 'finding', 'detail': copy.deepcopy(self.finding['event'])}],
                'attempt': {'event': 'attempt'}}},
            'independent_patch_review': {'contribution_id': 'patch', 'base': 'base', 'head': 'head',
                'changes': [{'path': 'safe_member.py', 'unified_diff': diff, 'after': {'bytes': len(REVISED.encode())}}]},
            'initial_artifact': str(self.root / 'initial/safe_member.py'),
            'revised_artifact': str(self.root / 'revised/safe_member.py'),
            'applied_artifact': str(self.root / 'applied/safe_member.py'),
            'application': {'actor': 'person-harness', 'subject': 'final', 'patch': 'patch', 'result': {'applied': True},
                'distinct_workspace': True, 'dirty_work_preserved': True, 'git_head_preserved': True},
            'final_context': [{'event': {'view': {'kind': 'review_recorded', 'author': 'researcher', 'standing': 'effective'},
                'body': {'review_recorded': {'subject': 'final', 'verdict': 'approve'}}}}]}
        review_outcome = {'ok': True, 'result': self.report['independent_patch_review']}
        transcript.write_text(json.dumps({'turns': [{'effects': [{'tool': 'exec.run', 'status': 'completed',
            'outcome': 'cmd: ./locust-scoped patch review --patch patch, exit_code: 0, stdout: ' +
                json.dumps(review_outcome) + ', stdout_truncated: false, success: true'}]}]}))
        initial_diff = ''.join(difflib.unified_diff(pilot.STARTER.splitlines(True), PORTABLE.splitlines(True),
                                                   fromfile='a/safe_member.py', tofile='b/safe_member.py'))
        self.report['initial_contribution'] = {'event': {'view': {'event': 'initial-contribution',
            'author': 'builder', 'kind': 'contribution_published', 'standing': 'effective'},
            'task': 'task:task-id', 'body': {'contribution_published': {'attempt': 'initial-attempt',
                'base': 'base', 'patch': 'initial-patch', 'artifacts': ['initial-head']}}}}
        self.report['initial_patch_review'] = {'base': 'base', 'contribution_id': 'initial-patch',
            'head': 'initial-head', 'changes': [{'path': 'safe_member.py', 'unified_diff': initial_diff,
                                              'after': {'bytes': len(PORTABLE.encode())}}]}

    def call(self, phase, tool, args, envelope, error=False):
        ident = len(self.events[phase]) + 1
        self.events[phase] += [{'direction': 'client_request', 'method': 'tools/call', 'id': ident,
                               'tool': tool, 'arguments': args},
                              {'direction': 'bridge_response', 'id': ident, 'tool': tool,
                               'isError': error, 'result': envelope}]

    def evaluate(self):
        for phase, events in self.events.items():
            (self.root / (phase + '.mcp.jsonl')).write_text(''.join(json.dumps(event) + '\n' for event in events))
        return evaluate(self.report)

    def test_complete_positive_fixture(self):
        self.assertTrue(all(self.evaluate().values()))

    def test_task_text_citation_does_not_replace_signed_direct_source(self):
        self.final['body']['contribution_published']['sources'] = []
        self.final['text'] = 'Task used finding'
        self.assertFalse(self.evaluate()['direct_source_provenance_declared'])

    def test_unavailable_declared_source_does_not_pass_inspection(self):
        self.report['source_inspection']['contribution_inspected']['declared_sources'][0]['detail'] = None
        self.assertFalse(self.evaluate()['direct_source_provenance_declared'])

    def test_substituted_receipt_does_not_acknowledge_finding(self):
        self.events['revision'][2]['arguments']['receipt'] = 'different-reference'
        self.assertFalse(self.evaluate()['attributed_finding_read_and_acknowledged'])

    def test_permission_recovery_requires_same_native_thread_and_locust_instance(self):
        self.events['initial'][1]['result']['result']['claimed']['instance'] = 'other-instance'
        self.assertFalse(self.evaluate()['missing_permission_stop_then_same_agent_recovery'])
        self.events['initial'][1]['result']['result']['claimed']['instance'] = 'instance-b'
        Path(self.runs[1]['stdout']).write_text(json.dumps({'type': 'thread.started', 'thread_id': 'new-thread'}))
        self.assertFalse(self.evaluate()['missing_permission_stop_then_same_agent_recovery'])

    def test_owner_credential_substitution_cannot_qualify(self):
        self.runs[1]['native_calls'] = [{'arguments': {'command': './locust-scoped --owner attempt start'},
                                        'success': True}]
        self.assertFalse(self.evaluate()['missing_permission_stop_then_same_agent_recovery'])

    def test_missing_authority_for_another_agent_does_not_qualify(self):
        self.events['permission'][3]['result']['result']['permissions']['agent'] = 'another-agent'
        self.assertFalse(self.evaluate()['missing_permission_stop_then_same_agent_recovery'])

    def test_a_person_grant_to_another_agent_does_not_qualify_recovery(self):
        self.report['permission_grant']['result']['permissions']['agent'] = 'another-agent'
        self.assertFalse(self.evaluate()['missing_permission_stop_then_same_agent_recovery'])

    def test_anticipated_private_requirement_does_not_prove_revision(self):
        Path(self.report['initial_artifact']).write_text(REVISED)
        self.assertFalse(self.evaluate()['artifact_changed_and_private_behavior_recovered'])

    def test_unchanged_or_broken_initial_code_cannot_prove_builder_work(self):
        for invalid in (pilot.STARTER, 'invalid Python !!!', PORTABLE.replace('return path.as_posix()', 'return None')):
            with self.subTest(invalid=invalid):
                Path(self.report['initial_artifact']).write_text(invalid)
                self.assertFalse(self.evaluate()['builder_work_preceded_private_finding'])

    def test_unsigned_initial_artifact_does_not_qualify_as_published_work(self):
        self.report['initial_contribution'] = {}
        self.assertFalse(self.evaluate()['builder_work_preceded_private_finding'])

    def test_generated_review_prose_is_not_completed_tool_evidence(self):
        path = Path(self.runs[-1]['transcript'])
        value = json.loads(path.read_text())
        value['turns'][0]['effects'][0]['tool'] = 'provider.complete'
        path.write_text(json.dumps(value))
        self.assertFalse(self.evaluate()['real_peer_reviewed_and_approved_exact_submission'])

    def test_partial_transcript_requires_complete_exact_review_effect(self):
        path = Path(self.runs[-1]['transcript'])
        value = json.loads(path.read_text())
        value['truncated'] = value['output_truncated'] = True
        path.write_text(json.dumps(value))
        self.assertTrue(self.evaluate()['real_peer_reviewed_and_approved_exact_submission'])
        value['turns'][0]['effects'][0]['tool_truncated'] = True
        path.write_text(json.dumps(value))
        self.assertFalse(self.evaluate()['real_peer_reviewed_and_approved_exact_submission'])

    def test_echoed_target_with_another_patch_review_does_not_qualify(self):
        path = Path(self.runs[-1]['transcript'])
        value = json.loads(path.read_text())
        effect = value['turns'][0]['effects'][0]
        effect['outcome'] = effect['outcome'].replace('cmd: ./locust-scoped', 'cmd: echo patch; ./locust-scoped').replace(
            '"contribution_id": "patch"', '"contribution_id": "different"')
        path.write_text(json.dumps(value))
        self.assertFalse(self.evaluate()['real_peer_reviewed_and_approved_exact_submission'])

    def test_missing_native_thread_preserves_raw_usage_as_unavailable(self):
        Path(self.runs[0]['stdout']).write_text(json.dumps({'type': 'turn.completed', 'usage': {
            'input_tokens': 10, 'cached_input_tokens': 4, 'output_tokens': 2}}))
        result = usage(self.runs[0], 'gpt-6-luna')
        self.assertEqual(result['raw_client_usage']['input_tokens'], 10)
        self.assertIsNone(result['input_tokens'])
        self.assertIsNone(result['native_thread'])
        self.assertIn('input_tokens', result['phase_usage_unavailable'])

    def test_approval_of_another_contribution_does_not_qualify(self):
        self.report['final_context'][0]['event']['body']['review_recorded']['subject'] = 'other'
        self.assertFalse(self.evaluate()['real_peer_reviewed_and_approved_exact_submission'])

    def test_reported_apply_is_insufficient_when_actual_file_differs(self):
        Path(self.report['applied_artifact']).write_text(PORTABLE)
        self.assertFalse(self.evaluate()['explicit_application_independently_verified'])

    def test_dirty_work_loss_fails_application(self):
        self.report['application']['dirty_work_preserved'] = False
        self.assertFalse(self.evaluate()['explicit_application_independently_verified'])

    def test_usage_keeps_cached_uncached_and_billing_separate(self):
        result = usage(self.runs[0], 'gpt-6-luna')
        self.assertEqual((result['input_tokens'], result['cached_input_tokens'], result['uncached_input_tokens']), (10, 4, 6))
        self.assertIsNone(result['reported_cost_usd'])
        self.assertIsNone(result['billed_cost_usd'])
        self.assertEqual(result['observed_models'], [])

    def test_resumed_cumulative_usage_is_differenced_by_native_thread(self):
        before = usage(self.runs[0], 'gpt-6-luna')
        self.runs[1]['resume_thread'] = 'same-thread'
        Path(self.runs[1]['stdout']).write_text(json.dumps({'type': 'thread.started', 'thread_id': 'same-thread'}) + '\n' +
            json.dumps({'type': 'turn.completed', 'usage': {'input_tokens': 25, 'cached_input_tokens': 10,
                                                           'output_tokens': 5}}))
        after = usage(self.runs[1], 'gpt-6-luna', previous=before)
        self.assertEqual((after['input_tokens'], after['cached_input_tokens'], after['uncached_input_tokens'], after['output_tokens']),
                         (15, 6, 9, 3))
        self.assertEqual(after['raw_client_usage']['input_tokens'], 25)

    def test_decreased_or_missing_prior_counters_are_unavailable(self):
        before = usage(self.runs[0], 'gpt-6-luna')
        before['raw_client_usage']['input_tokens'] = 20
        del before['raw_client_usage']['cached_input_tokens']
        self.runs[1]['resume_thread'] = 'same-thread'
        after = usage(self.runs[1], 'gpt-6-luna', previous=before)
        self.assertIsNone(after['input_tokens'])
        self.assertIsNone(after['cached_input_tokens'])
        self.assertIsNone(after['uncached_input_tokens'])
        self.assertIn('input_tokens', after['phase_usage_unavailable'])
        missing = usage(self.runs[1], 'gpt-6-luna')
        self.assertIsNone(missing['input_tokens'])


if __name__ == '__main__':
    unittest.main()
