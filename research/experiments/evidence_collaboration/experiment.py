"""Prepare, run, analyze, and verify the bounded Luna evidence study."""
import argparse
from concurrent.futures import ThreadPoolExecutor, wait, FIRST_COMPLETED
import fcntl
import heapq
import json
import os
from pathlib import Path
import shutil
import sys
import time

from protocol import (MODEL,EFFORT,SEED,INPUT_RATE,OUTPUT_RATE,PROMPTS,configurations,
                      digest,file_hash,case_key,job_graph,make_body,parse,make_cohort)
from transport import Ledger,Transport,atomic_json,now
from analysis import analyze,readiness,technically_ready

SOURCE_FILES=('protocol.py','transport.py','analysis.py','experiment.py')


def prepare(root,name,phase,budgets=(6000,12000),selection=None,development_summary=None):
    root=Path(root).resolve();folder=root/'runs'/name
    if folder.exists():raise ValueError('Run folder exists; refusing replacement')
    cohort=json.loads((root/'cohort.json').read_text())
    m={'version':1,'run_id':name,'phase':phase,'model':MODEL,'effort':EFFORT,'seed':SEED,
       'prompts':PROMPTS,'rates':{'input':INPUT_RATE,'output':OUTPUT_RATE},
       'study_ceiling_usd':250.,'phase_ceiling_usd':60. if phase=='development' else 150. if phase=='main' else 30.,
       'case_ceiling_usd':.25,'timeout_seconds':300,'repetition':int(phase=='repeat'),
       'cohort_sha256':digest(cohort),'prepared_at':now(),
       'failure_policy':'No retries. Stop on HTTP errors, three consecutive transport failures, or five in last 50 calls.',
       'source_hashes':{name:file_hash(Path(__file__).parent/name) for name in SOURCE_FILES}}
    if phase=='development':
        m.update(configs=configurations(budgets),family_ids=cohort['development_ids'])
    else:
        if not selection or not selection['ready'] or not development_summary:
            raise ValueError('Main/repeat needs a passed development selection')
        if readiness(development_summary,selection['exchange'],selection['independent'],selection['divided'])!=selection:
            raise ValueError('Selection does not reproduce from development summary')
        lookup=development_summary['configs']
        solo=sorted([(k,c) for k,c in lookup.items() if c['arm'].startswith('S_') and technically_ready(c)],
                    key=lambda x:(-x[1]['selection_rate'],x[1]['mean_case_usd'],x[0]))
        if not solo:raise ValueError('No technically valid solo configuration')
        chosen=[solo[0][0],selection['independent'],selection['divided'],selection['exchange'],selection['baseline']]
        m.update(configs=[{'name':k,'arm':lookup[k]['arm'],'budget':lookup[k]['budget']} for k in dict.fromkeys(chosen)],
                 family_ids=cohort['main_ids'] if phase=='main' else cohort['repeat_ids'],
                 baseline=selection['baseline'],exchange=selection['exchange'],independent=selection['independent'],
                 divided=selection['divided'],solo=solo[0][0],selection=selection,
                 development_summary_sha256=digest(development_summary))
        a=lookup[selection['exchange']]['families'];b=lookup[selection['baseline']]['families']
        weights={h:n/sum(cohort['main_strata'].values()) for h,n in cohort['main_strata'].items()}
        q=sum(weights[h]*sum(a[k]!=b[k] for k in a if k[0]==h)/sum(k[0]==h for k in a) for h in weights)
        m['power']={'development_discordance':q,'main_n':len(cohort['main_ids']),
                    'approx_detectable_difference_80pct':((1.96+.84)**2*q/len(cohort['main_ids']))**.5,
                    'target_difference':.08}
    folder.mkdir(parents=True)
    (folder/'source').mkdir()
    for name in SOURCE_FILES:shutil.copyfile(Path(__file__).parent/name,folder/'source'/name)
    atomic_json(folder/'manifest.json',m)
    (folder/'manifest.sha256').write_text(digest(m)+'\n')
    return folder,m


def load(folder):
    folder=Path(folder).resolve();root=folder.parent.parent
    m=json.loads((folder/'manifest.json').read_text())
    if digest(m)!=(folder/'manifest.sha256').read_text().strip():raise ValueError('Manifest changed')
    for name,sha in m['source_hashes'].items():
        if file_hash(folder/'source'/name)!=sha:raise ValueError('Frozen source changed: '+name)
    cohort=json.loads((root/'cohort.json').read_text())
    if digest(cohort)!=m['cohort_sha256']:raise ValueError('Cohort changed')
    return root,m,cohort


def compact(record):
    return {k:v for k,v in record.items() if k not in ('request','text')}


def load_records(folder):
    records={}
    for path in sorted((Path(folder)/'records').glob('*.json')):
        value=json.loads(path.read_text());key=value['job']['id']
        if key in records:raise ValueError('Duplicate job record')
        records[key]=compact(value)
    return records


class Experiment:
    def __init__(self,folder,transport_class=Transport):
        self.folder=Path(folder).resolve();self.root,self.manifest,self.cohort=load(folder)
        self.ledger=Ledger(self.root/'budget.sqlite',self.manifest['study_ceiling_usd'])
        self.transport=transport_class(self.folder,self.manifest,self.ledger)
        self.records=load_records(folder)
        self.rows={case_key(key,row['answerable'],self.manifest['repetition']):row
                   for key in self.manifest['family_ids'] for row in self.cohort['families'][key]['rows']}
        self.jobs={k:j for case in self.rows for k,j in job_graph(case,self.manifest['configs']).items()}
        if set(self.records)-set(self.jobs):raise ValueError('Records outside frozen graph')
        # An interrupted reservation is never interpreted as permission to retry.
        for key,entry in self.ledger.entries(self.manifest['run_id']).items():
            if key not in self.jobs:raise ValueError('Ledger entry outside frozen graph')
            existing=self.records.get(key)
            if existing is None or existing['status']=='pending':
                original=json.loads(self.transport.record_path(key).read_text()) if existing else {}
                original.update(job=self.jobs[key],status='interrupted',parsed=None,reserved_usd=entry['reserved'],
                                error_type='AmbiguousPriorAttempt',finished_at=now())
                atomic_json(self.transport.record_path(key),original);self.records[key]=compact(original)
                self.ledger.finish(entry['key'],'interrupted')
            elif entry['status']=='pending':
                self.ledger.finish(entry['key'],existing['status'],existing.get('cost_usd'))
        for key,record in self.records.items():
            if record.get('request_sha256') and all(dep in self.records and self.records[dep].get('parsed') for dep in self.jobs[key]['deps']):
                body=make_body(self.jobs[key],self.rows[self.jobs[key]['case_id']],self.records,self.manifest,self.manifest['repetition'])
                if digest(body)!=record['request_sha256']:raise ValueError('Recorded request differs from frozen graph')

    def run(self,workers=12,first_families=None,resume_after_stop=False):
        if (self.folder/'stop.json').exists() and not resume_after_stop:
            raise ValueError('Inspect stop.json; explicit --resume-after-stop only resumes unattempted jobs')
        selected=set(self.manifest['family_ids'][:first_families] if first_families else self.manifest['family_ids'])
        jobs={k:j for k,j in self.jobs.items() if j['case_id'].split('/')[0] in selected}
        pending=set(jobs)-set(self.records);children={k:[] for k in jobs};degree={};ready=[]
        for key in pending:
            degree[key]=sum(dep not in self.records for dep in jobs[key]['deps'])
            for dep in jobs[key]['deps']:children[dep].append(key)
            if degree[key]==0:heapq.heappush(ready,(digest([self.manifest['seed'],key]),key))
        started=time.monotonic();last=0
        def finished(key,record):
            self.records[key]=compact(record)
            for child in children[key]:
                if child not in pending:continue
                degree[child]-=1
                if degree[child]==0:heapq.heappush(ready,(digest([self.manifest['seed'],child]),child))
        def progress():
            subset=[r for k,r in self.records.items() if k in jobs]
            value={'at':now(),'phase':self.manifest['phase'],'selected_families':len(selected),
                   'resolved_jobs':len(subset),'selected_jobs':len(jobs),'all_planned_jobs':len(self.jobs),
                   'valid_calls':sum(r['status']=='completed' and bool(r.get('parsed')) for r in subset),
                   'status_counts':{s:sum(r['status']==s for r in subset) for s in {r['status'] for r in subset}},
                   'accounted_study_usd':self.ledger.total(),'elapsed_seconds':time.monotonic()-started,
                   'stop_reason':self.transport.stop_reason}
            atomic_json(self.folder/'progress.json',value);print(json.dumps(value),flush=True)
        progress()
        with ThreadPoolExecutor(max_workers=workers) as pool:
            futures={}
            while pending or futures:
                while ready and len(futures)<workers and not self.transport.stop.is_set():
                    _,key=heapq.heappop(ready)
                    if key not in pending:continue
                    job=jobs[key];pending.remove(key)
                    failed=[dep for dep in job['deps'] if self.records[dep]['status']!='completed' or not self.records[dep].get('parsed')]
                    if failed:
                        item={'job':job,'status':'dependency_failed','parsed':None,'failed_dependencies':failed,'finished_at':now()}
                        atomic_json(self.transport.record_path(key),item);finished(key,item);continue
                    row=self.rows[job['case_id']]
                    body=make_body(job,row,self.records,self.manifest,self.manifest['repetition'])
                    future=pool.submit(self.transport.invoke,job,body,{p['idx'] for p in row['paragraphs']})
                    futures[future]=key
                if not futures:
                    if self.transport.stop.is_set():break
                    if pending and not ready:raise ValueError('Request graph cannot progress')
                    if not pending:break
                    continue
                done,_=wait(futures,timeout=10,return_when=FIRST_COMPLETED)
                for future in done:
                    key=futures.pop(future)
                    try:record=future.result()
                    except Exception as error:
                        self.transport.halt('Internal scheduler/transport error: '+type(error).__name__)
                        raise
                    if record is None:
                        pending.add(key)
                    else:finished(key,record)
                if time.monotonic()-last>=30:
                    progress();last=time.monotonic()
        progress()
        result=analyze(self.manifest,self.cohort,self.records,
                       inference=self.manifest['phase'] in ('main','repeat') and len(self.records)==len(self.jobs))
        atomic_json(self.folder/'summary.json',result)
        return result


def verify(folder):
    root,m,cohort=load(folder);records=load_records(folder)
    rows={case_key(key,row['answerable'],m['repetition']):row for key in m['family_ids'] for row in cohort['families'][key]['rows']}
    jobs={k:j for case in rows for k,j in job_graph(case,m['configs']).items()}
    for path in (Path(folder)/'records').glob('*.json'):
        r=json.loads(path.read_text());job=r['job'];key=job['id']
        assert job==jobs[key]
        if r.get('request') is not None:
            expected=make_body(job,rows[job['case_id']],records,m,m['repetition'])
            assert r['request']==expected and digest(expected)==r['request_sha256']
        if 'text' in r:
            assert r['parsed']==(parse(r['text'],{p['idx'] for p in rows[job['case_id']]['paragraphs']}) if r['status']=='completed' else None)
        if 'usage' in r:
            assert r['returned_model']==m['model'] and r['service_tier'] in (None,'default')
            u=r['usage'];expected=(u['input_tokens']*m['rates']['input']+u['output_tokens']*m['rates']['output'])/1e6
            assert abs(expected-r['cost_usd'])<1e-12 and r['cost_usd']<=r['reserved_usd']
    recomputed=analyze(m,cohort,records,inference=m['phase'] in ('main','repeat') and len(records)==len(jobs))
    if (Path(folder)/'summary.json').exists():assert recomputed==json.loads((Path(folder)/'summary.json').read_text())
    return {'verified_records':len(records),'planned_jobs':len(jobs),'manifest_sha256':digest(m)}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);sub=p.add_subparsers(dest='command',required=True)
    cohort_cmd=sub.add_parser('cohort');cohort_cmd.add_argument('root',type=Path);cohort_cmd.add_argument('--calibration',type=Path,required=True)
    prep=sub.add_parser('prepare');prep.add_argument('root',type=Path);prep.add_argument('name');prep.add_argument('--phase',choices=['development','main','repeat'],default='development')
    prep.add_argument('--budgets',default='6000,12000');prep.add_argument('--development-summary',type=Path);prep.add_argument('--selection',type=Path)
    for action in ('run','analyze','verify','gate'):
        a=sub.add_parser(action);a.add_argument('folder',type=Path)
        if action=='run':
            a.add_argument('--workers',type=int,default=12);a.add_argument('--first-families',type=int);a.add_argument('--resume-after-stop',action='store_true')
        if action=='gate':a.add_argument('--budget',type=int,default=12000)
    a=p.parse_args()
    if a.command=='cohort':
        dest=a.root/'cohort.json'
        if dest.exists():raise ValueError('Cohort already exists; refusing overwrite')
        value=make_cohort(a.root/'data',a.calibration);atomic_json(dest,value)
        print(json.dumps({'sha256':digest(value),'development':len(value['development_ids']),'main':len(value['main_ids']),'strata':value['main_strata']}))
    elif a.command=='prepare':
        folder,m=prepare(a.root,a.name,a.phase,tuple(map(int,a.budgets.split(','))),
                         json.loads(a.selection.read_text()) if a.selection else None,
                         json.loads(a.development_summary.read_text()) if a.development_summary else None)
        print(json.dumps({'folder':str(folder),'manifest_sha256':digest(m),'families':len(m['family_ids']),'configs':m['configs']}))
    elif a.command=='run':
        if not 1<=a.workers<=32:raise ValueError('Workers must be 1-32')
        # Execute the snapshotted implementation, not mutable checkout source.
        frozen=a.folder.resolve()/'source'/'experiment.py'
        if Path(__file__).resolve()!=frozen:
            os.execv(sys.executable,[sys.executable,str(frozen),*sys.argv[1:]])
        with (a.folder/'run.lock').open('w') as lock:
            fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
            result=Experiment(a.folder).run(a.workers,a.first_families,a.resume_after_stop)
            print(json.dumps({k:v for k,v in result.items() if k!='configs'}),flush=True)
    elif a.command=='verify':print(json.dumps(verify(a.folder)))
    elif a.command=='analyze':
        _,m,c=load(a.folder);r=load_records(a.folder)
        result=analyze(m,c,r,inference=m['phase'] in ('main','repeat') and len(r)==sum(len(job_graph(case_key(k,row['answerable'],m['repetition']),m['configs'])) for k in m['family_ids'] for row in c['families'][k]['rows']))
        atomic_json(a.folder/'summary.json',result)
        print(json.dumps({k:v for k,v in result.items() if k!='configs'}))
    else:
        summary=json.loads((a.folder/'summary.json').read_text())
        result=readiness(summary,f'E_{a.budget}',f'I_{a.budget}',f'D_{a.budget}')
        atomic_json(a.folder/'selection.json',result);print(json.dumps(result))
