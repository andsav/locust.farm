"""Export and verify original Nous-run results without redistributing upstream prompts."""
import argparse
from datetime import date
import json
from pathlib import Path
import tempfile

from protocol import analyze, digest, prepare
from experiment import Experiment


def analysis_input(manifest):
    return {'scope':manifest['scope'],'phase':manifest['phase'],
            'cases':[{'id':c['id'],'cluster':c['cluster'],'outcome':c['outcome'],
                      'public':{'evidence':[{'id':e['id']} for e in c['public']['evidence']]}}
                     for c in manifest['cases']]}


def diagnostics(manifest, records):
    from statistics import mean
    from protocol import CONDITIONS, parse
    allowed={c['id']:{e['id'] for e in c['public']['evidence']} for c in manifest['cases']}
    def parsed(r):return parse(r,allowed.get(r.get('case_id'),set()))
    valid=[r for r in records if parsed(r) is not None]
    initial=[r for r in records if r.get('stage')=='initial' and 'usage' in r]
    balanced=[]
    for case in manifest['cases']:
        groups={c:[r['usage']['input_tokens'] for r in initial if r['case_id']==case['id'] and r['condition']==c] for c in CONDITIONS}
        if all(len(v)==10 for v in groups.values()):
            means={c:mean(v) for c,v in groups.items()}
            balanced.append({'case_id':case['id'],'mean_input_tokens':means,
                             'max_over_min_minus_one':max(means.values())/min(means.values())-1})
    return {'valid_responses':len(valid),'invalid_completed':[r['label'] for r in records if r['status']=='completed' and parsed(r) is None],
            'failed_attempts':[{'label':r['label'],'status':r['status'],'error_type':r.get('error_type')} for r in records if r['status']!='completed'],
            'paired_case_token_balance':balanced,
            'token_balance_within_5_percent':bool(balanced) and len(balanced)==len(manifest['cases']) and all(r['max_over_min_minus_one']<=.05 for r in balanced),
            'models':sorted({r['returned_model'] for r in records if 'returned_model' in r})}


def export(folder, destination):
    manifest=json.loads((folder/'manifest.json').read_text())
    if digest(manifest)!=(folder/'manifest.sha256').read_text().strip():raise ValueError('Manifest hash mismatch')
    records=json.loads((folder/'records.json').read_text())
    if any(r['status']=='pending' for r in records):raise ValueError('Wait for pending requests before export')
    if any(r.get('simulated') for r in records):raise ValueError('Live evidence exporter rejects simulated records')
    slim=analysis_input(manifest)
    bundle={'version':1,'scope':'Original model outputs and scoring; upstream copyrighted prompt/brief bodies omitted',
            'run_manifest_sha256':digest(manifest),'upstream_commit':manifest['upstream_commit'],
            'source_hashes':manifest['source_hashes'],'code_hashes':manifest['code_hashes'],
            'settings':{k:manifest[k] for k in ('phase','as_of','ceiling_usd','model','effort','max_output_tokens','input_rate_per_million','output_rate_per_million')},
            'analysis_input':slim,'records':records,'summary':analyze(slim,records),'diagnostics':diagnostics(manifest,records)}
    destination.write_text(json.dumps(bundle,indent=2,allow_nan=False)+'\n')
    return {'output':str(destination),'records':len(records),'summary_sha256':digest(bundle['summary'])}


def verify(path, upstream=None):
    bundle=json.loads(path.read_text());slim=bundle['analysis_input'];records=bundle['records']
    if analyze(slim,records)!=bundle['summary']:raise ValueError('Summary mismatch')
    if diagnostics(slim,records)!=bundle['diagnostics']:raise ValueError('Diagnostic mismatch')
    if any(r.get('simulated') or r['status']=='pending' for r in records):raise ValueError('Not finalized live evidence')
    settings=bundle['settings']
    for r in records:
        if 'usage' in r:
            expected=(r['usage']['input_tokens']*settings['input_rate_per_million']+r['usage']['output_tokens']*settings['output_rate_per_million'])/1e6
            if abs(expected-r['cost_usd'])>1e-12 or r['cost_usd']>r['reserved_usd']:raise ValueError('Accounting mismatch')
    verified_requests=False
    if upstream is not None:
        if settings['phase']=='prospective':raise ValueError('Prospective request verification also requires the original frozen cohort')
        with tempfile.TemporaryDirectory() as tmp:
            folder=Path(tmp)/'run'
            result=prepare(upstream,folder,settings['phase'],date.fromisoformat(settings['as_of']),settings['ceiling_usd'])
            if result['manifest_sha256']!=bundle['run_manifest_sha256']:raise ValueError('Rebuilt manifest mismatch')
            runner=Experiment(folder)
            if analysis_input(runner.frozen)!=slim:raise ValueError('Analysis labels differ from frozen source')
            runner.records=records;runner.verify_records();verified_requests=True
    return {'verified_records':len(records),'scores_and_costs_recomputed':True,'request_hashes_rebuilt_from_upstream':verified_requests}


def main():
    p=argparse.ArgumentParser(description=__doc__);sub=p.add_subparsers(dest='command',required=True)
    a=sub.add_parser('export');a.add_argument('folder',type=Path);a.add_argument('destination',type=Path)
    a=sub.add_parser('verify');a.add_argument('path',type=Path);a.add_argument('--upstream',type=Path)
    args=p.parse_args()
    result=export(args.folder,args.destination) if args.command=='export' else verify(args.path,args.upstream)
    print(json.dumps(result,indent=2))

if __name__=='__main__':main()
