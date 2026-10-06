#!/usr/bin/env python3
"""Collect immutable trial receipts after agents exit, then stop this trial's daemon.
Signature verification is separate (verify_events.rs); never derive metrics from prose.
"""
import importlib.util
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import sys
import time

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('runner',HERE/'runner.py')
r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
base=Path(sys.argv[1]);formation=sys.argv[2];d=r.demo.Demo(base/formation);goal=d.data['goal']
assert 'finished_at' in d.data, 'Do not collect/stop a running trial'
e=d.root/'evidence'
for name,args in {'board':['board'],'contributions':['contributions'],'pending':['pending'],'goal':['goal','status'],'events':['events','--limit','10000']}.items():
 r.write(e/f'after-{name}.json',d.call(args+['--goal',goal],role='codex'))
events=json.loads((e/'after-events.json').read_text())['events']
assert len(events)<10000,'Follow event pagination before claiming complete history'
details=[]
for row in events:
 details.append(d.call(['event','show','--goal',goal,'--event',row['event']],role='codex')['event'])
r.write(e/'event-details.json',details)
contributions=json.loads((e/'after-contributions.json').read_text())['contributions']
for c in contributions:
 for h in c['artifacts']:
  blob=d.call(['blob','get','--goal',goal,'--hash',h],role='codex')
  r.write(e/'artifacts'/f'{h}.json',blob)
os.kill(d.data['daemon_pid'],signal.SIGTERM)
# The store holds an exclusive lock while live. Read only after graceful shutdown.
for _ in range(100):
 try:
  con=sqlite3.connect(f'file:{d.home/"locust.db"}?mode=ro',uri=True,timeout=.1)
  rows=[{'position':p,'id':i.hex(),'author':a.hex(),'seq':s,'signature_hex':sig.hex(),'header_hex':h.hex()} for p,i,a,s,sig,h in con.execute('select position,id,author,seq,signature,header from events where goal=? order by position',(bytes.fromhex(goal),))]
  con.close();break
 except sqlite3.OperationalError:time.sleep(.1)
else:raise RuntimeError('Daemon store stayed locked')
r.write(e/'after-signed.json',rows)
assert {x['id'] for x in rows}=={x['event'] for x in events}
# API replay positions may differ from insertion positions; event identity is stable.
verified=subprocess.check_output([str(base/'verify-events'),str(e/'after-signed.json')],text=True)
r.write(e/'verified.json',verified)
print(json.dumps({'formation':formation,'verified_events':len(rows),'artifacts':sum(len(c['artifacts']) for c in contributions)}))
