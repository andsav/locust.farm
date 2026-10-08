# Agent crowds, simulations, and decision markets

Date: 2026-10-07. Status: research synthesis and proposed experiments, not an
accepted architecture or a demonstrated product capability.

## Judgment

There is a useful unifying product here: a workspace that turns context into
explicit alternatives, investigates their consequences, helps a person choose,
and preserves what actually happened. In this proposed product, DreamColor would
supply context and reusable workflows; Imago would represent assumptions and
possible consequences; Locust would coordinate investigation and execution.
A market is one candidate method for
combining judgments within that loop.

The papers justify testing that method. They do not establish that simulated
crowds reliably discover optimal decisions. The most useful immediate investment
is an experiment that can distinguish additional information, different methods,
and different models from simply spending more inference on the same problem.

Four problems should remain separate:

| Problem | What success means | What would not establish it |
| --- | --- | --- |
| Simulating people | Predicting held-out behavior of the target people | Interesting personas or internally consistent dialogue |
| Aggregating forecasts | Better predictions of resolved events at a stated cost | More disagreement, more agents, or a stable price |
| Choosing actions | Better outcomes under the person's values and constraints | Accurate predictions only for actions already selected |
| Allocating work | Better verified results per unit of time or money | Agents bidding confidently on their own competence |

My recommendation is to pursue forecast aggregation and decision evaluation
first. Keep human simulation as a distinct research track. Do not make synthetic
traders, permanent personality roles, reputation-weighted voting, or a market
engine prerequisites for the broader product.

## Sources and reading boundary

This review read the core arguments, methods, results, and limitations of the
five main papers below. It additionally inspected selected appendices on Nous
controls and power, and Park's normalization, ablations, and retrieval/inference
analysis. It did not audit every supplementary table in the 80-page aggregation
paper or the 86-page simulation paper. Key Nous, Silicon Crowd, and decision
market tables/equations were checked in rendered PDF pages.

The only computation independently rerun here is the arithmetic audit of frozen
Nous forecasts below. There were no paid model calls, new agent experiments,
human studies, or reproductions of paper training/extraction pipelines.

| Source | Version inspected | Most useful locations |
| --- | --- | --- |
| [Nous: An Attempt to Extract and Inject Cognition Behind Prediction-Market Behavior](https://arxiv.org/abs/2606.13038v1) | arXiv v1, June 2026; text describes its revised study as V2 | Sections 3-7; Tables 2 and 6; artifact reproduction guide |
| [Information Aggregation with AI Agents](https://arxiv.org/abs/2604.20050v4) | arXiv v4, September 2026 | Experimental design; results; Sections 6.5-6.6 |
| [Decision Markets with Good Incentives](https://yiling.seas.harvard.edu/files/2025/01/dm_full_version.pdf) | Chen, Kash, Ruberry, Shnayder; WINE 2011 full version | Sections 3-6; Theorems 1-3 |
| [Wisdom of the Silicon Crowd](https://eprints.lse.ac.uk/125626/1/sciadv.adp1528.pdf) | Published Science Advances version, November 2024 | Studies 1-2; Table 2; limitations |
| [LLM Agents Grounded in Self-Reports Enable General-Purpose Simulation of Individuals](https://arxiv.org/abs/2411.10109v3) | Park et al., v3, June 2026; earlier title was Generative Agent Simulations of 1,000 People | Main results; supplementary methods, robustness, and mechanisms |

Supporting readings: [Chen and Kash, Information Elicitation for Decision
Making](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/decisionrules.pdf),
[Hanson's futarchy proposal](https://mason.gmu.edu/~rhanson/futarchy.html), and
[ForecastBench's updated methodology](https://www.forecastbench.org/assets/pdfs/forecastbench_updated_methodology.pdf).
Futarchy is a conceptual governance proposal here, not empirical validation.

Downloaded PDF fingerprints and the repository revisions inspected are preserved
in the [source manifest](evidence/decision-markets-sources-2026-10-07.json).
Third-party PDFs, raw wallet data, and upstream source code are not vendored.

## Nous: the most useful research decomposition

### What the paper establishes and leaves open

Nous separates recovering behavioral profiles, injecting them into models, and
improving collective predictions. From 100 selected Polymarket wallets it derives
14 parameters across eight dimensions. Nine parameters meet its initial
reliability threshold; eight survive its stronger position-aware criterion.
That supports repeatability of some trading proxies, not direct measurement of
cognition. Apparent out-of-sample profit associations do not survive its
confound controls.

The injectability experiment is small and its measurement depends on the model:
embedding saturation and parsing failures complicate interpretation. Forecasting
uses ten agents per group, with 28 of 50 questions complete across four groups.
Against a length-matched placebo, profiles slightly increase forecast dispersion
but show no Brier improvement. The paper's power analysis cannot exclude modest
benefits. Its temperature and synthetic-profile tests do not rescue the result.
Close prompt embeddings suggest a transmission problem but do not identify its
cause. See Sections 4-7 and Table 6 of [Nous](https://arxiv.org/abs/2606.13038v1).

The released [reproduction guide](https://github.com/WillChienT/nous-paper/blob/3ff0b996784cdc8f4db40210c0505371e667ee1e/artifacts/REPRODUCE.md)
distinguishes frozen-output analysis from regeneration. Raw wallet extraction and
the structure-to-narrative translator are outside the release. The repository's
CC BY-NC-ND license also warrants checking before any reuse beyond analysis.

### Independent check of the released forecasts

An original, standard-library [audit script](experiments/audit_nous_frozen_outputs.py)
reads the released probabilities directly. It checks question/agent identities,
duplicates, probability validity, and shared complete cases; computes arithmetic
mean ensemble Brier scores and pairwise Jensen-Shannon divergence; and adds a
question-paired bootstrap. [Machine-readable results](evidence/nous-independent-audit-2026-10-07.json)
contain input hashes, question IDs, failures, and interval settings.

| Group | Successfully parsed records / 500 | Ensemble Brier on the same 28 questions | Mean pairwise JSD, bits |
| --- | ---: | ---: | ---: |
| Repeated-seed control | 490 | 0.093269 | 0.000000 |
| Different-seed control | 474 | 0.085690 | 0.002550 |
| Length-matched placebo | 465 | 0.091135 | 0.002481 |
| Nous profiles | 485 | 0.088844 | 0.006017 |

Lower Brier is better. More JSD means different forecasts, without saying which
are better. Profile minus placebo Brier is **-0.002291**, with an exploratory
paired 95% bootstrap interval of **[-0.039660, +0.045415]**. Against different-seed
control it is **+0.003153**, interval **[-0.004736, +0.011554]**. These arithmetic
results agree with the direction and magnitude of Table 6. They do not reproduce
its Welch-test procedure or establish a beneficial effect.

The paired interval resamples questions, not underlying event clusters. Related
markets can make it too optimistic. Complete-case selection also omits 22
questions; it does not estimate end-to-end reliability over all attempted
questions. The repeated-seed baseline is useful for diagnosing stochasticity,
but an unusually weak baseline for a practical ensemble.

For errors `e_i = p_i - y`, the following identity holds for an equally weighted
ensemble of `n` forecasts:

```text
ensemble squared error
  = mean individual squared error / n
    + (n - 1) / n * mean pairwise error product
```

The audit verifies this to floating-point precision. The pairwise product is not
centered covariance. Reporting it alongside ensemble Brier supplies a
decomposition, not independent corroboration. Likewise, high raw error
correlations can reflect shared outcomes and difficulty; they do not by
themselves identify a common failure mechanism or a causal effective crowd size.

### What I would extract for Locust

Treat a profile as an intervention that must pass three separate checks:

1. **Measurement:** does a trace predict behavior on a later task, beyond task
   selection, budget, tool access, and chance?
2. **Transmission:** does assigning the profile change a specified behavior in
   the intended direction?
3. **Usefulness:** does that change improve the result at comparable cost?

For example, a proposed base-rate specialist should actually retrieve reference
classes and change appropriately when a base rate changes. A counterexample
specialist should discover more valid failure cases, not just disagree more.
The other agents should then demonstrably benefit from those findings.

This suggests procedural diversity before elaborate biographies: independent
evidence collection, explicit calculations, counterexample search, and causal
assumption checking. These are hypotheses worth testing, not guaranteed fixes.
Start with literal parameter and procedure interventions before concluding that
fine-tuning is required. Embedding distance is a poor substitute for behavioral
compliance, especially when a small textual change can reverse a decision.

Human fidelity and useful forecasting may also conflict. A faithful simulation
of a biased trader could be excellent social simulation and a worse member of a
forecasting ensemble. We should select the objective before selecting the traits.

## Information Aggregation: make the information structure explicit

### Evidence

Galanis and coauthors study 4,076 controlled markets across experiment blocks.
Three traders receive private signals; their combined signals identify the
payoff. Trading uses an LMSR with known priors and tools that expose price
effects. Markets run for 3, 6, or 9 total turns, not that many turns per trader.
Results vary by model and information structure, with some confident errors and
persistent difficulty in the hardest setting. The limited mixed-model teams do
not isolate a causal heterogeneity benefit. Later qualitative disclosure runs
are not a clean randomized test of persistent learning. See the design, results,
and extensions in [the paper](https://arxiv.org/abs/2604.20050v4).

This is useful evidence that price-based interaction can aggregate deliberately
distributed information under some conditions. It is not evidence that trading
invented missing information, that a converged price is correct, or that telling
an LLM to maximize profit makes it behave like the rational trader assumed in an
economic model. A truthful pooling baseline can recover the answer directly in
this experiment; that baseline matters when transplanting the task to
cooperative agents.

### Research implication

The right Locust comparison is not market versus one uninformed agent. Give the
strong solo baseline the union of available evidence. Compare market prices
with plain pooling, independent averaging, and structured criticism under the
same resource budget. If a market wins only against agents deprived of evidence,
we learned that evidence access matters.

Separate three mechanisms: acquiring evidence, deciding what it implies, and
transmitting that judgment. Price compresses information; compression might help
coordination or discard the qualification needed to understand a claim. Compare
price alone with price plus explicit evidence and assumptions. Retain initial
forecasts so that convergence toward an error is visible.

## Decision Markets: incentives and counterfactuals are central

### Evidence

Chen, Kash, Ruberry, and Shnayder construct strictly proper decision markets for
full-support decision rules: every action has positive selection probability.
Their construction scales the selected action's proper score by inverse
selection probability. The expected contribution then cancels that selection
probability. For multiple reports, the corresponding score improvement is paid
using the final decision distribution. Their necessity result rules out strict
propriety without full support within the model. See Sections 3-6 and
[Theorems 1-3](https://yiling.seas.harvard.edu/files/2025/01/dm_full_version.pdf).

The guarantee concerns myopic, risk-neutral score maximization, not arbitrary
long-term strategy, outside preferences over actions, or demonstrated LLM
behavior. Very small action probabilities create large possible payments.
Choosing the apparent best action deterministically and scoring only it does
not inherit the theorem. A human override also changes the effective decision
rule unless accounted for.

### Research implication

Suppose the workspace considers shipping a feature, testing a prototype, or
doing nothing. After shipping, it observes that outcome; it never observes what
the same world would have done under both rejected actions. Forecast accuracy
on the chosen action cannot validate the entire ranking.

We need to distinguish `P(outcome | action chosen)` from a causal prediction
under an intervention. Selection may encode information omitted from the model.
Imago's causal assumptions can make that distinction explicit, but drawing a
graph does not identify the causal effect.

Begin in a simulator with known outcomes or generative mechanisms for every
action. Only later evaluate real decisions with prospectively recorded options,
selection policy, propensities where known, overrides, and outcome rules. Never
invent propensity values after the fact. Unobserved counterfactuals should remain
unobserved. We can support human choice without claiming an incentive theorem;
real-world randomization requires its own suitable setting and authorization.

The person's values are also inputs. A market can estimate specified
consequences; it cannot discover which tradeoff the person ought to prefer.

## Wisdom of the Silicon Crowd: preserve the simple baselines

### Evidence

Schoenegger and colleagues compare forecasts from 12 LLMs with 925 humans over 31
binary questions. Study 1 is prospective. The aggregate Brier score is about
0.20 versus 0.19 for the human crowd; Table 2 reports GPT-4 at 0.15. Thus the
ensemble does **not** outperform every constituent model. The equivalence
analysis uses a relatively wide margin. Access to information and forecast
timing need care in interpreting comparisons. In exploratory analyses, directly
combining human and AI forecasts performs better than asking the AI to revise
using the human aggregate. See [the published paper](https://eprints.lse.ac.uk/125626/1/sciadv.adp1528.pdf).

This corrects the stronger characterization of this study in Nous's introduction
and in the earlier conversation. It supports the feasibility of useful AI
forecast aggregates on these questions, not universal ensemble superiority.

### Research implication

Always retain a simple mean or median, a strong solo forecaster, and repeated
samples from that same strong model. Compare expensive discussion with spending
the same budget on retrieval or additional independent attempts. If we select
weights or models using calibration data, freeze that choice before evaluation.
Choosing the best member after seeing test answers is an oracle comparison,
not a deployable selection procedure.

Prospective collection is especially valuable: freeze the evidence cutoff and
first forecast before resolution, then preserve later revisions. Historical
questions answered by later models need an explicit contamination boundary.

## Grounded human simulations: context is valuable, transport is unproved

### Evidence

Park and colleagues construct agents for 1,052 US participants using interviews,
surveys, or simpler demographic/persona descriptions. Interview-based agents
achieve 65.67% raw GSS accuracy, about 0.83 relative to human test-retest
consistency. That is not 83% raw accuracy. Richer self-report context outperforms
the simpler descriptions on survey outcomes; the five economic games do not
show a significant difference between agent specifications. Supplementary
ablations and retrieval/inference analyses indicate that access to relevant
personal information contributes substantially. Agreement on effects across
five experimental studies is not 1,052 independent validations of experimental
effects. See the revised [main paper and supplement](https://arxiv.org/abs/2411.10109v3).

These results do not validate a synthetic population's social network dynamics,
joint error structure, novel product adoption, or behavior after an unfamiliar
intervention. The study's population and targets bound the claim.

### Research implication

For DreamColor, actual decisions, constraints, and domain context are a more
defensible starting point than invented personality detail. For customer
research, first ask whether a grounded agent predicts a held-out behavior of a
real target user better than a simple history-based baseline. Keep person-level
and time-level holdouts; avoid answering with an outcome already in the context.

Do not combine thousands of synthetic respondents and treat their count as
thousands of independent observations. Evaluate population totals, subgroup
errors, and joint behavior against real observations. Simulations can generate
hypotheses and stress cases before they earn the right to estimate demand.

## The broader product: a decision record that survives execution

The proposed loop is:

```text
Context and constraints
  -> alternatives, assumptions, measurable consequences
  -> independent investigations and initial forecasts
  -> evidence exchange, revisions, unresolved disagreement
  -> human decision and authorized workflow
  -> observed outcomes, forecast scoring, updated context
```

The practical unit should be a decision record, not an agent conversation.
For example: “Should we build integration A, integration B, or first interview
five users?” Record the objective, costs, time horizon, adoption hypothesis,
dependencies, and the observation that would change the ranking. Agents can
suggest the interview workflow when one uncertain assumption dominates the
choice. That is a concrete connection between context-driven idea generation
and evidence-driven execution.

Keep three distinct outputs visible: probability estimates, the person's value
tradeoffs, and the recommended next action. Include “gather evidence” among the
actions. Expected value of information is itself a model-based estimate; expose
its assumptions rather than presenting it as a certain benefit.

Initial research records can be ordinary immutable JSON artifacts alongside
Locust work. Suggested fields, not a proposed daemon API:

| Record | Minimum useful information |
| --- | --- |
| Question | Outcome definition, deadline, resolver, event-family ID, evidence cutoff |
| Forecast | Probability, model/prompt version, source IDs, tools allowed, cost, timestamp |
| Revision | Prior forecast ID, newly seen evidence or peer claim, updated probability |
| Decision | Alternatives, values/constraints, policy, selected action, authorization, override |
| Outcome | Observation, source/time, resolution status, missingness, disputed interpretation |

This makes corrections and failed assumptions useful to later work without
quietly converting conversation consensus into truth. It also lets the user
inspect whether several agents independently found evidence or copied one
source through several intermediaries.

### Fit with the current repositories

Locust's [sharing contract](../docs/guide/sharing.md) says every member reads the
shared goal and its history. Task or role labels do not isolate information.
The [goal access check](../crates/locust-core/src/node/access.rs) uses membership
and read epochs, while [content readability](../crates/locust-core/src/node/content_graph.rs)
checks goal references and epoch ceilings, not task or role privacy.
An independence experiment must freeze first forecasts outside that shared
board and control worker inputs, or use separate goals with suitable access
controls. Separate goals alone do not stop a local shell-capable worker from
reading accessible files. A prompt convention is not an enforced blind.

Coordination and result acceptance also do not establish factual correctness.
Keep the verification boundary described in the existing
[self-organizing collaboration plan](../docs/self-organizing-collaboration-plan.md).
Market rankings must not expand another participant's local execution authority.

The cross-repository inspection used Catalyst's
`src/lib/domain/causal-schema.ts` and Merak10's
`docs/DREAMCOLOR_KNOWLEDGE_IMPLEMENTATION_2026-10-01.md` plus its associated plan.
Their pinned revisions are in the source manifest. Imago has causal events,
mechanisms, evidence, and interventions; the inspected schema is not a complete
forecast provenance/outcome ledger. DreamColor's context/workflow direction is
relevant, while the unified Locust decision loop remains a proposal. This review
does not qualify either repository's runtime or change its implementation.

## Prioritized experiments

These are proposals. Budgets below are experimental design sizes, not authorized
spending. Set a monetary ceiling using the selected models before any live run.

### 1. Test whether an intended difference survives into behavior

Use 20 small diagnostic scenarios where a prescribed change has a known
direction: updating after stronger evidence, respecting a changed base rate,
reducing exposure under a stated risk limit, or seeking a counterexample.
Compare four conditions: default prompt, length-matched filler, narrative
profile, and explicit procedure/parameter. Use three repeat seeds per condition:
240 responses for one model, before any cross-model extension.

Score directional compliance, task accuracy, invalid outputs, and cost. Have
the expected behavior and rubrics written before generation; test the scoring
logic with scripted fixtures first. Prompt length matching alone does not make
filler semantically inert. If a treatment cannot reliably alter the intended
behavior, do not spend a larger forecasting budget trying to explain its null.

This is the cheapest useful follow-up to Nous. It tests a potential bottleneck
without attempting to rebuild an unavailable translator or pretending that
wallet behavior measures general cognitive skill.

### 2. Locate the source of any crowd benefit

Start with a 30-question feasibility pilot spanning unresolved difficulty, with
three-agent teams. Thirty questions estimate variance and failure rates; they
are not an efficacy target. Avoid the ceiling in Locust's previous
[negotiation pilot](negotiation-pilot-results-2026-10-07.md), where all 36 retained
initial solutions passed their hidden fixtures.

Use staged comparisons, avoiding an expensive full factorial initially:

| Comparison | Keep fixed | Question answered |
| --- | --- | --- |
| Strong solo vs three samples of the same model | Evidence union, total resource envelope | Does repeated sampling help? |
| Same-model team vs mixed-model team | Evidence, agent count, aggregation, budget | Does model choice add useful diversity? |
| Generic tasks vs distinct procedures | Models, evidence, budget | Do methods add useful diversity? |
| Shared evidence vs partitioned evidence | Source pool, task, models | What does distributing evidence change? |

The solo baseline gets the union of evidence, and a separate cooperative pooling
baseline makes that union explicit. Record per-agent inputs to distinguish a
useful discovery from duplicated or unavailable information. Model and prompt
selection belong in calibration; freeze them for a held-out evaluation grouped
by event or task family.

Primary outcome: paired change in aggregate Brier on common questions, with
event-family clustering when applicable. Secondary outcomes: individual Brier,
calibration, confident mistakes, parse/timeout failures, cost, latency, and
descriptive joint errors. Predeclare a failure/retry policy and report all
attempts; do not quietly drop the questions that make a method look unreliable.

Use pilot variance and a smallest worthwhile effect to size the later study.
No fixed sample count can establish adequate power without those assumptions.
Estimate gains with intervals and costs; avoid selecting a winner across many
exploratory metrics and reporting only its best comparison.

### 3. Test whether interaction improves the same initial forecasts

Freeze each team's first forecasts, then branch into simple aggregation,
structured evidence exchange, and a paper-style LMSR experiment. Use price-only
and price-plus-evidence variants only if the first comparison merits expansion.
Start with synthetic signal tasks, where all evidence can be pooled and the
correct posterior can be computed independently.

Rotate trader order; preserve identical starting evidence and forecasts across
branches. Count reasoning, tool calls, and aggregation against the budget. Add
a resource-matched independent arm so extra interaction time does not receive
free compute. Report helpful and harmful revisions separately, including cases
where discussion moves a correct answer toward the wrong one.

A market earns further investigation if it improves accuracy or resource use
against these simpler methods on held-out tasks. Price stability and lively
trading are diagnostic observations, not success criteria.

### 4. Evaluate decisions where counterfactuals can be checked

Build a small offline decision simulator with three actions, costs, a latent
state, and a known causal outcome mechanism. Include a costly information-gathering
action. Compare strong solo, simple forecast aggregation, and the best surviving
interaction method. Evaluate expected utility/regret using the known mechanism,
as well as forecasts. Vary misspecified priors, common biased evidence, and
near-tied actions.

Keep the selection policy separate from the forecasters. If testing the decision
market theorem, implement its full-support and scoring assumptions explicitly
and test strategic counterexamples; merely calling the simulation a market is
insufficient. Include an agent optimizing a conflicting external objective to
demonstrate where the guarantee stops applying.

Then consider a prospective, low-stakes real decision series with independent
outcome adjudication. The simulator establishes behavior in its model, not real
causal validity. Do not train and evaluate on the same simulated mechanism alone.

### 5. Investigate grounded user simulation separately

Choose one actual target behavior, collect appropriate consented context, and
predict a future or withheld response. Compare simple history-based prediction,
demographics/personas, retrieved relevant context, and interview-grounded agents.
Use real held-out observations as the target, not another model's judgment of
realism. Test transfer to a new situation before using simulations to recommend
a product investment. This is a separate validation effort, not a cheap source
of artificial customers for the market experiments.

## Questions worth digging into next

| Open question | Useful next evidence | Decision it could change |
| --- | --- | --- |
| Is the Nous bottleneck extraction, translation, or behavioral control? | Public extractor/translator release or a clean-room directional-compliance experiment | Whether profiles deserve a larger trial |
| Can disagreement identify valuable missing evidence? | Prospective test of evidence acquisition chosen from disagreement versus a fixed retrieval budget | Whether to recommend investigations automatically |
| Does a market add anything beyond explicit pooling? | Paired market/pooling/strong-solo comparisons on the same initial information | Whether to build a market UI or engine |
| Which errors are shared across agents? | Held-out joint failures, source overlap, and domain-stratified residual analysis | Whether team selection should use measured complementarity |
| Can we compare agents answering different question sets? | Overlapping anchor questions and difficulty-aware evaluation | Whether reputation weights are defensible |
| Do forecasts improve actual choices? | Known-counterfactual simulation followed by prospective real outcomes | Whether the product can claim decision improvement |

For the reputation question, [ForecastBench's methodology](https://www.forecastbench.org/assets/pdfs/forecastbench_updated_methodology.pdf)
is useful reading on difficulty adjustment and overlap. Start with paired
questions before importing a complex ranking system. For retrieval and forecast
pipeline design, [Approaching Human-Level Forecasting with Language Models](https://arxiv.org/abs/2402.18563)
is a next full-text reading, not a study independently assessed in this review.

The immediate deliverable should be a small, reproducible research harness and
a legible decision record. Promote a mechanism into product architecture only
after it shows a useful effect against simple, well-resourced alternatives.

## Reproducing this review's computation

Clone [the upstream artifact repository](https://github.com/WillChienT/nous-paper)
into a disposable directory and select commit
`3ff0b996784cdc8f4db40210c0505371e667ee1e`. Then run from the Locust root:

```sh
python3 research/experiments/audit_nous_frozen_outputs.py /path/to/nous-paper
python3 -m unittest discover -s research/experiments -p 'test_audit_nous_frozen_outputs.py'
```

The audit is offline, reads only upstream files, and writes JSON to standard
output. It neither runs upstream code nor regenerates model responses. The
saved result records exactly which inputs were read. This is a narrow
reproducibility check, not validation of all Nous claims.
