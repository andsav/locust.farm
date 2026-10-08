# Model mixtures and communication: evidence and research direction

Status: research synthesis and proposed follow-up, 8 October 2026. This note
combines the supplied literature report with our retained experiments. It makes
no production change, launches no experiment, and does not replace the frozen
protocols or results linked below.

Decision: the [collaboration thesis](../docs/collaboration-thesis.md) records
the narrowed claim and the three-arm experiment adopted from this synthesis.

## Finding

**We have already experimented with model mixtures. We have not demonstrated a
reliable general advantage from mixing models or having them communicate.**

The useful hypothesis is conditional: another agent can improve a result when
it contributes a complementary discovery, skill, search trajectory or check that
the existing agent cannot obtain as effectively for the same resources. Model
names, persona differences, conversation volume and agreement do not establish
that contribution.

Our studies expose different limits: saturated coding tasks, small historical
forecasting cohorts, unequal actual costs, incomplete treatment delivery, and
question/annotation ambiguity. These are reasons to improve identification and
measurement, not evidence that every team fails or that one more mixture will
necessarily work.

## Distinguish the mechanisms

| Mechanism | What changes | Comparison needed to identify its contribution |
| --- | --- | --- |
| Independent search | More attempts without peer feedback | Strong solo with comparable resources, including self-review or multiple attempts |
| Model mixture | Which model generates each attempt | Same-model and mixed-model portfolios with the same aggregation procedure |
| Synthesis | A consolidator receives independent outputs | Selection/aggregation alternatives, charging the consolidator's work |
| Reciprocal exchange | Investigators revise after seeing peers | Private rechecking from the same initial outputs with comparable additional resources |
| Tag team | A shared task passes between models | Same-model handoffs and context refresh, with equivalent tools and continuation budgets |
| Observer advice | An implementer receives interventions during work | Self-review, same-model and different-model advisers; record actual delivered advice |
| Adaptive organization | Participants or a coordinator choose methods during work | A strong fixed method selected on development data, charging routing/search overhead |

These are different interventions. A passing tag-team run does not establish a
debate benefit; a mixed portfolio win does not establish that communication
helped; an experimenter-selected team does not establish autonomous organization.

## What we have already tested

| Study | Observed result | What remains unestablished |
| --- | --- | --- |
| [Astra/Fable negotiation pilot](negotiation-pilot-results-2026-10-07.md) | Solo, independent mixed attempts, synthesis, same-model negotiation and mixed-model negotiation were exercised. All 21 completed cells passed their hidden fixtures; all 36 retained initial solutions already passed. Different algorithms and reuse of one peer contribution were observed. | There was no hidden-case correctness gain to measure. Three task families, technical reruns, incomplete exchange delivery and unequal realized spending prevent a general efficacy claim. |
| [Luna/Sonnet forecasting](experiments/nous_models/results-2026-10-08.md) | 4,200 unique selected requests across 20 historical questions and five provisional clusters; pure teams versus a five-Luna/five-Sonnet mixture. The mixture did not consistently outperform both pure teams. | Historical knowledge/leakage, near-saturated additional questions, a formatting amendment, one mixed roster and unequal costs limit generalization. Request count is not the independent sample size. |
| [Merak4 tag-team assessment](merak4-tag-team-comparison-2026-10-07.md) | Retained logs show real executor switching, including passing runs. The inspected archive contains 113 logs with flips and multiple executor models. | No reliable matched solo comparison or populated per-trial costs establishes that switching caused those passes. Some nominal hypothesis-swarm completions had only one successful worker. |
| [Merak8 observer assessment](observer-implementer-lessons-2026-10-07.md) | Saved traces establish adviser shutdown and timeout behavior. One passing run records zero completed adviser steps. | A configured observer is not proof of delivered advice or a correctness benefit. Runtime completion and artifact correctness differ. |
| [Luna evidence-collaboration development](evidence-collaboration-development-results-2026-10-08.md) | At 48k caps, exchange succeeded on 5/40 families, divided work on 5/40, and direct solo on 8/40. Development resolved all 3,040 graph jobs. | This was a same-model study. The cost gate failed and a qualitative audit found annotation/task conflicts. No held-out main or repeat evaluation ran. |

### The forecasting result illustrates why the comparator matters

For the mixed Luna/Sonnet team, lower Brier is better. These are the retained
five-cluster-weighted historical results, not estimates from a prospective trial:

| Profile condition | Independent initial forecasts | Private rechecking | Peer exchange |
| --- | ---: | ---: | ---: |
| Neutral | 0.092600 | 0.093805 | 0.096804 |
| Published | 0.077417 | 0.098224 | 0.087897 |
| Structured | 0.089917 | 0.092920 | 0.090249 |

Exchange improved on private rechecking in two conditions, but was worse than
the initial independent aggregate in all three. It would therefore be misleading
to describe the first comparison alone as a net forecasting improvement. A
post-hoc invalid-response sensitivity also made apparent mixed-team advantages
over Sonnet very small. Mixed independent forecasts cost about 12.4 times Luna
and about 52% of Sonnet: equal member counts did not mean equal dollars.
See the [full results and sensitivity](experiments/nous_models/results-2026-10-08.md).

### The evidence study exposed a separate qualification problem

The Luna study's exact answer/support scoring rejected some plausible alternate
answers and citations. Other annotated-answerable questions required relations
the provided sources did not clearly establish, such as inferring birthplace
from a musician being described as a city's drummer. The seven-example audit
does not estimate a dataset-wide error rate or provide corrected scores.

Technical readiness at 48k passed, but no solo/independent baseline consumed at
least 90% of exchange's weighted cost, as that protocol required. All candidate
baseline weighted success rates were also below its 30% quality threshold.
The [recorded no-go](evidence-collaboration-development-results-2026-10-08.md)
remains unchanged. A successor should use quality–cost curves rather than reject
a strong baseline merely because it is cheaper; it should also offer that baseline
sensible additional work at larger allowances, rather than force wasted tokens.

## What the literature adds

These are source-reported findings, not additional Locust measurements. The
linked primary sources were consulted on 8 October 2026; this is a focused
synthesis, not a systematic review or replication.

- **Independent attempts are a serious control.** Agent Forest obtains gains
  through sampling and voting without debate. That supports testing search and
  aggregation before crediting communication. It does not establish superiority
  over every equally resourced single-agent strategy.
  [More Agents Is All You Need](https://arxiv.org/abs/2402.05120)
- **Debate is sensitive to the protocol.** Smit and colleagues find that debate
  does not reliably beat self-consistency and other alternatives by default;
  tuning agreement dynamics can improve it. Neither universal benefit nor
  universal futility follows.
  [Should We Be Going MAD?](https://proceedings.mlr.press/v235/smit24a.html)
- **Model variety can trade off against candidate quality.** Mixture-of-Agents
  demonstrates useful layered aggregation, but its comparison does not isolate
  an equal-cost benefit from heterogeneous models. Self-MoA subsequently shows
  that aggregating outputs from one strong model often beats mixing models.
  Useful complementarity must be measured, not inferred from provider diversity.
  [Mixture-of-Agents](https://arxiv.org/abs/2406.04692),
  [Self-MoA](https://arxiv.org/abs/2502.00674)
- **Expressed diversity is not useful error diversity.** The Nous paper reports
  that prompt-injected behavioral profiles did not measurably improve ensemble
  Brier score or reduce error correlation. Its limited evaluation and prompt
  intervention do not disprove the value of deeper heterogeneity. Our local
  Luna/Sonnet extension is a separate experiment with additional interventions.
  [Nous](https://arxiv.org/abs/2606.13038)
- **Resource and context accounting can explain apparent architectural gains.**
  Tran and Kiela find strong solo performance on text multi-hop reasoning under
  matched thinking-token caps. They explicitly exclude tools/vision and note
  approximate API accounting and unequal actual cap consumption. Their
  information-theoretic argument concerns ideal access to information; it is
  not a theorem that a bounded practical model always beats a practical team.
  [Equal-thinking-budget study](https://arxiv.org/html/2604.02460v2)

The resulting interpretation is that collaboration must create a useful change
in the work: discover a missing fact, explore an otherwise untried branch, expose
a counterexample, relieve a context bottleneck, or reduce elapsed time. Better
error independence can help aggregation, but independence alone is insufficient
if individual candidates become less accurate or the aggregator loses good work.

### Corrections to the supplied report

The report correctly emphasizes separating ensemble gains, communication gains
and resource allocation. Three qualifications should travel with that conclusion:

1. Its communication-study count includes Nous despite describing no inter-agent
   dialogue in the original profile-injection experiment. Persona conditioning
   is not communication between investigators. That chart should not be treated
   as evidence about the frequency of successful communication.
2. Its emphasis on heterogeneous models needs the Self-MoA counterexample. A
   mixture can dilute a strong model's candidate quality; different weights are
   neither necessary nor sufficient for useful diversity.
3. The ideal-predictor information argument is not a practical compute-bound
   impossibility result. Summaries can discard information while still making
   what remains easier for a limited reasoner to use. Retrieval and tools can
   also change what information is available.

Counts of positive papers with different tasks, controls and budgets are not a
meta-analysis. Likewise, a nonsignificant result in a small experiment is not
evidence of equivalence.

## Implications for Locust

These are research proposals, not newly enforced runtime rules. They fit the
existing [supported self-organization proposal](../docs/self-organizing-collaboration-plan.md):

- Support independent attempts, evidence exchange, reviews and handoffs as
  available methods. Let participants continue alone, decline work or change
  arrangements when those choices are within their authority.
- Preserve independently produced candidates before exchange. Keep checkable
  artifacts, sources, counterexamples and failed tests available for later use.
- Record whether advice was actually delivered and used, along with both repairs
  and regressions. A signed message or successful lifecycle is evidence of
  transport/execution, not of a better solution.
- Evaluate adaptive escalation against a strong fixed method. Routing on a
  failing verifier or conflicting evidence is a testable proposal; a model's
  confidence alone should not be assumed to identify when help will pay off.
- Keep acceptance and local execution authority separate from working roles.
  A useful coordinator or adviser gains no authority to bypass verification or
  another participant's local controls.

Parallelism, access to separately governed tools, and shared durable work may
have operational value even without an accuracy gain. That value needs its own
measurement; it is not evidence that a group reasons better.

## A follow-up that would resolve something new

**Proposed target:** difficult repository repairs with independently validated
regression tests, rather than another easy puzzle set or ambiguous exact-answer
benchmark. This is a candidate direction, not a ready execution protocol.

First validate the acceptance tests and task descriptions, then use development
tasks to establish unresolved errors and candidate complementarity. Freeze the
held-out issue families, graders, models, prompts, aggregation and resource
accounting before scored calls. Do not select held-out cases because a particular
model or mixture is known to win. Hidden acceptance evidence must remain hidden
from workers and aggregators alike.

Use a staged comparison rather than a large uncontrolled factorial study:

| Condition | Purpose |
| --- | --- |
| Strong solo, with self-review and a sensible range of allowances | Establish the quality–cost–time baseline; include the strongest affordable model |
| Independent same-model attempts plus a fixed selection/synthesis procedure | Measure additional search without model-family variation |
| Independent mixed-model attempts with the same procedure | Measure the incremental value of changing the model roster |
| Mixed-model private rechecking from frozen initial candidates | Control for additional work without receiving peer findings |
| Mixed-model exchange from those same initial candidates | Measure the effect of peer information relative to private rechecking |

All conditions should have the same available repository information, tools and
permissions. Charge selection, synthesis, routing, failures and retries. Shared
initial calls can be paid once experimentally but must be attributed to every
deployable method that needs them. With different models, dollars, tokens and
latency cannot all be assumed equivalent: choose the primary resource constraint
in advance and report the others separately.

The primary unit is an independent issue family, not a test assertion, API call
or model message. Select sample size from development discordance and a declared
practical effect, then report paired uncertainty and source/task clustering.
Evaluate final verified quality, actual cost and elapsed time; record technical
failure and early stopping separately.

Mechanism diagnostics should distinguish three opportunities:

1. **Complementarity:** one initial candidate passes while another fails. An
   oracle choosing the best initial candidate gives a diagnostic ceiling for
   selection, not a deployable baseline.
2. **Integration:** the actual selector/synthesizer preserves the successful
   candidate or loses it.
3. **Repair:** exchange produces a newly passing result, or damages a passing
   one, compared with private rechecking from the same starting artifacts.

Matched continuations from frozen failing states can test local repair, but
should accompany end-to-end trials rather than replace them: choosing failure
states changes the population being studied. Calibrate arrangements on development
data, and include their selection cost when making deployment claims.

A useful outcome might be that a narrow kind of second opinion pays for itself,
that independent parallel work is enough, or that solo remains preferable over
the tested range. Each is actionable. More dialogue, model names or role changes
alone would not resolve the remaining question.

## Provenance and scope

The user supplied *Collaboration, Coordination, and Competition Among AI Agents:
What Game Theory and Economics Say About Communication vs. Stronger Single-Agent
Reasoning*, filename `deep-research-report (3).md`, SHA-256
`fd64c1fc57ae7ac795a42566a9445e96481ba2e00e991361b91f35c8727189f1`.
It is a secondary synthesis; its embedded citation tokens are not durable source
links. The primary links above support the claims retained here. The input is
not copied into the repository, and this note does not depend on its local path.

Local empirical claims are drawn from the linked experiment reports and their
retained evidence. Their figures were reviewed for this synthesis; no model
experiments or evidence-reconstruction checks were rerun for this note. Existing
literature context is also indexed in the
[self-organizing-agents review](self-organizing-agents-literature-2026-10-07.md).
No general collaboration advantage, new accepted architectural decision or new
experiment result is claimed by this note.
