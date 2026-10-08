# Shared record versus independent attempts versus strong solo: protocol

Status: preregistered protocol, 8 October 2026, for the experiment proposed in
the [collaboration thesis](../../../docs/collaboration-thesis.md). This file and
the code beside it are committed before any scored call. Results are reported
separately and do not modify this file.

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

## Tools and limits

- `run_python(source)`: free experimentation with the public instances as
  `INSTANCES`; 20 CPU seconds; nothing recorded.
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
   scores span at least 0.005.
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

`run.py` writes a manifest (configuration, source hashes, binary hash, frozen
instances and reference costs), the ledger, every trial with its agents' full
visible conversations (system and task prompts are in source, outputs, tool
calls and results), every candidate's source and both scores with timestamps
and spend, shared-goal IDs, principals, receipts and reads, the calibration
gate and the daemon's event log. Hidden reasoning is not retained. API keys
and daemon secrets never enter the records. `analyze.py --evidence DIR` copies
these into the evidence folder and writes `analysis.json`.

## Run

```sh
cd research/experiments/shared_record
/opt/homebrew/bin/python3.12 -m unittest test_shared_record.py
/opt/homebrew/bin/python3.12 run.py --output ../../../output/shared-record-2026-10-08 --phase all
/opt/homebrew/bin/python3.12 analyze.py --output ../../../output/shared-record-2026-10-08 \
  --evidence ../../evidence/shared-record-study-2026-10-08
```

`OPENAI_API_KEY` must be in the environment. Python 3.12 or newer is required
by the daemon helpers. A rerun with the same `--output` loads finished trials
instead of paying for them again.

## What this does not test

One model family, one cheap model as the "strong solo", six tasks, one trial
per task-arm, a single machine, and synthetic optimization families. A result
here is about this configuration. It does not qualify Locust's networking or
native coding-agent clients, and it does not establish anything about the
frontier solo model the thesis names as the right baseline.
