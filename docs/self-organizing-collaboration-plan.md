# Supported self-organization: research and implementation plan

Status: proposed, 7 October 2026. The owner requested this plan after clarifying
that Locust should let agents organize themselves, with help from skills and
daemon cues. The detailed design and experiments below are proposals, not an
approved change to the [v2 build order](master-plan.md). This document authorizes
no new experiment spending and introduces no production behavior. The
[collaboration thesis](collaboration-thesis.md) of 8 October narrows the
working hypothesis below and defers this plan's core comparison until a shared
record has shown a signal against independent attempts.

## The question

Can independently authorized agents choose and revise a useful way of working
together, and solve problems better for the resources spent than a strong solo
model or a well-designed fixed team?

The working hypothesis is that different models sometimes make different useful
observations, and that agents can exploit those differences by choosing when to
work independently, advise, challenge, divide work, consolidate or stop.
Organizing skills and timely daemon cues may make these choices more effective.
Neither heterogeneous models nor self-organization is assumed to improve every
task. The useful outcome might be a small repertoire of conditional methods,
including recognizing when another agent would add no value.

This combines the [first pilot](../research/negotiation-pilot-results-2026-10-07.md),
[Merak4 tag-team assessment](../research/merak4-tag-team-comparison-2026-10-07.md),
[observer lessons](../research/observer-implementer-lessons-2026-10-07.md) and the
[current Merak10/Locust source assessment](../research/merak10-locust-organization-2026-10-07.md).
Those records establish mechanisms and limitations, not a collaboration advantage.
The [OpenAI mathematics assessment](../research/openai-math-methodology-2026-10-07.md)
motivates retaining exploration, falsification, repair, reusable findings and
independent verification. Its reported outcomes do not establish which topology
Locust should use or what advantage a small Locust team will have.

## A more accurate Merak–Locust distinction

Two axes matter: **who chooses the organization**, and **who controls execution**.
Deterministic replay and authorization can support adaptive organization.

| Arrangement | Organization | Execution authority |
| --- | --- | --- |
| Authored Merak pipeline | Roles, routes and stages set in a blueprint | One local runtime and its inherited authority |
| Adaptive Merak run tree | Agents can choose delegation, child programs, models and discussions | Still locally governed parent/child runs |
| Fixed Locust formation or experimental team | Work pattern specified in advance | Each participant's local execution authority remains separate |
| Supported self-organizing Locust goal | Agents choose and revise working arrangements within the goal's rules | Each participant and harness remains locally governed |

Merak10 already supports dynamic `task.spawn`, authored child blueprints,
run-tree discussions, runtime watcher advice and versioned skills. It is more
adaptive than the earlier observer pipeline alone suggests. Locust already has
optional deterministic formation flows, review requests, task ordering and
organizing instructions. Neither product is an organizational blank slate.

The first comparison should run fixed and adaptive organizations through the
same Locust-backed harness. A direct Merak-versus-Locust score would mix
organization, models, tools, execution engines and distributed overhead. Later
integration tests can study a Merak executor without making it a requirement for
Locust or attributing all differences to self-organization.

## What self-organization means here

Agents receive a goal, its permitted work, available participants, complete task
information and any explicitly chosen resource allowance. They can make and
revise organizational decisions during work. The experimenter does not prescribe
a universal sequence of independent answers, critiques and synthesis.

| Agents may choose, within existing authority | This does not confer |
| --- | --- |
| Open subtasks, offer work, accept or decline, choose an attempt | Authority to enroll a participant, launch another owner's process or change its level |
| Request an observer, a counterexample, an independent attempt or a review | A new permission role or power to make an unchecked result count |
| Take a temporary lead or consolidation responsibility | Governance authority or an exclusive global task lock |
| Change method, end their own attempt, decline more work or replace their working hypothesis | Power to change goal membership, revise arbitrary tasks, change pinned rules or erase others' findings |
| Finish without using another participant when appropriate | Permission to bypass the goal's actual acceptance rule |

Task revision currently belongs to the host. Agents can open appropriate new
work and explain a changed plan without pretending that they can revise any
existing task. A temporary *observer* or *coordinator* is a working arrangement;
a formation role is an authorization fact. Keep those meanings distinct.

The observable test is whether a material choice was available, who selected
it, what evidence prompted it, and whether the agents could decline or change
it. A team may settle on one leader or a stable observer/implementer pair. It may
also choose no collaboration. These are valid outcomes. Role churn, message
volume and a particular graph shape are not success criteria.

For example, agents might notice a disagreement in a boundary case, choose
independent probes, ask one peer to construct a counterexample, then retain one
implementation and ask another to verify it. That is an illustrative trajectory,
not a daemon-prescribed workflow.

## Established starting points

The [literature review](../research/self-organizing-agents-literature-2026-10-07.md)
checks 23 primary works. Supported self-organization is established prior art;
this plan applies and tests it in Locust's setting. It does not propose inventing
another general orchestration framework.

| Prior work | Concrete use in this plan |
| --- | --- |
| Contract Net, GPGP | Voluntary allocation, explicit commitments, conditional coordination methods and mechanism ablation |
| Blackboard systems, stigmergy | Shared findings/artifacts as an organization mechanism without requiring live conversation |
| MOISE+, electronic institutions | Distinguish behavioral roles, plans, obligations and execution authority |
| AutoGen, AgentVerse, DyLAN | Established dispatch, recruitment and selective participation comparators |
| GPTSwarm, AFlow | Strong fixed methods selected or optimized on calibration; account for search costs |
| MorphAgent | Existing profile/role adaptation; compare proxy-based feedback with factual cues |
| CORAL | Reuse task/prompt ideas and a shared-memory control; do not treat its wall-time comparison as matched spend |
| Self-MoA, debate/voting and diversity studies | Strong same-model and independent-selection controls before attributing benefit to heterogeneous negotiation |
| Capability-scaling studies, MAST, AI Agents That Matter | Task-dependent gains, established failure labels, proper holdouts and complete cost accounting |

The linked review records primary URLs, versions, reading limits and empirical
boundaries. A paper's reported improvement is not a Locust result. Published
thresholds, role-diversity proxies and agent-count ratios are hypotheses or
measurements in particular settings, not daemon defaults.

Describe adaptation by when it occurs (before a trial, between attempts or during
work), who chooses it (designer, optimizer, coordinator or participants), and
whose execution authority applies. Cross-run workflow optimization is a strong
baseline but does not itself test peers organizing during work. No claim of
novelty or superiority follows merely from combining existing mechanisms.

## Keep four responsibilities separate

1. **Goal rules:** membership, eligible actions, acceptance, host acts and signed
   history. Locust enforces these deterministically.
2. **Local authorization and execution:** Locust enforces credentials, sessions,
   levels and task allowances; each harness enforces its process/tool/filesystem
   policy. A study runner or explicitly capable harness enforces an authorized
   dollar allowance. Do not assume every harness supplies monetary accounting.
3. **Organization:** agents' choices about decomposition, attention, temporary
   responsibilities, collaboration and when to change course.
4. **Support:** attributable skills and daemon observations that inform those
   choices. Support does not acquire the authority of the first two layers.

[Local access checks](../crates/locust-core/src/node/access.rs),
[task requests](../crates/locust-core/src/node/requests/tasks.rs) and
[attempt/session checks](../crates/locust-core/src/node/requests/claims.rs)
enforce existing boundaries. The new support contract below is proposed until
implemented and tested. An offer or suggestion remains data to assess, even
when it is delivered reliably at a tool boundary.

## Start with existing primitives

The initial study needs no organization optimizer, new board, replicated
strategy schema or model inside the daemon. Use tasks, offers, declines,
attempts, contributions, source references, reviews, context reads and the
shared workspace. Put a brief explanation of an arrangement or change in an
ordinary finding when it matters to another participant. Keep trial accounting
and detailed interaction measurements in a separate research trace.

Distinguish shared facts, attention cues, voluntary behavioral commitments and
enforced authority in those records. A commitment names a bounded output and
responsibility; accepting, declining or changing it is explicit. Allocation
negotiation is separate from epistemic negotiation about whether a claim is
correct. Neither a bid nor a confident commitment establishes competence.

Existing behavior must be recorded rather than silently treated as neutral:

- [Pending work](../crates/locust-core/src/node/views.rs) is ordered by other
  active attempts, result count and task identifier. It suggests work; current
  attempt start still names a task. Approved A2 will add delegated local choice
  when no task is named. Those are different decision paths.
- The [installed skill](../skills/locust/SKILL.md) prefers unattended work and
  asks for an independent result before reading peers' results on that task.
  This is a useful policy to test, not the definition of self-organization.
- [Review requests](../crates/locust-core/src/goal/flow.rs) already follow the
  goal's completion rule, including formations with no configured flow.
  Do not create another mandatory review mechanism.
- The built H1a/H1b/H2 hooks already deliver facts and stop interventions in
  associated chats. H3 remains pending; `client run` disables hooks. Native
  evidence differs by harness, and the latest hook fixes have process-replay
  evidence without renewed real-model qualification. Record the exact adapter
  and entry path; see the [hook qualification](../research/agent-hooks-qualification-2026-10-07.md).
- Parent tasks and the `After task:` title convention do not enforce dependency
  scheduling. An agent must inspect and honor the convention; a daemon picker
  must not be described as satisfying it automatically.
- Formation `context.guidance` is stored but is not in the agent's effective
  context snapshot. A study can supply a frozen skill explicitly; product
  guidance exposure is a separate small candidate change.

## Organizing skills

Provide a concise repertoire that helps agents choose a method, rather than
requiring all methods or assigning roles on arrival. Each method states when it
may help, evidence to inspect, what a peer needs, expected output, when to stop
or switch, and what authority it does not grant. Keep the operational skill's
authentication, freshness and correctness instructions common across conditions.

| Method | Useful trigger | Evidence to leave |
| --- | --- | --- |
| Offer a bounded contribution | Uncertain division of work or a peer seeking a capability | Relevant ability/availability, agreed output, acceptance/decline and changed commitments |
| Independent exploration | Uncertain approach; risk of anchoring; multiple plausible mechanisms | Initial claim or candidate before exposure, assumptions, discriminating test |
| Observer and implementer | Work is progressing but a specific assumption or decision warrants another view | Checkable concern, artifact/revision, suggested check; implementation response |
| Counterexample search | A result is plausible but brittle or disputed | Reproducer or falsifiable challenge, scope and outcome |
| Divide and recombine | Subproblems have separable interfaces | Inputs/outputs, interface agreement, integration responsibility and check |
| Temporary lead or consolidator | Several useful strands need a coherent next action | Chosen candidate, unresolved dissent, rationale and responsibility; no new authority |
| Tag-team handoff | A peer can benefit from an existing workspace and change approach | Exact workspace state, failed approaches, unresolved question, receiving acknowledgment |
| Review and correction | A concrete result is ready for eligible review | Exact candidate, actual checks, rejection or approval reasons |
| Continue alone or stop coordinating | No additional question needs a peer, or coordination costs exceed expected benefit | Brief reason when relevant; obey the real acceptance and lifecycle rules |

A handoff between independently governed agents transfers a usable artifact and
public work summary, not another model's private reasoning or a credential.
The local executor may support a model switch in one workspace, but that is a
distinct mechanism to label. An observer should be able to inspect task evidence
within its permission scope; commentary on the implementer's explanation alone
can inherit the same mistaken premise.

Hash the exact skill text and record discoverability, load and subsequent use
separately. Skills may suggest preserving an independent first view where useful;
they must also explain when early exchange is appropriate. In the research
harness, factor the installed skill's organizing sentences into the assigned
treatment, retaining its authority and tool-use rules in every arm. Do not
silently rewrite the production skill merely to create a baseline.

## Daemon cues: factual, attributable and optional

Begin with observations already derived for context and pending views. Add a cue
only when a trial identifies a missed, useful signal. Proposed examples are:

| Observed fact | Possible suggestion | Important qualification |
| --- | --- | --- |
| A candidate is currently eligible for this participant's review | Inspect it and decide whether a check is useful or required by the actual rules | Eligibility is not correctness; a suggestion adds no approval authority |
| Several attempts are observed on one task | Consider whether independent work or division is more useful | Overlap may be intentional diversity; the local view is incomplete |
| A peer has published relevant new evidence or explicitly reported a blocker | Read the evidence or offer a focused check | Do not infer a blocker merely from silence or age |
| A work offer is waiting | Accept, decline or propose a narrower contribution | An offer cannot launch an agent or override its owner |
| A referenced workspace revision is not locally ready | Resolve or report the concrete readiness problem | Accepted authority and available content are different facts |

Each cue identifies the subject, observed event IDs/revision, provenance, the
fact observed, why it may matter, and any suggested method. It must say when the
daemon does not know enough. Peer text stays attributed source material; it must
not be laundered into a system instruction. No model inference is presented as
a daemon-observed fact.

Read, acknowledgment, response, adoption and verified effect are separate.
Declining or deferring organizational advice has no effect on eligibility.
Deduplicate an unchanged cue by its evidence identity and context, rather than
interrupting on a fixed cadence. Recheck freshness before acting. A revised task,
new governance, removal, ended goal or restore hold must obsolete or qualify the
cue and cannot be bypassed by it. Any persistence needed across session restart
must be specified as local delivery bookkeeping, not new consensus state by
accident.

Use the existing [context](../crates/locust-core/src/node/context.rs),
[context views](../crates/locust-core/src/node/context_views.rs),
[pending projection](../crates/locust-core/src/node/views.rs) and approved
[H hook adapters](agent-memory-and-store-plan.md). MCP remains the action path.
Do not build a parallel scheduler, closed-chat launcher or new polling service.
H's stop intervention and A2's delegated task choice are real behavioral
interventions; keep them fixed or explicitly vary them in experiments.

Merak10's runtime watcher is a useful delivery precedent: the runtime can
observe activity without the executor volunteering it and present advisory
messages at a safe boundary. Its mandatory semantic-review gate is a different
mechanism. Copy the separation and delivery lessons, not Merak's authority tree,
internal event format or incidental limits.

## Evidence and outcome contract

Record public decisions and actions, not hidden reasoning. Retain enough to
reconstruct both the assigned treatment and what actually happened:

- Exact task, starting workspace, model IDs, harness/tool versions, skill and
  cue versions, initial information, permissions, goal rules and trial seed.
- Available participants versus participants activated; chosen tasks, offers,
  declines, arrangement proposals, local responses and revisions.
- Evidence available to a peer, its completed advice, recipient delivery,
  acknowledgment, action/rejection and candidate hashes before and after.
- Locust event/source IDs and attempt generations, alongside separate local
  execution IDs and provider usage. A signed source citation is a declared
  relationship, not proof of comprehension or causation.
- External quality, valid-artifact production, orchestration completion,
  intervention delivery, elapsed time, total usage/cost and unattended tails.

Keep private prompts, tool payloads and credentials out of shared/public views
unless their sharing is already authorized. Local observation does not imply a
right to stream a participant's whole execution trace to peers. An agent can
publish a useful finding without exposing its entire working context.

Use a frozen evaluator independent of peer approval. Preserve disagreement,
failed hypotheses, rejected advice, wrong-to-right and right-to-wrong changes.
An accepted contribution, successful process exit, complete graph node and
correct artifact are separate outcomes. On success, failure or cancellation,
stop observation that requires the implementer's now-inactive execution stream.
A separately accepted review of its artifact retains its own lifecycle and
acceptance conditions. Retain the evidence and account for all work after the
implementer terminates. Do not resurrect cancelled work through a cue.

## Experimental program

### First repair the measurement problem

The first pilot saturated: all completed initial candidates already passed the
hidden cases. Some critique phases also exhausted their allowance before
producing a response. Merak's historical passing runs did not establish useful
observer participation, and one failed implementer left observers running for
almost ten minutes. More team variants on those easy tasks would not answer the
research question.

Use separate calibration and held-out sets. Start calibration with six problem
instances across repository repair, specification/algorithm reasoning and
multi-step evidence synthesis with checkable claims. Prefer problems whose
correctness can be checked independently and whose difficulty is not simply
missing information from the solo condition. Include tasks with separable and
tightly coupled work, plus some simple controls where collaboration may be
wasteful. Keep unverifiable open research as a separately assessed track.

Calibration must demonstrate genuine unresolved errors or useful disagreements
under realistic allowances, reliable tool/communication delivery and an
independent evaluator sensitive to the relevant failures. Tune task generation,
prompts and allowances only on calibration; freeze them before scoring held-out
instances. Split by underlying problem/template where structural reuse could
leak a calibration answer; new random fixtures alone may not create a held-out
problem. Avoid a benchmark made entirely of near-universal successes or failures.
Do not inspect held-out grades to decide which tasks, agents or
organizations to keep. If the held-out set saturates, report the ceiling and
design a new preregistered study.

### Core comparison

Use the same Locust-backed runner, task information, public tools and total
dollar ceiling for these conditions. Match the eligible participant pool and
local authority between team arms; each solo gets its named model. Use `open`
as the proposed common experimental acceptance baseline so no arm needs another
agent to finish. The independent grader determines quality. This isolates
organization under that baseline; qualification under the product's default
peer-review rule is a separate step.

| Condition | What is controlled |
| --- | --- |
| Solo A | Strong model A with the full information/tools and self-review available |
| Solo B | Strong model B with the same opportunities; avoids choosing the weaker solo after observing results |
| Fixed A+B | Best fixed arrangement selected on calibration from a declared menu, then frozen |
| Adaptive A+B with support | Same available models; agents select participation, method, temporary responsibilities and changes using the frozen organizing repertoire |

The fixed menu can include independent portfolio, one-way synthesis, reciprocal
critique, observer/implementer and tag team. Publish the calibration rule used
to choose it and all calibration costs. A fixed policy may have frozen
evidence-dependent branches; it cannot author a new organizational policy during
the scored trial. Selection or optimization of its policy happens on calibration
only. Match available capabilities: a fixed arm must not be deprived of evidence
inspection or a tool the adaptive arm gets.
Use identical frozen method knowledge and cue-generation policy in fixed and
adaptive arms; cue contents naturally reflect their different observed states.
Only the declared instructions controlling organizational discretion differ.
If this cannot be achieved, label the contrast as a combined organization/support
effect. Retain every prompt difference. Predeclare and counterbalance the initial
acting model across team trials; the adaptive arm may choose not to activate its
available peer. Do not force peer activation to make a trace look collaborative.

Run the same held-out instances in every arm with isolated state and randomized
arm order. A practical first batch is 24 paired instances spanning at least six
families, with repeat model samples where feasible. This is a proposed initial
design, not a power claim: use calibration variance and the chosen smallest
worthwhile effect to size a confirmatory study. With a smaller available budget,
label the run exploratory rather than weakening the controls or promising
statistical significance.

Primary contrasts are adaptive versus fixed organization, and each team versus
each solo baseline. These measure the complete specified systems. They do not
by themselves isolate heterogeneous priors, particular advice, or independent
ownership. Add adaptive A+A and B+B controls before claiming an advantage from
heterogeneity rather than another sample of reasoning. Different model labels
alone do not establish different relevant priors.
Add a centrally adaptive A+B coordinator condition before claiming that peer
organization itself beats centrally controlled adaptation. The core comparison
cannot make that distinction.

Equal maximum dollars are not equal actual compute. Charge all selection,
planning, coordination, failed calls, summaries, tool/model work and unused
dependent work to the relevant trial. Record actual tokens, cost and time, and
report quality against expenditure as well as the assigned ceiling. Shared
study instrumentation does not silently supply extra reasoning to one arm.
If using multiple budget levels, freeze the levels and allocation rules in
advance; avoid the prior pilot's accidental starvation through rigid phase shares.

### Separate the supports and mechanisms

After the core runner is reliable, run a 2×2 study within adaptive organization:
basic operational instructions only, organizing skills only, daemon cues only,
and both. Keep existing task ordering, automatic review requests, hooks and
no-task selection behavior identical across these cells. Document any support
already present in the baseline. A cue can be read and rationally declined;
forced adoption would change the treatment.

Then investigate specific mechanisms only where evidence warrants:

- Shared-record coordination versus explicit exchange, keeping accessible
  findings and artifact checks fixed. Credit useful asynchronous reuse without
  requiring a conversation or inventing another shared-memory system.
- Evidence-grounded cues versus profile/role feedback using an established
  method such as MorphAgent. Treat profile clarity or diversity as a proposed
  mechanism, not the outcome to optimize.
- Self-review versus same-family and different-family advice, holding model
  rotation, tools and completion checking constant.
- Shared-state tag team versus same-model handoff with the same summary and
  compaction, separating switching from reset/summary effects.
- Independent portfolio versus one-way synthesis versus reciprocal exchange,
  charging candidate selection and synthesis. Hidden-label best-of selection is
  an explicitly labeled oracle upper bound, never the deployed selector.
- Matched continuations from a frozen uncertain/failing workspace, assigned to
  no message, self-review or peer critique. Predeclare collection and selection
  of those states before observing advice or continuation outcomes; retain the
  sampled states, including those where advice proves useless. Account for
  critique generation; this estimates correction at that state, not the whole
  adaptive policy.
- One local host versus independently operated hosts only after the mechanism
  works locally. Vary authority/distribution separately from organization.

Do not run every combination as a large factorial by default. Use the core
comparison to identify an uncertainty, preregister the relevant follow-up and
keep its claims narrower than its controls.

### Analysis and decisions

The unit is the goal/problem instance, not a message, hidden test case or agent.
Retain every assigned trial in system-level outcomes, including tool, provider,
budget and orchestration failures. Report delivery failures separately;
successful-treatment-only analyses are diagnostic and cannot replace the
assigned-condition comparison. Predeclare provider outage handling and show any
technical retries separately, preserving the original charges and results.
Use MAST's published failure categories as the starting vocabulary, with
human-checked evidence and additional Locust authorization/delivery/lifecycle
codes where necessary. A taxonomy label does not establish a causal diagnosis.

Report paired differences with uncertainty intervals and family-level results.
Cluster or resample at the problem/family level as appropriate; repeated samples
and fixtures are not independent new tasks. Freeze the primary quality measure,
smallest worthwhile effect, cost/latency tolerance and multiplicity treatment
before held-out runs. Report effort, latency and correctness separately instead
of inventing a favorable composite after the fact.

Adoption requires a repeatable useful tradeoff against strong solos and the
calibration-selected fixed team, demonstrated delivery of assigned capabilities
and recorded organizational choices, and no unacceptable increase in false
acceptance or lifecycle failures. Better method selection on some task families
can justify a conditional skill without justifying a default
for every goal. Equivalent quality at lower cost is useful; extra coordination
with no improvement is a negative result worth preserving. A failure to change
organization despite available choices is also a result, not permission to
quietly force an adaptive-looking trace.

Before scoring, the runner must pass delivery, authority, accounting, grader
isolation and lifecycle checks. Hidden-label exposure, unauthorized execution or
an unaccounted billable request stops affected execution and invalidates the
affected comparison; retain its artifacts and state the scope. A delivery
failure during scoring remains a failure of the assigned system. Pause expansion
to diagnose a systematic failure, without discarding completed or failed cells.
An exploratory favorable estimate can justify another study. A superiority
claim requires the prespecified uncertainty and practical-effect criteria; a
default product intervention also needs confirmation on a fresh held-out set.

## Phases and concrete deliverables

These are research steps, not new numbered v2 release phases. Preserve the
master plan's G2/E1/E2/R7/R8/R9/R10 order and existing A/H lane ownership. Recheck
the current build-order table before implementation; source is changing during
this assessment. Do not duplicate approved A2, A3, A4 or H work.

| Step | Work and likely seam | Exit evidence |
| --- | --- | --- |
| 0. Freeze the question | This plan, source assessment, existing pilot and historical lessons | Explicit hypotheses, boundaries and interventions; no efficacy claim |
| 1. Qualify a minimal study | Research runner beside `research/experiments/negotiation_pilot/`; shared operational instructions plus optional method repertoire; existing Locust tools | Real agents can offer/decline, choose an arrangement, read and act on evidence, switch or continue alone, and terminate cleanly; public decisions and all charges retained |
| 2. Calibrate and compare | Separate task calibration, frozen runner/graders/prompts, solo/fixed/adaptive arms | Preregistered held-out results, complete failure accounting, actual organization and uncertainty estimates |
| 3. Test missing support | Only evidenced gaps: guidance exposure or context cues; delivery through qualified H adapters | Skills/cues ablation; freshness, authority and delivery tests; no automatic assignment or role grant |
| 4. Explain and qualify | Focused mechanism controls, then distinct owners/hosts and supported harnesses | Replicated effects or a documented null result; operational qualification reported separately |
| 5. Choose product changes | Small conditional skill/default changes justified by results; optional Merak executor later | Updated design/manual, removal of superseded guidance, scoped tests and commits; unresolved research remains labeled |

Step 1 can use explicitly supplied study guidance with current APIs. It does
not need a production guidance endpoint, new signed record or autonomous agent
launcher. Frozen runner phases may initialize participants and collect results;
they must not prescribe the adaptive arm's substantive organization. If a
supported harness cannot deliver a chosen action, record that limitation and
repair it before making a quality comparison.

For a later **guidance projection**, inspect
[organization definitions](../crates/locust-proto/src/organization.rs),
[effective rules](../crates/locust-core/src/goal/rules.rs),
[context API](../crates/locust-proto/src/api/context.rs) and
[context assembly](../crates/locust-core/src/node/context_views.rs).
Expose the guidance pinned to the actual rules with source/version attribution
and advisory semantics. Test rule changes, unavailable definitions, stale reads
and task-specific pinning. Keep guidance out of authority decisions; follow the
then-current API/version contract and Lane A ownership.

For later **cues**, extend existing derived context only as needed. Characterize
eligibility, unchanged-evidence deduplication, pagination, restart, task revision,
membership removal, ended goals and restore holds. Test that declining advice
does not change authorization, multiple attempts remain possible, missing remote
state is not reported as absence, and unavailable workspace content is described
accurately. H owns hook wiring, current-fact/cancellation delivery and MCP
fallback within its approved scope. The research runner and local execution
lifecycle own dependent-observer shutdown, preserving separately accepted
artifact review as described above. A scripted fixture verifies transport; at
least one real agent trace must verify interpretation and action or a reasoned decline.

Research-only Python changes need focused evaluator, accounting, isolation and
trace tests. Rust implementation must pass `cargo fmt --all --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo test --locked --workspace`. Changed docs need `python3 scripts/check_docs.py`.
Any affected public recipes must execute against the pinned binary. Keep
source, simulator, single-machine real-agent and two-owner/two-machine claims
separate. No new study becomes a v2 release gate without an explicit change to
the accepted master plan.

## Optional Merak integration

A participant may eventually choose a Merak blueprint as its local executor.
Locust continues to own membership, tasks/attempts, collaboration history,
artifacts, synchronization and acceptance. Merak owns its local run tree, model
and tool execution, permissions and recovery. A peer is not synthesized as a
local Merak child run.

Persist the Locust attempt/generation-to-local-run binding before launch;
reconcile an uncertain launch and idempotent result publication across the two
stores. Keep cancellation requested, acknowledged and completed distinct. Use
the existing integration proposal as a starting point, revalidated against then
current Locust semantics. Integration remains a separate qualification step;
this plan does not need a second blueprint language or a Merak dependency.

## Decisions to freeze before a paid run

The design can be prepared without another product decision. Before execution,
record the selected task families and graders, model versions, treatment texts,
authority/acceptance baseline, support versions, total study allowance including
calibration and failures, planned sample size, stop/outage rules and primary
analysis. Select amounts from the user's authorization and measured calibration
costs. Do not infer a new standing allowance from the earlier $50 pilot or make
experimental ceilings into hidden product defaults.

The immediate next deliverable is therefore the minimal study protocol and
runner qualification for agent-selected organization. Production nudges follow
observed need; claims of collective problem-solving advantage follow held-out
evidence.
