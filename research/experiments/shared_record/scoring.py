"""Score candidate `solve(instance)` functions in a private macOS sandbox.

One subprocess per instance, bounded by CPU time rather than wall time, so a
busy host cannot turn a fast candidate into a timeout. An invalid, crashing or
over-budget instance counts as the family's fallback cost.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

from problems import FAMILIES

CPU_SECONDS_PER_INSTANCE = 2
CPU_LIMIT = CPU_SECONDS_PER_INSTANCE + 1  # Hard kill one second past the declared budget.
WALL_LIMIT = 40  # Generous: CPU time is the real limit; wall only catches a blocked process.
PROBE_CPU_LIMIT = 20
OUTPUT_LIMIT = 6000

SOLVE_RUNNER = '''import json, sys, resource, time
resource.setrlimit(resource.RLIMIT_CPU, (%d, %d))
data = json.load(sys.stdin)
namespace = {}
try:
    exec(compile(data["source"], "candidate.py", "exec"), namespace)
    started = time.process_time()
    solution = namespace["solve"](data["instance"])
    print(json.dumps({"solution": solution, "cpu": time.process_time() - started}))
except BaseException as error:
    print(json.dumps({"error": type(error).__name__ + ": " + str(error)[:500]}))
''' % (CPU_LIMIT, CPU_LIMIT)

PROBE_RUNNER = '''import json, sys, resource
resource.setrlimit(resource.RLIMIT_CPU, (%d, %d))
data = json.load(sys.stdin)
INSTANCES = data["instances"]
namespace = {"INSTANCES": INSTANCES, "__name__": "__main__"}
exec(compile(data["source"], "probe.py", "exec"), namespace)
''' % (PROBE_CPU_LIMIT, PROBE_CPU_LIMIT)


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
                                  input=json.dumps(payload), text=True, capture_output=True,
                                  cwd=root, env=env, timeout=wall)
        except subprocess.TimeoutExpired:
            return None


def probe(source, instances):
    """Free-form experimentation; returns captured output, never a score."""
    result = _sandboxed(PROBE_RUNNER, {'source': source, 'instances': instances}, WALL_LIMIT)
    if result is None:
        return {'error': f'probe exceeded the {PROBE_CPU_LIMIT} CPU / {WALL_LIMIT} wall second limit'}
    out = result.stdout[-OUTPUT_LIMIT:]
    err = result.stderr[-OUTPUT_LIMIT:]
    return {'stdout': out, 'stderr': err, 'exit_code': result.returncode,
            'truncated': len(result.stdout) > OUTPUT_LIMIT or len(result.stderr) > OUTPUT_LIMIT}


def score_instance(family, source, instance):
    problem = FAMILIES[family]
    fallback_cost = problem.cost(instance, problem.fallback(instance))
    result = _sandboxed(SOLVE_RUNNER, {'source': source, 'instance': instance}, WALL_LIMIT)
    record = {'fallback_cost': fallback_cost}
    if result is None:
        record.update(valid=False, cost=fallback_cost, error='wall-clock limit exceeded')
        return record
    if result.returncode:
        detail = result.stderr.strip()[-300:] or f'exit code {result.returncode}'
        if result.returncode < 0 or 'CPU time limit' in detail or result.returncode == 137:
            detail = f'CPU limit of {CPU_SECONDS_PER_INSTANCE} s exceeded (killed)'
        record.update(valid=False, cost=fallback_cost, error=detail)
        return record
    try:
        value = json.loads(result.stdout)
    except ValueError:
        record.update(valid=False, cost=fallback_cost, error='candidate output was not JSON')
        return record
    if 'error' in value:
        record.update(valid=False, cost=fallback_cost, error=value['error'])
        return record
    cpu = value.get('cpu')
    record['cpu_seconds'] = cpu
    if cpu is not None and cpu > CPU_SECONDS_PER_INSTANCE:
        record.update(valid=False, cost=fallback_cost, error=f'solve used {cpu:.2f} CPU s, over the {CPU_SECONDS_PER_INSTANCE} s budget')
        return record
    try:
        problem.validate(instance, value['solution'])
    except (ValueError, TypeError, KeyError, IndexError) as error:
        record.update(valid=False, cost=fallback_cost, error='invalid solution: ' + str(error)[:300])
        return record
    record.update(valid=True, cost=problem.cost(instance, value['solution']))
    return record


def score(family, source, instances, reference):
    """Per-instance records plus the normalized total (lower is better; 1.0 = reference)."""
    records = [score_instance(family, source, instance) for instance in instances]
    total = sum(r['cost'] for r in records)
    return {'instances': records, 'total_cost': total, 'reference_total': sum(reference),
            'normalized': total / sum(reference), 'valid_instances': sum(r['valid'] for r in records),
            'source_sha256': hashlib.sha256(source.encode()).hexdigest()}


def public_view(result, reference):
    """What the agent sees: per-instance cost against the reference, no hidden data."""
    rows = []
    for record, ref in zip(result['instances'], reference):
        row = {'cost': record['cost'], 'reference_cost': ref, 'valid': record['valid']}
        if record.get('cpu_seconds') is not None:
            row['cpu_seconds'] = round(record['cpu_seconds'], 3)
        if record.get('error'):
            row['error'] = record['error']
        rows.append(row)
    return {'public_normalized_score': round(result['normalized'], 6), 'instances': rows}
