# Goal lifecycle characterization — 2026-10-05

Status: measured current behavior, not an implementation proposal. No production
behavior changed. This tests claims previously read or inferred in
[Ending a goal](ending-a-goal-2026-10-05.md) and the
[joinable-farms rewrite contract](joinable-farms-rewrite-contract-2026-10-05.md),
using the method of the
[host-key experiments](host-key-failure-characterization-2026-10-05.md).
Source baseline: `2209b1b16dd859fe875a2f5cc7c4166bdf3d93a4`.

Here, host means the goal creator's agent signing key, called `administrator`
in the current code. A daemon's local departure or revocation is different
from a shared membership change. “Cannot sign” below means the daemon's API
cannot commit that authored record; it does not mean the signing seed was
erased or that another copy of the key is cryptographically unable to sign.

## Results

| Claim | Verdict | What the tests establish |
| --- | --- | --- |
| 1. Offline removal is not delivered; retries and stale membership prevent rejoining | Confirmed for the tested delivery order | After both remaining replicas learn the removal, the returning member receives 39 `NotAMember` refusals in ten simulated minutes. A fresh invitation returns local `Member` success without a new admission. |
| 2. A departed agent cannot withdraw publication consent | Confirmed for the local API | The owner cannot make that agent sign a withdrawal or contribution, including after restart. Its existing consent and public profile remain. |
| 3. Late consent suspends publication | More subtle than stated | The highest held consent suspends when pending or excluded, even if it accepts. Filling a predecessor gap resumes publication; replaying an older consent does not suspend it. |
| 4. An identical old request returns its stored farm receipt | Confirmed | A regenerated check-in at sequence 2 returns its old receipt after sequence 4 and a service restart. A different request at 2 or an unseen request at 3 receives conflict. |
| 5. Local host revocation ends governance for good | More subtle than stated | Restart, grants and enrollment do not undo revocation. There is no shared halt, and restoring the unrevoked local store permits governance again. |
| 6. Only the finish decider closes the goal; the shared state ignores it | More subtle than stated | Finish authority is enforced and the default formation has none. The close is retained in shared decisions and the public snapshot says `Ended`, while contributions, new tasks and admissions continue. Reopen works. |
| 7. No host-visible leave request; replication continues | Refuted as a whole | `Events` and `Event` expose the request to the host. The leaver does keep receiving records, text and a rotated key until removal. |
| 8. Idle goals exchange every 30 seconds forever | More subtle than stated | A healthy three-member goal opens 720 exchanges in a simulated hour, 120 for each ordered pair. This is finite evidence, not a bound under failures or an infinite run. |
| 9. An undelivered removal key stops every text write | More subtle than stated | A survivor holding the removal but missing its key cannot write text; the host holding the key still can. A keyless leave succeeds, and verified key delivery restores text writes. |

## 1. A removed computer returns with stale membership

Test in [sim/lifecycle_characterization.rs](../crates/locust-core/src/node/sim/lifecycle_characterization.rs):
`offline_removed_member_retries_refused_peers_and_fresh_ticket_returns_stale_membership`.

Sequence (simulator seed 8101):

1. Start three production nodes over separate memory stores. Enroll, create a
   goal, join the other two machines, and allow 60 seconds to settle.
2. Stop machine 3. Machine 1 removes its member. Run 120 seconds and assert
   machine 2 durably holds the removal before machine 3 returns.
3. Issue a fresh invitation and restart machine 3 over its unchanged store.
4. Run 600 seconds. Machine 3 opens 19 exchanges to machine 1 and 20 to
   machine 2. All 39 receive `NotAMember`. Its store lacks the removal and
   its goal status still lists three members.
5. Present the fresh invitation on machine 3. `GoalJoin` answers
   `Joined { membership: Member }` from its stale view. Run another 120
   seconds: the host's log has not grown and the host still excludes that
   member.

The join is a misleading local success, not an explicit fresh-ticket refusal.
The test establishes repeated failure of ordinary reconciliation in this
ordering, not “never” over every possible input. It does not cover another
member on the removed daemon's endpoint, an already admitted exchange,
manual record injection, historical halt-proof delivery, or clearing the
local copy. The normal peer admission and local early-return checks are in
[responder.rs](../crates/locust-core/src/sync/responder.rs) and
[invitations.rs](../crates/locust-core/src/node/requests/invitations.rs).

## 2. Leaving prevents a later consent withdrawal

Test in [farm_characterization.rs](../crates/locust-core/src/node/tests/farm_characterization.rs):
`departed_member_cannot_sign_consent_withdrawal_even_for_owner_after_restart`.

1. Create a goal and join a second locally enrolled agent. Turn publication
   on and obtain both agents' affirmative consents. The publisher emits an
   upload, whose receipt the harness acknowledges.
2. The second agent calls `GoalLeave`. Shared membership remains active and
   the shared goal can still compute that author's next position.
3. The owner requests `FarmConsent { accept: false }` for the departed agent,
   and a contribution on its behalf. Both return `Denied`; neither adds a
   record to the store.
4. Restart. The owner's withdrawal request still returns `Denied`.
   `FarmShow` remains eligible, with both public profiles, and the member's
   shared consent still says accept.

The tested restriction is local departure enforcement in
[authoring.rs](../crates/locust-core/src/node/authoring.rs), including the
owner's attempt to authorize a write. It is not removal of the key or a
shared prohibition on signatures from another copy. This test uses one
daemon with two enrolled agents; claim 7 separately measures remote
replication after leave. It exercises withdrawal and contribution, not every
API operation that reaches the common authoring check.

## 3. Which late consents suspend the publisher

Tests in [farm_characterization.rs](../crates/locust-core/src/node/tests/farm_characterization.rs):

- `later_consent_with_missing_predecessor_suspends_but_replayed_old_consent_does_not`
- `excluded_consent_arriving_after_removal_suspends_even_when_it_accepts`

**Missing predecessor:** publish with host and member consent; independently
copy the store; sign two further affirmative member consents. Restore the
copy and deliver only the second new consent through the production replica
adapter. It is `Pending(Predecessor)` and there is no administrator halt.
The publisher emits `Suspend`. Acknowledge that receipt, then deliver the
missing first consent. The latest consent becomes effective and the
publisher emits `Upload`. After acknowledging the upload, replay the old
consent: the preview remains eligible and no suspension is queued.

**Excluded after removal:** the member first publishes a contribution, so
its consent remains required even after removal. Publish with both agents'
consents and independently copy the store. Sign one more member consent,
then restore the copy and remove the member based on the older held prefix.
Before the late consent arrives, the public preview remains eligible.
Deliver that consent through the replica adapter: it is
`Excluded(PastRemoval)`, the projected effective consent still accepts, but
the publisher emits `Suspend`. Restart retries the identical suspension.
Run this sequence twice, once with a late decline and once with a late
acceptance; both suspend.

Thus arrival time alone is not the condition. The publisher inspects the
highest-sequence held consent, even when the shared effective-consent map
still contains an older acceptance
([farm.rs](../crates/locust-core/src/node/farm.rs)). A gap can be repaired.
The exclusion experiment uses removal, which exists today. The end record
and frontier proposed in the ending note do not exist, so the particular
“decline outside an ended frontier” scenario could not be run.

These tests inspect the publisher's signed requests and preview, with
harness-supplied receipts. They do not contact the deployed public page.
The existing service test `suspension_clears_snapshot_and_delete_is_permanent`
in [locust-farm/lib.rs](../crates/locust-farm/src/lib.rs) separately verifies
that applying a suspension clears the service snapshot.

## 4. Stored receipts do not prove a restored publisher is current

Test in [locust-farm/tests/lifecycle_characterization.rs](../crates/locust-farm/src/tests/lifecycle_characterization.rs):
`old_identical_check_in_returns_its_receipt_after_newer_requests_and_restart`.

1. Open the farm service over a temporary SQLite file. Apply upload sequence
   1, check-in sequence 2, then check-in sequence 4; retain receipt 2.
2. Drop and reopen the service against the same database.
3. Independently regenerate check-in 2 with the same signing key and `{}`
   body. The signed request is identical to the original.
4. Apply it. The result is exactly receipt 2, including its old stream
   version 2. The public service row remains at stream version 3.
5. At sequence 2 submit a suspension instead; at the never-used sequence 3
   submit a check-in. Each returns HTTP status `409 Conflict` from the
   service mutation handler.

This directly tests the receipt-before-sequence branch in
[lib.rs](../crates/locust-farm/src/lib.rs). It exercises the service handler
and real SQLite persistence, not an HTTP connection, deployed configuration,
or the publisher restoring a disk backup end to end. The duplicate is not
a freshness check; an old request with no matching receipt is rejected.

## 5. Revoking the host is durable locally, not a shared permanent halt

Test in [tests/lifecycle_characterization.rs](../crates/locust-core/src/node/tests/lifecycle_characterization.rs):
`revoked_host_cannot_resume_governance_through_grants_or_enrollment_but_old_store_can`.

1. Create a goal with host and member, then independently copy the store.
2. Revoke the host agent through the owner API. Both immediately and after
   restart, attempts to remove a member on behalf of the host fail.
3. Trying to grant daemon permissions to the revoked agent fails. Repeating
   its original enrollment returns `Conflict`. Setting its goal-level
   administer grant succeeds but still does not permit an invitation.
   Asking the other member to issue an invitation returns `Denied`, naming
   the host-only authority check.
4. No goal event has been added and the shared goal has no halt.
5. Restore the independent pre-revocation store. The host reconnects with
   its original credential and successfully removes the member.

No supported un-revoke request exists in the current API; the tested grants
and enrollment paths do not reverse it. “For good; nothing reverses it” is
stronger than the behavior: the flag is local persisted state, and the old
copy contains the same unrevoked key
([daemon.rs](../crates/locust-core/src/node/requests/daemon.rs)). The restore
control contains no intervening signed goal events, so it does not exercise
or recommend restoring a stale author log. This is a memory-store copy,
not a SQLite-directory restore drill, and it does not implement a takeover.

## 6. Goal-scope close is a decision, not a shutdown

Tests in [farm_characterization.rs](../crates/locust-core/src/node/tests/farm_characterization.rs):

- `default_formation_has_no_goal_finish_decider`
- `goal_close_follows_finish_role_instead_of_host_identity`
- `goal_close_is_shared_and_publicly_ended_but_does_not_stop_work_or_admission`

**Default:** create a goal with no formation override. Its resolved finish
authority is absent. Even the owner acting for the host receives `Conflict`
with “principal is not the named scope authority”; no event is committed.

**Role versus host:** use the coordinator formation, admit another member,
and bind the coordinator role to that member. The host's close is refused
with the same authority error and leaves the log unchanged. The owner
acting for the named member successfully commits an effective close.

**Effects:** with the host as coordinator, a non-decider member's close is
refused. Publish with both agents' consent, then close as coordinator. The
close is effective and appears in shared `state.decisions` under the goal's
closure scope; `FarmShow` reports `Ended`. The host then publishes a new
contribution, opens a new task, and admits another member successfully.
Obtain the newcomer's consent and reopen using the close ID as predecessor;
the public preview reports `Open` again and the goal remains unhalted.

“Shared state ignores it” is false literally: the decision is retained.
It does not end goal-wide work or admissions in the tested paths.
“Only the public page reads it” is also too broad: shared decisions and
ordinary event inspection retain it, and reopen uses it as its predecessor.
The farm snapshot interprets it as an ended *goal*, while
[projection.rs](../crates/locust-core/src/goal/projection.rs) supplies no
goal-wide closed flag. Effective authority is checked in
[fold.rs](../crates/locust-core/src/goal/fold.rs).

These tests do not enumerate every operation after close, run retention for
30 days, or prove every current or future consumer of the decision. An
unauthorized request can construct a signed event internally before commit
rejects it; the tested guarantee is no unauthorized committed close.

## 7. The host can inspect a leave; the leaver still replicates

Test in [sim/lifecycle_characterization.rs](../crates/locust-core/src/node/sim/lifecycle_characterization.rs):
`leave_is_visible_in_events_and_replication_and_rotated_keys_continue_until_removal`.

Sequence (seed 8102):

1. Settle three joined machines. Machine 2 signs `GoalLeave`; run 60 seconds.
2. On the host, `Events` lists the leave ID and `Event` returns its author
   and typed `LeaveRequested` body. The shared `leave_requests` set contains
   it, while goal status still lists three members.
3. Remove machine 3, rotating the content key to epoch 1. The host publishes
   “after leave and rotation”; run 60 seconds. Machine 2 holds that record,
   epoch-1 key, and readable text despite having left.
4. Remove machine 2, rotating to epoch 2, and publish another contribution.
   After 120 seconds machine 2 has neither that record nor the epoch-2 key.

The affirmative event inspection refutes “no view the host can see.” This
is not evidence of a dedicated pending-removal action, notification, or
roster badge in the CLI or website; those surfaces were not exercised.
The replication half of the claim is confirmed for a separate daemon with
one enrolled member, including an actual rotation after leave.

## 8. One quiet simulated hour

Test in [sim/lifecycle_characterization.rs](../crates/locust-core/src/node/sim/lifecycle_characterization.rs):
`idle_three_member_goal_keeps_exchanging_for_a_simulated_hour`.

Use seed 8103, three joined machines, true clocks and no injected faults or
stalls. Settle for 60 seconds. Record store log lengths, request count and
existing streams. Advance exactly 3,600 seconds with no local requests.
Count new streams by initiating and accepting machine, sampling each second.

| Initiator | To machine 1 | To machine 2 | To machine 3 | Total |
| --- | ---: | ---: | ---: | ---: |
| Machine 1 | — | 120 | 120 | 240 |
| Machine 2 | 120 | — | 120 | 240 |
| Machine 3 | 120 | 120 | — | 240 |
| All machines | 240 | 240 | 240 | **720** |

Every ordered pair has 30-second gaps at this sampling resolution, and
continues into the last 40 seconds of the hour. Local request count and all
three stored event counts are unchanged. These are reconciliation
**exchanges**, not necessarily fresh transport connections, and not farm
service check-ins. No agents perform work during the measured interval.

The run measures one healthy finite hour. It cannot establish “without end”
or a hard maximum of 30 seconds between real network operations. The
30-second anti-entropy policy, occupied exchanges, and endpoint failure
backoff are implemented in
[driver.rs](../crates/locust-core/src/sync/driver.rs); failures and shell
scheduling can extend intervals. No real-hour run, byte count, CPU, power,
relay cost, or many-goal scaling measurement was made.

## 9. A missing epoch key blocks the replicas that need it

Tests in [delivery_characterization.rs](../crates/locust-core/src/node/tests/delivery_characterization.rs):

- `undelivered_removal_key_blocks_survivor_text_but_host_can_write_and_survivor_can_leave`
- `verified_delivery_of_withheld_removal_key_restores_survivor_text_writes`

1. Use the existing two-node `Network`, with a third agent admitted locally
   on the host daemon. Synchronize before removal.
2. The host removes that third agent, creating epoch 1. Deliver only the
   removal record to the other daemon through its production replica
   adapter; withhold the key and sealed proof. Do not poll the network
   again. The survivor knows epoch 1 but has no epoch-1 key; the host has it.
3. Restart the survivor. Owner-authorized contribution, task-open, and
   document-revision requests all return `Unavailable` naming the missing
   content key, and leave its log unchanged.
4. With delivery still withheld, the host successfully signs a text
   contribution whose payload uses epoch 1. The survivor successfully signs
   a payload-free leave.
5. In a separate run of the same setup, first observe the survivor's text
   refusal, then stage the removal's sealed proof and offer its correct
   key through the replica adapter. Verification accepts the key. The
   survivor now commits a text contribution at epoch 1.

The common text-signing path requires the current key
([authoring.rs](../crates/locust-core/src/node/authoring.rs)). If the only
holder disappears, survivors that know the removal cannot write new text
until a key becomes available. This is not a freeze of every signature,
every replica, or the still-running host. The test fixes the record/key
interleaving directly; it does not reproduce a live Iroh disconnect at that
byte boundary, destroy the only key copy, or test every text-bearing API.

## Verification and limits

The added tests use existing engine, replica, simulator and service fixtures.
No production behavior, protocol, dependency, or timing policy changed.
Only test modules and this research index/note were added or changed.

All 13 new characterization tests passed (12 in `locust-core`, one in
`locust-farm`). To reproduce the focused experiments and simulator counts:

```sh
cargo test --locked -p locust-core lifecycle_characterization -- --nocapture
cargo test --locked -p locust-farm lifecycle_characterization -- --nocapture
```

Checks with the pinned Rust 1.96.1 toolchain:

- `cargo fmt --all --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace`: passed, 910 passed and 14 ignored across
  the reported suites. Ignored tests were not run.
- `python3 scripts/check_docs.py`: passed in both an export of the staged
  tree and the shared working tree. An earlier shared-tree invocation failed
  on another session's index link to its then-untracked review note. It
  passed after that session committed its note and index entry; neither was
  changed by this task.
- `git diff --cached --check`: passed.

Other sessions advanced HEAD through `2e83592` to `f12eec8` during
verification with separate documentation commits. The tested production
source remained unchanged from the baseline above.

The simulator runs real nodes and peer drivers with memory stores and a
simulated shell/network. It does not run Iroh, discovery, relays, SQLite,
Unix sockets, the CLI, operating-system sleep, or real scheduling. The
service receipt experiment separately uses real SQLite. No deployed farm
was contacted and no proposed end, host-replacement, cleanup or removal
notice API was fabricated. Every “forever” or “never” conclusion above is
explicitly limited to the measured sequence and the cited current rule.
