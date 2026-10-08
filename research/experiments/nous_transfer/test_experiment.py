import json
from pathlib import Path
import tempfile
from datetime import date
import unittest
from unittest.mock import patch
import io
from protocol import (BASE, BRANCHES, CONDITIONS, INITIAL, N_AGENTS, REVIEW, analyze,
                      code_hashes, digest, group_metrics, label, pad_bytes,
                      profile_blocks, request_plan)
from experiment import Experiment


def fixture():
    profiles=[{'wallet_id':f'W{i}', 'persona_prompt_text':f'Persona {i} é',
               'profile_vector':{'update_rate':i/10},'z_scores':{'update_rate':i-5}} for i in range(10)]
    return {'phase':'fixture','scope':'test only','seed':1,'agents':10,'base_prompt':BASE,'initial_prompt':INITIAL,'review_prompt':REVIEW,
            'profile_blocks':profile_blocks(profiles,'Neutral text'), 'model':'gpt-6-luna','effort':'high','max_output_tokens':4096,
            'ceiling_usd':10,'input_rate_per_million':.125,'output_rate_per_million':.5,'code_hashes':code_hashes(),
            'cases':[{'id':'q1','cluster':'c1','outcome':1,'public':{'question':'Question','information_cutoff':'2020-01-01','evidence':[{'id':'brief','text':'Evidence'}]}}]}


def initial_records(manifest):
    return {label('q1','published','initial',i):{'status':'completed','text':json.dumps({'probability':i/10,'evidence_ids':['brief'],'explanation':f'Observation {i}','used_peer_ids':[]})} for i in range(10)}


class ProtocolTests(unittest.TestCase):
    def test_live_transport_hashes_verify_and_reject_tampering(self):
        m=fixture()
        response={'status':'completed','model':'gpt-6-luna','usage':{'input_tokens':100,'output_tokens':20},
                  'output':[{'type':'message','content':[{'type':'output_text','text':json.dumps({'probability':.6,'evidence_ids':['brief'],'explanation':'fixture','used_peer_ids':[]})}]}]}
        with tempfile.TemporaryDirectory() as tmp:
            f=Path(tmp);(f/'manifest.json').write_text(json.dumps(m));(f/'manifest.sha256').write_text(digest(m))
            runner=Experiment(f)
            with patch.dict('os.environ',{'OPENAI_API_KEY':'unit-test-placeholder'}), patch('urllib.request.urlopen',return_value=io.StringIO(json.dumps(response))):
                runner.call(m['cases'][0],'neutral','initial',0)
            runner.verify_records()
            self.assertEqual(len(Experiment(f).records),1)
            runner.records[0]['request_sha256']='tampered'
            with self.assertRaises(ValueError):runner.verify_records()
    def test_byte_padding_handles_unicode(self):
        m=fixture();self.assertEqual(len({len(s.encode()) for xs in m['profile_blocks'].values() for s in xs}),1)
        self.assertEqual(len(pad_bytes('é',5).encode()),5)
    def test_wire_does_not_include_labels_profiles_or_categories(self):
        m=fixture();instructions,body=request_plan(m,{},m['cases'][0],'neutral','initial',0)
        self.assertNotIn('outcome',json.dumps(body));self.assertNotIn('cluster',json.dumps(body))
        self.assertEqual(set(body),{'question','information_cutoff','evidence','participant'})
    def test_exchange_is_only_difference_and_has_no_self_peer(self):
        m=fixture();r=initial_records(m)
        a,x=request_plan(m,r,m['cases'][0],'published','private',3)
        b,y=request_plan(m,r,m['cases'][0],'published','exchange',3)
        self.assertEqual(a,b);self.assertEqual(len(y['peer_forecasts']),9)
        self.assertNotIn(3,[p['peer_id'] for p in y['peer_forecasts']])
        self.assertEqual(x['peer_forecasts'],[])
        y['peer_forecasts']=[];self.assertEqual(x,y)
    def test_missing_or_invalid_initial_blocks_both_branches(self):
        m=fixture();r=initial_records(m);r.pop(label('q1','published','initial',9))
        for branch in BRANCHES:self.assertIsNone(request_plan(m,r,m['cases'][0],'published',branch,0))
    def test_probability_invalid_not_clamped(self):
        from protocol import parse
        for p in (True,1.1,-.1,float('nan')):
            r={'status':'completed','text':json.dumps({'probability':p,'evidence_ids':[],'explanation':'x','used_peer_ids':[]})}
            self.assertIsNone(parse(r))
    def test_brier_identity(self):
        ps=[.1,.2,.3,.4,.5,.6,.7,.8,.9,.99];m=group_metrics(ps,1)
        self.assertAlmostEqual(m['ensemble_brier'],m['individual_brier']/10+.9*m['pairwise_error_product'])
    def test_missing_is_retained_and_intervals_not_reported(self):
        s=analyze(fixture(),[])
        self.assertEqual(s['analysis_status'],'not_run');self.assertEqual(s['missing_calls'],90)
        self.assertTrue(all(x['cluster_bootstrap_95'] is None for x in s['contrasts'].values()))
        self.assertTrue(all(x['cluster_equal_brier']==.25 for x in s['groups'].values()))
    def test_full_dry_run_resume_isolation_and_tamper_rejection(self):
        m=fixture()
        with tempfile.TemporaryDirectory() as tmp:
            f=Path(tmp);(f/'manifest.json').write_text(json.dumps(m));(f/'manifest.sha256').write_text(digest(m))
            result=Experiment(f,simulated=True).run();self.assertEqual(result['attempted_calls'],90)
            self.assertEqual(result['accounted_usd'],0)
            self.assertEqual(Experiment(f,simulated=True).run()['attempted_calls'],90)
            with self.assertRaises(ValueError):Experiment(f)
            records=json.loads((f/'records.json').read_text());records[0]['request_sha256']='bad';(f/'records.json').write_text(json.dumps(records))
            with self.assertRaises(ValueError):Experiment(f,simulated=True)
    def test_prospective_cohort_rejects_outcomes_and_future_evidence(self):
        from protocol import read_cohort
        row={'id':'future1','cluster':'event1','question':'Will event happen?', 'resolution_date':'2026-12-01',
             'information_cutoff':'2026-10-08','evidence':[{'id':'source1','text':'A dated report', 'published_at':'2026-10-07','url':'https://example.org/report'}]}
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'cohort.jsonl';path.write_text(json.dumps(row)+'\n')
            cases=read_cohort(path,date(2026,10,8));self.assertIsNone(cases[0]['outcome'])
            row['outcome']=1;path.write_text(json.dumps(row)+'\n')
            with self.assertRaises(ValueError):read_cohort(path,date(2026,10,8))
            del row['outcome'];row['evidence'][0]['published_at']='2026-10-09';path.write_text(json.dumps(row)+'\n')
            with self.assertRaises(ValueError):read_cohort(path,date(2026,10,8))
    def test_unresolved_prospective_dry_run_never_scores(self):
        m=fixture();m['phase']='prospective';m['cases'][0]['outcome']=None
        m['cases'][0]['public']['evidence'][0]['id']='article1'
        with tempfile.TemporaryDirectory() as tmp:
            f=Path(tmp);(f/'manifest.json').write_text(json.dumps(m));(f/'manifest.sha256').write_text(digest(m))
            self.assertEqual(Experiment(f,simulated=True).run()['attempted_calls'],90)
            s=json.loads((f/'summary.json').read_text());self.assertEqual(s['groups'],{})
            self.assertEqual(s['analysis_status'],'awaiting_resolutions')
    def test_cluster_weighting_does_not_count_related_questions_as_independent(self):
        m=fixture();base=m['cases'][0]
        m['cases']=[dict(base,id='a1',cluster='A'),dict(base,id='a2',cluster='A'),dict(base,id='b1',cluster='B')]
        records=[]
        for case in m['cases']:
            for condition in CONDITIONS:
                for stage in ('initial',*BRANCHES):
                    for agent in range(N_AGENTS):
                        p=(1 if case['cluster']=='A' else 0) if condition=='published' else .5
                        records.append({'label':label(case['id'],condition,stage,agent),'status':'completed',
                            'condition':condition,'stage':stage,'reserved_usd':0,'cost_usd':0,
                            'text':json.dumps({'probability':p,'evidence_ids':['brief'],'explanation':'fixture','used_peer_ids':[]})})
        s=analyze(m,records)
        self.assertAlmostEqual(s['contrasts']['published/initial_profile_effect']['difference'],.25)
    def test_large_context_is_rejected_before_transport(self):
        m=fixture();m['cases'][0]['public']['evidence'][0]['text']='x'*280000
        with tempfile.TemporaryDirectory() as tmp:
            f=Path(tmp);(f/'manifest.json').write_text(json.dumps(m));(f/'manifest.sha256').write_text(digest(m))
            runner=Experiment(f,simulated=True)
            with self.assertRaises(ValueError):runner.call(m['cases'][0],'neutral','initial',0)
            self.assertEqual(runner.records,[])
    def test_manifest_modification_is_rejected(self):
        m=fixture()
        with tempfile.TemporaryDirectory() as tmp:
            f=Path(tmp);(f/'manifest.sha256').write_text(digest(m));m['effort']='low';(f/'manifest.json').write_text(json.dumps(m))
            with self.assertRaises(ValueError):Experiment(f)

if __name__=='__main__':unittest.main()
