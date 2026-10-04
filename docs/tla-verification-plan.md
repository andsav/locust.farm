# TLA+ formal verification implementation plan

Date: 2026-10-03. **Status: Stages 0 and 1 implemented, with separate
historical version-0 models and a version-1 bounded revision.** See the
[version-1 mapping and verification](../research/tla/version-1.md) and retained
[historical evidence](../research/evidence/tla/README.md). The bootstrap CI
workflow is configured but has not been observed running. Stages 2 through 5
remain proposed; no deductive proof is delivered. The version-1 revision covers
goal replay and scoped local claims, not all protocol-1 behavior.
Implementation began at `86980594acee708b2f9bf303afc9ee636dc55337`; the original
plan baseline was `3cb9c5da86d6351ccbeb620674e10ad5c9ea0d28`. The independent review
examined candidate `3422c7b51948a409481cf2cd9df1cc3f3a1b4dd1`. The evidence records
actual source identities, dirty state and checked path hashes.

Locust is a good candidate for formal modeling because its difficult failures
involve event ordering, authority, competing task actions and recovery. Start
with small models of those rules, reproduce known design failures, and connect
the results to Rust regression tests. Add replication and storage detail in
separate stages so the models remain understandable and exhaustively checkable.

## Scope and verification claims

The first deliverable is bounded exhaustive checking with TLC. A completed run
checks the specified properties for all behaviors in its finite configuration,
subject to its abstractions and assumptions. It does not establish correctness
for arbitrary swarm sizes or prove that the Rust implementation follows the
model. TLA+ models behavior above the code level; see
[Lamport's overview](https://lamport.azurewebsites.net/tla/high-level-view.html)
and the [TLC explanation](https://www.microsoft.com/en-us/research/uploads/prod/2016/12/Model-Checking-TLA-Specifications.pdf).

The plan covers goal replay, task decisions, local execution claims, event
reconciliation, retention, peer selection and durable signing. Small models can
use two or three logical replicas on one computer. They require no third
physical machine and do not alter the current M1/M2 daemon state, credentials or
test goal. Physical restart, sleep/wake and transport qualification remain
separate evidence in the [release ledger](release-evidence.md).

Abstract signatures as unforgeable author identities, hashes as collision-free
identifiers, and transport authentication as a correct endpoint binding. A member
may still sign conflicting events. Abstract payloads as identifiers and lengths
initially. This work does not verify Ed25519, BLAKE3, SQLite internals, Iroh,
discovery operators, coding-client integrations or the quality of agents' work.
Model the guarantees Locust requires from those components and test the relevant
implementation seams separately.

Keep two kinds of specifications explicit:

- **Current behavior:** faithfully represents the pinned implementation and
  reproduces known findings, including undesirable behavior.
- **Proposed behavior:** evaluates a documented rule change against desired
  properties. A passing proposed model does not qualify the current binary.

The [archived version-0 contract](protocol-v0.md) defines the historical modeled rules;
the [version-1 contract](protocol-v1.md) defines the rules for the separate
[version-1 models](../research/tla/version-1.md). Stage 0
corrected its stale scaffold status without changing protocol rules. The
[property map](../research/tla/property-map.md) separates implemented behavior,
desired guarantees and assumptions; contract prose alone is not verification.
The [independent review](../research/t1-candidate-independent-review.md) provides
the initial failure cases and probes.

## Models and properties

Use four models with explicit interfaces. Begin with atomic storage in the first
three; refine that assumption in the fourth. Keep task policy metadata such as
`depends_on` and `deadline_ms` out of enforced preconditions unless the contract
and code are changed to enforce them.

Every model checks `TypeOK` and its authority boundaries: a permitted local
session may author only for its bound principal, contributions require their
specified member or assignee, and only the coordinator may author decisions.

| Model | State and actions | Initial properties | Source and existing tests |
|---|---|---|---|
| `GoalLog` | Held events, author positions and predecessors, coordinator chain, membership tenures, standings, assignments, results and accepted heads. Author, deliver, duplicate, fork, remove and readmit. | Incremental view equals full replay; equal held sets yield equal projections and standings; only coordinator decisions advance the chain; one current assignment per task in a replayed chain; valid attempt budgets and acceptance preconditions; removal fences assignments from that tenure. | [Goal implementation](../crates/locust-core/src/goal/mod.rs), [chain](../crates/locust-core/src/goal/chain.rs), [fold](../crates/locust-core/src/goal/fold.rs), [transitions](https://github.com/andsav/locust.farm/blob/b758b12/crates/locust-core/src/goal/transition.rs), [goal tests](../crates/locust-core/src/goal/tests.rs). |
| `Sessions` | Principal/session binding, grants, assignment authorization, claim holder and generation, committed events and idempotency records. Claim, retry, take over, issue delayed writes, fail a commit and reopen. | One local holder per assignment; generations never decrease; stale writes cannot author new events; sessions cannot cross principals; identical request retries do not author twice; uncertain commit failure fences further signing until reopen. | [Claims](../crates/locust-core/src/node/requests/claims.rs), [sessions](../crates/locust-core/src/node/sessions.rs), [commit](../crates/locust-core/src/node/commit.rs), [lifecycle tests](../crates/locust-core/src/node/tests/lifecycle.rs), [failure tests](../crates/locust-core/src/node/tests/failure.rs). |
| `Replication` | Per-replica retained events, author frontiers, inventories, in-flight exchanges, peer eligibility and retry state. Transfer suffixes or missing IDs, interrupt exchanges, reconnect, screen events and change membership. | Transfers cannot invent or replace events; duplicates are harmless; equal-length divergent histories are detected; aborted exchanges cannot count as successful sync; retained history and fork evidence eventually reach eligible replicas under stated recovery assumptions. | [History](../crates/locust-core/src/goal/history.rs), [screening](../crates/locust-core/src/goal/screen.rs), [sync implementation](../crates/locust-core/src/sync/mod.rs), [driver](../crates/locust-core/src/sync/driver.rs), [peer selection](../crates/locust-core/src/node/peers.rs), [convergence tests](../crates/locust-core/src/sync/tests/convergence.rs), [driver tests](../crates/locust-core/src/sync/tests/driver.rs). |
| `Durability` | Process memory, OS-visible unflushed writes, durable records, authoring positions and exposed events. Write, flush, acknowledge, publish, kill process, reopen and lose power. | An acknowledged commit survives power loss; events and local idempotency/claim records commit atomically; an exposed locally signed event cannot disappear and permit a different event at the same author position after recovery. | [Store contract](../crates/locust-proto/src/store.rs), [SQLite store](../crates/locust-store/src/store.rs), [store tests](../crates/locust-store/src/tests.rs), [crash tests](../crates/locust-store/tests/crash.rs), [daemon durability tests](../crates/locust/src/daemon/durable_tests.rs). |

Claim generations are **local daemon state**, not distributed leases. A takeover
rejects subsequent stale local requests; it does not retract an already signed
contribution at another replica or guarantee that only one physical executor is
running. Model assignment authority separately. Likewise, deterministic replay
of equal held sets and eventual acquisition of those sets are different claims.

Make projection checks meaningful: maintain an incremental view as events arrive
and compare it with an independently defined canonical replay. Defining both
replicas' state as the same pure function would make equality automatic and
provide little evidence about the incremental implementation.

## Historical failures to reproduce

These scenarios originate in the version-0 review. Preserve their historical
counterexamples; version-1 revisions must model the implemented fixes identified
in the [impact assessment](../research/tla/upstream-impact-2026-10-04.md).
Each historical case starts with the version-0 rules. Save its counterexample, map the steps
to the existing review probe or a Rust regression, and evaluate any correction
in a separately identified model revision. Expected violations are evidence of
a reproduced finding, not passing verification of the violated property.

| Review finding | Required model scenario | Decision or correction to evaluate |
|---|---|---|
| IR-5 | A member forks a proposal's author position after another worker's results and dependent heads have been accepted. Use three tasks for the full downstream example. | Whether decisions pin the author history they depend on, so later equivocation cannot undo those decisions. Fork protection fails under the version-0 rules; version 1 implements canonical commitments, modeled in GoalLogV1 with removal/cutoff limits retained. |
| IR-6 | Screening drops a proposal referenced by a retained coordinator decision; a new replica cannot complete replay or joining. | Dependency-aware retention under bounded screening. Parameterize the current limits of 16 fork variants and 1,024 waiting events down to small values and document the correspondence. |
| IR-12 | A later submission changes the displayed result of an already accepted task. | Whether the accepted result remains the displayed result, and what subsequent submissions mean. |
| IR-13 | A coordinator fork removes newer admissions from the endpoint map, preventing those peers from receiving the halt evidence. | Which historically admitted endpoints must remain eligible to exchange evidence after a halt. Do not assume the implementation continues dialing them. |
| IR-9 | A process dies during an unflushed WAL write; reopening exposes the cached event to a peer; later power loss removes it and enables author-position reuse. | Durability requirements for recovered data before serving it or authoring from it. Process death and power loss must be distinct actions. |

After these models are stable, consider a separate `BlobRecovery` extension for
IR-7 and IR-32 through IR-34: expected length/hash, durable staged prefix,
resume offsets, promotion, failed flushes and row/file visibility. This extension
is outside the initial event/task milestone.

## Progress assumptions and model bounds

Check safety with partitions, duplicates, delayed input and modeled failures
enabled. Put liveness properties in separate configurations. State each
assumption next to the property it supports:

- Event production eventually stops for a catch-up check; retained source events
  remain available. Convergence means convergence on the retained, admissible
  history, with screening policy modeled explicitly.
- The required peers eventually stay online and network connectivity heals.
  Polling, enabled reconciliation and delivery receive fair scheduling. Derive
  peer selection from membership state rather than assuming away a missing peer.
- Joining requires a reachable coordinator that can admit the participant and
  provide the necessary history. Fork scenarios may lead to an agreed halt
  instead of successful admission.
- Task completion and acceptance require worker and coordinator actions. The
  protocol cannot promise acceptance while the coordinator remains offline or
  declines to decide. Sleep/wake is modeled as suspension and later recovery,
  without predicting wall-clock reconnect latency.

Use weak fairness for actions that remain enabled after recovery; justify any
strong fairness separately. Do not assume the desired outcome itself as a
fairness condition. See the
[TLA+ fairness tutorial](https://lamport.azurewebsites.net/tla/tutorial/session9.html).

Define progress obligations independently of the behavior under test. For IR-6,
track the transitive event dependencies named by retained coordinator decisions
and require those dependencies to become available and judged when a reachable
source retains them. Convergence on whatever screening kept is insufficient.
For IR-13, fix the expected halt recipients from admission evidence held before
fork detection, then test the later peer-selection map against that set. Do not
let dropping a peer erase its progress obligation. State the authorization limits
for serving halt evidence separately from permission to serve content keys.

Suggested starting configurations are planning choices, not checked results:

| Configuration | Starting bounds | Purpose |
|---|---|---|
| Task and session safety | One goal, one coordinator, one worker, one task, two attempts, two sessions; short bounded author histories. | Cancellation, takeover, delayed writes, retries and replay. |
| Fork and retention cases | One coordinator, two workers, up to three tasks; screening limits of two, with enough variants/waiting events to exceed them. | Reproduce IR-5, IR-6, IR-12 and IR-13. |
| Replication safety and liveness | Two replicas first; a separate three-replica case; small event sets and paginated inventories. | Partition recovery, divergent frontiers, interrupted exchange and peer selection. |
| Storage recovery | One signer and one observing peer, two author positions, bounded writes/reopens/flush failures. | Acknowledgment, exposure and IR-9. |

Record exact values in each `.cfg`. Increase one bound at a time after measuring
state growth. Use abstract IDs, lengths and logical timer states rather than
cryptographic bytes or real milliseconds. Model frontier fingerprints so equal
prefixes match and equal-length different histories differ; retain the actual
digest's golden-vector tests as separate evidence.

Symmetry reduction is allowed only for genuinely interchangeable workers in
safety checks. Keep the coordinator and special fault roles distinguished.
Disable symmetry in liveness checks, following the
[official Toolbox guidance](https://dl.tlapl.us/tlatoolbox/doc/model/model-values.html).
Review every state constraint: it can remove the behavior that would demonstrate
a bug. Distinguish expected terminal quiescence from unintended deadlock instead
of globally disabling deadlock checking. Label simulation as non-exhaustive.

## Implementation stages

The [model guide](../research/tla/README.md) documents the delivered runner and
models. The stage requirements below remain the roadmap; measured outcomes and
remaining qualifications belong to the [evidence](../research/evidence/tla/README.md).

These stages are sequential checkpoints; source fixes are separate changes with
their own review. Proposed responsibility follows the [workstream boundaries](workstreams.md):
the A orchestrator owns protocol/core/store semantics and integration; transport
and tooling reviewers contribute within their assigned paths. Assign an
independent model reviewer who also reads the implementation mappings.

### Stage 0 Establish the baseline and reproducible runner

Create a rule inventory mapping each property to contract text, source symbols,
existing tests, assumptions and known review findings. Resolve disagreements
between prose and code explicitly, including accepted-result permanence. Keep
the current-behavior baseline while proposing new rules.

Place initial tracked models under `research/tla/`, with a README indexed by
`research/README.md`. Proposed files are `GoalLog.tla`, `Sessions.tla`,
`Replication.tla`, `Durability.tla`, their named `.cfg` files, a property map and
`toolchain.json`. Keep accepted semantic decisions in `docs/`, linked to the
models and research. Do not create all four model skeletons before their stage
needs them.

Add a standard-library Python runner, `scripts/check_tla.py`, and tests under
`scripts/tests/`. Pin a supported JDK distribution/build and an immutable
`tla2tools.jar` release URL and SHA-256 in the tool manifest. Version 1.7.4 is the
adopted initial TLC pin; its
[release notes](https://github.com/tlaplus/tlaplus/releases/tag/v1.7.4)
include a fix for unsound liveness checking with multiple workers. Compute and
record the jar SHA-256 when acquiring it. Stage 0 acquired and hashed that release
and pinned Temurin 21.0.8+9 in [the manifest](../research/tla/toolchain.json).
Reevaluate the pin explicitly before adopting another release. Use one worker
for the initial liveness runs; delivered Stage 1 checks are safety only.

Keep downloaded tools, TLC state files and routine full logs under ignored
`output/tla/`. Implemented commands are `python3 scripts/check_tla.py --suite fast` and
`python3 scripts/check_tla.py --suite extended`. The runner must distinguish a
completed check, expected counterexample, unexpected violation, timeout and tool
failure. Validate those outcomes with small known-good and known-bad fixtures;
a zero exit status alone is insufficient evidence of completed checking.
Each configuration declares its exact expected outcome and properties. A
known-finding configuration requires the named violation, matching scenario and
a usable trace; unrelated errors cannot satisfy it.

**Exit:** the inventory is reviewed; a clean Mac and CI can run the same pinned
fixtures; wrong hashes, truncated runs and unexpected failures cannot pass.

### Stage 1 Model goal replay and local claims

Implement `GoalLog` and `Sessions` with atomic storage. Cover canonical
contribution order by anchor, author and sequence, missing references, coordinator
chain validity, removal/readmission, reassignment, cancellation, acceptance and
A-to-B-to-A takeover. Include duplicate requests and uncertain commit outcomes.

The historical models reproduce IR-5 and IR-12 under version-0 rules. The
separate [version-1 revision](../research/tla/version-1.md) checks the implemented
corrections, commitment conflicts and missing evidence, removal cutoffs, and
local departure. Its bounded results and coverage limits are recorded separately.
Use the goal permutation/replay tests and session lifecycle tests as the first
conformance targets. Begin trace-to-Rust mapping here, rather than postponing it
until all models exist.

Retain witness traces reaching acceptance, cancellation, takeover, removal and
fork detection. A safety invariant can hold simply because the action it guards
is unreachable. Deliberately remove a generation check in a test model and
require TLC to detect the named fencing violation. Expected-counterexample
checks must match the intended property and scenario; a parser error, unrelated
violation or timeout cannot satisfy them.

**Exit:** all declared current-rule invariants complete within their recorded
bounds; expected failures reproduce; at least one replay trace and one stale
generation trace run through Rust; required witnesses and the deliberate faulty
model behave as expected; proposed policy changes have explicit review
decisions and are not attributed to the current binary.

### Stage 2 Model reconciliation and retention

Implement suffix reconciliation and inventory fallback, including equal-length
forked histories, missing positions, pagination, duplicate delivery, exchange
interruption and restart. Add screening and membership-derived peer selection
before claiming catch-up. Include `Done` versus aborted/failed finish and retry
scheduling; abstract timer magnitudes, preserving their ordering and expiration.

Run safety configurations before adding conditional liveness. Reproduce IR-6
and IR-13, then evaluate retention and halt dissemination proposals without
assuming those corrections in the environment. Exercise two replicas before
adding the separate three-replica configuration.

**Exit:** bounded safety and fair-recovery checks complete; assumptions and
endpoint selection are independently reviewed; decision dependencies and fixed
halt-recipient obligations have explicit outcomes; an inventory-fallback trace
and an interrupted-exchange trace replay through the existing sync test kit.

### Stage 3 Refine durability and recovery

Implement `Durability` as a refinement of the atomic commit interface. Separate
process memory, OS-visible writes and durable writes. Model successful and
uncertain commits, reopening, publication of recovered events and power loss.
An unacknowledged write may survive or disappear; do not assume it always does
either. Reproduce IR-9 before evaluating recovery flush requirements.

Connect the model to existing store conformance/crash tests and the review's
flush-failure probes. A passed storage model is conditional on the modeled flush
contract; it is not evidence that a Mac's filesystem or hardware fulfills it.

**Exit:** the atomic contract and proposed recovery ordering complete their
checks under stated storage assumptions; the current-behavior model reproduces
IR-9 as an expected violation and its trace maps to a concrete recovery test.
The current binary remains unqualified for that property until a separately
reviewed Rust correction and regression establish conformance. Process-kill
evidence is labeled separately from actual power-loss evidence. Decide separately
whether to start `BlobRecovery`.

### Stage 4 Add CI and maintain implementation conformance

Add a separate formal-verification job alongside
[existing CI](../.github/workflows/ci.yml). Run the measured fast suite on model,
runner or mapped source changes; use a manually invoked or scheduled extended
suite for larger bounds. Start with a proposed ten-minute fast-suite budget and
tune it from measured results. Exceeding the budget is incomplete verification,
never a pass. Check workflow path filters include every source path in the
property map.

Translate saved TLC traces into deterministic Rust fixtures using symbolic ID
mapping and the existing `MemStore`, engine, sync and failure test kits. Construct
test signatures with deterministic test identities; use no real credentials,
tickets or participant state. Compare observable projections, event standings,
responses and peer outputs at each relevant step. A trace harness covers its
tested executions; it is not a proof of Rust refinement.

For each model-discovered failure, retain the counterexample, add a meaningful
regression or document the current-rule consequence, update the contract if its
semantics change, and rerun the affected model configurations. Changes to mapped
source require either model updates or a reviewed explanation of why its
abstraction remains valid. Commit verified logical changes promptly under
[repository rules](../AGENTS.md); apply the crate checks and workspace integration
checks those rules require.

**Exit:** CI rejects incomplete runs and unexpected counterexamples; the fast
suite and conformance fixtures pass for the same recorded source; an independent
review confirms the map covers the implemented properties. Physical qualification
and all unresolved review findings remain visible in the release ledger.

### Stage 5 Consider deductive safety proofs

After the models and desired rules stabilize, select a small safety theorem for
TLAPS: for example, local claim fencing or preservation of acceptance
preconditions across a coordinator-chain step. Prove initialization and
preservation of an inductive invariant for arbitrary parameter sizes under
explicit assumptions. Do not infer an unbounded theorem from larger TLC runs.

This is a separate follow-up milestone, not a requirement for the initial useful
TLC result. The [TLAPS documentation](https://proofs.tlapl.us/doc/web/content/Home.html)
describes support for nontrivial safety proofs and its current lack of general
temporal reasoning. General liveness proofs and proof of Rust implementation
refinement need a separate assessment and are not promised here.

**Exit:** selected theorems have complete mechanically checked proofs, documented
assumptions and no unproved obligations presented as established results.

## Evidence and completion criteria

Save durable result summaries and minimized counterexamples under
`research/evidence/tla/`, indexed by the existing research/evidence READMEs.
Each record includes source commit, dirty state and relevant path hashes,
model/config/runner/toolchain-manifest hashes, property names, constants,
assumptions, tool jar hash, JDK version, command and mode, worker count,
symmetry/constraints, fingerprint options if changed, completion status,
explored/generated state counts, memory/deadline limits, OS/architecture, elapsed
time, violation/deadlock details, action witnesses and related Rust test results.
Record a simulation seed when applicable. Link complete run output as
an artifact where available; full routine logs and state databases stay ignored.

Review three questions for every check: is the model faithful to the intended
boundary, does the property state the desired guarantee, and does the Rust code
follow the checked rules? A completed successful run establishes the configured
properties of the modeled behaviors; it does not establish that the model or
properties are adequate, or that Rust refines them. Those questions require
review and separate evidence. Avoid describing all of Locust as formally verified.

The initial project is complete when Stages 0 through 4 deliver reproducible
models, completed bounded checks, independently reviewed assumptions, retained
known counterexamples and concrete Rust conformance/regression tests. Record any
remaining failed desired properties explicitly. Publish claims at property
level, for example: “local claim fencing checked exhaustively for these bounds
and source; matching Rust regression passes.” Keep optional blob modeling,
deductive proofs and larger topology checks as separately scoped work.
