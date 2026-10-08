"""Eight heuristic-design problem families with a graded, lower-is-better cost.

Each family defines a seeded instance generator, a fixed reference heuristic,
a validator, a cost function and a trivial fallback. A candidate's normalized
score on an instance set is its total cost divided by the reference's total
cost; an invalid or timed-out instance counts as the fallback's cost.

Instance sizes are chosen so that 1.5 CPU seconds of plain Python cannot
converge: at the first calibration's sizes (about a tenth of these) every
agent reached the same hidden score. `shape` states the generator so agents
can target the distribution rather than the five public instances.
"""
import hashlib
import math
import random

DEVELOPMENT = ('maxcut', 'vertex_cover')
HELD_OUT = ('tsp', 'coloring', 'unrelated_machines', 'mkp', 'setcover', 'qap')
PUBLIC_INSTANCES, HIDDEN_INSTANCES = 5, 10


def seeded(study_seed, family, split, index):
    text = f'{study_seed}/{family}/{split}/{index}'
    return int.from_bytes(hashlib.sha256(text.encode()).digest()[:8], 'big')


def _graph(rng, n, p, weighted):
    edges = []
    for i in range(n):
        for j in range(i + 1, n):
            if rng.random() < p:
                edges.append([i, j, rng.randint(1, 10)] if weighted else [i, j])
    return edges


def _check_permutation(solution, n):
    if not isinstance(solution, list) or len(solution) != n or sorted(solution) != list(range(n)):
        raise ValueError('solution must be a permutation of range(n)')


def _check_int_list(solution, n, upper):
    if not isinstance(solution, list) or len(solution) != n:
        raise ValueError(f'solution must be a list of {n} integers')
    for value in solution:
        if type(value) is not int or value < 0 or value >= upper:
            raise ValueError(f'solution entries must be integers in [0, {upper})')


def _check_index_set(solution, n):
    if not isinstance(solution, list) or any(type(v) is not int or v < 0 or v >= n for v in solution):
        raise ValueError(f'solution must be a list of distinct indices in [0, {n})')
    if len(set(solution)) != len(solution):
        raise ValueError('solution indices must be distinct')


# --- Development families -------------------------------------------------

class MaxCut:
    name = 'maxcut'
    title = 'Maximum cut on a weighted graph'
    description = (
        'instance = {"n": int, "edges": [[u, v, w], ...]} with undirected weighted edges. '
        'Return a list of n integers in {0, 1} assigning each vertex to a side. '
        'Cost = total edge weight minus the weight of edges crossing the cut (uncut weight). '
        'Reference: greedy assignment then single-flip local search.')
    shape = 'n uniform in [580, 620]; each pair is an edge with probability 0.05; integer weights uniform in [1, 10].'

    def generate(self, rng):
        n = rng.randint(580, 620)
        return {'n': n, 'edges': _graph(rng, n, 0.05, True)}

    def validate(self, instance, solution):
        _check_int_list(solution, instance['n'], 2)

    def cost(self, instance, solution):
        return float(sum(w for u, v, w in instance['edges'] if solution[u] == solution[v]))

    def fallback(self, instance):
        return [0] * instance['n']

    def reference(self, instance):
        n, edges = instance['n'], instance['edges']
        adjacent = [[] for _ in range(n)]
        for u, v, w in edges:
            adjacent[u].append((v, w))
            adjacent[v].append((u, w))
        side = [-1] * n
        for v in range(n):
            gain0 = sum(w for u, w in adjacent[v] if side[u] == 1)
            gain1 = sum(w for u, w in adjacent[v] if side[u] == 0)
            side[v] = 0 if gain0 >= gain1 else 1
        improved = True
        while improved:
            improved = False
            for v in range(n):
                delta = sum(w if side[u] == side[v] else -w for u, w in adjacent[v])
                if delta > 0:
                    side[v] ^= 1
                    improved = True
        return side


class VertexCover:
    name = 'vertex_cover'
    title = 'Minimum vertex cover'
    description = (
        'instance = {"n": int, "edges": [[u, v], ...]} undirected. '
        'Return a list of distinct vertex indices such that every edge has at least one endpoint in the list. '
        'Cost = number of vertices in the cover. Reference: greedy by current degree.')
    shape = 'n uniform in [780, 820]; each pair is an edge with probability 0.01.'

    def generate(self, rng):
        n = rng.randint(780, 820)
        return {'n': n, 'edges': _graph(rng, n, 0.01, False)}

    def validate(self, instance, solution):
        _check_index_set(solution, instance['n'])
        chosen = set(solution)
        for u, v in instance['edges']:
            if u not in chosen and v not in chosen:
                raise ValueError(f'edge ({u}, {v}) is not covered')

    def cost(self, instance, solution):
        return float(len(solution))

    def fallback(self, instance):
        return list(range(instance['n']))

    def reference(self, instance):
        remaining = {tuple(e) for e in instance['edges']}
        cover = []
        while remaining:
            degree = {}
            for u, v in remaining:
                degree[u] = degree.get(u, 0) + 1
                degree[v] = degree.get(v, 0) + 1
            best = max(sorted(degree), key=lambda x: degree[x])
            cover.append(best)
            remaining = {e for e in remaining if best not in e}
        return cover


# --- Held-out families ----------------------------------------------------

class TSP:
    name = 'tsp'
    title = 'Euclidean travelling salesman tour'
    description = (
        'instance = {"points": [[x, y], ...]} with integer coordinates in [0, 1000]. '
        'Return a permutation of range(len(points)) visiting every point once; the tour is closed. '
        'Cost = Euclidean length of the closed tour. Reference: nearest neighbour from point 0, then 2-opt to convergence.')
    shape = 'number of points uniform in [580, 620]; coordinates independent integers uniform in [0, 1000].'

    def generate(self, rng):
        n = rng.randint(580, 620)
        return {'points': [[rng.randint(0, 1000), rng.randint(0, 1000)] for _ in range(n)]}

    def validate(self, instance, solution):
        _check_permutation(solution, len(instance['points']))

    def cost(self, instance, solution):
        pts = instance['points']
        total = 0.0
        for i in range(len(solution)):
            a, b = pts[solution[i]], pts[solution[(i + 1) % len(solution)]]
            total += math.hypot(a[0] - b[0], a[1] - b[1])
        return total

    def fallback(self, instance):
        return list(range(len(instance['points'])))

    def reference(self, instance):
        pts = instance['points']
        n = len(pts)
        dist = [[math.hypot(a[0] - b[0], a[1] - b[1]) for b in pts] for a in pts]
        unvisited = set(range(1, n))
        tour = [0]
        while unvisited:
            last = tour[-1]
            nxt = min(unvisited, key=lambda j: (dist[last][j], j))
            tour.append(nxt)
            unvisited.remove(nxt)
        improved = True
        while improved:
            improved = False
            for i in range(1, n - 1):
                for j in range(i + 1, n):
                    a, b = tour[i - 1], tour[i]
                    c, d = tour[j], tour[(j + 1) % n]
                    if dist[a][b] + dist[c][d] > dist[a][c] + dist[b][d] + 1e-9:
                        tour[i:j + 1] = reversed(tour[i:j + 1])
                        improved = True
        return tour


class Coloring:
    name = 'coloring'
    title = 'Graph colouring with the fewest colours'
    description = (
        'instance = {"n": int, "edges": [[u, v], ...]} undirected. '
        'Return a list of n non-negative integer colours such that no edge joins two vertices of the same colour. '
        'Cost = number of distinct colours used. Reference: DSATUR.')
    shape = 'n uniform in [280, 320]; each pair is an edge with probability 0.1.'

    def generate(self, rng):
        n = rng.randint(280, 320)
        return {'n': n, 'edges': _graph(rng, n, 0.1, False)}

    def validate(self, instance, solution):
        _check_int_list(solution, instance['n'], instance['n'])
        for u, v in instance['edges']:
            if solution[u] == solution[v]:
                raise ValueError(f'edge ({u}, {v}) has both ends coloured {solution[u]}')

    def cost(self, instance, solution):
        return float(len(set(solution)))

    def fallback(self, instance):
        return list(range(instance['n']))

    def reference(self, instance):
        n = instance['n']
        adjacent = [set() for _ in range(n)]
        for u, v in instance['edges']:
            adjacent[u].add(v)
            adjacent[v].add(u)
        colour = [-1] * n
        for _ in range(n):
            best, key = None, None
            for v in range(n):
                if colour[v] >= 0:
                    continue
                saturation = len({colour[u] for u in adjacent[v] if colour[u] >= 0})
                candidate = (saturation, len(adjacent[v]), -v)
                if key is None or candidate > key:
                    best, key = v, candidate
            used = {colour[u] for u in adjacent[best] if colour[u] >= 0}
            c = 0
            while c in used:
                c += 1
            colour[best] = c
        return colour


class UnrelatedMachines:
    name = 'unrelated_machines'
    title = 'Unrelated parallel machine scheduling (makespan)'
    description = (
        'instance = {"jobs": int, "machines": int, "time": [[t_j_m, ...], ...]} where time[j][m] is the processing '
        'time of job j on machine m. Return a list assigning each job an integer machine index. '
        'Cost = makespan, the largest total load on any machine. '
        'Reference: list scheduling in job order, each job to the machine with the least resulting load.')
    shape = 'jobs uniform in [290, 310]; 12 machines; processing times independent integers uniform in [1, 100].'

    def generate(self, rng):
        jobs, machines = rng.randint(290, 310), 12
        time = [[rng.randint(1, 100) for _ in range(machines)] for _ in range(jobs)]
        return {'jobs': jobs, 'machines': machines, 'time': time}

    def validate(self, instance, solution):
        _check_int_list(solution, instance['jobs'], instance['machines'])

    def cost(self, instance, solution):
        load = [0] * instance['machines']
        for j, m in enumerate(solution):
            load[m] += instance['time'][j][m]
        return float(max(load))

    def fallback(self, instance):
        return [0] * instance['jobs']

    def reference(self, instance):
        load = [0] * instance['machines']
        assignment = []
        for j in range(instance['jobs']):
            m = min(range(instance['machines']), key=lambda m: (load[m] + instance['time'][j][m], m))
            load[m] += instance['time'][j][m]
            assignment.append(m)
        return assignment


class MKP:
    name = 'mkp'
    title = 'Multidimensional 0/1 knapsack'
    description = (
        'instance = {"values": [v_i], "weights": [[w_i_d, ...], ...], "capacity": [c_d, ...]} with weights[i][d] the '
        'weight of item i in dimension d. Return a list of distinct chosen item indices whose total weight in every '
        'dimension is at most that dimension\'s capacity. Cost = total value of all items minus the value chosen '
        '(unselected value; lower is better). Reference: greedy by value over the sum of weight/capacity ratios, skipping infeasible items.')
    shape = ('items uniform in [290, 310]; 10 dimensions; weights integers uniform in [1, 50]; values integers uniform in [10, 100]; '
             'each capacity is 0.3 times that dimension\'s total weight.')

    def generate(self, rng):
        n, d = rng.randint(290, 310), 10
        weights = [[rng.randint(1, 50) for _ in range(d)] for _ in range(n)]
        values = [rng.randint(10, 100) for _ in range(n)]
        capacity = [int(0.3 * sum(weights[i][k] for i in range(n))) for k in range(d)]
        return {'values': values, 'weights': weights, 'capacity': capacity}

    def validate(self, instance, solution):
        _check_index_set(solution, len(instance['values']))
        for d, cap in enumerate(instance['capacity']):
            if sum(instance['weights'][i][d] for i in solution) > cap:
                raise ValueError(f'capacity exceeded in dimension {d}')

    def cost(self, instance, solution):
        return float(sum(instance['values']) - sum(instance['values'][i] for i in solution))

    def fallback(self, instance):
        return []

    def reference(self, instance):
        values, weights, capacity = instance['values'], instance['weights'], instance['capacity']
        n = len(values)
        order = sorted(range(n), key=lambda i: (-values[i] / sum(weights[i][d] / capacity[d] for d in range(len(capacity))), i))
        load = [0] * len(capacity)
        chosen = []
        for i in order:
            if all(load[d] + weights[i][d] <= capacity[d] for d in range(len(capacity))):
                for d in range(len(capacity)):
                    load[d] += weights[i][d]
                chosen.append(i)
        return sorted(chosen)


class SetCover:
    name = 'setcover'
    title = 'Weighted set cover'
    description = (
        'instance = {"universe": int, "sets": [[elements...], ...], "cost": [c_s, ...]} where elements are integers in '
        'range(universe). Return a list of distinct set indices whose union is the whole universe. '
        'Cost = sum of the chosen sets\' costs. Reference: greedy by cost per newly covered element.')
    shape = ('universe size uniform in [580, 620]; 450 sets, each a uniform sample of between 4 and 30 elements, '
             'with any uncovered element added to a random set; set costs integers uniform in [1, 20].')

    def generate(self, rng):
        universe, count = rng.randint(580, 620), 450
        sets = [sorted(rng.sample(range(universe), rng.randint(4, 30))) for _ in range(count)]
        covered = set().union(*map(set, sets))
        for element in range(universe):
            if element not in covered:
                sets[rng.randrange(count)].append(element)
        sets = [sorted(set(s)) for s in sets]
        return {'universe': universe, 'sets': sets, 'cost': [rng.randint(1, 20) for _ in range(count)]}

    def validate(self, instance, solution):
        _check_index_set(solution, len(instance['sets']))
        covered = set()
        for s in solution:
            covered.update(instance['sets'][s])
        if len(covered) != instance['universe']:
            raise ValueError('chosen sets do not cover the universe')

    def cost(self, instance, solution):
        return float(sum(instance['cost'][s] for s in solution))

    def fallback(self, instance):
        return list(range(len(instance['sets'])))

    def reference(self, instance):
        uncovered = set(range(instance['universe']))
        chosen = []
        while uncovered:
            best = min(range(len(instance['sets'])),
                       key=lambda s: (instance['cost'][s] / max(1e-9, len(uncovered & set(instance['sets'][s])))
                                      if uncovered & set(instance['sets'][s]) else math.inf, s))
            chosen.append(best)
            uncovered -= set(instance['sets'][best])
        return chosen


class QAP:
    name = 'qap'
    title = 'Quadratic assignment'
    description = (
        'instance = {"flow": [[f_ij]], "distance": [[d_kl]]} with symmetric n×n integer matrices. '
        'Return a permutation p of range(n) assigning facility i to location p[i]. '
        'Cost = sum over all i, j of flow[i][j] * distance[p[i]][p[j]]. '
        'Reference: identity assignment, then pairwise-swap first-improvement local search to a local optimum.')
    shape = 'n uniform in [68, 72]; flow and distance are symmetric with zero diagonal and off-diagonal integers uniform in [0, 20].'

    def generate(self, rng):
        n = rng.randint(68, 72)

        def symmetric():
            m = [[0] * n for _ in range(n)]
            for i in range(n):
                for j in range(i + 1, n):
                    m[i][j] = m[j][i] = rng.randint(0, 20)
            return m
        return {'flow': symmetric(), 'distance': symmetric()}

    def validate(self, instance, solution):
        _check_permutation(solution, len(instance['flow']))

    def cost(self, instance, solution):
        flow, dist = instance['flow'], instance['distance']
        n = len(flow)
        return float(sum(flow[i][j] * dist[solution[i]][solution[j]] for i in range(n) for j in range(n)))

    def fallback(self, instance):
        return list(range(len(instance['flow'])))

    def reference(self, instance):
        flow, dist = instance['flow'], instance['distance']
        n = len(flow)
        p = list(range(n))
        improved = True
        while improved:
            improved = False
            for i in range(n):
                for j in range(i + 1, n):
                    # Exact cost change of swapping p[i] and p[j]; symmetric matrices with zero diagonals,
                    # so only the cross terms with the other n-2 facilities move.
                    pi, pj, fi, fj = p[i], p[j], flow[i], flow[j]
                    di, dj = dist[pi], dist[pj]
                    delta = 0
                    for k in range(n):
                        if k != i and k != j:
                            pk = p[k]
                            delta += (fi[k] - fj[k]) * (dj[pk] - di[pk])
                    if delta < 0:
                        p[i], p[j] = pj, pi
                        improved = True
        return p


FAMILIES = {cls.name: cls() for cls in (MaxCut, VertexCover, TSP, Coloring, UnrelatedMachines, MKP, SetCover, QAP)}
assert set(FAMILIES) == set(DEVELOPMENT) | set(HELD_OUT)


def instances(study_seed, family, split):
    count = PUBLIC_INSTANCES if split == 'public' else HIDDEN_INSTANCES
    problem = FAMILIES[family]
    return [problem.generate(random.Random(seeded(study_seed, family, split, i))) for i in range(count)]


def reference_costs(family, items):
    problem = FAMILIES[family]
    costs = []
    for instance in items:
        solution = problem.reference(instance)
        problem.validate(instance, solution)
        costs.append(problem.cost(instance, solution))
    return costs
