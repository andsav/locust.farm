# Shared-record information policy: experiment and implementation plan

Status: harness implemented; calibration and development completed, 12 October
2026. The owner chose a Luna mechanism study plus a frontier solo benchmark
and subsequently authorized a $30 pilot. Evaluator timing instability prevented
the scored phase from qualifying; it was not launched. See the
[findings synthesis](../research/shared-record-information-policy-synthesis-2026-10-12.md)
and [verification note](../research/shared-record-information-policy-verification-2026-10-12.md).
This document preserves the design; individual run manifests supplied the
freezes. It authorizes no additional model calls. No weights were updated.

Basis: the [completed shared-record study](../research/shared-record-study-results-2026-10-08.md),
its [historical protocol](../research/experiments/shared_record/historical-protocol-2026-10-08.md),
the [collaboration thesis](collaboration-thesis.md), and the
[MASS assessment](../research/mass-self-supervision-assessment-2026-10-11.md).
Source inspection used Locust commit `8cedf9140c765da9b3e6bf1088182a3851ac7326`.

## 1. Question and claim boundary

Does a specified peer-information policy improve independently checked results
beyond both independent search and the same structured private review, at a
declared resource budget?

Sakana's [announcement](https://sakana.ai/mass/) calls MASS “Recursive
Self-Improvement through Collective Intelligence.” That can describe its
multi-agent training procedure. It does not, by itself, identify communication
or diversity as the cause of its gains. Our assessment separates workflow
search, execution, selection, training and independent validity. Two update
cycles can demonstrate the specified recursive procedure without establishing
sustained improvement or the causal explanation implied by the broader label.

This experiment manipulates access to peer evidence with fixed model weights.
It tests a narrower claim than MASS and does not reproduce its training loop.
Identical model weights do not imply identical searches; heterogeneous models
are neither required for behavioral diversity nor evidence of useful diversity.

| Claim | Evidence this study needs or can supply |
| --- | --- |
| Parallel attempts help | Independent attempts beat the same-model solo under the shared selector and resource accounting |
| Peer information helps | Peer review beats an otherwise matched private-review continuation |
| Sharing pays for itself | Peer review also beats the efficient independent baseline after its full costs |
| The policy preserves useful differences | Randomized policy changes measured behavioral diversity and candidate complementarity; these are secondary outcomes |
| Diversity causes the quality gain | Not identified here: diversity is a mediator, not a separately randomized factor |
| A model mixture helps | Not tested: Luna weights are fixed throughout the mechanism comparison |
| Collective intelligence causes RSI | Not tested: no training, successive model generations or training-data ablation |

In particular, improvement after reading a record is not a causal repair
estimate. It must be compared with the matched continuation that did not
receive peer evidence. Agreement, delivery and artifact completion are not
the quality endpoint.

## 2. Staged design and five scored arms

Use development runs to compare two new policies with an early-sharing
reference, then freeze exactly one policy for scored evaluation. This avoids
selecting whichever scored sharing arm happens to win. Timing and filtering
are separate interventions; their development comparisons are specified below.

The scored arms use the same task packs, tools, public selector and total
task-arm allowance B. Group size is three. Proposed Luna configuration is
`gpt-6-luna`, effort `high`. The proposed frontier comparator is `gpt-6-astra`,
effort `high`; verify the exact API model, returned identity, context limits
and prices before freezing a run. Substituting a model after freeze requires
a new study identity, not a silent continuation.

| ID | Arm | Procedure and purpose |
| --- | --- | --- |
| S | Continued Luna solo | One worker with B, a development-qualified continuation procedure and access to its own earlier work |
| I | Independent Luna attempts | Three workers with B/3 each; no peer exposure or mandatory review packet; same continuation procedure as S |
| C | Matched private review | Three workers; selected policy's checkpoint, evidence format and review instruction; each sees only its own eligible record |
| P | Selected peer policy | Same as C, plus eligible peer records delivered through the real Locust daemon |
| R | Frontier solo | One frontier worker with B and its separately qualified private continuation procedure; practical quality/cost comparator |

S is the same-model causal baseline. R addresses whether spending the money
on a stronger solo is preferable; a difference involving R also changes the
model and cannot isolate a communication mechanism. R must be able to perform
meaningful work at B; an allowance that barely buys its first response does
not make it a credible strong baseline.

For C and P, the only prescribed difference after their common checkpoint is
peer-record visibility, including truthful attribution. Both receive their
own record. Differences in peer context length, reading costs and subsequent
choices are part of the information treatment. Do not pad C with nonsense or
claim to isolate semantic content from all context-length or social effects.

## 3. Exact development policies

The first study uses one scheduled exposure, not continuous discussion or an
adaptive coordinator. Fractions below partition each worker's allowance;
they are experimental budget allocations, not arbitrary turn or token caps.

| Policy | Independent-stage allowance | Peer records eligible at the checkpoint |
| --- | ---: | --- |
| E: early full reference | B/12 per worker: 25% of B/3 | All of each peer's evaluated candidates in the frozen prefix |
| D: delayed full | B/6 per worker: 50% of B/3 | All of each peer's evaluated candidates in the frozen prefix |
| F: early filtered | B/12 per worker: 25% of B/3 | Only failed or non-improving attempts, defined below |

E and F share the same checkpoint and underlying prefix. E versus D changes
timing, including how much independent work exists at exposure. E versus F
changes eligibility at the same time. D versus F changes both and cannot
identify either component separately. E is a new controlled early-exposure
reference, not an exact reproduction of the old voluntary, repeated-sharing
arm. Do not add delayed-plus-filtered as another candidate in this study.

Each candidate D and F has a matched C variant on development data. Compare
them with I and the solo baselines. Select between D and F using the larger
family-balanced improvement over I at B on development data. If their
estimated improvements differ by less than 0.0025 normalized score, choose
the cheaper policy by actual charged cost; an exact remaining tie selects D.
Record all development results, including E and private-review controls.
Do not require a positive treatment result to pass readiness or retune the
policy after seeing scored outcomes.

### Checkpoint and frozen record

1. Run the three workers independently with identical initial instructions
   and no record tools. Workers are not told their future C/P allocation.
2. At the chosen independent-stage allowance, stop dispatching requests that
   cannot fit it, settle already launched calls and finish their public tool
   operations. An explicit early stop also ends that worker's stage. Unspent
   allowance carries forward. Record actual spend: a 50% allocation is not
   a promise of exactly 50% consumption.
3. Wait until all three prefixes have ended or failed. Freeze their histories,
   candidate objects and source hashes. A recipient must have evaluated at
   least one candidate valid on every public selection instance before peer
   access opens. If it has not, it continues privately; it stays assigned to
   P in the analysis and receives exactly C's private continuation, with no
   later peer exposure. Failed donors can contribute only records actually
   completed before failure.
4. Build an evidence packet deterministically from the frozen evaluated
   candidates. No new summarizer, judge, optimizer or model-written summary
   is involved. This keeps packet preparation separate from another reasoning
   intervention.
5. Publish packets in isolated Locust goals and deliver the C/P review input
   through the same harness interface. Both see their own eligible packet;
   P additionally sees eligible peer packets. Log receipt IDs, eligible,
   withheld and delivered candidate IDs, and reasons for withholding.
6. Allow continuation and public evaluation with the remaining allowance.
   Source retrieval is by explicitly eligible candidate ID. There are no
   additional publications or later peer-derived packets in this study.

Common review instruction, with only the provided evidence varying:

> Review the supplied evaluated attempts. Use the public evidence to decide
> whether to retain your approach, test a specific alternative, or reject a
> suggested change. Verify changes with the public evaluator. You may ignore
> unhelpful evidence. Continue using your remaining allowance.

The packet contains candidate/parent IDs, author, source hash, public-instance
results, validation status, chronological public-best delta, and available
source references. Full source can be retrieved for an eligible candidate;
there is no arbitrary source truncation. Source retrieval and review tokens
are charged. Explicit pagination may be used for a real transport limit, but
must not silently omit eligible records.

Candidate and source-reference IDs are opaque random UUID4 identifiers,
generated once per evaluated attempt and retained unchanged across exact
prefix forks, parent links, retrieval and private-review serialization. They
do not contain internal study, block, arm or checkpoint labels. Worker
attribution remains truthful. Internal execution and accounting identities
remain in the archive for audit. The [runner](../research/experiments/shared_record/run.py),
[identity tests](../research/experiments/shared_record/test_runner.py) and
[request tests](../research/experiments/shared_record/test_interruptions.py)
enforce this label control. It does not conceal whether peer evidence was
received, hide the policy field shared by a C/P pair, or isolate semantic
content from attribution, context length and reading costs. Earlier frozen
studies retain their original runtime and model-visible identifiers.

F keeps a candidate when it failed a public validation check or did not
strictly improve the author's best previously evaluated valid public score.
The first fully valid candidate establishes the baseline and is excluded;
earlier invalid attempts remain eligible. Compute eligibility from the
frozen public evaluation objects, never hidden scores or an agent's opinion.
Use unrounded scores and the frozen scorer's numeric comparison convention.

This is a **failed-attempt record filter**, not a guarantee that only negative
knowledge is communicated: a non-improving program can still reveal a useful
method. State that limitation explicitly. An empty filtered packet is a real
treatment outcome; do not substitute a successful candidate, invent a failure
or drop the trial.

### Common prefixes and interference

Generate a group's independent prefix once, then fork exact request histories
for C and P with separate immutable branch IDs and separate Locust goals.
Run I separately with its full B/3 allowance from the start: no policy-stage
allowance, checkpoint prompt or reset may alter its requests or stopping
points. Do not reuse the staged C/P prefix for I. Charge the common C/P
prefix in full to each arm; actual billing pays for it once.
Record both actual study expenditure and each arm's counterfactual deployment
cost. A reused prefix cannot be free evidence for the sharing arm.

All three workers in a branch form one experimental unit. Never mix C and P
recipients in one communicating goal, let one branch read another's later
work, or count their shared prefix as independent replication. Randomize
continuation launch order in each block and counterbalance order across
blocks; keep all branches of a block close in time. Matching request histories
does not make later API executions deterministic.

## 4. Tasks, selectors and strong solo

Reuse the six former scored families: TSP, coloring, unrelated-machine
scheduling, multidimensional knapsack, weighted set cover and QAP. Use max cut
and vertex cover for prospective development, with historical coloring
artifacts as offline selector-regression fixtures. All eight families have
already been observed. This is a fresh-instance test in known families;
it supports no claim about unseen problem families. A later family-held-out
study would need new families selected before any of their outcomes are seen.

Each new task pack has independently seeded public selection instances and a
sealed hidden evaluation set. Neither reuses earlier study instances. Every
arm in a block receives the same public pack and is scored on the same hidden
pack. Freeze generators, sizes, seeds/digests, references, validators and
candidate CPU allowance before scored execution. Retain the existing
two-CPU-second task definition unless development justifies and records a
common replacement.

The public selector remains algorithmic. Before freeze, enlarge the public
set and/or improve its public validity fixtures until a blinded replay of
known coloring defects separates the defective candidate from the sound one.
Apply the resulting selector to every arm. Rank fully public-valid candidates
first, then their unrounded aggregate public cost, then a source-hash tie-break
independent of arrival order. If none is public-valid, return the family's
fixed fallback. A tie-break cannot repair an uninformative public test set:
if the regression remains unresolved, stop readiness rather than remove
coloring after scoring or select by hidden outcomes.

Qualify the selector's regret and ties on development instances only. Repeated
execution of unchanged candidates estimates evaluator noise; choose public
and hidden repetitions before freeze so this noise is small relative to
the 0.01 useful-effect threshold. Never select a candidate by its best noisy
hidden run. Score identical sources on identical packs with the same frozen
evaluation repetition schedule and retain every outcome.

Strengthen S and R on development data. Compare the existing continuation
with (a) structured private verification plus a materially different attempt
and (b) a fresh private context retaining the worker's own artifacts. Freeze
one procedure per model using development quality/cost, and use Luna's chosen
procedure for I, C and P too. All context resets and reviews count against
the allowance. Do not endlessly prompt a solved worker merely to consume B;
unused spend remains an advantage. Report early stops and what continuation
was actually performed. “Strong solo” means this tested comparator, not an
exhaustive claim about every possible solo strategy.

## 5. Budgets, replication and freeze

Select B on development data so all five arms, especially R, can produce and
review meaningful candidates. Start cost calibration with the historical
$0.42 allowance as a reference point, not an automatic choice. Increase it
only in a funded development phase if needed. A cheaper baseline is never
disqualified for failing to spend enough.

The planning floor is six families, three fresh task packs per family and
three independently generated prefix/solo repetitions per pack: 54 paired
blocks and five scored arms. These are 54 blocks in six known families,
not 54 unseen problems or 270 independent arms. Use development paired
variation to choose the final sample for corrected interval half-width near
0.005 on the two key contrasts. Increase task packs before adding many more
repeats to the same pack. This floor is a costed exploratory design, not a
claim of adequate statistical power. If the funded size cannot achieve the
target precision, label the study exploratory before launching it.

For 54 blocks, a conservative scored allowance is `54 * 5 * B = 270B`:
$113.40 at B=$0.42, or $270 at B=$1. These are allowance examples, not price
quotes or an approved budget. Shared prefixes can reduce actual billing but
do not reduce the attributed resource cost of an arm. Add development,
verification and any separately authorized repetition costs to the study
ceiling. Local evaluator CPU time and daemon overhead are measured separately.

Before any scored call, commit a freeze manifest containing:

- study, block, family, pack, repetition, branch and attempt IDs and randomized
  launch schedule; readiness evidence for the opaque model-visible identifier
  control above, without claiming the information treatment is blinded;
- source commit, source hashes including imported helpers, binary hash,
  Python/runtime identity and tested environment;
- task splits and instance digests; selector, fallback, evaluator repetitions
  and comparison/tie rules;
- exact model/effort and API settings, verified rate table and cache accounting;
- B, stage allocation, selected D/F policy, packet schema, eligible-source
  rules, solo continuation and complete rendered prompts/tool definitions;
- funded development/scored/study ceilings, sample size and analysis version;
- readiness results, failure/resume rules and predeclared claim thresholds.

The manifest is immutable. Resume must compare its hashes with the actual
runtime and refuse mismatches. New policy or task choices after freeze belong
to a new study. The old protocol and results remain historical evidence.

## 6. Outcomes, analysis and claims

For each arm, Y is the hidden normalized cost of the candidate chosen by the
frozen public selector; lower is better. With no deliverable, use the fixed
fallback's hidden normalized cost. Keep transport failures, invalid candidates,
non-triggered exposure and empty packets in their assigned-arm results.

Positive G below means P improved the result:

| Contrast | Interpretation |
| --- | --- |
| G_info = Y_C - Y_P | Primary: incremental peer-record availability beyond matched private review |
| G_net = Y_I - Y_P | Required practical check: net value relative to independent attempts |
| Y_S - Y_I | Secondary: parallel independent search versus continued Luna solo |
| Y_I - Y_C | Secondary: effect of the structured private-review procedure |
| Y_R - Y_P | Practical frontier comparison, not a communication ablation |

The principal endpoint is at the frozen B allowance; show actual consumption
for every arm. Also report selected quality at cost-accounted checkpoints
B/3, 2B/3 and B. Only candidates available by a checkpoint can be selected.
At each stopping cut, charge every request already dispatched, including
pending reservations and all reused-prefix dependencies. Settle and report
actual costs afterward, but do not retroactively admit a candidate that did
not fit the committed-cost bound. Label known usage charges separately from
unknown failed-call reservations; equal allowance is not equal realized work.

Average repetitions within pack, packs within family, then weight the six
families equally. Preserve C/P/I pairing when resampling. For inference about
these fixed six generators, resample task packs within each family and then
paired repetitions within each pack. Do not treat workers, API calls,
individual instances or branches sharing a prefix as independent samples.
Show every family's results and uncertainty; between-family extrapolation is
outside the estimand. Use the exact frozen resampling rule and random seed.

Report 97.5% two-sided intervals for G_info and G_net at B, giving a Bonferroni
95% simultaneous interval family for these two predeclared comparisons,
subject to the bootstrap's sampling approximation. Other comparisons and
diversity analyses and lower-budget checkpoint curves are descriptive. A
result at B does not establish an effect across a resource range. With too
few packs, say the intervals are unstable rather than treating a nominal
interval as strong evidence.

Retain 0.01 normalized score as the proposed smallest useful effect, subject
only to a documented development-stage check of its practical meaning.
Decision wording after freeze:

- Both corrected lower bounds exceed 0.01: evidence that this peer policy
  adds a worthwhile benefit beyond private review and independent search
  on the declared known-family distribution at allowance B.
- G_info's corrected lower bound exceeds zero but G_net does not establish a
  useful gain: peer evidence may help the review procedure without making the
  whole policy worth its opportunity cost.
- G_net's upper bound is below 0.01: a worthwhile net gain is not supported
  at allowance B. An upper bound below zero supports net harm at B.
- Otherwise: inconclusive. Do not promote a favorable point estimate or
  switch the primary comparison to the arm that happened to lose.

Report validity rate, completion, fallback frequency, elapsed time, evaluator
CPU, tokens, cache use, actual/committed cost and request failures separately.
Do not clip fallback penalties, delete difficult tasks or silently replace
outage-hit observations to improve the quality estimate.

### Diversity, integration and repair diagnostics

Freeze pre-exposure candidates before examining any later result. Measure
per-instance cost/validity disagreement, failure signatures and source lineage
before and after review. Where solution equivalence has a simple canonical
form, add solution disagreement. Source hashes detect exact copying, not
conceptual diversity; scalar final-score spread is not enough either.

Report oracle candidate quality and public-selector regret only offline.
The oracle is not an available selector. Compare changes in C and P from
their common prefix: improvement after exposure alone cannot be attributed
to reading. Record damages to formerly valid candidates as well as repairs.
Receipt evidence establishes access; claimed uptake and code changes need
their own evidence. All these measures remain diagnostics: without a separate
diversity intervention, a correlation between diversity and quality is not
proof that diversity caused the gain.

## 7. Reuse and required harness changes

Extend the existing [shared_record harness](../research/experiments/shared_record/README.md)
in place. Keep one parameterized runner and analyzer; do not copy a parallel
runtime or add legacy result readers. Historical results remain reproducible
from their recorded commits and archived evidence. Replace obsolete hardcoded
arm/configuration paths when the new implementation lands. No production
daemon/protocol change is currently justified by this plan.

| Existing component | Reuse and required change |
| --- | --- |
| [problems.py](../research/experiments/shared_record/problems.py) | Reuse generators, validators, references and fallbacks; parameterize pack/repetition identity and instance counts in the manifest |
| [scoring.py](../research/experiments/shared_record/scoring.py) | Reuse sandboxed public evaluation; add frozen repetition/noise records and any qualified public-selector fixtures |
| [record.py](../research/experiments/shared_record/record.py) | Reuse private daemon, real publications and receipts; isolate every branch goal and restrict reads/source retrieval to its frozen eligible set |
| [luna.py](../research/experiments/shared_record/luna.py) | Reuse durable reservation/settlement and per-session accounting; qualify input-token bounds, parameterize model/rates, separate billing from reused-prefix attribution and record pending work |
| [run.py](../research/experiments/shared_record/run.py) | Replace arm-name conditionals with frozen policies; add checkpoints/forks, explicit candidate references, repeated-block identities and safe resume |
| [analyze.py](../research/experiments/shared_record/analyze.py) | Consume the immutable manifest; preserve repeated observations, failures and pairings; calculate corrected costs, contrasts and diagnostics |

Baseline defects and old choices addressed by the implementation below
(this list describes the source snapshot inspected during planning; current
verification is in the linked implementation note):

1. `Agent.tool('evaluate')` currently evaluates hidden instances synchronously.
   Move all hidden evaluation offline, after candidate/selector manifests
   are sealed. Hidden work must not affect live latency, exposure timing,
   packet filtering or operator decisions. Suppress live hidden/oracle output.
2. Trial paths currently omit repetitions; goal titles omit arm identity too.
   Assign short unique daemon principal names and retain their full identity
   map. Never overwrite or cross-read another arm, pack, replicate or attempt.
3. Current launch overwrites the manifest and accepts an existing final JSON
   without configuration validation. Add immutable provenance and explicit
   incomplete-attempt handling; never silently restart a partially paid trial.
4. Current analysis imports live CONFIG and can overwrite repeated trials in
   its dictionary. It also retains inherited-charge logic that predates the
   per-session allowance fix. New analysis must use recorded study/attempt IDs
   and the manifest, with a test for no double subtraction.
5. Current cost checkpoints omit requests that were dispatched but settle
   later. Retain dispatch/settlement identities and committed-cost snapshots.
   Shared prefixes and uncertain failed requests must be included exactly once
   in each consuming arm's attributed ledger. The existing bytes/2-plus-overhead
   input estimate is not a guaranteed upper bound. Before claiming a hard
   ceiling, establish model-specific token counting or a justified worst-case
   reservation covering instructions, tool definitions, replayed state and
   provider overhead; check it against recorded usage. A heuristic estimate
   plus an after-the-fact overspend check does not enforce a funded ceiling.
   If a reliable bound cannot be established, resolve the funding/accounting
   limitation before launching, rather than presenting the estimate as a
   guarantee.
6. Current analysis drops some no-candidate pairs. Preserve every assigned
   block with its fallback, failure category and delivery state.
7. Historical 150-turn, fixed output-token, text/source truncation and
   byte-based history-deletion choices are not part of this protocol. Remove
   incidental limits when adapting the runner. Use only funded resource
   allowances, documented provider/transport constraints and the explicit
   task evaluator limits. Record context exhaustion or real-limit failures;
   do not silently erase history or manufacture equal token consumption.

Transport failures stop that attempt; uncertain requests retain their full
reservation. Continue unaffected branches and record missing/failed branches
without cancellation cascades. No scored observation is voided or replaced
based on an outage or its quality. An unresolved infrastructure interruption
can stop further dispatch; any authorized repeat is a separate linked attempt
with new allowances while the original remains in the primary dataset.
Development infrastructure repairs require a new calibration identity and
preserve all old charges. This removes the need for a post-hoc outage rule.

## 8. Implementation sequence and readiness

1. **Deterministic core, offline:** manifest and identities; public-only live
   runner; independent hidden scorer; public selector; manifest-driven analysis
   and ledger attribution. Replay old failure fixtures without model calls.
2. **Information policies, offline:** frozen candidate packets, E/D/F eligibility,
   C/P history forks, same review instruction, signed delivery through isolated
   real-daemon goals and no cross-branch access. Use scripted model responses.
3. **Funded development only:** qualify private continuation and B; estimate
   evaluator noise and sample precision; exercise policies and select D or F.
   Keep development analysis separate from scored manifests.
4. **Freeze and score:** commit the completed manifest and passing readiness
   evidence, obtain the concrete spending authorization, then execute its
   complete schedule. Run hidden scoring and analysis only after sealed outputs.

Required tests extend the existing
[test suite](../research/experiments/shared_record/test_shared_record.py):
checkpoint boundaries and invalid-initial-candidate behavior; exact filtered
eligibility including first-valid/non-improving/empty cases; complete eligible
source retrieval; identical C/P prefixes; absent hidden calls in the live
path; branch/goal isolation; immutable resume compatibility; repeated blocks;
full uninterrupted I allowances; complete-request token accounting, pending
requests, duplicated prefixes and failed reservations; no-candidate fallbacks;
selector regression; and archive-only analysis reproducing its tables.
Preserve still-required invariants when replacing historical tests.

Before paid development, run the Python suite and the real-daemon scripted
replay; verify costs without paid calls. Before scoring, require delivery and
isolation checks, grader/selector qualification, usable solo baselines,
calibration precision/cost estimates and a complete freeze manifest. Do not
require a sharing win or relax readiness after viewing scored data.

The original suite passed 24 tests during the planning audit with
`/opt/homebrew/bin/python3.12 -m unittest test_shared_record.py` in the harness
directory; that result did not qualify the proposed behavior. The implemented
replacement and its expanded tests are described in the
[current harness instructions](../research/experiments/shared_record/README.md)
and [verification note](../research/shared-record-information-policy-verification-2026-10-12.md).
Documentation changes require
`python3 scripts/check_docs.py`; implementation will also require the focused
Python checks and scripted daemon replay. Rust gates apply only if Rust is
changed, which this plan does not currently need.

## 9. Run record and remaining launch decisions

Retain full visible requests and tool results, exact packet versions,
candidate sources/hashes, dispatch/settlement records, shared-prefix cost
attribution, isolated-goal receipts, policy decisions and withheld IDs,
public selector manifests, sealed hidden scores, every failed attempt and
the offline analysis. Keep secrets and private credentials out of the archive.
Index the eventual protocol and results separately under research.

The offline implementation is complete. Values that require measured
development or owner spending authority remain deliberately unfrozen: B and
total ceilings, exact available frontier deployment/prices, public/hidden
instance counts and evaluator repetitions, chosen D/F policy, selected solo
procedures and final powered or explicitly exploratory sample size. There is
no standing spending authorization from the October 8 experiment.
