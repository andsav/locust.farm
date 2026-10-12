# Shared-record information policy: findings and research direction

Status: synthesis of the completed calibration and development run, 12 October
2026. The six-family scored phase was not launched. This note records findings
and recommends priorities; it adds no paid run, accepted architecture decision,
or claim that collaboration generally helps or fails.

## Conclusion

The experiment established that we can deliver and account for controlled peer
records through the real Locust daemon. It did **not** establish that those
records improve results beyond matched private review. Filtered sharing was
the selected development policy, but its selected solutions essentially tied
private review and cost more. Delayed sharing suffered a runtime failure on
hidden instances. Broader timing variation and selector weaknesses prevented
the planned scored comparison from qualifying.

The useful next question is whether the existing traces contain a concrete
case of useful information transfer. Extensive timing repair and a rerun of
the same benchmark are low priority unless maintaining that benchmark has
independent value. A reliable measurement would make a later result more
credible; it would not supply a reason to expect a sharing advantage.

## What we tested

The [MASS assessment](mass-self-supervision-assessment-2026-10-11.md) motivated
a narrower question than the paper's combined workflow-search and fine-tuning
procedure: with model weights fixed, does giving an agent peer evidence improve
its next attempt beyond its own private review and independent search? This
does not reproduce MASS, test model learning, or identify diversity as the
cause of improvement. The [plan](../docs/shared-record-information-policy-plan.md)
separates those claims.

The authorized $30 pilot completed twelve solo-calibration trials followed by
fourteen group-development trials on fresh max-cut and vertex-cover packs.
Calibration selected private verification for both Luna and Astra. Astra
produced and iterated valid candidates, but its extra private-verification stage
did not dispatch within the allowance. It is a tested budget-limited comparator,
not an exhaustively optimized solo baseline.

Development compared three independent attempts with matched private and peer
review under early-full, delayed-full and early-filtered record policies.
Filtered records included failed and non-improving attempts. C/P branches
reused the exact same prefix histories; peer eligibility required an own
public-valid candidate. Record costs could change their affordable output
limits. Every arm had a $0.42 ceiling, with all actual usage and failures charged.
Public-only selection was frozen before hidden evaluation.

## Observations that matter

These are family-balanced development means on one pack per family. Lower
hidden normalized cost is better. They are descriptive, not a final or
confirmatory comparison.

| Procedure | Hidden normalized cost | Mean attributed usage cost |
| --- | ---: | ---: |
| Independent attempts | 0.956303 | $0.082891 |
| Private review, matched to filtered sharing | 0.951819 | $0.131490 |
| Filtered peer sharing | 0.951895 | $0.141030 |
| Private review, matched to delayed sharing | 0.952888 | $0.162572 |
| Delayed peer sharing | 1.130976 | $0.179541 |

The frozen rule selected filtered sharing over delayed sharing. Filtered
sharing improved over independent attempts by 0.004408 normalized cost, but
private review improved by 0.004484. The incremental peer-information contrast
was therefore **−0.000076**, slightly favoring private review. Equal ceilings
did not produce equal actual spending. The gain over independent attempts
cannot be credited to peer information when private review reached the same
quality with less spending.

The selected filtered/private endpoints were valid and identical across their
recorded repeats. In vertex cover, both selected the same inherited candidate
from before record exposure. The timing defect therefore does not explain away
the observed absence of an incremental filtered-sharing gain. It could have
affected earlier search and selection; we have not measured that counterfactual.

Delayed sharing's selected vertex-cover program passed all six public
executions but exceeded the two-second CPU allowance on seven of ten hidden
executions. Three hidden instances failed both repeats; a fourth straddled the
cutoff. This was mainly failure to meet the runtime budget on different inputs,
with additional timing variation. Other candidates tied for the public minimum
were hidden-valid, but the frozen source-hash tie-break selected the slower
one. The selector's large hidden regret remains part of the result.

These findings and all seven development arms are preserved in the
[execution report](shared-record-information-policy-nightly-2026-10-12.md)
and [compact result summary](evidence/shared-record-night-2026-10-12/summary.json).
The complete sealed analysis remains local, as described in the
[evidence index](evidence/shared-record-night-2026-10-12/README.md).

## What the checks establish

| Verified observation | Limit of the inference |
| --- | --- |
| All twelve early/delayed prefix workers produced a public-valid candidate within their slices | Prefix feasibility on these development packs, not on every task |
| Delivery/isolation audit passed twelve record trials and 36 recipients, with 336 signed publication receipts | Correct transport and exposure rules, not useful uptake |
| Peer metadata was submitted for 17 recipients; 12 received peer source returns and 11 submitted those sources in a later model request | Access and subsequent exposure, not understanding or beneficial use |
| Accounting audit reconciled input counts, returned model/tier, cache categories, usage and reservations | Usage-based cost calculation, not an independently reconciled invoice |
| The repaired harness passed 128 offline tests and a full scripted real-daemon replay | Implementation behavior; synthetic replay scores are not model-quality evidence |

One source retrieval had no subsequent model request because the recipient
stopped on budget. Empty packets, incomplete model responses, failed requests
and invalid candidate executions were retained. Shared prefixes were paid once
experimentally but attributed in full to every branch that reused them.

## Why the scored phase stopped

Unchanged programs sometimes crossed the CPU limit between repeats, causing a
large fallback penalty. Two public source records and five hidden source
records had normalized repeat ranges above the planned 0.01 useful-effect
scale; the largest ranges were 0.638879 and 0.374114. The
[readiness review](evidence/shared-record-night-2026-10-12/qualification-development-readiness.md)
concluded that the evaluator-noise requirement had not been met.

The six selected I/C_F/P_F endpoint references represented five unique sources,
each evaluated twice per instance. That does not qualify a low tail-failure rate for a later
study. Calling the pilot exploratory would limit its precision claim, but would
not satisfy this separate measurement requirement. Raising the cutoff or
discarding runtime failures would change the task, not retroactively validate
the original comparison. Modest repetition increases are not a demonstrated
repair either.

There was a second limitation: candidate identifiers exposed internal execution
labels in calibration and development. The opaque-ID repair was verified and
applied after preserving the original archive. It does not make the paid data
blinded retroactively. The recorded host-sleep episode affected a different,
unselected source batch; it does not explain the observed CPU-boundary failures.

The six-family scored study was fundable but **never allocated, frozen or
executed**. We therefore have no scored comparison against strong solo, no
estimate across those six families, and no unseen-family generalization result.

## What follows for Locust

The result leaves the incremental value of the shared record unestablished.
It is consistent with the earlier research's caution about treating exchange,
agent count or model diversity as a quality improvement, but does not turn
those distinct small studies into a pooled causal estimate. Locust's verified
record and delivery properties remain useful infrastructure facts; their
effect on problem-solving quality must be demonstrated separately.

The recommended next work is a small analysis of the existing archive, using
the actual available and delivered information:

1. Identify a peer discovery absent from the recipient's own prior work.
2. Trace whether receiving it changed a subsequent artifact or test.
3. Compare that change with the matched private continuation, including harmful
   changes, non-use, budget stops and additional cost. Treat individual examples
   as mechanism evidence to investigate, not an estimated causal effect.

Only a concrete mechanism would justify designing another paid comparison.
Repository repairs or investigations where attempts discover complementary
tests, bugs or evidence are plausible proposals, not established better tasks.
Such a study should retain independent attempts and private review, and include
a solo comparator given the same combined evidence, so that extra information
is not confused with a benefit from agent-to-agent interaction. Freeze the
task, resource accounting and acceptance checks before seeing outcomes; accept
another null result if that is what the comparison finds.

The proposed 5,760-execution timing diagnostic remains **unexecuted and
deprioritized**. Its value would be diagnosing this evaluator for future use,
not making the current sharing result positive. No new experiment is started
by this synthesis, and no product or architecture change follows from it.

## Cost and evidence boundary

Across the four executed studies, 564 generation requests produced 559 completed
responses, four incomplete responses with settled usage, and one HTTP 400 with
an uncertain charge. Known usage is **$3.61628804**; retaining the full
**$0.089425** uncertain reservation commits **$3.70571304** of the $30 ceiling.
**$26.29428696** remains unallocated. Development used $1.40213939 of that total;
its $1.98704157 sum of branch-attributed costs includes duplicated prefix
attribution and is not extra provider billing.

The [evidence index](evidence/shared-record-night-2026-10-12/README.md) keeps
the findings, compact summaries and diagnostic source in Git. Raw manifests,
archives, traces, detailed audits and original-history backup remain local and
ignored. A fresh clone contains the code and conclusions but cannot independently
reproduce the paid results without those local records. This synthesis performed
no model calls or candidate reevaluations. Original source identities and raw
bytes are preserved locally when the experiment commits are consolidated;
frozen manifests and seals are not rewritten to name the consolidated commit.
