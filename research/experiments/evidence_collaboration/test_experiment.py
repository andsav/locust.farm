import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from protocol import (PROMPTS,case_key,choose,component_ids,configurations,digest,job_graph,
                      make_body,parse,public,score)
from transport import BudgetExceeded,Ledger,Transport,atomic_json
from experiment import Experiment,load,prepare,verify
from analysis import paired,holm,readiness


def row(family='3hop__1_2_3',answerable=True):
    return {'id':family,'question':'Find the supported answer.','answer':'Alpha','answer_aliases':['The Alpha'],
            'answerable':answerable,'question_decomposition':[{'id':family+str(i),'answer':'DO_NOT_SEND'} for i in range(int(family[0]))],
            'paragraphs':[{'idx':i,'title':str(i),'paragraph_text':('Goldhere Alpha' if i==0 and answerable else 'Unrelated text'),
                           'is_supporting':i==0 and answerable} for i in range(6)]}


def finding(answerable=True,ids=(0,)):
    return {'answerable':answerable,'answer':'Alpha' if answerable else '', 'support_ids':list(ids),
            'evidence':[],'missing_links':[],'conflicts':[],'changed_by':[],'confidence':.8}


def fake_cohort():
    ids=['3hop__1_2_3','4hop__4_5_6_7']
    return {'development_ids':ids,'main_ids':ids,'repeat_ids':ids,'main_strata':{'3':1,'4':1},
            'families':{k:{'rows':[row(k,False),row(k,True)]} for k in ids}}


class ProtocolTests(unittest.TestCase):
    def test_labels_never_enter_public_payload(self):
        r=row();p=public(r)
        self.assertEqual(set(p),{'question','documents'})
        self.assertNotIn('DO_NOT_SEND',json.dumps(p))
        for doc in p['documents']:self.assertEqual(set(doc),{'id','title','text'})
    def test_graph_shares_only_initial_work(self):
        jobs=job_graph('case',configurations([12000]))
        self.assertEqual(len(jobs),19)
        for i in range(3):
            d=jobs[f'case/D_12000/revision/{i}'];e=jobs[f'case/E_12000/revision/{i}']
            self.assertEqual(d['deps'],[f'case/G_12000/initial/{i}'])
            self.assertEqual(e['deps'],[f'case/G_12000/initial/{j}' for j in range(3)])
            self.assertEqual(d['cap'],e['cap']);self.assertEqual(d['prompt'],e['prompt'])
        for arm in ('D','E'):
            final=jobs[f'case/{arm}_12000/final/0']
            self.assertIsNone(final['shard']);self.assertEqual(len(final['deps']),6)
    def test_each_arm_has_declared_output_cap(self):
        for budget in (6000,12000,18000,24000):
            for config in configurations([budget]):
                self.assertEqual(sum(j['cap'] for j in job_graph('case',[config]).values()),budget)
    def test_shards_partition_without_using_gold(self):
        r=row();jobs=job_graph('case',configurations([12000]));manifest={'model':'M','effort':'high','seed':7,'prompts':PROMPTS}
        seen=[]
        for i in range(3):
            body=make_body(jobs[f'case/G_12000/initial/{i}'],r,{},manifest)
            seen.extend(d['id'] for d in json.loads(body['input'])['documents'])
        self.assertEqual(sorted(seen),list(range(6)))
        flipped=json.loads(json.dumps(r))
        for p in flipped['paragraphs']:p['is_supporting']=not p['is_supporting']
        self.assertEqual(public(r,7),public(flipped,7))
    def test_peer_delivery_changes_only_prior_findings(self):
        jobs=job_graph('case',configurations([12000]));manifest={'model':'M','effort':'high','seed':7,'prompts':PROMPTS}
        records={f'case/G_12000/initial/{i}':{'parsed':finding(ids=[i])} for i in range(3)}
        d=make_body(jobs['case/D_12000/revision/1'],row(),records,manifest)
        e=make_body(jobs['case/E_12000/revision/1'],row(),records,manifest)
        di=json.loads(d.pop('input'));ei=json.loads(e.pop('input'))
        self.assertEqual(d,e);self.assertEqual(len(di.pop('prior_findings')),1);self.assertEqual(len(ei.pop('prior_findings')),3)
        self.assertEqual(di,ei)
    def test_scoring_distinguishes_answer_and_support(self):
        r={'status':'completed','parsed':finding(ids=[])}
        self.assertTrue(score(r,row())['answer']);self.assertFalse(score(r,row())['joint'])
        self.assertFalse(score({'status':'incomplete','parsed':finding()},row())['answer'])
    def test_abstention_is_not_full_case_success(self):
        r={'status':'completed','parsed':finding(False,[])}
        self.assertTrue(score(r,row(answerable=False))['joint']);self.assertFalse(score(r,row())['joint'])
    def test_schema_rejects_unknown_citations_and_nan(self):
        f=finding(ids=[99]);self.assertIsNone(parse(json.dumps(f),set(range(6))))
        f=finding();f['confidence']=float('nan');self.assertIsNone(parse(json.dumps(f),set(range(6))))
        f=finding(False,[]);f['answer']='unsupported';self.assertIsNone(parse(json.dumps(f),set(range(6))))
    def test_selection_excludes_shared_components(self):
        pool={}
        for h in (3,4):
            for i in range(8):
                k=f'{h}hop__{i}';r=row(k);r['question']=k
                r['question_decomposition']=[{'id':f'{h}-{i}-{j}'} for j in range(h)]
                pool[k]=[dict(r,answerable=False),dict(r,answerable=True)]
        # Two otherwise distinct questions share a seed question.
        pool['3hop__1'][0]['question_decomposition'][0]['id']='shared'
        pool['3hop__2'][0]['question_decomposition'][0]['id']='shared'
        a=choose(pool,12,set(),5);b=choose(dict(reversed(list(pool.items()))),12,set(),5)
        self.assertEqual(a,b);used=set()
        for k in a:
            ids=component_ids(pool[k][0]);self.assertFalse(ids&used);used|=ids
    def test_paired_inference_and_holm(self):
        p=paired([1]*10,[0]*10,resamples=100)
        self.assertEqual(p['difference'],1);self.assertEqual(p['ci95'],[1,1]);self.assertAlmostEqual(p['mcnemar_exact_p'],2/1024)
        self.assertEqual(holm({'a':.01,'b':.04}),{'a':.02,'b':.04})


class LedgerTests(unittest.TestCase):
    def test_atomic_reservation_and_shared_cost(self):
        with tempfile.TemporaryDirectory() as tmp:
            ledger=Ledger(Path(tmp)/'budget.sqlite',1)
            job={'id':'shared','case_id':'c','config':'G_12000'}
            key=ledger.reserve('r',job,.2,1,.25)
            with self.assertRaises(ValueError):ledger.reserve('r',job,.2,1,.25)
            for arm in ('D','E'):
                with self.assertRaises(BudgetExceeded):ledger.reserve('r',dict(job,id=arm,config=arm+'_12000'),.1,1,.25)
            ledger.finish(key,'completed',.05)
            ledger.reserve('r',dict(job,id='d',config='D_12000'),.1,1,.25)
            self.assertAlmostEqual(ledger.total(),.15)
            # D's private call is not charged to E; the shared initial call is.
            ledger.reserve('r',dict(job,id='e',config='E_12000'),.2,1,.25)
    def test_study_ceiling_is_shared_between_runs(self):
        with tempfile.TemporaryDirectory() as tmp:
            ledger=Ledger(Path(tmp)/'budget.sqlite',.3)
            job={'id':'a','case_id':'c','config':'S_direct_12000'}
            ledger.reserve('one',job,.2,.3,1)
            with self.assertRaises(BudgetExceeded):ledger.reserve('two',job,.2,.3,1)
            with self.assertRaises(ValueError):Ledger(Path(tmp)/'budget.sqlite',10)


class IntegrationTests(unittest.TestCase):
    def setup_run(self,tmp):
        root=Path(tmp);atomic_json(root/'cohort.json',fake_cohort())
        folder,_=prepare(root,'unit','development',budgets=(12000,))
        return folder
    @staticmethod
    def opener(req,timeout):
        body=json.loads(req.data);material=json.loads(body['input'])
        assert 'question_decomposition' not in material and 'DO_NOT_SEND' not in body['input']
        ids=[d['id'] for d in material['documents'] if 'Goldhere' in d['text']]
        obj=finding(bool(ids),ids)
        return io.BytesIO(json.dumps({'status':'completed','model':'gpt-6-luna','service_tier':'default',
            'usage':{'input_tokens':10,'output_tokens':20,'output_tokens_details':{'reasoning_tokens':0}},
            'output':[{'type':'message','content':[{'type':'output_text','text':json.dumps(obj)}]}]}).encode())
    def transport_class(self,opener=None):
        selected=opener or self.opener
        class FakeTransport(Transport):
            def __init__(self,*args,**kwargs):super().__init__(*args,**kwargs,opener=selected)
        return FakeTransport
    def test_full_graph_resume_and_offline_verification(self):
        with tempfile.TemporaryDirectory() as tmp, patch.dict(os.environ,{'OPENAI_API_KEY':'test-only'}):
            folder=self.setup_run(tmp);e=Experiment(folder,self.transport_class());s=e.run(workers=4)
            self.assertEqual(s['attempted_calls'],76);self.assertEqual(s['status_counts'],{'completed':76})
            before=e.ledger.total();second=Experiment(folder,self.transport_class()).run(workers=4)
            self.assertEqual(second['attempted_calls'],76);self.assertEqual(e.ledger.total(),before)
            self.assertEqual(verify(folder)['verified_records'],76)
    def test_reservation_precedes_network(self):
        with tempfile.TemporaryDirectory() as tmp, patch.dict(os.environ,{'OPENAI_API_KEY':'test-only'}):
            folder=self.setup_run(tmp);observed=[]
            def opener(req,timeout):
                with Ledger(Path(tmp)/'budget.sqlite',250).connect() as db:
                    observed.append(db.execute("SELECT COUNT(*) FROM calls WHERE status='pending'").fetchone()[0])
                return self.opener(req,timeout)
            Experiment(folder,self.transport_class(opener)).run(workers=1,first_families=1)
            self.assertTrue(observed);self.assertTrue(all(n>=1 for n in observed))
    def test_timeout_not_retried_and_dependents_fail(self):
        with tempfile.TemporaryDirectory() as tmp, patch.dict(os.environ,{'OPENAI_API_KEY':'test-only'}):
            folder=self.setup_run(tmp);calls=[]
            def timeout(req,timeout):calls.append(1);raise TimeoutError()
            e=Experiment(folder,self.transport_class(timeout));e.run(workers=1,first_families=1)
            self.assertEqual(len(calls),3);self.assertTrue((folder/'stop.json').exists())
            before=len(calls)
            # Resume with a healthy fake endpoint; previous failures remain failures.
            e2=Experiment(folder,self.transport_class());s=e2.run(workers=2,first_families=1,resume_after_stop=True)
            self.assertEqual(len(calls),before);self.assertEqual(s['status_counts']['transport_error'],3)
            self.assertGreater(s['status_counts'].get('dependency_failed',0),0)
    def test_orphan_reservation_cannot_be_resent(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder=self.setup_run(tmp);e=Experiment(folder,self.transport_class());job=next(j for j in e.jobs.values() if not j['deps'])
            e.ledger.reserve('unit',job,.01,60,.25)
            resumed=Experiment(folder,self.transport_class())
            self.assertEqual(resumed.records[job['id']]['status'],'interrupted')
    def test_manifest_change_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder=self.setup_run(tmp);m=json.loads((folder/'manifest.json').read_text());m['effort']='low';atomic_json(folder/'manifest.json',m)
            with self.assertRaises(ValueError):load(folder)

if __name__=='__main__':unittest.main()
