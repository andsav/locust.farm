# Development qualification and scored-phase stop

Status: archive-only review completed on 12 October 2026. The current paid pilot
stops before scored allocation, freeze or calls because evaluator-noise and
selector readiness have not been established. The additional diagnostic below
is a proposal, not a preregistration, required acceptance procedure, or executed
experiment. No candidate was rerun, no original score was replaced, and no
model call was made for this review.

The original [plan](../../../docs/shared-record-information-policy-plan.md)
requires development qualification of selector regret and ties, with evaluator
repetition chosen so unchanged-candidate noise is small relative to the 0.01
useful-effect threshold. The sparse stability of the eventual F endpoints does
not qualify the broader evaluator and selection process. Calling a scored run
exploratory would address its limited precision, but would not satisfy this
separate readiness requirement.

Raw evidence remains local-only in `qualification-development.json`,
`qualification-development-readiness.json` and `policy-development-sealed.tar.xz`.
These files and the other raw reports named below are excluded from Git; the
tracked [evidence index](README.md) records their inventory and retained summary. The
[read-only report builder](diagnostics/development_readiness.py) binds its
output to the manifest, live seal, hidden scores, hidden seal, analysis, and
qualification hashes. It never executes candidate code or imports the harness.

## Observed development results

The frozen rule selects F: its family-balanced gain over I is 0.004407773,
while D's gain is -0.174672812. The D-minus-F gain difference is
-0.179080585. This is a descriptive choice between two policies on one pack
in each of two known families; it is not evidence of a general sharing benefit.
The original selection remains unchanged.

All six early-prefix workers and all six delayed-prefix workers produced a
fully public-valid candidate within their assigned slices. Observed cost
through the first such candidate ranged from $0.0009706 to $0.0316437 for
early workers and $0.001834925 to $0.031676575 for delayed workers. These are
actual prefix observations, not extrapolations from full-allowance solo runs.
Exact delivery and accounting qualification are separate archived audits.

Across 241 distinct public candidate evaluations, two records had normalized
repeat ranges above 0.01; the largest was 0.638878602. Across 236 unique hidden
family/pack/source evaluations, five records exceeded 0.01; the largest was
0.374113687. There were two public and seven hidden instance-level validity
flips. Every observed score variation coincided with CPU-limit rejection;
the associated errors were not invalid-solution errors. Invalid candidate
rows remain scored with their fixed fallback cost.

The I/C_F/P_F selected endpoints had identical costs in their two recorded
public repeats and two hidden repeats, and every selected hidden row was valid:

| Family | Endpoint | Hidden normalized cost | Hidden repeat range | Largest public CPU seconds | Largest hidden CPU seconds |
| --- | --- | ---: | ---: | ---: | ---: |
| Max cut | I | 0.957826158 | 0 | 0.990269 | 0.945182 |
| Max cut | C_F | 0.951799772 | 0 | 1.937744 | 1.928118 |
| Max cut | P_F | 0.951951789 | 0 | 1.946223 | 1.907367 |
| Vertex cover | I | 0.954779412 | 0 | 1.384915 | 1.499385 |
| Vertex cover | C_F | 0.951838235 | 0 | 1.760471 | 1.845813 |
| Vertex cover | P_F | 0.951838235 | 0 | 1.760471 | 1.845813 |

These six endpoint references represent five unique evaluated sources: the
vertex-cover C_F and P_F arms selected the same inherited prefix candidate.
Their duplicate table entries are not independent repetition evidence. Two
successful repeats also do not establish a low probability of crossing the
CPU boundary on a future execution. The max-cut F endpoints were already
within approximately 0.05–0.07 CPU seconds of that boundary on observed runs.

## Why the delayed vertex-cover endpoint failed

The selected P_D candidate is
`b-e4b7a4ed895a0da5ef835156/P_D/agent-2/c14`, source SHA-256
`4603772dd45313b0e5045e0c0a530e113dd4a1837e521312c740c39502cd9ed3`.
It passed all six public rows, with CPU time 1.638273–1.803268 seconds and
public normalized cost 0.953134896. Three sources tied for that public minimum;
the frozen source-hash tie-break chose this source.

On hidden instances it passed only three of ten rows. Three instances failed
the 2-second CPU allowance in both repetitions. A fourth crossed the boundary:
2.029037 seconds failed, while 1.996724 seconds passed. The remaining instance
passed twice. All seven failures explicitly report excessive measured CPU
time, from 2.029037 to 2.233652 seconds. Hidden normalized costs were
1.361029412 and 1.260294118, averaging 1.310661765; selected-versus-best hidden
regret was 0.357720588.

The source performs fixed numbers of seeded searches, importing `random` and
`heapq`, with no clock-based stopping. This supports a runtime-generalization
failure plus CPU-boundary variation. It does not establish an invalid-cover
semantic bug. The evaluator rejects over-budget results before validating their
solutions, so the semantic validity of those seven returned results was not
established. More averaging would not make the repeatedly over-budget runs
valid or remove their intended fallback penalty.

The earlier local host-sleep diagnostic, `hidden-host-pause-diagnostic.json`, concerned
a different max-cut source, which was not selected by any endpoint and had zero
observed repeat variation. It does not explain these CPU failures. Per-row UTC
timing was not captured in the original hidden archive, so elapsed time across
the host pause must not be presented as active evaluator throughput.

## Repetition alone is not an immediate repair

As a rough scale calculation, use the sample standard deviation of the two
observed complete-pack repeats and divide it by the square root of a proposed
repetition count. At 20 repetitions, the nominal standard error would still
be about 0.01593 for the selected P_D vertex-cover source and 0.05915 for the
most variable hidden source. These both exceed 0.01.

Under those same unverified independent, stationary assumptions, even getting
nominal standard error below 0.0025 would require roughly 812 and 11,197
repetitions respectively. Here 0.0025 only illustrates one quarter of the
useful-effect scale; it is not a new readiness threshold. Two-repeat variance
estimates are far too weak to prescribe these counts, and host timing may not
be stationary. The calculation explains why changing two repeats to a modest
number is not a demonstrated solution.

A later study should qualify a revised task/evaluator runtime procedure and
repeat the affected calibration under a new identity. It should define the
runtime estimand explicitly: for example, whether the hard time boundary is an
essential task failure criterion, and how its measurement variation is
controlled. Neither silently raising the CPU limit nor dropping runtime
failures preserves the current estimand. Any change to task, selector, runtime
budget or scoring procedure must retain these original outcomes as historical
development evidence, with fresh calibration rather than retroactive rescoring.

## Proposed diagnostic, not executed

The local source-bound proposal, `proposed-evaluator-noise-diagnostic.json`, fixes a
36-source panel: 13 max-cut and 23 vertex-cover sources. The panel is the union
of all selected endpoints, every fully public-valid source tied for an arm's
public minimum, every fully public-valid but hidden-invalid source, and every
source with observed repeat variation. Source hashes and exact archived
candidate references are included; no favorable-survivor filter is applied.

The proposed schedule has 20 complete passes through each source's original
three public and five hidden instances, for 5,760 isolated solve executions.
Passes alternate the two original recorded seeds, repeating each seed ten
times. A specified hash order distributes sources across passes; execution is
serial, using the archived interpreter, scorer, problem generator and CPU/wall
limits. Every row would record CPU time, validity, cost, error, solution hash,
UTC start/end and host sleep state in a new journal. Ten fresh two-pass means
and the all-pass mean would be reported without replacing original outcomes.

This is a bounded diagnostic sample for understanding instability and choosing
a later design. It is not expected to prove readiness by itself. Execution is
not scheduled as part of this pilot. The stop follows the declared noise
readiness gate, not a lack of user authorization.
