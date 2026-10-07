"""Frozen synthetic task specifications and independent exhaustive graders."""
import itertools
import json
import random

SPECS = {
'replay': '''Implement solve(data) for a causal observed-remove set.
Input data.events is a list of JSON events. Each event has string id, kind (add or remove), string key, and deps (list of event IDs). Remove events also have targets (list of add IDs). Add events have no targets. IDs and keys are arbitrary strings.
Group events by ID before doing anything else. Byte-equivalent canonical JSON bodies (sort object keys; array order is significant) are duplicates and count once. If one ID has distinct bodies, that ID is equivocated: none of its bodies is eligible. An event is eligible iff its ID is unambiguous and ALL dependencies exist and are eligible. Self-dependency and all dependency cycles are ineligible, as are their descendants. An unambiguous event with no deps is eligible. Input order conveys no authority.
An eligible remove removes an add iff that add is eligible, has the same key, is named in targets, and is a strict transitive dependency ancestor of the remove. Naming a concurrent or future add cannot remove it. Multiple eligible removes combine. Removing an add does not make the add ineligible and does not invalidate descendants. A remove that removes nothing can still be eligible.
Return exactly {"eligible": sorted eligible IDs, "live": sorted IDs of eligible add events not removed}. Return every ID once. Use Python string ordering. Inputs contain at most 30 event bodies. Handle absent dependencies, duplicates, equivocation, cycles, concurrency and unordered input. Do not mutate the input.''',
'schedule': '''Implement solve(data), returning an optimal list of selected job IDs in execution order on one machine. Jobs are optional. Each job has id (distinct string), release (integer >=0), duration (>0), deadline (>=0), value (>0), family (0..2), and deps (list of other job IDs). A selected job requires every dependency to have appeared earlier in the selected sequence. Dependencies may form cycles or mention absent jobs, making affected jobs impossible to select.
The machine starts at time 0 with family 0. After a previous completion t in family f, job j cannot start before max(j.release, t + data.setup[f][j.family]). For the first job use t=0,f=0. Setup is just this transition delay and is allowed during blackouts. Execution occupies [start,start+duration); it cannot overlap any half-open blackout [a,b) in data.blackouts. Endpoints touching are allowed. Blackouts may overlap and arrive unsorted. For a chosen sequence each job starts at the EARLIEST allowed time, moving forward past overlapping blackouts as needed. Jobs cannot be interrupted. Completion must be <=deadline.
Choose a feasible sequence by these priorities: maximize total value, then minimize the final completion time (empty sequence completes at 0), then choose the lexicographically smallest sequence of IDs using Python list/string order. Return only that list of IDs. Up to 8 jobs. Integers are small. All information is in the input; an exact solution is required, not a heuristic.''',
'flow': '''Implement solve(data) for a directed integral minimum-cost flow.
Input n is the number of vertices labelled 0..n-1; balance[v] is REQUIRED total outgoing flow minus total incoming flow at v. Each edge in data.edges has u,v,lower,upper,cost. Choose one integer flow per edge in input order with lower<=flow<=upper, satisfying every vertex balance. Parallel edges and self loops are allowed. Costs can be negative, including negative cycles; circulations are allowed and count toward cost. Every edge is bounded. Return null (Python None) iff infeasible. Otherwise minimize sum(flow[i]*cost[i]), breaking ties by lexicographically smallest COMPLETE flow vector in original edge order, and return that vector as a list of integers. Empty edges are allowed. Up to 6 vertices and 9 edges; each upper<=3. Exact solutions are required. Negative-cost disconnected components matter.'''
}


def replay(data):
    bodies = {}
    for e in data['events']:
        bodies.setdefault(e['id'], {})[json.dumps(e, sort_keys=True)] = e
    unique = {ident: next(iter(values.values())) for ident, values in bodies.items() if len(values) == 1}
    ancestors = {}
    while True:
        ready = [ident for ident, e in unique.items() if ident not in ancestors
                 and all(dep in ancestors for dep in e['deps'])]
        if not ready:
            break
        for ident in ready:
            deps = set(unique[ident]['deps'])
            ancestors[ident] = deps | set().union(*(ancestors[x] for x in deps))
    removed = set()
    for ident in ancestors:
        e = unique[ident]
        if e['kind'] == 'remove':
            for target in e['targets']:
                if target in ancestors[ident] and unique[target]['kind'] == 'add' and unique[target]['key'] == e['key']:
                    removed.add(target)
    return {'eligible': sorted(ancestors), 'live': sorted(i for i in ancestors
            if unique[i]['kind'] == 'add' and i not in removed)}


def schedule(data):
    best = (0, 0, ())
    jobs = data['jobs']
    def visit(sequence, used, finish, family, value):
        nonlocal best
        result = (-value, finish, tuple(sequence))
        if result < best:
            best = result
        for j in jobs:
            if j['id'] in used or not set(j['deps']) <= used:
                continue
            start = max(j['release'], finish + data['setup'][family][j['family']])
            while True:
                conflicts = [b for a, b in data['blackouts'] if start < b and start+j['duration'] > a]
                if not conflicts:
                    break
                start = max(conflicts)
            end = start + j['duration']
            if end <= j['deadline']:
                visit(sequence+[j['id']], used | {j['id']}, end, j['family'], value+j['value'])
    visit([], set(), 0, 0, 0)
    return list(best[2])


def flow(data):
    best = None
    edges = data['edges']
    for values in itertools.product(*(range(e['lower'], e['upper']+1) for e in edges)):
        balances = [0] * data['n']
        cost = 0
        for e, value in zip(edges, values):
            balances[e['u']] += value
            balances[e['v']] -= value
            cost += value * e['cost']
        if balances == data['balance']:
            item = (cost, values)
            if best is None or item < best:
                best = item
    return None if best is None else list(best[1])


ORACLES = {'replay': replay, 'schedule': schedule, 'flow': flow}


def generate(name, seed, count):
    rng = random.Random(seed)
    result = []
    for case in range(count):
        if name == 'replay':
            events = []
            n = rng.randint(6, 18)
            for i in range(n):
                kind = rng.choice(['add', 'add', 'remove'])
                e = dict(id=f'e{i}', kind=kind, key=rng.choice(['x','y','z']),
                         deps=rng.sample([f'e{k}' for k in range(n+2)], rng.randrange(4)))
                if rng.random() < .45:
                    e['deps'] = rng.sample([f'e{k}' for k in range(i)], min(i, rng.randrange(3)))
                if kind == 'remove':
                    e['targets'] = rng.sample([f'e{k}' for k in range(n)], rng.randrange(4))
                events.append(e)
                if rng.random() < .15:
                    events.append(dict(e))
                if rng.random() < .12:
                    events.append(dict(e, key='conflict'))
            rng.shuffle(events)
            data = {'events': events}
        elif name == 'schedule':
            n = rng.randint(4, 8)
            jobs = []
            for i in range(n):
                deps = [] if rng.random() < .65 else [f'j{rng.randrange(n+1)}']
                jobs.append(dict(id=f'j{i}', release=rng.randrange(10), duration=rng.randint(1,5),
                                 deadline=rng.randint(5,25), value=rng.randint(1,9),
                                 family=rng.randrange(3), deps=deps))
            data = dict(jobs=jobs, setup=[[rng.randrange(4) for _ in range(3)] for _ in range(3)],
                        blackouts=[[a, a+rng.randint(1,5)] for a in rng.sample(range(1,20),3)])
        else:
            n = rng.randint(2,6)
            edges = []
            balances = [0]*n
            for _ in range(rng.randint(3,9)):
                upper = rng.randint(0,3)
                e = dict(u=rng.randrange(n),v=rng.randrange(n),lower=rng.randint(0,upper),upper=upper,cost=rng.randint(-5,5))
                edges.append(e)
                value = rng.randint(e['lower'], e['upper'])
                balances[e['u']] += value
                balances[e['v']] -= value
            if case % 3 == 0:
                balances[0] += 1
                balances[-1] -= 1
            data = dict(n=n,edges=edges,balance=balances)
        result.append(data)
    return result


def cases(name, hidden=False):
    # Seeds and cases are frozen before scored calls; runner never places this
    # module or hidden expected values inside model-accessible workspaces.
    data = generate(name, 773177 if hidden else 4103, 60 if hidden else 4)
    return [{'input': item, 'expected': ORACLES[name](item)} for item in data]
