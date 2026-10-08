#!/usr/bin/env python3
"""Analyze a shared-record study run and package its evidence. Makes no API calls."""
import argparse
import json
from pathlib import Path
import random
import shutil
import statistics
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import select, CONFIG  # noqa: E402

ARMS = ('solo', 'independent', 'shared')
PAIRS = (('shared', 'independent'), ('independent', 'solo'), ('shared', 'solo'))


def load(output):
    output = Path(output)
    manifest = json.loads((output / 'manifest.json').read_text())
    ledger = json.loads((output / 'ledger.json').read_text())
    trials = {}
    for path in sorted((output / 'trials').glob('*/*.json')):
        trial = json.loads(path.read_text())
        trials.setdefault(trial['phase'], {}).setdefault(trial['task'], {})[trial['arm']] = trial
    gate = json.loads((output / 'calibration-gate.json').read_text()) if (output / 'calibration-gate.json').exists() else None
    return manifest, ledger, trials, gate


def arm_spend_at(ledger, prefix, when):
    total = 0.0
    for entry in ledger:
        if not entry['label'].startswith(prefix):
            continue
        settled = entry.get('settled_at', entry['reserved_at'])
        if settled <= when:
            total += entry.get('cost', entry['reserved'])
    return total


def candidates_of(trial):
    return [c for a in trial['agents'] for c in a['candidates']]


def matched(trial, ledger, levels):
    prefix = f"{trial['phase']}/{trial['task']}/{trial['arm']}/"
    rows = {}
    for level in levels:
        eligible = [c for c in candidates_of(trial) if arm_spend_at(ledger, prefix, c['time']) <= level + 1e-12]
        chosen = select(eligible)
        rows[f'{level:.2f}'] = None if chosen is None else dict(hidden=chosen['hidden']['normalized'],
                                                                public=chosen['public']['normalized'],
                                                                agent=chosen['agent'], candidates=len(eligible))
    return rows


def bootstrap(diffs, seed=0, rounds=10000):
    if len(diffs) < 2:
        return None
    rng = random.Random(seed)
    means = sorted(statistics.fmean(rng.choices(diffs, k=len(diffs))) for _ in range(rounds))
    return [means[int(0.025 * rounds)], means[int(0.975 * rounds) - 1]]


def paired(per_task, key):
    out = {}
    for a, b in PAIRS:
        diffs = []
        for task, arms in per_task.items():
            if arms.get(a, {}).get(key) is not None and arms.get(b, {}).get(key) is not None:
                diffs.append(arms[a][key] - arms[b][key])
        if not diffs:
            continue
        out[f'{a}-{b}'] = dict(n=len(diffs), mean=statistics.fmean(diffs), median=statistics.median(diffs),
                               wins=sum(d < -1e-9 for d in diffs), losses=sum(d > 1e-9 for d in diffs),
                               ties=sum(abs(d) <= 1e-9 for d in diffs), bootstrap_95=bootstrap(diffs), diffs=diffs)
    return out


def trial_summary(trial, ledger):
    agents = trial['agents']
    cands = candidates_of(trial)
    selected = select(cands)
    oracle = min(cands, key=lambda c: (c['hidden']['normalized'], c['time'])) if cands else None
    finals = {a['name']: select(a['candidates'])['hidden']['normalized'] for a in agents if a['candidates']}
    initials = {a['name']: a['candidates'][0]['hidden']['normalized'] for a in agents if a['candidates']}
    summary = dict(
        arm=trial['arm'], k=trial['k'], spent_usd=trial['spent_usd'],
        allowance_usd=CONFIG['task_arm_allowance_usd'],
        hidden=None if selected is None else selected['hidden']['normalized'],
        public=None if selected is None else selected['public']['normalized'],
        oracle_hidden=None if oracle is None else oracle['hidden']['normalized'],
        integration_regret=None if selected is None else selected['hidden']['normalized'] - oracle['hidden']['normalized'],
        candidates=len(cands), fully_valid_candidates=sum(c['hidden']['valid_instances'] == CONFIG['hidden_instances'] for c in cands),
        turns=sum(a['turns'] for a in agents), elapsed_seconds=max((a['elapsed_seconds'] or 0) for a in agents),
        transport_failures=sum(1 for a in agents if a['failed']), budget_stops=sum(1 for a in agents for e in a['trace'] if e['event'] == 'budget_stop'),
        nudged=sum(1 for a in agents if a['nudged']),
        agent_final_hidden=finals, agent_initial_hidden=initials,
        complementarity_range=(max(finals.values()) - min(finals.values())) if len(finals) >= 2 else None,
        best_initial_agent=min(initials, key=initials.get) if initials else None,
        best_final_agent=min(finals, key=finals.get) if finals else None,
    )
    allowance = CONFIG['task_arm_allowance_usd']
    summary['matched'] = matched(trial, ledger, [allowance / 3, 2 * allowance / 3, allowance])
    if trial['arm'] == 'shared':
        summary['record'] = record_diagnostics(trial)
    return summary


def record_diagnostics(trial):
    out = dict(findings_posted=0, reads=0, reads_refused=0, reads_with_new_findings=0, findings_delivered=0,
               agents=[], receipts=len(trial.get('goal', {}).get('receipts', [])))
    for a in trial['agents']:
        reads_ok = [r for r in a['reads'] if not r['refused']]
        first_read = next((r['time'] for r in reads_ok if r['returned']), None)
        before = [c for c in a['candidates'] if first_read is None or c['time'] < first_read]
        after = [c for c in a['candidates'] if first_read is not None and c['time'] >= first_read]
        best_before = min((c['hidden']['normalized'] for c in before), default=None)
        best_after = min((c['hidden']['normalized'] for c in after), default=None)
        public_before = min((c['public']['normalized'] for c in before), default=None)
        improved_after = sum(1 for c in after if public_before is not None and c['public']['normalized'] < public_before - 1e-12)
        out['findings_posted'] += len(a['posts'])
        out['reads'] += len(a['reads'])
        out['reads_refused'] += sum(r['refused'] for r in a['reads'])
        out['reads_with_new_findings'] += sum(1 for r in reads_ok if r['returned'])
        out['findings_delivered'] += sum(len(r['returned']) for r in reads_ok)
        out['agents'].append(dict(name=a['name'], posted=len(a['posts']), reads=len(reads_ok),
                                  refused=sum(r['refused'] for r in a['reads']),
                                  candidates_before_first_peer_finding=len(before), candidates_after=len(after),
                                  best_hidden_before=best_before, best_hidden_after=best_after,
                                  candidates_after_improving_own_public_best=improved_after))
    return out


def analyze(output):
    manifest, ledger, trials, gate = load(output)
    result = dict(config=manifest['config'], spent_usd=manifest.get('spent_usd'), gate=gate,
                  ledger=dict(entries=len(ledger), failed=sum(e['status'] == 'failed' for e in ledger),
                              total_usd=sum(e.get('cost', e['reserved']) for e in ledger),
                              by_phase={p: sum(e.get('cost', e['reserved']) for e in ledger if e['label'].startswith(p + '/'))
                                        for p in ('calibration', 'scored')}),
                  phases={})
    for phase, tasks in trials.items():
        per_task = {task: {arm: trial_summary(t, ledger) for arm, t in arms.items()} for task, arms in tasks.items()}
        block = dict(tasks=per_task)
        if phase == 'scored':
            block['paired_full_spend'] = paired(per_task, 'hidden')
            block['paired_oracle'] = paired(per_task, 'oracle_hidden')
            levels = {}
            for level in list(next(iter(per_task.values()))['solo']['matched']):
                flat = {task: {arm: dict(hidden=(arms[arm]['matched'][level] or {}).get('hidden')) for arm in arms}
                        for task, arms in per_task.items()}
                levels[level] = paired(flat, 'hidden')
            block['paired_matched_spend'] = levels
            block['arm_totals'] = {arm: dict(spent_usd=sum(a[arm]['spent_usd'] for a in per_task.values()),
                                             mean_hidden=statistics.fmean(a[arm]['hidden'] for a in per_task.values() if a[arm]['hidden'] is not None),
                                             mean_elapsed_seconds=statistics.fmean(a[arm]['elapsed_seconds'] for a in per_task.values()),
                                             transport_failures=sum(a[arm]['transport_failures'] for a in per_task.values()))
                                   for arm in ARMS if all(arm in a for a in per_task.values())}
        result['phases'][phase] = block
    return result


def table(result):
    lines = []
    for phase, block in result['phases'].items():
        lines.append(f'\n## {phase}\n')
        lines.append('| Task | Arm | Selected hidden | Public | Oracle hidden | Spent $ | Turns | Elapsed s |')
        lines.append('| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |')
        for task, arms in block['tasks'].items():
            for arm, s in arms.items():
                h = '—' if s['hidden'] is None else f"{s['hidden']:.4f}"
                p = '—' if s['public'] is None else f"{s['public']:.4f}"
                o = '—' if s['oracle_hidden'] is None else f"{s['oracle_hidden']:.4f}"
                lines.append(f"| {task} | {arm} | {h} | {p} | {o} | {s['spent_usd']:.3f} | {s['turns']} | {s['elapsed_seconds']:.0f} |")
        if 'paired_full_spend' in block:
            lines.append('\nPaired differences in selected hidden score (negative favours the first arm):\n')
            lines.append('| Contrast | n | Mean | Median | Wins | Losses | Ties | 95% bootstrap |')
            lines.append('| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |')
            for name, p in block['paired_full_spend'].items():
                ci = p['bootstrap_95']
                lines.append(f"| {name} | {p['n']} | {p['mean']:+.4f} | {p['median']:+.4f} | {p['wins']} | {p['losses']} | {p['ties']} | "
                             + ('—' if ci is None else f'[{ci[0]:+.4f}, {ci[1]:+.4f}]') + ' |')
            lines.append('\nAt matched arm spend (selected hidden score among candidates produced within that spend):\n')
            lines.append('| Spend ≤ | Contrast | n | Mean | Wins | Losses |')
            lines.append('| --- | --- | ---: | ---: | ---: | ---: |')
            for level, pairs in block['paired_matched_spend'].items():
                for name, p in pairs.items():
                    lines.append(f"| ${level} | {name} | {p['n']} | {p['mean']:+.4f} | {p['wins']} | {p['losses']} |")
    return '\n'.join(lines)


def package(output, evidence):
    output, evidence = Path(output), Path(evidence)
    evidence.mkdir(parents=True, exist_ok=True)
    for name in ('manifest.json', 'ledger.json', 'calibration-gate.json', 'calibration-gate-preregistered-rule.json'):
        if (output / name).exists():
            shutil.copy2(output / name, evidence / name)
    for path in sorted(output.glob('daemon-events*.jsonl')):  # One daemon log per launch of the same output directory.
        shutil.copy2(path, evidence / path.name)
    for path in sorted((output / 'trials').glob('*/*.json')):
        destination = evidence / 'trials' / path.parent.name / path.name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, destination)
    # Trials voided by infrastructure failure and rerun are kept beside the live ones, never analyzed as results.
    if (output / 'voided').exists():
        shutil.copytree(output / 'voided', evidence / 'voided', dirs_exist_ok=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--evidence', type=Path)
    args = parser.parse_args()
    result = analyze(args.output)
    destination = (args.evidence or args.output) / 'analysis.json'
    if args.evidence:
        package(args.output, args.evidence)
    destination.write_text(json.dumps(result, indent=1, sort_keys=True) + '\n')
    print(table(result))
    print(f"\nTotal ledger: ${result['ledger']['total_usd']:.4f}; by phase: " +
          ', '.join(f'{p} ${v:.4f}' for p, v in result['ledger']['by_phase'].items()))


if __name__ == '__main__':
    main()
