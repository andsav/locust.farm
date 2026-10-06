#!/usr/bin/env python3
"""Materialize exact result bundles and retain independent fixture/test outcomes."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
HERE=Path(__file__).resolve().parent
root=Path(sys.argv[1])/sys.argv[2];e=root/'evidence';m=json.loads((e/'metrics.json').read_text());checks=[]
for c in m['results']:
 for h in c['artifacts']:
  raw=bytes(json.loads((e/'artifacts'/f'{h}.json').read_text())['blob']['bytes']);files=json.loads(raw)
  dest=e/'candidates'/h;dest.mkdir(parents=True,exist_ok=True)
  assert set(files)=={c['task']+'.py','test_'+c['task']+'.py'}
  for name,text in files.items():(dest/name).write_text(text)
  row={'contribution':c['event'],'task':c['task'],'agent':c['agent'],'artifact':h,'bundle_sha256':hashlib.sha256(raw).hexdigest(),'source_lines':len(files[c['task']+'.py'].splitlines()),'test_lines':len(files['test_'+c['task']+'.py'].splitlines())}
  for label,cmd in [('unit',[sys.executable,'-m','unittest','-v','test_'+c['task']]),('oracle',[sys.executable,str(HERE/'oracle.py'),c['task'],str(dest)])]:
   r=subprocess.run(cmd,cwd=dest,capture_output=True,text=True)
   row[label]={'exit_code':r.returncode,'stdout':r.stdout,'stderr':r.stderr}
  checks.append(row)
(e/'candidate-checks.json').write_text(json.dumps(checks,indent=2)+'\n')
print(json.dumps([{k:v for k,v in c.items() if k not in ['unit','oracle']}|{'unit_passed':c['unit']['exit_code']==0,'oracle_passed':c['oracle']['exit_code']==0} for c in checks],indent=2))
