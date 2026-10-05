"""False-pass contracts for two distinct real-client workspace qualification."""
import copy
import json
from pathlib import Path
import platform
import shutil
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from check_shared_workspace_models import (BASE_CODE, DIRTY_NOTES, TEST_CODE, UNTRACKED,
    author, coordinator_files, exact_review_read, execute, native_event, native_tests, prepare)
from check_shared_context_models import enroll
from client_qualification.production import ProductionDaemon
from client_qualification.runtime import Profile, private_write


def call_run(operation, result, success=True):
    return {'native_calls': [{'success': success, 'arguments': {'command': './locust-scoped ' + operation},
                             'output': json.dumps({'ok': True, 'result': result})}]}


class NativeEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.review = {'proposal': {'proposal': 'proposal-a', 'parent': 'revision-seed',
            'result_manifest': 'manifest-a', 'approved': False, 'evidence': []},
            'review_mode': 'base_diff', 'changes': [{'path': 'calculator.py', 'unified_diff': '-a-b\n+a+b'}]}

    def test_exact_diff_and_identity_required(self):
        run = call_run('workspace review', self.review)
        self.assertTrue(exact_review_read(run, self.review))
        for field, value in (('proposal', 'proposal-b'), ('result_manifest', 'manifest-b'), ('parent', 'revision-b')):
            changed = copy.deepcopy(self.review)
            changed['proposal'][field] = value
            self.assertFalse(exact_review_read(run, changed))
        changed = copy.deepcopy(self.review)
        changed['changes'][0]['unified_diff'] = 'another diff'
        self.assertFalse(exact_review_read(run, changed))

    def test_later_approval_and_different_materialization_destination_are_allowed(self):
        later = copy.deepcopy(self.review)
        later['proposal'].update(approved=True, evidence=['signed-review'])
        later['destination'] = '/another/materialization'
        self.assertTrue(exact_review_read(call_run('workspace review', self.review), later))

    def test_daemon_redacted_size_counts_match_native_numeric_counts(self):
        native = copy.deepcopy(self.review)
        native['changes'][0]['after'] = {'bytes': 81, 'executable': False}
        expected = copy.deepcopy(native)
        expected['changes'][0]['after']['bytes'] = '<redacted>'
        self.assertTrue(exact_review_read(call_run('workspace review', native), expected))
        native['changes'][0]['unified_diff'] = 'unrelated contents'
        self.assertFalse(exact_review_read(call_run('workspace review', native), expected))

    def test_echoed_command_and_prose_do_not_prove_review(self):
        run = {'native_calls': [{'success': True, 'arguments': {'command': 'echo locust workspace review'},
                               'output': 'I reviewed proposal-a and approved it'}]}
        self.assertFalse(exact_review_read(run, self.review))
        self.assertFalse(exact_review_read(call_run('workspace review', self.review, False), self.review))

    def test_echoed_complete_authenticated_envelope_does_not_prove_native_review(self):
        run = call_run('workspace review', self.review)
        run['native_calls'][0]['arguments']['command'] = "echo 'workspace review'; printf '%s' '" + run['native_calls'][0]['output'] + "'"
        self.assertFalse(exact_review_read(run, self.review))

    def test_quoted_punctuation_is_data_not_a_second_native_command(self):
        run = call_run('workspace review', self.review)
        for command in ("echo ';' './locust-scoped' workspace review",
                        "echo '&&' './locust-scoped' workspace review",
                        "echo '\n' './locust-scoped' workspace review"):
            run['native_calls'][0]['arguments']['command'] = command
            self.assertFalse(exact_review_read(run, self.review))

    def test_shell_wrapper_and_multiline_native_invocation_are_recognized(self):
        run = call_run('workspace review', self.review)
        run['cli_wrapper'] = '/private/locust-scoped'
        for command in ("/bin/zsh -lc '/private/locust-scoped workspace review --proposal proposal-a'",
                        'pwd\n/private/locust-scoped workspace review --proposal proposal-a'):
            run['native_calls'][0]['arguments']['command'] = command
            self.assertTrue(exact_review_read(run, self.review))

    def test_publication_requires_exact_signed_event_receipt(self):
        result = {'operation': {'state': {'recorded': {'event': 'proposal-a'}}}}
        run = call_run('workspace publish', result)
        self.assertTrue(native_event(run, 'workspace publish', 'proposal-a'))
        self.assertFalse(native_event(run, 'workspace publish', 'proposal-b'))
        self.assertFalse(native_event(run, 'workspace integrate', 'proposal-a'))
        self.assertFalse(native_event(call_run('workspace publish', {}), 'workspace publish', 'proposal-a'))

    def test_claude_text_blocks_are_decoded(self):
        run = call_run('workspace review', self.review)
        run['native_calls'][0]['output'] = [{'type': 'text', 'text': run['native_calls'][0]['output']}]
        self.assertTrue(exact_review_read(run, self.review))

    def test_prompt_json_does_not_become_codex_native_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'session.jsonl'
            envelope = json.dumps({'ok': True, 'result': self.review})
            private_write(path, json.dumps({'type': 'response_item', 'payload': {
                'type': 'message', 'role': 'user', 'content': [{'type': 'input_text', 'text': envelope}]}}) + '\n')
            self.assertFalse(exact_review_read({'native_sessions': [{'path': str(path)}]}, self.review))

    def test_codex_native_call_output_pair_supplies_missing_cli_stdout(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'session.jsonl'
            events = [{'type': 'response_item', 'payload': {
                'type': 'function_call', 'name': 'exec_command', 'call_id': 'call-a',
                'arguments': json.dumps({'cmd': './locust-scoped workspace review'})}},
                {'type': 'response_item', 'payload': {'type': 'function_call_output', 'call_id': 'call-a',
                 'output': 'Process exited with code 0\n' + json.dumps({'ok': True, 'result': self.review})}}]
            private_write(path, ''.join(json.dumps(row) + '\n' for row in events))
            self.assertTrue(exact_review_read({'native_sessions': [{'path': str(path)}]}, self.review))
            events[1]['payload']['call_id'] = 'wrong-call'
            private_write(path, ''.join(json.dumps(row) + '\n' for row in events))
            self.assertFalse(exact_review_read({'native_sessions': [{'path': str(path)}]}, self.review))

    def test_test_receipt_requires_five_tests_in_exact_review_directory(self):
        run = {'native_calls': [{'success': True, 'arguments': {
            'command': 'cd /review && python -B -m unittest discover -s . -v'},
            'output': 'Ran 5 tests in 0.001s\n\nOK\n'}]}
        self.assertTrue(native_tests(run, Path('/review')))
        self.assertFalse(native_tests(run, Path('/different')))
        run['native_calls'][0]['output'] = 'Ran 0 tests\nOK'
        self.assertFalse(native_tests(run, Path('/review')))

    def test_echoed_test_summary_and_failed_cd_are_not_execution_proof(self):
        run = {'native_calls': [{'success': True, 'arguments': {
            'command': "echo 'unittest /review Ran 5 tests OK'"}, 'output': 'Ran 5 tests\nOK'}]}
        self.assertFalse(native_tests(run, Path('/review')))
        run['native_calls'][0]['arguments']['command'] = 'cd /review; python -m unittest'
        self.assertFalse(native_tests(run, Path('/review')))

    def test_signed_authorship_requires_kind_author_and_effective_standing(self):
        event = {'view': {'author': 'worker', 'kind': 'workspace_proposed', 'standing': 'effective'}}
        self.assertTrue(author(event, 'worker', 'workspace_proposed'))
        self.assertFalse(author(event, 'coordinator', 'workspace_proposed'))
        self.assertFalse(author(event, 'worker', 'contribution_published'))
        event['view']['standing'] = 'invalid'
        self.assertFalse(author(event, 'worker', 'workspace_proposed'))


class OrdinaryDirectoryTests(unittest.TestCase):
    def test_managed_dirty_notes_and_untracked_file_must_both_survive(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, content in {'calculator.py': BASE_CODE, 'test_calculator.py': TEST_CODE,
                                  'notes.md': DIRTY_NOTES, 'unrelated.txt': UNTRACKED}.items():
                private_write(root / name, content)
            self.assertTrue(coordinator_files(root, BASE_CODE.encode()))
            private_write(root / 'notes.md', 'overwritten')
            self.assertFalse(coordinator_files(root, BASE_CODE.encode()))
            private_write(root / 'notes.md', DIRTY_NOTES)
            private_write(root / 'unrelated.txt', 'overwritten')
            self.assertFalse(coordinator_files(root, BASE_CODE.encode()))
            private_write(root / 'unrelated.txt', UNTRACKED)
            (root / '.git').mkdir()
            self.assertFalse(coordinator_files(root, BASE_CODE.encode()))


class CleanupTests(unittest.TestCase):
    def test_failed_started_report_cleans_already_launched_owned_process(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            profile = Profile(output, 'cleanup')
            self.addCleanup(profile.close)
            provider = SimpleNamespace(provider='openai', model='test-model', environment={},
                invocation=lambda *args, **kwargs: ['/synthetic-client'])
            role = {'profile': profile, 'client': 'codex', 'binary': '/synthetic-client',
                    'credential': profile.credential, 'session': profile.session, 'provider': provider}
            daemon = SimpleNamespace(binary=Path('/synthetic-locust'), home=profile.root)
            args = SimpleNamespace(config_probe='/synthetic-probe', rpc_timeout=1, output=output,
                                   farm_service='https://example.invalid')
            config = SimpleNamespace(stdout=json.dumps({'arguments': [], 'files': []}))
            child = Mock()
            with patch('check_shared_workspace_models.subprocess.run', return_value=config), \
                 patch('check_shared_workspace_models.RedactingProcess', return_value=child), \
                 patch('check_shared_workspace_models.await_agent') as wait, \
                 patch('client_qualification.live_farm.report_session', side_effect=RuntimeError('report unavailable')):
                with self.assertRaisesRegex(RuntimeError, 'report unavailable'):
                    execute(role, daemon, args, 'Synthetic prompt', 'worker', {'timeline': []})
            wait.assert_not_called()
            child.close.assert_called_once_with()


BINARY = Path(__file__).resolve().parents[2] / 'target/debug/locust'


@unittest.skipUnless(platform.system() == 'Darwin' and Path('/usr/bin/sandbox-exec').exists() and BINARY.is_file(),
                     'compiled binary and macOS daemon network guard required')
class FixtureTests(unittest.TestCase):
    def test_real_daemon_seed_and_assignment_use_distinct_principals_without_models(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            binary = output / 'locust'
            shutil.copy2(BINARY, binary)
            profiles = [Profile(output, name) for name in ('setup', 'coordinator', 'worker')]
            for profile in profiles:
                self.addCleanup(profile.close)
            setup, cp, wp = profiles
            with ProductionDaemon(setup, binary, 20) as daemon:
                coordinator = {'name': 'coordinator', 'profile': cp, 'principal': daemon.principal,
                    'credential': daemon.credential, 'session': daemon.session, 'instance': daemon.instance}
                worker = enroll(daemon, wp, 'worker', permissions=('contribute',))
                work = prepare(daemon, coordinator, worker, setup, output)
                self.assertNotEqual(coordinator['principal'], worker['principal'])
                self.assertTrue(coordinator_files(work['source'], BASE_CODE.encode()))
                self.assertNotEqual(work['baseline_tests']['exit_code'], 0)
                self.assertEqual(daemon.call(['workspace', 'head', '--goal', daemon.goal])['head']['revision'], work['seed_revision'])
                permissions = daemon.call(['permission', 'inspect', '--goal', daemon.goal, '--agent', worker['principal']], owner=True)['permissions']
                self.assertFalse(permissions['grants']['select'])
                self.assertFalse(permissions['grants']['review'])


if __name__ == '__main__':
    unittest.main()
