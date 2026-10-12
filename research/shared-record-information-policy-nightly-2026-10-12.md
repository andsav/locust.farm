# Shared-record information policy: $30 exploratory run

Status: calibration and policy development completed; scored execution stopped
at the declared evaluator-noise readiness gate, 12 October 2026. **No scored
pilot allocation or scored model calls were made.** The user authorized at most
**$30 total**, including calibration, development, failures and scored execution.
This is a smaller exploratory run of the
[information-policy plan](../docs/shared-record-information-policy-plan.md),
using the [implemented harness](experiments/shared_record/README.md).
It cannot establish the plan's target precision or recursive self-improvement.

## Observed outcome and spending

The funded run completed twelve solo-calibration trials and fourteen
policy-development trials. Its frozen rule selected **filtered sharing (F)**
over delayed sharing.
On the two development families, filtered sharing improved over independent
attempts by 0.004408 normalized cost, but was **0.000076 worse than matched
private review**. These sparse, unblinded development results do not establish
an incremental communication benefit, a diversity mechanism, or recursive
self-improvement. No six-family scored result exists.

The readiness review found CPU-limit validity changes for unchanged programs,
with normalized repeat ranges as large as 0.638879 on public evaluation and
0.374114 on hidden evaluation. The plan requires evaluation noise to be small
relative to the 0.01 useful-effect threshold before freezing the scored phase.
Stable selected F endpoints do not override that broader qualification failure.
This is a measurement-readiness stop, not a requirement that sharing win.

Across all four executed studies, 564 generation requests were dispatched:
563 returned responses with settled usage (559 completed and four incomplete),
and one HTTP 400 retained the uncertain reservation described below.
Known usage totals **$3.61628804**. Including the **$0.089425** retained reservation,
the campaign commits **$3.70571304** of its $30 ceiling, leaving **$26.29428696**
unallocated. These figures are computed from returned usage and frozen prices,
not an independently reconciled invoice. The local-only raw records
`campaign-after-development.json`, `analysis-development.json` and
`qualification-development.json` preserve the result and its evidence boundaries.
Raw manifests, archives and receipts named in this report remain on the local
machine and are excluded from Git. The tracked
[evidence index](evidence/shared-record-night-2026-10-12/README.md)
records their inventory and the retained summaries.

## Funding and sequence

One campaign ledger allocates a separate maximum to each immutable study
directory. Active allocations plus settled charges and unresolved reservations
must stay within $30. For started studies, funds are released only after sealing; failed
requests with unknown charges retain their full reservation. New attempts
require their own allocation and cannot replace failures in an earlier study.
Paid execution checks its campaign allocation before dispatch. Only one local
execution process is used at a time.

The initial arm allowance is B=$0.42. Each group worker receives B/3; early
prefixes receive B/12 and delayed prefixes B/6. Output limits derive from
remaining dollars and verified provider limits. Input reservations use the
provider's exact full-input count, including instructions and tool schemas;
they do not use a character heuristic or a reduced context-window assumption.

1. **Solo calibration:** continued, private verification and private restart
   each run on max cut and vertex cover, once per family for Luna and Astra.
   Each procedure has a $1.68 ceiling: two families × two models × B.
   Select one procedure per model by lowest family-balanced hidden cost of the
   public-selected candidate; exact score ties use lower settled actual cost,
   then the fixed order continue, verify, restart. Retain failures and fallbacks.
   Report whether the requested review/restart actually triggered. These sparse
   results select a tested comparator, not a generally optimal solo strategy.
2. **Policy development:** fresh max-cut and vertex-cover packs, once each,
   with I and the C/P pairs for E, D and F. Ceiling $5.88 before prefix savings.
   Use the selected Luna procedure. Apply the planned D/F selection rule:
   larger family-balanced gain over I; differences below 0.0025 use lower
   actual cost; an exact cost tie selects D. E remains a diagnostic comparison.
3. **Scored pilot:** the six known scored families, S/I/C/P/R, one independently
   generated repetition per pack. Freeze two fresh packs per family if at
   least $25.20 remains after earlier settled costs and retained reservations;
   otherwise freeze one pack per family if at least $12.60 remains. Use the
   same number of packs for every family. No scored outcomes inform this choice.
   If neither design is fundable, retain development results and report the
   funding limit. Do not partially launch a larger design and drop later blocks.

If calibration shows that B cannot produce a useful frontier candidate or
eligible early Luna prefixes, stop and record that failed qualification before
funding a revised development configuration. No scored run is represented as
ready merely because its manifest contains readiness strings.

## Frozen task and measurement choices

The new seed is `information-policy-night-2026-10-12`; calibration, development
and scored packs have distinct names. Calibration procedures share task packs.
The initial calibration/development evaluator uses three public instances,
five hidden instances and two executions per instance. Keep every execution,
including failures and CPU overruns. The task allowance remains two CPU seconds
per instance with the existing 40-second blocked-process guard.

The known K12 coloring defect fixture is appended to scored public packs only.
It checks the historical defect; it does not qualify detection of arbitrary
hidden bugs. Compare repeated source results on development data and record
evaluator noise, selector ties and regret before the scored freeze. Scored
evaluation counts are frozen from that evidence. Hidden scoring begins only
after all live outputs in a study are sealed.

The pilot is explicitly `exploratory: true`. Report C−P and I−P, all arm scores,
actual costs, failed/unused allowances, eligibility, packet contents and source
retrieval. Small-pack bootstrap intervals are descriptive and unstable; one
pack per family yields no pack-level uncertainty estimate. The analyzer must
not emit a confirmatory threshold-based verdict for this run. Disagreement,
copying and repairs remain diagnostics, not an identified causal mediation
effect of diversity.

## Verified provider configuration

Official documentation and account model retrieval on 12 October 2026 identify
`gpt-6-luna` and `gpt-6-astra`. Both use Responses, high reasoning effort,
default processing tier, a 1,050,000-token context window, 922,000 maximum input
tokens and 128,000 maximum output tokens. Account model retrieval succeeded for
both exact IDs; that alone does not verify a generation request.

| Model | Input | Cache read | Cache write | Output |
| --- | ---: | ---: | ---: | ---: |
| GPT-6 Luna | $0.10 | $0.01 | $0.125 | $0.50 |
| GPT-6 Astra | $10.00 | $1.00 | $12.50 | $50.00 |

Prices are USD per million tokens for requests up to 272,000 input tokens.
Above that boundary, input/cache rates double and output rates multiply by 1.5
for the full request. Reserve the highest applicable input category price;
settle ordinary, cached and cache-write tokens separately from returned usage.
These are API usage calculations, not an independently reconciled invoice.
No hosted tools are used. Local sandbox tools carry no provider tool fee.
The current price table lists no separate token-count endpoint fee; those
non-generating calls are retained in a separate complete count audit.

The prelaunch suite passed **118 offline tests** in 9.549 seconds, covering
cache-write and long-context accounting, exact count payloads, campaign funding
races, retained failures and exploratory analysis. The full scripted replay
through the real daemon also passed after these changes. No Rust source changed.

Sources: [pricing](https://developers.openai.com/api/docs/pricing),
[Luna](https://developers.openai.com/api/docs/models/gpt-6-luna),
[Astra](https://developers.openai.com/api/docs/models/gpt-6-astra),
[exact token counting](https://developers.openai.com/api/docs/guides/token-counting),
and [cache accounting](https://developers.openai.com/api/docs/guides/prompt-caching).

## Initial live accounting and provider constraint

The continued-solo calibration completed and sealed all four assigned trials.
Both model identities and default service tiers were returned as requested;
provider input counts matched returned input usage, including real cache reads
and writes. It made 31 generation requests: $0.710231735 in known usage charges
and a retained $0.089425 reservation for one rejected request.

The rejected Astra request followed valid vertex-cover candidates. Its remaining
allowance could fund only one output token; the Responses endpoint requires at
least 16. The harness now stops before dispatch when it cannot fund that provider
minimum. The [official create reference](https://developers.openai.com/api/reference/resources/responses/methods/create)
and the archived HTTP 400 establish this constraint. Earlier candidates and the
failed observation remain in the original study; its reservation is not released
on an assumption about billing.

The two other calibration manifests had not executed. Their original frozen
manifests remain evidence. Explicit cancellation before execution releases their
caps only after verifying that no execution marker, generation ledger, counter
audit or seal exists. Replacement studies use new identities and the fixed
source; their task packs, model settings, procedures and B are unchanged.

## Calibration complete and development freeze

All twelve assigned calibration trials completed with hidden-valid selected
solutions. Continued solo retained the one HTTP 400 above; verification and
restart had no failed generation requests. Across the three studies, known
usage cost was **$2.21414865**, with the **$0.089425** uncertain reservation still
retained: **$2.30357365** committed and **$27.69642635** available before the next
allocation. The independent accounting audit reconciled all completed request
counts, returned model/tier, cache categories and reservation bounds.

The local calibration selection record, `calibration-selection.json`, applies
the frozen rule without a tolerance threshold:

| Model | Continue | Verify | Restart | Selected setting |
| --- | ---: | ---: | ---: | --- |
| Luna | 0.9626274972 | 0.9515844673 | 0.9536086165 | Verify |
| Astra | 0.9484693368 | 0.9482588021 | 0.9485884321 | Verify |

Values are family-balanced hidden normalized costs; lower is better. Neither
selection required the actual-cost tie-break. These are sparse comparator
selection results from two shared development packs, not evidence that one
continuation generally outperforms another. Astra's score differences are
particularly small.

Luna actually executed explicit private verification on both families. Astra
never dispatched its extra private-verification stage at this allowance. It
still produced and iterated several valid candidates per trial; the selected
Astra configuration is therefore a tested budget-limited frontier comparator,
not verified evidence of an additional private-review benefit. In restart's
Astra max-cut trial, the private prompt was queued and input counted, but the
next generation could not fit the provider minimum. A queued flag alone is
not stage-execution evidence.

The frozen two evaluator repetitions showed no score variation or validity
flips across 20 continued, 31 verification and 27 restart source records.
Public ties and nonzero hidden selector regret remain; maximum observed regret
was 0.0021545424. This does not establish general noise or selector performance.

The local policy-development manifest, `policy-development-manifest.json.gz`,
uses fresh `development-pack-0` max-cut and vertex-cover packs, the same seed,
three public/five hidden instances, two evaluator repetitions, and verification
continuation. It assigns I and C/P pairs for E, D and F: fourteen group trials
with a $5.88 maximum allocation. Early and delayed prefixes are generated once
per family and reused as specified. No scored outcomes have been generated.

Actual early-prefix eligibility and real record delivery required qualification
from this phase before any scored freeze. Calibration's first-valid Luna solo
costs were below B/12, but their full-B output reservations differed from group
prefix limits; those observations do not prove group-prefix affordability or
eligibility. The existing gate retains non-triggered P recipients and empty
filtered packets in their assigned results.

## Execution-label defect found during development

A read-only audit of the running development archive found that model-visible
candidate IDs contained internal arm names and early/delayed checkpoint labels.
The first C/P review conversation histories matched except for the supplied
record; their budget-derived output maxima could differ because record input
costs differ. Later evaluation results and serialized private reviews could
disclose those execution labels.
Calibration and policy development therefore do **not** satisfy the planned
execution-label blinding. The effect of this cue on behavior is unknown.

The study retained its frozen source and original observations. A fix was
developed and tested separately: opaque candidate handles preserve internal
accounting identities, truthful author attribution and declared record policy
while removing execution labels from model-facing handles. It was applied only
after the original archive was complete. Policy selection followed the frozen
development rule. No paid observation was replaced or relabeled as blinded
because of this later repair.

## Development live execution complete

All fourteen assigned development trials completed and were sealed before
hidden scoring began. The phase returned **447 generation responses with settled
usage** (446 completed and one incomplete), with **$1.402139390** in known usage
charges, no failed dispatches and no unresolved reservations. Incomplete outputs
remain in the archive. An independent accounting audit matched every
provider input count, recomputed every charge and checked worker, prefix,
arm, phase and study ceilings. Nine additional count requests ended at the
provider's minimum-output budget stop without dispatching generation.

The audit found 163 shared-prefix generation requests. They were billed once
in the study and attributed in full to each assigned branch: 778 arm request
references total **$1.98704157** in counterfactual deployment costs. That larger
attributed total is not extra provider billing.

The independent delivery audit passed all twelve record trials and 36
recipients, with 336 signed publication receipts. Peer metadata appeared in
submitted requests for 17 recipients. Twelve recipients received a peer-source
tool return, and eleven submitted that returned source in a later model request.
The remaining recipient, max-cut P_E agent-0, stopped on budget immediately
after its source retrieval. Receipt, tool return and subsequent model exposure
are therefore reported separately; none establishes semantic use or benefit.

The completed label audit found internal execution labels in **371 of 447**
response-confirmed request payloads. This replaces the earlier partial snapshot
as the complete measurement of the disclosed defect. The isolated opaque-ID
repair passed 128 offline tests, the full scripted daemon replay and an
independent audit of all 306 scripted payloads. It was applied after development
hidden scoring and archive preservation finished, as recorded below.

After development settlement, the campaign committed **$3.70571304**:
**$3.61628804** in known usage and the retained **$0.089425** reservation from
calibration. **$26.29428696** remains. The two-pack-per-family scored allowance
of $25.20 was fundable. The completed hidden analysis selected F, but the
evaluator review below stopped the pilot before a scored freeze or allocation.

The host entered low-power sleep at 03:44:10 PDT and woke on AC at 07:41:23
while hidden scoring was in progress. The
local pause diagnostic, `hidden-host-pause-diagnostic.json`,
identifies one source batch spanning the suspension. Its ten recorded
executions were valid, with identical costs and solution hashes across both
repetitions on every instance. That source is not selected at B or either
reported budget checkpoint in C_D or P_D. Its recorded score therefore cannot
change the frozen policy-selection endpoint. No evaluation was rerun. The
archive lacks per-instance wall timestamps and does not establish a
counterfactual no-suspension result or a 40-second UTC bound across hibernation.

## Completed development selection and readiness decision

The complete hidden analysis scored all 236 distinct source/pack pairs on
five instances with two executions each. The fourteen original public selectors
and all failed executions remain unchanged. The independent qualification
report reproduced the frozen D/F rule: the gain difference was 0.179081 in F's
favor, above the 0.0025 tie threshold, so no cost tie-break was needed.

| Development arm | Mean hidden normalized cost | Gain over I | Mean attributed usage cost |
| --- | ---: | ---: | ---: |
| I | 0.956303 | +0.000000 | $0.082891 |
| C_E | 0.951819 | +0.004484 | $0.142158 |
| P_E | 0.953187 | +0.003116 | $0.153839 |
| C_D | 0.952888 | +0.003415 | $0.162572 |
| P_D | 1.130976 | -0.174673 | $0.179541 |
| C_F | 0.951819 | +0.004484 | $0.131490 |
| P_F | 0.951895 | +0.004408 | $0.141030 |

Lower normalized cost is better; positive gain over I is better. E is early
full sharing, D delayed full sharing, and F early filtered sharing. C uses the
matched private record and P receives eligible peer records. Every group arm
had the same $0.42 allowance; observed spending differed and includes its full
shared-prefix attribution. The 0.004408 F-over-I difference therefore does not
isolate peer information. The C_F/P_F comparison is the more direct information
contrast and slightly favored private review in this development sample.

All twelve observed early/delayed prefix workers produced fully public-valid
candidates within their prefix allowances. First valid early-prefix candidates
cost $0.000971–$0.031644 in known usage, below the $0.035 allowance. The independent
delivery and accounting audits passed. F packets retained their declared
failed/non-improving contents, including empty packets. No outcome was removed
because the policy did not expose or retrieve peer information.

The selected delayed-sharing vertex-cover program passed all six public
executions but passed only three of ten hidden executions. All seven hidden
failures were explicit CPU-budget overruns; three instances failed both
repetitions, and one crossed the boundary between 1.996724 and 2.029037 CPU
seconds. This is primarily a runtime generalization failure with additional
boundary variation. It is not evidence of an invalid-cover semantic defect,
and it is not discarded as an infrastructure failure. Its hidden selector
regret was 0.357721, the maximum among the fourteen trials.

Across the development archive, public evaluations had two instance validity
flips in two source records; hidden evaluations had seven flips in six source
records. Five hidden source records and two public records had normalized
repeat ranges above 0.01. All observed repeat flips were CPU-limit rejections.
The six selected I/C_F/P_F endpoint references represent five unique sources;
those existing repeats were identical, but their proximity to the CPU limit
and the broader observed failures do not establish a low tail-failure rate.
Calling the pilot exploratory relaxes its precision claim, not this declared
measurement-readiness requirement.

**Decision: stop the current paid pilot before the scored freeze.** The two-pack
scored design was fundable, but the noise/selector qualification did not pass.
Do not mark the readiness fields as passed, allocate its $25.20, or treat the
six known scored families as evaluated. Retain the selected F setting as a
development result; any revised runtime or measurement procedure needs a new
qualification identity and prospective evaluation schedule. Additional offline
noise work must use separate diagnostic artifacts and cannot replace the
original sealed evaluations or recompute the original selection from new draws.
The [readiness review](evidence/shared-record-night-2026-10-12/qualification-development-readiness.md)
records the endpoint/source evidence and a proposed, unexecuted diagnostic
panel. It does not establish a replacement evaluator or a passing repetition
count for a later study.

The opaque candidate-ID repair is now applied to the primary checkout in
`b558c99`, after the original development archive was sealed and packaged.
Its 35 source/helper hashes exactly match the independently verified scripted
replay. The isolated repair passed 128 offline tests and the full real-daemon
scripted replay; no paid observation used the repaired identifiers. The
[repair evidence](evidence/shared-record-night-2026-10-12/opaque-id-test-results.md)
and local complete development archive receipt,
`policy-development-sealed-receipt.json`,
retain the source boundary. The original label defect remains a limitation of
all calibration/development findings, regardless of the repair's later success.
