"""False-pass guards for late findings, provenance, recovery and application."""
import copy
import difflib
import json
import platform
import shutil
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


class WorkspaceHarnessTests(unittest.TestCase):
    @unittest.skipUnless(platform.system() == 'Darwin' and Path('/usr/bin/sandbox-exec').is_file()
        and (Path(__file__).resolve().parents[2] / 'target/debug/locust').is_file(),
        'compiled production binary and macOS guard required')
    def test_actual_seed_review_integration_update_preserves_dirty_ordinary_checkout(self):
        from check_shared_context_models import (checkout_role, enroll, raw_call,
                                                review_tree, seed_workspace)
        from client_qualification.production import ProductionDaemon
        binary = Path(__file__).resolve().parents[2] / 'target/debug/locust'
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            # Pin exact executable bytes: concurrent workspace builds must not
            # change the candidate between start and restart qualification.
            pinned = output / 'locust'
            shutil.copy2(binary, pinned)
            binary = pinned
            profiles = [Profile(output, name) for name in ('setup', 'researcher', 'builder', 'integrator')]
            for profile in profiles:
                self.addCleanup(profile.close)
            setup, researcher_profile, builder_profile, integration_profile = profiles
            fixture = case.seed(researcher_profile.workspace, builder_profile.workspace)
            with ProductionDaemon(setup, binary, 20) as daemon:
                formation = raw_call(daemon, ['formation', 'example', 'peer-review'])
                daemon.goal = raw_call(daemon, ['goal', 'create', '--title', 'Current workspace harness test',
                    '--formation-json', json.dumps(formation)])['goal_created']['goal']
                researcher = enroll(daemon, researcher_profile, 'researcher')
                builder = enroll(daemon, builder_profile, 'builder')
                integrator = enroll(daemon, integration_profile, 'integrator', permissions=('contribute',))
                seed = seed_workspace(daemon, builder_profile.workspace, fixture['builder_files'],
                    completion=formation['decisions']['completion'], reviewer=researcher)
                worker = checkout_role(builder, daemon, seed['revision'])
                destination = checkout_role(integrator, daemon, seed['revision'])
                dirty_readme = (integration_profile.workspace / 'README.md').read_text() + '\nLocal notes.\n'
                (integration_profile.workspace / 'README.md').write_text(dirty_readme)
                (integration_profile.workspace / 'unrelated.txt').write_text('preserve outside work\n')
                (builder_profile.workspace / 'safe_member.py').write_text(REVISED)
                captured = raw_call(daemon, ['workspace', 'propose', '--goal', daemon.goal,
                    '--checkout', worker['id'], '--only', '--path', 'safe_member.py', '--publish'], role=builder)
                proposal = captured['operation']['state']['recorded']['event']
                finding = raw_call(daemon, ['contribution', 'publish', '--goal', daemon.goal,
                    'Harness finding'], role=researcher)['recorded']['event']
                task_report = raw_call(daemon, ['contribution', 'publish', '--goal', daemon.goal,
                    '--sources', json.dumps([finding, proposal]), 'Task report cites exact candidate'], role=builder)
                inspected = raw_call(daemon, ['contribution', 'inspect', '--goal', daemon.goal,
                    '--contribution', task_report['recorded']['event']], role=researcher)['contribution_inspected']
                sources = inspected['contribution']['body']['contribution_published']['sources']
                self.assertEqual(set(sources), {finding, proposal})
                reviewed, review_artifact = review_tree(daemon, proposal, researcher, output, 'verified-review')
                self.assertEqual(reviewed['proposal']['parent'], seed['revision'])
                self.assertEqual(Path(review_artifact).read_text(), REVISED)
                with self.assertRaises(RuntimeError):
                    raw_call(daemon, ['workspace', 'integrate', '--goal', daemon.goal,
                        '--proposal', proposal, '--expected-head', seed['revision']])
                raw_call(daemon, ['review', 'record', '--goal', daemon.goal, '--subject', proposal,
                    '--verdict', 'approve', 'Checked exact candidate bytes'], role=researcher)
                raw_call(daemon, ['workspace', 'integrate', '--goal', daemon.goal,
                    '--proposal', proposal, '--expected-head', seed['revision']])
                accepted = raw_call(daemon, ['workspace', 'head', '--goal', daemon.goal])['head']
                raw_call(daemon, ['workspace', 'update', '--goal', daemon.goal,
                    '--checkout', destination['id'], '--revision', accepted['revision']], role=integrator)
                self.assertEqual((integration_profile.workspace / 'safe_member.py').read_text(), REVISED)
                self.assertEqual((integration_profile.workspace / 'README.md').read_text(), dirty_readme)
                self.assertEqual((integration_profile.workspace / 'unrelated.txt').read_text(), 'preserve outside work\n')
                self.assertFalse((integration_profile.workspace / '.git').exists())
                daemon.restart()
                observed = raw_call(daemon, ['workspace', 'status', '--goal', daemon.goal,
                    '--checkout', destination['id']], role=integrator)
                self.assertEqual(observed['checkout']['base_revision'], accepted['revision'])
                self.assertIn('README.md', observed['dirty_paths'])


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
                      'body': {'contribution_published': {'sources': ['finding', 'final-proposal'], 'attempt': 'attempt',
                               'artifacts': []}}}
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
            'outcome': 'cmd: ./locust-scoped workspace review --proposal final-proposal, exit_code: 0, success: true, stdout_truncated: false, unified_diff: safe_member.py'}]}]}))
        self.runs[-1]['transcript'] = str(transcript)
        diff = ''.join(difflib.unified_diff(pilot.STARTER.splitlines(True), REVISED.splitlines(True),
                                          fromfile='a/safe_member.py', tofile='b/safe_member.py'))
        self.report = {'runs': self.runs, 'goal': 'goal', 'base': 'base', 'base_revision': 'base-revision',
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
            'independent_tree_review': {'proposal': {'proposal': 'final-proposal', 'parent': 'base-revision', 'result_manifest': 'head'}, 'review_mode': 'base_diff',
                'changes': [{'path': 'safe_member.py', 'unified_diff': diff, 'after': {'bytes': len(REVISED.encode()), 'executable': False}}]},
            'initial_artifact': str(self.root / 'initial/safe_member.py'),
            'revised_artifact': str(self.root / 'revised/safe_member.py'),
            'applied_artifact': str(self.root / 'applied/safe_member.py'),
            'application': {'actor': 'person-harness', 'subject': 'final-proposal', 'result_manifest': 'head', 'integration': {'recorded': {'event': 'selection'}}, 'revision': 'selection', 'result': {'applied': True},
                'distinct_workspace': True, 'dirty_work_preserved': True, 'ordinary_directory': True},
            'final_context': [{'event': {'view': {'kind': 'review_recorded', 'author': 'researcher', 'standing': 'effective'},
                'body': {'review_recorded': {'subject': 'final-proposal', 'verdict': 'approve'}}}}]}
        review_outcome = {'ok': True, 'result': self.report['independent_tree_review']}
        transcript.write_text(json.dumps({'turns': [{'effects': [{'tool': 'exec.run', 'status': 'completed',
            'outcome': 'cmd: ./locust-scoped workspace review --proposal final-proposal, exit_code: 0, stdout: ' +
                json.dumps(review_outcome) + ', stdout_truncated: false, success: true'}]}]}))
        initial_diff = ''.join(difflib.unified_diff(pilot.STARTER.splitlines(True), PORTABLE.splitlines(True),
                                                   fromfile='a/safe_member.py', tofile='b/safe_member.py'))
        self.report['initial_contribution'] = {'event': {'view': {'event': 'initial-contribution',
            'author': 'builder', 'kind': 'contribution_published', 'standing': 'effective'},
            'task': 'task:task-id', 'body': {'contribution_published': {'attempt': 'initial-attempt',
                'sources': ['initial-proposal'], 'artifacts': []}}}}
        self.report['initial_tree_review'] = {'proposal': {'proposal': 'initial-proposal',
            'parent': 'base-revision', 'result_manifest': 'initial-head'}, 'review_mode': 'base_diff',
            'changes': [{'path': 'safe_member.py', 'unified_diff': initial_diff,
                        'after': {'bytes': len(PORTABLE.encode()), 'executable': False}}]}
        for phase, proposal, manifest, code in (
            ('initial', 'initial-proposal', 'initial-head', PORTABLE),
            ('revision', 'final-proposal', 'head', REVISED)):
            field = 'initial_proposal' if phase == 'initial' else 'final_proposal'
            self.report[field] = {'event': {'view': {'event': proposal, 'author': 'builder',
                'kind': 'workspace_proposed', 'standing': 'effective'},
                'body': {'workspace_proposed': {'context': {'scope': 'workspace'},
                    'parent': 'base-revision', 'result_manifest': manifest, 'sources': []}}}}
            copy_path = self.root / (phase + '-review') / 'safe_member.py'
            copy_path.parent.mkdir()
            copy_path.write_text(code)
            self.report['initial_reviewed_artifact' if phase == 'initial' else 'reviewed_artifact'] = str(copy_path)
            next(run for run in self.runs if run['phase'] == phase)['native_calls'] = [
                {'success': True, 'arguments': {'command': './locust-scoped workspace publish --operation op'},
                 'output': json.dumps({'ok': True, 'result': {'recorded': {'event': proposal}}})}]

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

    def test_echoed_target_with_another_tree_review_does_not_qualify(self):
        path = Path(self.runs[-1]['transcript'])
        value = json.loads(path.read_text())
        effect = value['turns'][0]['effects'][0]
        effect['outcome'] = effect['outcome'].replace('cmd: ./locust-scoped', 'cmd: echo final-proposal; ./locust-scoped').replace(
            '"proposal": "final-proposal"', '"proposal": "different"')
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

    def test_review_destination_does_not_change_exact_candidate_identity(self):
        path = Path(self.runs[-1]['transcript'])
        value = json.loads(path.read_text())
        review = copy.deepcopy(self.report['independent_tree_review'])
        review['destination'] = '/another/explicit/review-copy'
        review['proposal']['approved'] = True
        review['proposal']['evidence'] = ['peer-approval']
        value['turns'][0]['effects'][0]['outcome'] = (
            'cmd: ./locust-scoped workspace review --proposal final-proposal, exit_code: 0, stdout: ' +
            json.dumps({'ok': True, 'result': review}) + ', stdout_truncated: false, success: true')
        path.write_text(json.dumps(value))
        self.assertTrue(self.evaluate()['real_peer_reviewed_and_approved_exact_submission'])
        review['changes'][0]['after']['bytes'] += 1
        value['turns'][0]['effects'][0]['outcome'] = (
            'cmd: ./locust-scoped workspace review --proposal final-proposal, exit_code: 0, stdout: ' +
            json.dumps({'ok': True, 'result': review}) + ', stdout_truncated: false, success: true')
        path.write_text(json.dumps(value))
        self.assertFalse(self.evaluate()['real_peer_reviewed_and_approved_exact_submission'])

    def test_task_report_review_does_not_approve_workspace_proposal(self):
        self.report['final_context'][0]['event']['body']['review_recorded']['subject'] = 'final'
        self.assertFalse(self.evaluate()['real_peer_reviewed_and_approved_exact_submission'])

    def test_replaced_review_copy_and_wrong_manifest_fail(self):
        Path(self.report['reviewed_artifact']).write_text(PORTABLE)
        self.assertFalse(self.evaluate()['submitted_tree_matches_revised_artifact'])
        Path(self.report['reviewed_artifact']).write_text(REVISED)
        self.report['final_proposal']['event']['body']['workspace_proposed']['result_manifest'] = 'another-tree'
        self.assertFalse(self.evaluate()['submitted_tree_matches_revised_artifact'])

    def test_task_report_must_cite_exact_proposal_and_native_receipt(self):
        self.final['body']['contribution_published']['sources'] = ['finding']
        self.assertFalse(self.evaluate()['direct_source_provenance_declared'])
        self.runs[3]['native_calls'][0]['output'] = 'final-proposal'
        self.assertFalse(self.evaluate()['submitted_tree_matches_revised_artifact'])

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
