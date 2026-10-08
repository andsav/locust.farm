# Evidence collaboration: a study that can change what we build

Status: experiment design with an implemented runner. The
[implementation and pre-run amendments](experiments/evidence_collaboration/README.md)
specify the actual cohort, budget enforcement, and operational gates.
This is not yet a collaboration result or an accepted Locust runtime design.

## Question and decision

**Does exchanging evidence between agents improve a verifiable investigation
beyond what we get from one well-resourced agent or independent attempts?**

Start with evidence investigation. An agent market is a later intervention, not
something to build before establishing that the underlying information can be
usefully combined. The earlier [Luna pilot](luna-decision-pilot-results-2026-10-08.md)
was nearly saturated and tested supplied arithmetic. More runs of that experiment
would estimate an uninteresting difference more precisely.

A useful result can be positive or negative:

| Result | Consequence |
| --- | --- |
| Sharing raw evidence matches the team | Improve retrieval and context assembly first. |
| Independent attempts match discussion | Support parallel attempts and synthesis; discussion has not earned its cost. |
| Exchange helps fragmented workers but does not beat full-evidence solo | Coordination repairs fragmentation; do not claim greater collective intelligence. |
| Exchange beats the strongest baseline at competitive cost on held-out questions | Proceed to a real Locust integration trial and a second task domain. |
| Extra discussion causes correct answers to become wrong | Preserve independent judgments and evidence provenance; investigate conformity before adding more rounds. |
| Confidence improves but answers do not | Report calibration separately; do not call it a correctness improvement. |

These are conditional engineering decisions. No task here measures forecasting
returns, autonomous self-organization, or the reliability of Locust networking.

## Task choice and source audit

Use **MuSiQue-Full evidence investigations**, initially its three- and four-hop
questions. A case contains a question, roughly twenty source paragraphs, an answer
with aliases, and a supporting chain. Complete and insufficient-evidence versions
share a question ID. Agents must identify an answer supported by the supplied
corpus, or say that a necessary link is missing. The released data and evaluator
are available from the [authors' repository](https://github.com/StonyBrookNLP/musique).
The data is CC BY 4.0; retain attribution and license in redistributed evidence.

This is a bounded investigation over published text, not open-web research. It
provides objective labels and distractors without inventing another arithmetic
puzzle. It also has weaknesses: old public questions may be memorized, annotations
may miss alternative support, and multi-hop trivia is not a software investigation.
The authors explicitly warn about overlap with source QA datasets. A held-out split
prevents our tuning leakage; it cannot establish absence of model training leakage.

I also inspected [HiddenBench](https://proceedings.mlr.press/v306/li26ej.html),
including its [released tasks](https://github.com/Yassellee/HiddenBench_ICML)
at commit `3be6ca16973e4fb751ffc0dfb7eb11f2d28335d1`.
Its hidden-profile structure is a useful coordination diagnostic. It is unsuitable
as our sole correctness benchmark: several inspected answers require subjective
tradeoffs or exclusions stronger than their evidence, many scenarios are related,
and short discussion limits can themselves create information loss. For example,
task 13's theft conclusion relies on an exclusion about remaining in a building
that does not, by itself, rule out stealing an item inside it. This is our source
audit, not a measured model failure. Do not silently treat its 65 labels as 65
independent, indisputable investigation outcomes.

### Exact information boundary

Only send the question and paragraphs (`id`, `title`, `text`) to models. Never
send answer aliases, supporting flags, question decomposition, task names that
reveal the answer, or evaluator explanations. Every model request starts a fresh
conversation. Labels stay in the local evaluator, with no model file or network
tools. The harness is responsible for the boundary; prompt instructions alone
are insufficient. Randomize paragraph presentation identically across arms for
one case, retaining stable IDs. Record every delivered message and request hash.

For fragmented workers, distribute the paragraphs by a seeded permutation into
three disjoint shards. Use no supporting labels to construct the shards. Some
shards will contain several useful paragraphs and some none; report this after
scoring. The union is always exactly the solo corpus. The final synthesizer always
receives the full raw corpus in both fragmented arms, so a team cannot win merely
because the baseline was denied documents.

## Comparisons

Use the same model and reasoning setting throughout the first study. Luna remains
a suitable starting point only if calibration shows room to discriminate methods.
Do not introduce personality, model-family diversity, market incentives, adaptive
team sizes, or extra rounds in the first confirmatory comparison.

| Arm | Procedure | Maximum output tokens per case |
| --- | --- | --- |
| S: strong solo | On development data choose either one 12,000-token investigation, or draft, verify, and final answer with 4,000 tokens each. All calls see all documents. Freeze the better policy. | 12,000 |
| I: independent attempts | Three independent full-corpus investigators, 3,000 tokens each; one full-corpus synthesizer, 3,000. | 12,000 |
| D: divided work, no exchange | Three shard investigators, 1,500 each; each privately rechecks its own work, 1,500 each; one full-corpus synthesizer, 3,000. | 12,000 |
| E: evidence exchange | Reuse the exact three initial records from D. Each investigator receives all initial records and rechecks its own shard, 1,500 each; the same full-corpus synthesis policy gets the revised records, 3,000. | 12,000 |

The D/E branch is the clean mechanism comparison: same initial work, shard
assignment, calls, output caps, and final evidence access. Revision messages differ
only by access to peers' initial findings. One simultaneous exchange round prevents
order effects and an unbounded debate. No peer can see the other arm's revisions.
The synthesizer gets initial and revised records in both arms. Attribute the shared
initial calls' full cost to **each** arm when comparing deployable methods, even
though the experiment pays for them once.

All investigators return a concise structured record: candidate answer or
abstention, document IDs, evidence excerpts, missing links, conflicting claims, and
confidence. Revisions also identify which peer evidence changed their answer, if
any. Self-reports are process clues, not proof of causal influence. Use equivalent
schemas in S/I; do not cripple solo output to make collaboration look useful.

### Resource fairness

Equal output caps are **not equal realized compute or money**. These are initially
cap-matched configurations. Record input, output and reasoning-token usage,
provider charges or explicitly labeled price estimates, latency, failures, and
all synthesis/revision calls. Higher input consumption in E is part of its cost.
Do not turn unused tokens into an assertion that agents reasoned equally hard.

During development, measure each arm at 6,000 and 12,000 total output-token caps
(stage allocations scale proportionally). Select a solo configuration whose
average actual cost is within 10% of E, or more expensive, for the primary
competitive-cost claim. If neither qualifies, add an 18,000-cap solo configuration
**on development data only**. Do not choose a baseline after seeing held-out arm
outcomes. Freeze its selection rule: highest development primary score among
cost-qualified S/I configurations, ties favor lower cost then S.

Freeze the exact model ID, reasoning level, stage caps, price schedule, per-case
dollar ceilings, and complete-study dollar ceiling in a manifest before the main
run. Set case ceilings conservatively above development p99 usage; reserve every
call before dispatch. If the resulting held-out mean costs differ by more than
10%, report both performance and the discrepancy. A victory by a more expensive
team is not a cost-matched victory. Publish the cap/quality and actual-cost/quality
curves, and latency, even when the headline comparison is negative. A higher-tier
single model at comparable dollars is a subsequent deployment baseline, not an
unannounced substitution mid-experiment.

## Development, calibration and stopping

1. **Task calibration now:** 12 training question families, six three-hop and six
   four-hop. Run both evidence variants plus a no-document diagnostic: 36 calls.
   Freeze sample and prompt before calls; no treatment is selected on its wins.
   This establishes task plausibility, not effect size or statistical power.
2. **Method development:** 40 new complete question families. Compare the solo
   policies, stage budgets, schemas and all four arms. Inspect truncation, evidence
   delivery and genuine disagreement. Keep all explored configurations and costs.
3. **Freeze:** save exact case IDs, prompts, code hash, evaluator, analysis script,
   exclusions, budget, model and randomized dispatch order before evaluation.
4. **Confirmatory run:** target 500 new question families, both evidence variants,
   all four arms, one run per case/arm. Balance three- and four-hop families. Use
   the official development split if enough eligible disjoint families exist;
   otherwise reserve untouched training families in advance and label the split
   honestly. Never use unlabelled official test data as if it had known gold.
5. **Replication:** only after the frozen result, test a second task domain with
   fresh evidence and an independently checkable outcome. A software bug-fix study
   with hidden regression tests would be more directly useful to Locust, but needs
   its own task audit and contamination controls.

Keep both variants of a question together. Exclude any main family sharing a
single-hop decomposition ID with calibration or development. Within the main pool,
use a seeded greedy selection that also permits no reused single-hop decomposition
ID across selected families; freeze the seed and resulting IDs. Report source-title
and answer overlap rather than claiming complete topic independence. Audit
near-duplicate questions before outputs. If fewer than 500 eligible families
remain, state the attainable sample and detectable effect **before** dispatch,
or acquire another dataset. Never weaken exclusions after viewing arm results.

Development must show at least 95% valid completed outputs, no label leakage, and
no systematic per-stage truncation. Prefer a strong solo group-success range of
30–85%; outside it, the benchmark is probably too difficult or too easy for this
comparison. Fewer than 10% initially discordant candidate answers makes the
revision mechanism poorly exercised. Increase stage caps or change task/model
choice during development if needed, record why, and freeze a fresh evaluation.
Do not manufacture diversity by instructing agents to be wrong.

No-document abstention and complete/missing-evidence pairs test evidence discipline.
They cannot prove the model never memorized a question. Review suspicious cases
where an answer is right but supplied support is absent. If annotations are
ambiguous, pre-adjudicate before outputs or retain them and report a blinded
sensitivity analysis; never delete only team losses. Technical failures count as
failures in the primary deployment score. No automatic paid retries; report a
separate completed-case diagnostic without replacing the primary score.

## Outcomes and analysis

The independent unit is the **question family**, not each agent, API call, variant,
or repeat. The primary family success is 1 only when the complete variant has the
correct normalized answer and exact annotated support set, and the incomplete
variant correctly abstains. This stringent score explicitly means agreement with
the annotation, not that every alternative citation is invalid.

Secondary outcomes: answer exact match and token F1, answerability accuracy,
support F1, group success without exact-support matching, unsupported confident
answers, confidence Brier score against response correctness, initially correct
answers damaged by revision, initially wrong answers repaired, and dollars per
successful family. Report raw counts and denominators, not just percentages.
Blind-review alternative-support cases without arm names; keep the frozen primary
score and separately report adjudicated sensitivity results.

The sole primary hypothesis is E minus the development-selected strongest S/I
baseline on family success. Report paired difference and a two-sided 95% family
bootstrap interval (10,000 resamples, frozen seed), plus exact paired McNemar test.
For E−D and E−I mechanism comparisons, report intervals and Holm-adjusted tests as
secondary. A second seed on a prespecified 50-family subset measures run variability;
it does not add 50 independent task families. Account for those calls separately.

Target a practically interesting gain of **8 percentage points**. With paired
binary outcomes, the approximate sample requirement is
`N = (1.96 + 0.84)^2 * discordance / 0.08^2`.
At 30% discordance this is about 368 families; at 40% it is about 490. Thus 500 is a
reasonable target for the one primary comparison, not a universal power guarantee.
Estimate discordance in development, lock sample size before the main run, and
publish the calculation. Repeated wording or shared source families can reduce
effective information; report a source-overlap sensitivity analysis as well.

Call evidence promising if the primary interval excludes zero, its point estimate
is at least +8 points, and cost qualification holds. This does not establish that
the true gain is at least 8 points. If the interval's upper end is below +8, rule
out that practical gain for this tested setting. If the interval spans zero and
+8, call it inconclusive. No optional stopping when significance first appears;
finish the frozen sample unless budget, provider, or validity failures stop it.
Publish every planned case's status and the reason for any stop.

With the three-call solo policy, the 500-family main run requires 18,000 new API
calls: 1,000 evidence variants times 18 calls across S/I/D/E after sharing D/E
initial work. The prespecified repeat adds 1,800 calls. Development adds its own
calls and must be included in the research cost, though excluded from per-case
production comparisons. Plan for a $250 study ceiling initially, then replace it
with an explicit manifest ceiling using measured development usage before the
main run. This is a planning envelope, not a provider price quote or permission
to run an unfrozen study. Cost is not the reason to skip a necessary control.

## Market extension, only after the investigation baseline

A later market study should hold initial private evidence and messages fixed and
compare aggregation rules: probability mean, proper-score-weighted aggregation
(weights fitted only on development data), and an explicitly specified trading
mechanism. Include a no-new-information trading condition. Otherwise market gains
can merely reflect extra evidence exchange or more inference calls. Evaluate
proper scoring rules and final decision utility under a frozen payoff function;
volume, consensus, and profitable simulated trades are not external truth.
Do not infer outcomes from telling an LLM it has a monetary incentive. This is a
separate preregistration, not an extra arm added after inspecting the first result.

## Execution boundary

The initial study uses a direct API harness to isolate the method. It establishes
nothing about actual Locust delivery, authorization, multi-machine operation, or
heterogeneous harnesses. A later Locust replication should replay the frozen
protocol through real participants and compare message delivery, outcomes, costs,
and failures. Keep this method outside daemon policy, consistent with the
[self-organizing collaboration plan](../docs/self-organizing-collaboration-plan.md).

The calibration runner is in
[experiments/evidence_calibration](experiments/evidence_calibration/README.md).
The completed calibration made 60 attempts (59 valid). The revised configuration
answered 6/12 complete cases correctly, with exact support on 4/12, but achieved
only 3/12 strict paired successes after one timeout. This supports further method
development, **not immediate launch of the 500-family run**. See the runner's
[results table and evidence](experiments/evidence_calibration/README.md#recorded-results)
for the initial failure, the changed prompt/settings, and accounting limits. The [four-method runner](experiments/evidence_collaboration/README.md) is now
implemented; its pre-run amendments supersede the proposed balanced cohort and
spell out the enforced gates. Live results are recorded separately after execution.
