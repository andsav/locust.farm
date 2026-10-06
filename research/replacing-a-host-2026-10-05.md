# Replacing a host that is gone: groundwork, four designs and what survived

Status: research and proposals of 2026-10-05. Nothing here is accepted and
nothing was built. The only decisions are the owner's, quoted in section 1.
The material was produced in one day. Four groundwork agents covered the
governance record, the fault model, classical consensus and peer-to-peer
governance. Four design agents then worked in parallel, and each design was
attacked by a safety reviewer ("attack" below) and a code-and-product reviewer
("fit" below). One fit review names commit `7bcdbbc` as the code it read,
another says that commit landed while it was reading, and the fault groundwork
names `cfb5b45`. The other sources read the live checkout without naming a
commit. Other sessions were editing the checkout, so line numbers in the
source material drift and are not repeated here. Tests are named where they
exist. No design agent or reviewer ran anything. Every counterexample below is
a sequence traced by hand against a design's rules and the code as read.

Answered by the owner after this note was written (2026-10-05): governance is
signed by a key that does nothing else, separate from the host's working
agent; a host may name one backup host, who can take over alone, and naming
one is optional; file acceptance moves to the host's computer before a backup
host exists; and where two sound designs differ in what they ask of the
person, the one that asks less wins. Questions in the last sections that
these answer are left as written.

## 1. The question and the fault model

A goal's governance is signed by one key, the creator's agent. Governance is
who is a member, the rules, task rounds, the tree epoch and the public page
policy. That key is hashed into the goal's identifier. If the computer holding
it is gone, nobody can ever join or be removed and the rules never change. The
question is what record and what rule let another computer continue the same
goal, and what that costs.

The owner's answers (2026-10-05), in the words given to the design round. The
first two and the rule in the third are also recorded under "Design still
open" in [the roles plan](../docs/roles-and-permissions-plan.md); the fourth
is under "Decisions this plan assumes" there.

- "The failure to design for is a host that disappears. A hostile host is not
  assumed. Hostile members are the host's to remove."
- "Agreement among several people is used only when a host is replaced.
  Ordinary governance stays one signature by the host with no waiting." The
  price is accepted: "a takeover fixes the last host record its signers hold,
  and whatever the old host signed after it is void even if it appears later."
  The plan adds: "A host that was only asleep can return to find its last
  admissions undone."
- Who may replace a host "is a rule the host writes in advance", with the hint
  "maybe that's the modular piece: people named in advance, vote, etc".
- "There is no integrator role; accepting file changes is automatic" (Phase 9
  of the plan).
- From earlier accepted rules: nothing is decided by clock, timeout, arrival
  order or lowest hash ([formations.md](../docs/formations.md): "Clocks and
  arrival order never decide anything").

The fault model that follows. The host, and any party it names, is honest but
may be lost for good, asleep for days, or restored from an old backup. Members
may be hostile. There is no clock. The groundwork states two consequences, and
every design inherits them. First, a lost host cannot be told from a sleeping
one, so any rule that recovers from loss can also replace a present host
unless the host authorised it beforehand. Second, an honest computer restored
from a backup behaves as an equivocator, because nothing makes it recover its
own later records before it signs.

## 2. What the code assumes about the one signer

Read in the code by the groundwork agents; the reviewers re-read most of these
and their reviews confirm them. A designer must respect each. Nothing in this
list is measured unless it names a test or the second list says so.

- The goal id is a hash of {administrator, definition, salt}, and a genesis is
  valid only from that author. A later signer must be reachable from that key
  through signed records every daemon holds, or it is a different goal
  ([event.rs](../crates/locust-proto/src/event.rs)).
- The chain is structural: the administrator's usable log prefix, governance
  records only, each anchored at the previous one. Positions are derived, not
  signed. A record whose content is refused still takes a position and a
  snapshot ([chain.rs](../crates/locust-core/src/goal/chain.rs),
  [history.rs](../crates/locust-core/src/goal/history.rs)).
- That log is not pure governance. The creator's agent admits itself at
  position 1 and signs reviews, results and stage steps in the same log
  ([goals.rs](../crates/locust-core/src/node/requests/goals.rs)). The formal
  models assume a pure governance log and a constant administrator
  ([Organization.tla](tla/Organization.tla),
  [Workspace.tla](tla/Workspace.tla)).
- Every work event names one governance record, its anchor, and is judged
  against the members, rules, key epoch and tree epoch there. An anchor the
  daemon does not hold makes the event wait until the anchor arrives. An
  anchor that is held but was retracted by a fork waits with nothing that can
  resolve it. Each new event walks its author's ancestry back to the nearest
  effective ancestor ([chain.rs](../crates/locust-core/src/goal/chain.rs)).
- Nothing is acknowledged. A record is in force on a daemon the moment it is
  held; there is no committed state. Admission on an invitation is signed
  unattended inside the join exchange
  ([peers.rs](../crates/locust-core/src/node/peers.rs)).
- A second record at any used position of that key's log cuts the usable
  prefix there and retracts every later governance record on every daemon. No
  code clears the fork, so the halt is never lifted
  ([history.rs](../crates/locust-core/src/goal/history.rs),
  [chain.rs](../crates/locust-core/src/goal/chain.rs)). The tests below show
  it for a review, a contribution and an acceptance; not every record kind was
  tried.
- Two precedents answer "records the actor had not seen". A removal names the
  removed member's last accepted event and fences later ones even if delivered
  later. A tree epoch names a checkpoint and fences the other branch the same
  way ([chain.rs](../crates/locust-core/src/goal/chain.rs),
  [workspace.rs](../crates/locust-core/src/goal/workspace.rs),
  [shared-file-tree-plan.md](../docs/shared-file-tree-plan.md)). A rules
  change has no cutoff.
- A daemon that a record excludes is not sent it. Sync runs only between
  endpoints bound to current members. The two-event fork proof is the only
  frame that reaches non-members
  ([responder.rs](../crates/locust-core/src/sync/responder.rs),
  [peers.rs](../crates/locust-core/src/node/peers.rs)). The refusal of a
  removed endpoint is measured by test
  `an_offline_removed_endpoint_is_refused_after_restart_without_learning_new_history`
  in [replica_tests.rs](../crates/locust-core/src/node/replica_tests.rs).
- Retention keeps events only from the administrator or keys it admitted. A
  sync frontier holds at most 4,096 authors
  ([screen.rs](../crates/locust-core/src/goal/screen.rs),
  [limits.rs](../crates/locust-proto/src/limits.rs)).
- A ticket names the administrator key as both the goal's root and its issuer.
  Invitation records live on that daemon's disk only, and so would the planned
  door record of the [joinable farms plan](../docs/joinable-farms-plan.md),
  which is not built ([invite.rs](../crates/locust-proto/src/invite.rs),
  [invitations.rs](../crates/locust-core/src/node/requests/invitations.rs)).
- The stage runner is the genesis key. A step's identity excludes the signer,
  and a stage task's round is the step with the lowest (author position, event
  id) across authors ([flow.rs](../crates/locust-core/src/goal/flow.rs),
  [fold.rs](../crates/locust-core/src/goal/fold.rs)).
- Content keys: one per epoch, the epoch being the count of effective
  removals. They are stored by the bare epoch number. After the first, a key
  is created only by the signer, on a removal
  ([entry.rs](../crates/locust-core/src/node/entry.rs),
  [replica.rs](../crates/locust-core/src/node/replica.rs)).
- The public page's upload key is derived from a seed on the host's disk and
  the farm id is its hash. Publishing needs a consent from every active member
  and every author of effective work
  ([farm.rs](../crates/locust-core/src/node/farm.rs)).
- The decision-successor index is keyed by author, so two different authors
  deciding after one predecessor is not a dispute. The projection then sorts a
  stream by the author's sequence number and takes the last
  ([fold.rs](../crates/locust-core/src/goal/fold.rs),
  [projection.rs](../crates/locust-core/src/goal/projection.rs); both re-read
  for this note).
- `Node::administrator()` requires the caller's key to equal the goal's
  administrator. The administrator cannot leave or be removed "before
  authority handoff", which does not exist
  ([access.rs](../crates/locust-core/src/node/access.rs),
  [goals.rs](../crates/locust-core/src/node/requests/goals.rs)). Restoring a
  copy is untested ([operations.md](../docs/guide/operations.md)).

Measured by test
([host-key-failure-characterization-2026-10-05.md](host-key-failure-characterization-2026-10-05.md)).
Eleven tests were added in `7bcdbbc`; the replica test loop was corrected in
`0fade2c` and its assertion tightened in `711cb5e`. That note reports all
eleven passing. They were not run again for this note.

- A forked review at host position 6 retracts the admissions at 7 and 8. Other
  members' work at the surviving head stays effective
  (`host_review_fork_retracts_later_governance_but_preserves_prefix_work`).
- A worker whose event anchored at a retracted admission stays
  `Pending(Anchor)` for three further events, even when they anchor at
  surviving rules
  (`retracted_anchor_keeps_same_goal_author_descendants_pending_even_at_surviving_head`).
- A restored host that signs a different record before peers return its later
  records forks. If it recovers first it extends normally
  (`restored_host_signing_before_peer_recovery_forks_but_recovery_first_extends`;
  with a real data directory,
  `sqlite_older_directory_signs_at_used_host_position_unless_later_events_are_recovered_first`).
- A restored host redeems an outstanding invitation at a used position with no
  person involved
  (`restored_host_redeems_outstanding_invitation_before_recovery_at_an_already_used_position`).
- A stage step replayed from the same input at the same position is
  byte-identical and is not a fork. If another event used that position, it
  forks
  (`restored_host_automatic_stage_replay_is_identical_unless_another_event_used_its_position`).
- A held gap blocks signing
  (`restored_host_known_gap_blocks_signing_until_missing_predecessor_arrives`).
  No guard tells a complete-looking old prefix from a current one: that is
  read in the code, and the restore tests above sign from such a prefix.
- Two acceptances by the host key as integrator at one log position halt
  governance. At distinct positions they dispute only the tree
  (`host_integrator_acceptances_at_same_log_position_halt_governance`,
  `host_integrator_acceptances_at_distinct_log_positions_dispute_only_workspace`).
- A member that loses a goal and rejoins with the same key recovers its own
  log first when sync finishes. When membership and content arrive before its
  own log, it signs a second record at position 0
  (`same_key_rejoin_recovers_own_log_before_signing_when_sync_finishes`,
  `same_key_rejoin_can_sign_at_zero_if_membership_and_content_arrive_before_own_log`).
- That member fork at position zero then spreads in one round of ordinary
  reconciliation. Both daemons hold both records, no exchange is refused, and
  governance does not halt (asserted in the second rejoin test). An earlier
  run showed `Refused(ProtocolError)` and no convergence. The cause was the
  test's delivery loop, not the protocol: it handed a frame to a responder
  that still owed part of its answer. The loop now holds a frame until the
  receiver is readable. That the daemon's transport does the same is read from
  the code and from its worker test
  `request_acknowledgement_waits_for_the_whole_response_to_drain`. It was not
  observed between two machines.

## 3. The trade-off the literature forces

Locust today is a degenerate quorum system. In Cheap Paxos with one main
processor every quorum contains it. In Flexible Paxos with a write quorum of
one, a leader change needs every acceptor. So "the host alone decides and it
is final" and "others can replace a lost host" cannot both hold. One must
give.

There are two ways out.

- Consensus only at replacement (Vertical Paxos's configuration master, chain
  replication's master, Cheap Paxos's auxiliaries). The normal case stays one
  signature. There is at most one successor per term, and the old signer is
  fenced from a named position. The cost: records held only by the old signer,
  or by members outside the takeover set, are lost. Equivocation by the
  current signer stays detect-and-halt.
- A second signature on every record. Nothing in force is ever lost and a
  restored host no longer ends the goal. The cost: every admission and rule
  change waits for another computer.

The owner chose the first and accepted its cost (section 1). The plan records
the second as set aside.

Read at source by the classical groundwork (fifteen primary sources in full
text):

- Vertical Paxos: a master decrees each new ballot naming the previous one. In
  Vertical Paxos II it activates the ballot only if that previous ballot is
  still current and after the new leader holds the inherited state. Locust's
  `WorkspaceEpoch` already has this shape for the tree. The open problem is
  the master itself, which today is the same key as the sequencer.
- Cheap Paxos: with two processors a survivor cannot continue alone and a
  returning peer cannot know what happened. A third party is needed, but only
  at failure time, and it need not hold the data.
- Flexible Paxos: with w holders per record, a takeover that loses nothing
  needs N minus w plus 1 participants. With w of 1 that is everyone. For N of
  5: w of 2 needs 4, w of 3 needs 3.
- Raft's membership change: joint consensus, or one server at a time. The 2015
  single-server bug in Ongaro's own algorithm stood for about a year. It shows
  that voters holding different member lists form disjoint majorities. The fix
  was one ordering rule: no configuration change until the leader has
  committed an entry of its own term.
- Viewstamped Replication Revisited: a recovering replica acts only after
  re-learning its state from others. Reconfiguration is the last request of
  the old epoch, and every message names its epoch.
- Also read at source: Chain Replication (a trusted failure detector lets k+1
  nodes survive k failures with no vote; a wrong detection is outside the
  model), PBFT, Tendermint, HotStuff, SUNDR, RFC 6962, PeerReview (silence can
  be suspected, never proven) and Ebb-and-Flow (no protocol is both live under
  sleep and safe under partition). Viewstamped Replication Revisited and PBFT
  were read from course mirrors because the publisher's host refused requests.
- From memory only: Paxos Made Live, production incident lore, accountable
  safety (Casper FFG, BFT forensics), the Sleepy model, commercial witness
  products. Paxos itself was read in Lamport's reviews inside the Cheap Paxos
  and Vertical Paxos papers, not in Paxos Made Simple.

Read at source by the peer-to-peer groundwork (thirteen systems):

- Matrix room version 12 (2025): state resets on real public rooms,
  investigated from 2022. The fix made creators permanently supreme and moved
  succession to re-founding the room, with the right to re-found delegated in
  advance. The project's own lesson is that access control requires a
  hierarchy with the creator at the top.
- Radicle: a strict majority of the delegates named in the parent revision,
  one accept per delegate per parent, siblings rejected on adoption. One
  delegate survives no loss, two need unanimity, three survive one. It is the
  closest structural match to Locust's compare-and-swap records.
- Tailscale tailnet lock: refuses a single signer at setup. One key admits a
  node. Revoking compromised keys needs co-signatures from more signing nodes
  than the keys revoked. An offline disablement secret is the last resort.
  Remedies are graded by severity.
- Keybase teams: many admins but one sequencer, the server. Downgrade leases
  exist because withdrawing authority while it was in use was a real race even
  with one sequencer. A team always has at least one owner.
- Also read at source: SSB groups (the authors found no tie-break that is
  deterministic, total and ungameable), MLS and decentralised MLS (one commit
  per epoch; who picks it is left to the application), Keyhive and p2panda
  (equal admins converge but have no finality, and the last tie-break is a
  timestamp or a hash), Autobase (a majority of indexers, and a majority that
  has seen that majority, before order is final), Syncthing, OpenPGP
  revocation certificates (pre-signed, one-way, no freshness) and Kleppmann
  and Howard's limit result.
- From memory: Git signed tags, local-first/auth, TUF, and the body of the
  Weidner and Kleppmann key-agreement paper (only the abstract was read; two
  hosts blocked the fetch).

Two findings cut across both surveys. First, every working succession path
uses material prepared before the loss. Second, in every protocol read, timing
decides only when a leader change is attempted, never whether it is safe. A
person's "the host is gone" can replace the timeout but not the quorum. The
exception is chain replication's fail-stop assumption, under which a wrong
assertion is outside the model.

## 4. Why a members' vote is ruled out

The fault groundwork and both surveys rule out "a majority of the members" as
the electorate, and the successor and electors designs list it as a rejected
variant. No reviewer was given such a design to review.

- Counting is per agent key, and keys are free. A person has no identity in
  signed history. One person may hold several keys, and the host's person adds
  agents without a ticket. A vote measures invitations, not people.
- In a public goal, as planned, up to 15 of 16 members can be strangers
  admitted unattended. The
  [joinable farms plan](../docs/joinable-farms-plan.md) already rules that
  door members decide nothing.
- With two members a majority is both, so no majority rule survives the host's
  loss in the commonest goal.
- The electorate is not agreed. The host changes the member list alone, in
  steps of any size, unacknowledged. Members holding different host prefixes
  count different electorates, and with the host gone their majorities can be
  disjoint: {host, 2, 3} gives {2, 3}; {host, 2..8} gives {4..8}. This is the
  hazard Raft's joint consensus exists for (inferred by the classical
  groundwork).
- The proposer chooses the anchor, so the proposer chooses the denominator.
  Reading "the members" at a chosen past position lets removed members vote to
  undo their own removal.
- Availability (binomial arithmetic from the fault groundwork, an assumption,
  not a measurement): at 50% reachability per computer a majority of five is
  reachable half the time and a majority of sixteen 40% of the time. Today's
  one named computer is reachable 50% of the time. With the host gone the
  figures drop to 31% and 30%.
- Kleppmann and Howard: a uniqueness invariant such as "exactly one host"
  cannot be made safe against free identities by any merge rule. It needs a
  named, bounded set of deciders (the mapping to "one host" is the
  groundwork's inference).
- The owner's rule points the other way: hostile members are the host's to
  remove, and a vote hands them power while the host is away.

What remains is one named party, or a threshold of a named list the host
writes in advance.

## 5. Four designs

All four are proposals. Words used below: the base is the governance record a
takeover continues from. The cutoff is the last record of the old signer that
is kept. A record is superseded (two designs say fenced) when it is held but
excluded, whenever it was delivered. A term is one signer's tenure.

| Design | What moves | Who can act | Lost at a takeover | Smallest group | Verdict |
| --- | --- | --- | --- | --- | --- |
| Named successor | The seat, to a fresh seat key per term, from an exact base | The one member the host named, optionally with named witnesses | Every old-seat record after the base and the work anchored there | 2: host plus one member on another computer | Build it with six changes (attack); build this family, not this text (fit) |
| Named electors | The host, to a fresh governance key, by a certificate pinning N of M approvals | N of M electors named in the rules at the base | Old-host records after the base that no certificate participant held | 2 for 1-of-1; 2-of-3 among other people means four computers (derived) | Build after the validity rule is amended (attack); one backup host first (fit) |
| Countersigned | Nothing in force; every governance record waits for a countersignature | A named standby, countersigned by the standby or an always-on witness | Old-host records never countersigned; old-host work past the cutoff | 3: host, standby, witness | Do not build (fit); only with a witness, opt-in, after five changes (attack). It is the option the owner set aside |
| Move the key | The key itself, to another computer; a `Recovered` record fences | Whoever holds a copy of the key, on a member's daemon | Records only the old device held beyond the base | 2: own second computer admitted as a member | Build the dedicated key regardless; the rest is a one-person path (fit); build this family (attack) |

### Named successor

Idea. The right to sign governance is called the seat. It belongs to a
dedicated seat key per term, never to a member. The host signs `SuccessionSet`
naming one member's agent. On a loss that member's daemon syncs with every
reachable member, generates a fresh seat key and signs
`SeatTaken { base, agent, endorsement }` at position 0 of the new key's log.
Everything the old seat key signed after the base is superseded, whenever
delivered. Two complete takeovers of one term dispute it for good.

Confirmed by the reviewers. No member the host never named can take, move or
block the seat through the chain rule: every forged or self-endorsed
`SeatTaken` fails the successor check. A member that was once named is another
matter; see the first break. The chain is a pure function of the held set. A
later fork of the old seat log below the base cannot retract the kept prefix,
because the kept prefix is the base's ancestry, not the usable prefix.
Old-host records after the base have the same standing whether delivered
before or after the takeover. A two-member goal recovers. A halted host log
can be recovered by taking from one branch, which nothing does today. The
planned handoff needs no agreement.

Broken:

- The taker chooses the base, and with it the snapshot the successor check
  reads (both reviewers). Sequence: the host signs `SuccessionSet{A}` at G5,
  `SuccessionSet{B}` at G6, `MemberRemoved{A}` at G7. A's daemon is never sent
  its removal and is refused by every peer. It honestly sees a dead goal and
  signs `SeatTaken{base: G5}`. The snapshot at G5 names A as successor and
  member, so the record is in force. G6 and G7 are superseded. A is host and a
  member again while the real host is present. Renaming, cancelling and
  removal all fail against a stale copy of a once-named key, with no time
  limit.
- The dispute is terminal and cascades (both reviewers). Sequence: A takes the
  seat, then A's laptop is restored from a backup taken before its own
  `SeatTaken`. It holds no seat key and no record, and status says A is the
  successor. A runs `seat take` again. There are now two candidates for term 0
  and the term is disputed. The first seat key is then "not the seat of any
  term", so every later term is void in one delivery. If the old host is lost,
  the goal is frozen for good. A variant needs no restore of the successor and
  freezes the goal with everyone present: the host's own seat log forks, the
  host retakes on one branch through the default self-successor, and the named
  successor takes on the other.
- The base must be a governance record but the fence is by log position (fit).
  Even the cleanest takeover drops every stage step signed since the last
  admission or rules change, and under the plan's Phases 8 and 9 every plan
  text and file acceptance too. Forty changes landed since an admission three
  weeks ago are all superseded. They cannot replay: a proposal's parent must
  equal the previous decision's id, so change 2 can never be accepted once
  change 1 is re-signed.
- The receipt filter keeps any `SeatTaken` endorsed by a current member, so
  one member can mint 4,097 seat logs and stop sync at the 4,096-author cap
  (both reviewers). Read as "current member", the filter also drops a valid
  takeover on any daemon holding the successor's later removal, so honest
  daemons never converge.
- Dropping a superseded epoch's content key makes everything sealed under it
  unreadable to its own authors, because signing commits only the sealed blob
  (attack). "The author reposts" is then false for results, reviews and plan
  text. The reviewer did not search for a local plaintext copy.
- The restart guard ("no seat signature until one exchange with a member
  completes") deadlocks a host whose other members are all gone. It can
  neither admit nor remove, because both are seat-signed (both reviewers). It
  also does not cover waking from sleep.
- Smaller: witness statements do not name the new seat key, so a restored
  successor reuses the same pinned statements for a second takeover. The stage
  runner is defined two ways. The base's ancestry is undefined when the base
  is held but an earlier seat record is not. A takeover waiting for a witness
  statement is `Pending`, so daemons run the old term for days. A host that
  named someone else cannot repair its own forked log.
- Unverified when reviewed: whether ordinary sync delivers both records of a
  fork at position 0, on which the dispute and the seat-log fork depend (fit).
  The doubt came from an earlier test result, which has since been corrected
  for a member's log (section 2). It is not shown for a seat key, which is not
  a member and whose records today's retention filter drops at receipt.

Survives as a family. Both reviewers would build it after the fixes in section
7, with one named member and no witnesses first.

### Named electors

Idea. The host binds rules naming M electors and a threshold N. A candidate
publishes `HostCandidacy { term, base, governance_key }`. Each elector
approves one candidate per term with `SuccessionApproved`, on a person's
command; the design lets it approve the same candidate again on a later base.
When the candidate's daemon holds N approvals it signs
`HostChanged { expected: base, successor, host_member, evidence }` at position
0 of the fresh key, unattended. The design calls this Radicle's rule. A
planned handoff is a `HostChanged` by the host itself with empty evidence.

Confirmed by the reviewers. Hostile non-elector members cannot approve, forge
a certificate or partition the mesh. With N above M/2 and one elector list,
two certificates on one base need an elector who signed for both. Pinned
approvals survive a later fork or removal of the approver, as picks do today.
Of about 45 line references into the code, all but three point at what the
design says. The 1-of-1 case for a person's own second computer works.

Broken:

- A certificate whose evidence ids nobody holds is `Pending` and "the walk
  stops here" (both reviewers). One hostile elector, or any member when
  candidates may be members, publishes a candidacy and signs
  `HostChanged { expected: h, evidence: [ids that do not exist] }` on any past
  head h. Every daemon stops its chain at h for good, and the host's later
  removal of the attacker is never reached. Today no member-signed record can
  stop the chain.
- Two certificates on different bases of one branch are not a dispute; the
  earlier base wins silently (both reviewers). Sequence with no fault, through
  the design's own re-basing allowance: X stands on R1 and A approves. B holds
  R2, refuses, and hands over the longer chain. X stands again on R2, A and B
  approve, H2 is signed and a week of governance follows. C wakes holding only
  R1 and approves the first candidacy. X's daemon now holds N approvals of it
  and signs H1 on R1. At head R1 exactly one certificate exists, so R2, H2 and
  the week are void everywhere. Which candidacy reaches N first at one daemon
  decides the base.
- A planned handoff sits in the old host's log. A restored old host that signs
  an unattended admission at that position forks the log. The fork rule
  rescues only elected certificates, so the successor's whole term is
  retracted.
- The cold spare key recovers nothing. A non-member endpoint is never synced,
  and a solo goal's only history was on the lost disk.
- Removing an elector does not remove its elector power; approvals are counted
  with no membership check. The candidate also picks the base, and with it the
  elector list, so un-named electors can still act (fit).
- A restored certifier holding a different subset of approvals signs a
  different certificate. That is a fork at position 0 of the new key. The
  reviewers read the outcome differently: the certificate stands but the term
  has no usable log (attack), or both certificates are disputed and governance
  is dead (fit).
- A candidate lost after collecting approvals spends those votes for good.
  With M of 1 that is the end of the goal.
- Approvals never expire and cannot be withdrawn. A second certificate
  delivered a year later retracts the whole term and leaves no host.
- The seat moves unattended when the last approval arrives. The plan's own
  reasoning is that the daemon must never move a seat while the host is away
  (fit).
- Smaller: two bases with different elector lists give two certificates with
  no elector signing twice. The size is understated five to eight times. The
  dedicated key as specified cannot sign stage steps, which are work and need
  a member.

Survives as a family. The attack reviewer would build it with M in {1, 3, 5},
N fixed at a strict majority, candidates only from the electors, no `Pending`
standing for governance, and equivocating electors discounted. The fit
reviewer would build one backup host first and thresholds second.

### Countersigned governance

Idea. A governance record counts only once an acker has countersigned it. The
acker is the standby's daemon or an always-on witness. The chain is the
countersigned anchor chain, not the host log's prefix, so a fork of the host's
log is harmless. A named standby takes over with
`HostChanged { base, cutoff }`, acked like any record. The design claims
nothing in force is ever lost.

Confirmed by the reviewers. Under an honest witness with durable storage there
is at most one in-force successor per anchor on any daemon. The in-force
function is a pure acyclic recursion. Equivocation by the host is harmless
once an acker exists. Members are on no path. A sleeping host's unattended
admission is never in force, which the fit reviewer says is handled better
than in a cutoff-only design.

Broken:

- It is the option recorded as set aside in the plan on 2026-10-05. Under it a
  removal is not in force until someone else signs, against "hostile members
  are the host's to remove".
- With the design's own dedicated key the cutoff fences the governance key's
  log only (both reviewers). Sequence, under the plan's Phase 9 where the
  host's agent records acceptances: the host sleeps; the standby takes over
  and is acked; the new host's agent accepts file change X after decision D_k.
  The old host wakes, its automatic loop runs with the old anchor, and its
  agent accepts change Y after the same D_k. The fold keys the dispute by
  author, so the two are not siblings. The projection sorts by author sequence
  and takes the last, and the old agent's log is longer. Y silently replaces X
  on every daemon that receives it, with no dispute and no proof. The attack
  reviewer did not search for another check that would flag it first.
- The rule for an unacked prefix (an acked `SuccessionSet` beats an unacked
  sibling) is a silent rewind. A host restored from a backup older than a
  removal runs `goal witness set`, the fresh witness acks, the removal is
  fenced, the removed member is back and the key epoch drops. A restore older
  than the first `SuccessionSet` retracts the whole countersigned chain.
- A hostile standby freezes a present host. It signs one record at the head,
  the witness holds two siblings and acks neither, and every further host
  record is a third sibling. "Ack none while two are held" has no exit but a
  person's withdrawal, which the hostile party never gives.
- A removed standby still takes the seat; the rules never consult its
  membership.
- A witness restored from a backup that forgot one ack, plus one sleeping
  member who holds that ack, ends the goal: two acked successors of one
  anchor, disputed, re-found. Permanent loss of the witness freezes a healthy
  host, because changing the acker needs the old acker's ack. Re-founding is
  invoked for five outcomes and never designed. The goal id commits to the
  first key, so it is a new goal with no history.
- The laptop-standby configuration is a 2-of-2. By the fault groundwork's
  arithmetic both laptops are awake together 6%, 25% or 81% of the time at
  25%, 50% or 90% availability. Losing the standby freezes the healthy host
  with no exit, and a routine restore of the standby silently disarms the
  protection.
- A joiner admitted while the witness is down holds the history and the
  content key and is never a member. The farm as witness would hold every
  member key, endpoint and removal in clear, for people who agreed to none of
  it.
- Which of a woken host's record and the standby's takeover reaches the
  witness first decides who is host. That is arrival order at one party (both
  reviewers).
- Size: three to four phases plus an operated service holding a
  safety-critical key, and non-member transport the design does not list.

Does not survive as designed. The fit reviewer keeps only the parts that need
no countersigner: the named standby, `HostChanged` with base and cutoff,
host-at-position in the chain builder, the proof frame, the ticket issuer
check and keys stored by removal id. The attack reviewer would build it only
with a witness, only as opt-in, and only after five changes.

### Move the key

Idea. The goal's authority is a per-goal governance key G, never a member,
named as the administrator in Genesis. The host's agent is the seat, named by
`SeatBound`. Custody is a passphrase-sealed bundle or M-of-N Shamir shares.
After a loss, a member loads G on its daemon and runs `goal recover`, which
signs `Recovered` at the tip it holds. The chain builder walks G's held
records as a tree keyed by `prev`. At a fork the branch with the deepest
`Recovered` wins; equal depth, or no `Recovered` on either side, is a halt.
Tickets, the goal id and the retention filter do not change.

Confirmed by the reviewers. The resolver is a pure function. The fence is
arrival-independent. A `Recovered` below a later fork protects its ancestry.
No unattended signature, member record or restored backup can move a settled
change. Members without G cannot take or move the seat. Custody material is
stateless, so a restored custodian cannot equivocate. The dedicated key
matches what the formal model already assumes.

Broken:

- A deposed host that the new host also removed never converges (attack; fit
  rates it minor). The one-record notice carries only the newest `Recovered`.
  After a second recovery the deposed host lacks that record's predecessor, so
  by the design's rule it ignores it. Ordinary sync is refused. A joiner
  arrives on an old ticket and is admitted at the fenced position. No daemon
  ever holds both records at that position, so no fork proof is generated. The
  old host and its joiners form an island that believes the old host is host,
  with no end.
- The base is whatever the recovering daemon reached, and a hostile member
  steers it by under-reporting its frontier (attack; the reviewer found no way
  to detect that and did not try it). Sequence: the host admits Y, rebinds
  rules and removes stranger W at n+1 to n+3. It syncs them with hostile X and
  with M, who then sleeps, and is lost. X answers the recoverer with n. The
  report says "reached 4 of 5, latest record n" and the person confirms. When
  M wakes, n+1 to n+3 are fenced for everyone and W is a member again holding
  every key.
- The retained rule "sign nothing while G records are waiting" lets one
  orphaned record, delivered without its predecessor, freeze the new host.
  After any fence the waiting count never returns to zero (both reviewers).
- Work does not continue during a halt for members who anchored above the
  fork, and `goal recover --keep` then makes their keys unusable (fit; from
  reading the ancestry walk, which was re-read for this note; no test confirms
  it). Sequence: e50 is anchored above the fork; the chain stops at j. e51,
  anchored at j, is `Pending(Anchor)` because e50's anchor has no position.
  The host keeps the original branch and e50's position n returns. e51 is now
  `AnchorRegressed`, since n is above j. Every later event walks through e51,
  and the early exit stops only at an effective ancestor.
- A replacement computer holding the key cannot recover, because a non-member
  endpoint receives no history. The recoverer must already be a member, which
  in a public goal may be a stranger.
- `SeatBound` has no cutoff, so the old and new seat are both valid signers of
  one stream. The successor index is per author, so it is not a dispute. A
  late old-seat stage step with a lower (position, id) re-keys a task's round.
- A stale honest holder voids live governance with one yes. A custodian
  offline for three weeks runs `goal recover`, the report says zero members
  reached, the person confirms, and three weeks of the present host's records
  are fenced.
- A new host restored from a backup taken while it was host forks above its
  own `Recovered`. Neither side scores, the halt is undecided, and work since
  the fork waits for a person to pick a branch. The boot gate is vacuous when
  nobody is reachable (attack).
- `goal recover --from` on an already halted goal adds a third sibling at the
  fork and fences both old branches. A holder who signs `Recovered` as a pin
  with a stale view flips a settled change (attack).
- The key can never be rotated or revoked and the set of holders only grows.
  Any earlier holder can fence the whole history by forking at position 1; the
  design lists this as a weakness. `host backup` puts the root key one command
  away from an agent with a shell.
- It does not deliver "people named in advance, vote, etc". Every rule value
  still ends with one person holding one key, and the first recovery
  reconstructs the whole key on one device.

Survives in part. The fit reviewer would build the dedicated governance key
whichever design is chosen, and calls the rest the one-person, two-computers
path that should not be extended with shares to look like a named-people rule.
The attack reviewer would build this family, with G's log served to every
historical endpoint and Shamir deferred.

## 6. What survived

Proposed, not accepted. The four reviewers of the two leading designs (Named
successor and Named electors), joined by the fit reviewer of the countersigned
design, converge on one skeleton. Where a rule rests on fewer reviewers it
says so.

1. A dedicated governance key, separate from the host's agent, decided before
   Phase 1 of the plan, because it changes Genesis and what every `host` field
   means.
2. The host names who may replace it in a signed governance record, in clear.
   It does not go in the formation, which is sealed under the content key and
   whose hash it would change.
3. A change of host is one record signed by a fresh key at position 0 of its
   own log. It names its exact predecessor by event id, as
   `RulesBound.expected` and `WorkspaceEpoch.expected_epoch` do.
4. The kept prefix is the base's ancestry read by `prev`, so a fork of the old
   key below the base changes nothing.
5. Everything the old key signed after the cutoff is excluded with a new
   standing, `superseded`, whenever delivered. That the cutoff may be any
   event of the old key's log, not only a governance record, is the successor
   fit reviewer's amendment.
6. Work anchored at a superseded record is excluded, not pending, and the
   author's next event on the kept chain is judged normally. The ancestry walk
   changes for this one case.
7. Governance never waits on content. A change-of-host record that cannot be
   fully checked from the held set is ignored, never `Pending`.
8. The chain stays a pure function of the held set: no clock, hash or arrival
   order anywhere.
9. The party that takes over syncs with every reachable member before choosing
   the base. This is a daemon rule, not validity. The plan it shows names
   which members were heard from.
10. A planned handoff by a present host needs no agreement.
11. The change travels on the fork-proof route to every endpoint ever
    admitted, in the first version, and a receiver keeps it only if it passes
    the full validity rule.
12. Content keys are stored by the id of the removal record that opened them,
    and fenced keys stay readable. Whether the wire request must also name the
    record is disputed (section 7).
13. The old host stays a member by default. Removing it is a separate
    decision, because the protocol cannot tell theft from sleep.
14. The new host's first acts close the decision streams: a new tree epoch
    with a checkpoint and a rules rebinding.
15. The sentence a user reads states the loss. The words "host" and "backup
    host" are the successor fit reviewer's.

## 7. Open problems and the fixes proposed

Each is a problem any version must solve. Where reviewers propose different
fixes, all are given and none is chosen. Each fix is tagged with the design it
was written against and the reviewer.

**Who picks the base, and how a renamed or removed successor is stopped.** The
taker chooses the base, so a stale copy of any once-named key acts for ever
(Named successor, first break). Fixes proposed:

- Removal outranks naming: a takeover endorsed by A is not a candidate on any
  daemon holding a `MemberRemoved` of A by the term's key above the base.
  Cost: an honest removed backup that took over unknowingly loses the seat
  when the removal arrives. If the host also died before naming anyone else,
  the goal is stuck (successor, attack).
- The old host's later word survives the fence: a takeover is void if a held
  old-key record descending from its base names a different successor or
  removes the taker. Cost: a settled takeover is undone when such a record is
  delivered later (successor, fit).
- Removing an elector drops it from the elector list in the same act
  (electors, attack). The electors fit review shows this does not stop a
  candidate who picks a base from before the removal.
- Send the un-naming over the evidence channel, and make the election
  restriction a validity rule through `AnchorRegressed`. Against N colluding
  former electors there is no fix inside the rule set: either elector changes
  are acknowledged by the old electors, or a later host record may void a
  certificate, which gives up finality (electors, fit).
- A removed standby is no standby: read the standby only if it is a member at
  the base, and refuse to remove a member while it is named (countersigned,
  both).

**A dispute that must not be permanent.** One honest restore produces two
takeovers of one term. Fixes proposed:

- A takeover may name the other candidates it supersedes; one naming every
  other held candidate is in force. Cost: a bounded, self-inflicted
  retraction. Also refuse `seat take` when no member was heard from, unless
  the person overrides (successor, attack).
- Derive the seat key from the taker's agent seed, the goal and the term's
  first record, and sign with time 0. A restored copy then re-creates the
  identical record, which is not a fork (measured for stage steps, section 2).
  On two different records, halt at the earlier base and never hand the chain
  back to the old key; a later record by the same endorser may void its
  earlier ones (successor, fit).
- Count all held approvals as evidence, treat two same-content certificates by
  one key as one decision, and extend the dispute check to every certificate
  of the term at any base. Discount electors present in both evidence sets; if
  exactly one certificate still has N, it stands. The certifier signs once per
  term (electors, attack).
- Drop the evidence list and count held approvals, so the certificate's bytes
  depend only on base, key and host member. Make one candidacy per elector per
  term a validity rule, drop re-basing, and make certification a person's
  command (electors, fit).
- The deeper `Recovered` wins (Move the key, as designed). Cost: an honest
  holder with a stale view can flip a settled change, so `Recovered` is signed
  only on import, on a visible halt or on resume (Move the key, attack).

**Removals a takeover has not seen.** A removal lives on the host's disk until
its first sync. A takeover that fences it makes the removed member a member
again, holding every key. No pure fix exists without acknowledgements, which
the owner set aside. Mitigations proposed:

- Status shows for each removal and admission whether the backup has seen it,
  derived from the backup's anchors with no new record. The words say a
  removal is final only once the backup holds it (successor, fit).
- The recover command refuses while any member in the snapshot is unreached
  unless the person names each one to skip. The reach report distinguishes
  refused, unreachable and current per member. The old host's daemon never
  prints a removal as done until another member holds it (Move the key,
  attack).
- When a fenced removal is later held, every daemon holding the key shows it
  and offers to sign it again (Move the key, fit).

**The cutoff being any record of the old signer.** If the base must be a
governance record and the fence is by position, every recording since the last
governance record is lost at every takeover (successor, fit). Fixes proposed:

- The cutoff is the last event of the old key's log the taker holds, and the
  record's anchor is the last governance record at or before it (successor,
  fit; the countersigned design's `cutoff: AuthorPoint` has the same shape).
- With a dedicated key the cutoff must also cover every key that held host
  authority at the base, or the takeover must close each stream with a new
  epoch and round (countersigned, attack). The fit reviews say the same: a
  second cutoff for the host agent's key, or a tree epoch and rules rebinding
  made mandatory (countersigned, fit); the seat change carries the old seat's
  last accepted point (Move the key, fit).
- The decision-successor index drops the author from its key, so two
  authorities after one predecessor are disputed rather than ordered by author
  sequence (countersigned, attack; electors, fit).

**Telling the deposed host and whoever it admitted afterwards.** Ordinary sync
refuses a removed daemon, and a one-record notice cannot be placed without its
predecessor. Fixes proposed:

- Build the proof frame in the first version and have the old host push it to
  every endpoint it admitted after the base. A joiner whose held takeover
  fences its own admission is told to ask for a new ticket (successor,
  attack).
- Until the frame exists, refuse to remove the previous host's agent
  (successor, fit).
- Serve the governance key's whole author log to every historical endpoint
  over the evidence path regardless of membership. This removes the island,
  the two-partition case and the deposed host's blindness in one change (Move
  the key, attack).
- Keep the notice but send every `Recovered` on the path, keep a delivered
  record per endpoint, and stop pushing proofs of resolved forks (Move the
  key, fit).
- A quorum certificate is not self-verifying. Its proof needs the candidacy,
  the N approvals and their ancestry, and the rules at the base are sealed
  under the content key. Define a bounded proof frame and put the elector list
  where it is readable without content keys (electors, fit).

**Catching up before signing after a restore.** Every design adds "sign
nothing until one exchange with a member completes". As written it deadlocks a
host whose members are gone, is stalled by one member that never finishes a
round, and is vacuous when nobody is reachable, which is when a restore signs.
All reviewers treat the fix as a liveness aid, not validity. Fixes proposed:

- Attempt every endpoint; sign after the first completed exchange or after
  every endpoint refused or failed; a person may override (successor, attack).
- Apply the gate only when the person says the directory was restored or a
  marker kept outside the data directory disagrees. Never count members whose
  loss is recorded. On wake, try one exchange before the first unattended
  signature (successor, fit).
- The gate counts attempts per member and is bounded by a stated timer. After
  a boot or restore it needs one completed round or a person's confirmation.
  The flag clears on wake, not only on boot (Move the key, attack). The
  reviewer's argument is that such a timer only delays and never decides; the
  accepted rule against clocks is in section 1.
- The gate opens when each connected member's exchange has ended with any
  outcome. Unplaced records never block recovery. Joins get a retry-later
  refusal instead of a permanent one. The gate also covers the seat agent
  (Move the key, fit).

**Content keys across two signers.** Two removals at one epoch number on two
branches give two keys. Three designs store keys by the id of the removal that
opened them. The successor design instead drops the superseded key, and both
its reviewers ask for the record-id store. Additions proposed:

- The wire request must also name the record; today it names the bare epoch
  (Move the key, attack; electors, fit). The Move the key fit reviewer
  disagrees: the store change is enough, because a receiver already rejects a
  key that does not open the effective removal.
- Fenced keys must stay readable. Daemons hold only sealed blobs, and dropping
  the key destroys the authors' own content (successor, attack; countersigned,
  attack).
- A standalone rotation record in the first version, so a recovery can change
  the key without removing anyone. The Move the key design defers it (Move the
  key, attack).

**Public page, door and invitations.** The upload key never left the old disk
and the farm id is its hash. The old page can neither be updated nor taken
down and keeps pointing at the dead endpoint. Republishing needs a consent
from every author of effective work, which the lost host can never give,
unless the joinable plan's host-only consent is built. Outstanding invitations
and the planned door record are on the old disk. Old tickets are dead, and a
host that was only asleep admits a joiner on an old ticket into the superseded
copy. Tickets need an issuer field beside the root (three designs) or stay
unchanged because the key is unchanged (Move the key). Fixes proposed:

- Say in the takeover plan that a public goal gets a new address and a new
  door (successor, fit; electors, fit). The designs have the new host issue
  invitations again.
- Derive the upload seed from the governance key, or carry it in the custody
  bundle at the cost that the bundle also unlocks the page (Move the key).
- Later, let the farm service re-key an existing farm id from the signed chain
  (successor, fit).

**The dedicated signing key.** All four designs want it. Every reviewer either
requires it or conditions it only on the per-authority cutoff above. What it
changes (the phases are plan text, not code):

- Genesis names the governance key, hashed into the goal id, and the creating
  agent separately. Move the key keeps the Genesis bytes and makes the
  administrator a non-member.
- Phase 1: `Node::host()`, `plan_join`, the stage runner and every `host`
  field read the current key from the chain. "The host is the person whose
  agent started the goal" stops being true.
- Phase 4: the signed-format changes must land in its one protocol version
  bump or force a second.
- Phases 8 and 9: the recording authority becomes the host agent at the act's
  anchor. `resolve` takes no anchor today, with 26 call sites by the
  reviewer's count. The exit criteria "signed by the host agent" change. Two
  seats after one predecessor need the successor index fixed.
- A key that is neither member nor enrolled agent also touches `signer()` and
  `next_place()` in authoring, `drive_flow`, farm consent eligibility and
  `task_creator`. In farm eligibility the key becomes an author who cannot
  consent, so publication stops in any goal with stages.
- Cost: a Phase-1-sized rename. A count for this note found 211 uses of
  `administrator` in 45 Rust files, the figure the reviewer gave.

**Size.** Authors' estimates against reviewers', all estimates. Named
successor: 1,500 to 2,500 lines against 4,000 to 6,000, one to two of the
larger phases and the riskiest because it replaces the chain builder. Named
electors: 700 to 1,000 against 1,800 to 3,000 of production plus 2,500 to
4,000 of tests, about three phases. Countersigned: 2,000 to 3,000 plus a
service against three to four phases plus an operated service. Move the key:
about 2,000 against 3,500 to 5,000, about two phases. Yardstick: the
characterization commit added 1,031 lines for eleven tests and their note,
with no production change.

## 8. The smallest sound first version

Proposed, not accepted. The successor reviewers and the fit reviewers of the
electors and countersigned designs converge on one backup host, with this
shape:

- A dedicated governance key in Genesis beside the creating agent; the goal id
  hashes the governance key.
- One record by which the host names one member as backup host. The backup
  must be a member at the base. Door members may not be named.
- One takeover record, signed by a fresh key at position 0. It names the exact
  base and a cutoff that may be any event of the old key's log. Everything
  after it is superseded whenever delivered, with `Excluded(Superseded)` in
  the ancestry walk.
- The rule slot exists with one value, "alone". No witnesses, no thresholds,
  no branch choice at retake.
- A planned handoff is the same record with the old host's signed consent as
  the approval (electors, fit), or the host names and the other person takes
  (successor, fit). No `handed` flag.
- No `Pending` standing for the takeover record. A strict receipt filter: the
  base is held and the endorser is the backup named there. The cap is two per
  endorser per term (successor, fit) or one per term, endorser and base
  (successor, attack).
- The question of whether the host's later removal of the backup outranks the
  takeover, decided one way (section 7, first problem).
- The proof frame on the fork-proof route from the start. The previous host
  stays a member and cannot be removed until the frame has reached it or the
  person confirms the loss.
- Content keys by removal record id in the store; fenced keys kept readable.
- The new host's first acts: a tree epoch with a checkpoint and a rules
  rebinding, under one confirmation.
- Status shows, for each removal and admission, whether the backup has seen
  it, derived from the backup's anchors.
- The takeover plan says tickets, the door and the public page address do not
  carry over.
- The sentence (successor, fit): "The host can name one member as backup host.
  The backup can take over the goal at any time; whatever the old host's
  computer signed that the backup had not received is dropped." If "removal
  outranks naming" is chosen, the attack reviewer adds a clause, here in this
  note's words: if the host removes the backup, that stands even if the backup
  acts later.

The Move the key reviewers describe a different small version: the dedicated
key, `Recovered`, one exported key file for the person's own second computer,
and no shares.

A second step, a threshold among named people on the same record, needs these
decided first:

- Naming an elector is permanent unless a later host record may void a
  certificate, which gives up finality (electors, fit).
- An approval has no end date and cannot be withdrawn (electors, fit).
- A second certificate means no host and history back to the base (electors,
  fit), or equivocators are discounted and the surviving certificate is kept
  (electors, attack).
- M in {1, 3, 5} with N fixed at a strict majority, and candidates only from
  the electors (electors, attack).
- Approval is always a person's act (the design), and so is certification
  (electors, fit).
- Whether the farm may be a named elector for public goals. The electors
  design leaves it as a later option and the fault groundwork lists it as an
  open question.

## 9. Model before building, and the tests that exist

The project's practice is to model a new authority rule before building it, as
it did for the tree's checkpoint mode ([workspace.md](tla/workspace.md)).
Every reviewer asks for it here, and the successor fit reviewer says its first
three breaks would have appeared as failed invariants. What the models need:

- The signer as a function of position in
  [Organization.tla](tla/Organization.tla) and
  [Workspace.tla](tla/Workspace.tla), where the administrator is the constant
  0, and the new records in the transcripts.
- Properties over delivery steps, which no model has today. Once a change is
  in force, a later delivery keeps it or disputes it and never puts a
  different one in force. Nothing after the base of an in-force change is ever
  effective. No `Pending` is caused by a superseded anchor. There is at most
  one in-force change per term.
- A small node model with `sign`, `sync` and `restore` actions, because a
  restored honest daemon is a transition, not an assumption.
- Key epochs, which no model has.

Scenarios the designs and reviewers list, each a fixed signed transcript
delivered in every order:

- two takeovers on one base; two on different bases of one branch; two on
  divergent branches of a forked old log;
- an old-host record beyond the base delivered before and after the change; an
  old-host fork below the base;
- a fork at position 0 and above position 0 of the new log;
- a rename on one branch with both the old and the new successor taking; a
  removed successor taking from a base before its removal; a takeover endorsed
  through the self default by an earlier host's agent;
- a dispute in an early term after later terms exist; a dispute delivered
  after many records of the new term;
- a certificate naming approvals nobody holds while the host is present; a
  restored successor holding a different set of approvals; an elector list
  changed between two bases;
- a restored host after a planned handoff, forking at the handoff's position;
- mechanical records after the base; the receipt rule, since convergence
  depends on it and no model covers retention;
- two seats in one epoch and one rules binding, in Workspace.tla and
  [FlowEffects.tla](tla/FlowEffects.tla);
- events signed by members during a halt, followed by each resolution; a stale
  holder recovering over a present host.

Tests that exist and pin today's behaviour. The eleven from the
characterization note are:

- `host_review_fork_retracts_later_governance_but_preserves_prefix_work` and
  `retracted_anchor_keeps_same_goal_author_descendants_pending_even_at_surviving_head`
  in [tests.rs](../crates/locust-core/src/goal/tests.rs);
- `restored_host_signing_before_peer_recovery_forks_but_recovery_first_extends`,
  `restored_host_redeems_outstanding_invitation_before_recovery_at_an_already_used_position`,
  `restored_host_automatic_stage_replay_is_identical_unless_another_event_used_its_position`
  and
  `restored_host_known_gap_blocks_signing_until_missing_predecessor_arrives`
  in [delivery.rs](../crates/locust-core/src/node/tests/delivery.rs);
- `host_integrator_acceptances_at_same_log_position_halt_governance` and
  `host_integrator_acceptances_at_distinct_log_positions_dispute_only_workspace`
  in [workspace_tests.rs](../crates/locust-core/src/goal/workspace_tests.rs);
- the two `same_key_rejoin_*` tests in
  [replica_tests.rs](../crates/locust-core/src/node/replica_tests.rs);
- `sqlite_older_directory_signs_at_used_host_position_unless_later_events_are_recovered_first`
  in [durable_tests.rs](../crates/locust/src/daemon/durable_tests.rs).

Any redesign rewrites these eleven (successor, fit). Two more tests bear on
the designs:
`an_offline_removed_endpoint_is_refused_after_restart_without_learning_new_history`
in replica_tests.rs, and
`decision_successors_keep_author_scope_purpose_and_predecessor_separate` in
tests.rs, which pins the per-author successor index.

Tests the groundwork and reviewers ask for:

- Stranding of members who built on retracted governance. Several reviews
  still call it untested;
  `retracted_anchor_keeps_same_goal_author_descendants_pending_even_at_surviving_head`
  now covers it for three descendants.
- Two host acceptances after one predecessor. The plan's Phase 9 test table in
  [roles-and-permissions-plan-details.md](../docs/roles-and-permissions-plan-details.md)
  assigns this to the two `host_integrator_*` tests. One review calls the test
  `two_host_acceptances_after_one_predecessor`; no test or plan text has that
  name.
- Not in tracked code when this note was checked: two different authors
  holding host authority and deciding after one predecessor; events signed by
  members during a halt followed by `--keep`; a removal whose new key reached
  nobody stopping text-carrying writes; a restore drill across two machines.
  A diverged log run between two real daemons was added afterwards, in
  `6b924d0` (`reconcile_tests.rs` in the daemon crate).

## 10. Decisions for the owner

1. Is a dedicated governance key in Genesis, separate from the host's agent
   and decided before Phase 1, accepted?
2. Is it accepted that a named backup can take over a present host, because
   lost and asleep cannot be told apart, with the words saying so?
3. When the host has removed or renamed the backup and the backup acts anyway
   from an older base: does the removal win when it arrives (finality given up
   in that one case), or does the takeover stand?
4. On two complete takeovers of one term: permanent halt, halt at the earlier
   base, resolvable by a later record from the same party, or the deepest
   record wins?
5. May the cutoff be any event of the old key's log, so landed file changes
   and plan texts up to it are kept?
6. Does the old host stay a member by default after a takeover, with removal a
   separate decision by the new host?
7. Is the proof frame to the deposed host and its late joiners part of the
   first version?
8. Is a new public page address and a new door after a takeover acceptable, or
   should the upload seed be derived from the governance key so the page
   survives?
9. Is it accepted that a takeover can undo a removal the backup had not
   received, with status showing which removals the backup holds?
10. First version with one backup host only, thresholds among named people as
    a second step?
11. Model in TLA before the chain builder changes?
12. At creation, is "nothing named, the goal dies with this computer" the
    default with a warning, or must a backup be named before a goal is
    published or opens a door?
13. Does the owner have any figure for how often two of their own computers
    are on together? It decides how useful "name your second computer" is as
    the standard advice.
14. Is a catch-up gate after a restart acceptable, given that it delays joins?
    May it use a timer that only delays signing, or must it wait for an
    exchange or a person?

## 11. Limits

Read: the note was written from the sixteen source outputs of the design
round. The joinable farms plan, the shared file tree plan, the formal model
guides and the role-free board note
([role-free-board-2026-10-05.md](role-free-board-2026-10-05.md)) are cited
through those outputs. A checking pass read the eight reviews and the record
and fault groundwork in full, the two surveys and the four designs in part,
the characterization note and the "Design still open" section of the roles
plan. It re-read the code for the successor index, the projection order, the
ancestry walk, the signing guard and the second rejoin test, and confirmed
that every named test exists. The fault groundwork read the code at `cfb5b45`.
One fit review names `7bcdbbc` and another says it landed mid-read. The rest
read the live checkout without naming a commit, and other sessions were
editing it throughout. The classical groundwork read fifteen primary sources
in full text, two of them from course mirrors. The peer-to-peer groundwork
read thirteen systems at source and four from memory, marked in section 3. No
fit reviewer read the two surveys in full: two read the summaries, one skimmed
them and one did not read them. Two say they did not check the designs'
literature attributions, and no review reports checking them.

Run: nothing by the design agents or the reviewers, and nothing for this note.
The eleven tests in the characterization note were run by its author on the
pinned toolchain and are reported there as passing. The availability figures
in section 4 are a binomial calculation in the scratch area, not a
measurement. No availability figure exists in the repository and no
two-machine run with sleep and wake has been recorded.

Inferred, not shown: that stranding extends beyond the three descendants the
test exercises (read from the code, not executed), and that a peer
under-reporting its frontier cannot be detected (read, not tried). Every size
estimate is a judgement. Every counterexample is a hand trace against a
design's rules and the chain builder as read, not a model check. No reviewer
verified the literature mappings inside the designs. Not established: that a
restored daemon always forks. The tests show it forks only if it signs a
different record before catching up.
