# Strong solo versus heterogeneous negotiation: first pilot

Status: measured local API experiment, 2026-10-07. This is a small synthetic
pilot. The results do **not** establish a heterogeneous-negotiation advantage.

The tasks were too easy for the selected models. Every completed original trial
passed all 60 hidden cases for its task, including all six solo trials. In
completed paired trials, the two independently developed initial solutions also
agreed on every hidden output. This creates no opportunity to measure correction
of a substantive disagreement. It is a ceiling effect, not evidence that
negotiation cannot help harder research.

## What was actually run

The [protocol](experiments/negotiation_pilot/README.md) was committed as `869e90d`
before scored calls. It compares GPT-6 Astra and Claude Fable 5.1, each alone,
against an isolated heterogeneous portfolio, one-way synthesis, homogeneous
negotiation with each family, and heterogeneous negotiation. Both models used
high effort through the same direct-API harness. Every participant received the
complete specification, the same public examples, Python execution and evaluator.
No specialist held information unavailable to the solo model.

There were three synthetic coding tasks: causal observed-remove replay, optional
job scheduling, and bounded integral minimum-cost flow. Each had four public
fixtures and 60 frozen hidden fixtures. Hidden expected outputs were never
available to a model or its candidate code. Three tasks are the experimental
units; the 180 fixtures are not 180 independent trials.

The allowance was $2 per task/arm, or $42 for 21 planned trials, within a $50 user
ceiling. Initial work, self-review, exchange and synthesis all counted toward
that allowance. Unused phase allowances were not transferred. This matches
maximum dollar allocations, not actual expenditure or exact compute.

A real local Locust daemon created separate open goals and enrolled principals.
The harness published phase snapshots as signed contributions, and peer prompts
were assembled from authenticated contribution reads. Models wrote the candidate
code and critiques. The experimenter chose membership and exchange timing; this
was not a test of autonomous organization. An independent frozen grader scored
outputs; neither agreement nor a Locust completion declaration established truth.

## Original batch and technical reruns

The initial batch completed 15 of 21 trials. All 15 achieved 60/60. Six trials
aborted because the runner attempted to JSON-decode a tool call cut off at its
output allowance. This was a harness defect, not evidence that those models
could not solve the task. It nevertheless counts as an execution failure of the
original experiment.

The six affected cells were replay/independent, replay/heterogeneous negotiation,
replay/homogeneous A, scheduling/independent, scheduling/heterogeneous negotiation,
and scheduling/homogeneous A. Their original traces and charges are retained.
The malformed final argument bytes
were lost before the original parser wrote its trace; their response IDs, usage
and error messages remain. The repaired parser retains future partial arguments.

Commit `e350a85` corrected the already specified fallback: discard incomplete
calls, retain their visible partial arguments, and keep the best existing public
candidate when an allowance ends. It did not alter the models, task inputs,
graders, prompts, dollar allocations, or counting margins. All six failed cells
were then explicitly rerun in separate goals and a separate output directory,
using the same global spending ledger. These are disclosed technical reruns,
not fresh preregistered replications or invisible replacements in the original
21-trial denominator.

| Arm | Original completed / planned | Original completed scores | Technical reruns completed / attempted | Original charge bound | Rerun charge bound |
| --- | --- | --- | --- | --- | --- |
| Solo Astra | 3/3 | 180/180 | 0/0 | $0.906 | $0.000 |
| Solo Fable | 3/3 | 180/180 | 0/0 | $1.881 | $0.000 |
| Independent Astra + Fable | 1/3 | 60/60 | 2/2 | $1.214 | $0.987 |
| One-way synthesis | 3/3 | 180/180 | 0/0 | $1.688 | $0.000 |
| Astra + Astra negotiation | 1/3 | 60/60 | 2/2 | $0.989 | $0.904 |
| Fable + Fable negotiation | 3/3 | 180/180 | 0/0 | $2.070 | $0.000 |
| Astra + Fable negotiation | 1/3 | 60/60 | 2/3 | $1.367 | $1.797 |

Every completed rerun also scored 60/60. Across the original successes and
technical reruns, each of the 21 cells has one completed result: all 21 score
60/60. All **36 retained initial solutions** from those completed trials also
score 60/60. None of the 15 initial pairs disagrees on a hidden output. There are
no measured hidden-case correctness gains or regressions.

One scheduling/heterogeneous rerun encountered an OpenAI HTTP 503 overload
response. Its full $0.25 request reservation remains charged to the conservative
bound; it is unresolved, not assumed free. A separate second technical rerun
completed. Overall there were **28 trial attempts: 21 completed and seven
technical failures** (six original parser failures and one provider error).

Total conservative API charge bound, including all attempts and smoke checks:
**$13.8380355**, below the $50 authorization. Original scored attempts cost
$10.1155495; the first technical rerun batch costs $3.0575765 including the failed
request reservation; the second technical rerun costs $0.6308145; and API smoke
checks cost $0.034095. All charges use recorded response usage except the retained
$0.25 reservation. There were 212 requests in the ledger including smoke checks
and that unresolved request. The same final global ledger is copied into all
three evidence packages for audit; those copies must not be summed together.

## What the exchanges show

The heterogeneous flow trial used different implementations: Astra
used meet-in-the-middle dynamic programming over balance states; Fable used a
lexicographic depth-first enumeration with pruning. Both initial implementations
already passed every hidden fixture.

Astra's final implementation incorporated the peer's self-loop observation:
choose the upper flow bound for a negative-cost self loop, and the lower bound
otherwise, instead of enumerating every possible self-loop flow. The retained
source diff confirms that change. Its probe output records 1,000 randomized
comparisons against exhaustive enumeration, both before and after the change.
This supports the narrow observation that a contribution was reused. It does
not demonstrate increased correctness, runtime speedup, or mathematical novelty.

The initial Astra source hash was
`ccc1c075e452f85d695929a626dc8bcd670dbbb00556303befb469a151383373`;
the final hash was
`351b85c39e0f2c3ae19d602af531d62446ef74e0c97b59d3abd9dfc27ba122a3`.
Both exact sources, peer contributions, author/event IDs, public explanations and
tool results are retained in the evidence package.

Source diversity alone is not epistemic disagreement. Different algorithms can
produce the same correct answer. This batch did not produce a measurable
wrong-to-right correction on the hidden cases in its completed trials.

## Limits exposed by the pilot

- **Difficulty:** small exact state spaces made complete enumeration or simple
  dynamic programming sufficient. Both models solved the hidden cases before
  negotiation. More repetitions of these same tasks would not fix this ceiling.
- **Treatment delivery:** the input reservation deliberately overestimated
  charges. As histories grew, small exchange allocations sometimes allowed only
  partial calls or no new model response. A declared phase is not evidence that
  a useful reciprocal exchange occurred. The analysis records response counts,
  incomplete responses and budget stops per phase. Among the 84 declared phases
  in completed trials, four received no model response, 13 recorded responses
  ended incomplete, and 23 budget stops occurred. Three of the zero-response
  phases were peer critiques, so not every negotiation cell received the full
  intended reciprocal exchange.
- **Budget:** equal ceilings produced different actual charges. The pilot cannot
  support a claim about equal realized compute or a general cost/quality curve.
- **Replications:** one trial per task/arm, only two model families, and a 2:1
  split of final model identity in mixed groups leave substantial uncertainty.
- **Scope:** this establishes local signed transport and instrumented API trials.
  It does not qualify LAN behavior, real-agent native clients, distributed
  recovery, autonomous swarm scheduling, or research-level mathematical work.

For the next comparison, use a separate calibration set to find tasks where a
strong solo model sometimes fails and independent candidates make different,
checkable claims. Suitable work would include multi-file protocol repairs with
held-out interleavings or constrained search with graded solution quality.
Freeze the scored cases after calibration. Measure which peer counterexamples
cause verified repairs, include both strong solo baselines and isolated search,
and report quality against actual expenditure. Carry unused budget within a
trial and reduce avoidable prompt/code retransmission before choosing small
exchange allocations. Those are proposed follow-up design changes, not changes
silently applied to this experiment.

The [follow-up research note](merak4-tag-team-comparison-2026-10-07.md) records
the bounded hypothesis, compares Merak4's historical tag-team and hypothesis
swarm mechanisms, and adds shared-state handoffs and context-refresh controls
to the proposed next experiment. It does not change the frozen pilot results.

## Reproducibility and verification

The [analysis script](experiments/negotiation_pilot/analyze.py) regenerates scores,
behavioral-disagreement counts, phase-delivery measurements and cost summaries.
The [task module](experiments/negotiation_pilot/tasks.py) contains the frozen
specifications, seeds and exhaustive graders. The protocol explains the sandbox,
selection rule, original command and repair flags.

- Original batch: [analysis](evidence/negotiation-pilot-2026-10-07/original/analysis.json), [public evidence](evidence/negotiation-pilot-2026-10-07/original/evidence.json), [checksums](evidence/negotiation-pilot-2026-10-07/original/manifest.json).
- Parser repair batch: [analysis](evidence/negotiation-pilot-2026-10-07/repair/analysis.json), [public evidence](evidence/negotiation-pilot-2026-10-07/repair/evidence.json), [checksums](evidence/negotiation-pilot-2026-10-07/repair/manifest.json).
- Provider-error retry: [analysis](evidence/negotiation-pilot-2026-10-07/repair2/analysis.json), [public evidence](evidence/negotiation-pilot-2026-10-07/repair2/evidence.json), [checksums](evidence/negotiation-pilot-2026-10-07/repair2/manifest.json).


Evidence includes public prompts and responses, tool calls/results, every retained
candidate, hidden scores, signed-contribution IDs and provider usage. It excludes
API keys, local capabilities, private daemon stores and provider hidden reasoning.
The usage figure is a conservative estimate from recorded API usage and the
verified pricing schedule, not an independently audited invoice.

The tracked bundle can regenerate its analysis without the ignored runtime
profiles. For example:

```sh
python3 research/experiments/negotiation_pilot/analyze.py \
  --from-evidence research/evidence/negotiation-pilot-2026-10-07/original/evidence.json \
  --destination output/negotiation-pilot-reanalysis
```

Verification: all 11 focused grader, sandbox, budget and partial-call tests passed;
API tool round trips for both providers passed; a two-principal Locust publication
and readback passed; the copied daemon built with `cargo build --locked -p locust`;
and documentation/link checks passed. No Rust source or website code changed,
so the full Rust and website suites were not run for this research-only change.
