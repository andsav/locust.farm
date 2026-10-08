"""Resume a historical Nous run with an append-only ledger of bounded retries."""
import argparse
from datetime import datetime, timezone
import fcntl
import json
from pathlib import Path
import threading

from evidence import verify
from experiment import Experiment, Runner
from protocol import code_hashes, digest, file_hash, label, request_plan


def selected_records(attempts):
    selected={}
    for attempt in attempts:
        name=attempt.get('logical_label',attempt['label'])
        previous=selected.get(name)
        if previous is None or previous['status']!='completed':
            selected[name]={**attempt,'label':name}
    return list(selected.values())


class Recovery(Experiment):
    @property
    def records(self):
        return selected_records(self.attempts)

    def __init__(self, original, folder, evidence):
        self.folder=folder
        self.frozen=json.loads((original/'manifest.json').read_text())
        baseline=json.loads(evidence.read_text())
        if digest(self.frozen)!=baseline['run_manifest_sha256']:
            raise ValueError('Original manifest differs from verified evidence')
        if json.loads((original/'records.json').read_text())!=baseline['records']:
            raise ValueError('Original records differ from verified evidence')
        if self.frozen['phase'] not in ('smoke','replay'):
            raise ValueError('Recovery currently supports historical cohorts only')
        self.lock=threading.Lock();self.stop=threading.Event();self.simulated=False
        policy={'original_manifest_sha256':digest(self.frozen),'baseline_evidence_sha256':file_hash(evidence),
                'code_hashes':code_hashes(),'recovery_code_sha256':file_hash(Path(__file__)),
                'maximum_attempts_per_request':4,'retry_delays_seconds':[5,15,30],
                'selection':'first completed response, including invalid-schema responses; otherwise latest attempt',
                'retryable':'transport_error only; never schema failures or completed forecasts',
                'ceiling_usd':self.frozen['ceiling_usd']}
        if folder.exists():
            saved=json.loads((folder/'amendment.json').read_text())
            if saved['policy']!=policy:raise ValueError('Recovery policy or code changed')
            self.attempts=json.loads((folder/'attempts.json').read_text())
            if self.attempts[:len(baseline['records'])]!=baseline['records']:
                raise ValueError('Baseline attempts changed')
        else:
            folder.mkdir(parents=True)
            self.attempts=baseline['records']
            for name in ('manifest.json','manifest.sha256'):
                (folder/name).write_bytes((original/name).read_bytes())
            (folder/'amendment.json').write_text(json.dumps({'created_at':datetime.now(timezone.utc).isoformat(),'policy':policy},indent=2)+'\n')
            (folder/'attempts.json').write_text(json.dumps(self.attempts,indent=2)+'\n')
        if any(r['status']=='pending' or r.get('budget_error') for r in self.attempts):
            raise ValueError('Pending or accounting-error attempt needs inspection')
        self.verify_records()

    def call(self,case,condition,stage,agent):
        name=label(case['id'],condition,stage,agent)
        for retry in range(4):
            if self.stop.is_set():return
            with self.lock:
                records={r['label']:r for r in self.records}
                previous=records.get(name)
                attempts=sum(r.get('logical_label',r['label'])==name for r in self.attempts)
            if previous and previous['status']!='transport_error':return
            if attempts>=4:return
            plan=request_plan(self.frozen,records,case,condition,stage,agent)
            if plan is None:return
            if len(json.dumps(self.body(*plan)).encode())+4096>272000:
                self.stop.set();raise ValueError('Input exceeds price bound')
            if attempts and self.stop.wait([5,15,30][attempts-1]):return
            # Shared ledger/lock preserves aggregate reservation accounting. Each
            # request has its own transport stop flag so transient errors can retry.
            transport=object.__new__(Runner)
            transport.folder=self.folder
            transport.frozen=self.frozen;transport.records=self.attempts
            transport.records_path=self.folder/'attempts.json';transport.lock=self.lock
            transport.stop=threading.Event()
            result=transport.invoke(name+f'#attempt{attempts+1}',*plan,logical_label=name,
                                    condition=condition,stage=stage,agent=agent,case_id=case['id'])
            if result is None or result.get('budget_error'):
                self.stop.set();return
            if result['status']=='transport_error':
                if result.get('http_status') in (400,401,403,404):
                    self.stop.set();return
                continue
            return

    def run(self):
        result=super().run()
        (self.folder/'records.json').write_text(json.dumps(self.records,indent=2)+'\n')
        accounting={'attempts':len(self.attempts),
                    'accounted_usd_including_all_attempts':sum(r.get('cost_usd',r['reserved_usd']) for r in self.attempts),
                    'transport_failures':sum(r['status']=='transport_error' for r in self.attempts)}
        (self.folder/'accounting.json').write_text(json.dumps(accounting,indent=2)+'\n')
        return {**result,**accounting}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('original',type=Path);parser.add_argument('folder',type=Path)
    parser.add_argument('--evidence',type=Path,required=True);parser.add_argument('--upstream',type=Path,required=True)
    args=parser.parse_args()
    verify(args.evidence,args.upstream)
    # Lock outside the new folder so initialization is covered as well as dispatch.
    with args.folder.with_suffix('.lock').open('w') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        runner=Recovery(args.original,args.folder,args.evidence)
        print(json.dumps(runner.run(),indent=2),flush=True)


if __name__=='__main__':main()
