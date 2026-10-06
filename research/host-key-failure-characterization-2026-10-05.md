# Host-key failure characterization — 2026-10-05

Status: current-behavior experiments, not a recovery design. No production
behavior changed. These tests address the assumptions in
[Design still open and Phase 9](../docs/roles-and-permissions-plan.md) and the
[operations guide's untested restore warning](../docs/guide/operations.md).
Production source baseline: `8a6b0dbd13c139f83c8299441b41d20bc0e8f94f`.

“Host” here means the goal creator's agent signing key, called `administrator`
in the code, not the daemon's transport endpoint key. Log positions belong to
one author **in one goal**. A fork requires two different event IDs at one
position; receiving or reproducing the identical signed event is not a fork.

## Results

| Claim | Verdict | Observed qualification |
| --- | --- | --- |
| 1. Any host event fork retracts later governance and admissions | Confirmed for the tested non-governance event types | The halt stops host governance/signing; unaffected members' work at the surviving head can remain effective. |
| 2. An ancestor anchored to retracted governance poisons future effective work | Confirmed for descendants in that goal | Three successive descendants anchored to the surviving head stay `Pending(Anchor)`. This is not a ban on that key in every goal. |
| 3. Signing from an old backup before recovering later events forks | More subtle | A different event at the reused position forks. Identical deterministic stage replay does not. Recovery first extends normally; a locally visible gap or fork blocks signing. |
| 4. Host integrator: same log position halts governance; distinct positions dispute files | Confirmed, with the halt qualification from claim 1 | Both branches use the same workspace predecessor. Only the same author-log position creates an administrator fork. |
| Cleanup: losing a goal and rejoining with the same key restarts at zero or recovers first? | Ordering dependent | Completed sync recovers the log. Membership and content without the author's old log permit a new event at zero. There is no complete-history signing barrier in the tested path. |

## 1. A review fork retracts admissions

Test in [goal/tests.rs](../crates/locust-core/src/goal/tests.rs):
`host_review_fork_retracts_later_governance_but_preserves_prefix_work`.

Sequence:

1. Host genesis at position 0; admissions of host and three workers at 1–4;
   initial rules at 5.
2. A worker publishes an effective contribution anchored to rules 5.
3. Host records an effective review at its position 6, also anchored to 5.
4. Host admits two more members at 7 and 8. Both admissions are effective.
5. Deliver a differently signed review header at host position 6 (same body
   and predecessor, different timestamp).
6. The previously evaluated goal rebuilds to governance head 5, reports
   `Halt::Fork` at 6, excludes both admissions as `AfterHalt`, and removes
   both late members from the projected member map. A fresh fold agrees.
7. The earlier contribution remains effective. An unaffected worker publishes
   new work at the surviving head and it is effective. `next(host)` is absent.

The non-governance review is enough: [AuthorLog::insert](../crates/locust-core/src/goal/history.rs)
cuts the usable prefix by author position before governance classification;
[Chain::build](../crates/locust-core/src/goal/chain.rs) reads governance only
inside that prefix and reports the administrator fork. Additional tests below
exercise host contribution and acceptance forks. We did not enumerate every
`Body` variant. “The whole goal halts” must not mean all past and future work
by every other author is ineffective.

## 2. Retracted anchors remain in author ancestry

Test in [goal/tests.rs](../crates/locust-core/src/goal/tests.rs):
`retracted_anchor_keeps_same_goal_author_descendants_pending_even_at_surviving_head`.

Sequence:

1. Use the same setup through host position 5. Host publishes at 6 and admits
   a new member at 7.
2. An already admitted worker publishes its first event at worker position 0,
   anchored to admission 7. It is initially effective.
3. Fork host position 6. Admission 7 disappears from the governance chain;
   the worker event becomes `Pending(Anchor)`.
4. Append worker positions 1, 2, and 3, each naming its actual preceding
   worker event but anchoring directly to surviving rules 5. Each stays
   `Pending(Anchor)`, despite the worker still being a member and its author
   log having no structural gap or fork.
5. A different worker with clean ancestry publishes at rules 5 successfully.
   Incremental re-evaluation and a fresh fold agree.

The test signs these events with the fold fixture. `Goal::next` still offers
positions for that worker; it does not validate semantic ancestry. The
[authorization walk](../crates/locust-core/src/goal/chain.rs) reaches the old
anchor on an ancestor before it can authorize a descendant. The
[node commit path](../crates/locust-core/src/node/commit.rs) additionally rejects
locally authored transactions whose events would not be effective; the test
does not claim that a public API call commits these pending descendants.

The finite test verifies three descendants, not an infinite execution. The
reason the conclusion extends to further ordinary descendants is the
unchanged full-ancestry walk and the absence of the retracted anchor from the
chain. It says nothing about a future takeover/reset design, replacing the
key, or work in another goal. Rewriting the author's own earlier branch is
not exercised here.

## 3. Restore ordering, admission, automatic stages, and guards

Tests in [node/tests/delivery.rs](../crates/locust-core/src/node/tests/delivery.rs)
use the existing two-node `Network`: real `Node` and peer driver, encoded
transport frames, in-memory stores, and explicitly controlled delivery. The
snapshot helper creates an independent store, not another `MemStore::reopen`
handle. It copies local records, logs, and directly named held content; these
fixtures have no staged or unreferenced content.

### Agent work before or after peer recovery

`restored_host_signing_before_peer_recovery_forks_but_recovery_first_extends`:

1. Two nodes share a goal and its rules. Copy the host store at next position N.
2. Host publishes A at N; the peer durably receives A.
3. Restore the old host store and restart the nodes. The peer retains A;
   the host does not.
4. In the first ordering, request a different host contribution B before any
   peer polling. B uses N and A's predecessor. Reconciliation brings both
   nodes to the same fork at N and both report the administrator halt. A
   further host request returns `Unavailable`.
5. In the second ordering, poll recovery first and assert that the host holds
   A before making the request. B uses N+1 and `prev=A`; both nodes remain
   unhalted after synchronization.

### Outstanding invitation before recovery

`restored_host_redeems_outstanding_invitation_before_recovery_at_an_already_used_position`:

1. Host issues an unused invitation and the backup includes it.
2. Host publishes A at N; the existing peer receives A.
3. Restore the backup. A third principal, enrolled on the peer daemon,
   presents an authenticated `JoinRequest` for the outstanding invitation.
   Invoke the production `Host::join` handler before peer recovery input.
4. Automatic admission succeeds at N and the host temporarily sees the new
   member. No local request from the host agent was needed to sign it.
5. Poll the encoded network. Both nodes detect the host fork, and neither
   retains the new principal as a member.

This isolates the join-handler-before-recovery ordering. It does not measure
which Iroh stream wins a real scheduling race. The admission checks in
[plan_join](../crates/locust-core/src/node/peers.rs) validate the invitation,
identity, endpoint and local grants, then use ordinary authoring; they do not
first retrieve the administrator's remote suffix.

### Automatic stage replay can be byte-identical

`restored_host_automatic_stage_replay_is_identical_unless_another_event_used_its_position`:

1. Copy a goal with a desired pipeline effect and the host's flow grant off.
2. Enable the grant. The production flow driver signs the materialization.
   Synchronize it to the peer, restore the earlier copy, and enable the grant
   before peer recovery again.
3. Without intervening host work in the original run, the regenerated event
   has exactly the original ID. Both replicas remain unhalted after sync.
4. Repeat, but in the original run publish host work at N before enabling
   flow, so the original materialization uses N+1. After restore, enabling
   flow without recovering that work signs materialization at N. It differs
   from the old work at N, and both replicas halt after sync.

[drive_flow](../crates/locust-core/src/node/flow.rs) uses timestamp zero and
deterministic effect input. Thus “anything makes it sign” is too strong if
it means every repeated signature necessarily creates a distinct record.
This test triggers automatic flow through a grant change, not a crash between
the grant commit and startup's flow pass.

### The guards are local

`restored_host_known_gap_blocks_signing_until_missing_predecessor_arrives`:

1. Copy the host at N; then create events A at N and B at N+1.
2. Restore and receive only B through the replica adapter.
3. `next(host)` is absent and a granted contribution request returns
   `Unavailable`; a held gap blocks signing.
4. Receive A. A new contribution uses N+2, names B as predecessor, and no
   administrator halt exists.

[Goal::next](../crates/locust-core/src/goal/mod.rs) rejects an author's known
fork or waiting suffix, and rejects administrator signing at a known
administrator halt. [next_place](../crates/locust-core/src/node/authoring.rs)
requires local membership and not having left. None of those conditions
distinguishes a complete-looking old prefix from a genuinely current one.
The tests establish the missing freshness barrier; they do not claim the
absence of ordinary authorization, content-key, storage-failure, or
idempotency guards.

### Actual stopped-directory restore

Test in [daemon/durable_tests.rs](../crates/locust/src/daemon/durable_tests.rs):
`sqlite_older_directory_signs_at_used_host_position_unless_later_events_are_recovered_first`.

1. Start the production daemon with real SQLite, a Unix socket and a local
   Iroh endpoint; enroll the host, create a goal and grant work permissions.
2. Stop it and copy the whole data directory's regular files and directories,
   including identity and database data, omitting runtime sockets.
3. Restart the original, open task A, stop, and retain its persisted events.
4. Start the older copy with the same key and request task B via Unix socket.
   B is different from A but uses A's log position and predecessor.
5. Stop, add the original events to that SQLite store, and restart. Status
   reports a halt and another task request returns `Unavailable`.
6. Repeat with the original events committed into the backup before starting
   the restored daemon. B extends A at N+1. Another restart remains unhalted
   and permits a further task.

This is an actual directory-copy/daemon-signing experiment. The returned
suffix is installed through `Store::commit` while stopped, not transported
by live Iroh in this test. Actual peer reconciliation is covered separately
by `Network`; we do not claim a two-machine restore drill or crash-consistent
backup procedure.

## 4. Host key as file integrator

Tests in [workspace_tests.rs](../crates/locust-core/src/goal/workspace_tests.rs):

- `host_integrator_acceptances_at_same_log_position_halt_governance`
- `host_integrator_acceptances_at_distinct_log_positions_dispute_only_workspace`

Both bind the host key as integrator, establish an unseeded workspace epoch,
and obtain two distinct proposals and independent qualifying reviews. The
host's first acceptance is effective. Both competing acceptances name the
same workspace predecessor, `None` (the initial workspace position).

In the first test the second acceptance is signed at the first acceptance's
author-log position. A later host admission is excluded, the new member is
absent, `next(host)` is absent, and the workspace has no selected head.

In the second test the second acceptance is the next author-log event and
names the first acceptance in its **author** predecessor, while still naming
`None` as its **workspace decision** predecessor. Both acceptances are
`Disputed`, the workspace head retracts, but the later admission remains
effective and there is no administrator halt.

Both tests also check rotated/reversed arrival orders, repeated batches, and
store reload. The fold tests deliberately create conflicting signed records;
they do not claim a single up-to-date public accept request bypasses its
compare-and-swap checks. “Only files” here means this workspace selection
conflict does not halt governance; consumers requiring a selected file head
may still wait. The same-position case has claim 1's qualification: it does
not globally invalidate clean members' work at surviving anchors.

## Cleanup: rejoining after losing the goal

Tests in [node/replica_tests.rs](../crates/locust-core/src/node/replica_tests.rs):

- `same_key_rejoin_recovers_own_log_before_signing_when_sync_finishes`
- `same_key_rejoin_can_sign_at_zero_if_membership_and_content_arrive_before_own_log`

Common sequence: snapshot the member daemon after enrollment but before it
has the goal; join it; publish member event A at position 0; synchronize A to
the host; restore the member's pre-goal snapshot. The enrolled signing key
and transport identity survive, but the goal and its local records are gone.
Reuse the original redeemed invitation on the same endpoint. Immediately
after `GoalJoin`, there is no governance head and `next(member)` is absent.

**Completed synchronization:** let the normal multi-node reconciliation
finish. Assert A is recovered before publishing B. B uses position 1 with
`prev=A`, and neither node sees a member fork or administrator halt.

**Partial recovery:** through the production replica adapter, deliver all
host events, directly named content, and the verified epoch key, while
withholding A. The daemon sees the principal as a member and a normal
contribution request succeeds. B has position 0 and no predecessor, with a
different ID from A. The missing author log is interpreted as an unused log,
not as history that must be recovered before signing.

One round of ordinary reconciliation then carries both records to both
daemons. Each answers the other's frontier with an inventory of the member's
log, pushes the record the other lacks and requests the one it lacks. No
exchange is refused, and both nodes retain a member fork at zero, with no
administrator halt. The test asserts this without handing either record over
directly.

An earlier version of this note said these rounds emitted
`Refused(ProtocolError)` and did not converge. That was the test's delivery
loop, not the protocol. A responder refuses any frame other than `Done` or
a refusal that arrives while it still owes part of an answer
([responder.rs](../crates/locust-core/src/sync/responder.rs)), and the
transport must hold the next frame until `peer_readable` allows it
([engine.rs](../crates/locust-proto/src/engine.rs)). The daemon does:
`read_frames` in [network.rs](../crates/locust/src/daemon/network.rs) waits
for the worker's acknowledgement of each frame, and the worker withholds it
until the exchange is readable
([worker.rs](../crates/locust/src/daemon/worker.rs), test
`request_acknowledgement_waits_for_the_whole_response_to_drain`). The loop in
replica_tests.rs delivered each frame as it was produced, without asking
whether the receiver could read it. When two logs diverge, the
initiator pushes its record as soon as the inventory arrives, while the
responder still owes its closing frontier. The loop delivered that push early,
the responder refused it, and the exchange ended before either record crossed.
A frame trace showed the push sent while the receiver was not readable, in
both directions and in every round. The loop now holds a frame while its
receiver is not readable, as the `Network` harness, the simulator and
sync/tests/net.rs already did. With that one change the fork spreads by
itself and the rest of the locust-core suite passes unchanged.

That the daemon's transport never delivers a frame early is now observed
between two production daemons over loopback Iroh, in
`a_diverged_author_log_reconciles_between_two_real_daemons`
([daemon/reconcile_tests.rs](../crates/locust/src/daemon/reconcile_tests.rs)):
a member daemon restored from an older copy of its directory signs at the
position the host already holds a record for, pushes its record while the
host still owes its closing frontier, and the host's shell delivers that push
only after the frontier, so both daemons end holding both records, each
reported as pending, the member's daemon signs nothing more, and no exchange
is refused as a protocol error. The test intervenes twice. The restored
daemon opens no exchange until it has signed, and the host's engine is paused
while it owes the frontier, because over loopback the answer otherwise drains
before the push arrives. Without the pause, a worker changed to acknowledge
every frame at once still converged with no refusal in 20 of 20 runs; with
it, that worker failed 10 of 10 with `Refused(ProtocolError)` from the host.
Two independent readers had tried to find a path that delivers early and
found none. They noted two limits, both now covered. The daemon and the test
loops enforce the rule differently: the daemon stops reading after each frame
until the exchange is readable, and the loops check readability before each
delivery. These agree
only while nothing but a received frame can make a responder owe an answer,
which `only_its_own_next_frame_makes_a_readable_accepted_exchange_stop_reading`
([sync/tests/driver.rs](../crates/locust-core/src/sync/tests/driver.rs)) now
states. And no daemon-level test ran a diverged log through `read_frames`,
the worker and a real node together; the test above does. Its divergence
comes from a copy taken after joining. We have not
reproduced this partial-arrival/signing interleaving over live Iroh. The
experiment proves the authoring/ingestion hazard and absence of a barrier at
that state, not that every normal small rejoin naturally reaches that
interleaving.

This does not test rejoining under a changed endpoint, a fresh invitation,
removed membership, or a future cleanup API. It restores an enrolled member
with the original redeemed ticket. Losing the host's whole goal and trying
to invite itself back is not covered. A cleanup design must specify which
identity, invitation, membership and recovery ordering it preserves; the
present code cannot be described as always resetting or always recovering
before signing.

## Verification and limits

The tests use the repository-pinned Rust 1.96.1. All 11 added tests passed.
Completed checks:

- `cargo fmt --all --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace`: passed (895 passed,
  14 ignored across reported suites; ignored tests were not run).
- `python3 scripts/check_docs.py`: passed with the note and index staged.
- `git diff --cached --check`: passed.

After the change to the test loop described under the cleanup section, the
same four checks were run again and passed, with the same counts. After the
two tests named under the cleanup section were added, they passed again, with
897 passed and 14 ignored.

No production behavior was changed. The tests are named for the observed
behavior so a deliberate redesign must update them.

Not tested: physical computer loss, power interruption during copying, live
multi-machine backup restore, every possible scheduling interleaving, every
host event body, startup's grant-commit/materialization crash window, changed
identity or endpoint recovery, and proposed host replacement or goal-ending
semantics. An infinite descendant sequence is a source-backed consequence,
not something a finite regression test executes.
