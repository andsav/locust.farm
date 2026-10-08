"""Case/family outcomes, readiness gates, paired inference, and source sensitivity."""
from collections import Counter, defaultdict
import math
import random
import statistics

from protocol import attributable, case_key, job_graph, normalize, score


def cost(record):
    return record.get('cost_usd',record.get('reserved_usd',0)) if record else 0


def mean(values):
    return statistics.mean(values) if values else None


def pct(values,p):
    if not values:return None
    values=sorted(values);x=(len(values)-1)*p;i=int(x);f=x-i
    return values[i]*(1-f)+values[min(i+1,len(values)-1)]*f


def paired(a,b,seed=20261008,resamples=10000):
    if len(a)!=len(b) or not a:raise ValueError('Paired vectors must have equal nonzero length')
    delta=[int(x)-int(y) for x,y in zip(a,b)];rng=random.Random(seed);n=len(a)
    values=[sum(delta[rng.randrange(n)] for _ in range(n))/n for _ in range(resamples)]
    wins=sum(x==1 for x in delta);losses=sum(x==-1 for x in delta);d=wins+losses
    p=min(1.,2*sum(math.comb(d,k) for k in range(min(wins,losses)+1))/2**d) if d else 1.
    return {'n':n,'difference':sum(delta)/n,'ci95':[pct(values,.025),pct(values,.975)],
            'wins':wins,'losses':losses,'discordance':d/n,'mcnemar_exact_p':p}


def holm(pvalues):
    ordered=sorted(pvalues,key=pvalues.get);out={};last=0.
    for i,key in enumerate(ordered):
        last=max(last,min(1.,pvalues[key]*(len(ordered)-i)));out[key]=last
    return out


def source_clusters(cohort,ids):
    parents={key:key for key in ids}
    def root(k):
        while parents[k]!=k:
            parents[k]=parents[parents[k]];k=parents[k]
        return k
    owners={}
    for key in ids:
        row=next(r for r in cohort['families'][key]['rows'] if r['answerable'])
        for p in row['paragraphs']:
            if not p['is_supporting']:continue
            title=p['title'].casefold().strip()
            if title in owners:parents[root(key)]=root(owners[title])
            else:owners[title]=key
    groups=defaultdict(list)
    for key in ids:groups[root(key)].append(key)
    return list(groups.values())


def cluster_interval(clusters,differences,seed=20261009,resamples=10000):
    rng=random.Random(seed)
    aggregates=[(sum(differences[k] for k in group),len(group)) for group in clusters]
    values=[]
    for _ in range(resamples):
        draws=[aggregates[rng.randrange(len(aggregates))] for _ in aggregates]
        values.append(sum(d[0] for d in draws)/sum(d[1] for d in draws))
    return {'clusters':len(clusters),'largest_cluster':max(map(len,clusters)),
            'ci95':[pct(values,.025),pct(values,.975)],'few_clusters':len(clusters)<20}


def analyze(manifest,cohort,records,inference=False):
    ids=manifest['family_ids'];configs=manifest['configs'];repetition=manifest['repetition']
    rows={case_key(key,row['answerable'],repetition):row for key in ids for row in cohort['families'][key]['rows']}
    jobs={job_id:job for key in rows for job_id,job in job_graph(key,configs).items()}
    result={'phase':manifest['phase'],'run_id':manifest['run_id'],'families':len(ids),'planned_jobs':len(jobs),
            'recorded_jobs':sum(k in records for k in jobs),
            'attempted_calls':sum(r.get('request_sha256') is not None for k,r in records.items() if k in jobs),
            'accounted_usd':sum(cost(r) for k,r in records.items() if k in jobs),
            'status_counts':dict(Counter(records[k]['status'] if k in records else 'not_dispatched' for k in jobs)),
            'configs':{}}
    if manifest['phase'].startswith('development'):
        weights={h:n/sum(cohort['main_strata'].values()) for h,n in cohort['main_strata'].items()}
    else:
        counts=Counter(k[0] for k in ids);weights={h:n/len(ids) for h,n in counts.items()}
    for config in configs:
        name=config['name'];case_results={};attributed=[];stage=defaultdict(Counter);latencies=[];case_costs=defaultdict(float)
        for key,row in rows.items():
            final=records.get(f'{key}/{name}/final/0');case_results[key]=score(final,row)
            if final and 'elapsed_seconds' in final:latencies.append(final['elapsed_seconds'])
        for k,job in jobs.items():
            if not attributable(job,config):continue
            r=records.get(k)
            if r:
                attributed.append(r);case_costs[job['case_id']]+=cost(r)
            if r and r.get('request_sha256'):
                stage[job['stage']]['attempted']+=1
                stage[job['stage']]['valid']+=int(r['status']=='completed' and bool(r.get('parsed')))
                stage[job['stage']]['truncated']+=int(r['status']=='incomplete' and
                    (r.get('incomplete_details') or {}).get('reason')=='max_output_tokens')
        family={key:int(all(case_results[case_key(key,flag,repetition)]['joint'] for flag in (False,True))) for key in ids}
        family_answer={key:int(all(case_results[case_key(key,flag,repetition)]['answer'] for flag in (False,True))) for key in ids}
        by_hop={h:{'n':sum(k[0]==h for k in ids),'successes':sum(v for k,v in family.items() if k[0]==h)} for h in ('3','4')}
        attempted=sum(v['attempted'] for v in stage.values());valid=sum(v['valid'] for v in stage.values())
        claimed_cost=sum(cost(r) for r in attributed)
        valid_final=sum(r['valid'] for r in case_results.values())
        # Discordance is checked on complete-evidence cases; universal abstention
        # on deliberately incomplete evidence does not dilute this diagnostic.
        disagreement=Counter();revision=Counter();quotes=Counter()
        for key,row in rows.items():
            if config['arm'] in ('I','D','E') and row['answerable']:
                initial_config=name if config['arm']=='I' else f"G_{config['budget']}"
                initial=[records.get(f'{key}/{initial_config}/initial/{i}',{}).get('parsed') for i in range(3)]
                if all(initial):
                    disagreement['groups']+=1
                    disagreement['discordant']+=int(len({(o['answerable'],normalize(o['answer'])) for o in initial})>1)
            if config['arm'] in ('D','E','S_review'):
                source=name if config['arm']=='S_review' else f"G_{config['budget']}"
                for i in range(1 if config['arm']=='S_review' else 3):
                    before=score(records.get(f'{key}/{source}/initial/{i}'),row)
                    after_record=records.get(f'{key}/{name}/revision/{i}');after=score(after_record,row)
                    if before['valid'] and after['valid']:
                        revision['pairs']+=1;revision['repaired']+=int(not before['answer'] and after['answer'])
                        revision['damaged']+=int(before['answer'] and not after['answer'])
                        revision['self_reported_change']+=int(bool(after_record['parsed']['changed_by']))
            if final:=records.get(f'{key}/{name}/final/0',{}).get('parsed'):
                lookup={p['idx']:p['paragraph_text'] for p in row['paragraphs']}
                for e in final['evidence']:
                    quotes['excerpts']+=1;quotes['verbatim']+=int(e['quote'] in lookup[e['doc_id']])
        briers=[r['brier'] for r in case_results.values() if r['brier'] is not None]
        cfg={'arm':config['arm'],'budget':config['budget'],'family_successes':sum(family.values()),'family_n':len(ids),
             'primary_rate':mean(list(family.values())),
             'selection_rate':sum(weights[h]*by_hop[h]['successes']/by_hop[h]['n'] for h in weights),
             'family_answer_successes':sum(family_answer.values()),'by_hop':by_hop,
             'case_n':len(rows),'final_valid':valid_final,'final_valid_rate':valid_final/len(rows),
             'answer_exact':sum(r['answer'] for r in case_results.values()),
             'answerable_exact':sum(r['answer'] for k,r in case_results.items() if rows[k]['answerable']),
             'answerable_n':sum(row['answerable'] for row in rows.values()),
             'answerability_correct':sum(r['answerability'] for r in case_results.values()),
             'answer_f1':mean([r['answer_f1'] for r in case_results.values()]),
             'support_f1_complete':mean([r['support_f1'] for k,r in case_results.items() if rows[k]['answerable']]),
             'brier_completed':mean(briers),'brier_completed_n':len(briers),
             'attempted_calls':attempted,'valid_outputs':valid,'valid_output_rate':valid/attempted if attempted else 0,
             'stage_counts':{k:dict(v) for k,v in stage.items()},'accounted_usd':claimed_cost,
             'mean_case_usd':claimed_cost/len(rows),
             'selection_mean_case_usd':sum(weights[h]*sum(v for k,v in case_costs.items() if k[0]==h)/(2*by_hop[h]['n']) for h in weights),
             'case_cost_usd':dict(case_costs),
             'confident_answer_on_incomplete':sum(bool(records.get(f'{key}/{name}/final/0',{}).get('parsed')) and records[f'{key}/{name}/final/0']['parsed']['answerable'] and records[f'{key}/{name}/final/0']['parsed']['confidence']>=.8 for key,row in rows.items() if not row['answerable']),
             'dollars_per_successful_family':claimed_cost/sum(family.values()) if sum(family.values()) else None,
             'final_call_seconds_median':mean([]) if not latencies else statistics.median(latencies),
             'initial_disagreement':dict(disagreement),'revision':dict(revision),'final_quote_audit':dict(quotes),
             'families':family,'family_answer':family_answer,'cases':case_results}
        result['configs'][name]=cfg
    if inference:
        baseline=manifest['baseline'];exchange=manifest['exchange'];independent=manifest['independent'];divided=manifest['divided']
        e=result['configs'][exchange]['families'];result['comparisons']={}
        for label,name in [('primary',baseline),('exchange_vs_divided',divided),('exchange_vs_independent',independent)]:
            b=result['configs'][name]['families'];comparison=paired([e[k] for k in ids],[b[k] for k in ids])
            comparison.update(exchange=exchange,baseline=name)
            result['comparisons'][label]=comparison
        adjusted=holm({key:result['comparisons'][key]['mcnemar_exact_p'] for key in ('exchange_vs_divided','exchange_vs_independent')})
        for key,value in adjusted.items():result['comparisons'][key]['holm_p']=value
        clusters=source_clusters(cohort,ids);b=result['configs'][baseline]['families']
        result['source_overlap_sensitivity']=cluster_interval(clusters,{k:e[k]-b[k] for k in ids})
        ec=result['configs'][exchange]['mean_case_usd'];bc=result['configs'][baseline]['mean_case_usd']
        result['cost_comparison']={'exchange_mean_case_usd':ec,'baseline_mean_case_usd':bc,
                                   'exchange_to_baseline':ec/bc if bc else None,
                                   'competitive':bool(bc and ec<=1.1*bc)}
    return result


def technically_ready(config):
    return (config['valid_output_rate']>=.95 and config['final_valid_rate']>=.95 and
            all(v.get('truncated',0)/v['attempted']<=.05 for v in config['stage_counts'].values() if v['attempted']))


def select_baseline(summary,exchange):
    e=summary['configs'][exchange]
    candidates=[(name,c) for name,c in summary['configs'].items()
                if c['arm'] in ('S_direct','S_review','I') and technically_ready(c)
                and c['selection_mean_case_usd']>=e['selection_mean_case_usd']*.9]
    if not candidates:return None
    return sorted(candidates,key=lambda x:(-x[1]['selection_rate'],x[1]['selection_mean_case_usd'],not x[1]['arm'].startswith('S'),x[0]))[0][0]


def readiness(summary,exchange,independent,divided):
    baseline=select_baseline(summary,exchange);reasons=[]
    if summary['recorded_jobs']!=summary['planned_jobs']:reasons.append('Planned development jobs are not all resolved')
    for key in (exchange,independent,divided):
        if key not in summary['configs'] or not technically_ready(summary['configs'][key]):reasons.append(key+' fails completion/truncation gate')
    if not any(c['arm'].startswith('S_') and technically_ready(c) for c in summary['configs'].values()):reasons.append('No technically valid solo configuration')
    if baseline is None:reasons.append('No cost-qualified valid solo/independent baseline')
    elif not .30<=summary['configs'][baseline]['selection_rate']<=.85:reasons.append('Selected baseline outside 30-85% success range')
    d=summary['configs'].get(independent,{}).get('initial_disagreement',{})
    if not d.get('groups') or d.get('discordant',0)/d['groups']<.10:reasons.append('Less than 10% independent initial-answer discordance')
    return {'ready':not reasons,'reasons':reasons,'baseline':baseline,'exchange':exchange,'independent':independent,'divided':divided}
