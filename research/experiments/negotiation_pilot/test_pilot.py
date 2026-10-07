import json
from pathlib import Path
import tempfile
import unittest

from execution import execute, evaluate
from providers import Ledger, BudgetStop
from tasks import replay, schedule, flow, cases


class GraderTests(unittest.TestCase):
    def test_replay_concurrent_remove_and_ancestry(self):
        a=dict(id='a',kind='add',key='x',deps=[])
        b=dict(id='b',kind='add',key='x',deps=[])
        r=dict(id='r',kind='remove',key='x',deps=['a'],targets=['a','b'])
        self.assertEqual(replay({'events':[r,a,b,a]}),dict(eligible=['a','b','r'],live=['b']))
        conflict=dict(a,key='y')
        self.assertEqual(replay({'events':[r,a,b,conflict]}),dict(eligible=['b'],live=['b']))

    def test_replay_cycles_missing_and_wrong_key(self):
        events=[dict(id='a',kind='add',key='x',deps=['b']),dict(id='b',kind='add',key='x',deps=['a']),
                dict(id='c',kind='add',key='x',deps=[]),dict(id='d',kind='remove',key='y',deps=['c'],targets=['c']),
                dict(id='e',kind='add',key='x',deps=['absent'])]
        self.assertEqual(replay({'events':events}),dict(eligible=['c','d'],live=['c']))

    def test_schedule_blackouts_and_global_value(self):
        job=lambda i,v,d:dict(id=i,value=v,duration=d,release=0,deadline=10,family=0,deps=[])
        data=dict(jobs=[job('a',9,4),job('b',6,2),job('c',6,2)],setup=[[0]*3 for _ in range(3)],blackouts=[[4,6],[3,5]])
        self.assertEqual(schedule(data),['b','a'])
        data['jobs'][0]['deps']=['absent']
        self.assertEqual(schedule(data),['b','c'])

    def test_schedule_lex_tie_after_release(self):
        def j(i,d,r=0):return dict(id=i,value=1,duration=d,release=r,deadline=20,family=0,deps=[])
        data=dict(jobs=[j('a',3),j('b',1),j('c',1,10)],setup=[[0]*3 for _ in range(3)],blackouts=[])
        self.assertEqual(schedule(data),['a','b','c'])

    def test_flow_negative_disconnected_cycle_lower_and_tie(self):
        edge=lambda u,v,l,h,c:dict(u=u,v=v,lower=l,upper=h,cost=c)
        data=dict(n=4,balance=[1,-1,0,0],edges=[edge(0,1,0,2,1),edge(2,3,0,2,-2),edge(3,2,0,2,1)])
        self.assertEqual(flow(data),[1,2,2])
        data=dict(n=2,balance=[1,-1],edges=[edge(0,1,0,1,0),edge(0,1,0,1,0)])
        self.assertEqual(flow(data),[0,1])
        data['edges'][0]['lower']=1
        self.assertEqual(flow(data),[1,0])
        self.assertIsNone(flow(dict(n=1,balance=[1],edges=[])))
        self.assertEqual(flow(dict(n=1,balance=[0],edges=[])),[])

    def test_fixtures_are_frozen_and_separate(self):
        for name in ('replay','schedule','flow'):
            self.assertEqual(cases(name),cases(name))
            self.assertFalse(any(c in cases(name,True) for c in cases(name)))


class BoundaryTests(unittest.TestCase):
    def test_pending_usage_is_charged_and_budget_fails_closed(self):
        with tempfile.TemporaryDirectory() as d:
            path=Path(d)/'ledger.json'
            ledger=Ledger(path,1)
            ledger.reserve('trial','model',.8)
            self.assertAlmostEqual(Ledger(path,1).spent(),.8)
            with self.assertRaises(BudgetStop):ledger.reserve('trial','model',.21)
            self.assertEqual(len(json.loads(path.read_text())),1)

    def test_settled_usage_releases_unused_reservation(self):
        with tempfile.TemporaryDirectory() as d:
            ledger=Ledger(Path(d)/'ledger.json',1)
            entry=ledger.reserve('trial','model',.8)
            ledger.settle(entry,dict(usage=dict(input_tokens=100,output_tokens=10)), 'b')
            self.assertAlmostEqual(ledger.spent(),.0015)

    def test_execution_denies_home_and_network(self):
        result=execute('def solve(x):\n return open("/Users/andrei/.zshrc").read()',[None])
        self.assertIn('PermissionError',result[0]['error'])
        result=execute('import socket\ndef solve(x):\n return socket.create_connection(("127.0.0.1",80))',[None])
        self.assertIn('PermissionError',result[0]['error'])
        result=execute('import os\ndef solve(x): return sorted(os.environ)',[None])
        self.assertNotIn('OPENAI_API_KEY',result[0]['value'])
        self.assertNotIn('ANTHROPIC_API_KEY',result[0]['value'])

    def test_evaluator_rejects_wrong_and_accepts_right(self):
        fixtures=[dict(input=1,expected=2),dict(input=3,expected=4)]
        self.assertEqual(evaluate('def solve(x): return x+1',fixtures)['passed'],2)
        self.assertEqual(evaluate('def solve(x): return 2',fixtures)['passed'],1)
        self.assertEqual(evaluate('def solve(x): raise ValueError("oops")',fixtures)['passed'],0)

class PartialResponseTests(unittest.TestCase):
    def test_incomplete_tool_arguments_are_not_executed_or_replayed(self):
        from unittest.mock import patch
        from providers import Session
        response=dict(id='mock',usage=dict(input_tokens=10,output_tokens=10),status='incomplete',
                      output=[dict(type='function_call',call_id='call',name='submit',arguments='{"source":"partial')])
        with tempfile.TemporaryDirectory() as d:
            ledger=Ledger(Path(d)/'ledger.json',1)
            session=Session('a','test',[],ledger,'trial')
            session.user('test')
            with patch('providers.post',side_effect=[{'input_tokens':10},response]):
                calls,text,stop=session.request(.1)
            self.assertEqual(calls,[])
            self.assertIn('Incomplete tool call discarded',text[0])
            self.assertEqual(stop,'incomplete')
            self.assertEqual(session.history,[dict(role='user',content='test')])
            self.assertGreater(ledger.spent(),0)


if __name__=='__main__':unittest.main()
