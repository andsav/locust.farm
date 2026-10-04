# TLA+ property and implementation map

The current version-1 mapping is in [Protocol/API version-1 bounded models](version-1.md).
The inventory below preserves the version-0 baseline; its tables are historical.

Date: 2026-10-03; integration update 2026-10-04. **Status: historical version-0
Stage 0/1 inventory. Upstream protocol/API version 1 changes the implementation
and required the separate [version-1 revision](version-1.md).** Initial baseline
`86980594acee708b2f9bf303afc9ee636dc55337`. This maps the
[verification plan](../../docs/tla-verification-plan.md) to current code,
[GoalLog](GoalLog.tla), [Sessions](Sessions.tla) and their configurations.
The rule descriptions are source-based observations. Run identities, completion,
state counts and test outcomes belong to the separately recorded evidence;
this inventory does not certify a run or prove implementation refinement.

The [upstream impact assessment](upstream-impact-2026-10-04.md) supersedes any
reading of the table below as a description of current version-1 behavior.
The table preserves the model's version-0 mapping; local code links now open the
merged implementation, while the recorded source hashes/commits identify the
historical source. In version 1, canonical commitments can retain selected
member branches, and late submissions preserve the accepted display. The Rust
IR-5/IR-12 transcript tests have been adapted to those corrected outcomes; the
old function names/outcomes below describe the historical fixtures. No
version-1 TLA+ qualification is claimed.

## Goal replay and task authority

| Rule or desired property | Current implementation and tests | Model boundary or decision |
|---|---|---|
| State and standings depend on held events, not arrival order. Contributions sort by anchor, author and sequence. | [History](../../crates/locust-core/src/goal/history.rs), [fold](../../crates/locust-core/src/goal/fold.rs), [append](../../crates/locust-core/src/goal/append.rs); `arrival_permutations_append_and_restart_match_full_replay` in [goal tests](../../crates/locust-core/src/goal/tests.rs). | `IncrementalEqualsReplay` compares the maintained incremental projection with independently constructed author-prefix/canonical replay, including task/result order and standings. Structural append guards update the existing state directly; other arrivals refold. Both paths share `Apply`, so equality does not independently validate its transition algebra. |
| Only coordinator decisions advance the decision chain; a broken anchor or coordinator fork halts it. | [Chain](../../crates/locust-core/src/goal/chain.rs), [standings](../../crates/locust-core/src/goal/standing.rs), [fold](../../crates/locust-core/src/goal/fold.rs). | `CoordinatorAuthority`, [coordinator-fork](configs/goal-coordinator-fork.cfg), [broken-anchor](configs/goal-broken-anchor.cfg) and [authority-anchor](configs/goal-authority-anchor.cfg). No coordinator is known before genesis arrives. Member forks exclude that author's suffix and do not imply a coordinator halt. |
| Missing predecessors or referenced contributions defer judgment. Unknown anchors gate later contributions; regressed anchors grant nothing. | `AuthorLog::insert` in [history](../../crates/locust-core/src/goal/history.rs), `Trail::place`, `reference` and `scan` in [fold](../../crates/locust-core/src/goal/fold.rs). | Arbitrary delivery subsets create missing predecessors/references. A held excluded reference refuses a decision, which still advances the applied head; an absent applicable reference stalls it. Membership/removal/regression exclusions are applied when replay reaches the contribution's anchor, not prematurely past a stalled decision. |
| Assignments advance attempt counts within the budget and have one current assignment per replayed task. | `assign` in [transitions](../../crates/locust-core/src/goal/transition.rs); `decline_reassignment_rejection_and_attempt_budget_are_enforced` in [lifecycle tests](../../crates/locust-core/src/node/tests/lifecycle.rs). | `AssignmentAttempts`; [cancel-reassign](configs/goal-cancel-reassign.cfg) includes attempts 1 and 2 and a refused attempt 3 against budget 2 before final acceptance. Task `depends_on` and `deadline_ms` remain unenforced metadata. |
| Cancellation and supersession prevent subsequent finalization; acceptance refers to an undecided result of the correct current assignment and matching base when a head is advanced. | `decidable`, `accept`, `cancel` and `record` in [transitions](../../crates/locust-core/src/goal/transition.rs); `cancellation_requires_holder_generation_and_cannot_finalize` in [lifecycle tests](../../crates/locust-core/src/node/tests/lifecycle.rs). | `AcceptedPreconditions` checks result/current-assignment/verdict/cancellation consistency; `Apply` checks the base against the latest accepted head. [Cancel-accept](configs/goal-cancel-accept.cfg) retains a late signed result as evidence but refuses acceptance of the cancelled assignment. Historical acceptance is not required to vanish solely because a later removal revokes its assignment. |
| Removal fences open assignments from that membership tenure, including after readmission. | `past_removal` in [chain](../../crates/locust-core/src/goal/chain.rs); `removal_permanently_fences_open_assignment_even_after_readmission` in [goal tests](../../crates/locust-core/src/goal/tests.rs). | `RemovalFencesAssignments`; [removal-readmission](configs/goal-removal-readmission.cfg) computes tenure effects across the coordinator chain, retroactively excludes earlier-tenure work with an absent cutoff, and permanently revokes the old assignment. Nonempty cutoff variants are not covered by the initial configurations. |
| Accepted heads should survive a member's later fork. | [IR-5](../t1-candidate-independent-review.md), [fold](../../crates/locust-core/src/goal/fold.rs), and `tla_ir5_member_fork_rolls_back_dependent_accepted_heads` in [goal tests](../../crates/locust-core/src/goal/tests.rs). | Desired `AcceptedHeadsPermanent` remains false. The [IR-5 configuration](configs/goal-ir5.cfg) preserves three accepted heads before event 20 forks the middle proposal and leaves one head. [IR-5 current](configs/goal-ir5-current.cfg) continues through event 21's removal, which does not repair the rollback. Pinning decision-dependent history remains a proposal. |
| An accepted task should continue displaying its accepted result. | [IR-12](../t1-candidate-independent-review.md), `record` in [transitions](../../crates/locust-core/src/goal/transition.rs), and `tla_ir12_later_submission_changes_display_without_changing_acceptance` in [goal tests](../../crates/locust-core/src/goal/tests.rs). | Desired `AcceptedResultDisplayed` remains false in the [IR-12 configuration](configs/goal-ir12.cfg): event 9 becomes the displayed result while result 7 remains accepted and head 8 remains unchanged. [IR-12 current](configs/goal-ir12-current.cfg) checks current-rule invariants through the same schedule. Local API refusal does not prevent receipt of this signed event. Correction remains undecided. |

## Local sessions and durable requests

| Rule | Current implementation and tests | Model boundary |
|---|---|---|
| Session binding belongs to one principal and a claim belongs to one local session. | [Sessions implementation](../../crates/locust-core/src/node/sessions.rs), [callers](../../crates/locust-core/src/node/callers.rs), [claim requests](../../crates/locust-core/src/node/requests/claims.rs); `sessions_survive_restart_drop_requires_finished_claim_and_binding_is_permanent` in [lifecycle tests](../../crates/locust-core/src/node/tests/lifecycle.rs). | `ClaimHolderBound`, `NoCrossPrincipalEvents`; two principals and two sessions on one daemon. The [principal witness](configs/session-principal-witness.cfg) binds A to another principal, then rejects the assignee's claim through it. No distributed lease or uniquely running physical executor is modeled. |
| Authorization/grants and current assignment checks precede authoring; takeover raises the generation and stale writes fail. | [Access](../../crates/locust-core/src/node/access.rs), [claim requests](../../crates/locust-core/src/node/requests/claims.rs); `takeover_a_b_a_fences_old_generation_even_when_secret_returns` in [lifecycle tests](../../crates/locust-core/src/node/tests/lifecycle.rs), and `tla_sessions_aba_delayed_write_and_idempotent_retry_survive_reopen` in [formal trace tests](../../crates/locust-core/src/node/tests/formal.rs). | `GenerationsNeverDecrease`, `StaleWritesCannotAuthor`, `OnlyCurrentAuthorizedWrites`. The [A-B-A witness](configs/session-aba-witness.cfg) delays A1 across B2 and A3, then refuses it. The [deliberate mutation](configs/session-generation-mutation.cfg) disables the generation check and must violate `StaleWritesCannotAuthor`; it is a faulty model, not a product finding. |
| Identical keyed requests return the original response without signing twice; a changed digest under that key fails. | `replayed`, `request_digest`, `remember` in [commit](../../crates/locust-core/src/node/commit.rs); dispatch in [requests](../../crates/locust-core/src/node/requests/mod.rs); [formal trace tests](../../crates/locust-core/src/node/tests/formal.rs). | `IdempotentRequestsAuthorOnce`, `IdempotencyMatchesEvent`. Keys are principal-scoped; generation represents the varied digest field, while session is excluded. Active credential resolution precedes replay; stored replay precedes membership/current-claim checks. The [restart replay witness](configs/session-restart-replay-witness.cfg) returns old success after takeover and reopen without signing again. Arbitrary serialized request digests are not modeled. |
| A successful transaction binds events, local records and the idempotency response atomically. An uncertain failed commit fences further requests until reopen. | [Commit](../../crates/locust-core/src/node/commit.rs), [authoring](../../crates/locust-core/src/node/authoring.rs), [failure tests](../../crates/locust-core/src/node/tests/failure.rs); storage seam in [Store](../../crates/locust-proto/src/store.rs). | `UncertainCommitFencesSigning`, `HealthyMemoryMatchesStore`. Atomic failure leaves all or none of a transaction in `db`; memory stays unchanged and requests are fenced until reopen. [Failure-before](configs/session-failure-before-witness.cfg) retries by authoring once after reopen; [failure-after](configs/session-failure-after-witness.cfg) replays the stored response. WAL, page cache, flush ordering and power loss are absent. |

## Exact finite coverage

Every GoalLog configuration includes `TypeOK`, `IncrementalEqualsReplay`,
`CoordinatorAuthority`, `AssignmentAttempts`, `AcceptedPreconditions` and
`RemovalFencesAssignments`. Unscripted safety configurations explore all held
subsets and permitted arrival orders of their immutable authenticated event
universe. Scripted current-rule counterparts, findings and witnesses follow
their declared numeric-prefix schedules; duplicate delivery is an explicit no-op. No arbitrary body generator,
symmetry reduction or state constraint is used. The coordinator key is 0;
represented author order is `0 < 1 < 2`. This represents one ordering of those
roles rather than enumerating every possible public-key ordering.
The synthetic coordinator always sorts before the workers. Relative
public-key order permutations are not a checked parameter of these cases.

`AcceptedPreconditions` is a partial post-state consistency invariant, not an
independent theorem of every acceptance precondition at decision time. The
latest-result, live-assignment and base-matching rules are implemented in
`Apply`/`Decidable`, shared by both replay paths. IR-12 demonstrates why the
post-state displayed result need not equal the accepted result under the
current rules. Passing replay equality cannot independently validate those
shared rules.

| GoalLog configuration | Universe and delivery boundary |
|---|---|
| [Safety](configs/goal-safety.cfg) and [broken anchor](configs/goal-broken-anchor.cfg) | 8 events, one coordinator and one worker, one task and one assignment; the final acceptance's anchor distinguishes the cases. |
| [Coordinator fork](configs/goal-coordinator-fork.cfg) and [member fork](configs/goal-member-fork.cfg) | 9 events each. Coordinator fork conflicts at sequence 2 between worker admissions; member fork conflicts with the worker's sequence-0 take. |
| [Authority/anchors](configs/goal-authority-anchor.cfg) | 10 events; a worker-authored decision, contribution anchor regression, unknown anchor 99 and a later gated contribution. |
| [Canonical order](configs/goal-canonical-order.cfg) | 10 events and two task proposals sharing an anchor; coordinator/worker contributions also share a later anchor. Task and result sequences are observed. |
| [Cancelled acceptance](configs/goal-cancel-accept.cfg) | 10 events; cancelled current assignment receives another signed result, which the coordinator cannot accept. |
| [Cancellation/reassignment](configs/goal-cancel-reassign.cfg) | 13 events, one task, two valid attempts and a refused third attempt against budget 2; cancellation acknowledgment and final acceptance are present. |
| [Removal/readmission](configs/goal-removal-readmission.cfg) | 13 events; absent removal cutoff, two membership tenures, permanently revoked old assignment and a new attempt. |
| [IR-12](configs/goal-ir12.cfg) and [current-rule counterpart](configs/goal-ir12-current.cfg) | 9 events delivered in numeric order; desired display-binding failure is expected at event 9. |
| [IR-5](configs/goal-ir5.cfg) and [current-rule counterpart](configs/goal-ir5-current.cfg) | 21 events, coordinator and two workers, three tasks/assignments and three accepted heads; numeric delivery preserves the causal prefix. Expected rollback is at event 20; the counterpart continues through removal 21. |

All represented task budgets are 2. The largest represented author sequence is
12, in IR-5. Contribution bodies are proposal, take, submit, cancellation
acknowledgment and no-op note. Decision bodies are genesis, admission, removal,
assignment, cancellation and acceptance. Decline, failure, progress, rejection,
nonempty removal cutoffs, unlimited budgets, integer overflow, cancellation
outcome variants, document contents, payload epochs and endpoint maps are
outside these initial transcript families. Their omission is a coverage limit,
not evidence that their implementation is correct.

The negated reachability invariants [NoAcceptance](configs/goal-witness-acceptance.cfg),
[NoCancellation](configs/goal-witness-cancellation.cfg),
[NoRemoval](configs/goal-witness-removal.cfg),
[NoForkDetection](configs/goal-witness-fork.cfg) and
[NoAppendContribution](configs/goal-witness-append.cfg) intentionally fail when
their action becomes observable. These scripted witnesses prevent reporting
safety solely for an unreachable path. They are not failed product guarantees.

[Session safety](configs/session-safety.cfg) uses exactly two principal values,
two session values, one assignment/assignee, two principal-scoped idempotency
keys, generations 0 through 3 and at most three committed events. The first
claim is generation 1. Claim, takeover, delayed progress requests, binding,
grants/owner authorization, revocation, cancellation, assignment end,
acceptance/supersession flags, atomic commit outcomes and reopen are explored.
Assignment changes are abstract inputs; this model does not replay goal events.
Diagnostics are quotiented out in safety mode, while pending requests and store
state remain represented. Directed witnesses use the same action definitions
under schedules of at most nine steps. `CheckGeneration = FALSE` is confined to
the deliberately faulty mutation configuration; current-rule cases use `TRUE`.

All session safety properties listed above plus `TypeOK` are declared in the
safety configuration. A witness requires its exact named negated invariant and
scenario trace; an unrelated violation cannot satisfy it. A duplicate or
quiescent self-loop can make a state non-deadlocked without producing progress.
None of these configurations declares fairness or a liveness property.

## Concrete trace correspondence and its limits

The [goal tests](../../crates/locust-core/src/goal/tests.rs) construct real
signed events with deterministic test identities and synthetic head values.
IR-12 maps model events 1 through 9 and compares scrambled/duplicate arrival,
incremental replay, full fold and `MemStore` reopen. IR-5 maps events 1 through
21. The original version-0 fixture checked fork rollback followed by removal.
The merged version-1 fixture checks that the fork preserves all three heads,
then distinguishes their withdrawal under the final explicit empty removal
cutoff. Reverse-order replay agrees. The IR-12 fixture now preserves the accepted
display while retaining the late submission. These corrected Rust executions
do not establish conformance with the unchanged historical model.

The new [session trace test](../../crates/locust-core/src/node/tests/formal.rs)
executes A1-B2-A3 through `Engine`, refuses delayed A1/B2 writes, preserves a
keyed success across takeover/reopen and checks that retries do not add events.
It combines the delayed-generation and restart/idempotency schedules rather
than serving as a general TLC-trace interpreter. Real signatures, API responses
and observable state are checked for these hand-mapped executions. This is
trace correspondence, not a mechanically established refinement relation or a
verification of all Rust executions.

## Assumptions and deferred properties

Cryptographic verification, collision-free IDs and authenticated endpoint
binding are assumptions. A permitted author may equivocate. Model IDs and
payload references are synthetic; source bytes, golden vectors and signatures
remain covered by the protocol crate's own tests.

`GoalLog` and `Sessions` assume atomic storage and correct authentication at
their input seams. The goal model deliberately shares its transition algebra
between incremental and canonical paths; the two paths differ in placement and
replay construction. Neither a matching projection nor the concrete Rust traces
prove that every modeled transition matches Rust. Source review, adequate
properties and implementation correspondence remain separate obligations.
The session model checks local authoring permission at processing time; it does
not retract a contribution already signed or prevent two physical executors
from running. No model here proves catch-up or task completion.

IR-6 dependency retention and IR-13 halt dissemination belong to Stage 2.
IR-9 page-cache exposure and power loss belong to Stage 3. The four later blob
findings are also deferred. The [independent review](../t1-candidate-independent-review.md)
retains those failures; atomic storage or a finite delivery universe must not be
used to claim they are fixed.

Original Stage 1 decisions: preserve version-0 runtime behavior, reproduce IR-5
and IR-12, and record acceptance permanence/display binding as unsatisfied goals.
Upstream version 1 subsequently adopted runtime corrections. This historical inventory supplied no corrected protocol model. The separate
[version-1 revision](version-1.md) now checks the updated bounded rules; neither
revision proves the Rust implementation.
