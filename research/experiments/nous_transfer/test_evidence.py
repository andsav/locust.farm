import json
from pathlib import Path
import tempfile
import unittest
from evidence import diagnostics, verify
from protocol import analyze
from test_experiment import fixture


class SavedEvidenceTests(unittest.TestCase):
    def bundle(self):
        m=fixture()
        record={'label':'q1/neutral/initial/0','case_id':'q1','condition':'neutral','stage':'initial','status':'completed',
                'text':json.dumps({'probability':.5,'evidence_ids':['brief'],'explanation':'unit fixture','used_peer_ids':[]}),
                'usage':{'input_tokens':100,'output_tokens':20},'cost_usd':.0000225,'reserved_usd':.001,'returned_model':'gpt-6-luna'}
        return {'analysis_input':m,'records':[record],'settings':{'input_rate_per_million':.125,'output_rate_per_million':.5},
                'summary':analyze(m,[record]),'diagnostics':diagnostics(m,[record])}
    def test_scores_recomputed_and_tampering_rejected(self):
        b=self.bundle()
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'fixture.json';p.write_text(json.dumps(b))
            self.assertEqual(verify(p)['verified_records'],1)
            b['summary']['accounted_usd']=999;p.write_text(json.dumps(b))
            with self.assertRaises(ValueError):verify(p)
    def test_cost_tampering_rejected_even_if_summary_recomputed(self):
        b=self.bundle();b['records'][0]['cost_usd']=.0005;b['summary']=analyze(b['analysis_input'],b['records'])
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'fixture.json';p.write_text(json.dumps(b))
            with self.assertRaises(ValueError):verify(p)
    def test_revision_change_is_scored_against_initial_forecast(self):
        b=self.bundle();r=dict(b['records'][0]);r['label']='q1/neutral/exchange/0';r['stage']='exchange'
        obj=json.loads(r['text']);obj['probability']=.8;obj['used_peer_ids']=[3];r['text']=json.dumps(obj)
        d=diagnostics(b['analysis_input'],b['records']+[r])['revision_diagnostics_descriptive_only']['neutral/exchange']
        self.assertEqual(d['helpful'],1);self.assertEqual(d['harmful'],0)
        self.assertEqual(d['self_reported_peer_use'],1)
        self.assertAlmostEqual(d['mean_individual_brier_change'],-.21)
    def test_diagnostics_use_case_specific_evidence_ids(self):
        b=self.bundle();b['analysis_input']['cases'][0]['public']['evidence'][0]['id']='article-1'
        obj=json.loads(b['records'][0]['text']);obj['evidence_ids']=['article-1'];b['records'][0]['text']=json.dumps(obj)
        self.assertEqual(diagnostics(b['analysis_input'],b['records'])['valid_responses'],1)

if __name__=='__main__':unittest.main()
