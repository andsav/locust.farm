# Merak10 and Locust: organization, support and authority

Status: read-only source assessment, 7 October 2026. This supports the proposed
[self-organizing collaboration plan](../docs/self-organizing-collaboration-plan.md).
No runtime test, new model experiment or cross-machine qualification was run.
Tests below were inspected as evidence of intended/enforced mechanisms; they
were not rerun in this assessment.

## Source boundary

Merak10 was clean at `eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae`.
Locust review began at `93e187e8d84b33188b08b84ec3b67a9a957cfa61` while other work
continued in the checkout. Hook commits landed during the assessment; the final
source pass incorporates `5c0ebb9` and explicitly distinguishes their evidence
below. The
[source manifest](evidence/self-organization-source-2026-10-07.json) pins the
selected files, Git objects and content hashes used for this assessment. An
unrelated research index edit was preserved. This snapshot is not a claim that
all pending plan phases are built or that today's source reproduces historical
Merak4/Merak8 runs.

## Main finding

The useful distinction is not deterministic Merak versus non-deterministic
Locust. Both enforce deterministic contracts, and both can support adaptive
choices. Compare organizational discretion separately from distribution of
execution authority.

Merak's admitted blueprint is fixed for that run, but agents can create child
programs, choose delegation and participate in discussions. Its team can evolve
within a locally governed run tree. Locust lets independently authorized agents
coordinate through durable shared state, while local harnesses execute their
work. Locust can also run a fixed formation. The experiments must specify which
of these capabilities are available rather than infer the treatment from the
product name.

## What current Merak already contributes

The Merak links below refer to the inspected commit in its configured origin.

| Mechanism | Source evidence | Consequence for the plan |
| --- | --- | --- |
| Dynamic delegation | [`tasks.rs`, 250–397](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/crates/merak-agent/src/tasks.rs#L250): inline agent, inline blueprint, stored hash/template; model, tools and detach options | An adaptive team need not be authored completely in advance; child execution remains locally governed |
| Agent-authored programs | [`blueprint_put.rs`, 1–15](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/crates/merak-agent/src/blueprint_put.rs#L1), `workflow.rs:74` | Agents can validate and reuse exact program bodies; the narrower chat workflow lane must not be confused with all delegation |
| Agent-chosen discussions | [`board.rs`, 31–123](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/crates/merak-agent/src/board.rs#L31); `provider_first.rs:32` | Create/read/post/subscribe/unsubscribe allows choices of communication; board scope is an opted-in run tree, not a distributed goal |
| Runtime observation and advisory delivery | [`watch.rs`, 175–249](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/crates/merak-agent/src/watch.rs#L175) and its module contract | Observation need not depend on the executor volunteering a progress message; advice can be delivered at a replay-stable provider boundary |
| Enforced completion review | [`semantic_review.rs`, 1–40](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/crates/merak-agent/src/semantic_review.rs#L1) | A mandatory approval gate is distinct from advice, even if both are called an observer |
| Pinned skill bodies | [`skills.rs`, 665–791](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/crates/merak-tools/src/skills.rs#L665) | Discover/load/version/use are separate; preserve the exact skill used without making Merak's registry a Locust dependency |
| Child authority checks | [`authority.rs`, 1–14 and 99–115](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/crates/merak-domain/src/authority.rs#L1) | Inherited local authority does not transfer to independently owned Locust participants |

The blueprint editor can save changed topology and run an exact compiled hash
(`src-tauri/src/blueprint_verbs.rs:697`). Topology review uses the executed
snapshot and a digest (`topology_review.rs:225`); proposed edits are for human
application, not evidence of an autonomous live graph rewriter. Likewise,
`merak-loop/src/lib.rs:552` separates produce/evaluate/select/stop, while dreaming
is a host policy. These are useful experiment patterns, not proof that Merak
already optimizes team organization autonomously.

The source-inspected test boundaries are unusually useful:

- [`board_scope.rs`](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/crates/merak-store/tests/it/board_scope.rs#L173),
  173, 181, 234 and 338: separate roots, ordinary chats without automatic board
  scope, reopen and nested inheritance.
- [`pubsub_agent_tools.rs`](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/crates/merak-engine/tests/it/pubsub_agent_tools.rs#L2386),
  572, 1001 and 1161: waits and steering; 2386 and 2474: an uncooperative executor
  can be observed, with private watcher transport outside model authority;
  2805: completion rejection and repair.
- The same test file, 3126–3185, uses `FableSolProtocolProvider` to exchange
  evidence and change both scripted agents' next decisions. Its run name's
  `live-protocol` wording does not turn that into a real-model efficacy result.
- `crates/merak-engine/tests/it/board_agent_tools.rs:176,506,621,686` covers
  durable scope, non-consuming notices, replay and repeated waits.

The larger `docs/AGENT_SWARMS_IMPLEMENTATION_PLAN_2026-10-03.md` is explicitly
plan-only. Its work-item/pool contract was not found in the inspected board and
Map runtime surfaces. Existing discussion and monitoring features are built;
they should not be called unbuilt just because that broader plan is pending.

Merak also has implementation-specific child, delegation, workflow and wait
limits (`tasks.rs:35–85`, `merak-engine/src/subgraph.rs:1219`,
`workflow_limits.rs:74`) and a ceiling in the topology-review blueprint. These
are observed constraints, not proposed Locust defaults. The new research must
not smuggle them in as requirements or silently impose new product budgets.

## What Locust already supports and nudges

| Mechanism | Current source/test | Consequence |
| --- | --- | --- |
| Member-created work, offers and refusal | [Task requests](../crates/locust-core/src/node/requests/tasks.rs), 60, 108, 147, 170 | Decomposition and offers are available; task revision is host-only; an offer grants no execution authority |
| Independent attempts | [Claim requests](../crates/locust-core/src/node/requests/claims.rs), 87, 153; [goal tests](../crates/locust-core/src/goal/tests.rs), 419 | The caller currently names a task; retries and generations matter; several independent attempts can coexist |
| Ranked available work | [Views](../crates/locust-core/src/node/views.rs), 361 and 640; [context-view tests](../crates/locust-core/src/node/tests/context_views.rs), 625 | Least-attended ordering already influences attention; it is not a neutral unordered board or a global assignment |
| Automatic review opportunities | [Flow](../crates/locust-core/src/goal/flow.rs), 238; [role tests](../crates/locust-core/src/node/tests/roles.rs), 891 and 953 | Eligible-peer requests arise from completion rules; a new nudge must not add another acceptance mechanism |
| Exact context and acknowledgments | [Context](../crates/locust-core/src/node/context.rs), [assembly](../crates/locust-core/src/node/context_views.rs); [tests](../crates/locust-core/src/node/tests/context_views.rs), 266, 378, 448 | Reuse observational snapshots, revision fencing and per-session receipts; acknowledgment does not prove adoption |
| Local action boundaries | [Access](../crates/locust-core/src/node/access.rs), 213, 226, 312–339; [level tests](../crates/locust-core/src/node/tests/levels.rs), 101, 169, 243 | Replay checks the goal's contract; local levels and allowances further constrain acts |
| Explicit local launch and recovery | [Client](../crates/locust/src/cli/client.rs), 138–193, 248–273; [sessions](../crates/locust-core/src/node/requests/sessions.rs), 99 | Pending work is not authority to launch another person's process; unfinished and uncertain work needs reconciliation |
| Contribution-backed completion | [Claims](../crates/locust-core/src/node/requests/claims.rs), 24; [lifecycle tests](../crates/locust-core/src/node/tests/lifecycle.rs), 221 and 862 | Completed reporting needs an effective matching contribution, not necessarily review/integration/external correctness |

`goal/flow.rs:1` explicitly describes deterministic logical effects; configured
stages at line 26 can materialize prescribed work. Self-organization is a mode
within enforceable rules, not a requirement that Locust abandon fixed flows.

The [installed skill](../skills/locust/SKILL.md), 126, asks agents to prefer work
nobody holds and publish an independent result before reading peers' results on
the same task. At 155 it explains the unenforced `After task:` convention. These
are methodological interventions and must be held fixed or varied explicitly.
Its source citations at 85 correctly distinguish a declared source from proven
understanding. New trace analysis should retain that distinction.

Approved A2 would permit no-task start and therefore delegate a local choice to
the daemon; it remains unbuilt at this snapshot. H1a/H1b/H2 hooks landed during
this assessment. A1 tool/context wording and A3 compact current findings are
present, despite stale prose in parts of the companion plan. The
[master build-order table](../docs/master-plan.md) remains
the source of accepted sequencing; review the exact current revision before any
implementation.

### Hook work incorporated during this assessment

The [hook core](../crates/locust-adapter/src/hooks/core.rs), 397, supplies
pending-work stop interventions, cancellation and lost-claim notices. Associated
interactive workers do not park; unattended parking follows the explicit mode.
The [runtime](../crates/locust/src/hook.rs), 313, authenticates and reads state;
local marks track delivery. It does not perform shared work for an agent. A
successful attributed Locust callback establishes chat association, and inactive
unassociated chats remain silent.

The [qualification record](agent-hooks-qualification-2026-10-07.md), 124 and 234,
contains earlier real-model/native continuation evidence for Codex, Claude Code
and Droid. Pi's installed shim uses a fake host API against a real daemon;
native Pi discovery, execution and model behavior remain unrun (296). The latest
review fixes have recorded four-adapter process replay and full repository
checks, without renewed real-model trials (416). These are the other change's
reported checks, not tests rerun for this plan or proof of better collaboration.

H3 acknowledgment reset/presence is still pending. `client run` explicitly
disables hooks; native interactive prompt behavior, closed-chat waking and
two-computer collaboration are not qualified by these records. The new
`WorkItem.unattended` uses all current running attempts, including the caller's,
whereas `attempting` lists other participants. Future A2 selection and free-work
cues must use the right fact. The study must freeze its exact entry path and
adapter instead of treating hooks as either universally absent or universally
qualified.

### A concrete missing guidance surface

[`organization.rs`](../crates/locust-proto/src/organization.rs), 69, stores
formation context guidance. [`EffectiveRules`](../crates/locust-core/src/goal/rules.rs),
13, does not include it; the full
[context snapshot](../crates/locust-core/src/node/context_views.rs), 80–103,
serializes those effective rules. The [status document](../docs/status.md) also
calls guidance reading unbuilt. Exposing pinned, attributed guidance would close
a real surface gap, but it is not necessary for an initial study with an
explicitly supplied skill. Guidance remains advice, not imported authority.

## What to carry forward

1. Define organizational freedom by actual choices and ability to revise them,
   not by the absence of a leader or deterministic infrastructure.
2. Start with current Locust tools and a method repertoire. Preserve independent
   experimentation as a choice, not a mandatory first stage for every problem.
3. Build cues from observed context, exact evidence and current eligibility.
   Overlapping attempts may be useful; wall-clock silence is not proof of a
   stalled model. Missing local evidence is not global absence.
4. Reuse Merak's advisory-delivery separation and lifecycle lessons. Keep hard
   review/authority gates explicit; an acknowledgment does not show that advice
   was useful or even accepted.
5. Preserve task, attempt, generation, local run, contribution, acceptance and
   verification as distinct records. Measure harmful persuasion and wasted
   coordination alongside corrections.
6. Compare supported fixed and adaptive teams under one harness before comparing
   products or distributing them across owners. Add same-family controls before
   attributing an effect specifically to model heterogeneity.
7. Do not make this research a replacement for approved v2 implementation or a
   new release gate. Skill-only research can precede production cue features.

## Optional executor integration

Merak's
[Locust product plan](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/docs/LOCUST_POLARIS_PRODUCT_PLAN_2026-10-03.md#L53),
53–74 and 180–183, preserves the appropriate seam: Locust owns distributed
collaboration; Merak can execute a locally accepted attempt. Persist the binding
before launch and reconcile idempotently across stores. Its earlier detailed
Locust capability table is historical, not a substitute for the current source
review above. Remote participants do not become synthetic local child runs.

This assessment strengthens the design vocabulary and identifies reusable
mechanisms. It supplies no new evidence that an adaptive or heterogeneous team
outperforms a strong solo model.
