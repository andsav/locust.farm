#!/usr/bin/env python3
"""Post-run analysis only. Never calls a model or changes the frozen protocol."""
import argparse
from collections import defaultdict
import hashlib
import json
import tempfile
from pathlib import Path

from execution import execute
from tasks import cases


def json_text(value):
    return json.dumps(value,sort_keys=True,separators=(',',':'))


def correct_indices(candidate):
    hidden=candidate['hidden']
    if hidden.get('error'):
        return set()
    return set(range(hidden['total']))-{f['index'] for f in hidden['failures']}


def analyze(output):
    results=json.loads((output/'results.json').read_text())
    ledger=json.loads((output/'ledger.json').read_text())
    manifest=json.loads((output/'run-manifest.json').read_text())
    actual_task_hash=hashlib.sha256(Path(__file__).with_name('tasks.py').read_bytes()).hexdigest()
    if manifest['task_sha256'] != actual_task_hash:
        raise RuntimeError('Task/grader module differs from the frozen run manifest')
    if sum(e.get('cost',e['reserved']) for e in ledger)>50+1e-9:
        raise RuntimeError('Recorded cost exceeded the authorized ceiling')
    rows=[]
    for result in sorted(results,key=lambda r:(r['task'],r['arm'])):
        row={k:result[k] for k in ('task','arm','status','cost_upper_estimate','elapsed_seconds')}
        recorded=sum(e.get('cost',e['reserved']) for e in ledger if e['label']==result['label'])
        if abs(recorded-result['cost_upper_estimate'])>1e-9:
            raise RuntimeError('Trial charge does not reconcile with the global ledger')
        directory=output/result['label']
        workers=[json.loads(p.read_text()) for p in sorted(directory.glob('worker-*.json'))]
        row['requests']=sum(e['label']==result['label'] for e in ledger)
        row['phases']=[]
        for worker in workers:
            phase=None
            for event in worker['trace']:
                if event['event']=='phase':
                    phase=dict(provider=worker['provider'],name=event['name'],allocation=event['allocation'],responses=0,incomplete=0)
                    row['phases'].append(phase)
                elif phase is not None and event['event']=='response':
                    phase['responses']+=1
                    phase['incomplete']+=event['stop'] in ('incomplete','max_tokens')
        row['budget_stops']=[e['reason'] for w in workers for e in w['trace'] if e['event']=='budget_stop']
        row['tools']={name:sum(c['name']==name for w in workers for e in w['trace']
                              if e['event']=='response' for c in e['calls'])
                      for name in ('evaluate','probe','submit')}
        if result['status']!='complete':
            row['error']=result['error']
            rows.append(row)
            continue
        candidates=result['initial']+result['intermediate']+[result['final']]
        for candidate in candidates:
            if hashlib.sha256(candidate['source'].encode()).hexdigest()!=candidate['sha256']:
                raise RuntimeError('Candidate source hash mismatch')
        hashes={c['sha256'] for c in candidates}
        if any(r['source_sha256'] not in hashes or r['goal']!=result['goal'] for r in result['receipts']):
            raise RuntimeError('Publication receipt does not match the scored trial')
        final=result['final']
        row.update(passed=final['hidden']['passed'],total=final['hidden']['total'],
                   initial_scores=[c['hidden']['passed'] for c in result['initial']],
                   intermediate_scores=[c['hidden']['passed'] for c in result['intermediate']],
                   initial_confidence=[c['confidence'] for c in result['initial']],
                   final_confidence=final['confidence'],final_summary=final['summary'],
                   final_sha256=final['sha256'],publication_count=len(result['receipts']))
        chooser=1 if result['task']=='schedule' and len(result['initial'])>1 else 0
        previous=correct_indices(result['initial'][chooser])
        current=correct_indices(final)
        row['fixed_cases']=sorted(current-previous)
        row['regressed_cases']=sorted(previous-current)
        row['source_changed_from_designated_initial']=final['sha256']!=result['initial'][chooser]['sha256']
        if len(result['initial'])==2:
            inputs=[c['input'] for c in cases(result['task'],hidden=True)]
            outputs=[execute(c['source'],inputs) for c in result['initial']]
            if all(isinstance(v,list) and len(v)==len(inputs) for v in outputs):
                row['initial_behavior_disagreements']=[i for i,(a,b) in enumerate(zip(*outputs)) if json_text(a)!=json_text(b)]
            else:
                row['initial_behavior_disagreements']='execution failed during analysis'
        rows.append(row)
    totals=defaultdict(lambda:dict(passed=0,total=0,complete_tasks=0,all_pass_tasks=0,cost=0))
    for row in rows:
        arm=totals[row['arm']]
        arm['cost']+=row['cost_upper_estimate']
        if row['status']=='complete':
            arm['passed']+=row['passed']
            arm['total']+=row['total']
            arm['complete_tasks']+=1
            arm['all_pass_tasks']+=row['passed']==row['total']
    return dict(rows=rows,arms=dict(totals),request_count=len(ledger),
                batch_cost_upper_estimate=sum(row['cost_upper_estimate'] for row in rows),
                cost_upper_estimate=sum(e.get('cost',e['reserved']) for e in ledger),
                unresolved_reservations=sum(e['status']=='pending' for e in ledger),
                input_tokens=sum(e.get('usage',{}).get('input_tokens',0) for e in ledger),
                output_tokens=sum(e.get('usage',{}).get('output_tokens',0) for e in ledger))


def export(output,destination):
    destination.mkdir(parents=True,exist_ok=True)
    analysis=analyze(output)
    # Export only named, public evidence. Never traverse daemon profiles, keys,
    # arbitrary logs, binary files, or provider opaque reasoning blocks.
    paths=[output/name for name in ('results.json','ledger.json','run-manifest.json')]
    paths+=sorted(output.glob('*/worker-*.json'))
    artifacts={str(path.relative_to(output)):json.loads(path.read_text()) for path in paths}
    packed=json.dumps(artifacts,sort_keys=True,indent=2)+'\n'
    for token in ('encrypted_content','"thinking":','OPENAI_API_KEY=','ANTHROPIC_API_KEY='):
        if token in packed:
            raise RuntimeError('Evidence export contains forbidden provider/private material: '+token)
    (destination/'evidence.json').write_text(packed)
    (destination/'analysis.json').write_text(json.dumps(analysis,sort_keys=True,indent=2)+'\n')
    manifest=dict(files={p.name:hashlib.sha256(p.read_bytes()).hexdigest()
                         for p in (destination/'evidence.json',destination/'analysis.json')},
                  source_output=str(output),protocol_commit='869e90d')
    (destination/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(analysis,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    source=parser.add_mutually_exclusive_group(required=True)
    source.add_argument('--output',type=Path)
    source.add_argument('--from-evidence',type=Path)
    parser.add_argument('--destination',type=Path,required=True)
    args=parser.parse_args()
    if args.from_evidence:
        artifacts=json.loads(args.from_evidence.read_text())
        with tempfile.TemporaryDirectory(prefix='lnp-analysis-') as directory:
            root=Path(directory)
            for name,value in artifacts.items():
                relative=Path(name)
                if relative.is_absolute() or '..' in relative.parts or relative.suffix!='.json':
                    raise ValueError('Invalid evidence path')
                path=root/relative
                path.parent.mkdir(parents=True,exist_ok=True)
                path.write_text(json.dumps(value))
            export(root,args.destination.resolve())
    else:
        export(args.output.resolve(),args.destination.resolve())
