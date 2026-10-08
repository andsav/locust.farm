"""Durable bounded paid calls. One ledger for this study, no automatic retries."""
from collections import deque
from contextlib import contextmanager
from datetime import datetime, timezone
import json
from pathlib import Path
import sqlite3
import threading
import time
import os
import urllib.error
import urllib.request

from protocol import digest, parse


def now():
    return datetime.now(timezone.utc).isoformat()


def atomic_json(path,value):
    path=Path(path);temp=path.with_suffix('.tmp')
    with temp.open('w') as f:
        json.dump(value,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
    temp.replace(path)


class BudgetExceeded(Exception):
    pass


class Ledger:
    def __init__(self,path,study_ceiling):
        self.path=Path(path);self.ceiling=study_ceiling
        self.path.parent.mkdir(parents=True,exist_ok=True)
        with self.connect() as db:
            db.execute('CREATE TABLE IF NOT EXISTS settings (name TEXT PRIMARY KEY, value REAL NOT NULL)')
            db.execute('INSERT OR IGNORE INTO settings VALUES (?,?)',('study_ceiling',study_ceiling))
            if db.execute('SELECT value FROM settings WHERE name=?',('study_ceiling',)).fetchone()[0]!=study_ceiling:
                raise ValueError('Study ceiling differs from existing ledger')
            db.execute('''CREATE TABLE IF NOT EXISTS calls (
                key TEXT PRIMARY KEY, run_id TEXT NOT NULL, job_id TEXT NOT NULL,
                case_id TEXT NOT NULL, config TEXT NOT NULL,
                reserved REAL NOT NULL, cost REAL, status TEXT NOT NULL)''')
    @contextmanager
    def connect(self):
        db=sqlite3.connect(self.path,timeout=30)
        try:
            db.execute('PRAGMA synchronous=FULL')
            with db:
                yield db
        finally:
            db.close()
    def reserve(self,run_id,job,reserve,phase_ceiling,case_ceiling):
        key=run_id+'/'+job['id']
        with self.connect() as db:
            db.execute('BEGIN IMMEDIATE')
            if db.execute('SELECT key FROM calls WHERE key=?',(key,)).fetchone():
                raise ValueError('Paid attempt already reserved; refusing duplicate dispatch')
            total=db.execute('SELECT COALESCE(SUM(COALESCE(cost,reserved)),0) FROM calls').fetchone()[0]
            phase=db.execute('SELECT COALESCE(SUM(COALESCE(cost,reserved)),0) FROM calls WHERE run_id=?',(run_id,)).fetchone()[0]
            if total+reserve>self.ceiling or phase+reserve>phase_ceiling:
                raise BudgetExceeded('Study or phase ceiling reached')
            config=job['config'];budget=config.rsplit('_',1)[-1]
            owners=[f'D_{budget}',f'E_{budget}'] if config.startswith('G_') else [config]
            for owner in owners:
                configs=[owner,f'G_{budget}'] if owner.startswith(('D_','E_')) else [owner]
                placeholders=','.join('?' for _ in configs)
                case=db.execute(f'SELECT COALESCE(SUM(COALESCE(cost,reserved)),0) FROM calls WHERE run_id=? AND case_id=? AND config IN ({placeholders})',(run_id,job['case_id'],*configs)).fetchone()[0]
                if case+reserve>case_ceiling:
                    raise BudgetExceeded('Case/arm ceiling reached')
            db.execute('INSERT INTO calls VALUES (?,?,?,?,?,?,?,?)',
                       (key,run_id,job['id'],job['case_id'],config,reserve,None,'pending'))
        return key
    def finish(self,key,status,cost=None):
        with self.connect() as db:
            db.execute('UPDATE calls SET status=?,cost=? WHERE key=?',(status,cost,key))
    def entries(self,run_id):
        with self.connect() as db:
            db.row_factory=sqlite3.Row
            return {r['job_id']:dict(r) for r in db.execute('SELECT * FROM calls WHERE run_id=?',(run_id,))}
    def total(self):
        with self.connect() as db:
            return db.execute('SELECT COALESCE(SUM(COALESCE(cost,reserved)),0) FROM calls').fetchone()[0]


class Transport:
    def __init__(self,folder,manifest,ledger,opener=None):
        self.folder=Path(folder);self.manifest=manifest;self.ledger=ledger
        self.records_dir=self.folder/'records';self.records_dir.mkdir(exist_ok=True)
        self.stop=threading.Event();self.lock=threading.Lock();self.recent=deque(maxlen=50)
        self.opener=opener or urllib.request.urlopen
        self.stop_reason=None
    def record_path(self,job_id):
        return self.records_dir/(digest(job_id)+'.json')
    def halt(self,reason):
        with self.lock:
            self.stop_reason=reason;self.stop.set()
            atomic_json(self.folder/'stop.json',{'at':now(),'reason':reason})
    def invoke(self,job,body,allowed_ids):
        path=self.record_path(job['id'])
        if path.exists():
            raise ValueError('Existing record must be handled by scheduler, not redispatched')
        encoded=json.dumps(body).encode()
        # UTF-8 bytes bound input tokens conservatively; add fixed protocol overhead.
        input_bound=len(encoded)+4096
        rates=self.manifest['rates']
        reserve=(input_bound*rates['input']+job['cap']*rates['output'])/1e6
        item={'job':job,'request':body,'request_sha256':digest(body),'started_at':now(),
              'status':'pending','reserved_usd':reserve,'input_bound':input_bound}
        if self.stop.is_set():
            return None
        try:
            key=self.ledger.reserve(self.manifest['run_id'],job,reserve,self.manifest['phase_ceiling_usd'],self.manifest['case_ceiling_usd'])
        except BudgetExceeded as error:
            self.halt(str(error));return None
        # Ledger reservation is durable before both the record and network call.
        atomic_json(path,item)
        started=time.monotonic();failed=False
        try:
            req=urllib.request.Request('https://api.openai.com/v1/responses',data=encoded,
                headers={'Content-Type':'application/json','Authorization':'Bearer '+os.environ['OPENAI_API_KEY']})
            with self.opener(req,timeout=self.manifest['timeout_seconds']) as response:
                value=json.load(response)
            usage=value['usage'];incoming=usage['input_tokens'];outgoing=usage['output_tokens']
            if type(incoming) is not int or type(outgoing) is not int or min(incoming,outgoing)<0:
                raise ValueError('Invalid provider usage')
            cost=(incoming*rates['input']+outgoing*rates['output'])/1e6
            text='\n'.join(c['text'] for o in value.get('output',[]) if o.get('type')=='message'
                           for c in o.get('content',[]) if c.get('type')=='output_text')
            status=value.get('status','unknown')
            item.update(status=status,text=text,usage=usage,cost_usd=cost,
                        response_id=value.get('id'),returned_model=value.get('model'),
                        service_tier=value.get('service_tier'),incomplete_details=value.get('incomplete_details'),
                        parsed=parse(text,allowed_ids) if status=='completed' else None)
            if incoming>input_bound or outgoing>job['cap'] or cost>reserve:
                item['accounting_error']='Usage exceeded reservation';self.halt(item['accounting_error'])
            if value.get('service_tier') not in (None,'default') or value.get('model')!=self.manifest['model']:
                item['provider_error']='Unexpected model or service tier';self.halt(item['provider_error'])
        except Exception as error:
            failed=True
            item.update(status='transport_error',error_type=type(error).__name__,parsed=None)
            if isinstance(error,urllib.error.HTTPError):
                item['http_status']=error.code
                # Do not record provider response bodies or request headers.
                self.halt('HTTP '+str(error.code)+'; inspect before resuming unattempted jobs')
        finally:
            item.update(finished_at=now(),elapsed_seconds=time.monotonic()-started)
            atomic_json(path,item)
            self.ledger.finish(key,item['status'],item.get('cost_usd'))
            with self.lock:
                self.recent.append(failed)
                too_many=(len(self.recent)>=3 and all(list(self.recent)[-3:])) or sum(self.recent)>=5
            if too_many:
                self.halt('Transport failure threshold reached; no paid attempt will be retried')
        return item
