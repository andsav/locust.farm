# Heterogeneous collaboration: lessons from the pilot and Merak4

Status: research interpretation and proposed follow-up design, 2026-10-07.
The Merak4 findings are a read-only source and saved-artifact assessment. No
new model calls or historical benchmark reruns were made for this comparison.

## The hypothesis we are keeping

The research question is whether different models can combine different useful
approaches to produce a better verified solution than a strong single model
with the same information, tools and total resource allowance. Negotiating over
evidence is one possible mechanism. Switching the model working on a shared
problem state is another. Neither mechanism requires a claim that a model's
internal priors are directly observable.

A testable claim is:

> On a defined set of difficult tasks, heterogeneous evidence exchange improves
> independently verified solution quality at a matched resource allowance over
> both strong solo baselines and an isolated portfolio of the same models.

This is a hypothesis, not a conclusion from the
[first Locust pilot](negotiation-pilot-results-2026-10-07.md). That pilot exposed
a ceiling: all 21 completed cells, including the six solos, passed all hidden
cases; all 36 retained initial solutions were already correct. Different source
algorithms and one observed reuse of a peer's improvement establish approach
diversity and information transfer, but no measured correctness advantage.
Some small phase allowances also prevented the intended exchange from happening.

The next experiment needs difficult held-out tasks, observable complementary
errors or graded quality, reliable delivery of each treatment, and separate
measurement of diversity, synthesis and reciprocal exchange. Strong intuition
is a reason to make that test discriminating; an easy task or an undelivered
exchange cannot establish or refute the claim.

## What Merak4 implements

Inspected checkout: `merak4`, commit
`c38e99c5955fd10bbc42a4102c4ea3d1753e4ada` (2026-03-14), clean tracked state.
Paths below are relative to that repository. The
[audit record](evidence/merak4-tag-team-2026-10-07.json) retains source hashes,
per-trial metadata and selected log fields. It does not establish that this
source commit was the build used by the earlier March runs.

### Tag team: serial execution with observers

`observer/tagteam.go:85` runs one `agent.Agent` and rotates its provider/model.
The other slots observe the executing agent's telemetry. Their observer memory
persists across flips (`observer/tagteam.go:180`), so this is one shared execution
trajectory with additional observer histories, not literally one identical
context for every participant.

On a handoff, the runner checkpoints the shared working tree, asks the incoming
model's observer for a diagnostic handoff, and requests an atomic compaction and
provider swap (`observer/tagteam.go:577`, `:597`, `:618`). The handoff includes
accomplishments, failed approaches, diagnosis and next steps
(`observer/observer.go:124`). `agent/compaction.go:43` summarizes history and
strips provider-specific tool blocks and images. Switching therefore changes
both the executor and its representation of the accumulated context.

Flips can follow the configured timer or repeated high-severity observer
interventions (`observer/tagteam.go:277`). Observer behavior is not uniform:
`taskrunner/slots.go:43` assigns an adversarial mode to one slot and tag-team
mode to the others. The latter is explicitly a progress/stuckness monitor rather
than a source of detailed technical strategy (`observer/prompt.go:90`); the
handoff has a separate diagnostic prompt. A no-flip configuration can still
contain model observers. In particular, `MERAK_ADVERSARIAL` disables flips, not
all other model participation (`taskrunner/main_path_runner.go:276`). It is not
by itself a pure-solo baseline.

The inspected default profile has GPT-5.4 and Opus 4.6, with the third executor
slot empty (`taskrunner/models.go:30`). Earlier logs include three-model runs.
Descriptions of a three-slot architecture should not be substituted for a
particular run's actual model sequence.

### Hypothesis swarm: independent proposals, then synthesis

There is also a closer precursor to the Locust pilot in
`cmd/merak/swarm_explore.go:85`: four roles, literalist, contrarian, domain mapper
and verifier-risk analyst, propose alternative interpretations and cheap tests.
Workers receive a shared task/workspace/presearch packet and run concurrently
(`:201`, `:218`). A configurable model pool can supply different models; four
roles alone do not imply four model families.

One pruning model combines the proposals into hypotheses, discriminating tests
and an executor brief (`:129`, `:269`). The results become `HYPOTHESES.md` and
`.hypotheses.json`, and the executor is prompted to run a discriminating test
before settling on an interpretation (`taskrunner/main_path_runner.go:506`).
Workers propose tests rather than execute them in this stage, and do not receive
the judge's response for a reciprocal revision round. This is independent
hypothesis generation plus one-way synthesis, not a demonstrated negotiation.

The inspected router/default derivation sets `SwarmExplore = false`
(`taskrunner/router.go:195`, `taskrunner/pipeline_config.go:213`). The comment
says the feature added latency without clear benefit. That is a recorded
engineering judgment, not a controlled estimate of its effect.

No separate implementation named "melded" was found in the inspected source
or matching commit-message searches. The concrete mechanisms located here are
model handoffs, observer interventions and hypothesis synthesis; there is no
basis in this inspection to describe them as model-weight merging.

## What the retained experiments establish

The audit enumerates all local files matching `jobs/*/*/result.json` and
`jobs/*/*/agent/merak.log`, including ignored job artifacts. It found:

| Retained artifact | Observation |
| --- | --- |
| Per-trial results | 696 files, covering 451 distinct task names; 100 task names have repeated attempts |
| Recorded verifier outcomes | 360 reward 1, 320 reward 0, 16 with no reward |
| Cost fields | `agent_result.cost_usd` is null in all 696 results |
| Detailed logs | 234 files; 144 have a corresponding retained result, 90 do not |
| Actual executor switching | 113 logs contain flips and more than one executor model |
| Hypothesis-swarm activity | Five logs contain a start marker; three contain a completion marker |

These counts describe the local archive, not independent experimental samples
or a benchmark success-rate estimate. Tasks, retries, harness versions and
configurations are mixed. A top-level `config.agent.model_name` is insufficient
to classify a run as solo: 690 results name GPT-5.4 there, while detailed logs
can show multiple actual executors.

For a concrete positive example, the retained trial
`jobs/2026-03-11__20-10-07/instance_ansible__ansible-811093__zHsmvdN/` records
Opus 4.6, then GPT-5.4, then Gemini 3.1 Flash Lite Preview as executor. Its log
contains 19 observer interventions and its result records external verifier
reward 1.0. The model turns occur at log lines 290, 739 and 1247. This is direct
evidence that a heterogeneous tag team ran and produced a passing result. It
does not show that either handoff caused the pass or that a matched solo run
would have failed.

The swarm evidence has a revealing limit. All five starts are Raman-fitting
attempts from March 8. Each logs three worker failures. All three completion
markers report **one successful worker**, three hypotheses and three tests;
none of those three has a corresponding retained per-trial result. Only one of
the five has a result, which records cancellation and no reward. Thus these
artifacts do not demonstrate a complete four-worker treatment or its efficacy.
This echoes the Locust pilot's delivery problem: a phase name or completion
marker does not prove that the intended group interaction occurred.

The human-maintained trackers are useful engineering history, but not a matched
comparison. `benchmark-tracker.md` reports 72/83 wins while another line says all
85 tasks were tested; `swe-bench-pro-tracker.md` reports 272/365 while its passed
section heading says 262. Entries also explicitly mix reruns with restored web
search, dependency changes, removed model slots and new judges. These figures
must not be compared to the Locust pilot as if only coordination differed.

One tracker diagnosis is especially relevant: for `raman-fitting`, it says the
research model misread the units and the executor trusted the resulting wrong
specification. This is a retained diagnosis, not a cause independently reproduced
in this audit. It illustrates the failure mode our next experiment should test:
sharing an interpretation can spread an error as well as repair one.

No matched solo/heterogeneous ablation with complete resource accounting was
found in the inspected source, trackers and retained results. This is a limit
of the evidence found, not a claim that no such experiment was ever run.

## How the approaches compare

| Mechanism | What remains separate | How another model affects the answer | Main question |
| --- | --- | --- | --- |
| Merak4 tag team | Observer histories and model identity; one evolving workspace | Observes, diagnoses and eventually takes over execution | Does changing the solver help escape an unproductive trajectory? |
| Merak4 hypothesis swarm | Initial hypothesis proposals, with role prompts and configurable models | A judge synthesizes proposals into tests for one executor | Do diverse proposed interpretations improve the starting investigation? |
| Locust pilot's heterogeneous negotiation | Independently developed initial implementations, then separate participant contexts | Participants inspect signed contributions, critique, and one resolves | Does reciprocal evidence exchange improve on isolated search and one-way synthesis? |

The expected advantages and risks are hypotheses. Tag team may reuse accumulated
work cheaply and change a stuck approach, but the incoming model inherits prior
assumptions and a compressed history. Independent work may preserve alternatives
long enough to expose complementary errors, but duplicates effort and spends
budget on explanation, selection and reconciliation. Both can suffer from a
persuasive but incorrect shared interpretation. Neither topology guarantees
independent errors simply by using different model brands.

Locust itself is a collaboration substrate rather than this one experimental
topology. The pilot adapter fixed membership, isolation and exchange timing.
A tag-team policy could also be studied using Locust's signed artifacts, with
execution authority kept local. That would require an adapter; this assessment
does not claim an existing, qualified Merak4 tag-team integration.

## Follow-up design to preserve

1. **Calibrate separately, then freeze.** Find task families where strong solo
   models sometimes fail or differ in graded quality. Use a separate held-out
   scored set; do not keep modifying tasks in response to scored outcomes.
2. **Keep strong controls.** Give each solo model all starting information and
   tools available to the group. Retain isolated heterogeneous search, one-way
   synthesis and homogeneous negotiation controls. Select portfolio outputs by
   a declared observable rule, not hidden test answers.
3. **Add shared-state heterogeneous tag team.** This is the missing Merak4
   comparison. Test its value separately from independent-start negotiation.
   A same-model handoff with the same compaction and an observer-only/no-flip
   condition help separate model changes from context refresh and coaching.
   Stage these comparisons if the budget cannot support the entire matrix.
4. **Declare switching and communication rules.** Balance the starting/final
   model and task order. Predeclare timed or stuckness-triggered switching. A
   retrospective comparison of runs that happened to flip versus those that
   did not is confounded by difficulty and progress.
5. **Measure the treatment that actually arrived.** Record successful workers,
   delivered critiques, observed counterexamples, revisions, switches and
   compactions. Keep failures in the result accounting. Carry unused budget
   within a trial and budget enough for an actual exchange.
6. **Measure outcomes independently.** Report quality versus actual total
   spend and latency, charging observer, synthesis, compaction and tool costs.
   Retain wrong-to-right and right-to-wrong changes, not just final agreement.
   Use repeated task-level trials and uncertainty estimates appropriate to
   the number of tasks; hidden test cases are not independent replications.

A win for tag team would support serial reframing without establishing a need
for negotiation. A win for isolated search that synthesis merely preserves
would support selection rather than reciprocal exchange. Comparable gains from
homogeneous interaction or same-model compaction would weaken attribution to
heterogeneity. These outcomes are useful distinctions, subject to the precision
and task scope of the experiment.

## Reproduction and verification

The [read-only audit script](experiments/merak4_tag_team_audit.py) exports only
allowlisted metadata, hashes and model-turn/completion fields. From the Locust
checkout, with the historical sibling checkout and its saved jobs present:

```sh
python3 research/experiments/merak4_tag_team_audit.py \
  ../merak4 output/merak4-tag-team-audit.json
cmp research/evidence/merak4-tag-team-2026-10-07.json \
  output/merak4-tag-team-audit.json
```

The JSON record can be read without the sibling repository; reproducing the
archive inventory requires the original job files. Source hashes and original
log/result hashes identify exactly what was inspected. No raw prompts, private
environment values or model reasoning are copied into this record. This review
does not rerun Merak4, prove its current build, or establish a treatment effect.

Verification for this note: the audit regenerated byte for byte; all 943 source,
result and log hashes matched; exported fields and the cited trial details were
checked; Python compilation and the staged documentation/link check passed.
The shared working-tree documentation check reported an unrelated unstaged
removal of the `lan-sync-host-offline-2026-10-07.md` index entry. That change was
left untouched. No Go or Rust source changed, so their build/test suites were
not run for this assessment.
