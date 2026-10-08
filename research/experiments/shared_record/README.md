# Shared record versus independent attempts versus strong solo: protocol

Status: preregistered protocol, 8 October 2026, for the experiment proposed in
the [collaboration thesis](../../../docs/collaboration-thesis.md). This file and
the code beside it are committed before any scored call. Results are reported
separately and do not modify this file, with one exception: the first
calibration failed its gate and the protocol was amended once, before any
scored call, as recorded in [Amendment A1](#amendment-a1-8-october-2026-before-any-scored-call).

## Question

On hard problems with a graded, independently computed score, does a shared
Locust record of findings let k agents do better than k isolated agents with
the same total spend, and does either beat one agent with the whole spend?

The primary contrast is **shared minus independent**. The secondary contrast is
**independent minus solo**. Both are measured on held-out problem families
frozen before scoring.

## Arms

All arms use `gpt-6-luna` at reasoning effort `high` through the direct
Responses API, with the same system prompt, tools and task text except for the
arm-specific paragraph in `run.py`'s `ARM_NOTES`. B is the task-arm allowance,
$0.42 of API spend at the conservative rates below. Because every arm uses one
model, dollars and price-weighted tokens are the same resource.

| Arm | Agents | Allowance each | Extra tools | Knows |
| --- | ---: | ---: | --- | --- |
| solo | 1 | B | none | It works alone |
| independent | 3 | B/3 | none | Two peers exist; no communication; result is the best of three |
| shared | 3 | B/3 | `read_record`, `post_finding` | Same as independent, plus the record |

Each task-arm's **result** is the candidate with the best public score among
all its agents' evaluated candidates, earliest on ties. Hidden scores never
enter selection. The result's hidden score is the outcome.

### The shared record is a real Locust goal

A private copy of the current `locust` binary runs one daemon for the study.
Each shared trial creates a goal (`open` formation) with one enrolled principal
per agent. `post_finding` publishes a goal-wide contribution containing the
agent's summary, its latest candidate's public score and source hash, and the
source if requested. `read_record` lists the goal's contributions through the
daemon, drops the caller's own and any already returned, and returns the rest
oldest first. The harness refuses `read_record` until the caller has evaluated
its own first candidate, which is the installed skill's convention made
enforceable for the experiment. Reading and posting are otherwise voluntary.
Receipts (signed event IDs) and every read's returned contribution IDs are
retained, so delivery is measured rather than assumed.

## Tasks

Heuristic design: write `solve(instance)` in standard-library Python for a
combinatorial optimization family. A fixed evaluator scores each evaluated
candidate on 5 public instances the agent sees and 10 hidden instances from the
same seeded generator that it never sees. Score = total cost / the reference
heuristic's total cost on the same instances, lower is better, 1.0 equals the
reference. An instance that raises, returns an invalid solution or exceeds 2
CPU seconds counts as the trivial fallback's cost. Validators and cost
functions are in `problems.py`; instances derive from the study seed.

| Split | Families | Reference heuristic |
| --- | --- | --- |
| Development (calibration only) | maximum cut; minimum vertex cover | greedy + flip local search; greedy by degree |
| Held-out (scored) | Euclidean TSP; graph colouring; unrelated parallel machines; multidimensional knapsack; weighted set cover; quadratic assignment | NN + 2-opt; DSATUR; list scheduling; ratio greedy; cost-effectiveness greedy; swap local search |

Development and held-out families share no problem template. The held-out set
is fixed here and will not change after calibration.

Instance sizes (Amendment A1): max cut n 580–620 at edge probability 0.05;
vertex cover n 780–820 at 0.01; TSP 580–620 points; colouring n 280–320 at
0.1; 290–310 jobs on 12 machines; 290–310 items in 10 knapsack dimensions at
0.3 tightness; set cover over 580–620 elements with 450 sets; QAP n 68–72. Each
family's `shape` string states its generator to the agent, so that agents can
target the distribution rather than the five public instances.

## Tools and limits

- `run_python(source)`: free experimentation with the public instances as
  `INSTANCES`; 20 CPU seconds; nothing recorded. The instances are not in the
  prompt (Amendment A1); the prompt states the generator and the reference and
  fallback costs, and tells the agent to inspect `INSTANCES`.
- `evaluate(source)`: scores on public instances, returns per-instance cost
  against the reference, records the candidate; the hidden score is computed
  at the same moment and stored, never shown.
- Both run under `sandbox-exec` with no network, no access to the user's files
  and one subprocess per instance bounded by `RLIMIT_CPU`. CPU time, not wall
  time, is the limit, so host load cannot create timeouts.
- Each tool result reports the agent's spend so far and its allowance.
- An agent that stops with more than 40% of its allowance left is told once
  that it may continue or reply "done". A response cut off at the output cap
  is asked once to continue. The oldest tool outputs are replaced by a short
  placeholder once the conversation exceeds a byte-based soft limit; the same
  rule applies in every arm. Safety cap: 150 turns per agent.
- No automatic retry on transport failure; the agent stops and its full
  reservation stays charged. A failed agent's candidates still count for its
  arm. Trials are never silently rerun.

## Calibration and gate

Calibration runs the independent and shared arms on maximum cut, and the
independent and solo arms on minimum vertex cover, under a $2.00 phase ceiling.
The scored phase runs only if all of the following hold (`run.py`, `gate`):

1. At most one transport failure across calibration.
2. In each group trial at least two agents, and in the solo trial the agent,
   produced a candidate valid on every public instance.
3. In each independent trial, the three agents' final candidates' hidden
   scores span at least 0.005. (Relaxed post hoc by
   [Amendment A2](#amendment-a2-post-hoc-8-october-2026-after-calibration-2s-verdict)
   to: in at least one independent trial.)
4. In the shared trial, at least one finding was published with a receipt and
   at least one read by a different agent returned a peer's finding.
5. At least one calibration candidate scored below 1.0 on hidden instances.

Failing the gate stops the study; the calibration results are reported as such.
Passing it changes nothing in this protocol or the held-out set.

## Scored phase

Six held-out tasks, three arms each, one trial per task-arm, in a seeded task
order with two tasks running concurrently and the three arms of a task running
concurrently. Phase ceiling $8.50; study ceiling $15.00 including calibration
and failures. These are maximum reservations; actual spend is reported.

## Analysis

The unit is the task. For each task-arm: the result's hidden score, its public
score, actual spend, turns, elapsed time, transport failures and budget stops.

- **Full spend.** Paired differences in the result's hidden score for
  shared−independent, independent−solo and shared−solo, with wins/losses/ties
  and a task-level percentile bootstrap 95% interval (10,000 resamples). With
  six tasks this is exploratory; no significance claim is made.
- **Matched spend.** The same selector applied only to candidates produced
  while the arm's cumulative actual spend (summed across its agents, from
  settled ledger entries) was at most B/3, 2B/3 and B. This compares arms at
  equal used dollars, not equal caps.
- **Smallest effect worth acting on:** 0.01 in normalized score (1% of the
  reference cost), declared here. Smaller mean differences, whatever their
  sign, will be reported as not actionable.
- **Diagnostics.** Complementarity: the range of the three agents' final hidden
  scores in each group trial, and whether the best initial agent is the best
  final agent. Integration: result hidden minus the oracle best hidden
  candidate in the arm (oracle is a diagnostic ceiling, never a result).
  Repair: in the shared arm, per agent, candidates and best hidden score
  before and after the first read that returned a peer finding, and how many
  later candidates improved the agent's own public best; findings posted,
  reads, refused reads and findings delivered.
- Scores of anytime heuristics vary between runs; the recorded hidden score
  of each candidate is the one computed when it was evaluated. This noise is
  equal across arms and is a limit on precision, reported alongside results.

## Accounting

Rates are $0.125 per million input tokens and $0.50 per million output tokens,
every input token at the uncached rate. Each request reserves a worst-case
charge before it is sent (input bound from request bytes, output at the
requested cap) and settles on returned usage. A request is refused when it
cannot fit the agent's allowance, the phase ceiling or the study ceiling. All
model work, including reading the record, posting, failed calls and the nudge,
is charged to the agent that caused it. Daemon operations are local and free.

## Retained evidence

`run.py` writes a manifest (configuration, source hashes, binary hash, a
digest of each family's frozen public and hidden instance lists, which
regenerate exactly from the study seed and `problems.py`, and reference
costs), the ledger, every trial with its agents' full
visible conversations (system and task prompts are in source, outputs, tool
calls and results), every candidate's source and both scores with timestamps
and spend, shared-goal IDs, principals, receipts and reads, the calibration
gate and the daemon's event log. Hidden reasoning is not retained. API keys
and daemon secrets never enter the records. `analyze.py --evidence DIR` copies
these, and any `voided/` trials, into the evidence folder and writes
`analysis.json`.

## Run

```sh
cd research/experiments/shared_record
/opt/homebrew/bin/python3.12 -m unittest test_shared_record.py
/opt/homebrew/bin/python3.12 run.py --output ../../../output/shared-record-2026-10-08b --phase all
/opt/homebrew/bin/python3.12 analyze.py --output ../../../output/shared-record-2026-10-08b \
  --evidence ../../evidence/shared-record-study-2026-10-08
```

`OPENAI_API_KEY` must be in the environment. Python 3.12 or newer is required
by the daemon helpers. A rerun with the same `--output` loads finished trials
instead of paying for them again. The first calibration ran in
`output/shared-record-2026-10-08`; the amended study uses a fresh directory so
nothing from the failed calibration is reused.

## Amendment A1 (8 October 2026, before any scored call)

The first calibration, run with the protocol as committed in `e493d60`, failed
the gate on criterion 3: in `maxcut/independent` all three agents' final
candidates had the same hidden score, 0.9352, a range of 0.0000. The other
criteria passed: no transport failures; every agent produced a fully valid
candidate; the shared trial published 13 findings with receipts and 7 reads
returned peer findings; vertex cover's independent range was 0.0058, but only
because one agent's public-score tie-break picked an earlier candidate; every
vertex-cover agent, in both arms, also reached one hidden score, 0.9595. The
four trials cost $0.70. The instances (max cut n≈60, vertex cover n≈80, and
held-out families of similar scale) were small enough that 1.5 CPU seconds of
plain Python converged to the same local optimum whatever the approach, which
leaves nothing for selection or a shared record to add. A local check without
API calls showed that simple local search also converges well inside the CPU
budget on the held-out families at their original sizes (2-opt on 100-point
tours and swap descent on 20-facility assignments finish in a fraction of a
second), so the same saturation was expected there. The evidence of this
calibration is retained in
[shared-record-calibration-1-2026-10-08](../../evidence/shared-record-calibration-1-2026-10-08/).

What changed, in this order and before any scored call:

1. Instance sizes rose about tenfold in every family, development and
   held-out, to the values in the Tasks section. Families, generators' forms,
   validators, cost functions, fallbacks and reference heuristics are the
   same. The QAP reference now computes each swap's cost change directly
   instead of recomputing the whole cost; a test checks it returns the same
   permutation as the full recomputation.
2. The public instances moved out of the prompt into `run_python`'s
   `INSTANCES`, because inlining them at the new sizes would have cost a large
   share of each allowance in input tokens on every turn. The prompt now states
   each family's generator (`shape`) in place of the instance data. This applies
   identically to every arm.
3. The manifest records instance digests instead of the instances themselves.

What did not change: the question, the arms, k, the allowances, the selector,
the gate and its thresholds, the development/held-out split, the analysis plan,
the smallest effect worth acting on, and the ceilings. The $0.70 spent on the
first calibration counts against the $15.00 study ceiling; the amended run's
own ceilings ($2.00 calibration, $8.50 scored) keep the total under it.

The amendment was decided after seeing development-family results only. No
held-out task had been run.

### Calibration attempt 2 (amended protocol): vertex cover voided by a network outage

The amended calibration's maxcut trials completed and met their criteria:
independent hidden range 0.0113, 33 findings posted, 13 reads returned peer
findings, best hidden 0.9516. Between 11:50 and 11:56 local time, while the
vertex-cover trials were in their first minutes, every one of their four
agents lost a request: two hung until the client timed out after 1529 s and
two failed at once with `URLError`. `api.openai.com` answered normally again
minutes later and the provider's status page reported no incident. The gate
therefore failed on four transport failures and on the vertex-cover criteria
those failures caused ($0.78 spent on this attempt).

Because this is an infrastructure failure and not evidence about the
protocol, the two vertex-cover trials were rerun, once, in the same output
directory: the finished maxcut trials were loaded unchanged, the ledger kept
every entry including the four failed reservations, and the voided
vertex-cover trial files, that attempt's gate verdict, manifest and daemon
log were moved to `voided/calibration-attempt-2-network-outage/` and are
retained with the evidence. Nothing else was rerun or changed. If the rerun's
gate passes, the scored phase proceeds under the original rule of no retries:
a transport failure in a scored trial stays in that trial's result.

## Amendment A2 (post hoc, 8 October 2026, after calibration 2's verdict)

This amendment weakens the preregistration and is labelled as such. The
vertex-cover rerun completed with no transport failures and the gate failed
on one criterion only: `vertex_cover/independent` hidden range 0.0024, below
0.005. Every other criterion passed: `maxcut/independent` range 0.0113; all
agents produced fully valid candidates; the shared trial posted 33 findings
and 13 reads returned peer findings; best hidden 0.9473. Calibration 2 cost
$1.23 in total, $1.93 with calibration 1.

The vertex-cover result is not the saturation criterion 3 was written to
catch. In calibration 1 every candidate of every agent had one hidden score.
In calibration 2 candidates within one agent ranged from 0.948 to 0.968,
agents were still improving when their allowances ran out, and the solo agent
with three times the allowance finished at 0.9568, 0.009 behind the best
independent agent. The three independent agents simply ended close together.

Decision, by the repository owner when asked: proceed to the scored phase
under a relaxed criterion 3, "at least one independent calibration trial
spans 0.005", rather than stop or tune the development family again. `run.py`
records which rule applied in the gate output; the verdict under the
preregistered rule is kept as `calibration-gate-preregistered-rule.json` with
the evidence. The held-out set, arms, allowances, selector, analysis plan and
ceilings are unchanged.

Consequence for interpretation: the study no longer has calibration evidence
that between-agent spread exists on every family of this kind. The scored
phase measures that spread directly on each held-out task (the
complementarity diagnostic), and the results note must report, per task,
whether the independent agents spread by at least the smallest effect of
interest before any shared-versus-independent difference on that task is
read as informative.

### Scored phase: `unrelated_machines/shared` ran after the others

When the scored phase started, the shared arm of `unrelated_machines` failed
before any agent ran: the daemon rejects principal names outside 1–32
characters of `a-z`, `0-9` and `-`, and the harness derived
`scored-unrelated_machines-agent-0` from the task name. The other five tasks'
shared arms had valid names and ran as scheduled; the task's solo and
independent arms ran as scheduled. The harness now derives valid names and a
test covers every phase, task and agent the study can produce (commits
`0de79bf`, `5a0504d`). The failure also cancelled the two tasks that had not yet started, `mkp` and
`setcover`, because the task scheduler's `map` iterator cancels pending work
when a task raises. These seven trials ran in a later launch of the same
output directory under the same protocol, allowance and tools. The only
differences are time of day and host load; CPU-time limits make host load
irrelevant to scoring, and API conditions are not controlled in any trial.
The run-level `manifest.json` records the source hashes of the last launch;
the git history above gives the sources each trial ran under. Each launch's
daemon log is kept as `daemon-events-launch-N.jsonl`.

### Scored phase: `tsp` and `qap` voided by a second network outage

Between 13:52:08 and 13:52:27 local time, ten agents' requests failed with
`URLError` within seconds of each other: all seven agents of `qap` and three
of the seven of `tsp` (its solo agent, one independent, one shared). The
provider was reachable again at once. Under the preregistered rule these
agents stop and their trials stand; applied here that would leave `qap` with
almost no work in any arm and `tsp` with the solo arm cut off at $0.07 while
two of three agents in each group arm ran to completion. An outage that
removes different fractions of each arm is not the random, isolated transport
failure the rule was written for.

Rule applied: every scored trial with at least one agent lost in that window
is voided and rerun, in full, whatever its scores. The rule was formulated
from the failure timeline alone; the listing used to move the files then
printed each voided trial's selected hidden score, so those numbers were seen
before the rerun and are reported with it. That is all six `tsp` and `qap`
trials. The voided trials are kept in
`voided/scored-outage-1352/` and the results note reports the preregistered
treatment (voided trials counted as they stand) beside the rerun as a
sensitivity analysis. Spend on voided trials counts against the ceilings.

Two development-family observations from calibration 2 are recorded here so
that they are not mistaken for scored results later: on maxcut, shared
(0.9558) finished 0.004 behind independent (0.9516), with its three agents
within 0.005 of one another while the independents spread over 0.011; on
vertex cover, independent (0.9475) finished 0.009 ahead of solo (0.9568) at
the same total allowance. One trial each.

## What this does not test

One model family, one cheap model as the "strong solo", six tasks, one trial
per task-arm, a single machine, and synthetic optimization families. A result
here is about this configuration. It does not qualify Locust's networking or
native coding-agent clients, and it does not establish anything about the
frontier solo model the thesis names as the right baseline.
