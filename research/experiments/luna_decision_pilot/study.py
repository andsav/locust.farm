"""Frozen synthetic decision pilot: original cases, exact oracle, and analysis.

Only public_case and instructions are sent to a model. The oracle is local.
Three calls per condition are independent requests, not independent models.
"""

from collections import defaultdict
from fractions import Fraction
import hashlib
import json
import math
import random
import statistics


MODEL = "gpt-6-luna"
ARMS = ("default", "filler", "persona", "procedure")
STYLES = ("base_rate", "evidence", "utility")
COMMON = """Solve this synthetic decision using only the supplied information.
The event is binary: success or failure. All listed observations occurred.
Different underlying_observation_id values are independent conditional on the
event. Reports with the same underlying_observation_id repeat exactly the same
observation; they add no new evidence. The likelihoods are probabilities of
observing that evidence conditional on success or failure, not posterior beliefs.
Choose ACT to maximize expected points if it is better than WAIT. ACT earns
reward_if_success on success and loses loss_if_failure on failure. WAIT earns
zero. Choose WAIT on an exact tie. Return only one JSON object with fields
probability_success (number from 0 to 1), action (ACT or WAIT), and
evidence_ids_used (array of distinct underlying_observation_id strings).
Round the probability to at least six decimal places when necessary.
"""
PERSONAS = {
    "base_rate": "You are a cautious statistical forecaster whose professional habit is to anchor on population frequencies. You distrust vivid stories and are known for taking base rates seriously before reaching a conclusion.",
    "evidence": "You are an investigative analyst whose professional habit is to trace reports back to original sources. You distrust repeated claims and are known for distinguishing independent evidence from repetition.",
    "utility": "You are a pragmatic decision analyst whose professional habit is to weigh possible gains against losses. You distrust intuitive recommendations and are known for connecting probabilities to the consequences of acting.",
}
PROCEDURES = {
    "base_rate": "Compute prior odds as prior/(1-prior). For each distinct observation multiply the odds by likelihood_if_success/likelihood_if_failure. Convert final odds to probability using odds/(1+odds), then apply the stated payoffs.",
    "evidence": "Group reports by underlying_observation_id. Count each group once. Multiply the prior probability by the distinct success likelihoods and the failure prior by the distinct failure likelihoods. Normalize these two joint probabilities, then apply payoffs.",
    "utility": "Compute the posterior probability from the prior and distinct observations. Compute ACT expected points as p*reward_if_success-(1-p)*loss_if_failure. Compare it with WAIT's zero. Use expected points rather than whether p exceeds one half.",
}
FILLER = "This is a self-contained evaluation item. The response will be stored as a structured record. Read the item and return the requested fields. "
PAD_LENGTH = max(map(len, list(PERSONAS.values()) + list(PROCEDURES.values()))) + 100


def instructions(arm, style):
    if arm == "default":
        return COMMON
    text = {"filler": "", "persona": PERSONAS[style], "procedure": PROCEDURES[style]}[arm]
    padding = (" " + FILLER * 10)[:PAD_LENGTH - len(text)]
    return COMMON + "\n" + text + padding


def oracle(case):
    p = Fraction(str(case["prior_success"]))
    yes, no = p, 1 - p
    seen = {}
    for observation in case["reports"]:
        key = observation["underlying_observation_id"]
        likelihoods = tuple(Fraction(str(observation[k])) for k in
                            ("likelihood_if_success", "likelihood_if_failure"))
        if key in seen:
            if seen[key] != likelihoods:
                raise ValueError("Conflicting duplicate observation")
            continue
        seen[key] = likelihoods
        yes *= likelihoods[0]
        no *= likelihoods[1]
    posterior = yes / (yes + no)
    expected = posterior * case["reward_if_success"] - (1 - posterior) * case["loss_if_failure"]
    return dict(probability=float(posterior), exact_probability=str(posterior),
                action="ACT" if expected > 0 else "WAIT", act_value=float(expected),
                evidence_ids=sorted(seen))


def cases():
    # Ten related pairs are the units for the exploratory paired bootstrap.
    configs = [
        ("base_rate", .008, [(.84, .12), (.56, .35), (.18, .63)], .08),
        ("base_rate", .035, [(.73, .21), (.61, .44), (.87, .52)], .35),
        ("base_rate", .12, [(.34, .72), (.68, .29), (.43, .59)], .62),
        ("dependence", .07, [(.78, .13), (.42, .63), (.55, .39)], None),
        ("dependence", .23, [(.64, .32), (.81, .27), (.31, .62)], None),
        ("dependence", .41, [(.39, .65), (.76, .38), (.62, .47)], None),
        ("diagnosticity", .025, [(.57, .38), (.82, .41), (.66, .44)], .09),
        ("diagnosticity", .19, [(.71, .52), (.32, .64), (.83, .49)], .12),
        ("utility", .045, [(.79, .17), (.67, .31), (.28, .61)], None),
        ("utility", .37, [(.48, .73), (.88, .26), (.59, .43)], None),
    ]
    result = []
    for index, (family, prior, likelihoods, changed) in enumerate(configs):
        for variant in range(2):
            public = dict(prior_success=changed if family == "base_rate" and variant else prior,
                          reward_if_success=100, loss_if_failure=65, reports=[])
            for j, (a, b) in enumerate(likelihoods):
                if family == "diagnosticity" and variant and j == 0:
                    b = changed
                public["reports"].append(dict(report_id=f"report-{j}", underlying_observation_id=f"obs-{j}",
                                               likelihood_if_success=a, likelihood_if_failure=b))
            if family == "dependence":
                repeated = dict(public["reports"][0], report_id="report-3")
                if variant:
                    repeated["underlying_observation_id"] = "obs-3"
                public["reports"].append(repeated)
            if family == "utility":
                q = oracle(public)["probability"]
                # Put the decision threshold on opposite sides of the exact posterior.
                threshold = q + (-.06 if variant == 0 else .06)
                public["loss_if_failure"] = round(100 * threshold / (1 - threshold))
            item = dict(case_id=f"pair-{index:02d}-{variant}", pair_id=f"pair-{index:02d}",
                        family=family, variant=variant, public_case=public, truth=oracle(public))
            result.append(item)
    return result


def design():
    return dict(model=MODEL, effort="low", max_output_tokens=4096, ceiling_usd=2.0,
                arms=list(ARMS), styles=list(STYLES), order_seed=20261008,
                cases=cases(), prompts={a: {s: instructions(a, s) for s in STYLES} for a in ARMS})


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def score(text, truth, status="completed"):
    try:
        value = json.loads(text)
        p = value["probability_success"]
        action = value["action"]
        evidence = value["evidence_ids_used"]
        if (status != "completed" or type(p) not in (int, float) or not math.isfinite(p)
                or not 0 <= p <= 1 or action not in ("ACT", "WAIT")
                or not isinstance(evidence, list) or not all(isinstance(x, str) for x in evidence)):
            raise ValueError("Invalid response schema or provider completion")
    except (ValueError, TypeError, KeyError):
        return dict(valid=False, squared_error=1.0, action_correct=False,
                    regret=abs(truth["act_value"]), evidence_correct=False)
    return dict(valid=True, probability=p, action=action,
                squared_error=(p - truth["probability"]) ** 2,
                action_correct=action == truth["action"],
                regret=0.0 if action == truth["action"] else abs(truth["act_value"]),
                evidence_correct=sorted(evidence) == truth["evidence_ids"])


def interval(values):
    rng = random.Random(20261008)
    resamples = sorted(statistics.mean(rng.choices(values, k=len(values))) for _ in range(10000))
    return dict(mean=statistics.mean(values), pair_cluster_bootstrap_95=[resamples[250], resamples[9749]],
                clusters=len(values), resamples=len(resamples))


def analyze(records, frozen):
    indexed = {c["case_id"]: c for c in frozen["cases"]}
    groups = defaultdict(list)
    for r in records:
        if r["kind"] != "scored":
            continue
        groups[r["arm"]].append((r, score(r.get("text", ""), indexed[r["case_id"]]["truth"], r["status"])))
    summary, pair_scores = {}, {}
    for arm, rows in groups.items():
        summary[arm] = {"attempts": len(rows), "valid": sum(s["valid"] for _, s in rows)}
        for metric in ("squared_error", "action_correct", "regret", "evidence_correct"):
            summary[arm][metric] = statistics.mean(s[metric] for _, s in rows)
        summary[arm]["probability_within_1pp"] = statistics.mean(s["valid"] and math.sqrt(s["squared_error"]) <= .01 for _, s in rows)
        by_case, by_pair = defaultdict(list), defaultdict(list)
        for record, scored in rows:
            by_case[record["case_id"]].append(scored)
            by_pair[indexed[record["case_id"]]["pair_id"]].append(scored["squared_error"])
        pair_scores[arm] = {p: statistics.mean(scores) for p, scores in by_pair.items()}
        ensemble = []
        for case_id, scores in by_case.items():
            if len(scores) == 3 and all(s["valid"] for s in scores):
                p = statistics.mean(s["probability"] for s in scores)
                ensemble.append((p - indexed[case_id]["truth"]["probability"]) ** 2)
        summary[arm]["complete_ensemble_cases"] = len(ensemble)
        summary[arm]["ensemble_squared_error_complete_cases"] = statistics.mean(ensemble) if ensemble else None
        directional = []
        for pair in sorted({c["pair_id"] for c in indexed.values()}):
            first, second = by_case.get(pair + "-0", []), by_case.get(pair + "-1", [])
            if len(first) == len(second) == 3 and all(s["valid"] for s in first + second):
                c0, c1 = indexed[pair + "-0"], indexed[pair + "-1"]
                if c0["family"] == "utility":
                    directional.append(all(s["action_correct"] for s in first + second))
                else:
                    actual = statistics.mean(s["probability"] for s in second) - statistics.mean(s["probability"] for s in first)
                    expected = c1["truth"]["probability"] - c0["truth"]["probability"]
                    directional.append(actual * expected > 0)
        summary[arm]["directional_pairs_complete"] = len(directional)
        summary[arm]["directional_pair_success"] = statistics.mean(directional) if directional else None
    contrasts = {}
    for arm, baseline in [("persona", "filler"), ("procedure", "persona"), ("procedure", "default")]:
        if arm in pair_scores and baseline in pair_scores:
            common = sorted(pair_scores[arm].keys() & pair_scores[baseline].keys())
            contrasts[f"{arm}_minus_{baseline}"] = interval([pair_scores[arm][p] - pair_scores[baseline][p] for p in common])
    return dict(arms=summary, contrasts=contrasts, total_requests=len(records),
                accounted_usd=sum(r.get("cost_usd", r["reserved_usd"]) for r in records),
                caveat="Synthetic exact-posterior tasks, ten related pairs; not real forecasting, a market, or Locust runtime qualification.")
