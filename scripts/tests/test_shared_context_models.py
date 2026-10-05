"""False-pass guards for the shared-context real-model experiment."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from client_qualification import collaboration_case as case
from client_qualification.collaboration_evidence import evaluate, _tree_matches


class CollaborationCaseTests(unittest.TestCase):
    def test_private_requirement_is_absent_from_builder_setup(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            seeded = case.seed(root / 'researcher', root / 'builder')
            public = seeded['prompts']['builder'] + ''.join(
                (root / 'builder' / name).read_text() for name in seeded['builder_files'])
            self.assertNotIn(seeded['researcher_only']['rule_summary'], public)
            self.assertNotIn('assets\\..\\outside.txt', public)
            self.assertFalse((root / 'builder' / 'private').exists())
            self.assertFalse(case.verify(root / 'builder')['passed'])

    def test_oracle_rejects_a_fix_that_only_handles_double_backslashes(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            case.seed(root / 'researcher', root / 'builder')
            path = root / 'builder' / 'safe_member.py'
            path.write_text(case.STARTER.replace('path = PurePosixPath(name)',
                                                r'path = PurePosixPath(name.replace("\\\\", "/"))'))
            self.assertFalse(case.verify(path.parent)['passed'])

    def test_oracle_accepts_required_portable_paths(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            (root / 'safe_member.py').write_text('''import ntpath
from pathlib import PurePosixPath

def normalize_member_path(name):
    if not isinstance(name, str) or not name or "\\x00" in name:
        return None
    normalized = name.replace("\\\\", "/")
    path = PurePosixPath(normalized)
    if ntpath.splitdrive(normalized)[0] or path.is_absolute() or ".." in path.parts:
        return None
    return path.as_posix()
''')
            self.assertTrue(case.verify(root)['passed'])

    def test_rejects_overlapping_role_workspaces(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            with self.assertRaises(ValueError):
                case.seed(root, root / 'builder')


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.finding = {'event': {
            'view': {'event': 'finding', 'author': 'researcher',
                     'kind': 'contribution_published', 'standing': 'effective'},
            'body': {'contribution_published': {'attempt': None, 'context': {'scope': 'goal'}}},
            'task': None, 'text': 'Windows backslash paths reach Linux; normalize before checking ..'},
            'text_complete': True}
        self.receipt = '0123456789abcdef0123456789abcdef'
        self.events = []
        self.call(1, 'locust_context_read', {'goal': 'goal'}, {'context': {
            'items': [self.finding], 'receipt': self.receipt}})
        self.call(2, 'locust_context_acknowledge', {'goal': 'goal', 'receipt': self.receipt},
                  {'context_acknowledged': {'goal': 'goal', 'principal': 'builder', 'session': 'session-b',
                      'entries': [{'event': 'finding', 'version': 'exact-version'}]}})
        (self.root / 'stdout').write_text(json.dumps({'type': 'turn.completed', 'usage': {'output_tokens': 1}}))
        self.report = {'goal': 'goal', 'base': 'base', 'principals': {
            'researcher': {'principal': 'researcher', 'instance': 'session-r'},
            'builder': {'principal': 'builder', 'instance': 'session-b'}},
            'runs': [{'exit_code': 0, 'response': {'status': 'completed', 'totals': {'output_tokens': 1}}},
                     {'exit_code': 0, 'mcp_events': str(self.root / 'mcp.jsonl'),
                      'stdout': str(self.root / 'stdout'), 'native_calls': []}],
            'research_findings': [copy.deepcopy(self.finding)], 'final_context': [],
            'artifact_path': str(self.root / 'missing.py')}

    def call(self, ident, tool, args, response):
        self.events += [{'direction': 'client_request', 'method': 'tools/call',
                         'id': ident, 'tool': tool, 'arguments': args},
                        {'direction': 'bridge_response', 'id': ident, 'tool': tool,
                         'isError': False, 'result': {'ok': True, 'result': response}}]

    def evaluate(self):
        (self.root / 'mcp.jsonl').write_text(''.join(json.dumps(x) + '\n' for x in self.events))
        return evaluate(self.report)

    def test_exact_receipt_chain_and_substituted_reference(self):
        self.assertTrue(self.evaluate()['builder_acknowledged_exact_receipt'])
        self.events[2]['arguments']['receipt'] = 'different-reference'
        self.assertFalse(self.evaluate()['builder_acknowledged_exact_receipt'])

    def test_matching_event_id_does_not_mask_wrong_delivered_text(self):
        self.events[1]['result']['result']['context']['items'][0]['event']['text'] = 'Unrelated content'
        result = self.evaluate()
        self.assertFalse(result['builder_read_finding_with_exact_receipt'])
        self.assertFalse(result['builder_acknowledged_exact_receipt'])

    def test_another_sessions_receipt_cannot_qualify_builder(self):
        self.events[3]['result']['result']['context_acknowledged']['session'] = 'another-session'
        self.assertFalse(self.evaluate()['builder_acknowledged_exact_receipt'])

    def test_task_attribution_does_not_claim_direct_contribution_citation(self):
        def item(kind, ident, task, text='', body=None):
            return {'event': {'view': {'kind': kind, 'event': ident, 'author': 'builder'},
                              'task': task, 'text': text, 'body': body or {}}}
        self.report['final_context'] = [
            item('task_opened', 'task-id', 'task:task-id', 'Use finding'),
            item('attempt_started', 'attempt-id', 'task:task-id'),
            item('contribution_published', 'contribution', 'task:task-id', 'Implemented helper',
                 {'contribution_published': {'attempt': 'attempt-id'}})]
        result = self.evaluate()
        self.assertTrue(result['contribution_is_bound_to_citing_task_and_attempt'])
        self.assertFalse(result['direct_contribution_summary_cites_finding_event'])

    def test_tree_review_must_match_exact_materialized_artifact(self):
        changed = case.STARTER.replace('path = PurePosixPath(name)', 'path = PurePosixPath(name.strip())')
        artifact = self.root / 'safe_member.py'
        artifact.write_text(changed)
        copied = self.root / 'verified-copy.py'
        copied.write_text(changed)
        review = {'proposal': {'proposal': 'proposal', 'parent': 'revision', 'result_manifest': 'manifest'},
            'review_mode': 'base_diff', 'changes': [{'path': 'safe_member.py',
                'after': {'bytes': artifact.stat().st_size, 'executable': False}}]}
        self.assertTrue(_tree_matches(review, artifact, 'revision', copied))
        artifact.write_text(changed.replace('strip()', 'upper()'))
        self.assertFalse(_tree_matches(review, artifact, 'revision', copied))
        artifact.write_text(changed)
        self.assertFalse(_tree_matches(review, artifact, 'wrong-parent', copied))
        copied.unlink()
        copied.symlink_to(artifact)
        self.assertFalse(_tree_matches(review, artifact, 'revision', copied))


if __name__ == '__main__':
    unittest.main()
