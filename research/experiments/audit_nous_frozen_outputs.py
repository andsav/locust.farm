#!/usr/bin/env python3
"""Audit released Nous forecasts, offline, using only the Python standard library.

Usage: python3 research/experiments/audit_nous_frozen_outputs.py CHECKOUT
Reads the upstream artifact checkout; writes JSON to stdout. Does not invoke models,
change upstream files, or reproduce extraction or prompt generation. The paired
bootstrap samples questions, not underlying event clusters, and is exploratory.
"""

import argparse
import hashlib
import itertools
import json
import math
from pathlib import Path
import random
import statistics
import subprocess


GROUPS = ("control", "control_b", "placebo", "treatment")
N_AGENTS = 10
AGENT_IDS = {
    "control": set(range(0, 10)),
    "treatment": set(range(10, 20)),
    "control_b": set(range(20, 30)),
    "placebo": set(range(30, 40)),
}


def entropy(p):
    return -sum(x * math.log2(x) for x in (p, 1 - p) if x > 0)


def jsd(p, q):
    return entropy((p + q) / 2) - (entropy(p) + entropy(q)) / 2


def pearson(xs, ys):
    xmean, ymean = statistics.mean(xs), statistics.mean(ys)
    xdev = [x - xmean for x in xs]
    ydev = [y - ymean for y in ys]
    denom = math.sqrt(sum(x * x for x in xdev) * sum(y * y for y in ydev))
    return sum(x * y for x, y in zip(xdev, ydev)) / denom if denom else None


def paired_summary(differences):
    rng = random.Random(20261007)
    samples = sorted(
        statistics.mean(rng.choices(differences, k=len(differences)))
        for _ in range(20000)
    )
    return {
        "mean_difference": statistics.mean(differences),
        "question_paired_bootstrap_95_percentile": [samples[500], samples[19499]],
        "n_questions": len(differences),
        "resamples": len(samples),
        "seed": 20261007,
        "caution": "Questions may share events; this is not an event-cluster interval.",
    }


def audit(root):
    folder = root / "artifacts/usefulness/heterogeneity"
    names = (
        "results_heterogeneity.json",
        "results_heterogeneity_controlb.json",
        "results_heterogeneity_placebo.json",
        "data/eval_set_mantic_baseline.jsonl",
    )
    paths = [folder / name for name in names]
    raw = [record for path in paths[:3] for record in json.loads(path.read_text())]
    question_rows = list(map(json.loads, paths[3].read_text().splitlines()))
    questions = {row["market_id"]: row for row in question_rows}
    if len(questions) != len(question_rows):
        raise ValueError("Duplicate question identity")
    by_question = {qid: {g: {} for g in GROUPS} for qid in questions}
    counts = {g: {"records": 0, "parsed": 0, "failed": 0} for g in GROUPS}
    seen = set()
    for record in raw:
        qid, group, agent = record["market_id"], record["group"], record["agent_idx"]
        key = qid, group, agent
        if key in seen:
            raise ValueError(f"Duplicate agent record: {key}")
        seen.add(key)
        if qid not in questions or group not in GROUPS or not isinstance(agent, int):
            raise ValueError(f"Unexpected record identity: {key}")
        if agent not in AGENT_IDS[group]:
            raise ValueError(f"Unexpected agent for group: {key}")
        counts[group]["records"] += 1
        prob = record["prob"]
        if record["parse_failed"] or prob is None:
            counts[group]["failed"] += 1
            continue
        if not isinstance(prob, (int, float)) or not 0 <= prob <= 1:
            raise ValueError(f"Invalid probability: {key}")
        counts[group]["parsed"] += 1
        by_question[qid][group][agent] = prob
    complete = sorted(
        qid for qid, groups in by_question.items()
        if all(len(groups[g]) == N_AGENTS for g in GROUPS)
    )
    if len(complete) < 2:
        raise ValueError("At least two common complete questions are required")
    pairs = list(itertools.combinations(range(N_AGENTS), 2))
    metrics = {g: {} for g in GROUPS}
    for qid in complete:
        outcome = questions[qid]["resolved_outcome"]
        if outcome not in ("Yes", "No"):
            raise ValueError(f"Unrecognized outcome: {qid}")
        y = int(outcome == "Yes")
        for group in GROUPS:
            ps = [p for _, p in sorted(by_question[qid][group].items())]
            errors = [p - y for p in ps]
            individual = statistics.mean(e * e for e in errors)
            product = statistics.mean(errors[i] * errors[j] for i, j in pairs)
            brier = (statistics.mean(ps) - y) ** 2
            metrics[group][qid] = {
                "ensemble_brier": brier,
                "individual_brier": individual,
                "pairwise_error_product": product,
                "mean_pairwise_jsd_bits": statistics.mean(jsd(ps[i], ps[j]) for i, j in pairs),
                "identity_gap": abs(brier - individual / N_AGENTS - (N_AGENTS - 1) * product / N_AGENTS),
                "errors": errors,
            }
    summary = {}
    for group in GROUPS:
        rows = metrics[group]
        summary[group] = {
            field: statistics.mean(row[field] for row in rows.values())
            for field in ("ensemble_brier", "individual_brier", "pairwise_error_product", "mean_pairwise_jsd_bits")
        }
        summary[group]["max_identity_gap"] = max(row["identity_gap"] for row in rows.values())
        correlations = [
            pearson([rows[q]["errors"][i] for q in complete], [rows[q]["errors"][j] for q in complete])
            for i, j in pairs
        ]
        summary[group]["mean_raw_error_correlation"] = statistics.mean(c for c in correlations if c is not None)
    contrasts = {}
    for comparison in ("placebo", "control_b"):
        contrasts[f"treatment_minus_{comparison}"] = {
            field: paired_summary([metrics["treatment"][q][field] - metrics[comparison][q][field] for q in complete])
            for field in ("ensemble_brier", "mean_pairwise_jsd_bits")
        }
    return {
        "scope": "Independent arithmetic audit of frozen forecasts; not a model rerun or full replication.",
        "upstream_commit": subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip(),
        "inputs_sha256": {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},
        "nominal_questions": len(questions),
        "complete_questions": len(complete),
        "complete_question_ids": complete,
        "omitted_question_ids": sorted(set(questions) - set(complete)),
        "records_by_group": counts,
        "group_metrics_on_common_complete_set": summary,
        "paired_contrasts": contrasts,
        "limitations": [
            "No claim about raw-wallet extraction, translator fidelity, or new-model performance.",
            "Complete-case selection can be biased; parse failures are not scored as correct or incorrect.",
            "Raw error correlation is descriptive and does not identify a shared-error cause.",
            "Bootstrap does not correct multiple comparisons or dependence among related markets.",
        ],
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checkout", type=Path)
    args = parser.parse_args()
    print(json.dumps(audit(args.checkout.resolve()), indent=2, sort_keys=True, allow_nan=False))
