#!/usr/bin/env python3
"""Recompute the ecdsa.fail leaderboard figures from the reduced capture.

Usage: python3 research/evidence/ecdsa_fail_leaderboard_analysis.py \
           research/evidence/ecdsa-fail-submissions-2026-10-03.tsv
"""
import collections, csv, datetime as dt, math, statistics, sys

rows = list(csv.DictReader(open(sys.argv[1]), delimiter="\t"))
ts = lambda s: dt.datetime.fromisoformat(s.replace("Z", "+00:00"))
for r in rows:
    r["q"] = int(r["qubits"]) if r["qubits"] else None
    r["t"] = int(r["toffoli"]) if r["toffoli"] else None
    r["s"] = int(r["score"]) if r["score"] else None
pct = lambda a, b: (a - b) / a * 100

scored = [r for r in rows if r["q"] is not None]
accepted = [r for r in rows if r["status"] == "accepted"]
print("submissions", len(rows), "solvers", len({r["solver"] for r in rows}))
print("status", dict(collections.Counter(r["status"] for r in rows)))
print("scored", len(scored), "with note", sum(1 for r in rows if int(r["note_bytes"])))
print("rejection reasons on scored rows",
      dict(collections.Counter(r["reason"] for r in scored if r["status"] == "rejected")))

# Pareto display: replay every scored row in time order, count non-dominated arrivals.
front, adv, adv_acc, adv_rej_low, adv_solvers = [], 0, 0, 0, set()
for r in scored:  # the capture is sorted by creation time
    q, t = r["q"], r["t"]
    if any(fq <= q and ft <= t for fq, ft in front):
        continue
    front = [(fq, ft) for fq, ft in front if not (q <= fq and t <= ft)] + [(q, t)]
    adv += 1
    adv_acc += r["status"] == "accepted"
    adv_rej_low += r["status"] != "accepted" and q < 1150
    adv_solvers.add(r["solver"])
print("pareto advances", adv, "accepted", adv_acc, "rejected below 1,150 qubits", adv_rej_low,
      "solvers", len(adv_solvers), "frontier points", len(front))
print("scored below 1,150 qubits", sum(1 for r in scored if r["q"] < 1150),
      "accepted among them", sum(1 for r in scored if r["q"] < 1150 and r["status"] == "accepted"))

# Official acceptance: the promoted chain is strictly decreasing.
promoted = sorted((r for r in accepted if r["promotion"] == "promoted"), key=lambda r: r["promoted"])
sc = [r["s"] for r in promoted]
print("promoted", len(promoted), "strictly decreasing", all(a > b for a, b in zip(sc, sc[1:])),
      "first", sc[0], "last", sc[-1])
month_end, month_acc = {}, collections.Counter()
for r in promoted:
    month_end[r["created"][:7]] = r["s"]
for r in accepted:
    month_acc[r["created"][:7]] += 1
print("month-end best", month_end)
print("accepted per month", dict(month_acc))
steps = sorted(((pct(a["s"], b["s"]), b["created"][:10], b["id"], b["q"], b["t"])
                for a, b in zip(promoted, promoted[1:])), reverse=True)
print("largest steps", [(round(p, 1), d, i, q, t) for p, d, i, q, t in steps[:8]])
print("lowest accepted Toffoli", min((r["t"], r["q"], r["id"]) for r in accepted))

# Scored rejections that exactly tied the best at verdict time.
ties = 0
for r in scored:
    if r["status"] != "rejected":
        continue
    best = None
    for p in promoted:
        if ts(p["promoted"]) > ts(r["updated"]):
            break
        best = p["s"]
    ties += best is not None and r["s"] == best
print("scored rejections tying the best", ties)

# Repeated scores.
cnt = collections.Counter(r["s"] for r in scored)
first, repeat, repeat_other = {}, 0, 0
for r in scored:
    if r["s"] in first:
        repeat += 1
        repeat_other += first[r["s"]] != r["solver"]
    else:
        first[r["s"]] = r["solver"]
acc_scores = {r["s"] for r in accepted}
print("rows sharing a score", sum(v for v in cnt.values() if v > 1),
      "repeating an earlier score", repeat, "first posted by another solver", repeat_other,
      "shared rows equal to an accepted score",
      sum(1 for r in scored if cnt[r["s"]] > 1 and r["s"] in acc_scores))
both = [r for r in rows if r["claimed"] and r["s"] is not None]
print("claimed scores with an official score", len(both),
      "mismatching", sum(1 for r in both if int(float(r["claimed"])) != r["s"]))

# Submission-to-verdict latency, minutes.
lat = lambda rs: sorted((ts(r["updated"]) - ts(r["created"])).total_seconds() / 60 for r in rs)
a = lat(accepted)
print("accepted latency: median %.2f p90 %.2f" % (statistics.median(a), a[int(0.9 * len(a))]))
big = lat([r for r in scored if r["t"] >= 10**8])
print("latency at >=1e8 Toffoli: n %d min %.1f median %.1f p90 %.1f"
      % (len(big), big[0], statistics.median(big), big[int(0.9 * len(big))]))
by = collections.defaultdict(list)
for r in rows:
    by[r["solver"]].append(ts(r["created"]))
print("most submissions by one solver within an hour",
      max(sum(1 for u in v[i:] if (u - t).total_seconds() <= 3600)
          for v in by.values() for i, t in enumerate(sorted(v))))

# Last 14 days.
cut = ts(max(r["created"] for r in rows)) - dt.timedelta(days=14)
recent = [(x, y) for x, y in zip(promoted, promoted[1:]) if ts(y["created"]) >= cut]
gains = [pct(x["s"], y["s"]) for x, y in recent]
fixed_q = sum(1 for x, y in recent if x["q"] == y["q"] and abs(x["t"] - y["t"]) < 2000)
print("last 14 days: promotions", len(recent), "median gain %% %.4f" % statistics.median(gains),
      "largest single gain %% %.2f" % max(gains), "fixed-qubit and |dT|<2000", fixed_q,
      "top solver's promotions", collections.Counter(y["solver"] for _, y in recent).most_common(1)[0][1])
print("last 14 days: total fall %% %.2f" % pct(recent[0][0]["s"], recent[-1][1]["s"]))
active = [r for r in rows if ts(r["created"]) >= cut]
print("last 14 days: active solvers", len({r["solver"] for r in active}),
      "with an accepted row", len({r["solver"] for r in active if r["status"] == "accepted"}))

# Qubit bands and best Toffoli at selected widths.
def band(q):
    return ("<850" if q < 850 else "850-1149" if q < 1150 else "1150-1174" if q < 1175
            else "1175-1248" if q < 1249 else "1249-1349" if q < 1350 else ">=1350")
n, acc, best_t = collections.Counter(), collections.Counter(), {}
for r in scored:
    n[band(r["q"])] += 1
    acc[band(r["q"])] += r["status"] == "accepted"
    if r["q"] not in best_t or r["t"] < best_t[r["q"]][0]:
        best_t[r["q"]] = (r["t"], r["status"], r["id"], r["created"][:10])
print("bands scored/accepted", {k: (n[k], acc[k]) for k in n})
mid = sorted(r["created"][:10] for r in scored if 1175 <= r["q"] < 1249)
print("1175-1248 band: first", mid[0], "last", mid[-1])
for q in (792, 838, 973, 1011, 1112, 1145, 1164, 1173, 1174, 1250):
    print("best at", q, best_t[q], "product", best_t[q][0] * q)

# Notes.
print("notes mentioning nonce", sum(int(r["nonce"]) for r in rows),
      "carrying 'limiting contract'", sum(int(r["contract"]) for r in rows),
      "rows with a claimed score", sum(1 for r in rows if r["claimed"]))
models = collections.Counter(r["model"] for r in rows if r["model"])
macc = collections.Counter(r["model"] for r in accepted if r["model"])
print("declared model: accepted/total", {m: (macc[m], c) for m, c in models.most_common(9)})

# Landing arithmetic for a reported 19.727 failing 64-shot batches out of 141.
lam, batches = 19.727, 9024 // 64
p_clean = (1 - lam / batches) ** batches
print("draws per clean set: batch model %.2e, e^lambda shortcut %.2e; per-shot failure 1 in %.0f"
      % (1 / p_clean, math.exp(lam), 1 / (1 - (1 - lam / batches) ** (1 / 64))))
