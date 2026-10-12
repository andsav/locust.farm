#!/usr/bin/env python3
"""Score sealed outputs offline and analyze the immutable information-policy archive.

There is no dependency on the live runner, model API or mutable CONFIG. Every
assigned block/arm is retained, including failures with no valid deliverable.
The current archive format is the only supported format; historical studies
remain reproducible at their original commits.
"""
import argparse
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import platform
import random
import shutil
import statistics
import sys

import problems
import protocol
import scoring

SCHEMA = protocol.SCHEMA


def _read(path):
    return json.loads(Path(path).read_text())


def _trial_key(block, arm):
    return f'{block["id"]}/{arm}'


def _entries(ledger):
    by_id = {}
    for item in ledger:
        if item['id'] in by_id:
            raise ValueError('duplicate ledger request ID')
        if not math.isfinite(item['reserved']) or item['reserved'] < 0:
            raise ValueError('invalid request reservation')
        if not math.isfinite(item['reserved_at']):
            raise ValueError('invalid request dispatch time')
        if item.get('cost') is not None and (not math.isfinite(item['cost']) or item['cost'] < 0):
            raise ValueError('invalid settled request cost')
        if item.get('settled_at') is not None and item['settled_at'] < item['reserved_at']:
            raise ValueError('request settled before dispatch')
        by_id[item['id']] = item
    return by_id


def _load_archive(output):
    output = Path(output)
    manifest = protocol.verify_seal(output)
    ledger = _read(output / 'ledger.json')
    by_id = _entries(ledger)
    trials = {}
    request_blocks = {}
    for block in manifest['blocks']:
        for arm in manifest['arms']:
            key = _trial_key(block, arm)
            trial = _read(output / 'trials' / f'{key}.json')
            for field, expected in (('block_id', block['id']), ('family', block['family']),
                                    ('pack', block['pack']), ('repetition', block['repetition']),
                                    ('attempt', block['attempt']), ('phase', block['phase']), ('arm', arm)):
                if trial.get(field) != expected:
                    raise ValueError(f'trial identity mismatch for {key}: {field}')
            ids = trial['request_ids']
            if len(ids) != len(set(ids)):
                raise ValueError(f'duplicated request dependency in {key}')
            if set(ids) - set(by_id):
                raise ValueError(f'missing ledger dependency in {key}')
            for request_id in ids:
                previous = request_blocks.setdefault(request_id, block['id'])
                if previous != block['id']:
                    raise ValueError('a model request cannot be shared across independent blocks')
            candidates = trial['candidates']
            if len({candidate['id'] for candidate in candidates}) != len(candidates):
                raise ValueError(f'duplicate candidate ID in {key}')
            for candidate in candidates:
                actual_hash = hashlib.sha256(candidate['source'].encode()).hexdigest()
                if candidate['sha256'] != actual_hash or candidate['public']['source_sha256'] != actual_hash:
                    raise ValueError(f'candidate source hash mismatch in {key}')
                scoring.all_valid(candidate['public'])
            chosen = scoring.select_public(candidates)
            selected = chosen['id'] if chosen else None
            if trial['selected'] != selected:
                raise ValueError(f'sealed selector does not match public-only rule in {key}')
            trials[key] = trial
    return manifest, ledger, by_id, trials


def _hidden_link(output, manifest):
    output = Path(output)
    return dict(schema=SCHEMA, seal_sha256=protocol.file_digest(output / 'seal.json'),
                manifest_sha256=protocol.file_digest(output / 'manifest.json'),
                evaluation=manifest['evaluation'])


def _load_hidden(output, manifest):
    output = Path(output)
    hidden = _read(output / 'hidden.json')
    for key, expected in _hidden_link(output, manifest).items():
        if hidden.get(key) != expected:
            raise ValueError('hidden scores do not belong to this sealed archive: ' + key)
    proof = _read(output / 'hidden-seal.json')
    if (proof.get('hidden_sha256') != protocol.file_digest(output / 'hidden.json')
            or proof.get('seal_sha256') != hidden['seal_sha256']
            or proof.get('manifest_sha256') != hidden['manifest_sha256']):
        raise ValueError('hidden score file differs from its completion seal')
    expected_files = proof.get('journal_files')
    actual_files = _journal_files(output)
    if not expected_files or actual_files != expected_files:
        raise ValueError('hidden evaluation journal differs from its completion seal')
    return hidden


def _scorer_provenance(manifest):
    expected = manifest['runtime']['source_sha256']
    actual = {}
    for module in (problems, scoring):
        path = Path(module.__file__).resolve()
        name = str(path.relative_to(protocol.ROOT))
        actual[name] = protocol.file_digest(path)
        if expected.get(name) != actual[name]:
            raise ValueError('offline scorer differs from the frozen source: ' + name)
    environment = dict(python=sys.version, executable=sys.executable, platform=platform.platform())
    for key, value in environment.items():
        if key in manifest['runtime'] and manifest['runtime'][key] != value:
            raise ValueError('offline evaluation environment differs from freeze: ' + key)
    return dict(source_sha256=actual, **environment)


def _journal_files(output):
    output = Path(output)
    return {str(path.relative_to(output)): protocol.file_digest(path)
            for path in sorted((output / 'hidden-journal').glob('*.json'))}


def _save_once(path, value):
    """Create an enduring start marker; even a partial write blocks a rerun."""
    with Path(path).open('x') as target:
        target.write(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + '\n')
        target.flush()
        os.fsync(target.fileno())


def _open_journal(output, manifest, trials, provenance):
    directory = output / 'hidden-journal'
    directory.mkdir(exist_ok=True)
    sources = {}
    for block in manifest['blocks']:
        for arm in manifest['arms']:
            for candidate in trials[_trial_key(block, arm)]['candidates']:
                identity = [block['family'], block['pack'], candidate['sha256']]
                sources[protocol.digest(identity)] = identity
    identity = dict(_hidden_link(output, manifest), runtime=provenance, sources=sources)
    identity_path = directory / 'identity.json'
    if identity_path.exists():
        if _read(identity_path) != identity:
            raise ValueError('hidden evaluation journal differs from the frozen scoring schedule')
    else:
        if any(directory.iterdir()):
            raise ValueError('hidden evaluation journal identity is missing; explicit recovery required')
        _save_once(identity_path, identity)
    journal_hash = protocol.file_digest(identity_path)
    allowed = {identity_path}
    completed = {}
    for key, source_identity in sources.items():
        pending = directory / f'{key}.pending.json'
        complete = directory / f'{key}.complete.json'
        allowed.update((pending, complete))
        if not pending.exists():
            if complete.exists():
                raise ValueError('hidden result has no execution marker; explicit recovery required')
            continue
        expected = dict(journal_sha256=journal_hash, identity=source_identity)
        if _read(pending) != expected:
            raise ValueError('hidden execution marker differs from frozen identity')
        if not complete.exists():
            raise ValueError('hidden source evaluation is incomplete; explicit recovery required, automatic rerun refused')
        record = _read(complete)
        score = record['score']
        if (record.get('pending_sha256') != protocol.file_digest(pending)
                or record.get('score_sha256') != protocol.digest(score)
                or score.get('source_sha256') != source_identity[2]):
            raise ValueError('hidden completed evaluation differs from its execution marker')
        scoring.all_valid(score)
        completed[key] = score
    if set(directory.iterdir()) - allowed:
        raise ValueError('unrecognized or partial hidden journal file; explicit recovery required')
    return directory, journal_hash, completed


def offline_hidden(output):
    """Evaluate every source only after every assigned output and selector is sealed.

    Existing completed scores are reused, never silently replaced by a more
    favorable execution. Duplicate sources share one frozen repetition record
    within a family/pack. Hidden results stay outside the sealed live trials.
    """
    output = Path(output)
    manifest, _, _, trials = _load_archive(output)
    if (output / 'hidden.json').exists() or (output / 'hidden-seal.json').exists():
        return _load_hidden(output, manifest)
    provenance = _scorer_provenance(manifest)
    journal, journal_hash, score_cache = _open_journal(output, manifest, trials, provenance)
    evaluation = manifest['evaluation']
    result = dict(_hidden_link(output, manifest), runtime=provenance, trials={})
    pack_cache = {}
    for block in manifest['blocks']:
        pack_key = (block['family'], block['pack'])
        settings = block['hidden']
        if pack_key not in pack_cache:
            items = problems.instances(settings['seed'], block['family'], 'hidden',
                                       pack=settings['pack'], count=settings['count'])
            if protocol.digest(items) != settings['digest']:
                raise ValueError('hidden instance digest differs from freeze: ' + block['id'])
            reference = problems.reference_costs(block['family'], items)
            eval_seed = protocol.digest([evaluation['seed'], block['family'], block['pack'], 'hidden'])
            fallback = scoring.fallback_score(block['family'], items, reference,
                                               repetitions=evaluation['repetitions'], seed=eval_seed)
            pack_cache[pack_key] = (settings, items, reference, eval_seed, fallback)
        old_settings, items, reference, eval_seed, fallback = pack_cache[pack_key]
        if old_settings != settings:
            raise ValueError('one family/pack has inconsistent hidden settings')
        for arm in manifest['arms']:
            key = _trial_key(block, arm)
            trial = trials[key]
            scores = {}
            for candidate in trial['candidates']:
                identity = [*pack_key, candidate['sha256']]
                cache_key = protocol.digest(identity)
                if cache_key not in score_cache:
                    pending = journal / f'{cache_key}.pending.json'
                    complete = journal / f'{cache_key}.complete.json'
                    _save_once(pending, dict(journal_sha256=journal_hash, identity=identity))
                    score = scoring.score(
                        block['family'], candidate['source'], items, reference,
                        repetitions=evaluation['repetitions'], seed=eval_seed,
                        cpu_seconds=evaluation['cpu_seconds'], wall_seconds=evaluation['wall_seconds'])
                    protocol.save(complete, dict(pending_sha256=protocol.file_digest(pending),
                                                 score_sha256=protocol.digest(score), score=score))
                    score_cache[cache_key] = score
                scores[candidate['id']] = score_cache[cache_key]
            result['trials'][key] = dict(selected=trial['selected'], fallback=fallback, candidates=scores)
    # Completed source records survive interruption. An unresolved execution
    # marker requires explicit recovery because its partial outcomes are unknown.
    protocol.save(output / 'hidden.json', result)
    protocol.save(output / 'hidden-seal.json', dict(
        schema=SCHEMA, hidden_sha256=protocol.file_digest(output / 'hidden.json'),
        seal_sha256=result['seal_sha256'], manifest_sha256=result['manifest_sha256'],
        journal_files=_journal_files(output)))
    return result


def request_charge(entry, when=None):
    """Cost known at a stopping cut; pending/failed uncertainty remains reserved."""
    if when is not None and entry['reserved_at'] > when:
        return 0.0
    settled = entry.get('settled_at')
    if (entry.get('cost') is not None and settled is not None
            and (when is None or settled <= when)):
        return entry['cost']
    return entry['reserved']


def arm_spend_at(ledger, request_ids, when):
    """Count every dispatched dependency once, including unsettled peer work."""
    by_id = ledger if isinstance(ledger, dict) else _entries(ledger)
    if set(request_ids) - set(by_id):
        raise ValueError('missing cost dependency')
    return math.fsum(request_charge(by_id[key], when) for key in set(request_ids))


def spending(trial, by_id):
    entries = [by_id[key] for key in trial['request_ids']]
    known = [entry for entry in entries if entry.get('cost') is not None and entry.get('settled_at') is not None]
    uncertain = [entry for entry in entries if entry not in known]
    usage = {}
    for entry in entries:
        for key, value in (entry.get('usage') or {}).items():
            if isinstance(value, (int, float)):
                usage[key] = usage.get(key, 0) + value
    return dict(attributed_cost=math.fsum(request_charge(entry) for entry in entries),
                known_usage_cost=math.fsum(entry['cost'] for entry in known),
                uncertain_reservations=math.fsum(entry['reserved'] for entry in uncertain),
                unknown_cost_requests=len(uncertain),
                requests=len(entries), pending=sum(entry['status'] == 'pending' for entry in entries),
                failed=sum(entry['status'] in ('failed', 'accounting_error') for entry in entries), usage=usage)


def _outcome(trial, hidden, selected):
    return hidden['fallback'] if selected is None else hidden['candidates'][selected['id']]


def matched(trial, by_id, hidden, levels):
    """Select only candidates affordable at their original availability time.

    Later settlements cannot retroactively admit an earlier unaffordable
    candidate. These curves are descriptive; inference is at the full B arm.
    """
    rows = []
    for level in levels:
        eligible = []
        for candidate in trial['candidates']:
            cost = arm_spend_at(by_id, trial['request_ids'], candidate['time'])
            if cost <= level or math.isclose(cost, level, rel_tol=0, abs_tol=1e-12):
                eligible.append(candidate)
        selected = scoring.select_public(eligible)
        outcome = _outcome(trial, hidden, selected)
        rows.append(dict(allowance=level, selected=selected['id'] if selected else None,
                         hidden=outcome['normalized'], eligible_candidates=len(eligible),
                         fallback=selected is None))
    return rows


def _disagreement(selected, hidden):
    """Behavioral comparisons at identical evaluation rows, not score spread alone."""
    pairs = []
    for (agent_a, candidate_a), (agent_b, candidate_b) in itertools.combinations(sorted(selected.items()), 2):
        left = hidden['candidates'][candidate_a['id']]['instances']
        right = hidden['candidates'][candidate_b['id']]['instances']
        if len(left) != len(right):
            raise ValueError('unmatched hidden evaluation schedule')
        if not left:
            continue
        solution_pairs = [(a['solution_sha256'], b['solution_sha256']) for a, b in zip(left, right)
                          if a.get('solution_sha256') and b.get('solution_sha256')]
        pairs.append(dict(agents=[agent_a, agent_b],
                          mean_absolute_cost_difference=statistics.fmean(abs(a['cost'] - b['cost']) for a, b in zip(left, right)),
                          validity_disagreement=statistics.fmean(a['valid'] != b['valid'] for a, b in zip(left, right)),
                          solution_hash_disagreement=(statistics.fmean(a != b for a, b in solution_pairs)
                                                      if solution_pairs else None),
                          exact_source_match=candidate_a['sha256'] == candidate_b['sha256']))
    return pairs


def diagnostics(trial, hidden):
    grouped = {}
    for candidate in trial['candidates']:
        grouped.setdefault(candidate['agent'], []).append(candidate)
    finals = {agent: chosen for agent, candidates in grouped.items()
              if (chosen := scoring.select_public(candidates)) is not None}
    failure_signatures = {}
    for candidate in trial['candidates']:
        for row in hidden['candidates'][candidate['id']]['instances']:
            if not row['valid']:
                signature = str(row.get('error', 'invalid'))
                failure_signatures[signature] = failure_signatures.get(signature, 0) + 1
    result = dict(final_agent_selected={agent: candidate['id'] for agent, candidate in finals.items()},
                  final_behavioral_disagreement=_disagreement(finals, hidden),
                  unique_sources=len({candidate['sha256'] for candidate in trial['candidates']}),
                  hidden_failure_signatures=failure_signatures)
    if 'prefix_candidate_ids' in trial:
        prefix_ids = set(trial['prefix_candidate_ids'])
        if prefix_ids - {candidate['id'] for candidate in trial['candidates']}:
            raise ValueError('prefix references absent candidate')
        before = {agent: chosen for agent, candidates in grouped.items()
                  if (chosen := scoring.select_public([c for c in candidates if c['id'] in prefix_ids])) is not None}
        result['prefix_behavioral_disagreement'] = _disagreement(before, hidden)
        result['agent_changes'] = []
        for agent, old in before.items():
            new = finals.get(agent)
            old_score = hidden['candidates'][old['id']]
            new_score = hidden['candidates'][new['id']] if new else hidden['fallback']
            result['agent_changes'].append(dict(
                agent=agent, before=old['id'], after=new['id'] if new else None,
                hidden_improvement=old_score['normalized'] - new_score['normalized'],
                lost_hidden_validity=scoring.all_valid(old_score) and not scoring.all_valid(new_score)))
    return result


def trial_summary(trial, hidden, by_id, allowance):
    if hidden['selected'] != trial['selected']:
        raise ValueError('hidden scoring selected a different deliverable')
    if set(hidden['candidates']) != {candidate['id'] for candidate in trial['candidates']}:
        raise ValueError('hidden score set differs from sealed candidates')
    selected = scoring.select_public(trial['candidates'])
    outcome = _outcome(trial, hidden, selected)
    oracle = min((item['normalized'] for item in hidden['candidates'].values()), default=hidden['fallback']['normalized'])
    started, finished = trial.get('started'), trial.get('finished')
    branch_elapsed = finished - started if started is not None and finished is not None else None
    prefix_elapsed = trial.get('prefix_elapsed_seconds', 0)
    elapsed = prefix_elapsed + branch_elapsed if branch_elapsed is not None else None
    failures = [agent['failed'] for agent in trial['agents'] if agent.get('failed')]
    if trial.get('error'):
        failures.append(trial['error'])
    return dict(arm=trial['arm'], hidden=outcome['normalized'], selected=trial['selected'],
                public=selected['public']['normalized'] if selected else None,
                fallback=selected is None, hidden_valid=scoring.all_valid(outcome),
                candidates=len(trial['candidates']),
                public_valid_candidates=sum(scoring.all_valid(candidate['public']) for candidate in trial['candidates']),
                oracle_hidden=oracle, selector_regret=outcome['normalized'] - oracle,
                elapsed_seconds=elapsed, branch_elapsed_seconds=branch_elapsed,
                prefix_elapsed_seconds=prefix_elapsed,
                evaluator_cpu_seconds=math.fsum(row.get('cpu_seconds', 0)
                                               for item in hidden['candidates'].values() for row in item['instances']),
                failures=failures,
                spending=spending(trial, by_id),
                checkpoints=matched(trial, by_id, hidden, [allowance / 3, 2 * allowance / 3, allowance]),
                diagnostics=diagnostics(trial, hidden))


def _mean_nested(data):
    return statistics.fmean(statistics.fmean(statistics.fmean(values) for values in packs.values())
                            for packs in data.values())


def _quantile(values, fraction):
    index = (len(values) - 1) * fraction
    lo, hi = math.floor(index), math.ceil(index)
    return values[lo] + (values[hi] - values[lo]) * (index - lo)


def hierarchical_interval(data, *, samples, seed, coverage=0.975):
    """Fixed families; resample packs, then paired repetitions inside each pack."""
    if type(samples) is not int or samples < 1:
        raise ValueError('bootstrap sample count must be positive')
    if not 0 < coverage < 1 or not data:
        raise ValueError('invalid bootstrap coverage or empty contrast')
    rng = random.Random(seed)
    draws = []
    for _ in range(samples):
        family_means = []
        for family in sorted(data):
            packs = data[family]
            keys = sorted(packs)
            pack_means = []
            for key in rng.choices(keys, k=len(keys)):
                values = packs[key]
                pack_means.append(statistics.fmean(rng.choices(values, k=len(values))))
            family_means.append(statistics.fmean(pack_means))
        draws.append(statistics.fmean(family_means))
    draws.sort()
    tail = (1 - coverage) / 2
    return [_quantile(draws, tail), _quantile(draws, 1 - tail)]


def _contrast_specs(arms):
    singles = {kind: [name for name, item in arms.items() if item['kind'] == kind]
               for kind in ('S', 'I', 'R')}
    for kind, names in singles.items():
        if len(names) > 1:
            raise ValueError('ambiguous baseline kind: ' + kind)
    solo, independent, frontier = (singles[kind][0] if singles[kind] else None for kind in ('S', 'I', 'R'))
    peer_arms = [name for name, item in arms.items() if item['kind'] == 'P']
    specs = []
    for peer in peer_arms:
        policy = arms[peer]['policy']
        controls = [name for name, item in arms.items() if item['kind'] == 'C' and item['policy'] == policy]
        if len(controls) != 1:
            raise ValueError('each peer policy requires exactly one matched private control')
        control = controls[0]
        suffix = '' if len(peer_arms) == 1 else ':' + policy
        specs.append(('G_info' + suffix, control, peer, True))
        if independent:
            specs.extend([('G_net' + suffix, independent, peer, True),
                          ('private_review' + suffix, independent, control, False)])
        if frontier:
            specs.append(('frontier_comparison' + suffix, frontier, peer, False))
    if solo and independent:
        specs.append(('independent_vs_solo', solo, independent, False))
    return specs


def _descriptive_reasons(manifest):
    reasons = []
    if manifest['analysis'].get('exploratory'):
        reasons.append('the frozen analysis declares an exploratory study')
    packs = {}
    for block in manifest['blocks']:
        packs.setdefault(block['family'], set()).add(block['pack'])
    if any(len(values) < 2 for values in packs.values()):
        reasons.append('at least one family has fewer than two independent task packs')
    return reasons


def contrasts(manifest, summaries, *, checkpoint=None):
    out = {}
    descriptive_reasons = _descriptive_reasons(manifest)
    for name, first, second, primary in _contrast_specs(manifest['arms']):
        data, block_differences = {}, {}
        for block in manifest['blocks']:
            a, b = (summaries[_trial_key(block, arm)] for arm in (first, second))
            if checkpoint is None:
                delta = a['hidden'] - b['hidden']
            else:
                delta = a['checkpoints'][checkpoint]['hidden'] - b['checkpoints'][checkpoint]['hidden']
            data.setdefault(block['family'], {}).setdefault(block['pack'], []).append(delta)
            block_differences[block['id']] = delta
        family_results = {family: dict(mean=statistics.fmean(statistics.fmean(values) for values in packs.values()),
                                       packs=len(packs), repetitions=sum(map(len, packs.values())))
                          for family, packs in data.items()}
        row = dict(first=first, second=second, mean=_mean_nested(data), families=family_results,
                   block_differences=block_differences, blocks=len(block_differences),
                   positive_blocks=sum(value > 0 for value in block_differences.values()),
                   negative_blocks=sum(value < 0 for value in block_differences.values()),
                   tied_blocks=sum(value == 0 for value in block_differences.values()),
                   descriptive=bool(descriptive_reasons) or checkpoint is not None or not primary
                               or manifest['stage'] != 'scored')
        if primary and checkpoint is None:
            row['interval_97_5'] = hierarchical_interval(
                data, samples=manifest['analysis']['bootstrap_samples'],
                seed=f'{manifest["analysis"]["seed"]}/{name}', coverage=0.975)
            row['interval_note'] = ('Fixed-family pack/repetition percentile bootstrap; two 97.5% marginal intervals '
                                    'give a Bonferroni 95% simultaneous family only for the two frozen scored contrasts. '
                                    'Small-pack intervals can be unstable.')
            row['few_packs'] = any(len(packs) < 4 for packs in data.values())
            if descriptive_reasons:
                row['interval_note'] += (' Descriptive interval only: ' + '; '.join(descriptive_reasons)
                                         + '. It cannot support a confirmatory conclusion.')
        out[name] = row
    return out


def scored_decision(manifest, full):
    useful = manifest['analysis']['useful_effect']
    reasons = _descriptive_reasons(manifest)
    if reasons:
        return dict(verdict='descriptive exploratory pilot; no confirmatory conclusion',
                    useful_effect=useful, descriptive=True, reasons=reasons,
                    caution='Small-pack bootstrap intervals may be unstable or degenerate; report observed effects and failures without threshold-based claims.')
    info, net = full['G_info']['interval_97_5'], full['G_net']['interval_97_5']
    if info[0] > useful and net[0] > useful:
        verdict = 'worthwhile peer-policy benefit at B on the declared known-family distribution'
    elif net[1] < 0:
        verdict = 'net harm at B on the declared known-family distribution'
    elif net[1] < useful:
        verdict = 'a worthwhile net gain is excluded at B by the approximate interval'
    elif info[0] > 0:
        verdict = 'peer information helps matched review; worthwhile net gain at B remains unestablished'
    else:
        verdict = 'inconclusive at B'
    return dict(verdict=verdict, useful_effect=useful, descriptive=False,
                caution='Intervals depend on the frozen bootstrap approximation; no unseen-family or RSI claim.')


def development_selection(manifest, summaries):
    """Apply the preregistered D/F development rule without selecting on scored data."""
    if manifest['stage'] != 'development':
        return None
    independent = [name for name, item in manifest['arms'].items() if item['kind'] == 'I']
    arms = {}
    for name, definition in manifest['arms'].items():
        outcomes, gains, costs, unknown = {}, {}, [], 0
        for block in manifest['blocks']:
            row = summaries[_trial_key(block, name)]
            outcomes.setdefault(block['family'], {}).setdefault(block['pack'], []).append(row['hidden'])
            costs.append(row['spending']['known_usage_cost'])
            unknown += row['spending']['unknown_cost_requests']
            if len(independent) == 1:
                baseline = summaries[_trial_key(block, independent[0])]['hidden']
                gains.setdefault(block['family'], {}).setdefault(block['pack'], []).append(baseline - row['hidden'])
        arms[name] = dict(definition, family_balanced_hidden=_mean_nested(outcomes),
                          family_balanced_gain_over_I=_mean_nested(gains) if gains else None,
                          mean_known_usage_cost=statistics.fmean(costs), unknown_cost_requests=unknown,
                          mean_actual_cost=statistics.fmean(costs) if unknown == 0 else None)
    report = dict(selected_policy=None, selected_arm=None, arms=arms, gain_tie_threshold=0.0025,
                  rule='Larger family-balanced gain over I; below 0.0025 difference use mean actual charged cost; exact cost tie D.',
                  status='incomplete')
    choices = {policy: [name for name, definition in manifest['arms'].items()
                        if definition['kind'] == 'P' and definition['policy'] == policy]
               for policy in ('D', 'F')}
    if len(independent) != 1 or any(len(names) != 1 for names in choices.values()):
        report['reason'] = 'Development selection requires one I arm and one peer arm each for D and F.'
        return report
    delayed, filtered = (arms[choices[policy][0]] for policy in ('D', 'F'))
    delta = delayed['family_balanced_gain_over_I'] - filtered['family_balanced_gain_over_I']
    report['D_minus_F_gain'] = delta
    if abs(delta) >= report['gain_tie_threshold']:
        selected = 'D' if delta > 0 else 'F'
        reason = 'larger family-balanced improvement over I'
    elif delayed['mean_actual_cost'] is None or filtered['mean_actual_cost'] is None:
        report['reason'] = 'Gain difference is below 0.0025, but actual charged cost is unresolved; reservations cannot choose a policy.'
        return report
    elif delayed['mean_actual_cost'] == filtered['mean_actual_cost']:
        selected, reason = 'D', 'gain difference below 0.0025 and exact actual-cost tie selects D'
    else:
        selected = 'D' if delayed['mean_actual_cost'] < filtered['mean_actual_cost'] else 'F'
        reason = 'gain difference below 0.0025; lower mean actual charged cost'
    report.update(selected_policy=selected, selected_arm=choices[selected][0], status='selected', reason=reason)
    return report


def analyze(output):
    """Read only sealed archive data; hidden scoring must already be complete."""
    output = Path(output)
    manifest, ledger, by_id, trials = _load_archive(output)
    hidden = _load_hidden(output, manifest)
    if set(hidden['trials']) != set(trials):
        raise ValueError('hidden trial set differs from assigned arms')
    allowance = manifest['budget']['task_arm_usd']
    summaries = {key: trial_summary(trial, hidden['trials'][key], by_id, allowance)
                 for key, trial in trials.items()}
    full = contrasts(manifest, summaries)
    result = dict(schema=SCHEMA, study_id=manifest['study_id'], stage=manifest['stage'],
                  seal_sha256=hidden['seal_sha256'], manifest_sha256=hidden['manifest_sha256'],
                  hidden_sha256=protocol.file_digest(output / 'hidden.json'),
                  analyzer_sha256=protocol.file_digest(__file__), analysis=manifest['analysis'],
                  endpoint_allowance=allowance, blocks=len(manifest['blocks']), trials=summaries,
                  contrasts=full,
                  checkpoint_contrasts={str(fraction): contrasts(manifest, summaries, checkpoint=index)
                                        for index, fraction in enumerate((1 / 3, 2 / 3, 1))},
                  billing=dict(synthetic=manifest['stage'] == 'scripted',
                               entries=len(ledger), total=math.fsum(request_charge(entry) for entry in ledger),
                               known_usage_cost=math.fsum(entry['cost'] for entry in ledger
                                                         if entry.get('cost') is not None and entry.get('settled_at') is not None),
                               uncertain_reservations=math.fsum(entry['reserved'] for entry in ledger
                                                               if entry.get('cost') is None or entry.get('settled_at') is None)))
    if manifest['stage'] == 'development':
        result['development_selection'] = development_selection(manifest, summaries)
    if manifest['stage'] == 'scored':
        result['decision'] = scored_decision(manifest, full)
    return result


def table(result):
    lines = ['| Block/arm | Selected hidden | Fallback | Attributed cost | Candidates |',
             '| --- | ---: | --- | ---: | ---: |']
    for key, row in sorted(result['trials'].items()):
        lines.append(f'| {key} | {row["hidden"]:.6f} | {row["fallback"]} | '
                     f'{row["spending"]["attributed_cost"]:.6f} | {row["candidates"]} |')
    lines += ['', '| Contrast (positive favors second arm) | Mean | 97.5% interval |',
              '| --- | ---: | --- |']
    for name, row in result['contrasts'].items():
        interval = row.get('interval_97_5')
        text = 'descriptive' if interval is None else f'[{interval[0]:+.6f}, {interval[1]:+.6f}]'
        if interval is not None and row['descriptive']:
            text += ' (descriptive; unstable with few packs)' if row.get('few_packs') else ' (descriptive)'
        lines.append(f'| {name} | {row["mean"]:+.6f} | {text} |')
    if 'development_selection' in result:
        selection = result['development_selection']
        lines += ['', f'Development policy selection: {selection["selected_policy"] or "unresolved"}. '
                  + selection['reason']]
    if 'decision' in result:
        lines += ['', 'Decision: ' + result['decision']['verdict'] + '.']
    return '\n'.join(lines)


def package(output, evidence):
    """Copy the exact sealed archive and offline scores; no legacy conversions."""
    output, evidence = Path(output), Path(evidence)
    manifest = protocol.verify_seal(output)
    _load_hidden(output, manifest)
    paths = set(_read(output / 'seal.json')['files'])
    paths.update(('manifest.json', 'seal.json', 'hidden.json', 'hidden-seal.json'))
    paths.update(_journal_files(output))
    if (output / 'analysis.json').exists():
        paths.add('analysis.json')
    if evidence.exists() and any(evidence.iterdir()):
        raise ValueError('evidence destination must be empty')
    for name in sorted(paths):
        destination = evidence / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(output / name, destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--score-hidden', action='store_true', help='score only after every live output is sealed')
    parser.add_argument('--evidence', type=Path)
    args = parser.parse_args()
    if args.score_hidden:
        offline_hidden(args.output)
    result = analyze(args.output)
    protocol.save(args.output / 'analysis.json', result)
    if args.evidence:
        package(args.output, args.evidence)
    print(table(result))
    label = 'Synthetic accounted cost' if result['billing']['synthetic'] else 'Actual billed or reserved study cost'
    print(f'\n{label}: ${result["billing"]["total"]:.6f}')


if __name__ == '__main__':
    main()
