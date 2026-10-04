# ecdsa.fail leaderboard analysis

Captured 2026-10-03. **Status: evidence appendix. A script and its output over public data; no challenge code was run.** Interpreted in the [benchmark note](../ecdsa-fail-benchmark.md).

## Input

The body of `GET https://ecdsa.fail/api/benchmarks/1ffb695a-309b-46b6-a728-2f97d8c7be74/submissions`, an unauthenticated endpoint that the site's own page calls. Fetched once on 2026-10-03 at about 20:40 UTC: 7,225,196 bytes, SHA-256 `a217de61c229c06502831d23d016a302411ee825b7e78e3416f491955698e173`, 1,355 rows. The capture is not stored in this repository. It contains solver handles and the full text of 1,339 public notes, and the endpoint serves the current version. Later captures will give larger counts.

Each row carries `status`, `officialScore`, `officialMetrics {qubits, toffoli}`, `rejectionReason`, `promotionStatus`, `note`, `coauthors`, `solverUsername`, `createdAt`, `updatedAt` and `promotionFinishedAt`.

## Script

```python
#!/usr/bin/env python3
"""Recompute the ecdsa.fail leaderboard figures from the public submissions JSON.

Usage: python3 leaderboard_analysis.py submissions.json
Input: the body of GET https://ecdsa.fail/api/benchmarks/<benchmark id>/submissions
"""
import collections, datetime as dt, json, statistics, sys

rows = json.load(open(sys.argv[1]))["submissions"]
ts = lambda s: dt.datetime.fromisoformat(s.replace("Z", "+00:00"))
metric = lambda r: (r["officialMetrics"]["qubits"], r["officialMetrics"]["toffoli"])

measured = [r for r in rows if r["officialMetrics"]]
accepted = [r for r in rows if r["status"] == "accepted"]
print("submissions", len(rows), "solvers", len({r["solverUsername"] for r in rows}))
print("status", dict(collections.Counter(r["status"] for r in rows)))
print("scored", len(measured), "with note", sum(1 for r in rows if r["note"]))

# Pareto display: replay every scored row in time order, count non-dominated arrivals.
front, advances, adv_accepted, adv_solvers = [], 0, 0, set()
for r in sorted(measured, key=lambda r: r["createdAt"]):
    q, t = metric(r)
    if any(fq <= q and ft <= t for fq, ft in front):
        continue
    front = [(fq, ft) for fq, ft in front if not (q <= fq and t <= ft)] + [(q, t)]
    advances += 1
    adv_accepted += r["status"] == "accepted"
    adv_solvers.add(r["solverUsername"])
print("pareto advances", advances, "accepted among them", adv_accepted,
      "solvers", len(adv_solvers), "current frontier points", len(front))

# Official acceptance: the promoted chain is strictly decreasing.
promoted = sorted((r for r in accepted if r["promotionStatus"] == "promoted"),
                  key=lambda r: r["promotionFinishedAt"])
scores = [int(r["officialScore"]) for r in promoted]
print("promoted", len(promoted), "strictly decreasing",
      all(a > b for a, b in zip(scores, scores[1:])), "first", scores[0], "last", scores[-1])
month_end = {}
for r in promoted:
    month_end[r["createdAt"][:7]] = int(r["officialScore"])
print("month-end best", month_end)

# Scored rejections that exactly tied the best at verdict time.
ties = 0
for r in rows:
    if r["status"] != "rejected" or not r["officialMetrics"]:
        continue
    best = None
    for p in promoted:
        if ts(p["promotionFinishedAt"]) > ts(r["updatedAt"]):
            break
        best = int(p["officialScore"])
    ties += best is not None and int(r["officialScore"]) == best
print("scored rejections tying the best", ties)

# Submission-to-verdict latency for accepted rows, in minutes.
lat = sorted((ts(r["updatedAt"]) - ts(r["createdAt"])).total_seconds() / 60 for r in accepted)
print("accepted latency: median %.2f p90 %.2f" % (statistics.median(lat), lat[int(0.9 * len(lat))]))

# Last 14 days of promotions.
cut = ts(max(r["createdAt"] for r in rows)) - dt.timedelta(days=14)
recent = [(a, b) for a, b in zip(promoted, promoted[1:]) if ts(b["createdAt"]) >= cut]
gains = [(int(a["officialScore"]) - int(b["officialScore"])) / int(a["officialScore"]) * 100 for a, b in recent]
fixed_q = sum(1 for a, b in recent
              if metric(a)[0] == metric(b)[0] and abs(metric(a)[1] - metric(b)[1]) < 2000)
print("last 14 days: promotions", len(recent), "median gain %% %.4f" % statistics.median(gains),
      "fixed-qubit and |dT|<2000", fixed_q,
      "top solver", collections.Counter(b["solverUsername"] for _, b in recent).most_common(1))
first, last = int(recent[0][0]["officialScore"]), int(recent[-1][1]["officialScore"])
print("last 14 days: total fall %% %.2f" % ((first - last) / first * 100))

# Qubit bands and best Toffoli at selected widths.
def band(q):
    return ("<850" if q < 850 else "850-1149" if q < 1150 else "1150-1174" if q < 1175
            else "1175-1248" if q < 1249 else "1249-1349" if q < 1350 else ">=1350")
scored, acc = collections.Counter(), collections.Counter()
best_t = {}
for r in measured:
    q, t = metric(r)
    scored[band(q)] += 1
    acc[band(q)] += r["status"] == "accepted"
    if q not in best_t or t < best_t[q][0]:
        best_t[q] = (t, r["status"], r["id"][:8], r["createdAt"][:10])
print("bands scored/accepted", {k: (scored[k], acc[k]) for k in scored})
for q in (792, 1145, 1164, 1173, 1174, 1250):
    print("best at", q, best_t[q], "product", best_t[q][0] * q)

notes = [r["note"].lower() for r in rows if r["note"]]
print("notes mentioning nonce", sum("nonce" in n for n in notes),
      "carrying 'limiting contract'", sum("limiting contract" in n for n in notes))
print("rows with coauthors", sum(1 for r in rows if r["coauthors"]))
```

## Output

```text
submissions 1355 solvers 142
status {'accepted': 569, 'rejected': 558, 'failed': 225, 'cancelled': 3}
scored 1017 with note 1339
pareto advances 843 accepted among them 564 solvers 76 current frontier points 27
promoted 565 strictly decreasing True first 10753444395 last 1108039260
month-end best {'2026-05': 8405420100, '2026-06': 1571592960, '2026-07': 1488026454, '2026-08': 1140989148, '2026-09': 1109316122, '2026-10': 1108039260}
scored rejections tying the best 105
accepted latency: median 2.19 p90 2.80
last 14 days: promotions 49 median gain % 0.0057 fixed-qubit and |dT|<2000 42 top solver [('dysnasia', 34)]
last 14 days: total fall % 2.58
bands scored/accepted {'>=1350': (244, 197), '1249-1349': (261, 199), '1175-1248': (23, 23), '1150-1174': (209, 150), '850-1149': (77, 0), '<850': (203, 0)}
best at 792 (755617938, 'rejected', 'ae98f360', '2026-09-24') product 598449406896
best at 1145 (1132785, 'rejected', 'dd3621dc', '2026-10-01') product 1297038825
best at 1164 (970983, 'rejected', '17250b2e', '2026-09-30') product 1130224212
best at 1173 (944620, 'accepted', '0b98178b', '2026-10-03') product 1108039260
best at 1174 (943826, 'accepted', 'ee410040', '2026-10-03') product 1108051724
best at 1250 (888879, 'accepted', 'ab11959f', '2026-09-29') product 1111098750
notes mentioning nonce 756 carrying 'limiting contract' 61
rows with coauthors 15
```

## Reading the output

- The Pareto replay reproduces the three figures on the site's default leaderboard view exactly: 843 advances, 76 solvers, 27 frontier points. Only 564 of the 843 were accepted, which shows that the Pareto view is a display over every scored submission and not the acceptance rule.
- The promoted chain is strictly decreasing, and the only rejection reason on scored rows is failing to improve the current best. Acceptance is by scalar product.
- Latency is `updatedAt` minus `createdAt`. On accepted rows `updatedAt` equals `promotionFinishedAt`. Treating this difference as the official evaluation turnaround is an interpretation of the field names; it agrees with the GitHub Actions job durations of 80 to 92 seconds read on the same day.
- "Tying the best" compares each scored rejection with the promoted score in force at its verdict time. An agent's independent count gave 104; the difference is one row at a timestamp boundary.
- The counts of notes mentioning a nonce or carrying the "limiting contract" text are substring matches on untrusted text. They show how common the topic is, not what any note proves.
