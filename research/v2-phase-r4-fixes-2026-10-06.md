# Locust v2 phase R4 review fixes

Status: implementation and verification record, 6 October 2026.

This follows the [R4 review](v2-phase-r4-review-2026-10-06.md) of `8c086c1`,
the intervening fixes `5b3fe29` and `15a8aac`, and the corrected
[roles plan](../docs/roles-and-permissions-plan.md) in `0ed4dc5`.
The 39 assigned findings are accounted for below, plus the already-fixed first
finding. The two formal-model extensions are left under the requested
small-change exception; their Rust test gaps and the property map are addressed.
No signed encoding changes. No performance improvement was measured.

The concurrent R5 work, its four owned implementation surfaces, the R6 guides,
and the protected plans and companions are outside this change. Shared files
were reread before editing; commits contain only this session's changes.

## Findings and disposition

The old paths and line numbers identify the review headings, not current lines.
Commit labels in the last column are expanded below.

| # | Review heading | Disposition and evidence | Commit |
| --- | --- | --- | --- |
| 0 | `goal/fold.rs:1103`, unheld witness anchor | **Already fixed** in `15a8aac`. Ineffective witnesses are skipped before resolving their rules; the existing hostile-anchor regression remains in the full suite. | `15a8aac` |
| 1 | `goal/flow.rs:412`, offers for finished stages | **Fixed.** Wanted offers require an open, incomplete, unselected current round. The regression covers completion, selection and closure and preserves already signed offers. | A |
| 2 | `cli/selectors.rs:113`, ambiguous member selector | **Fixed.** An exact key wins; otherwise key-prefix, signed-name and enrolled-name candidates are combined and deduplicated by key. Different matching members are listed with distinguishing prefixes and refused. | D |
| 3 | `cli/workspace.rs:551`, overwritten workspace policy | **Fixed.** Initialization preserves the formation's integrator and completion. Only a missing workspace part or explicit completion override requires rebinding. Invalid task-creator completion is refused before a plan or capture. The reviewed rules revision is carried through to the write. | E |
| 4 | `goal/fold.rs:968`, first files after a role move | **Fixed.** Rust and the site's validator refuse a role as workspace integrator, removing that moving-role path. A participant remains explicit; no encoding changes. | E |
| 5 | `goal/chain.rs:115`, missing earlier formation | **Fixed.** A role-bearing admission initializes a missing list with the host's agent, then adds the admitted member. Partial-definition and complete-definition replays agree. | A |
| 6 | `proto/event.rs:507`, oversized holder list | **Fixed.** Give/take return `limit_exceeded` with a plain explanation when the resulting list cannot fit a record. Tests cover both and atomic refusal of automatic assignment during rules bind; `--no-role` permits that binding without the oversized role change. | F |
| 7 | `goal/fold.rs:563`, evidence anchored before its subject | **Fixed.** Reviews, named checks and declarations require an anchor no earlier than the subject's, in forward, reversed and reloaded replay. | A |
| 8 | `cli/only_you.rs:1329`, reviewers after rules bind | **Fixed.** The daemon gives the counting role to current members in the same durable commit as the rules and optional workspace epoch. `--no-role` keeps holders unchanged. The plan/result name the assignment and Undo, or missing reviewers and role-give commands. Atomicity, restart, CLI and real Node/CLI tests cover both choices. | F |
| 9 | `goal/flow.rs:494`, departed request author | **Fixed.** A departed result author's review requests retain the result anchor, so later members do not create new requests that author cannot sign. Leave and removal cases still deliver review work to a newly admitted member without a stalled step. | A |
| 10 | `requests/invitations.rs:317`, waiting join name | **Fixed.** A waiting join may update its requested name before admission. Refusal is checked first; admitted names remain signed and immutable. A redeemed-ticket retry does not compare a later local spelling to the admitted name. | D |
| 11 | `requests/goals.rs:168`, unused role declaration | **Fixed.** A role unused by the new formation does not trigger a kind-change refusal. Actually changing a previously declared group into a picker/closer remains refused. | F |
| 12 | `goal/fold.rs:1091`, latest-review scans | **Fixed.** `latest_review` caches the latest effective `(sequence, id)` per author, once per subject/check per fold. A test counter checks scans, without timing claims. | C |
| 13 | `goal/chain.rs:268`, unused role-list copies | **Fixed.** The final role map is moved into state once after chain construction; the necessary per-anchor snapshots remain. | A |
| 14 | `goal/fold.rs:112`, repeated rule resolution | **Fixed.** The cache is keyed by context. Each call checks its anchor and fills roles/only-member from that snapshot. Unused role parameters were removed from resolution helpers. A counter regression checks one resolution across anchors and the distinct membership snapshots. | C |
| 15 | `goal/flow.rs:427`, two repeated scans | **Fixed.** Wanted effects read the projection's materialized-effect map; review eligibility reads its already grouped and ordered decisions. These close both costs described under this heading. | A |
| 16 | `goal/projection.rs:416`, head resolution for selected tasks | **Fixed.** Projection reads the binding at the selected decision's own anchor. It does not substitute the ordinary task projection for proof-pinned context. | C |
| 17 | `cli/only_you.rs:957`, already-member output | **Fixed.** Repeated add/join returns a no-op before confirmation, prints the existing signed name and actual level, explains immutable names, and gives a separate role command when applicable. Real Node/CLI tests verify no admission or role mutation. | D |
| 18 | `goal/chain.rs:575`, participant refusal | **Fixed.** A nonmember participant is named as the participant who picks, closes or accepts files; it no longer gets a role-holder sentence. | A |
| 19 | `cli/only_you.rs:1316`, stranded file counts | **Fixed.** Only effective, nonstale, unintegrated proposals in the current epoch count. First files instead name `workspace init`; singular and plural output are covered. | F |
| 20 | `cli/roles.rs:189`, unused-role duties | **Fixed.** An unused role says that no rule names it, instead of printing an empty duties sentence. | F |
| 21 | `PointBox.svelte:449`, rejection text | **Fixed.** Editor copy distinguishes a member's latest unpinned review from an approval pinned by an accepted decision. | E |
| 22 | `organization/explanation.rs:8`, stale role descriptions | **Fixed.** Rust, the TypeScript mirror and generated exports describe role holders, local owner-set levels, stage delivery and direct shared-tree acceptance. Role and stage field comments and the threshold correction match current behavior. | E |
| 23 | `live_farm_demo.py:289`, first-files declaration | **Fixed.** Preparation no longer declares first files. Its test rejects that obsolete command and verifies give/take for frontend, backend and reviewer roles. | B |
| 24 | `check_operations.py:218`, later file declarations | **Fixed.** Both workflows explicitly initialize shared files with contribution-author declaration completion and omit obsolete seed declarations. Later declaration steps retain the behavior those workflows test. | B |
| 25 | `requests/invitations.rs:26`, unused stored host name | **Fixed.** Removed `InviteRecord.host_name`; signed invitation host names remain. No compatibility reader was added. | D |
| 26 | `proto/invite.rs:133`, misplaced comments | **Fixed.** Signature, role, host-name, join-name and validation comments describe their actual fields and checks. The redeemed-request comment matches retry behavior. Field order and signed bytes are unchanged. | D |
| 27 | `tla/organization.md:30`, stale property map | **Fixed.** Both obsolete test names and the model header are corrected. The map now states which guarantees are assumptions or Rust-only evidence. | G |
| 28 | `goal/closure.rs:65`, lead close/reopen | **Left in the model; Rust gap fixed.** `a_new_lead_reopens_after_the_old_leads_later_log_position` checks reopened work and renewed review requests over three replay orders. Modeling closure/reopening requires new event kinds, validity and transitions, beyond a small correction. | A, G |
| 29 | `Organization.tla:307`, vacuous role coverage | **Left in the model; boundary corrected.** `RolesNeverEmpty` is identified as true by construction; the fallback witness is narrower evidence. Host-agent start, cardinality and invalid role records remain outside the transcript model. Their Rust regressions are linked, including the new `neither_of_two_lead_holders_can_pick`. Modeling them requires changing founding transcripts, validation and selection. | A, G |
| 30 | `proto/event.rs:778`, three name refusals | **Fixed.** Bad admission role names, correctly signed tickets with bad host names, and signed join requests with unusable names are rejected in tests. Frozen signed vectors are unchanged. | D |
| 31 | `tests/cli.rs:2581`, role output assertions | **Fixed.** The lead's Undo restores its old holder; earlier-rules wording, every relevant duty sentence and an unused role are asserted. | F |
| 32 | `tests/workspace.rs:567`, initial file rule | **Fixed.** The fixture uses peer-review, distinct from `Formation::default`, and asserts that exact completion and the host agent's participant key. Existing policies, explicit overrides and concurrent rebinding are tested separately. | E |
| 33 | `tests/cli.rs:2917`, rules-bind plan gaps | **Fixed.** Task-creator completion is refused before confirmation or binding. Stranded-proposal filtering, first-files advice and singular/plural counts are asserted. | F |
| 34 | `tests/cli.rs:2817`, add only exercised a plan | **Fixed.** A real Node/CLI test confirms two review-panel additions and one `--no-role` addition, checks stored holders, then checks no-op add/join output. | D |
| 35 | `node/tests/roles.rs:290`, names/roles across peers | **Fixed.** Two independent Nodes exchange admission records after a waiting-name correction, restart both stores, and retain the exact signed name and role. Same-ticket retries change neither. This is two replicas on one computer, not two physical machines. | D |
| 36 | `goal/tests.rs:2837`, signed review request | **Fixed.** The test signs a request between admission and role removal and asserts its effectiveness in all replay orders, beyond checking only wanted requests at the head. | A |
| 37 | `node/tests/roles.rs:199`, earlier lead replacement | **Fixed.** The existing test asserts the exact singleton holder list after giving the earlier-rules lead. | A |
| 38 | `goal/workspace_tests.rs:1095`, first-files assertions | **Fixed.** Tests assert the accepted first-files head, the empty-start restored epoch's first acceptance, and completion behavior after the second admission and empty restore. | A |
| 39 | `goal/delegation.rs:122`, authority inheritance | **Fixed.** Selection and finish tests reject different roles with the same holder and participant/role substitution, while accepting unchanged or removed authority. | A |

## Implementation references

- Replay and automatic work: [chain](../crates/locust-core/src/goal/chain.rs), [fold](../crates/locust-core/src/goal/fold.rs), [flow](../crates/locust-core/src/goal/flow.rs), [projection](../crates/locust-core/src/goal/projection.rs), [rules](../crates/locust-core/src/goal/rules.rs), [replay tests](../crates/locust-core/src/goal/tests.rs), [workspace tests](../crates/locust-core/src/goal/workspace_tests.rs), [delegation](../crates/locust-core/src/goal/delegation.rs).
- Host requests and replication: [goal requests](../crates/locust-core/src/node/requests/goals.rs), [invitation requests](../crates/locust-core/src/node/requests/invitations.rs), [peers](../crates/locust-core/src/node/peers.rs), [role tests](../crates/locust-core/src/node/tests/roles.rs), [replica tests](../crates/locust-core/src/node/replica_tests.rs).
- Terminal and workspace: [owner commands](../crates/locust/src/cli/only_you.rs), [selectors](../crates/locust/src/cli/selectors.rs), [roles](../crates/locust/src/cli/roles.rs), [workspace commands](../crates/locust/src/cli/workspace.rs), [CLI assertions](../crates/locust/tests/cli.rs), [workspace assertions](../crates/locust/tests/workspace.rs), [real Node/CLI tests](../crates/locust/tests/t2_flow.rs).
- Validation and copy: [Rust validation](../crates/locust-core/src/organization/validation.rs), [explanation](../crates/locust-core/src/organization/explanation.rs), [site validation](../sites/locust.farm/src/lib/formation-editor/contract/rules.ts), [site explanation](../sites/locust.farm/src/lib/formation-editor/contract/explain.ts), [editor copy](../sites/locust.farm/src/lib/formation-editor/ui/PointBox.svelte), [formation exports](../docs/reference/generated/organization.vectors.json).
- Formal boundaries and results: [property map](tla/organization.md), [model evidence](evidence/tla/organization/README.md).

## Commits

| Label | Commit | Scope |
| --- | --- | --- |
| A | `726de21` | Role authority, subject anchors, automatic work, replay assertions. |
| B | `974ea33` | Demo and operation harness completion rules. |
| C | `e801764` | Rule-resolution and latest-review caches. |
| D | `2de65f0` | Names, selectors, join retry, protocol comments and tests. |
| E | `a9992d8` | Workspace policy, validation, copy and site exports. |
| F | `ec1e078` | Atomic rules/role assignment, role size refusals and CLI assertions. |
| G | `cffd2cf` | Model header, property map and retained check. |

## Verification

Each Rust change was checked with formatting, workspace Clippy and the workspace
tests before its commit. Concurrent R5 edits and shared-target rebuilds required
isolated source snapshots and a separate Cargo target directory for later steps.
The final snapshot was compared byte-for-byte with `ec1e078`: every tracked file
matched except the three model/header/documentation corrections subsequently
committed in `cffd2cf`. An initial run against the shared target was invalidated by another
build and is not counted.

Final source checks used that snapshot, `output/r4-rules-verification`, and
`output/r4-target` (also its `target` link). The site checks used the primary
checkout's exact files committed in `a9992d8`. All Python checks used
`/opt/homebrew/bin/python3`.

| Required check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed. |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed for the R4 composition. |
| `cargo test --locked --workspace --no-fail-fast` | Passed: 1,127 tests, 0 failures, 14 ignored; serialized test scheduling. |
| `python3 scripts/check_formations.py` | Passed; six examples and all diagnostic conformance vectors verified. Generated contracts were regenerated in the relevant source commits. |
| `python3 scripts/check_documentation.py --binary target/debug/locust --timeout 60` | Passed: shared-workspace-loop, local-collaboration, private-authoring, separate-goal-export; selected binary unchanged. |
| `python3 -m unittest discover -s scripts/tests` | Passed: 300 tests, 3 skipped. |
| `python3 scripts/check_docs.py` | Passed with the new report and its index entry staged. |
| `python3 scripts/check_tla.py --suite organization` | Passed: all 42 expected outcomes; 21 safety cases, 12 witnesses, 9 deliberate mutations. Stable source hashes match `ec1e078`; [retained result](evidence/tla/organization/r4-fixes.json). |
| Site `npm run lint` | Passed. |
| Site `npm run check` | Passed: 0 errors, 0 warnings. |
| Site `npm test` | Passed: 219 tests. |
| Site `npm run build` | Passed, including 89 prerendered routes, 14 raw articles and 10 exact assets. |

An earlier policy-step run hit a reconciliation timing failure in
`a_diverged_author_log_reconciles_between_two_real_daemons`; that test passed
alone on retry, and the final full suite passed without retries. A separate
check of the combined working tree initially encountered R5's Clippy
`nonminimal_bool` warning in `node/views.rs`. After the owning session corrected
it, combined-tree formatting and Clippy passed too; this session did not edit
that surface. The full test result qualifies the committed R4 composition,
not R5's uncommitted work.

The final binary's SHA-256 was
`f1ecec8f050054931ca77f4cef5dc1d20d99ad0134129534a636a3597e9d649c`.
It was compiled before the source commit and reports `a9992d809e82-dirty`;
the source comparison above establishes its R4 composition. Supplemental
`check_operations.py --network local` results against that binary:

- `--workflow workspace` passed: seed replication, declared and accepted file
  changes, local-edit preservation, and both restarts with peer readback.
  Evidence: `output/operations/20261007T002915Z-aef0e4d6/summary.json`.
- `--workflow operations` failed its startup wait after all three daemons were
  ready and all three members converged: `workers establish their independent
  peer link: qualification wait timed out in startup`. This repeated on the
  isolated retry, before the amended workspace/declaration workflow began.
  Evidence: `output/operations/20261007T002922Z-14917447/summary.json`.
  No full three-daemon operational qualification or new baseline diagnosis is
  claimed.

No browser, deployment, real-agent or two-physical-machine claim is made by
these checks. The six cost changes are supported by code review and regression
tests, including scan/resolution counters; no speed-up was measured.

## Remaining plan corrections

No protected plan was edited. The owner of the roles plan and its companion
should make these corrections:

- Describe a context-keyed rule cache with an anchor-specific role/only-member
  overlay, and projection resolving at the selected decision's anchor.
- Name closure ordering in `closure.rs` and the projected decisions used by
  `flow.rs`, alongside selection ordering in `projection.rs`. Cite
  `a_new_lead_reopens_after_the_old_leads_later_log_position` for the close/reopen
  half of the companion's “pick or close” claim.
- State that one participant signer per workspace epoch underpins the
  once-per-empty-epoch first acceptance and competing-successor dispute rule.
- Mark `RolesNeverEmpty` as true by construction, and host-agent start,
  one-holder authority, invalid role records and close/reopen as Rust-only
  evidence, as the property map now does.
- Attribute the earlier-rules output assertion to
  `a_leads_undo_restores_its_previous_holder_and_role_duties_are_explained`,
  instead of `role_give_sends_the_holders_it_read_and_a_change_in_between_is_conflict`.

The corrected waiting-join behavior takes precedence over the review's older
test sketch requiring a name conflict: waiting names can change; signed
admitted names cannot.
