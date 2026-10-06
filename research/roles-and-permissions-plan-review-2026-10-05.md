# Roles and permissions implementation plan review — 2026-10-05

Status: review findings and recommendations, not accepted decisions or implemented
behavior. Reviewed the [implementation plan](../docs/roles-and-permissions-plan.md)
and its [companion](../docs/roles-and-permissions-plan-details.md) against source
at `711cb5e`. This was a source-and-design review; no behavior was changed and no
runtime tests were rerun. Existing tests cited below were inspected as evidence,
not executed for this review.

**Outcome: support the direction, with revisions required before full
implementation.** The separation between shared rules and local participation
levels substantially simplifies the current permissions model. Removing redundant
grants, matching local role switches, and the integrator role is a sound direction.
The remaining concerns involve lifecycle boundaries, historical roles, approval
finality, and automatic acceptance.

1. **Host recovery must precede automatic document and file acceptance.**

   The plan acknowledges that a host restored from an older backup can reuse a
   signing position, fork its history, retract admissions, and strand subsequent
   member work. Phases 8–9 increase unattended signing while deferring recovery.
   The [host-key failure characterization](host-key-failure-characterization-2026-10-05.md)
   supplies evidence and its limits; it is not a recovery design.

   Define the recovery contract before implementing those phases: how the old
   host's history becomes final, how replacement authority is established, how
   affected member histories recover, and when a restored signer may resume. This
   is an ordinary recovery concern under the stated honest-host model.

   **Requested change:** make recovery safeguards an explicit dependency of
   Phases 8–9, with tests for restore-before-sync, takeover, and the old host
   returning.

2. **The local-level descriptions and allowance lifetime need reconciliation.**

   Two promises currently exceed the proposed behavior. “Read … finishes what it
   holds” is not generally true: Phase 3 requires `ask` for publishing a result,
   while [attempt_report](../crates/locust-core/src/node/requests/claims.rs)
   requires an already-published result before reporting `Completed`. An agent
   lowered to `read` before publication cannot finish normally.

   “Allowed until the task ends” conflicts with Phase 3's allowances that are
   never removed on completion. Revision can make the task startable again under
   the retained allowance. That is standing permission for a task identity,
   rather than permission until its current lifecycle ends.

   **Requested change:** specify whether `read` permits narrowly scoped
   completion of existing attempts or only reporting and relinquishing them.
   Define whether completion consumes an allowance and whether reopening or
   revising a task requires renewed permission. Keep the three levels and one
   allowance primitive; clarify their boundaries rather than adding switches.

3. **Dynamic roles need defined behavior across formation changes.**

   Looking up role holders at each event's anchor solves an important existing
   limitation. However, tasks retain earlier rules while Phase 4's role
   administration consults the current formation. The existing
   [rule resolver](../crates/locust-core/src/goal/rules.rs) shows how task contexts
   retain their binding; the proposal changes how that binding obtains holders.

   The plan does not clearly settle what happens when an unfinished task requires
   a role that the new formation removes, or when a role used as a single decider
   becomes a multi-member role in the new formation. Can the old task's role still
   be administered, and which cardinality constraint applies?

   **Requested change:** define role lifetime, name reuse, and cardinality
   constraints across rules bindings. Add replay tests showing that unfinished
   tasks retain an administrable authority model. This is an unresolved contract,
   not a claim that the proposed implementation has already failed.

4. **Approval finality requires an explicit product decision.**

   The plan preserves the behavior where an approval continues to count after the
   same member records a rejection. The UI can therefore show that member's latest
   rejection while counting their earlier approval toward automatic acceptance.
   The current [completion predicate](../crates/locust-core/src/goal/fold.rs)
   collects qualifying positive records rather than replacing them with each
   member's latest judgment.

   This is defensible if approvals are irrevocable attestations. It is surprising
   if users understand them as current review judgments. Preventing objections
   from vetoing progress does not, by itself, require retaining superseded
   approvals. This is an intentional semantic choice requiring confirmation, not
   an accidental implementation defect.

   **Requested change:** choose between irrevocable attestations and replaceable
   judgments. If approvals remain irrevocable, explain that at approval time and
   provide a clear correction workflow. Resolve this before automatic landing.

5. **Automatic file acceptance can block behind an unusable candidate.**

   Phase 9 selects the lowest-ID due proposal before checking content completeness
   and private-path restrictions. As specified, the same blocked proposal can
   remain selected while another approved, usable proposal waits. This is a
   consequence of the proposed selection order, not a reproduced runtime failure.

   **Requested change:** select among locally acceptable candidates, or skip
   blocked candidates while reporting their status. Add a test where the lower-ID
   proposal cannot land but another proposal can.

   Also reconcile the ordering descriptions: “lowest ID among candidates held”
   and “first change to count” are different guarantees. Describe the actual
   host-observed ordering consistently.

6. **The two-agent file workflow needs qualification through composition and
   recovery.**

   Composing proposals from both members can make both ineligible to approve the
   combined result because source authors are excluded recursively. Rebasing an
   agent's own proposal onto an accepted head is different and can remain viable:
   authorship of the accepted parent is not itself inherited as source authorship.
   See `source_authors` in [workspace.rs](../crates/locust-core/src/goal/workspace.rs)
   and the existing tests
   `workspace_nested_source_authors_are_ineligible_but_independent_review_counts`
   and `workspace_parent_history_does_not_permanently_exclude_old_authors` in
   [workspace_tests.rs](../crates/locust-core/src/goal/workspace_tests.rs).

   **Requested change:** demonstrate both cases. Show the ordinary stale-proposal
   recovery path without unnecessary additional reviewers, and clearly report
   when a genuine composition needs another eligible reviewer.

**The proposed primitives are converging, but their explanations should become
more precise.**

| Primitive | Recommended meaning |
| --- | --- |
| Member | An admitted agent identity |
| Role | A named set of members referenced by rules |
| Level | An agent's local participation policy |
| Task allowance | Permission with an explicit lifetime |
| Approval | Evidence concerning one exact result |
| Accepted revision | An approved change recorded after a specific predecessor |

A role should not be explained primarily as something exactly one member holds;
reviewer roles already have multiple holders. Single-holder requirements belong
to particular decision slots.

Documents and files should share the conceptual rule that an approved revision
may advance its stream when its base is current. Their validation can differ
without introducing unrelated user-facing models. Phase 8 should also explain
why `open` and `pipeline` do not receive automatic document selection; the promise
of a usable shared plan in a role-free goal is otherwise preset-dependent.

The permissions changes reduce user-facing complexity. Historical role
eligibility and automatic revision acceptance add internal complexity. Their
value depends on coherent semantics that users rarely need to understand, not
only fewer commands or renamed types.

The review recommends retaining `peer-review` as the default, counting agent
identities explicitly, and avoiding a separate role-acceptance ceremony. Advisory
reviews under `open` would improve expressiveness: recording feedback should be
distinguishable from requiring that feedback for completion. Host-observed
ordering for competing document and file revisions is acceptable as a proposed
tradeoff, provided the contract and recovery behavior state it precisely.

**The main decisions to settle are approval finality, allowance lifetime, host
replacement, historical role administration, and competing-revision ordering.**
Naming choices and display details are secondary to these contracts.

**Qualification and document consistency also need adjustment.**

The companion retains a 12-question comprehension gate while the main plan
specifies 14. Consolidate the companion's corrections into one current
specification.

More importantly, Phase 7 qualifies the system before automatic document and file
acceptance exists. Retain intermediate qualification, but add final qualification
after Phase 9 covering competing revisions, corrected reviews, missing content,
restarts, and host recovery. Qualification before those changes does not verify
the resulting complete workflow.

**Implementation recommendation:** proceed with the permissions simplification
after resolving level lifetimes and historical role behavior. Treat recovery,
approval finality, and acceptance progress as prerequisites for automatic
document and file acceptance.
