"""Prepare, inspect, dry-run or execute the isolated Nous transfer experiment."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import date
import fcntl
import json
from pathlib import Path
import random
import sys
import threading

from protocol import (BRANCHES, CONDITIONS, N_AGENTS, analyze, code_hashes, digest,
                      label, prepare, request_plan)
# Reuse the prior experiment's transport, never its study design or analysis.
sys.path.insert(0,str(Path(__file__).resolve().parent.parent/'luna_decision_pilot'))
from run import Runner, INPUT_RATE, OUTPUT_RATE


class Experiment(Runner):
    def __init__(self, folder, simulated=False):
        self.folder=Path(folder)
        self.frozen=json.loads((self.folder/'manifest.json').read_text())
        if (self.folder/'manifest.sha256').read_text().strip()!=digest(self.frozen):
            raise ValueError('Frozen manifest hash mismatch')
        if self.frozen['model']!='gpt-6-luna':
            raise ValueError('This transport price policy is fixed to gpt-6-luna')
        if self.frozen['code_hashes'] != code_hashes():
            raise ValueError('Runner code differs from frozen manifest; prepare a new run')
        if (self.frozen['input_rate_per_million'],self.frozen['output_rate_per_million']) != (INPUT_RATE,OUTPUT_RATE):
            raise ValueError('Price assumptions differ from bounded transport')
        self.records_path=self.folder/'records.json'
        self.records=json.loads(self.records_path.read_text()) if self.records_path.exists() else []
        self.lock=threading.Lock();self.stop=threading.Event();self.simulated=simulated
        if any(bool(r.get('simulated')) != simulated for r in self.records):
            raise ValueError('Never mix dry-run and live records')
        self.verify_records()
        if any(r['status'] in ('pending','transport_error') or r.get('budget_error') for r in self.records):
            raise ValueError('Ambiguous or failed paid attempt retained; inspect before any further run')

    def body(self,instructions,public):
        return dict(model=self.frozen['model'],instructions=instructions,input=json.dumps(public,sort_keys=True),
                    reasoning={'effort':self.frozen['effort']},max_output_tokens=self.frozen['max_output_tokens'],service_tier='default',store=False)

    def verify_records(self):
        records={r['label']:r for r in self.records}
        if len(records)!=len(self.records):raise ValueError('Duplicate records')
        cases={c['id']:c for c in self.frozen['cases']}
        for r in self.records:
            qid,condition,stage,agent=r['label'].split('/');agent=int(agent)
            if condition not in CONDITIONS or stage not in ('initial',*BRANCHES) or not 0<=agent<N_AGENTS:
                raise ValueError('Unexpected record identity')
            plan=request_plan(self.frozen,records,cases[qid],condition,stage,agent)
            if plan is None or digest(self.body(*plan))!=r['request_sha256']:
                raise ValueError('Recorded request differs from frozen inputs')

    def call(self,case,condition,stage,agent):
        if self.stop.is_set():return
        if not self.simulated and self.frozen['phase']=='prospective' and date.today()>=date.fromisoformat(case['public']['resolution_date']):
            self.stop.set();return
        name=label(case['id'],condition,stage,agent)
        with self.lock:records={r['label']:r for r in self.records}
        if name in records:return
        plan=request_plan(self.frozen,records,case,condition,stage,agent)
        if plan is None:return
        if len(json.dumps(self.body(*plan)).encode())+4096>272000:
            self.stop.set()
            raise ValueError('Conservative input bound crosses the standard pricing tier')
        if self.simulated:
            # Deterministic plumbing fixture, never an estimate of model quality.
            probability=(int(digest(name)[:8],16)%999+1)/1000
            r={'label':name,'status':'completed','simulated':True,'request_sha256':digest(self.body(*plan)),
               'text':json.dumps({'probability':probability,'evidence_ids':[case['public']['evidence'][0]['id']],'explanation':'SIMULATED plumbing fixture','used_peer_ids':[]}),
               'reserved_usd':0,'cost_usd':0,'condition':condition,'stage':stage,'agent':agent,'case_id':case['id']}
            with self.lock:self.records.append(r);self.save()
        else:
            self.invoke(name,*plan,condition=condition,stage=stage,agent=agent,case_id=case['id'])

    def run_case(self,case):
        conditions=list(CONDITIONS);random.Random(digest([self.frozen['seed'],case['id'],'conditions'])).shuffle(conditions)
        for condition in conditions:
            for agent in range(N_AGENTS):self.call(case,condition,'initial',agent)
            branches=list(BRANCHES);random.Random(digest([self.frozen['seed'],case['id'],condition])).shuffle(branches)
            for stage in branches:
                for agent in range(N_AGENTS):self.call(case,condition,stage,agent)

    def run(self):
        with ThreadPoolExecutor(max_workers=4) as pool:
            list(pool.map(self.run_case,self.frozen['cases']))
        self.verify_records()
        result=analyze(self.frozen,self.records)
        result['simulated']=self.simulated
        (self.folder/'summary.json').write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
        return {k:result[k] for k in ('phase','simulated','planned_calls','attempted_calls','completed_calls','accounted_usd')}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    sub=parser.add_subparsers(dest='command',required=True)
    p=sub.add_parser('prepare');p.add_argument('folder',type=Path);p.add_argument('--upstream',type=Path,required=True)
    p.add_argument('--phase',choices=['smoke','replay','prospective'],default='smoke')
    p.add_argument('--cohort',type=Path);p.add_argument('--as-of',type=date.fromisoformat,required=True);p.add_argument('--ceiling-usd',type=float,default=10)
    for name in ('inspect','dry-run','run','analyze'):
        p=sub.add_parser(name);p.add_argument('folder',type=Path)
        if name=='analyze':p.add_argument('--resolutions',type=Path)
    args=parser.parse_args()
    if args.command=='prepare':
        print(json.dumps(prepare(args.upstream,args.folder,args.phase,args.as_of,args.ceiling_usd,args.cohort),indent=2));return
    if args.command=='analyze':
        # Analysis stays available after transport failures or a runner-code update.
        manifest=json.loads((args.folder/'manifest.json').read_text())
        if digest(manifest)!=(args.folder/'manifest.sha256').read_text().strip():raise ValueError('Manifest hash mismatch')
        resolution_hash=None
        if args.resolutions:
            if manifest['phase']!='prospective':raise ValueError('Only prospective runs accept later resolutions')
            resolved=json.loads(args.resolutions.read_text());resolution_hash=digest(resolved)
            if set(resolved)-{c['id'] for c in manifest['cases']}:raise ValueError('Unexpected resolution ID')
            for case in manifest['cases']:
                if case['id'] not in resolved:continue
                r=resolved[case['id']]
                if set(r)!={'outcome','resolved_at','source_url'} or type(r['outcome']) is not int or r['outcome'] not in (0,1):
                    raise ValueError('Resolution schema mismatch')
                if date.fromisoformat(r['resolved_at'])<date.fromisoformat(case['public']['information_cutoff']) or not r['source_url'].startswith(('https://','http://')):
                    raise ValueError('Invalid resolution provenance')
                case['outcome']=r['outcome']
        records=json.loads((args.folder/'records.json').read_text()) if (args.folder/'records.json').exists() else []
        result=analyze(manifest,records);result['resolution_sha256']=resolution_hash;result['simulated']=any(r.get('simulated') for r in records)
        print(json.dumps(result,indent=2,allow_nan=False));return
    folder=args.folder
    if args.command=='dry-run':
        folder=folder/'dry-run';folder.mkdir(exist_ok=False)
        for filename in ('manifest.json','manifest.sha256'):(folder/filename).write_bytes((args.folder/filename).read_bytes())
    with (folder/'run.lock').open('w') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        runner=Experiment(folder,simulated=args.command=='dry-run')
        if args.command=='inspect':
            case=runner.frozen['cases'][0]
            plan=request_plan(runner.frozen,{},case,'published','initial',0)
            print(json.dumps({'manifest_sha256':digest(runner.frozen),'cases':len(runner.frozen['cases']),
                'planned_calls':len(runner.frozen['cases'])*90,'example_request_sha256':digest(runner.body(*plan)),
                'length_control':runner.frozen['length_control']},indent=2))
        else:
            if args.command=='run' and runner.frozen['phase']=='prospective' and any(date.today()>=date.fromisoformat(c['public']['resolution_date']) for c in runner.frozen['cases']):
                raise ValueError('Prospective generation must finish before declared resolution dates')
            print(json.dumps(runner.run(),indent=2))

if __name__=='__main__':main()
