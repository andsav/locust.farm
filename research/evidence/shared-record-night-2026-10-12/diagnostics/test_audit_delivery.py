"""Read-only replay and adversarial archive tests; no candidate or daemon execution."""
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest

from audit_delivery import Audit, policy_metadata

ROOT = Path(__file__).resolve().parent
SCRIPTED = ROOT.parent/'shared-record-policy-paid-preflight-2026-10-12'


def read(path):
    return json.loads(Path(path).read_text())


def save(path, value):
    Path(path).write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')


@contextmanager
def altered_study():
    """Copy only sealed evidence; no binary, private daemon state or hidden scores."""
    with tempfile.TemporaryDirectory(prefix='delivery-audit-test-') as directory:
        target=Path(directory)
        seal=read(SCRIPTED/'seal.json')
        for name in set(seal['files']) | {'seal.json','manifest.json'}:
            path=target/name
            path.parent.mkdir(parents=True,exist_ok=True)
            shutil.copy2(SCRIPTED/name,path)
        yield target


def reseal(study, name):
    seal=read(study/'seal.json')
    seal['files'][name]=hashlib.sha256((study/name).read_bytes()).hexdigest()
    save(study/'seal.json',seal)


def first_trial(study,arm):
    path=sorted((study/'trials').glob(f'*/{arm}.json'))[0]
    return path,read(path)


def failure_names(report):
    return {row['check'] for row in report['failures']}


class DeliveryAuditTests(unittest.TestCase):
    def test_completed_scripted_replay_checks_real_daemon_and_exposure(self):
        report=Audit(SCRIPTED).run()
        self.assertEqual(report['status'],'pass',report['failures'])
        self.assertEqual(report['warnings'],[])
        self.assertTrue(report['synthetic'])
        self.assertEqual(report['summary']['isolated_goals'],24)
        self.assertEqual(report['summary']['publication_receipts'],208)
        self.assertEqual(report['summary']['recipients'],72)
        self.assertEqual(report['summary']['recipients_with_peer_metadata_dispatched'],24)
        self.assertEqual(report['summary']['recipients_with_peer_source_submitted'],24)
        self.assertEqual(report['summary']['empty_deliveries'],4)
        for trial in report['trials'].values():
            self.assertFalse(trial['recipients']['agent-2']['own_fully_public_valid_prefix'])
            self.assertEqual(trial['recipients']['agent-2']['delivered_peer_candidate_ids'],[])
            if trial['kind']=='C':
                self.assertTrue(all(not row['delivered_peer_candidate_ids'] for row in trial['recipients'].values()))

    def test_filtered_policy_preserves_failures_and_exact_non_improvement(self):
        rows=[]
        for sequence,(valid,score) in enumerate([(False,.1),(True,1.0),(True,1-1e-12),(True,1.0),(False,.01),(True,1-1e-12)]):
            source=f'# candidate {sequence}\n'
            rows.append(dict(id=f'c{sequence}',sequence=sequence,agent='agent-0',source=source,
                             sha256=hashlib.sha256(source.encode()).hexdigest(),parent_id=None,
                             public=dict(instances=[{'valid':valid},{'valid':valid}],total_instances=2,
                                         valid_instances=2 if valid else 0,normalized=score)))
        eligible,withheld=policy_metadata(rows,'F')
        self.assertEqual([r['candidate_id'] for r in eligible],['c0','c3','c4','c5'])
        self.assertEqual([r['eligibility_reason'] for r in eligible],['failed_public_validation','non_improving','failed_public_validation','non_improving'])
        self.assertEqual([r['reason'] for r in withheld],['first_valid_baseline','strict_public_improvement'])
        for policy in ['E','D']:
            self.assertEqual(len(policy_metadata(rows,policy)[0]),len(rows))

    def test_returned_peer_source_is_not_claimed_submitted_without_a_later_request(self):
        source='def solve(i): return []\n'
        candidate=dict(id='peer-c0',agent='agent-1',source=source,sha256=hashlib.sha256(source.encode()).hexdigest())
        value=dict(candidate_id='peer-c0',source=source,sha256=candidate['sha256'])
        request=dict(id='r1',status='completed',reserved_at=1,payload={'input':[]},
                     response={'output':[dict(type='function_call',name='get_source',call_id='source-call',
                                              arguments=json.dumps({'candidate_id':'peer-c0'}))]})
        agent=dict(id='block/P/agent-0',name='agent-0',trace=[dict(event='tool_results',time=2,results=[['source-call',value]])])
        prior=dict(trace=[],candidates=[])
        audit=Audit(SCRIPTED)
        first=audit.source_tools(agent,prior,[request],{'peer-c0'},{'peer-c0':candidate},[{'candidate_id':'peer-c0'}])
        self.assertEqual(first['peer_full_source_return_count'],1)
        self.assertEqual(first['peer_source_submitted_count'],0)
        later=dict(id='r2',status='completed',reserved_at=3,
                   payload={'input':[dict(type='function_call_output',call_id='source-call',output=json.dumps(value))]},
                   response={'output':[]})
        second=audit.source_tools(agent,prior,[request,later],{'peer-c0'},{'peer-c0':candidate},[{'candidate_id':'peer-c0'}])
        self.assertEqual(second['peer_source_submitted_count'],1)
        self.assertEqual(second['full_source_returns'][0]['subsequent_response_request_ids'],['r2'])
        self.assertEqual(audit.failures,[])

    def test_parent_authority_check_is_separate_from_a_returned_source(self):
        request=dict(id='r1',status='completed',reserved_at=1,payload={'input':[]},
                     response={'output':[dict(type='function_call',name='evaluate',call_id='evaluate-call',
                                              arguments=json.dumps({'source':'def solve(i): return []','parent_id':'peer-c0'}))]})
        agent=dict(id='block/P/agent-0',name='agent-0',trace=[dict(event='tool_results',time=2,results=[['evaluate-call',{'candidate_id':'own-c1'}]])])
        audit=Audit(SCRIPTED)
        result=audit.source_tools(agent,dict(trace=[],candidates=[]),[request],{'peer-c0'},{},[{'candidate_id':'peer-c0'}])
        self.assertEqual(result['non_own_parent_ids_declared'],['peer-c0'])
        self.assertEqual(result['full_source_returns'],[])
        self.assertEqual(audit.failures,[])

    def test_denied_source_lookup_does_not_become_a_delivery(self):
        request=dict(id='r1',status='completed',reserved_at=1,payload={'input':[]},
                     response={'output':[dict(type='function_call',name='get_source',call_id='source-call',
                                              arguments=json.dumps({'candidate_id':'withheld-c0'}))]})
        agent=dict(id='block/C/agent-0',name='agent-0',trace=[dict(event='tool_results',time=2,results=[['source-call',{'error':'PermissionError: source is not eligible'}]])])
        audit=Audit(SCRIPTED)
        result=audit.source_tools(agent,dict(trace=[],candidates=[]),[request],set(),{},[])
        self.assertEqual(result['full_source_returns'],[])
        self.assertEqual(len(result['denied_or_failed_source_calls']),1)
        self.assertEqual(audit.failures,[])

    def test_unsealed_study_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError,'live study refused'):
                Audit(directory).run()

    def test_changed_sealed_bytes_are_refused(self):
        with altered_study() as study:
            path,trial=first_trial(study,'P_F')
            trial['record']['valid_prefix']['agent-2']=True
            save(path,trial)
            with self.assertRaisesRegex(ValueError,'sealed bytes changed'):
                Audit(study).run()

    def test_false_own_validity_gate_fails_even_after_resealing(self):
        with altered_study() as study:
            path,trial=first_trial(study,'P_F')
            trial['record']['valid_prefix']['agent-2']=True
            save(path,trial)
            reseal(study,str(path.relative_to(study)))
            report=Audit(study).run()
            self.assertIn('own_valid_prefix_gate_exact',failure_names(report))

    def test_control_peer_delivery_fails_even_after_resealing(self):
        with altered_study() as study:
            path,trial=first_trial(study,'C_E')
            peer=trial['record']['candidates']['agent-1'][0]['candidate_id']
            trial['record']['deliveries'][0]['delivered_candidate_ids'].append(peer)
            save(path,trial)
            reseal(study,str(path.relative_to(study)))
            report=Audit(study).run()
            self.assertIn('control_has_no_peer_delivery',failure_names(report))
            self.assertIn('complete_delivery_exact',failure_names(report))

    def test_copied_prefix_request_mutation_fails_ancestry(self):
        with altered_study() as study:
            path,trial=first_trial(study,'P_D')
            trial['agents'][0]['requests'][0]['payload']['input'][0]['content']+=' altered'
            save(path,trial)
            reseal(study,str(path.relative_to(study)))
            report=Audit(study).run()
            self.assertIn('copied_prefix_requests_exact',failure_names(report))
            self.assertIn('matched_C_P_common_request_ancestry',failure_names(report))

    def test_truncated_returned_source_fails_full_source_check(self):
        with altered_study() as study:
            path,trial=first_trial(study,'P_E')
            found=False
            for event in trial['agents'][0]['trace']:
                if event['event']=='tool_results':
                    for _,value in event['results']:
                        if 'source' in value:
                            value['source']=value['source'][:-1]
                            found=True
                            break
                if found: break
            self.assertTrue(found)
            save(path,trial)
            reseal(study,str(path.relative_to(study)))
            report=Audit(study).run()
            self.assertIn('full_source_return_exact',failure_names(report))

    def test_wrong_daemon_author_fails_receipt_evidence(self):
        with altered_study() as study:
            path=study/'daemon-events.jsonl'
            events=[json.loads(line) for line in path.read_text().splitlines()]
            event=next(row for row in events if row.get('operation')=='contribution inspect' and row.get('response',{}).get('ok'))
            event['response']['result']['contribution_inspected']['contribution']['view']['author']='f'*64
            path.write_text(''.join(json.dumps(row,sort_keys=True)+'\n' for row in events))
            reseal(study,'daemon-events.jsonl')
            report=Audit(study).run()
            self.assertIn('daemon_effective_author_and_event',failure_names(report))

    def test_missing_daemon_publication_fails_event_binding(self):
        with altered_study() as study:
            path=study/'daemon-events.jsonl'
            events=[json.loads(line) for line in path.read_text().splitlines()]
            index=next(i for i,row in enumerate(events) if row.get('operation')=='contribution publish' and row.get('response',{}).get('ok'))
            events.pop(index)
            path.write_text(''.join(json.dumps(row,sort_keys=True)+'\n' for row in events))
            reseal(study,'daemon-events.jsonl')
            report=Audit(study).run()
            self.assertIn('daemon_publication_receipt',failure_names(report))


if __name__=='__main__':
    unittest.main()
