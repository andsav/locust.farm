"""Offline replay of the archived graph-coloring validity defect.

This fixed K12 counterexample is a public historical regression fixture. It is
constructed from the source defect, is outside the random benchmark graph
population, and is never a held-out pack. Passing it proves only that the current
sandbox exposes the known defect and the public selector rejects it after that
exposure; it cannot prove that public evaluation detects arbitrary hidden bugs.

The two source strings below are byte-exact archival evidence, not an old harness
reader or alternate runtime. Candidate code is executed only by scoring.py's
current subprocess sandbox. This module performs no model or network requests.
"""
import argparse
import hashlib
import json
from pathlib import Path

from problems import reference_costs
import scoring

ARCHIVE_PATH = 'research/evidence/shared-record-study-2026-10-08/trials/scored/coloring-shared.json.gz'
ARCHIVE_SHA256 = 'd4945cd635f62e5f4c376819b08c55c056219f3ecb3cdebf48102ed68eb1f4fa'
BAD_SOURCE_SHA256 = '078d839975c0436a364fc48d8a80190bc281c63b2e24eed4e9079341ee976a9a'
REPAIRED_SOURCE_SHA256 = '7493bf5cd556cf8d494e06c5d0a499890e9cdbf3cadfe189fcb3f2bd7d7d56b9'
FIXTURE_SEED = 'historical-coloring-k12-public-regression-v1'

# Do not reformat archival source strings: their exact bytes establish provenance.
BAD_SOURCE = """import time, random

def _tabu_color(adj, start, k, rng, deadline):
    n=len(adj)
    base=start[:]
    while time.process_time()<deadline:
        col=base[:]
        cnt=[[0]*k for _ in range(n)]
        for v in range(n):
            for w in adj[v]: cnt[v][col[w]]+=1
        tabu=[[0]*k for _ in range(n)]
        current=sum(cnt[v][col[v]] for v in range(n))//2
        best=current; it=0
        while it<200000 and time.process_time()<deadline:
            it+=1
            conflicted=[v for v in range(n) if cnt[v][col[v]]>0]
            if not conflicted: return col
            bestdelta=10**9; moves=[]
            for v in conflicted:
                old=col[v]; oldcnt=cnt[v][old]
                for c in range(k):
                    if c==old: continue
                    delta=cnt[v][c]-oldcnt
                    if tabu[v][c]>it and current+delta>=best: continue
                    if delta<bestdelta:
                        bestdelta=delta; moves=[(v,c)]
                    elif delta==bestdelta:
                        moves.append((v,c))
            if not moves:
                for v in conflicted:
                    old=col[v]; oldcnt=cnt[v][old]
                    for c in range(k):
                        if c==old: continue
                        delta=cnt[v][c]-oldcnt
                        if delta<bestdelta:
                            bestdelta=delta; moves=[(v,c)]
                        elif delta==bestdelta:
                            moves.append((v,c))
            v,c=rng.choice(moves); old=col[v]
            current+=cnt[v][c]-cnt[v][old]
            for w in adj[v]:
                cnt[w][old]-=1; cnt[w][c]+=1
            col[v]=c
            if current<best: best=current
            tabu[v][old]=it+rng.randint(5,10)+int(0.6*len(conflicted))
        # re-randomize 20% of vertices for the next basin
        base=[rng.randrange(k) if rng.random()<0.20 else x for x in base]
    return None

def solve(instance):
    n=instance['n']; adj=[[] for _ in range(n)]
    for u,v in instance['edges']:
        adj[u].append(v); adj[v].append(u)
    deg=[len(a) for a in adj]
    col=[-1]*n; sat=[set() for _ in range(n)]
    for _ in range(n):
        v=max((i for i in range(n) if col[i]<0),key=lambda i:(len(sat[i]),deg[i]))
        c=0
        while c in sat[v]: c+=1
        col[v]=c
        for w in adj[v]:
            if col[w]<0: sat[w].add(c)
    if max(col)<10: return col
    counts=[col.count(c) for c in range(11)]
    drop=min(range(11),key=lambda c:counts[c])
    start=[(0 if x==drop else x-1 if x>drop else x) for x in col]
    rng=random.Random(7281)
    deadline=time.process_time()+1.35
    ten=_tabu_color(adj,start,10,rng,deadline)
    if ten is None:
        return col
    counts=[ten.count(c) for c in range(10)]
    drop=min(range(10),key=lambda c:counts[c])
    start9=[(0 if x==drop else x-1 if x>drop else x) for x in ten]
    nine=_tabu_color(adj,start9,9,rng,deadline)
    return nine if nine is not None else ten"""

REPAIRED_SOURCE = """import time, random

def _reduce_colors(adj, col, k):
    n=len(col); groups={c:[v for v in range(n) if col[v]==c] for c in set(col)}; label=col[:]
    while len(groups)>k:
        s=min(groups,key=lambda c:(len(groups[c]),c)); src=groups[s]
        dest=None; best=None
        for t,vs in groups.items():
            if t==s:continue
            cross=0
            for v in src:
                for w in adj[v]:
                    if label[w]==t:cross+=1
            key=(cross,-len(vs),t)
            if best is None or key<best:best=key;dest=t
        groups[dest].extend(src)
        for v in src:label[v]=dest
        del groups[s]
    remap={c:i for i,c in enumerate(groups)}
    return [remap[label[v]] for v in range(n)]

def _tabu_color(adj, start, k, rng, deadline):
    n=len(adj);base=start[:]
    while time.process_time()<deadline:
        col=base[:];cnt=[[0]*k for _ in range(n)]
        for v in range(n):
            for w in adj[v]:cnt[v][col[w]]+=1
        tabu=[[0]*k for _ in range(n)];current=sum(cnt[v][col[v]] for v in range(n))//2;best=current;it=0
        while it<200000 and time.process_time()<deadline:
            it+=1;conf=[v for v in range(n) if cnt[v][col[v]]>0]
            if not conf:return col
            bd=10**9;moves=[]
            for v in conf:
                old=col[v];oc=cnt[v][old]
                for c in range(k):
                    if c==old:continue
                    delta=cnt[v][c]-oc
                    if tabu[v][c]>it and current+delta>=best:continue
                    if delta<bd:bd=delta;moves=[(v,c)]
                    elif delta==bd:moves.append((v,c))
            if not moves:
                for v in conf:
                    old=col[v];oc=cnt[v][old]
                    for c in range(k):
                        if c!=old:
                            delta=cnt[v][c]-oc
                            if delta<bd:bd=delta;moves=[(v,c)]
                            elif delta==bd:moves.append((v,c))
            v,c=rng.choice(moves);old=col[v];current+=cnt[v][c]-cnt[v][old]
            for w in adj[v]:cnt[w][old]-=1;cnt[w][c]+=1
            col[v]=c;best=min(best,current);tabu[v][old]=it+rng.randint(5,10)+int(.6*len(conf))
        base=[rng.randrange(k) if rng.random()<.20 else x for x in base]
    return None

def solve(instance):
    n=instance['n'];adj=[[] for _ in range(n)]
    for u,v in instance['edges']:adj[u].append(v);adj[v].append(u)
    deg=[len(a) for a in adj];col=[-1]*n;sat=[set() for _ in range(n)]
    for _ in range(n):
        v=max((i for i in range(n) if col[i]<0),key=lambda i:(len(sat[i]),deg[i]));c=0
        while c in sat[v]:c+=1
        col[v]=c
        for w in adj[v]:
            if col[w]<0:sat[w].add(c)
    colors=max(col)+1
    if colors<=9:return col
    rng=random.Random(7281);deadline=time.process_time()+1.35
    ten=col if colors<=10 else _tabu_color(adj,_reduce_colors(adj,col,10),10,rng,deadline)
    if ten is None:return col
    nine=_tabu_color(adj,_reduce_colors(adj,ten,9),9,rng,deadline)
    if nine is None:return ten
    eight=_tabu_color(adj,_reduce_colors(adj,nine,8),8,rng,deadline)
    return eight if eight is not None else nine"""


def sha256(value):
    return hashlib.sha256(value).hexdigest()


def public_regression_pack():
    """K12 forces DSATUR to use twelve distinct colors, exposing the bad index."""
    return [{'n': 12, 'edges': [[u, v] for u in range(12) for v in range(u + 1, 12)]}]


def replay():
    """Execute both historical sources with the current evaluator and selector."""
    if sha256(BAD_SOURCE.encode()) != BAD_SOURCE_SHA256:
        raise ValueError('defective historical source hash mismatch')
    if sha256(REPAIRED_SOURCE.encode()) != REPAIRED_SOURCE_SHA256:
        raise ValueError('repaired historical source hash mismatch')
    instances = public_regression_pack()
    reference = reference_costs('coloring', instances)
    candidates = [
        {'id': 'archived-defect', 'archive_agent': 'agent-1', 'archive_candidate_index': 2,
         'source_sha256': BAD_SOURCE_SHA256, 'source': BAD_SOURCE},
        {'id': 'archived-repair', 'archive_agent': 'agent-1', 'archive_candidate_index': 5,
         'source_sha256': REPAIRED_SOURCE_SHA256, 'source': REPAIRED_SOURCE},
    ]
    for candidate in candidates:
        candidate['public'] = scoring.score(
            'coloring', candidate.pop('source'), instances, reference,
            repetitions=1, seed=FIXTURE_SEED)
    bad, repaired = candidates
    selected = scoring.select_public(candidates)
    reversed_selection = scoring.select_public(list(reversed(candidates)))
    # A control that drops validity would select the defective source on this tie.
    cost_only = min(candidates, key=lambda item: (
        item['public']['normalized'], item['public']['source_sha256']))
    checks = {
        'archive_defect_raises_index_error': all(
            not row['valid'] and row.get('error', '').startswith('IndexError:')
            for row in bad['public']['instances']),
        'archive_repair_is_public_valid': scoring.all_valid(repaired['public']),
        'fallback_and_repair_costs_tie': (
            bad['public']['normalized'] == repaired['public']['normalized'] == 1.0),
        'cost_only_control_selects_defect': cost_only['id'] == bad['id'],
        'public_selector_selects_repair': selected is not None and selected['id'] == repaired['id'],
        'selection_independent_of_input_order': (
            reversed_selection is not None and reversed_selection['id'] == repaired['id']),
        'defect_alone_is_ineligible': scoring.select_public([bad]) is None,
    }
    root = Path(__file__).resolve().parent
    return {
        'schema': 'historical-coloring-selector-regression-v1',
        'status': 'passed' if all(checks.values()) else 'failed',
        'scope': 'public historical regression fixture; not held-out benchmark evidence',
        'limitation': ('Validity gating rejects the known defect only after a public fixture exposes it; '
                       'this does not establish hidden-bug detection or benchmark performance.'),
        'archive': {'path': ARCHIVE_PATH, 'sha256': ARCHIVE_SHA256},
        'evaluator': {
            'cpu_seconds_per_instance': scoring.CPU_SECONDS_PER_INSTANCE,
            'wall_seconds_per_instance': scoring.WALL_LIMIT,
            'repetitions': 1, 'seed': FIXTURE_SEED,
            'code_sha256': {name: sha256((root / name).read_bytes()) for name in (
                'selector_regression.py', 'scoring.py', 'problems.py')},
        },
        'fixture': {
            'name': 'complete-graph-k12', 'instances': instances, 'reference_costs': reference,
            'sha256': sha256(json.dumps(instances, sort_keys=True, separators=(',', ':')).encode()),
            'derivation': ('DSATUR uses colors 0 through 11. The defective source removes one of '
                           'the first eleven classes, leaving color 10 for a ten-column counter.'),
        },
        'candidates': candidates,
        'cost_only_control_selected': cost_only['id'],
        'selected': None if selected is None else selected['id'],
        'checks': checks,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, help='Write the complete offline replay report to this JSON file')
    args = parser.parse_args(argv)
    report = replay()
    rendered = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + '\n'
    if args.output is None:
        print(rendered, end='')
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered)
    return 0 if report['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
