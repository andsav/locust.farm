"""Export auditable run records and prepare an arm-blinded annotation review queue."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from protocol import case_key,digest,file_hash
from transport import atomic_json


def export(root,name,destination):
    root=Path(root).resolve();folder=(root/'runs'/name).resolve();destination=Path(destination)
    if folder.parent!=root/'runs':raise ValueError('Run must be directly under this study root')
    verification=subprocess.run([sys.executable,str(folder/'source'/'experiment.py'),'verify',str(folder)],
                                capture_output=True,text=True,check=True)
    checked=json.loads(verification.stdout)
    destination.mkdir(parents=True,exist_ok=True)
    # Only explicit non-secret artifacts are admitted; no environment, DB headers,
    # logs, arbitrary tool files, or full provider response objects are copied.
    record_paths=sorted((folder/'records').glob('*.json'))
    data=b''.join(json.dumps(json.loads(p.read_text()),sort_keys=True,separators=(',',':')).encode()+b'\n'
                  for p in record_paths)
    packed=gzip.compress(data,mtime=0)
    (destination/(name+'-records.jsonl.gz')).write_bytes(packed)
    payload={'run_name':name,'manifest':json.loads((folder/'manifest.json').read_text()),
             'summary':json.loads((folder/'summary.json').read_text()),
             'sources':{p.name:p.read_text() for p in sorted((folder/'source').glob('*.py'))},
             'offline_verification':checked,
             'records_sha256':hashlib.sha256(packed).hexdigest(),
             'records_jsonl_sha256':hashlib.sha256(data).hexdigest(),
             'records_count':len(record_paths)}
    if (folder/'selection.json').exists():payload['selection']=json.loads((folder/'selection.json').read_text())
    if (folder/'stop.json').exists():payload['stop']=json.loads((folder/'stop.json').read_text())
    atomic_json(destination/(name+'-results.json'),payload)
    return {'records':len(record_paths),'gzip_bytes':len(packed),'verification':checked}


def review_queue(root,name,destination):
    root=Path(root);folder=root/'runs'/name
    m=json.loads((folder/'manifest.json').read_text());cohort=json.loads((root/'cohort.json').read_text())
    rows={case_key(k,r['answerable'],m['repetition']):r for k in m['family_ids'] for r in cohort['families'][k]['rows']}
    summary=json.loads((folder/'summary.json').read_text())
    unique={};mapping={}
    for p in sorted((folder/'records').glob('*.json')):
        r=json.loads(p.read_text());job=r['job'];candidate=r.get('parsed')
        if job['stage']!='final' or not candidate:continue
        case=job['case_id'];cfg=job['config'];outcome=summary['configs'][cfg]['cases'][case]
        if outcome['joint']:continue
        row=rows[case]
        # Review plausible alternative answers/citations, not universal failed
        # abstention. Primary scores remain immutable whatever review concludes.
        if not candidate['answerable']:continue
        fingerprint=digest([case,candidate['answer'],sorted(candidate['support_ids'])])
        unique[fingerprint]={'review_id':fingerprint,'question':row['question'],
                             'documents':[{'id':p['idx'],'title':p['title'],'text':p['paragraph_text']} for p in row['paragraphs']],
                             'candidate_answer':candidate['answer'],'candidate_support_ids':candidate['support_ids']}
        mapping.setdefault(fingerprint,[]).append({'case_id':case,'config':cfg,'gold_answerable':row['answerable'],
            'gold_answer':row['answer'],'gold_aliases':row.get('answer_aliases',[]),
            'gold_support_ids':[p['idx'] for p in row['paragraphs'] if p['is_supporting']]})
    order=sorted(unique,key=lambda k:digest([2026100803,k]))
    destination=Path(destination);destination.mkdir(parents=True,exist_ok=True)
    atomic_json(destination/(name+'-blinded-review.json'),[unique[k] for k in order])
    atomic_json(destination/(name+'-review-mapping.json'),mapping)
    return {'unique_candidates':len(order),'queue_sha256':digest([unique[k] for k in order])}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('command',choices=['export','review'])
    p.add_argument('root',type=Path);p.add_argument('run');p.add_argument('destination',type=Path);a=p.parse_args()
    print(json.dumps((export if a.command=='export' else review_queue)(a.root,a.run,a.destination)))
