#!/usr/bin/env python3
"""One-Mac real-agent experiment. No assignments, offers, or interventions after launch.
Uses the live_farm_demo profile/daemon setup, not its assigned-phase launcher.
Private runtime data is retained outside Git. Each run is capped at 1200 seconds.
"""
import argparse
from datetime import datetime, timezone
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import sqlite3
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('demo', ROOT / 'scripts/live_farm_demo.py')
demo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(demo)
HERE = Path(__file__).resolve().parent
CLIENTS = {'codex': ('codex', shutil.which('codex')), 'claude': ('claude-code', shutil.which('claude')), 'pi': ('pi', str(Path.home()/'.locust-demos/team-chat-20261004/client-tools/pi-1.0.1/node_modules/.bin/pi'))}
TASKS = [
('slug', 'slugify(text): lowercase ASCII letters/digits; replace every run of other characters with one hyphen; strip leading/trailing hyphens. Unicode non-ASCII is a separator. Empty input returns empty string.'),
('intervals', 'merge_intervals(items): merge overlapping or touching [start,end] numeric pairs into sorted non-overlapping pairs. Do not mutate input. Reject reversed endpoints with ValueError. Empty input returns [].'),
('chunks', 'chunks(items, size): return a list of consecutive lists, preserving order, including a shorter last chunk. Positive integer size only; bool, non-integer and non-positive sizes raise ValueError. Empty input returns [].'),
('counts', 'word_counts(text): count case-insensitive ASCII word tokens matching [a-z]+, returning a dict. Apostrophes and numbers separate tokens. Empty input returns {}.'),
('duration', 'parse_duration(text): parse one or more nonnegative integer-unit pairs with units h,m,s in any order, each at most once, optional whitespace between pairs. Return total seconds. Accept 0s and 1h30m. Reject empty input, unknown units, signs, decimals, repeated units and trailing junk with ValueError.'),
('unique', 'stable_unique(items): return first occurrences preserving order, supporting unhashable items through equality. Do not mutate input. Empty input returns [].'),
('rotate', 'rotate(items, steps): return a new list rotated right by integer steps, negative steps rotate left, large steps wrap. Do not mutate input. Empty input returns [].'),
('flatten', 'flatten(items): recursively flatten nested lists only into a new list, preserving order. Tuples, strings, dicts and other non-list objects remain single items. Do not mutate input. Empty input returns [].'),
]

def write(path, value):
    demo.private_write(path, json.dumps(value, indent=2) + '\n' if not isinstance(value, str) else value)

def snapshot(d, label):
    goal = d.data['goal']
    for name, args in {'board':['board'], 'contributions':['contributions'], 'pending':['pending'], 'goal':['goal','status'], 'events':['events','--limit','10000']}.items():
        write(d.root / 'evidence' / f'{label}-{name}.json', d.call(args + ['--goal', goal], role='codex'))


def prepare(base, formation):
    d = demo.Demo(base / formation)
    if d.manifest.exists():
        raise RuntimeError('Fresh state required')
    d.data = {'schema':2,'agents':{},'phases':{},'formation':formation,'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()}
    demo.private_write(d.binary, (base / 'locust').read_bytes(), executable=True)
    d.start()
    for name,(client,exe) in CLIENTS.items():
        d.data['agents'][name] = d.call(['agent','enroll',name,'--manage-goals'],owner=True)['agent_enrolled']['agent']
        d.call(['session','create',d.root/'sessions'/f'{name}.secret'],owner=True)
        ws = d.workspace(name)
        ws.mkdir(parents=True)
        subprocess.run(['git','init','-q',str(ws)],check=True)
        write(ws/'README.md', 'Throwaway Python standard-library utilities. Each task produces NAME.py and test_NAME.py. Use python3 -m unittest test_NAME. Results are immutable JSON file bundles attached to signed Locust contributions. No integration is required.\n')
        d.prepare_profile(name,client)
        wrapper = d.root/'tools'/name/'locust-board'
        # Log CLI receipts independently of native summaries; no credential contents.
        code = '#!/opt/homebrew/bin/python3\nimport json,subprocess,sys,time\n'
        code += f'cmd={d.command(name)!r}\np=subprocess.run(cmd+sys.argv[1:],capture_output=True,text=True)\n'
        code += f'with open({str(d.root/"logs"/(name+"-cli.jsonl"))!r},"a") as f: f.write(json.dumps({{"at":time.time(),"args":sys.argv[1:],"code":p.returncode,"stdout":p.stdout,"stderr":p.stderr}})+"\\n")\n'
        code += 'print(p.stdout,end=""); print(p.stderr,end="",file=sys.stderr); sys.exit(p.returncode)\n'
        demo.private_write(wrapper,code,executable=True)
        write(ws/'artifact.py', '''import json,subprocess,sys,pathlib
CLI=__CLI__
GOAL=__GOAL__
def call(args):
 p=subprocess.run([CLI]+args,capture_output=True,text=True)
 if p.returncode: raise RuntimeError(p.stdout+p.stderr)
 return json.loads(p.stdout)['result']
if sys.argv[1]=='put':
 data=json.dumps({p:pathlib.Path(p).read_text() for p in sys.argv[2:]}).encode()
 print(json.dumps(call(['blob','put','--goal',GOAL,'--bytes',json.dumps(list(data))])))
elif sys.argv[1]=='get':
 result=call(['blob','get','--goal',GOAL,'--hash',sys.argv[2]])
 print(json.dumps(result))
'''.replace('__CLI__',repr(str(wrapper))))
    goal = d.call(['goal','create','--title',f'Unassigned utilities / {formation}','--formation',formation],role='codex')['goal_created']['goal']
    d.data['goal']=goal
    for name in CLIENTS:
        if name!='codex': d.call(['goal','add-local','--goal',goal,'--agent',name,'--yes'],owner=True)
        d.call(['goal','grant','--goal',goal,'--agent',d.data['agents'][name],'--grants',json.dumps({'administer':False,'contribute':True,'review':True,'select':False,'execute':True,'flow':True,'takeover':False})],owner=True)
        p=d.workspace(name)/'artifact.py'; write(p,p.read_text().replace('__GOAL__',repr(goal)))
    d.data['tasks']={}
    for name,description in TASKS:
        text=f'{name}: Implement {name}.py and test_{name}.py. {description} Add unittest cases covering normal and edge cases. Publish a JSON artifact bundle of those two files and cite it in an attempt-backed contribution. No other task depends on this one.'
        result=d.call(['task','open','--goal',goal,text],role='codex')
        d.data['tasks'][name]='task:' + result['recorded']['event']
        for member in CLIENTS:
            d.call(['task','authorize','--goal',goal,'--task',d.data['tasks'][name],'--agent',d.data['agents'][member]],owner=True)
    d.save()
    snapshot(d,'before')
    return d


def run(d):
    started=time.time(); deadline=started+1200
    d.data['started_at']=started; d.data['deadline']=deadline; d.save()
    procs={}; files=[]
    for name,(client,exe) in CLIENTS.items():
        prompt=(HERE/'prompt-template.txt').read_text().replace('{WORKSPACE}',str(d.workspace(name))).replace('{CLI}',str(d.root/'tools'/name/'locust-board')).replace('{GOAL}',d.data['goal']).replace('{FORMATION}',d.data['formation']).replace('{DEADLINE}',datetime.fromtimestamp(deadline,timezone.utc).isoformat()).replace('{SKILL}',str(ROOT/'skills/locust/SKILL.md'))
        write(d.root/'prompts'/f'{name}.txt',prompt)
        version=subprocess.check_output([exe,'--version'],text=True).strip()
        cmd=d.command(name)+['client','run','--client',client,'--executable',exe,'--workspace',str(d.workspace(name)),'--profile',str(d.root/'profiles'/name),'--client-version',version,'--goal',d.data['goal'],'--prompt',prompt]
        if client=='codex':
            for arg in ['--ask-for-approval','never','--sandbox','danger-full-access']:
                cmd += ['--global-arg',arg]
            cmd += ['--arg','--skip-git-repo-check']
        elif client=='claude-code':
            for arg in ['--bare','--permission-mode','acceptEdits','--allowedTools','Read,Write,Edit,Glob,Grep,Bash,mcp__locust__*']:
                cmd += ['--arg',arg]
        if client=='pi':
            cmd += ['--native-session',str(d.root/'profiles'/name/'.pi/agent/sessions/trial.jsonl')]
            for arg in ['--provider','openai','--model','gpt-6-luna','--approve']:cmd += ['--arg',arg]
        out=open(d.root/'logs'/f'{name}-result.json','w'); err=open(d.root/'logs'/f'{name}-native.jsonl','w'); files += [out,err]
        procs[name]=subprocess.Popen(cmd,cwd=d.workspace(name),stdout=out,stderr=err,stdin=subprocess.DEVNULL,start_new_session=True)
        d.data.setdefault('launches',{})[name]={'pid':procs[name].pid,'at':time.time(),'version':version,'executable':exe}
        d.save()
    while any(p.poll() is None for p in procs.values()) and time.time()<deadline:
        for name,p in procs.items():
            if p.poll() is not None and 'finished_at' not in d.data['launches'][name]:
                d.data['launches'][name]['finished_at']=time.time(); d.save()
        time.sleep(2)
    for name,p in procs.items():
        forced=p.poll() is None
        if forced:
            os.killpg(p.pid,signal.SIGTERM)
            try:p.wait(timeout=10)
            except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
        d.data['launches'][name].setdefault('finished_at',time.time())
        native=json.loads((d.root/'logs'/f'{name}-result.json').read_text())
        d.data['launches'][name].update(exit_code=p.returncode, native_exit_code=native.get('result',{}).get('exit_code'), envelope_ok=native.get('ok'), deadline_terminated=forced)
    for f in files:f.close()
    d.data['finished_at']=time.time();d.save();snapshot(d,'after')
    print(json.dumps({'formation':d.data['formation'],'duration':d.data['finished_at']-started,'launches':d.data['launches']}),flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('mode',choices=['prepare','run','snapshot']);parser.add_argument('base',type=Path);parser.add_argument('formation',choices=['open','peer-review']);args=parser.parse_args()
    if args.mode=='prepare':d=prepare(args.base,args.formation);print(json.dumps(d.data,indent=2))
    else:
        d=demo.Demo(args.base/args.formation)
        if args.mode=='run':run(d)
        else:snapshot(d,'after')
