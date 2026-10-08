"""Additional usage, cache, timing, and delivered-message audits (no rescoring)."""
import argparse
from collections import defaultdict
from datetime import datetime
import json
from pathlib import Path
import statistics

from protocol import attributable,case_key,job_graph,normalize
from experiment import load,load_records
from transport import atomic_json


def mean(values):return statistics.mean(values) if values else None


def usage_metrics(folder):
    _,manifest,cohort=load(folder);records=load_records(folder)
    rows={case_key(k,r['answerable'],manifest['repetition']):r for k in manifest['family_ids'] for r in cohort['families'][k]['rows']}
    jobs={key:j for case in rows for key,j in job_graph(case,manifest['configs']).items()}
    result={'run_id':manifest['run_id'],'configs':{},
            'cost_note':'Ledger/selection use actual token counts at a uniform conservative rate. Cache-aware estimates describe this multi-arm run, where cross-arm cache reuse may differ from deploying one method alone.',
            'latency_note':'Observed case spans include scheduler waits. Critical-path latency is reconstructed from call durations and graph dependencies, excluding scheduler waits; it is not a Locust runtime measurement.'}
    for config in manifest['configs']:
        selected={key:j for key,j in jobs.items() if attributable(j,config)}
        totals=defaultdict(float);cache_unknown=0;usage_unknown=0;reasoning_unknown=0;case_spans=[];paths=[];stage_spans=[]
        for key in selected:
            r=records.get(key,{})
            if not r.get('request_sha256'):continue
            if 'usage' not in r:usage_unknown+=1;continue
            u=r['usage'];totals['input_tokens']+=u['input_tokens'];totals['output_tokens']+=u['output_tokens']
            detail=u.get('output_tokens_details',{})
            if 'reasoning_tokens' in detail:totals['reasoning_tokens']+=detail['reasoning_tokens']
            else:reasoning_unknown+=1
            incoming=u.get('input_tokens_details',{})
            if 'cached_tokens' not in incoming or 'cache_write_tokens' not in incoming:
                cache_unknown+=1;continue
            read=incoming['cached_tokens'];write=incoming['cache_write_tokens'];fresh=u['input_tokens']-read-write
            if min(read,write,fresh)<0:raise ValueError('Inconsistent cache usage fields')
            totals['cached_input_tokens']+=read;totals['cache_write_tokens']+=write;totals['uncached_input_tokens']+=fresh
            totals['cache_aware_usd_known_calls']+=(read*.01+write*.125+fresh*.1+u['output_tokens']*.5)/1e6
        for case in rows:
            members={key:j for key,j in selected.items() if j['case_id']==case}
            if not all(key in records and records[key]['status']=='completed' and records[key].get('parsed') for key in members):continue
            if not all('started_at' in records[key] and 'finished_at' in records[key] for key in members):continue
            starts=[datetime.fromisoformat(records[key]['started_at']) for key in members]
            ends=[datetime.fromisoformat(records[key]['finished_at']) for key in members]
            case_spans.append((max(ends)-min(starts)).total_seconds())
            cache={}
            def latency(key):
                if key not in cache:
                    cache[key]=records[key]['elapsed_seconds']+max((latency(dep) for dep in jobs[key]['deps']),default=0)
                return cache[key]
            final=f"{case}/{config['name']}/final/0";paths.append(latency(final))
            stage_spans.append(sum(records[key]['elapsed_seconds'] for key in members))
        # Collapse known answer aliases so surface wording alone cannot masquerade
        # as informative disagreement. This supplements the frozen raw-string gate.
        canonical={'groups':0,'discordant':0,'mixed_answer_correctness':0,'mixed_joint_correctness':0}
        if config['arm']=='I':
            from protocol import score
            for case,row in rows.items():
                if not row['answerable']:continue
                initial=[records.get(f"{case}/{config['name']}/initial/{i}") for i in range(3)]
                if not all(r and r.get('parsed') for r in initial):continue
                aliases={normalize(a) for a in [row['answer']]+row.get('answer_aliases',[])}
                keys=[];scores=[]
                for r in initial:
                    o=r['parsed'];answer=normalize(o['answer'])
                    keys.append((o['answerable'],'__gold_alias__' if answer in aliases else answer))
                    scores.append(score(r,row))
                canonical['groups']+=1;canonical['discordant']+=int(len(set(keys))>1)
                canonical['mixed_answer_correctness']+=int(len({s['answer'] for s in scores})>1)
                canonical['mixed_joint_correctness']+=int(len({s['joint'] for s in scores})>1)
        result['configs'][config['name']]={'usage':dict(totals),'calls_without_usage':usage_unknown,
            'calls_without_cache_details':cache_unknown,'calls_without_reasoning_count':reasoning_unknown,
            'completed_pipeline_cases':len(paths),'observed_case_seconds_median':mean([]) if not case_spans else statistics.median(case_spans),
            'critical_path_seconds_mean':mean(paths),'critical_path_seconds_median':statistics.median(paths) if paths else None,
            'summed_call_seconds_mean':mean(stage_spans),'canonical_initial_disagreement':canonical}
    return result


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('folder',type=Path);a=p.parse_args()
    result=usage_metrics(a.folder);atomic_json(a.folder/'usage-metrics.json',result)
    print(json.dumps({'run_id':result['run_id'],'configs':len(result['configs'])}))
