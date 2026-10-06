#!/usr/bin/env python3
"""Metrics from signature-verified headers; refusal evidence is local CLI receipts.
Run after collect.py. Retains IDs, times and durations instead of reading model claims.
"""
from collections import Counter
from datetime import datetime,timezone
import json
from pathlib import Path
import statistics
import sys

base=Path(sys.argv[1]);formation=sys.argv[2];root=base/formation;e=root/'evidence'
m=json.loads((root/'demo.json').read_text());events=json.loads((e/'verified.json').read_text());details={x['view']['event']:x for x in json.loads((e/'event-details.json').read_text())}
agents={v:k for k,v in m['agents'].items()};tasks={v:k for k,v in m['tasks'].items()};byid={x['id']:x for x in events};start=m['started_at']*1000
for event in events:
 detail=details[event['id']]; h=event['header']
 assert detail['body']==h['body']
 assert detail['view']['at_ms']==h['at_ms'] and detail['view']['author']==h['author']
 assert detail['view']['standing']=='effective'
attempts=[];results=[];reviews=[];declarations=[];requests=[]
for event in events:
 h=event['header'];kind,body=next(iter(h['body'].items()));who=agents[h['author']];t=h['at_ms']
 row={'event':event['id'],'position':event['position'],'agent':who,'at_ms':t,'seconds':round((t-start)/1000,3)}
 if kind=='attempt_started':
  row.update(task=tasks[body['context']['scope']['task']],offer=body['offer']);attempts.append(row)
 elif kind=='contribution_published' and body['attempt']:
  row.update(task=tasks[body['context']['scope']['task']],attempt=body['attempt'],artifacts=body['artifacts']);results.append(row)
 elif kind=='review_recorded':
  row.update(subject=body['subject'],verdict=body['verdict']);reviews.append(row)
 elif kind=='completion_declared':row.update(subject=body['subject']);declarations.append(row)
 elif kind=='effect_materialized' and 'request_review' in body['effect']['action']:
  req=body['effect']['action']['request_review']
  requests.append({'event':event['id'],'position':event['position'],'subject':req['subject'],'recipient':agents[req['recipient']],'signed_by':who,'at_ms':t})
for a in attempts:
 output=[c for c in results if c['attempt']==a['event']]
 end=[x for x in events if x['header']['body'].get('attempt_reported',{}).get('attempt')==a['event'] and x['header']['body']['attempt_reported']['status'] in ['completed','failed','abandoned']]
 a['first_result_seconds']=min((c['seconds'] for c in output),default=None)
 a['claim_to_result_seconds']=round(min((c['at_ms'] for c in output),default=m['finished_at']*1000)/1000-a['at_ms']/1000,3)
 a['end_seconds']=round((end[0]['header']['at_ms']-start)/1000,3) if end else None
for review in reviews:
 c=next((c for c in results if c['event']==review['subject']),None)
 if c:
  review.update(task=c['task'],result_agent=c['agent'],wait_seconds=round((review['at_ms']-c['at_ms'])/1000,3),self_review=review['agent']==c['agent'])
  review['text']=details[review['event']]['text']
  review['prior_automatic_requests']=[r['event'] for r in requests if r['subject']==review['subject'] and r['recipient']==review['agent'] and r['position']<review['position']]
for c in results:
 evidence=[r for r in (declarations if formation=='open' else reviews) if r['subject']==c['event'] and (r['agent']==c['agent'] if formation=='open' else r['agent']!=c['agent'] and r['verdict']=='approve')]
 c['counted_seconds']=min((r['seconds'] for r in evidence),default=None)
 c['first_review_wait_seconds']=min((r['wait_seconds'] for r in reviews if r['subject']==c['event'] and not r['self_review']),default=None)
counts=Counter(a['task'] for a in attempts);extras=[a for i,a in enumerate(attempts) if a['task'] in [p['task'] for p in attempts[:i]]]
refusals=[]
for name in agents.values():
 p=root/'logs'/f'{name}-cli.jsonl'
 for line in p.read_text().splitlines():
  x=json.loads(line)
  if not x['code']:continue
  try:err=json.loads(x['stdout']).get('error',{})
  except ValueError:err={'message':x['stderr']}
  refusals.append({'agent':name,'at':x['at'],'args':x['args'],'exit_code':x['code'],'error':err})
first_counted={task:min(c['counted_seconds'] for c in results if c['task']==task and c['counted_seconds'] is not None) for task in tasks.values() if any(c['task']==task and c['counted_seconds'] is not None for c in results)}
last_counted=max(first_counted.values(),default=0)
activity=[x for x in events if x['header']['at_ms']>=start and x['header']['at_ms']<=start+last_counted*1000]
points=[('launch',start)]+[(x['id'],x['header']['at_ms']) for x in activity]
gaps=sorted([{'from':a[0],'to':b[0],'seconds':round((b[1]-a[1])/1000,3)} for a,b in zip(points,points[1:])],key=lambda x:x['seconds'],reverse=True)
out={'formation':formation,'goal':m['goal'],'source_commit':m['source_commit'],'start_utc':datetime.fromtimestamp(m['started_at'],timezone.utc).isoformat(),'elapsed_process_seconds':round(m['finished_at']-m['started_at'],3),'all_tasks_counted_seconds':last_counted,'verified_events':len(events),'agents':m['agents'],'launches':m['launches'],'attempts':attempts,'results':results,'reviews':reviews,'automatic_review_requests':requests,'strict_unsolicited_review_count':sum(not r['prior_automatic_requests'] for r in reviews),'declarations':declarations,'tasks_more_than_one_attempt':sum(n>1 for n in counts.values()),'extra_attempts':extras,'extra_claim_to_result_seconds':round(sum(a['claim_to_result_seconds'] for a in extras),3),'untaken_tasks':sorted(set(tasks.values())-set(counts)),'counted_tasks':len({c['task'] for c in results if c['counted_seconds'] is not None}),'refusals':refusals,'longest_event_gaps_before_completion':gaps[:5]}
(e/'metrics.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({k:v for k,v in out.items() if k not in ['agents','launches','attempts','results','reviews','declarations']},indent=2))
