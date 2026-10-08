# Collaboration thesis and the next experiment

Status: decision recorded 8 October 2026 from the research linked below. The
narrowed thesis is accepted as the claim Locust tests. The experiment direction
is a proposal: it is not a frozen protocol and authorizes no spending. Nothing
in this document changes runtime behavior.

## The thesis Locust tests

The earlier working hypothesis, recorded in the
[supported self-organization plan](self-organizing-collaboration-plan.md), was
that different models make different useful observations and that agents can
exploit those differences by choosing when to advise, challenge, divide work,
consolidate or stop. Communication between agents was the mechanism under test.

That hypothesis is replaced by a narrower one:

> On hard problems whose results can be checked independently, several
> independent attempts with a trustworthy check find results that one attempt
> usually misses. A shared record of what was tried, including what failed,
> lets later attempts do better than isolated attempts for the same spend.

The first sentence is supported by the published evidence reviewed in
[groups of agents versus one](../research/swarm-evidence.md). The second is the
claim specific to Locust's design, and nothing has measured it yet.

What the narrowing drops as the primary mechanism: reciprocal exchange,
negotiation and debate between agents; mixing model families as the source of
useful diversity; personas and behavioral profiles. These may still help in
particular cases. They are not what Locust claims.

What it keeps, and where the product already enforces or asks for it:

| Condition | Where it lives today |
| --- | --- |
| Independent attempts before exposure to peers' results | The installed skill asks agents to post a result before reading other members' results on the same task ([skill](../skills/locust/SKILL.md)); this is a convention, not enforced by the daemon |
| Results count by check or review, not by headcount or agreement | Completion rules per formation; review requests follow the goal's rule ([flow](../crates/locust-core/src/goal/flow.rs)) |
| Review by a non-author | The default `peer-review` preset requires another member's approval ([formations](guide/formations.md)) |
| A shared, attributable record including failures | Findings, contributions, signed `--source` declarations and `ctx:` acknowledgments ([sharing](guide/sharing.md), [runtime reference](guide/runtime-reference.md)) |
| Local execution authority separate from working roles | Levels and task allowances per agent ([concepts](guide/concepts.md)) |

## Why: what five experiments found

Each experiment is retained with its evidence. None shows a collaboration
advantage; each failed for a different reason, and the pattern across them is
consistent.

| Experiment | Result | What blocked a conclusion |
| --- | --- | --- |
| [Astra/Fable negotiation pilot](../research/negotiation-pilot-results-2026-10-07.md) | All 36 initial solutions already passed every hidden case; nothing to correct | Ceiling: tasks too easy; some critique phases delivered no model response |
| [Luna/Sonnet forecasting](../research/experiments/nous_models/results-2026-10-08.md) | Mixed team did not beat both pure teams; exchange was worse than independent aggregation in all three profile conditions; mixture cost 12.4 times Luna | Historical questions, five clusters, unequal spend |
| [Luna evidence collaboration](../research/evidence-collaboration-development-results-2026-10-08.md) | Direct solo at 48k was the best (26.7%) and cheapest configuration; exchange had one win and four losses against it at 6.56 times the cost | Cost gate failed; annotations conflicted with the instructions; same model only |
| [Luna decision pilot](../research/luna-decision-pilot-results-2026-10-08.md) | All conditions near-perfect | Ceiling |
| [Merak4 tag team](../research/merak4-tag-team-comparison-2026-10-07.md) and [Merak8 observer](../research/observer-implementer-lessons-2026-10-07.md) archives | Handoffs and observers demonstrably ran, with passing runs | No matched solo, no costs; one passing observer run delivered zero advice |

The pattern: wherever a comparison existed, independent attempts plus selection
were at least as good as exchange, and cheaper; and the cheapest strong solo was
often the best result per dollar. This is not a proof of the null. The samples
are small and clustered. But four attempts with different tasks and models point
the same way, and the matched-compute literature agrees
([synthesis](../research/model-mixtures-and-communication-synthesis-2026-10-08.md)).

The experiments also failed on measurement before they could fail on the
hypothesis: saturated tasks, treatments that were declared but not delivered,
equal caps that produced unequal spend, and graders that disputed plausible
answers. The next study must fix those first.

## What has not been measured

Every experiment above tested exchange, mixture or personas through the direct
API. None tested Locust's actual mechanism: a shared record of attempts,
findings and failures that isolated agents read and cite. The only matched
evidence for that design is external and thin: CORAL (three tasks, matched on
wall clock, not tokens) and AlphaEvolve's archive ablation (two tasks). Both the
[literature review](../research/swarm-evidence.md) and the
[ecdsa.fail prior-art note](../research/ecdsa-fail-swarm-prior-art.md) name this
as the open question. Locust itself has been qualified end to end with real
agents ([live trials](../research/collaboration-followups.md)) but never measured
for an outcome.

## The next experiment

Three arms, matched on resources actually used:

| Arm | Design | Purpose |
| --- | --- | --- |
| 1. Strong solo | One agent with budget B, including self-review and retries | Quality, cost and time baseline |
| 2. Independent | k agents with B/k each, isolated; a frozen selection rule picks the result by independent check | Does additional search help on these tasks? |
| 3. Shared record | k agents with B/k each, isolated workspaces, reading and writing one Locust goal's findings, failures and artifacts; the same selection rule | Does the record add anything beyond search? |

The primary contrast is arm 3 against arm 2. That is the claim the product
rests on. Arm 2 against arm 1 is a secondary check that search helps at all on
the chosen tasks. Arm 3 runs through the real daemon, so the study also
qualifies the product.

Within the budget, k should be small (two to four) and fixed before scoring.
State in advance whether B is dollars, tokens or wall clock, and report the
others.

### Tasks

Prefer tasks with a graded score over pass/fail. Two studies died on pass/fail:
the negotiation pilot on a ceiling and the evidence study on disputed exact
matches. A continuous score (an optimization target, a measured cost, a
verified metric) produces signal on every trial, has no annotation to dispute,
and rarely saturates. CORAL and the ecdsa.fail challenge are both of this kind.

Hard repository repairs with independently validated regression tests are the
alternative, as the synthesis proposes. They need a calibration round that
shows the strong solo fails on a useful fraction of instances, roughly 30 to 70
percent, before anything is frozen.

Either way: calibrate on separate instances, then freeze the held-out set and
never revise it after seeing arm outcomes. The unit is the task family, not a
test case, call or message.

### Requirements carried over from the failed studies

1. **Match on used spend, not caps.** Report quality against actual cost as a
   curve. Do not use a gate that rejects a baseline for being cheap, as the
   evidence study's 90 percent rule did; offer the cheaper arm sensible extra
   work at larger allowances instead.
2. **Verify the treatment was delivered.** A phase name or a completed
   lifecycle is not evidence that sharing happened. Arm 3 should record, per
   candidate, which findings were read, acknowledged and cited as sources
   before the candidate was produced. The `ctx:` acknowledgments and signed
   source declarations make this measurable without trusting the model's
   account.
3. **Keep the grader out of reach.** Hidden evidence stays hidden from every
   arm, including the selector. Agreement, Locust acceptance and a model's
   verdict are advisory. Selection by hidden labels is an oracle diagnostic,
   never the deployed rule.
4. **Charge everything.** Selection, reading the record, failed calls, retries
   and unused dependent work count against the arm that needed them. Shared
   work paid once experimentally is still attributed to every arm that uses it.
5. **Diagnose the mechanism.** Report three things separately:
   complementarity (one initial candidate scores well where another fails),
   integration (the selector keeps the better one), and repair (reading the
   record produces a better result, or damages a good one, relative to arm 2).
6. **Report failures as results.** Technical failures, early stops and
   undelivered treatments stay in the assigned-arm comparison. Reruns are
   disclosed, never substituted silently.
7. **Size from calibration.** Choose the sample from calibration variance and
   a declared smallest effect worth acting on. With a smaller budget, label the
   run exploratory rather than weakening the controls.

### Before a paid run

Freeze and record: task families and grader, model versions, k, B and its
unit, the selection rule, the skill text given to each arm, sample size, stop
and outage rules, and the primary analysis. Choose amounts from the owner's
authorization and measured calibration costs. No standing allowance follows
from earlier pilots.

## Sequencing and what this changes

- The four-arm core comparison in the
  [self-organization plan](self-organizing-collaboration-plan.md) asks who
  should choose the organization. That matters only once some arrangement
  beats a strong solo at all. It is deferred until arm 3 shows a signal.
- Deprioritized: model mixture as the diversity lever; personas and profiles;
  reciprocal exchange as a default method; decision markets, which the
  [paper review](../research/agent-crowds-and-decision-markets-2026-10-07.md)
  already separates as its own track.
- No runtime change follows. The product's existing rules already match the
  narrowed thesis.
- The site's line about collective intelligence should be read as the
  narrowed claim: hard and checkable problems, pooled attempts and findings
  rather than a group that reasons better. Changing the wording is a separate
  site task.
- A result in any direction is useful: the record pays for itself on some
  task families, independent attempts are enough, or solo remains preferable
  over the tested range. Each would change the default formation guidance.
