# Shared record versus independent attempts versus strong solo: results

**Status: executed and analyzed, 8 October 2026. Exploratory: six held-out
tasks, one trial per task-arm, one model. No confirmatory claim.**

The [protocol](experiments/shared_record/README.md) tests the second sentence
of the [narrowed thesis](../docs/collaboration-thesis.md): that a shared record
of findings lets k agents do better than k isolated agents for the same spend.
Three arms on each task, all `gpt-6-luna` at high reasoning effort with the
same tools and $0.42 of API spend per task-arm: **solo** (one agent, $0.42),
**independent** (three agents, $0.14 each, no communication) and **shared**
(three agents, $0.14 each, reading and posting findings through a real Locust
daemon). Each arm's result is its best candidate by public score, earliest on
ties; the outcome is that candidate's hidden score (cost relative to a fixed
reference heuristic, lower is better). Evidence:
[shared-record-study-2026-10-08](evidence/shared-record-study-2026-10-08/README.md).

## Headline

1. **Shared minus independent: no measurable advantage.** Across the six
   tasks the median difference is +0.001 (shared slightly behind) with two
   wins and four losses; on the five tasks without the compromised `tsp` run,
   +0.001 with two wins and three losses; on the four clean tasks, −0.001
   with two and two. Every difference except `coloring` is within ±0.006,
   below the 0.01 declared as the smallest effect worth acting on. The
   `coloring` difference (+5.39) is a selection artefact explained below.
   The arms' best candidates (oracle, a diagnostic) differ by 0.000 at the
   median.
2. **Independent minus solo: small, in the expected direction, below the
   threshold on clean tasks.** Six tasks: mean −0.022, median −0.009, five
   wins to one, bootstrap interval [−0.055, −0.0003]. That headline is carried
   by `coloring` (−0.099, the tie artefact) and `tsp` (−0.025, the
   compromised run). On the four clean tasks: mean −0.002, median −0.005,
   three wins to one, interval [−0.009, +0.006]. Three independent agents
   with $0.14 each were at least as good as one agent with $0.42 at equal
   used spend, but not by an actionable margin.
3. **What the record did do:** it made the three agents converge. In five of
   six shared trials the agents' final hidden scores lie within 0.0032 of
   one another (four within 0.0005); the independent agents spread by 0.001
   to 0.019. Shared agents paid about 60% more per candidate than
   independent agents (cost per turn was the same; reading and posting are
   turns that produce no candidate) and all three hit their allowance in
   five of six tasks, producing 24% fewer candidates for 20% more money.
   Once, on `coloring`, the record carried a candidate
   that was correct on every public instance and wrong on two hidden ones to
   all three agents.

This study does not support the claim that the record adds value beyond
independent search, for this model, these tasks and this record protocol.
It does not refute it either: one trial per task, six tasks, and the record
was read voluntarily after each agent's first candidate rather than at a
designed point.

## Scored results

Selected hidden score per task-arm (lower is better; 1.0 equals the reference
heuristic). "Spent" is the arm's own spend in this run; "inherited" is what a
voided earlier run had already charged against the same agents' allowances
(see [Reruns inherited voided spend](experiments/shared_record/README.md#reruns-inherited-voided-spend-against-their-allowances-found-at-analysis)).

| Task | Arm | Selected hidden | Public | Oracle hidden | Spent $ | Inherited $ | Candidates |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| coloring | solo | 0.9189 | 0.8182 | 0.8198 | 0.046 | 0 | 6 |
| coloring | independent | 0.8198 | 0.8182 | 0.8198 | 0.146 | 0 | 19 |
| coloring | shared | 6.2072 | 0.8182 | 0.8198 | 0.306 | 0 | 23 |
| mkp | solo | 0.9678 | 0.9635 | 0.9675 | 0.138 | 0 | 23 |
| mkp | independent | 0.9669 | 0.9631 | 0.9668 | 0.333 | 0 | 52 |
| mkp | shared | 0.9680 | 0.9634 | 0.9674 | 0.376 | 0 | 33 |
| qap | solo | 0.9847 | 0.9843 | 0.9815 | 0.323 | 0.016 | 46 |
| qap | independent | 0.9748 | 0.9757 | 0.9748 | 0.273 | 0.044 | 74 |
| qap | shared | 0.9761 | 0.9745 | 0.9743 | 0.333 | 0.043 | 59 |
| setcover | solo | 0.8816 | 0.8982 | 0.8802 | 0.196 | 0 | 28 |
| setcover | independent | 0.8916 | 0.9012 | 0.8855 | 0.363 | 0 | 62 |
| setcover | shared | 0.8862 | 0.8974 | 0.8823 | 0.373 | 0 | 54 |
| tsp (uninformative) | solo | 0.9734 | 0.9700 | 0.9708 | 0.172 | 0.071 | 23 |
| tsp (uninformative) | independent | 0.9489 | 0.9531 | 0.9483 | 0.092 | 0.309 | 18 |
| tsp (uninformative) | shared | 0.9498 | 0.9497 | 0.9498 | 0.057 | 0.348 | 11 |
| unrelated_machines | solo | 0.7357 | 0.7293 | 0.7343 | 0.205 | 0 | 17 |
| unrelated_machines | independent | 0.7276 | 0.7209 | 0.7262 | 0.354 | 0 | 58 |
| unrelated_machines | shared | 0.7262 | 0.7216 | 0.7262 | 0.377 | 0 | 32 |

Of 638 scored candidates, 590 beat the reference on hidden instances and 13
were at the fallback cost on every hidden instance; the best results are 27%
below the reference on unrelated machines and 12% below on set cover. The
selector's integration regret (selected minus oracle) is at most 0.006
everywhere except `coloring` (solo +0.099, shared +5.387).

### Paired contrasts

Negative favours the first arm. Bootstrap intervals are task-level percentile
intervals from 10,000 resamples and are reported as declared, not as
significance tests.

| Tasks | Contrast | n | Mean | Median | Wins | Losses | 95% bootstrap |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| all (preregistered) | shared − independent | 6 | +0.8973 | +0.0010 | 2 | 4 | [−0.0022, +2.6939] |
| all (preregistered) | independent − solo | 6 | −0.0221 | −0.0090 | 5 | 1 | [−0.0551, −0.0003] |
| all (preregistered) | shared − solo | 6 | +0.8752 | −0.0042 | 3 | 3 | [−0.0132, +2.6419] |
| without tsp (owner decision) | shared − independent | 5 | +1.0766 | +0.0011 | 2 | 3 | [−0.0028, +3.2324] |
| without tsp (owner decision) | independent − solo | 5 | −0.0216 | −0.0081 | 4 | 1 | [−0.0616, +0.0027] |
| without tsp (owner decision) | shared − solo | 5 | +1.0550 | +0.0002 | 2 | 3 | [−0.0072, +3.1722] |
| without tsp and coloring | shared − independent | 4 | −0.0011 | −0.0002 | 2 | 2 | [−0.0037, +0.0012] |
| without tsp and coloring | independent − solo | 4 | −0.0022 | −0.0045 | 3 | 1 | [−0.0090, +0.0055] |
| without tsp and coloring | shared − solo | 4 | −0.0033 | −0.0042 | 2 | 2 | [−0.0091, +0.0024] |

Per-task differences, shared − independent: coloring +5.387, mkp +0.001,
qap +0.001, setcover −0.005, tsp +0.001, unrelated_machines −0.001.
Independent − solo: coloring −0.099, mkp −0.001, qap −0.010, setcover +0.010,
tsp −0.025, unrelated_machines −0.008.

On the arms' best candidates (oracle hidden, a ceiling that no deployed
selector sees), shared − independent without `tsp` is −0.0006 (median
0.0000, two wins, one loss, two ties); independent − solo is −0.0020 (median
−0.0007, three wins, one loss).

### Matched used spend

Applying the same selector only to candidates produced while the arm's
cumulative actual spend was at most $0.14, $0.28 or $0.42 (this run's own
spend only):

| Spend ≤ | Contrast | n (all) | Mean | Wins | Losses | n (without tsp, coloring) | Mean | Wins | Losses |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| $0.14 | shared − independent | 6 | +0.8987 | 1 | 5 | 4 | +0.0009 | 1 | 3 |
| $0.14 | independent − solo | 6 | −0.0224 | 5 | 1 | 4 | −0.0027 | 3 | 1 |
| $0.14 | shared − solo | 6 | +0.8762 | 3 | 3 | 4 | −0.0018 | 2 | 2 |
| $0.28 | shared − independent | 6 | +0.8977 | 2 | 4 | | | | |
| $0.28 | independent − solo | 6 | −0.0227 | 5 | 1 | | | | |
| $0.42 | shared − independent | 6 | +0.8973 | 2 | 4 | | | | |
| $0.42 | independent − solo | 6 | −0.0221 | 5 | 1 | | | | |

The pattern does not move with the spend level. At $0.14, three agents that
had each used about $0.05 were ahead of one agent that had used $0.14 on
three of four clean tasks.

## Why `coloring` and `tsp` are not read as evidence

**`coloring`: the public score could not discriminate.** Colour counts are
integers on five public instances, and every arm's best public score was the
same 0.8182, shared by 34 of the 48 candidates. The preregistered selector
then picks the earliest tied candidate. In the shared arm that was agent 1's
third candidate (89 s in), which was correct on every public instance and
raised `IndexError` on two of ten hidden instances; those two count at the
fallback cost, giving 6.2072. Agents 0 and 2 then adopted that approach from
the record and reproduced the same hidden failure before all three fixed it
(the fixed source agent 2 later evaluated is byte-identical to agent 1's,
same `sha256`). The independent arm had the same kind of hidden-only bug in
one agent (final 11.86) but its earliest tied candidate happened to be sound;
the solo arm's earliest tied candidate was sound but weak (0.9189), while a
later tied candidate scored 0.8198. All three arms had an oracle of 0.8198.
The `coloring` outcomes are therefore tie-order lotteries: the shared arm's
is a real failure of that arm's deliverable under the preregistered rule and
a real instance of the record propagating a defect, but its size is an
artefact of the fallback cost, and the solo−independent gap says nothing
about search. Lesson: a continuous public objective or many more public
instances; integer objectives saturate at this scale.

**`tsp`: unequal effective allowances.** The first `tsp` trials were voided
by a network outage that removed different fractions of each arm (protocol,
[second outage](experiments/shared_record/README.md#scored-phase-tsp-and-qap-voided-by-a-second-network-outage)).
The rerun then inherited the voided run's charges against the same agents'
allowances through a harness defect found at analysis: the group arms ran on
$0.11 and $0.07 of their $0.42, the solo on $0.35 (and then lost one request
to an isolated HTTP 503). Neither the voided nor the rerun `tsp` is a matched
comparison. The repository owner chose not to spend further; `tsp` is
reported in every table and excluded from the interpretation. For the record:
in the rerun the group arms still finished ahead of the solo (0.949 and 0.950
against 0.973) on a quarter of its spend; in the voided run, as it stood when
the outage hit, the three arms were 0.955, 0.956 and 0.944 (independent,
solo, shared).

**`qap` carries a small caveat.** Its rerun inherited $0.044 and $0.043
against the group arms and $0.016 against the solo, so the group arms ran on
about 90% of their nominal allowance and the solo on 96%. The handicap goes
against the observed result (independent 0.010 ahead of solo; shared 0.001
behind independent), so it does not create either finding.

### Sensitivity: preregistered treatment of the outage

Under the preregistered rule the outage-hit `tsp` and `qap` trials would have
stood as they were. Counting them instead of the reruns
(`analysis-preregistered-treatment.json` in the evidence): shared −
independent over six tasks is mean +0.8952, median −0.0002, three wins and
three losses; independent − solo over five tasks (the voided `qap` solo had
no candidate) is mean −0.0194, median −0.0009, three wins and two losses. The
reading is the same: nothing actionable between shared and independent, a
small edge for independent attempts over the solo.

## Mechanism diagnostics

Reported as the protocol asks: complementarity, integration, repair.

**Complementarity (independent agents' final hidden spread).** mkp 0.0010,
setcover 0.0039, unrelated_machines 0.0092, qap 0.0153, tsp 0.0185, coloring
0.009 among its two sound agents. Amendment A2 requires that a shared-versus-
independent difference be read as informative only on tasks whose independent
agents spread by at least 0.01. That is `qap` alone among the clean tasks,
where shared finished 0.0013 behind independent. On the tasks where three
agents with the same prompt and model ended within 0.004 of one another,
there was little for a record to transfer.

**Integration.** Public-score selection kept the hidden-best candidate or one
within 0.006 of it in every arm except `coloring` (above). The best initial
agent was the best final agent in three of the twelve group trials
(`coloring/independent`, `tsp/shared`, `unrelated_machines/independent`);
rank at the first candidate mostly did not predict rank at the end.

**Repair (what reading the record did).** Delivery was real and measured: 124
findings posted with signed receipts over the six shared trials (8 to 28 per
task), 97 reads, none refused, 77 of which returned at least one new peer
finding; in total 191 peer findings delivered. Every agent read the record
after its first or second candidate, so almost all shared-arm work happened
after exposure: 1 to 2 candidates before the first peer finding, 1 to 20
after. After exposure the agents' scores collapsed together (final spreads
0.0000, 0.0005, 0.0004, 0.0032, 0.0004 on coloring, mkp, qap, setcover and
unrelated_machines; the under-funded `tsp` 0.0183, where two agents ended on
the identical 0.9681). The convergence point was no better than the
independent arm's best. The record's measurable costs: over the five
non-`tsp` tasks the shared arms produced 201 candidates for $1.77 ($0.0088
each) against the independent arms' 265 for $1.47 ($0.0055 each), and all
three shared agents hit their allowance in five of six tasks against four
budget stops among eighteen independent agents.

**Solo behaviour.** The solo agent never used its allowance. It stopped by
choice after one nudge in `coloring`, `mkp`, `setcover` and
`unrelated_machines` at 11%, 33%, 47% and 49% of $0.42, stopped by choice
without a nudge at 80% in `qap`, and was cut off by the 503 in `tsp`. The
independent agents, with a third of the money each, mostly stopped by choice
too but later (35% to 86% of the arm allowance). This is part of what "independent
attempts beat one attempt" means in practice: three agents with small
budgets keep working where one with a large budget declares itself done. At
matched used spend the independent arm still led on three of four clean
tasks, so the effect is not only the solo's early stop.

## Development-family calibration

Calibration ran on maximum cut and minimum vertex cover (not scored). After
Amendment A1 raised instance sizes tenfold: maxcut independent 0.9516 (agent
spread 0.0113), shared 0.9558 (spread 0.0047, 33 findings posted, 13 reads
returned peer findings); vertex cover independent 0.9475 (spread 0.0024,
with the $0.051 inherited-allowance caveat) and solo 0.9568. The gate failed
under its preregistered spread criterion on the vertex-cover spread and the
owner chose to proceed under the post-hoc Amendment A2; the preregistered
verdict is kept as `calibration-gate-preregistered-rule.json`. The first
calibration, before A1, saturated at small instance sizes and is retained in
[shared-record-calibration-1-2026-10-08](evidence/shared-record-calibration-1-2026-10-08/README.md).

## Deviations, in order

All are described in the protocol file; this is the list.

1. Amendment A1 (before any scored call): tenfold instance sizes, instances
   through `run_python` instead of the prompt, instance digests in the
   manifest, a faster but identical QAP reference. Decided on development
   results only.
2. Calibration vertex cover voided and rerun once after a network outage took
   all four of its agents' requests (11:50–11:56).
3. Amendment A2 (post hoc): spread criterion relaxed to "at least one
   independent calibration trial", after seeing calibration 2's verdict.
4. `unrelated_machines/shared`, `mkp` and `setcover` ran in a later launch
   after a principal-name bug in the harness; fixed with a test.
5. `tsp` and `qap` voided and rerun after a second outage (13:52) hit all
   seven `qap` agents and three of seven `tsp` agents; rule applied to every
   trial with an agent lost in the window; the voided scores had been
   printed by the file listing before the rerun.
6. An isolated HTTP 503 ended `tsp/solo` early; stands under the
   preregistered rule.
7. Reruns inherited the voided runs' charges against their allowances
   (found at analysis); owner chose no further spending; `tsp` uninformative,
   `qap` and calibration vertex cover caveated; harness fixed with a test.

## Spend

| Phase | Spend |
| --- | ---: |
| Calibration 1 (saturated, before A1) | $0.70 |
| Calibration 2, including the voided vertex-cover attempt | $1.23 |
| Scored, including the voided `tsp` and `qap` trials | $5.30 |
| **Study total** (ceiling $15.00) | **$7.22** |

1,805 ledger entries, 15 failed requests (all kept at their full reservation),
every settled request on the expected model. Daemon operations were local
and free.

## What this changes

- The product claim that a shared record lets agents do better than isolated
  attempts for the same spend has now been measured once, on the real daemon,
  and did not show. The default guidance should not promise it. Independent
  attempts with public-check selection are the configuration this study
  supports, weakly, over one agent with the pooled budget.
- A record that agents read voluntarily right after their first candidate
  acts as a synchronizer: it removes the diversity that selection needs and
  costs tokens on every turn. If the record is to add value, the next design
  should test a later or gated exposure (for example, read only after a
  plateau, or only failures and dead ends rather than approaches), and
  measure whether diversity survives it.
- Measurement lessons for the next study: integer objectives saturate the
  public score at this scale (use continuous objectives or many more public
  instances); a "strong solo" needs a continuation protocol, not one nudge;
  a per-request timeout shorter than the 600 s socket timeout and a fresh
  output directory or per-session accounting for any rerun; and more than
  one trial per task-arm before any effect near 0.01 can be read.

## Limits

One model family at one effort level; the thesis names a frontier solo model
as the right baseline and this study used the same cheap model in every arm.
Six synthetic optimization families, one trial each, with between-run noise
in anytime heuristics that the design does not estimate. Two tasks lost to
measurement problems. Two post-hoc decisions (A2 and the outage rule) and a
harness defect, all disclosed. Nothing here qualifies Locust's networking or
native coding-agent clients.
