# ecdsa.fail leaderboard analysis

Captured 2026-10-03. **Status: evidence appendix. A reduced capture of public data, a script and its output. No challenge code was run.** Interpreted in the [benchmark note](../ecdsa-fail-benchmark.md) and the [prior-art note](../ecdsa-fail-swarm-prior-art.md).

## Files

- [Reduced capture](ecdsa-fail-submissions-2026-10-03.tsv): one row per submission, 1,355 rows, sorted by creation time.
- [Script](ecdsa_fail_leaderboard_analysis.py): reads the reduced capture and prints the figures below.

## Provenance

The source is the body of `GET https://ecdsa.fail/api/benchmarks/1ffb695a-309b-46b6-a728-2f97d8c7be74/submissions`, an unauthenticated endpoint that the site's own page calls. It was fetched once on 2026-10-03 at 23:43 UTC (16:43 PDT). The newest row was created at 20:34 UTC. The full body was 7,225,196 bytes with SHA-256 `a217de61c229c06502831d23d016a302411ee825b7e78e3416f491955698e173`. It is not stored here: it carries the full text of 1,339 public notes. The endpoint serves only the current state, so a later fetch gives larger counts.

The reduced capture keeps, per row: the first eight characters of the submission id, the three timestamps, status, promotion status, qubits, Toffoli count, official score, claimed score, a solver index in order of first appearance in place of the GitHub handle, a class for the rejection reason, the note's length in bytes, whether the note contains "nonce", whether it contains "limiting contract", and the first `Model:` line of the note cut to 32 characters. Note text is not included, so claims labelled *reported* in the notes cite a submission id and can be read on the public leaderboard.

## Output

```sh
python3 research/evidence/ecdsa_fail_leaderboard_analysis.py research/evidence/ecdsa-fail-submissions-2026-10-03.tsv
```

```text
submissions 1355 solvers 142
status {'accepted': 569, 'rejected': 558, 'failed': 225, 'cancelled': 3}
scored 1017 with note 1339
rejection reasons on scored rows {'no_improvement': 448}
pareto advances 843 accepted 564 rejected below 1,150 qubits 268 solvers 76 frontier points 27
scored below 1,150 qubits 280 accepted among them 0
promoted 565 strictly decreasing True first 10753444395 last 1108039260
month-end best {'2026-05': 8405420100, '2026-06': 1571592960, '2026-07': 1488026454, '2026-08': 1140989148, '2026-09': 1109316122, '2026-10': 1108039260}
accepted per month {'2026-05': 20, '2026-06': 379, '2026-07': 25, '2026-08': 92, '2026-09': 33, '2026-10': 20}
largest steps [(18.8, '2026-06-02', '0c1d4d95', 1698, 2447846), (14.8, '2026-08-21', '3616dbf2', 1321, 952707), (11.9, '2026-05-31', 'f94f726c', 2310, 3656039), (8.7, '2026-06-01', '437e22ad', 2310, 3063680), (7.3, '2026-06-02', '66ad478c', 1698, 1704086), (7.2, '2026-06-02', '75927ba7', 1698, 2223350), (7.2, '2026-06-01', '112d5a4d', 2310, 3361759), (6.9, '2026-05-31', 'd35d6e7c', 2708, 3546654)]
lowest accepted Toffoli (888879, 1250, 'ab11959f')
scored rejections tying the best 105
rows sharing a score 215 repeating an earlier score 143 first posted by another solver 130 shared rows equal to an accepted score 203
claimed scores with an official score 592 mismatching 17
accepted latency: median 2.19 p90 2.80
latency at >=1e8 Toffoli: n 180 min 5.7 median 18.7 p90 39.3
most submissions by one solver within an hour 15
last 14 days: promotions 49 median gain % 0.0057 largest single gain % 1.27 fixed-qubit and |dT|<2000 42 top solver's promotions 34
last 14 days: total fall % 2.58
last 14 days: active solvers 14 with an accepted row 6
bands scored/accepted {'>=1350': (244, 197), '1249-1349': (261, 199), '1175-1248': (23, 23), '1150-1174': (209, 150), '850-1149': (77, 0), '<850': (203, 0)}
1175-1248 band: first 2026-06-10 last 2026-06-13
best at 792 (755617938, 'rejected', 'ae98f360', '2026-09-24') product 598449406896
best at 838 (22019787, 'rejected', '48d934d4', '2026-09-22') product 18452581506
best at 973 (11844334, 'rejected', 'a9d89c26', '2026-10-01') product 11524536982
best at 1011 (9329693, 'rejected', '4188053f', '2026-10-02') product 9432319623
best at 1112 (1739356, 'rejected', 'c1c0da9f', '2026-10-02') product 1934163872
best at 1145 (1132785, 'rejected', 'dd3621dc', '2026-10-01') product 1297038825
best at 1164 (970983, 'rejected', '17250b2e', '2026-09-30') product 1130224212
best at 1173 (944620, 'accepted', '0b98178b', '2026-10-03') product 1108039260
best at 1174 (943826, 'accepted', 'ee410040', '2026-10-03') product 1108051724
best at 1250 (888879, 'accepted', 'ab11959f', '2026-09-29') product 1111098750
notes mentioning nonce 756 carrying 'limiting contract' 61 rows with a claimed score 809
declared model: accepted/total {'Claude Opus 4.8': (150, 209), 'GPT-5': (70, 163), 'GPT-5 Codex': (81, 134), 'Claude Opus 5': (34, 106), 'GPT-6 Astra': (0, 87), 'GPT-Codex': (5, 53), 'Claude Opus 5.5': (36, 51), 'Grok 4.3': (1, 41), 'MiniMax M3': (0, 30)}
draws per clean set: batch model 1.69e+09, e^lambda shortcut 3.69e+08; per-shot failure 1 in 425
```

## Reading the output

- **Pareto display.** Replaying every scored row in time order reproduces the three figures on the site's default leaderboard view: 843 advances, 76 solvers, 27 frontier points. Only 564 of the 843 were accepted, so the Pareto view is a display over every scored submission and not the acceptance rule. No submission below 1,150 qubits was ever accepted.
- **Acceptance.** The promoted chain is strictly decreasing, and the only rejection reason on scored rows is failing to improve the current best.
- **Latency** is `updated` minus `created`. Reading this as the official turnaround is an interpretation of the field names. It agrees with GitHub Actions job durations of 80 to 92 seconds read on the same day, plus dispatch and merge.
- **Ties.** Each scored rejection is compared with the promoted score in force at its verdict time. An agent's independent count gave 104; the difference is one row at a timestamp boundary.
- **Repeated scores.** 215 rows share a score with some other row. 143 repeat an earlier row and 130 of those repeat a score first posted by a different solver. Almost all shared scores are accepted scores, which means resubmissions of the current best from a stale base or with a new nonce. This is weak evidence of duplicated research.
- **Models.** The model is whatever the note's first `Model:` line declares. Rates mostly reflect which campaigns a solver ran: every low-qubit submission is rejected whatever produced it.
- **Notes.** The nonce and contract counts are substring matches on untrusted text. They show how common a topic is, not what any note proves.
- **Landing arithmetic.** The record's note reports 19.727 failing 64-shot batches per run. With 141 batches per run, the chance that none fails is (1 − 19.727/141)¹⁴¹. The e^λ shortcut that many notes use understates the draw count by a factor of about 4.6 at this rate.
