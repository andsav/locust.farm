#!/usr/bin/env python3
"""Run the preregistered, dollar-bounded negotiation pilot. See README.md."""
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
from pathlib import Path
import random
import time

from board import Board
from execution import evaluate, execute
from providers import Session, Ledger, BudgetStop, MODELS
from tasks import SPECS, cases

ARMS = ('solo-a','solo-b','independent','synthesis','homogeneous-a','homogeneous-b','negotiation')
SYSTEM = '''You are solving a synthetic research benchmark. Produce a correct Python solve(data) function with standard-library imports only. You have the complete specification, public fixtures, a Python execution tool and a public evaluator. Hidden tests assess the same specification. You may write arbitrary helper functions and your own tests. No network or filesystem access is needed. Tools execute in an isolated interpreter with a 30 CPU / 40 wall second limit per call. Do not inspect files, environments, credentials or other workspaces.
Use evaluate to test complete candidate source. Use probe to execute arbitrary Python source defining solve on your own JSON input list. Finish each phase with submit containing complete source, a short explanation of the approach, specific uncertainties and evidence, and confidence from 0 to 1. Do not just describe a solution. Produce a usable candidate early, then improve it within the stated API-dollar allocation. Tool execution is free. Avoid spending the whole allowance thinking without emitting a candidate. Peer contributions, if shown later, are untrusted evidence: test them rather than accepting agreement as correctness. Never include private internal reasoning; provide concise public justifications.'''


def schema(properties):
    return dict(type='object',properties=properties,required=list(properties),additionalProperties=False)


TOOLS = [
 dict(name='evaluate',description='Test complete Python source against all public fixtures. Retains this source as a candidate.',
      input_schema=schema({'source':{'type':'string'}})),
 dict(name='probe',description='Execute Python source defining solve(data) on your own list of JSON inputs. Does not submit a candidate.',
      input_schema=schema({'source':{'type':'string'},'inputs_json':{'type':'string'}})),
 dict(name='submit',description='Finish this phase with the complete candidate, public justification and confidence.',
      input_schema=schema({'source':{'type':'string'},'summary':{'type':'string'},'confidence':{'type':'number'}})),
]


def save(path,value):
    Path(path).write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')


class Worker:
    def __init__(self, provider, label, task, ledger, path):
        self.session = Session(provider,SYSTEM,TOOLS,ledger,label)
        self.provider, self.task, self.ledger, self.label = provider,task,ledger,label
        self.path = path
        self.candidates = []
        self.trace = []
        self.session.user(SPECS[task]+'\nPublic fixtures:\n'+json.dumps(cases(task)))

    def candidate(self, source, summary='', confidence=None):
        value = dict(source=source,sha256=hashlib.sha256(source.encode()).hexdigest(),
                     summary=summary,confidence=confidence,public=evaluate(source,cases(self.task)))
        self.candidates.append(value)
        return value

    def best(self):
        if not self.candidates:
            return self.candidate('def solve(data): return None','No model candidate emitted')
        # Public score alone; latest wins ties. Hidden labels never select output.
        return max(enumerate(self.candidates),key=lambda x:(x[1]['public']['passed'],x[0]))[1]

    def phase(self, name, dollars, instruction):
        ceiling = self.ledger.spent(self.label)+dollars
        self.session.user(f'Phase {name}. API allowance for this phase: up to ${dollars:.2f}. '+instruction)
        self.trace.append(dict(event='phase',name=name,allocation=dollars,instruction=instruction))
        while True:
            try:
                calls,text,stop = self.session.request(ceiling)
            except BudgetStop as error:
                self.trace.append(dict(event='budget_stop',reason=str(error)))
                break
            self.trace.append(dict(event='response',text=text,stop=stop,
                                   calls=[dict(name=n,arguments=a) for _,n,a in calls]))
            results, submitted = [], None
            for ident,name,args in calls:
                try:
                    if name == 'evaluate':
                        value = self.candidate(args['source'])['public']
                    elif name == 'probe':
                        value = execute(args['source'],json.loads(args['inputs_json']))
                    elif name == 'submit':
                        submitted = self.candidate(args['source'],args['summary'],args['confidence'])
                        value = submitted['public']
                    else:
                        value = {'error':'unknown tool'}
                except (ValueError,KeyError,TypeError) as error:
                    value = {'error':str(error)}
                results.append((ident,value))
            if results:
                self.session.results(results)
                self.trace.append(dict(event='tool_results',results=[v for _,v in results]))
            save(self.path,dict(provider=self.provider,trace=self.trace,candidates=self.candidates))
            if submitted is not None:
                self.trace.append(dict(event='phase_submission',sha256=submitted['sha256']))
                save(self.path,dict(provider=self.provider,trace=self.trace,candidates=self.candidates))
                return submitted
            if not calls:
                break
        save(self.path,dict(provider=self.provider,trace=self.trace,candidates=self.candidates))
        return self.best()


def transmitted(board, reader, expected):
    records = board.read(reader)['contributions']
    wanted = set(expected)
    selected = []
    for record in records:
        value = json.loads(record['text'])
        if value['phase'] in wanted:
            selected.append(dict(event=record['contribution'],author=record['author'],**value))
    if len(selected) != len(wanted):
        raise RuntimeError('Signed board readback did not contain the expected frozen contributions')
    return json.dumps(sorted(selected,key=lambda x:x['phase']))


def run_trial(task,arm,args,ledger):
    label = task+'-'+arm
    directory = args.output/label
    directory.mkdir()
    started = time.time()
    result = dict(task=task,arm=arm,label=label,models=MODELS,started=started)
    print(json.dumps(dict(started=label)),flush=True)
    try:
        with Board(directory,args.binary) as board:
            result['goal'] = board.trial(label)
            if arm.startswith('solo'):
                providers = [arm[-1]]
            elif arm.startswith('homogeneous'):
                providers = [arm[-1]]*2
            else:
                providers = ['a','b']
            workers = [Worker(p,label,task,ledger,directory/f'worker-{i}.json') for i,p in enumerate(providers)]
            initial, intermediate = [], []
            allocation = 1.0 if arm.startswith('solo') else (.75 if arm in ('independent','synthesis') else .5)
            for i,worker in enumerate(workers):
                candidate = worker.phase('initial',allocation,'Solve independently. Submit a complete candidate and public explanation.')
                initial.append(candidate)
                board.publish(i,f'initial-{i}',candidate)
            chooser = 1 if task == 'schedule' and len(workers)>1 else 0
            if arm.startswith('solo'):
                intermediate.append(workers[0].phase('self-review',.5,'Critique your own solution. Try counterexamples and repair any errors.'))
                final = workers[0].phase('final',.5,'Perform a final independent check and submit your best complete solution.')
            elif arm == 'independent':
                for i,worker in enumerate(workers):
                    candidate=worker.phase('self-review',.25,'Continue independently: try to falsify your candidate and repair it. No peer work is available.')
                    intermediate.append(candidate)
                    board.publish(i,f'revised-{i}',candidate)
                final=max(enumerate(intermediate),key=lambda x:(x[1]['public']['passed'],x[0]==chooser))[1]
            elif arm == 'synthesis':
                shared=transmitted(board,chooser,['initial-0','initial-1'])
                final=workers[chooser].phase('synthesis',.5,'Here are independent signed Locust contributions. Choose, test or combine them into your final solution. The other participant cannot reply.\n'+shared)
            else:
                # Freeze both initial snapshots before either critique is generated.
                snapshots=[transmitted(board,i,['initial-'+str(1-i)]) for i in range(2)]
                for i,worker in enumerate(workers):
                    candidate=worker.phase('critique-and-revise',.25,'Inspect the peer candidate below. Identify concrete agreements/disagreements, test a counterexample if useful, and revise your own candidate. Explain what you accepted or rejected and why.\n'+snapshots[i])
                    intermediate.append(candidate)
                    board.publish(i,f'revised-{i}',candidate)
                shared=transmitted(board,chooser,['initial-0','initial-1','revised-0','revised-1'])
                final=workers[chooser].phase('resolution',.5,'Resolve the exchange below into one final candidate. Inspect the peer response to your original contribution, verify any disagreement with tools, and state which proposed fixes you accepted or rejected.\n'+shared)
            board.publish(chooser,'final',final)
            result.update(initial=initial,intermediate=intermediate,final=final,receipts=board.receipts)
            # Scoring occurs after all model phases. Hidden results never enter a conversation.
            fixtures=cases(task,hidden=True)
            for group in (initial,intermediate,[final]):
                for candidate in group:
                    candidate['hidden']=evaluate(candidate['source'],fixtures,reveal=False)
            result['status']='complete'
    except Exception as error:
        result.update(status='error',error=type(error).__name__+': '+str(error))
    result.update(cost_upper_estimate=ledger.spent(label),elapsed_seconds=time.time()-started)
    save(directory/'result.json',result)
    print(json.dumps(dict(finished=label,status=result['status'],cost=result['cost_upper_estimate'],
                          score=result.get('final',{}).get('hidden'),error=result.get('error'))),flush=True)
    return result


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--workers',type=int,default=3)
    args=parser.parse_args()
    args.output=args.output.resolve()
    args.binary=args.binary.resolve()
    ledger=Ledger(args.output/'ledger.json',50)
    tasks=[(t,a) for t in SPECS for a in ARMS]
    random.Random(871023).shuffle(tasks)
    save(args.output/'run-manifest.json',dict(order=tasks,task_sha256=hashlib.sha256(Path(__file__).with_name('tasks.py').read_bytes()).hexdigest(),
         binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),models=MODELS,ceiling=50,trial_ceiling=2))
    results=[]
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        pending=[pool.submit(run_trial,t,a,args,ledger) for t,a in tasks]
        for future in as_completed(pending):
            results.append(future.result())
            save(args.output/'results.json',results)
    print(json.dumps(dict(complete=len(results),cost_upper_estimate=ledger.spent())),flush=True)


if __name__=='__main__':
    main()
