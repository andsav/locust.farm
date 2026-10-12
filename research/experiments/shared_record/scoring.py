"""Public candidate evaluation and the public-only selector.

Every repeated evaluation is retained. Hidden evaluation is invoked only by
``analyze.py`` after the complete live archive has been sealed. This module
knows no hidden pack or run configuration and makes no model calls.
"""
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile

from problems import FAMILIES

CPU_SECONDS_PER_INSTANCE = 2
WALL_LIMIT = 40  # Explicit blocked-process limit, frozen with the evaluator.
PROBE_CPU_LIMIT = 20

SOLVE_RUNNER = '''import json, sys, resource, time, random
from contextlib import redirect_stdout
from io import StringIO
data = json.load(sys.stdin)
resource.setrlimit(resource.RLIMIT_CPU, (data["hard_cpu_limit"], data["hard_cpu_limit"]))
random.seed(data["seed"])
namespace = {"__name__": "__candidate__"}
output = StringIO()
try:
    with redirect_stdout(output):
        exec(compile(data["source"], "candidate.py", "exec"), namespace)
        started = time.process_time()
        solution = namespace["solve"](data["instance"])
        cpu = time.process_time() - started
    print(json.dumps({"solution": solution, "cpu": cpu, "stdout": output.getvalue()}, allow_nan=False))
except BaseException as error:
    print(json.dumps({"error": type(error).__name__ + ": " + str(error), "stdout": output.getvalue()}))
'''

PROBE_RUNNER = '''import json, sys, resource
from contextlib import redirect_stdout
data = json.load(sys.stdin)
resource.setrlimit(resource.RLIMIT_CPU, (data["cpu_limit"], data["cpu_limit"]))
namespace = {"INSTANCES": data["instances"], "__name__": "__main__"}
exec(compile(data["source"], "probe.py", "exec"), namespace)
'''


def _sandboxed(runner, payload, wall):
    with tempfile.TemporaryDirectory(prefix='lsr-') as directory:
        root = str(Path(directory).resolve())
        guard = ('(version 1)(allow default)(deny network*)'
                 '(deny file-read* (subpath "/Users") (subpath "/private/tmp")'
                 '(subpath "/private/var/folders"))'
                 '(allow file-read* (subpath ' + json.dumps(root) + '))'
                 '(deny file-write*)(allow file-write* (subpath ' + json.dumps(root) + '))'
                 '(deny process-fork)')
        script = Path(root) / 'runner.py'
        script.write_text(runner)
        env = {'PATH': '/usr/bin:/bin', 'HOME': root, 'TMPDIR': root, 'LANG': 'en_US.UTF-8'}
        try:
            return subprocess.run(['/usr/bin/sandbox-exec', '-p', guard, sys.executable, '-I', str(script)],
                                  input=json.dumps(payload, allow_nan=False), text=True, capture_output=True,
                                  cwd=root, env=env, timeout=wall)
        except subprocess.TimeoutExpired:
            return None


def probe(source, instances, *, cpu_seconds=PROBE_CPU_LIMIT, wall_seconds=WALL_LIMIT):
    """Free-form public experimentation with complete captured output."""
    _limits(cpu_seconds, wall_seconds)
    result = _sandboxed(PROBE_RUNNER, {'source': source, 'instances': instances,
                                     'cpu_limit': math.ceil(cpu_seconds)}, wall_seconds)
    if result is None:
        return {'error': f'probe exceeded the {cpu_seconds} CPU / {wall_seconds} wall second limit'}
    return {'stdout': result.stdout, 'stderr': result.stderr, 'exit_code': result.returncode}


def _limits(cpu_seconds, wall_seconds):
    if not math.isfinite(cpu_seconds) or cpu_seconds <= 0:
        raise ValueError('CPU allowance must be positive and finite')
    if not math.isfinite(wall_seconds) or wall_seconds <= 0:
        raise ValueError('wall allowance must be positive and finite')


def repetition_seed(seed, instance_index, repetition):
    value = json.dumps([str(seed), instance_index, repetition], separators=(',', ':'))
    return int.from_bytes(hashlib.sha256(value.encode()).digest()[:8], 'big')


def score_instance(family, source, instance, *, seed, cpu_seconds=CPU_SECONDS_PER_INSTANCE,
                   wall_seconds=WALL_LIMIT):
    """Execute one source/instance/repetition; invalid outputs use fallback cost."""
    _limits(cpu_seconds, wall_seconds)
    problem = FAMILIES[family]
    fallback_cost = problem.cost(instance, problem.fallback(instance))
    result = _sandboxed(SOLVE_RUNNER, {'source': source, 'instance': instance, 'seed': seed,
                                    'hard_cpu_limit': math.ceil(cpu_seconds) + 1}, wall_seconds)
    record = {'fallback_cost': fallback_cost, 'seed': seed}
    if result is None:
        return dict(record, valid=False, cost=fallback_cost, error='wall-clock limit exceeded')
    if result.returncode:
        detail = result.stderr.strip() or f'exit code {result.returncode}'
        if result.returncode < 0 or 'CPU time limit' in detail or result.returncode == 137:
            detail = f'CPU limit of {cpu_seconds} s exceeded (killed)'
        return dict(record, valid=False, cost=fallback_cost, error=detail)
    try:
        value = json.loads(result.stdout)
    except ValueError:
        return dict(record, valid=False, cost=fallback_cost, error='candidate output was not JSON',
                    stdout=result.stdout, stderr=result.stderr)
    if not isinstance(value, dict):
        return dict(record, valid=False, cost=fallback_cost, error='candidate output was not an object')
    record['stdout'] = value.get('stdout', '')
    if 'error' in value:
        return dict(record, valid=False, cost=fallback_cost, error=value['error'])
    cpu = value.get('cpu')
    if not isinstance(cpu, (float, int)) or not math.isfinite(cpu) or cpu < 0:
        return dict(record, valid=False, cost=fallback_cost, error='invalid CPU measurement')
    record['cpu_seconds'] = cpu
    if cpu > cpu_seconds:
        return dict(record, valid=False, cost=fallback_cost,
                    error=f'solve used {cpu:.6f} CPU s, over the {cpu_seconds} s budget')
    try:
        problem.validate(instance, value['solution'])
        cost = problem.cost(instance, value['solution'])
        if not math.isfinite(cost) or cost < 0:
            raise ValueError('solution cost must be finite and nonnegative')
    except (ValueError, TypeError, KeyError, IndexError, OverflowError) as error:
        return dict(record, valid=False, cost=fallback_cost, error='invalid solution: ' + str(error))
    solution_json = json.dumps(value['solution'], sort_keys=True, separators=(',', ':'), allow_nan=False)
    return dict(record, valid=True, cost=cost,
                solution_sha256=hashlib.sha256(solution_json.encode()).hexdigest())


def _schedule(items, reference, repetitions):
    if not items or len(items) != len(reference):
        raise ValueError('instances and references must be nonempty and equally sized')
    if type(repetitions) is not int or repetitions < 1:
        raise ValueError('evaluation repetitions must be a positive integer')
    if any(not math.isfinite(x) or x < 0 for x in reference) or sum(reference) <= 0:
        raise ValueError('reference costs must be finite, nonnegative and have a positive total')
    return [(index, repeat) for index in range(len(items)) for repeat in range(repetitions)]


def _aggregate(records, reference, repetitions, source_sha256=None):
    reference_total = sum(reference) * repetitions
    total = sum(row['cost'] for row in records)
    return {'instances': records, 'total_cost': total, 'reference_total': reference_total,
            'normalized': total / reference_total,
            'valid_instances': sum(bool(row['valid']) for row in records),
            'total_instances': len(records), 'instance_count': len(reference),
            'repetitions': repetitions, 'source_sha256': source_sha256}


def score(family, source, instances, reference, *, repetitions, seed,
          cpu_seconds=CPU_SECONDS_PER_INSTANCE, wall_seconds=WALL_LIMIT):
    """Aggregate every scheduled outcome, without best-run selection.

    Seeds control Python's random module, not CPU scheduling or arbitrary
    entropy read by candidate code. Repetition therefore estimates remaining
    execution noise rather than claiming fully deterministic programs.
    """
    records = []
    for index, repeat in _schedule(instances, reference, repetitions):
        row = score_instance(family, source, instances[index],
                             seed=repetition_seed(seed, index, repeat),
                             cpu_seconds=cpu_seconds, wall_seconds=wall_seconds)
        records.append(dict(row, instance_index=index, repetition=repeat))
    return _aggregate(records, reference, repetitions, hashlib.sha256(source.encode()).hexdigest())


def fallback_score(family, instances, reference, *, repetitions, seed):
    """Outcome for a missing public-valid deliverable; never an omitted arm."""
    problem = FAMILIES[family]
    records = []
    for index, repeat in _schedule(instances, reference, repetitions):
        cost = problem.cost(instances[index], problem.fallback(instances[index]))
        records.append({'instance_index': index, 'repetition': repeat,
                        'seed': repetition_seed(seed, index, repeat), 'valid': False,
                        'cost': cost, 'fallback_cost': cost, 'error': 'no public-valid deliverable'})
    return _aggregate(records, reference, repetitions)


def all_valid(result):
    """A candidate must pass every public instance in every frozen repetition."""
    rows = result['instances']
    total = result['total_instances']
    if total != len(rows) or total <= 0:
        raise ValueError('invalid evaluation row count')
    if result['valid_instances'] != sum(bool(row['valid']) for row in rows):
        raise ValueError('invalid evaluation validity count')
    if not math.isfinite(result['normalized']):
        raise ValueError('nonfinite normalized score')
    return result['valid_instances'] == total


def select_public(candidates):
    """Valid-first, unrounded public cost, then source hash; hidden data unused."""
    eligible = [candidate for candidate in candidates if all_valid(candidate['public'])]
    if not eligible:
        return None
    return min(eligible, key=lambda candidate: (candidate['public']['normalized'],
                                                candidate['public']['source_sha256']))


def public_view(result, reference):
    """The complete public evidence, without hidden fields or score rounding."""
    rows = []
    for record in result['instances']:
        row = {'instance_index': record['instance_index'], 'repetition': record['repetition'],
               'cost': record['cost'], 'reference_cost': reference[record['instance_index']],
               'valid': record['valid']}
        for key in ('cpu_seconds', 'error', 'stdout'):
            if key in record:
                row[key] = record[key]
        rows.append(row)
    return {'public_normalized_score': result['normalized'], 'instances': rows}
