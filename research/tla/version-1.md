# Protocol/API version-1 bounded models

**Historical protocol-0/1 evidence.** These executable model revisions and
configurations were removed by the organization protocol cutover. Links below
pin their source to `28a34f0368818e373edfdcdaf9454b5f403ceb90`; current checks are described
in the [organization model map](organization.md). They do not qualify protocol 2.


Date: 2026-10-04 (work began 2026-10-03; runs use UTC timestamps).
**Status: implemented; verification results are recorded in the
[version-1 evidence](../evidence/tla/version-1.json).**
The source baseline is `15399466160d7ffc4a19de19e7af5ea7a1c3015a`.
The executed-input and Rust source hashes in the evidence identify the checked
bytes. These finite model checks do not prove the Rust implementation or
qualify transport, storage durability, or a physical multi-machine run.

The [historical GoalLog](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/research/tla/GoalLog.tla), [historical Sessions](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/research/tla/Sessions.tla),
their configurations and original evidence remain unchanged. The separate
[GoalLogV1](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/research/tla/GoalLogV1.tla) and [SessionsV1](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/research/tla/SessionsV1.tla) implement the Stage 1
rebaselining identified by the [integration assessment](upstream-impact-2026-10-04.md).
The [case registry](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/research/tla/cases.json) gives every version-1 case an explicit protocol,
API and source baseline; legacy cases inherit the historical registry baseline.
Mixed-suite reports preserve both identities. No run claims Rust refinement.

## Verified results

All 36 version-1 cases matched their registered outcomes: 22 positive checks
and 14 expected witness or mutation counterexamples. The session safety check
explored 4,944,001 distinct states; all goal checks completed on the final
model. The runner/checker suite passed 96 tests and `locust-core` passed 106.
The evidence retains an initial parser error, timeout and interrupted obsolete
run, followed by the successful final checks. These are bounded results, not
a protocol-wide correctness proof.

## Reproduce

```sh
python3 -m unittest discover -s scripts/tests
python3 scripts/check_tla.py --bootstrap --suite v1
cargo test --locked -p locust-core
```

`--bootstrap` is needed only to populate the verified private tool cache.
The session safety deadline is 1,800 seconds; the two 13-event goal cases
allow 900 seconds each. `--suite v1` runs the version-1 cases; `fast` and `extended` also retain historical
checks. The version-1 session safety exploration and the two 13-event
cancellation/removal explorations belong to `extended`, not `fast`. All cases use the existing pinned TLC/JDK, breadth-first exploration,
one worker, frozen inputs, explicit deadlines and strict outcome classification.
Witnesses and deliberate mutations must produce the registered invariant and
trace predicates; arbitrary checker failure never counts as success.

## Goal replay mapping

| Version-1 behavior | Model and corresponding Rust seam |
|---|---|
| Canonical decisions select exact member branches from the held set, independent of arrival history. | `Select` considers decisions in chain order, checks target kind, prior anchor and membership, and evaluates a tentative `ReplayWith`. This maps to [Commitments::build](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/goal/commitments.rs). |
| Selection follows predecessor and semantic dependencies transitively. | `Closure` follows member event predecessors, assignment/result/cancellation references and proposal dependencies. It stops at coordinator events, whose history is never selected through a fork. `Compatible` rejects conflicting positions and held malformed predecessor links. Missing evidence remains eligible for later delivery. |
| Only validated branches become usable. | Effective trial decisions publish pins. Pending trials record a pending decision without publishing tentative pins; excluded trials publish neither. Every earlier validated decision must stay effective. `SelectedPrefix`, `PreStanding` and `Walk` map to [scan and fold_with_commitments](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/goal/fold.rs). |
| Referenced headers can prove a decision invalid before ancestry arrives. | `ReferenceStatus` checks kind, anchor, removal and membership before accepting a pending reference. Wrong-kind and semantically invalid targets cannot resurrect a forked branch. |
| A late submission remains evidence without replacing accepted display. | `Apply` changes the displayed result only before acceptance, matching [record](https://github.com/andsav/locust.farm/blob/b758b12/crates/locust-core/src/goal/transition.rs). `AcceptedResultDisplayed` is checked throughout, and the IR-12 completion assertion requires both the accepted display and the retained late result. |
| Fork protection does not override removals. | IR-5 requires all three heads after the fork, then only the first head after the explicit empty removal cutoff. A separate nonempty-cutoff case retains the accepted branch while revoking its assignment. These map to [chain cutoffs](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/goal/chain.rs) and the corrected IR-5 Rust fixture. |
| Incremental and canonical projections agree. | `Increment` retains a direct append path with conservative guards; any held fork forces replay. `IncrementalEqualsReplay` compares it to full replay, including standings and canonical task/result order. Both paths share transition algebra, so this is not independent validation of that algebra. |

Most goal cases explore every delivery order/subset for their finite transcript,
with duplicate and terminal no-op actions. The full 21-event IR-5 case uses its
causal scripted delivery, including removal; smaller fork/conflict/dependency
cases explore arbitrary arrival. Additional conflict, dependency, canonical-order
and IR-5 configurations reverse numeric public-key ordering, including the
coordinator's relative position. This tests two orders, not all possible keys.

`AcceptedHeadsPermanent` is also checked at every reachable state of the
member-fork and conflicting-commitment families, where no removal or
coordinator halt is present. Completion assertions check the expected accepted
branch, refusal, dependency
selection, or removal result. Negated witnesses reach a preserved acceptance
under a fork, missing-predecessor pending followed by recovery, and removal after
fork protection. Disabling commitments reproduces acceptance loss; disabling
the display guard reproduces the late-submission display mismatch. These are
model mutations, not findings against the current Rust code.

The model omits retention quotas and the `required` retention set: dependencies
are modeled only for branch selection. Payload epochs, endpoint maps, invalid
cryptography, task revisions, result rejection, unlimited attempt budgets and
integer overflow remain outside this finite event vocabulary. `depends_on`
participates in dependency closure but does not become a scheduling condition.
Coordinator self-removal and arbitrary malformed transcripts are not represented.

## Sessions mapping

[SessionsV1](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/research/tla/SessionsV1.tla) preserves the historical claim/generation/commit
abstraction and adds durable local departure. Canonical membership is supplied
as an external Boolean. A worker's new claim/progress write requires active
credentials, membership, no departure and the current assignment/claim. Removing
membership, revoking credentials and leaving are distinct transitions.

The source mapping was rechecked against
[claims](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/requests/claims.rs),
[session binding](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/sessions.rs),
[member access](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/access.rs),
[signing](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/authoring.rs),
[local departure](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/requests/goals.rs),
[request dispatch](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/requests/mod.rs) and
[commit handling](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/commit.rs).
Principal-scoped keyed progress replay occurs after credential validation and
before member/claim checks. Therefore a retry after departure may return an
already committed success, but cannot sign a new event. Directed witnesses
check new-write refusal after departure and reopen, and successful replay after
departure. The original A-B-A, cancellation, principal binding and both atomic
failure witnesses and generation mutation are rerun against this revision.

The safety universe remains two principals, two sessions, two keys, generations
0–3 and at most three committed claim/progress events. `Leave` abstracts an
externally committed departure: its signed `LeaveRequested` event, request
grants and failure modes are outside this event counter and request model.
Its assignment belongs to a worker;
local departure does not model a coordinator leaving. The goal is assumed to
have a usable, non-halted history and available signing/content keys. Invitations,
read entitlements/content epochs, shutdown requests and daemon/socket cleanup
are not part of this claim/progress model. In particular, version 1 excludes
shutdown from persisted replay; the model's keyed progress requests do not claim
otherwise. Atomic storage cannot establish staging, flush or power-loss safety.

## Concrete regression correspondence

The existing [goal tests](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/goal/tests.rs) include
`tla_ir5_member_fork_preserves_the_committed_accepted_heads`,
`tla_ir12_later_submission_preserves_the_accepted_display`,
`canonical_acceptance_pins_transitive_author_ancestry_across_arrivals_and_reopen`,
`invalid_canonical_reference_does_not_resurrect_a_forked_branch`, and
`incomplete_canonical_branch_waits_then_recovers_through_a_full_variant_quota`.
The two named TLA goal fixtures retain their original symbolic transcripts.

The [session fixture](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/tests/formal.rs) replays
A-B-A, delayed writes, keyed retry and reopen. Additional
[authorization tests](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/tests/authorization.rs)
cover canonical admission, local departure, read epochs and shutdown retry;
[failure tests](https://github.com/andsav/locust.farm/blob/28a34f0368818e373edfdcdaf9454b5f403ceb90/crates/locust-core/src/node/tests/failure.rs) cover fencing
and recovery around an uncertain atomic commit. The recorded crate test run
checks these concrete behaviors. This remains hand-reviewed correspondence,
not a generic trace importer or a mechanically proved refinement relation.

Replication/retention/halt-proof delivery, storage durability and full model CI
remain the later stages in the [implementation plan](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/tla-verification-plan.md).

## Upstream integration

Merged upstream `e23df22d97e41fa2fc3eda0090f3f8d9730fe186` after the
model checks, incorporating 28 commits. The evidence records identical Git
objects for 14 modeled source areas: goal replay, session/claim authority,
signing, commit handling and the relevant event/API/storage types. New content
graph/key-cache and peer-clock behavior remains outside the assumptions above.
The original TLC source hashes and baseline are preserved; this review does not
relabel them as runs against the merged workspace.

The merged workspace passed formatting and Clippy with warnings denied; Rust
tests reported 571 passed and 11 ignored. Website lint, type checking, all 26
tests and build passed. Documentation
checks passed; the script suite ran 173 tests successfully with one skipped
production-binary qualification test. The initial script run exposed a local
Python/Expat mismatch; a command-scoped Homebrew Expat library path resolved it.
Exact commands, counts, source identities and the initial failure are retained
in the [version-1 evidence](../evidence/tla/version-1.json).
