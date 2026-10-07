# Organization model, implementation and evidence map

Organization model: Phase 4 role-holder subset, protocol 7 / API 7.
Each organization case records its source baseline in [cases.json](cases.json);
the session and effect cases retain their separate protocol/API 2 baseline.
The [retained verification record](../evidence/tla/organization/README.md) records
actual source hashes, model/configuration hashes, tool identity and each outcome.
Its earlier runs predate E1 and E2; its section of 2026-10-07 records the runs
of the current model, with the E1 and E2 cases.
**These are bounded safety and reachability checks of an explicit subset, not a
proof that Rust implements the model or that arbitrary organizations are safe.**

## Organization authority and completion

[Organization.tla](Organization.tla) fixes a small authenticated transcript per
scenario, begins with nine verified founding/admission/role/rules records, and explores
every ordering and duplicate/stutter of the remaining records. Founding delivery,
arbitrary event construction, signatures and hash collisions are assumptions or
outside scope. A missing-definition scenario independently delivers the already
pinned definition. It may delay work; it may not substitute default rules.

| Property or scenario | Model rule | Current Rust implementation and regression |
| --- | --- | --- |
| Governance stays narrow | `Governance` follows the governance key's usable, correctly anchored chain. A conflicting governance record retracts its suffix. | [Chain](../../crates/locust-core/src/goal/chain.rs), [history](../../crates/locust-core/src/goal/history.rs); broader wire/governance checks remain Rust evidence. |
| The governance key is no member | Identity 0 signs every governance record and holds no admission, so `MemberAt` never names it; `GovernanceKeyIsNoMember` keeps its contribution and review (`governance-key-work`) out of `view.ordinary` in every reachable state. A fork in the host's agent's log (`host-agent-fork`, identity 5) costs no governance record: `HostAgentForkCostsGovernanceNothing`. | `authorize_base` in [Chain](../../crates/locust-core/src/goal/chain.rs); `the_governance_key_is_never_a_member_and_its_ordinary_work_is_excluded` and `a_review_fork_by_the_hosts_agent_costs_what_a_members_fork_costs` in [goal tests](../../crates/locust-core/src/goal/tests.rs). |
| Each review reads holders at its own anchor | `RolesAt` and `RoleHeldAtAnchor`; `role-change` keeps the former holder's earlier review, excludes its later one, and accepts the new holder only after the change. The deliberate current-head lookup mutation violates this invariant. | Phase 4 Rust regressions: `role_holders_are_read_at_each_events_governance_position` and `an_ex_holder_anchored_before_the_change_still_counts_and_after_it_is_excluded` in [goal tests](../../crates/locust-core/src/goal/tests.rs). |
| A later lead's valid selection is current | `Current` orders valid selections by anchor position, author, sequence and ID. `LaterLeadWins` and the `lead-change` witness keep both valid decisions and choose the later lead's in every explored delivery order. | `decisions_by_successive_authorities_follow_governance_chronology` in [goal tests](../../crates/locust-core/src/goal/tests.rs). |
| Removed holders leave a nonempty role | `RolesNeverEmpty` is true by construction; the evidence is the `role-removal` witness, which drops the lead and witnesses identity 5's valid selection after host-agent fallback. | `removal_drops_the_member_from_every_role_and_an_empty_role_falls_to_the_host` in [goal tests](../../crates/locust-core/src/goal/tests.rs). |
| Close and reopen after a change of lead | Not modeled: the transcript algebra has selections but no closure or reopening. Extending it needs new events, validity and transition rules, beyond this small correction. | Rust-only `a_new_lead_reopens_after_the_old_leads_later_log_position` in [goal tests](../../crates/locust-core/src/goal/tests.rs) covers the reopened attempt and renewed review requests across forward, reversed and reloaded replay. |
| Latest unpinned reviews and rejects | Not modeled: all review records are approvals and selection evidence is pinned. | Rust-only regressions `a_members_latest_review_counts_and_a_later_reject_withdraws_only_its_own_approval` and `a_reject_after_a_pinned_approval_leaves_the_decision_and_its_subject_selected` in [goal tests](../../crates/locust-core/src/goal/tests.rs). |
| Only-member completion | Not modeled: the founding prefix holds five members and every modeled completion needs reviews. | Rust-only regressions `the_only_members_result_counts_as_posted_and_a_second_admission_ends_that` and `nobody_but_the_host_agent_is_ever_the_only_member` in [goal tests](../../crates/locust-core/src/goal/tests.rs). |
| Opinions and admissions that grant roles | Not modeled: rules always ask for reviews and admissions carry no role. | Phase 4 Rust tests of opinion effectiveness and role-bearing admissions; no formal conformance claim. |
| Evidence anchored before its subject | Not modeled: `Valid` does not compare a review's anchor with its subject's, no transcript anchors a review before its subject, and the model has no check or declaration records; only the pick-side bound in `Decision` is modeled. | Rust-only `reviews_checks_and_declarations_cannot_use_an_anchor_before_their_subject` in [goal tests](../../crates/locust-core/src/goal/tests.rs). |
| Taskless work does not require selection | Ordinary contributions use their rules context and member author prefix; no selection is a prerequisite. `taskless` witness reaches two ordinary contributions without tasks or decisions. | `open_taskless_work_needs_no_host_decision` in [goal tests](../../crates/locust-core/src/goal/tests.rs); `open_findings_need_no_task_and_completion_does_not_create_a_selection` in [public-engine tests](../../crates/locust-core/tests/organizations.rs). |
| Exact definitions and pinned completion rules | `KnownRule`, `RuleFor`, `DistinctPinnedQuorum`; a later one-review rule cannot reinterpret an earlier two-review task. | [Rules resolution](../../crates/locust-core/src/goal/rules.rs), [definition validation](../../crates/locust-core/src/goal/mod.rs); `unknown_definition_waits_and_refresh_uses_exact_hash`, `active_round_revision_never_reinterprets_old_evidence`. |
| Approval counts distinct eligible identities for the exact subject/round | `Reviewers`, `Approved`, `ExactScope`; duplicate signatures by one reviewer cannot satisfy two identities; a subject from another task cannot be selected. | [Predicate evaluator](../../crates/locust-core/src/goal/fold.rs); `threshold_counts_distinct_non_author_principals_on_the_exact_subject`, `a_selection_cannot_substitute_another_task_or_count_unlisted_reviews`. |
| Named authority and its own stream | `Decision` validates its unpinned signer, same-scope predecessor and double-successor exclusion. | [Decision evaluator](../../crates/locust-core/src/goal/fold.rs); `same_slot_scope_authority_equivocation_is_explicitly_disputed`, `scope_equivocation_halts_only_that_stream_and_keeps_other_work`, `selection_predecessor_cannot_cross_task_scopes`. |
| Exact scope proof survives an unrelated author fork | `Proof` includes signed ancestry and typed dependencies; only its own decision can use the pins. Two scopes may accept opposite exact branches. Their combination is incompatible. | [Proof closure index](../../crates/locust-core/src/goal/commitments.rs), [scoped projection](../../crates/locust-core/src/goal/projection.rs); `accepted_fork_branch_is_readable_only_in_its_selected_scope`, `incompatible_proof_branches_dispute_only_their_scope`; [read-side regression](../../crates/locust-core/src/node/tests/content.rs). |
| Cutoffs are exact global tenure restrictions | `Admission`, `CutoffAllows`, `RetainedByCutoff`; empty cutoff excludes old evidence; readmission does not backdate it; a retained original review does not retain its sibling. | [Tenure/cutoff checks](../../crates/locust-core/src/goal/chain.rs); `removal_retains_only_exact_cutoff_ancestry_and_readmission_does_not_backdate`, `scope_proof_cannot_retain_evidence_past_the_host_cutoff`. |
| Ending a goal is terminal | `EndIsTerminal` ensures that in any held set with an end in force, no governance record after the end and no record anchored at or after it is effective. Each of the six E1 scenarios also checks its named outcome in every reachable state with `ScenarioOutcome`: records anchored before the end count when delivered after it (`end-late-record`, which also has a witness); governance signed after the end is never effective (`end-later-governance`); the end is in force exactly when no fork at or before its position is held (`end-fork-at-or-before`); a fork above the end leaves the end and everything before it in force (`end-fork-above`); work anchored at the end never counts (`end-anchored-at-end`); and after a readmission and an end, the cutoff still retains record 10, excludes its fork 14 and keeps selection 12 (`end-after-readmission`). | Goal ending in [Chain](../../crates/locust-core/src/goal/chain.rs) and [state](../../crates/locust-core/src/goal/state.rs). |
| Member leaving a goal | `leave` is an ordinary kind signed by a member. As the leave arm of `Verifier::check` in [fold.rs](../../crates/locust-core/src/goal/fold.rs) does, `Valid` accepts a leave only when it names its author's admission at its anchor; as E2 adds there, it also rejects a leave signed by the host's agent (identity 5). A leave excludes nothing by itself: the member stays, marked as left, until a removal, and the removal that names the leave as its cutoff is what stops later work counting (`CutoffAllows`), as `authorize_base` in [Chain](../../crates/locust-core/src/goal/chain.rs) does. Four E2 scenarios each check their named outcome with `ScenarioOutcome` and have a reachability witness: `organization-leave-removal` (work below the leave counts; work above it counts until the removal is held and never after); `organization-leave-fork` (identity 2 signs a second record at the position below the leave; once the removal is held, the leave and the record it builds on stay and the fork record does not); `organization-leave-readmission` (identity 2 is admitted again; its record under the first admission stays by the cutoff, and its record under the second admission, which builds on its leave, counts); and `organization-leave-host-agent` (identity 5's leave marks nobody: the leave is invalid, identity 5 stays a member and its later work counts). | Agent leaving in [Chain](../../crates/locust-core/src/goal/chain.rs), [fold](../../crates/locust-core/src/goal/fold.rs) and [flow](../../crates/locust-core/src/node/flow.rs). |

The model has 31 ordinary safety scenarios (including the six E1 goal-ending scenarios and four E2 leave scenarios). Their transcripts contain 11–20 total
records including the nine-record founding prefix. The largest scenario has
11 remaining records and 2,048 held subsets; the missing-definition case adds
one independent definition arrival. Two rule identities use review thresholds
2 and 1. The founding role events put identities 2 and 3 in `reviewer` and identity 4
in `lead`; later role records change those lists. `RolesAt` drops later-removed
holders and falls back to identity 5 when a list empties. These two modeled role
names are treated as already declared; unknown roles, arbitrary declarations,
role kind changes and admission-carried role grants are outside this subset.
The host-agent start before any role record, the one-holder requirement for a
picker, and rejection of invalid role records are also Rust-only evidence:
`rules_bound_fills_unheld_declared_roles_with_the_host_and_requires_one_holder_per_authority`,
`neither_of_two_lead_holders_can_pick` and
`role_holders_must_be_distinct_admitted_and_non_empty` in
[goal tests](../../crates/locust-core/src/goal/tests.rs). `RolesAt`'s fallback is
an assumption of this model, not independent verification of nonempty role
lists or of every holder's membership. Exercising these missing cases would
change founding transcripts, role-record validation and the selection rule;
this correction leaves those semantics unchanged.
Identity 0 is the goal's governance key: it alone
governs admission/rules/end and is not a member. Identity 5 is the host's agent,
admitted by the founding prefix's second record. These are finite
verification choices, not role, member, history or execution limits in Locust.

Nine deliberate mutations disable pins, scope isolation, cutoff enforcement,
distinct counting, exact-scope matching, named authority, rule pinning, or exact
definition availability, or read roles at the current head instead of the act's own anchor. Each must violate its named property with the registered
fault present. Seventeen negated reachability cases require their specific positive
witness, including two valid independent scope selections, rejection of a
combined incompatible proof, role replacement, later lead selection, removal
fallback, delivery after goal end (`organization-end-late-record-witness`) and
the four leave outcomes (`organization-leave-*-witness`).

### Ending a goal (E1) and unmodeled behavior

The formal model adds the governance kind `end` and the safety invariant `EndIsTerminal`.
Not modelled, and explicitly recorded here:
- The signing gate and `cut_end`, which are daemon behaviour and not replay.
- The frontier, which is not built.
- An end against a takeover base, which waits for the takeover record to have a shape.

The registry's baseline in [cases.json](cases.json) records that the model covers an
explicit subset of the organization rules at an older protocol number (`source_commit`:
`8c086c1eb72ffa5ad0572777c8944d8d66b5c7ee`, protocol 7, API 7, status:
`current-organization-role-holder-subset`); these E1 cases extend that subset and do
not move the baseline.

### Leaving a goal (E2) and unmodeled behavior

The formal model adds the ordinary kind `leave` and models four member departure
scenarios, following E2's plan text and the Rust validity rule for a leave. A
leave carries the admission it ends; no rule reads a leave other than its own
validity, so excluding a member's later work is left to the removal's cutoff.
Not modelled, and explicitly recorded here:
- The network exchange with leaver peers before signing removal, which is daemon sync behavior.
- The `Member.left` local state in the daemon.
- Leave tickets for departing members.
- Content encryption key rotation across member removal epochs.
- Re-signing removals during replay.

`ReplayMatchesHeld` checks that delivery updates the stored projection to fresh
replay. Both use the same mathematical projection algebra; this is not independent
algorithmic evidence and does not qualify Rust's closure cache or its performance.
Rust separately compares rotated/reversed batched delivery in the selected-fork
regression. The formal model deliberately has no equivalent indexing optimization.

## Local attempt sessions

[AttemptSessions.tla](AttemptSessions.tla) models local claims per attempt. It
never asserts one global worker or one attempt per task. Shared eligibility and
local admission are separate inputs. The independent-attempt witness starts
two attempts under two sessions of the same principal.

| Property | Model check | Current implementation evidence |
| --- | --- | --- |
| Principal/session binding | `ClaimHolderBound`, `NoCrossPrincipalEvents`; a session bound to another principal cannot start. | [Sessions](../../crates/locust-core/src/node/sessions.rs), [claim requests](../../crates/locust-core/src/node/requests/claims.rs); `sessions_survive_restart_drop_requires_finished_claim_and_binding_is_permanent` in [lifecycle tests](../../crates/locust-core/src/node/tests/lifecycle.rs). |
| Generation fencing survives A–B–A takeover | `GenerationsNeverDecrease`, `StaleWritesCannotAuthor`; delayed A1 cannot write through A3. Disabling the generation check must produce a stale signed event. | `takeover_a_b_a_fences_old_generation_even_when_secret_returns` in [lifecycle tests](../../crates/locust-core/src/node/tests/lifecycle.rs); [formal trace regression](../../crates/locust-core/src/node/tests/formal.rs). |
| Local admission and current attempt gate authoring | `OnlyAuthorizedWrites`; local-admission/shared-eligibility witness checks both refusals; cancellation fences a queued report. | [Attempt authoring](../../crates/locust-core/src/node/requests/claims.rs), `cancellation_requires_holder_generation_and_is_not_completion_evidence`; [independent attempts API regression](../../crates/locust-core/tests/organizations.rs). |
| Stopped cancellation ends the local attempt durably | `AcknowledgeStopped` abstracts the atomic terminal transition; the cancellation witness rejects a queued report, acknowledges stopped, then reopens with the attempt ended. | `stopped_cancellation_commits_a_terminal_report_and_retries_without_new_events` in [lifecycle tests](../../crates/locust-core/src/node/tests/lifecycle.rs) checks both signed records and reverse replay; `cancellation_acknowledgment_and_terminal_report_commit_together` in [failure tests](../../crates/locust-core/src/node/tests/failure.rs) checks failure-before and failure-after recovery. |
| Keyed retry cannot sign twice | `IdempotentRequestsAuthorOnce`, `IdempotencyMatchesEvent`; request digest varies by attempt and generation, excludes connection/session; stored replay precedes current-claim checks. | [Commit/idempotency](../../crates/locust-core/src/node/commit.rs); [formal trace regression](../../crates/locust-core/src/node/tests/formal.rs). |
| Unknown commit outcome fences later signing | `UncertainCommitFencesSigning`, `HealthyMemoryMatchesStore`; failure-before retries once after reopen, failure-after replays once. | [Node commit](../../crates/locust-core/src/node/commit.rs); `failed_commit_before_or_after_durability_fences_node_and_reopen_resolves_outcome` in [failure tests](../../crates/locust-core/src/node/tests/failure.rs). |

The general single-attempt safety case uses two principals, two sessions, two
request keys, one attempt, generations 1–3 and at most two authored events. The
general independent-attempt safety case uses the same identities/keys, two
attempts, generation 1 and at most two authored events. It isolates concurrent
attempt starts from takeover state expansion. Directed witnesses allow three
generations and three events and use the same actions as safety checking.

One principal is shared-eligible. The model aggregates local level and task
allowance admission into one Boolean; it does not enumerate those settings or
formation selectors. Membership,
current-round, end and cancellation transitions are supplied as validated external
facts. The model does not re-prove organization authority or model transport,
managed adapters, all attempt statuses, completed/uncertain cancellation outcomes or physical
process exclusivity. Local claim generations are not distributed leases.

The 2026-10-05 cancellation extension models stopped acknowledgment as one atomic
state transition. Its two signed records, result guard and pending views are Rust
test obligations, not modeled guarantees. Earlier retained model results predate
this extension; new checks must identify the amended source hashes.
The [2026-10-05 session checks](../evidence/agent-lifecycle-2026-10-05.json)
matched all 11 expected outcomes: two completed safety cases, eight reachability
witnesses and one deliberate generation-check mutation. The stopped witness
reached an ended attempt after reopening; its initial comment-parse failure was
corrected before these checks.

## Daemon effects and delivery

[FlowEffects.tla](FlowEffects.tla) uses two configured round identities and three
valid signatures. Signatures 1 and 2 carry different witnesses for the same logical
effect; signature 3 has a different round. The logical identity tuple includes
round, transition, trigger, target and action. Cryptographic encoding is abstract.
The [frozen Rust vectors](../../crates/locust-proto/src/vectors.rs) separately check
that witness choice does not change identity while target/round changes do.

| Property | Model check | Current implementation evidence |
| --- | --- | --- |
| One logical task from duplicate signatures | `LogicalTaskDeduplication`; duplicate witness reaches three signatures and two tasks; disabling deduplication must fail. | [Effect resolver](../../crates/locust-core/src/goal/flow.rs), [projection](../../crates/locust-core/src/goal/projection.rs); `daemon_effects_advance_configured_stages_and_deduplicate_logical_work`. |
| Evidence triggers daemon materialization | `Drive` materializes from verified evidence without a poll or start action; the acknowledgment witness includes this path. | [Daemon effects](../../crates/locust-core/src/node/flow.rs); `pipeline_materializes_without_any_setting` in [public-engine tests](../../crates/locust-core/tests/organizations.rs). |
| Durable materialization reconstructs outbox | `OutboxHasDurableSource`, `HealthyMemoryMatchesStore`; failure-before and failure-after witnesses recover one task/outbox identity. | [Daemon effects](../../crates/locust-core/src/node/flow.rs), [atomic commit](../../crates/locust-core/src/node/commit.rs). |
| Delivery, acknowledgment and start are distinct | `OnlyPermittedStarts`, `AcknowledgmentHasMaterialization`; delivery plus acknowledgment does not start without local admission; a mutation that makes acknowledgment start fails. | `only_delivery_recipients_can_acknowledge_and_ack_does_not_start_work` in [goal tests](../../crates/locust-core/src/goal/tests.rs), [local attempt claims](../../crates/locust-core/src/node/requests/claims.rs). |
| Retracted evidence stops outstanding effect authority | Retraction witness keeps immutable signature, empties active outbox and starts no attempt. | `effect_cannot_change_recipients_and_fork_retraction_removes_outbox_projection` in [goal tests](../../crates/locust-core/src/goal/tests.rs). |

Configured signer/action/recipient validity is assumed in this effects subset;
the Rust evaluator checks it separately. Acknowledgment represents a valid signed
recipient action, not evidence that a physical recipient has executed the task.
There is no fairness assumption, eventual-delivery theorem, process supervisor,
network retry timing or power-loss proof. The model's task collection is an
abstract projection atomically corresponding to durable signatures, not a claim
about a particular SQLite schema.

## What remains outside these checks

See the [formal plan](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/tla-verification-plan.md) for expanded signed-body
generation, document and closure streams, full selectors, transport/retention,
fair scheduling, storage refinement and implementation correspondence. Model
state counts, witness traces, passing Rust tests and native/end-to-end qualification
are different evidence boundaries. None may be substituted for another.
