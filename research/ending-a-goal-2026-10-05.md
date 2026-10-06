# Ending a goal and cleaning up: what exists, one design and what broke

Status: research and proposals of 2026-10-05; not accepted; nothing built. One
design round produced it. Three agents mapped the code, the public side and
other systems. One wrote a design. Two reviewers attacked it: an adversary,
and a reviewer of fit with the code and the product. Code was read at
`8a6b0db` and checked at `7bcdbbc`, which adds tests and a note only. Every
cited passage was checked again at `6b924d0`; no cited code file changed in
between except test files. Tests are named; they were found by search, not
run. Code is cited by file name and line; section 10 links each file.

Measured after this note was written, in [goal lifecycle
characterization](goal-lifecycle-characterization-2026-10-05.md) (`c9c3b2c`). Four statements below are corrected by
it and are left as written:

- A leave request is not invisible to the host. The event list and the event
  view show it. No status line, roster or pending action does, and the leaver
  keeps receiving records and new content keys until removed.
- A close on the goal scope is kept in the shared decisions and can be
  reopened; the public snapshot then says ended. Work, new tasks and
  admissions continue after it.
- Revoking the host's agent is local and lasting, with no command that
  reverses it. It is not a shared halt, and an older copy of the store still
  holds the unrevoked key.
- The publisher suspends the page on the highest consent it holds for a
  member, even one that is pending or excluded and even when it accepts. A
  replayed older consent does not suspend it.

It also confirms that a computer removed while off is refused on every dial
and never told (39 refusals in ten simulated minutes), that a fresh
invitation then answers "joined" from its stale view with no new admission,
that an agent that left cannot withdraw its page consent, and that a quiet
goal of three opens 720 exchanges in an hour.

Answered by the owner after this note was written (2026-10-05): governance is
signed by a key that does nothing else, separate from the host's working
agent; a host may name one backup host, who can take over alone, and naming
one is optional; file acceptance moves to the host's computer before a backup
host exists; and where two sound designs differ in what they ask of the
person, the one that asks less wins. The questions of section 9 are still open.

Terms. A *goal* is the shared board (a swarm or farm to people). The *host* is
the agent key that created it and signs governance (the administrator in the
code). A *record* is one signed entry in an agent's log (an event in the
code). To *dial* is to open a sync exchange with another member's daemon.
*End*: a shared signed record that the goal is over. *Leave*: one agent stops
taking part. *Delete*: one computer removes its copy. *Frontier* of an end:
for each member, the last record of theirs that counts. *Halted*: the host's
log holds two records at one position. *Anchor*: the governance record a
record was signed against. *Page*: the goal's public page at the farm service.

Decided by the owner (2026-10-05); nothing else in this note is accepted.
Design for a host that disappears, not a hostile one; hostile members are the
host's to remove. Agreement among several people is used only to replace a
host; ordinary governance is one host signature. A takeover names the last
host record its signers hold; whatever the old host signed after it is void
even if it appears later. Who may replace a host is a rule the host writes in
advance. An earlier accepted rule also holds: nothing is decided by clock,
timeout, arrival order or lowest hash; a daemon may use its own clock for its
own behaviour.

## 1. The two questions and the direct answers

**Do we allow for a host destroying its own swarm?** Today, not on purpose.
The host cannot leave or remove itself: goals.rs 263-267 and 291-295 refuse
with "the administrator cannot leave before authority handoff" and "the
administrator cannot remove itself before authority handoff", and no handoff
exists (read in the code). The nearest thing is to remove every member, turn
the page off, revoke each invitation and stop the daemon. By accident, yes:
section 2 names two accidents that end a goal for everyone.

What "destroy" can mean under the design (section 4; proposed, not built): the
host ends the goal. Nothing signed after the end counts, nobody is admitted
and automatic steps stop. The host can also have the page deleted: the farm
service's copy is the one copy a host controls. What it cannot mean: making
copies go away. Every member's daemon holds the full signed history and every
content key of its time, and a removed member keeps all it had (state.rs
18-19; the [sharing guide](../docs/guide/sharing.md): "nothing can recall
those copies"). No record or command reaches another person's copy, and the
product must not say one does.

**How do we clean up after the work is done?** Today only partial stops exist;
none is an end. The design proposes three acts: end (the host, shared, final),
leave (your agent, a shared request), delete (your computer, local). Nothing
is deleted automatically. A goal nobody ends stays open in every copy; each
computer dials less often, and each person leaves or deletes at will. Section
5 lists what the reviewers broke; section 7 gives a smaller first step.

## 2. What exists today

Unless marked, each claim was read in the code. "Inferred" marks reasoning no
test shows; "measured" names a test. The first table lists every way a goal's
activity can stop today, the second what a designer would not guess.

| Who | Command | Signed record | Effect on the shared state | Effect on that computer | What stays on disk | Read in |
| --- | --- | --- | --- | --- | --- | --- |
| The finish decider's agent | `scope close` on a task | `ScopeDecided { Close }` | That task's round is closed: a member who has seen the close cannot start a new attempt; results, reviews and picks continue; `scope reopen` undoes it | None of its own | Everything | projection.rs 432-449; closure.rs 65-72 |
| The finish decider's agent | `scope close` on scope `goal` | `ScopeDecided { Close }` | Stored in the decisions list; nothing else | On the host's computer the page snapshot reads "Ended" and the check-in stops | Everything | projection.rs 432-458; farm.rs 467-485 |
| A member's agent or person | `goal leave` | `LeaveRequested` | Its id joins `leave_requests`; the member stays a member | Signs nothing more for that agent; `status` shows "left"; syncing continues | Everything | goals.rs 260-281 |
| The host | `member remove` | `MemberRemoved`, with an empty text sealed under a fresh content key | Member removed; its later records excluded; the key number (epoch in the code) rises | Host stores the new key | Everything, on every computer, the removed one included | goals.rs 283-335; measured by test `removal_seals_new_epoch_proof_and_stops_member_writes` (lifecycle.rs) |
| The host | `workspace epoch` with a policy that has no file writes | `WorkspaceEpoch` | The shared file tree stops advancing; the chosen checkpoint is kept | None of its own | Everything | the [file-tree plan](../docs/shared-file-tree-plan.md) 272-275; the code path was not re-read |
| The host's person | `farm off` | `PublicationSet` with no visibility, then a signed delete to the service | Page visibility becomes none | Publisher record kept, flagged deleted once the service confirms | Service blanks the snapshot, keeps a tombstone row | farm.rs 746-776, 935-939 |
| The host's person | `invitation revoke` | None | None | That one invitation is stamped revoked; refused for a redeemed one | The record | invitations.rs 86-132 |
| A member's agent | `blob withdraw` | None | None | Stops serving and showing one object | The bytes, "as evidence" | content.rs 173-194 |
| Any person | `agent revoke` | None | None; the agent stays a member everywhere | Signs nothing more; for a host agent, nobody can sign governance in its goals again | The signing seed | daemon.rs 133-152 |
| Any person | Stop the daemon or uninstall | None | None | Everything stops, for every goal at once | The whole data directory | installation.rs 493-577 |
| Accident | Second host record at a used position | A fork | Governance cut there; halted for good | Host can sign nothing | Everything | history.rs 78-86; chain.rs 216-228 |

| Fact | What the code does | Read in, or measured by |
| --- | --- | --- |
| A close on the goal scope exists; only the page reads it | `scope close` on scope `goal` signs a close under the current rules record. The farm code reads it as goal state `Ended` and stops the check-in; a changed snapshot is still uploaded. The service deletes the page 30 days later; a reopen in time clears that timer. The signer must be the formation's "finish decider" and needs the local `select` permission. It is an agent tool, and the host has no right of its own to close. Only the `coordinator` preset names a finish decider, so in most goals nobody can close anything | tasks.rs 339-376, 387-388; farm.rs 467-485, 851-859; locust-farm lib.rs 34, 395-436, 836-840; fold.rs 708-715 |
| Leaving is a request nobody is shown | `leave_requests` is written and never read. The dial list ignores the local "left" flag, so the leaver's daemon keeps receiving records, content and new keys until the host removes it. After leaving, the agent cannot withdraw content: that needs a current member that has not left. It cannot decline page consent either: a decline is a signed record, and signing is refused (inferred for consent, not run) | projection.rs 298; peers.rs 179-198; access.rs 62-77; authoring.rs 127-133 |
| A removed daemon is never told and keeps dialing | Members refuse it with `NotAMember` and stop dialing it, so the removal never reaches it; the repository's own check states this. Its view still lists the members, so it dials them and retries each refusal after a wait that grows to between 30 and 60 seconds, for ever (read; no test shows the dialing) | responder.rs 116, 133-140; check_operations.py 413-415, 430; peers.rs 179-198; driver.rs 452-467 |
| A daemon keeps working for a goal nobody uses | It dials every other current member of every held goal every 30 seconds or on change, and retries a failed endpoint after 1 to 60 seconds. There is no idle slowdown, failure limit or per-goal pause. It admits any holder of a valid invitation with nobody present. It signs automatic steps when records arrive. While a page is on and not ended, it checks in with the service every 30 seconds | driver.rs 14-21, 443-468; peers.rs 304-382; flow.rs 11-48; farm.rs 854-859 |
| The store cannot delete a goal | The store trait has no delete for a record or a goal. The SQLite code has two DELETE statements, for one local record and one sealed object. A commit can list objects to drop, and no code outside tests fills that list. After a goal is over a computer still holds members' addresses, invitation records, keys, permissions, folder registrations, sessions and feed cursors for it | store.rs 131-133, 165-268; locust-store local.rs 15, objects.rs 37 |
| Two accidents already destroy a goal | (a) Two different records at one position of the host's log: the usable prefix is cut there, governance is read only from the prefix, and the goal is marked `Halt::Fork`. Nothing clears it. (b) Revoking the host's agent on its own daemon: the request checks nothing about hosted goals, and no un-revoke exists | history.rs 78-86; chain.rs 55, 216-228; daemon.rs 133-152; (a) measured by test `host_review_fork_retracts_later_governance_but_preserves_prefix_work` (goal/tests.rs) |
| Deleting a local copy and rejoining with the same key is unsafe | The next position of a key is the count of its own records held locally; nothing else remembers used positions. When sync completes first, the daemon recovers its log and signs at position 1. When membership and content arrive before its own log, it signs a different record at position 0; both daemons then hold a member fork at zero, and one round of ordinary reconciliation spreads it. That case withholds the record through the replica adapter, not over a live network. A restored host also admits an invitation holder at a used position | goal/mod.rs 253-268; the [host-key characterization](host-key-failure-characterization-2026-10-05.md), under "Cleanup: rejoining after losing the goal"; measured by tests `same_key_rejoin_recovers_own_log_before_signing_when_sync_finishes` and `same_key_rejoin_can_sign_at_zero_if_membership_and_content_arrive_before_own_log` (replica_tests.rs) and `restored_host_redeems_outstanding_invitation_before_recovery_at_an_already_used_position` (delivery.rs) |

Around the goal. Invitations live only on the host's disk, never expire unless
an expiry is given, and are never deleted (invitations.rs 19-31). Admission
checks no closure (peers.rs 304-382). The file-tree plan never deletes a
user's local data, and the API has no folder unregister. The page's upload key
exists only on the host's disk (farm.rs 23, 55, 714-716). Under a lost host
the page stays "Quiet" for ever today, and no member can take it or their name
down. So today ending a goal deletes its page and abandoning it keeps the
page; the [live demo note](../docs/live-farm-demo.md) says its ended page
stays visible. The [joinable plan](../docs/joinable-farms-plan.md) (proposed)
would delete an abandoned page after 30 days without a check-in.

## 3. What other systems do

"Source" means read on 2026-10-05: the Matrix spec, the Synapse admin docs,
the Signal Desktop string table, two Keybase client files, the Radicle
protocol guide and GitHub's page on deleted repositories. "Memory" means not
checked.

| Pattern or pitfall | System | Basis |
| --- | --- | --- |
| Three verbs kept apart: end, leave or stop, delete my copy | Signal: "End group", "Leave group", "Delete chat". Matrix: leave, forget, purge | source |
| Ending spends the identifier | Keybase: a deleted team's name can never be used again. Signal: an ended group refuses joins | source; that the delete is the last link of Keybase's signed chain is memory |
| Ending stops what honest software controls and says that history stays | Signal's confirmation says both, and that the end is permanent | source |
| Peer-held copies make the end permanent; one operator holding the only copy can make it reversible | Permanent: Signal, Keybase. Reversible: Slack, GitHub archive | source for Signal and Keybase; memory for the others |
| The person in charge must hand over or end before leaving | Signal; Discord | source for Signal; memory for Discord |
| A successor is a pointer, and members move by their own act | Matrix: a room upgrade does not carry memberships over | source |
| Abandonment is handled locally, with no shared record | Radicle: a repository nobody seeds is unreachable. Matrix: a room with no members cannot be rejoined | memory for Radicle; source for Matrix (the Synapse docs) |
| A real delete covers only copies one service holds | GitHub: local clones are retained, and the owner must get people to delete them | source |
| Pitfall: calling an end "delete" | WhatsApp | memory |
| Pitfall: an advisory end | Matrix: a tombstone only recommends raising power levels, where possible | source |
| Pitfall: authority that vanishes | Matrix: a room whose admins all left cannot be closed from inside | memory |
| Pitfall: destruction by accident as the only destruction, and local loss that makes a key sign twice | Secure Scuttlebutt: a feed restored from an old backup forks and dies, as Locust's host log does | memory |
| Pitfall: cleanup that replicates | Syncthing: a delete inside a folder that is still shared reaches every device | memory |
| Pitfall: a service picks the successor | WhatsApp names a new admin. GitHub makes an active public fork the new upstream | memory for WhatsApp; source for GitHub |
| Pitfall: personal data in the part of the record that cannot be dropped | Matrix membership events | source |
| Pitfall: treating thrown-away keys as erasure | The local-first literature | memory |
| Not found: a permanent end together with replacing the person in charge by agreement and no central orderer | None of the systems above | the survey |

## 4. The design

Everything in this section is the design agent's proposal. None of it exists,
and section 5 lists what the reviewers broke. There are three acts, each a
command only the person runs (`--owner`, plan then confirm, no agent tool).

| Command, after `locust --owner` | Who | What it would do | The plan would show | After confirm |
| --- | --- | --- | --- | --- |
| `goal end --goal G` (new) | the host | Shared and final. Freezes the board for everyone and deletes nothing anywhere. The same commit revokes pending invitations, clears the door record (the joinable plan's) and sweeps task allowances and claims | members and roles; when each member was last heard from and whether an exchange is in progress; "Anything a member signed that has not reached this computer will not count."; pending invitations; the door, if open; that the page stays up marked ended until `farm off`; "This is final." | `Ended "T". Nothing new counts. Every member keeps a copy.` |
| `goal leave --goal G --agent A` (exists; gains effects) | any member's person | The agent signs a leave request and signs nothing more. Once every local agent in the goal has left, this computer stops dialing for it and takes no new content keys. It still answers, so the host's daemon can fetch the request | the agent's standing; content it serves and consent it gave, with "A cannot withdraw these after leaving"; "This computer stops asking for updates once every agent here has left." | `A left "T". Copies already received stay with the goal.` The host's `status` shows "A asked to leave" with the remove command |
| `goal delete --goal G` (new) | anyone | Removes this computer's copy: records, objects no other held goal names, content keys, levels, allowances, folder registrations, invitation records, claims and feed cursors; never a byte inside a connected folder. Keeps a tombstone: the goal id, the local agent keys that were members (retired in that goal for good, so they cannot sign at a used position) and the page's publisher record if this computer published the page. Refused for the host of an open goal that is not halted: "End it first" | what is deleted and kept; the connected folders that are not touched; `A, B never return to "T" under these names.` | `Deleted your copy of "T". Nothing changed on anyone else's computer.` |
| `member remove`, `farm off`, `invitation revoke --all` (the last is the roles plan's, not built) | the host | Unchanged, except that after an end `farm off` sends the delete only and signs nothing | unchanged | unchanged |
| `status` | anyone | Gains "ended", "asked to leave" and idle hints with the matching command | | |

**States.** Shared, per goal, a function of held records: *open* and *ended*.
Only the governance signer in force ends a goal, by one record, and no record
moves it back. *Halted* stays a fault state: a halted goal cannot be ended,
only deleted locally. Local, per computer: *held* (a local agent is a current
member that has not left; the daemon dials, answers, serves and offers work);
*quiet* (the goal is ended and this copy complete, or every local agent left
or was removed; the daemon answers, does not dial and offers no work; the copy
stays readable); *deleted* (the tombstone only). No clock moves any state.

**The shared record.** One new governance kind, `GoalEnded { frontier }`,
signed by the host with no sealed payload. It would join the seven kinds in
`is_governance()` (event.rs 502-513). The frontier maps each current member to
the id of its last counting record as the host's computer holds it, built as
`member_remove` builds `last_accepted` (goals.rs 306-317). Later governance,
from the old host or a takeover, would be `Excluded(AfterEnd)`: the chain is
finished, not halted. A work record would count only if it is its author's
frontier record or an ancestor of it, reusing removal's cutoff walk (chain.rs
114-145, 246-294). The goal-scope close would become invalid, as it is for the
file tree (fold.rs 700-706). The end would change no content key.

Once a daemon holds the end, `Goal::next` would return nothing. The fit
reviewer found that this alone stops unattended admission (peers.rs 366-377)
and automatic steps (flow.rs 27). The daemon would also refuse redemptions,
upload one last snapshot marked `Ended`, stop dialing once its copy is
complete, and keep answering. The host's daemon would push the end to removed
endpoints over the path fork proofs use (peers.rs 92-121, 385-400). A removed
daemon that cannot check the end against its chain would use the host's
signature only to stop dialing and show "ended".

**A goal nobody ends** would stay open in the shared record for ever: no clock
may end it. The dial interval of a goal with no new record would grow from 30
seconds toward one hour, and `status` would hint at `goal end`, `goal leave`
or `goal delete`. Under the joinable plan the service would delete a
never-ended page after 30 days without a check-in. The fit reviewer corrected
this. A host computer in daily use checks in every 30 seconds for a year-old
goal, so its page reads "Receiving updates" for ever and is never deleted.
Agents at level auto (the [roles plan](../docs/roles-and-permissions-plan.md),
proposed) keep taking tasks, and an unattended host daemon keeps admitting
invitation holders. Only dialing slows, and not as written: any successful
exchange resets the interval, so a healthy idle goal stays at 30 seconds. Only
a new record should reset it.

## 5. What the reviewers broke

Each sequence is inferred from reading: the end record does not exist, so none
was run. Where a step rests on a test of today's code, the test is named. The
table after the sequences gives the reviewers' proposed fix for each break.

**1. An end that a restored backup undoes.** Host H ends goal G at position p
of its log, deletes its copy, then restores its data directory from a backup
taken before the end. The backup holds G open at p-1, its invitations not
revoked, and no tombstone. H's daemon dials, and records land batch by batch
(outbox.rs 100-139). After any commit, flow.rs signs an automatic step at p,
or an old invitation is redeemed at p with nobody present, as today's code
does (the `restored_host_redeems_...` test of section 2). Two host records now
sit at p: history.rs 83-85 cuts the prefix, chain.rs 55 no longer reads
`GoalEnded`, and 216-228 halts. Every "ended" copy becomes "halted" for good,
the frontier excludes nothing, and members other than the host can sign again
(the `host_review_fork_...` test of section 2).

**2. An end that a takeover from an earlier base undoes.** The host ends G at
p. M1 holds the end, goes quiet, sweeps its allowances and is told "final". M3
and M4, the successors the host named, were offline or restored from before
the end. They sign a takeover naming base B before p, so by the decided rule
the end is void. The takeover reaches M1 when M3 dials it, since quiet daemons
answer. On M1 the goal is open again, but M1 has no transition back from
quiet, its allowances are gone, and revoked invitations and local deletes are
not undone. The design's "an end the takeover's signers hold blocks the
takeover" is not a function of held records: no record says what a signer held
when it approved.

**3. "End, then delete" losing the end.** The design stops dialing once a
daemon holds every frontier record, which the host does as it signs. A host
that ends while the others are offline and deletes at once leaves no computer
holding the end. Members see an open goal whose host never returns and whose
host key is retired. A member record only the host held is lost the same way.
The proposed push does not exist: `HaltProof` carries two conflicting records
(sync.rs 358; peers.rs 149-166), goes only to endpoints that are not current
peers, and has no delivery mark (driver.rs 298-324, 352-359). Without one the
host's daemon would dial every endpoint it ever admitted while it keeps the
goal.

**4. The frontier un-counting work.** Ending un-counts whatever had not
reached the host's computer, on the member's own board too, and the host
cannot judge from last-heard times whether its copy is current. This reverses
the rule the code states for closing a task (closure.rs 1-2). The cutoff walk
starts at the named record and follows each earlier one; when any of them is
not held, every record of that author is pending (chain.rs 258-268, 374-380).
The end would do to every member at once what removal does to one. Example: S,
a computer that was behind, holds M1's records 0-4 and 6 (the frontier record)
but not 5. The end arrives and all of M1's records become pending. S "holds
every frontier record" and goes quiet, so record 5 never arrives and M1's
contribution is absent from S's final state. The frontier carries an id only,
where removal carries position and id (event.rs 404-408). It sits in the
signed header, capped at 16 KiB (limits.rs 11); the fit reviewer estimates a
ceiling near 245 members.

**5. Records arriving after the end, and the page.** After the end nobody can
be removed, and a quiet daemon still answers. The responder accepts records
from any current member (responder.rs 187-207), and screen.rs 23-42 keeps any
record by an author who was ever admitted. So an ended copy keeps growing. On
the host's computer a late record by a named author shows on the page as a
retraction and causes an upload (farm.rs 487-516, 851-853). Consent: M1
declines just before the end, so the decline lies outside the frontier. If it
later reaches the host, `eligible()` takes M1's latest held consent whatever
its standing (farm.rs 151-163) and fails, and 822-833 suspends the page for
good, since nobody can sign a consent that counts. If the decline never
reaches the host, M1's earlier acceptance stands and its name stays public.

**6. "Quiet" entered too early.** Holding every frontier record is not
completeness. A gap below a frontier record makes the whole author pending, as
in break 4. Keys and sealed objects are fetched only on exchanges this daemon
opens (initiator.rs 269-273, 458-473), so a quiet daemon never finishes
fetching while its `status` says complete. For the same reason the design's
"takes no new keys" change does nothing: a daemon that does not dial is never
sent a key.

**7. Deleting without leaving.** M1, active with a review owed and page
consent given, runs `goal delete`, which the design allows. Nothing is signed.
On every other computer M1 stays an active member: peers.rs 179-198 lists its
endpoint, and the driver dials it and is refused `NotAMember`, for ever. The
host then changes the page title; `eligible()` needs M1's fresh consent and
never gets it, so the page is suspended. The adversary says removing M1 lifts
this. Read for this note: only if none of M1's work still counts, because
`eligible()` also requires every author of counting work (farm.rs 124-147).
Leaving first has a gap too: the leaver's daemon stops dialing, so the leave
travels only when another daemon dials in, and never if the person deletes
right after.

**8. The tombstone and the store.** Any `Space::Goal` record creates a goal
entry at start (node/mod.rs 206-209), so a tombstone stored there brings the
deleted goal back as an empty held goal. An unknown record tag fails the open
(node/local.rs 225-226), and deleting folder, farm or last-sync records
through an ordinary commit hits the same error and marks the node failed
(commit.rs 225-233). Writing the tombstone and forgetting the goal must be one
atomic step, and a commit has no field for it (store.rs 121-134). The store
cannot find the records that name a goal: keys are opaque to it (store.rs
69-71). `farm off` looks the goal up first (farm.rs 588-591), so taking a page
down after delete needs a publisher that runs without a goal. The tombstone
lives in the data directory, so a restore removes it, as it would remove a
counter of used positions. Recovery copies of changed files sit beside each
connected folder and have no cleanup ([workspace.md](../docs/workspace.md)
84-85, 110-111).

| Break | Proposed fix | From |
| --- | --- | --- |
| 1 | An effective `GoalEnded` whose chain back to the first record is intact is the governance chain; other host records at or before it are excluded; two ends on two branches still halt. A daemon signs nothing after start until one exchange with a current member has completed | adversary |
| 1 | A daemon that has ever held a host-signed end never signs in that goal again and shows "ended, then halted". The user text says "final unless the host's log forks at or before the end" | fit |
| 2 | Exclude a takeover anchored at or after the end, or pinning an approval anchored there. Add the local transition from quiet back to held. Sweep allowances at delete, not at end. Replace "final" in the user text with the exact rule | adversary |
| 3 | The host's delete plan shows "N of M members have received the end", from the completed-exchange times the daemon already records (peers.rs 263-276), and refuses at zero. One acknowledged "host notice" message carries a single host-signed record, an end or a removal, and stops once acknowledged | fit |
| 4 | Carry position and id. Keep an author's held, unforked records below that position counting. Have `goal end --plan` ask every member's computer for news first and report who answered. Bind the plan to the governance head and the member list, not the frontier. State the member ceiling | fit |
| 5 | Exempt `PublicationConsent` from the frontier rule and the no-signing rule, or make `goal end` default to `farm off` with `--keep-page`. Refuse records beyond the frontier on arrival. Freeze the publisher after the final upload | adversary; fit |
| 6 | Complete means no missing dependency, no pending record, and no wanted key or object. Keep dialing at the slow rate until then | adversary; fit |
| 7 | `goal delete` by a current member signs `LeaveRequested` in the same transaction, or refuses. A leaver's daemon keeps dialing until the host holds the leave | adversary; fit |
| 8 | A separate space for tombstones and retired keys. Forget as a commit field. The node, not the store, lists the keys to delete. The plan lists recovery folders by path and says that no local record survives a restore | fit; adversary |
| Smaller: a removal never reaches the removed computer (section 2), so "the host removes it and invites it again" fails: a fresh invitation is refused there (invitations.rs 265-270; inferred, no two-daemon test) | Removals travel on the host notice; add the two-daemon test | fit |
| Smaller: a host whose agent was revoked, or whose log forked, can neither end nor delete an open goal | Let delete proceed after the warning | adversary |
| Smaller: in an ended goal `goal leave` cannot sign, so only delete stops this computer answering | Let leave set the local flag alone | fit |
| Smaller: a daemon that left but was not removed is still a member, yet acts on the host's bare signature | Only daemons that are no longer members do | adversary |
| Smaller: after an end `farm off` signs nothing, so members' `status` keeps showing a page; `farm on` and consent both sign, so a page must be on and consented before the end | Say both in the `goal end` plan | adversary; fit |
| Smaller: "open" is also a formation and a door state, and "Quiet" is the gallery's label for a page with no recent check-in | Say "not ended" and "idle" | fit |

## 6. How ending interacts with replacing a host

All proposed or inferred; the takeover record has no shape yet. The end would
be a host record, so the decided takeover rule would apply unchanged.

| Case | What would happen under the design |
| --- | --- |
| End at or before the base a takeover names | Every signer that holds the chain holds the end, and their daemons would refuse to sign a takeover of an ended goal. A takeover that appears anyway is governance after the end and is excluded. An ended goal cannot be taken over or continued under its id. The adversary: this holds only if "held the end" is read from anchors (break 2) |
| End after the base | The end is void like any other late host record, and the goal continues under the new host. The old host's daemon, on receiving the takeover, shows `Your end of "T" did not hold: the members replaced you as host at record X.` An old host that already deleted its copy is out, like any member who deleted |
| A host that was only asleep | With no takeover it syncs and may end. With one, `goal end` is refused and whatever its daemon signed on waking is void |
| The new host ends | It would be valid under the same rule. Two ends cannot both stand: two records by one key at one position are a fork |
| An end and a takeover from the same base | The only conflict. The takeover rule settles it from positions alone: the end lies after the base and is void |
| The page | No plan covers it. Its upload key exists only on the old host's disk, so a new host can neither update nor delete it. The old host, if it returns, still can, because the service checks only the page's key (inferred) |

## 7. The smallest first step, and what to leave unbuilt

The fit reviewer's proposed first step, worth building alone:
`locust --owner goal end` as one host governance record with no frontier. The
chain would mark the goal ended and exclude later governance. `Goal::next`
would return nothing for every key, which by itself stops signing, unattended
admission and automatic steps. The farm code would read the new state in place
of the goal-scope close, which would be removed. The same commit would revoke
pending invitations, and `status` would say "ended by the host". The catch:
records signed before a computer learned of the end still count when they
arrive, as for a task close. The board is not sealed, and a modified client
could keep adding. The reviewer holds that the owner's assumption (no hostile
host; hostile members removed first) accepts that. Two additions need no
format change: show "A asked to leave" to the host, and warn or refuse
`agent revoke` for an agent that hosts open goals. Slower dialing and
check-ins for idle goals are a separate step, and the one that helps the goal
nobody ends.

| Leave unbuilt for now | The reviewers' reason |
| --- | --- |
| `goal delete` and its tombstone | It is the largest part. The danger it guards against (a key signing twice after losing a goal) is the one a restored backup has today, measured by the tests in section 2. One guard that remembers used positions, the `SignerRecovery` state the API declares and never sets, should serve both |
| The frontier rule | Model it first in [Organization.tla](tla/Organization.tla): a late record after an end; an end that arrives before a record it names; a second host record at or before the end; an end after a member was removed and admitted again; a pick inside the frontier whose evidence lies outside it; an end against a takeover base, once the takeover record has a shape |
| Notices to removed computers | The sync message is not designed |
| Permanent ended pages | Keep the 30-day expiry until the owner answers; it is the only recourse a named contributor has |
| A new refusal kind for joins | Not needed: a refused join already stops retrying (peers.rs 277-289) |

For the full design the adversary asks first for: the fixed ended prefix with
the sync-before-sign guard, the consent exemption, the completeness
definition, delete signing a leave, the anchor rule for takeovers, and one
delivery of the end per endpoint. Then a two-daemon test per break. Its size,
from the fit review: seven of eight crates, about 40 Rust files, a new sync
message, a store delete, and protocol version 6 to 7, after which no existing
goal loads.

## 8. Not built, and why

The design leaves these out on purpose. The reasons are the design agent's.

| Left out | Reason |
| --- | --- |
| Reopen | Peer-held systems that end do so for good; a reversible end reopens "which came later" and makes local quieting and deletion unsafe |
| A member-initiated end or a vote | Ordinary governance is one host signature |
| Ending by clock, silence or inactivity | Nothing shared is decided by a clock |
| Delete-for-everyone or a "please delete" record | A promise nobody can keep |
| A new content key at the end | There is no new content to protect |
| A successor pointer, or importing one goal's history into another | Members move by their own act; a pointer would be a claim the record cannot enforce |
| Compaction or redaction inside an ended goal | It needs signatures over body hashes and a tombstone-collection rule an open membership cannot give |
| A counter of used positions in place of retiring keys on delete | A copied data directory defeats a counter. The adversary answers that a restore defeats the tombstone equally |
| A per-goal pause without leaving | Leave plus quiet covers it; a pause would be a fourth word |
| Disconnecting one folder, or deleting inside a connected folder | Locust never deletes a user's local data; an unregister is a small later addition |
| A voluntary host handoff | It is the other half of the host-replacement design |
| Purging the farm service's tombstones and receipts | The rows are the operator's; the design only says truthfully what "deleted" means there |
| Ending halted goals | Nothing can be signed in one; delete is its only exit |

## 9. Decisions for the owner

1. Should an ended page stay as a finished record, be deleted after 30 days as
   today, or be taken down by `goal end` unless `--keep-page` is given?
2. May a host delete its copy of an open, unhalted goal after a warning?
3. Should `agent revoke` be refused for an agent hosting open goals, or warn?
4. Should the end un-count work the host's computer had not received (the
   frontier), or should earlier-signed records count, as a task close does?
5. Should a member be able to withdraw page consent after the end?
6. May a daemon that is no longer a member act on the host's bare signature?
7. Is one hour an acceptable ceiling between dials for an idle goal?
8. Should a takeover anchored at or after the end, or pinning an approval that
   is, be excluded, and an end signed after a takeover's base be void?
9. Should `goal end` take an optional short public note, or stay bare?
10. Should a leaver's computer stop answering as well as dialing?
11. Should `goal delete` by a current member sign a leave, or refuse instead?
12. Is a protocol version step that stops every existing goal loading
    accepted?
13. Word choice: "not ended" rather than "open", and "idle" rather than
    "quiet"?
14. Should the host's daemon pause check-ins for a goal with no new record?

## 10. Limits

| Basis | What rests on it |
| --- | --- |
| Read | Every file in the next table, at `8a6b0db` by the code map and at `7bcdbbc` by the fit review, and each cited passage again at `6b924d0`; the plans named; six primary sources for other systems |
| Run | Nothing: no cargo, build or script, while other sessions edited the checkout |
| Measured by named tests, which this round did not run | The fork retraction, the restored host admitting at a used position, the removal proof, and the two same-key rejoin cases |
| Inferred | Every sequence in section 5, because the end record does not exist; that a removed agent on another computer cannot rejoin (invitations.rs 265-282 answers from the stale local view; single-daemon tests only); that a departed member cannot withdraw consent; that a removed daemon keeps dialing; and all of host replacement, stated against a takeover record that has no shape yet |
| Not established | What the deployed service at locust.farm retains, and its retention setting; the code path behind the `workspace epoch` row; whether deletions recorded in the shared file tree reach members' connected folders; what the network library and relays keep about a daemon after a goal is over; whether Signal's "End group", found in the desktop string table, is released, and who may use it |
| From memory | Every row marked memory in section 3 |

| Area | Files cited |
| --- | --- |
| `locust-core` goal | [chain.rs](../crates/locust-core/src/goal/chain.rs), [closure.rs](../crates/locust-core/src/goal/closure.rs), [fold.rs](../crates/locust-core/src/goal/fold.rs), [history.rs](../crates/locust-core/src/goal/history.rs), [goal/mod.rs](../crates/locust-core/src/goal/mod.rs), [projection.rs](../crates/locust-core/src/goal/projection.rs), [screen.rs](../crates/locust-core/src/goal/screen.rs), [state.rs](../crates/locust-core/src/goal/state.rs), [goal/tests.rs](../crates/locust-core/src/goal/tests.rs) |
| `locust-core` node | [access.rs](../crates/locust-core/src/node/access.rs), [authoring.rs](../crates/locust-core/src/node/authoring.rs), [commit.rs](../crates/locust-core/src/node/commit.rs), [farm.rs](../crates/locust-core/src/node/farm.rs), [flow.rs](../crates/locust-core/src/node/flow.rs), [node/local.rs](../crates/locust-core/src/node/local.rs), [node/mod.rs](../crates/locust-core/src/node/mod.rs), [peers.rs](../crates/locust-core/src/node/peers.rs), [replica_tests.rs](../crates/locust-core/src/node/replica_tests.rs), [delivery.rs](../crates/locust-core/src/node/tests/delivery.rs), [lifecycle.rs](../crates/locust-core/src/node/tests/lifecycle.rs) |
| `locust-core` node requests | [content.rs](../crates/locust-core/src/node/requests/content.rs), [daemon.rs](../crates/locust-core/src/node/requests/daemon.rs), [goals.rs](../crates/locust-core/src/node/requests/goals.rs), [invitations.rs](../crates/locust-core/src/node/requests/invitations.rs), [tasks.rs](../crates/locust-core/src/node/requests/tasks.rs) |
| `locust-core` sync | [driver.rs](../crates/locust-core/src/sync/driver.rs), [initiator.rs](../crates/locust-core/src/sync/initiator.rs), [outbox.rs](../crates/locust-core/src/sync/outbox.rs), [responder.rs](../crates/locust-core/src/sync/responder.rs) |
| `locust-proto` | [event.rs](../crates/locust-proto/src/event.rs), [limits.rs](../crates/locust-proto/src/limits.rs), [store.rs](../crates/locust-proto/src/store.rs), [sync.rs](../crates/locust-proto/src/sync.rs) |
| Other crates and scripts | [locust-store local.rs](../crates/locust-store/src/local.rs), [objects.rs](../crates/locust-store/src/objects.rs), [locust-farm lib.rs](../crates/locust-farm/src/lib.rs), [installation.rs](../crates/locust/src/installation.rs), [check_operations.py](../scripts/check_operations.py) |
