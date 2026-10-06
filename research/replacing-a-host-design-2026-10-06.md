# Replacing a host: a design for the first version, and what its check found

Status: research finding, 6 October 2026. A proposed design. Not accepted,
and nothing here was built or run. Replacing a host comes after
[Locust v2](../docs/master-plan.md). This note follows
[the first design round](replacing-a-host-2026-10-05.md), which ended with a
skeleton and competing fixes for its hardest problems.

One designer wrote one design for the first version: one optional backup
host who can take over alone. Two readers attacked it, one for safety and
agreement and one for fit with the code and the other plans. Between them
they reported 30 breaks, two of them fatal for the draft. The designer
revised the design. A fourth reader checked the revision against the code
and judged it "not ready to become phases yet, but close": no fatal break,
eight serious ones and four minor groups, each with a named fix. **Those
fixes are not applied in the text below.** The draft, both attack reports and
the check are kept whole in the
[evidence folder](evidence/replacing-a-host-design-2026-10-06/README.md).

Two things to know when reading it.

- **The numbers of the owner's answers here are not the master plan's.** The
  designers were given an earlier list. Where this note says answer 18 the
  master plan says 12 (the host's removal of a backup wins); 19 is 13 (wait
  for one command after a whole-computer restore); 20 is 14 (a leave removes
  the member automatically); 22 is 19 (a door member may be named with one
  explicit command); 9 is 7 (which commands ask); 12 is 18 (a door member's
  task); 4 and 24 are 9 (the key for members and rules); 15 is 2 (no
  migration); 16 is 3.
- **It predates two answers of 6 October 2026.** The owner said the swarm
  must never stop to ask a human, and restated the door-task answer as
  approval by a trusted agent. Where this note says a door member's tasks
  "ask first", read that rule.

## What it means for v2

- **The takeover record has a shape.** A change of host is one record at the
  first position of a new key's log, naming a base: one record in the old
  key's log. Because v2's K1 puts what the host's computer records by itself
  into that same log, one base cuts off members, rules and recordings
  together. The roles plan's Phases 8 and 9 waited on this. It supports K1's
  choice that the key for members and rules also signs those recordings: a
  second key would mean two cutoffs. That choice is still to be confirmed
  with the contract for what the host's computer signs by itself.
- **It gives the only way out of v2's worst failure.** In v2 a second record
  at a used position of the host's log stops membership and rules for good.
  In this design a host can continue from one branch of its own forked log
  with the same record and command. Until this work is built, v2 has no
  recovery from that failure.
- **It is larger than the v2 count assumed.** By the designer's judgement,
  not measured: 4,500 to 6,400 lines of non-test Rust and 7,000 to 9,200 of
  tests for the first two phases, and 1,000 to 1,400 lines of models. The
  [v2 count](v2-complexity-count-2026-10-06.md) carried 1,800 to 3,800
  non-test lines as a provisional figure.
- **What it asks of the v2 plans** is listed under
  [Changes it asks of other plans](#changes-it-asks-of-other-plans). None is
  applied.

## The design in short

A host can name one member as backup host; nothing asks for this, and a goal
without one keeps working until its host's computer is lost. The backup host's
owner can take over at any time with one command, alone, because a lost
computer and a sleeping one look the same from outside. From then on their
computer keeps who is in and the rules, and whatever the old host's computer
had signed that the backup's had not received is dropped even if it shows up
later: someone it admitted joins again, and a removal it made is repeated by
the new host's computer when it arrives. A host who was only asleep wakes up
as an ordinary member: they keep their work and their copy, their invitations
stop working, their public page goes blank, and the new host can hand the goal
back. One thing of the old host's wins: if, before their computer learned of
the takeover, they removed the backup host or named another or none, the
takeover and everything done under it is dropped when that record arrives;
once their computer has received the takeover it is final, and if that
computer never returns it never becomes final. Goals made before this version
of Locust do not continue under it.

One design for the first version: one optional backup host, revised against
both reviews. Nothing was built or run. Every name taken from the roles plan,
K1, G1/G2 and E1 is plan text. Code was re-read at 1a7c332 (chain.rs and
sync/driver.rs changed since the first draft's dd223af; the rest is the same).

What stands from the first draft: naming is one governance record,
`BackupSet`; a change of host is one record, `HostChanged`, at position 0 of a
new key computed from the taker's agent seed, the goal and the base, so a
repeated takeover is the same bytes; whatever the old key signed outside the
base's ancestry is set aside whenever it arrives; work built on it is excluded
and its author is not stranded; the change record opens a content key;
invitations, door and page do not move.

What changed, and why.
(1) What a computer keeps. A change record is kept whenever its path is held
and it is authorized there, void or not, with its term's records. The first
draft dropped a void one, and arrival order then decided standings (both
reviewers' fatal break).
(2) The host's removal or un-naming of the backup voids a takeover by descent
from the base, not by position. A branch of a forked old log that never held
the base no longer voids anything.
(3) A takeover becomes final. The old host's computer, on receiving a takeover
it has no objection to, signs one record by itself, `HostYielded`. A removal
or un-naming signed after it does not void the takeover, and the guard's marks
then hold a restored old host until that record is back.
(4) Everyone converges. A small claim-and-answer exchange on the fork-proof
route runs in two cases: a computer that is refused by a computer it counts as
a member's asks why, and every computer tells an earlier host that has not
yielded. It is in B1. Two computers that each removed the other now end with
one host.
(5) A host can continue from one branch of its own forked history with the
same record and command. That also lifts K1's permanent halt.
(6) Disputes: a duplicate takeover with nothing signed after it is withdrawn
by the daemon itself; the withdrawal record is fully defined; the texts say
who waits.
(7) The repeated removal computes its own cutoff and is never signed against
the backup host or a role holder.
(8) Ergonomics: `backup set` and `goal give` apply at once with an `Undo:`
line; `--alone` is gone, one yes covers it; `goal keep` un-names.

Cost: four record kinds (one more than the draft), one new exchange with two
frames, two refusal values, the chain builder rewritten around terms. By
judgement B1 is 3,600 to 5,000 production lines and 5,500 to 7,000 of tests,
B2 900 to 1,400 and 1,500 to 2,200; the models are 1,000 to 1,400 lines of
TLA. Not measured.

Stated limits: with a host that never returns a takeover never becomes final;
a removal only the lost computer held is lost; a member that was once named
backup can fill frontiers with void change records; a dispute between two
different people needs one of them to withdraw.

Three phases: B1 (naming, taking over, the whole replay rule, convergence,
protocol 8), B2 (handing over, the host's view of its backup, the old host's
removals, what a replaced host is shown, the page), B3 (qualification and
release). Three models are written first.

## What the last check found

Not ready to become phases yet, but close. The replay rule is sound on the
main paths: a lost host, a sleeping host, a planned handoff, a removed backup
that takes over (S4a, S4b, S4c) all come out as the design says, in any
arrival order, and I found no way for a member the host never named to take,
move or block the seat. I found no fatal break. I found eight serious breaks
and three groups of minor ones. Four of the serious ones need no adversary at
all: `goal keep` does not take the offer back when the other person was
already the backup; `agent revoke` of a new host's agent freezes the goal for
good; the takeover plan's id changes with who has answered, so the yes can
fail in the ordinary case; and the rule that ends the new host's wait after a
restore lets it fork its own log. The other four need a restored copy or a
lost host: a computer that holds the host's removal but is not a member on the
other side can never hand it over, so two honest groups keep two hosts; on a
forked host log a removal on one branch does not stop a takeover from the
other; members who worked in a later term are stranded for good when an
earlier term is disputed and then settled; and two of the three new unattended
signatures can fail inside the landing path, after which that daemon refuses
every batch and does not start.

Check of the 30 answered breaks. Really closed, by the rule as written: Safety
2, 10, 11, 13; Code 1, 2 (on roles Phase 4's sort), 4, 7, 8, 9, 12, 14, 15,
16, 17. Closed for the sequences given but with a hole beside them: Safety 1
(the refused side can only receive, break 1), Safety 3 (the repeat can now
target the host's own agent, break 6), Safety 5 (the release rule for the new
hold is not G1's, break 7), Safety 6 and Code 3 (descent closes S18 and opens
its mirror image, break 2), Safety 7 and Code 5 (one level of dispute only,
break 3; the automatic withdrawal reads what happens to be held, break 9),
Safety 12 and Code 10 (the refusal is by position, and the plan id, breaks 2
and 8), Safety 9 and Code 11 (`goal keep` un-names only when someone else was
named before, break 4), Code 13 (an `Undo:` line is printed for commands whose
effect on a takeover in flight cannot be undone). Renamed, not closed: Safety
4 (a takeover with a lost host is never final; closing the notice route to a
bare removal is what causes break 1) and Safety 8 (the residue is not one
record per base, it is unbounded, and the notice route delivers it for a
removed member, break 10). Code 6 I could not check against farm.rs.

Read in the code at 78438af (docs-only since 1a7c332): chain.rs, history.rs,
screen.rs, goal/mod.rs whole; peers.rs, responder.rs, driver.rs whole;
commit.rs (land, land_once, advance), node/flow.rs (drive_flow), authoring.rs
(sign_at, signer), identity.rs (active), requests/daemon.rs (agent_revoke),
replica.rs (receive, key), sync.rs to the frame checks, limits.rs, event.rs
(Header, check, sign), fold.rs (the decision index, decision, current_round).
Skimmed: invite.rs, requests/invitations.rs. Not read: workspace.rs,
requests/goals.rs, farm.rs, the TLA files beyond their shape. Nothing was run.
Every sequence below is a hand trace against the design's text and that code.

Each break, with the fix the reader named:

**1. Two honest groups keep two hosts for good: a computer that holds the
host's removal of the backup, and is not a member on the other side, has no
way to hand it over. The safety claim 'Over connected computers' and the model
property OneHostWhenQuiet are false as stated. (serious)**

1. Cast S0. g6 = BackupSet(Juniper). A, B, C hold g0..g6.
2. A signs g7 = MemberRemoved(Juniper) and g8 = admits Birch (Dee's ticket). D
   joins and holds g0..g8. A is lost before any other exchange.
3. B hears from C and takes over: T, base (6,g6). B and C compute host Ben.
4. D dials C (Maple is a member on D's chain). C refuses: Birch is no member
   there. D asks: no claim, admission g8. C accepts (g8 is signed by a known
   key) and answers T.
5. D's screen keeps T (path held, Juniper backup at g6). g7 descends from g6
   with no yield, so T is Revoked on D. D's chain stays g0..g8: host Ana,
   Birch a member.
6. D cannot send g7. A notice carries one change record in force on the
   dialer's chain, and D has none. C never dials D: D is no peer of C, and the
   'told' rule dials only the earlier host's computer, which is lost.
7. Quiet: the next answer brings nothing, D stops asking. B and C compute last
   term K1. D computes last term G0. All are honest and reachable. D's status
   shows a goal hosted by Ana in which every computer refuses it; none of B-4,
   B-5, B-6 applies, because on D's own chain its admission is not set aside.
Same shape with Wren's computer W: W received g7 from A, was off at the
takeover, and Ben removed Wren. W asks, learns T, holds it as Revoked, and can
tell nobody.
What each side computes is a pure function of its set; the sets never meet.
Whether the takeover is voided now depends on whether the holder of g7 was
admitted before the base (ordinary sync runs, S17) or after it (nothing runs).

Fix: Give the notice a way back. When the answered change is revoked or
withdrawn on the asker's chain, the asker's next notice carries up to two
records of its reason (the path from the base to the revoking record, or the
withdrawal). The responder lands them when, with them held, the asker's
endpoint speaks for a member. Records of a known governance key pass screen
anyway, so only the acceptance test is new. Say plainly that this lets D void
the takeover (the S17 outcome); the other choice is to state the split as a
limit and give D a text. Add D to the node model.

**2. On a forked host log, the host's removal of the backup does not win.
Revocation by descent closes S18 and opens its mirror image: a takeover from
the branch that lacks the removal is in force, and a present host cannot
contest it once it has arrived. (serious)**

1. g6 = BackupSet(Juniper). A signs g7 = MemberRemoved(Juniper), then g8, g9.
   C holds them. B is refused and never told.
2. A's computer is put back from a copy that holds g0..g6, marks lost, C
   unreachable. Ana runs goal continue. A's daemon signs g7' (an admission on
   a new ticket). C returns: G0 is forked at 7.
3. The fork proof [g7, g7'] goes to every endpoint ever admitted, B's included
   (halt_proofs in crates/locust-core/src/node/peers.rs). B now holds g7 and
   g7'.
4. On B, takeover_bases(Juniper) gives the tip g7': Juniper is the backup
   along g0..g6, g7'. The command refuses only on a removal 'at a position
   after the base'. g7 is at position 7 and the base is at position 7. Not
   refused. Ben says yes, or a modified daemon skips the check: T' =
   HostChanged(base (7,g7'), host Juniper).
5. Every computer: T' is placed and authorized on its own path. No record of
   G0 descends from g7' and removes Juniper. T' is live, alone, in force. g7,
   g8, g9 are set aside. The removed member is a member and the host.
6. On A once it holds T': G0 is not the last term, so takeover_bases gives Ana
   no base and she cannot continue. A cannot revoke (its key signs nothing)
   and cannot yield (marker_place answers none on a forked log). If Ana had
   continued before T' arrived, the term is disputed between two people
   instead, and a hostile Juniper never withdraws. So what the host can do
   depends on which arrived first at its own computer.
7. S18 itself says 'A receives T, is a member, and yields'. A's log is forked
   at 5 there, so by the design's own marker_place rule it cannot yield.

Fix: Choose one rule for a forked old log and model both directions. Either
(a) a removal of the member by the old key revokes on any held branch, unless
a yield for that change is in the removal's own kept log (the owner's rule
first; S18 with a removal then voids, as S16 already does); or (b) keep
descent and state the mirror case as a limit. In both: the command refuses on
any held removal or un-naming outside the base's kept log, not 'after the
base'; and a host whose term log is forked can still continue when a change
from the other branch is already held.

**3. A dispute in an early term strands, for good, members who worked in a
later term. By the definitions, records of a later term built on a disputed
change are 'set aside' (skipped in the walk), not 'undecided' (pending). The
research note names this scenario; the model list does not. (serious)**

1. g6 = BackupSet(Juniper). A is lost. B takes over: T1, base (7,s7), key K1.
   B signs h1 = BackupSet(Maple). B is lost. C takes over: T3, base in K1's
   log, key K3.
2. Wren (admitted at g5, signed nothing in term 1) posts w9 anchored at a K3
   record, chain position 60.
3. A copy of B's computer from before T1 is started, overridden twice, and
   signs T2 = HostChanged(base (6,g6), host Juniper), key K2. It reaches
   Wren's computer.
4. There: two live changes on term 0. HostDispute, chain g0..g6. T1, T2 and
   the records of K1 and K2 are undecided. K3's records fall under 'any record
   of the key of a change record ... whose path holds a change that is not in
   force': set aside. So w9 is Excluded(SetAside) and skipped.
5. Wren's agent posts w10. The daemon anchors at the head g6 (position 6). The
   walk skips w9. w10 is effective and is signed.
6. The copy withdraws T2. T1 and T3 are in force again. w9 is effective at
   position 60. w10 is anchored at 6 with an ancestor anchored at 60:
   AnchorRegressed. Every later record of Wren walks through w10 to w9 (the
   early exit in authorize_base stops only at an effective ancestor) and is
   excluded. Wren's key is dead in this goal.
This is the stranding the design rejected 'let members go on signing at the
common part' to avoid.

Fix: Undecided must cover every record of every key whose path holds a
disputed change. Set aside is only for a path that holds a revoked, withdrawn
or unauthorized change, or one that lost to a change in force. Add the
scenario 'a dispute in an early term after later terms exist, settled each
way, with a member who signed only in the later term' to HostChange.tla.

**4. `goal keep` does not take the offer back when the person being given the
goal was already the backup, which is the ordinary handoff. The text printed
says the opposite. (serious)**

1. g6 = BackupSet(Juniper), signed weeks ago.
2. Ana runs goal give --member Juniper. Juniper is the backup, so no record is
   signed. Local record g = Giving { to: Juniper, earlier: Some(Juniper) }.
3. Ben runs goal takeover while A is off for a minute: T, base (6,g6).
4. Ana runs goal keep. The rule is 'one BackupSet naming earlier': g7 =
   BackupSet(Juniper). B-8 prints 'You keep hosting ... Juniper is no longer
   its backup host. A takeover Juniper's owner made without having received
   this is dropped.'
5. g7 names the same member. It is not a BackupSet 'naming anyone else or
   nobody', so it revokes nothing. T is live on every computer. When A
   receives T, G0 is no longer the last term and A yields. Ana was told she
   keeps hosting.
Without step 3 the text is still wrong: Juniper stays the backup.

Fix: When earlier equals to, keep signs BackupSet(none) and then
BackupSet(earlier). The first voids a takeover in flight by descent; the
second names the backup again from a new base. Or make goal give always sign a
naming record of its own. Fix B-8 and B2's third exit drill to cover an
already-named backup.

**5. After a takeover or a continuation, disconnecting the host's agent
freezes the goal for good. K1's promise holds for the first term only, and
E2's revoke plan tells the person the goal keeps running. (serious)**

1. B takes over. The term's key is computed from Juniper's seed. B1 says
   hosts(entry) is true 'where a local active agent is the last term's host's
   agent'.
2. Ben switches coding agent and runs agent revoke --agent juniper. The plan
   (E2's words) says inviting, removing and rule changes need no agent.
   agent_revoke in crates/locust-core/src/node/requests/daemon.rs sets
   revoked; active() in identity.rs then answers none. The seed stays on disk.
3. hosts(entry) is false. host() refuses every host request; Host::join and
   drive_flow get no key from signs_alone. Nobody is admitted or removed,
   rules never change, and from R8 and R9 the plan and the files stop.
4. No command undoes a revoke and the name cannot be enrolled again. Unless
   Ben had named a backup, nothing unsticks it.

Fix: hosts() and key_for read the seed of the term's host's agent whether or
not that agent is disconnected. Only goal.takeover asks for an active agent.
Add E2's revoke plan and K1's test
`revoking_the_hosts_agent_stops_that_agent_and_not_governance` to the changes,
run for a later term.

**6. Two of the three new unattended signatures can fail inside the landing
path. After that the daemon answers every received batch with a protocol error
and does not start. G1 found this for a held step and added a test to
drive_flow; this design adds acts without one. (serious)**

Read in crates/locust-core/src/node/commit.rs: land() runs drive_flow(goal)?
after the commit, and advance() answers Conflict for an authored record that
is not effective. replica.rs maps a failed land to Refusal::ProtocolError.
Node::open runs drive_flow for every goal.
The repeat (B2):
1. S18 with a removal: A sleeps; B takes over, T, base (6,g6). A is put back
   from a copy at g4, overridden, and Ana retypes: g5', g6', g7' =
   MemberRemoved(Juniper, admission g3). A reconnects.
2. g7' does not descend from g6, so T stays in force and g7' is set aside. B
   holds it.
3. B2's rule matches: a held set-aside removal by an earlier term's key whose
   member is still active under the admission it names. Juniper is not the
   backup and, under peer-review, is in no role list. The two listed
   exceptions do not apply.
4. drive_flow signs MemberRemoved(Juniper) with K1. K1's replay rule excludes
   a removal of the term's host's agent. advance() answers Conflict. land
   fails. The set-aside record stays held, so every later landing and every
   start fails the same way.
The yield (B1): rule (1) in node/flow.rs has three conditions and none asks
that marker_place answers a place. On an old host whose own log is forked or
has a gap (S15, S18) it answers none.

Fix: One rule for every unattended act: it is passed over, never an error,
unless the record would be effective here and a place exists. For the repeat,
add the term's host's agent to the members never removed, and report it in
not_repeated. For the yield and the withdrawal, require marker_place to
answer. Add a node test that lands each of these states and restarts.

**7. The hold on a new host's key after a restore ends too early. Liveness
says it ends 'once any other member's computer answers twice'. G1's rule for a
host's computer is every other computer. With 'any', a restored new host forks
its own new log. (serious)**

1. B hosts term 1 and signed h1..h43. h5 admitted Robin (computer D). C was
   off since h20 and holds h1..h20. D holds h1..h43.
2. B's whole computer is put back from a copy made before its takeover. The
   copy lists Harbor, Maple, Wren; it does not know D.
3. B hears from C: T and h1..h20 arrive. T names a local agent and no local
   record says this store signed it, so B writes T and UNHEARD for K1. The
   copy's own hold ends when the computers it listed have answered.
4. C answers a second time with nothing new. By the liveness sentence the hold
   on K1 ends.
5. B's daemon signs h21' (a recording or an admission) at position 21. D holds
   h21. K1's log, the last term's, is forked: Halt::Fork. The repair is a
   continuation that drops one branch and the work built on it.
With 'every other computer on the current chain', D is waited for as soon as
h5 is held, and h21..h43 arrive first.
Separately: a change 'signed in a goal that holds RESTORED' gets the hold.
RESTORED stays until the next ordinary start, so a backup whose computer was
restored and caught up weeks ago takes over and then signs nothing as host. In
a goal of two, with the old host lost, there is nobody to hear from and only
goal continue ends it.

Fix: State the rule: UNHEARD on a governance key this daemon holds follows
G1's host row, every other computer on the current chain, less an earlier host
that has not yielded. Write the hold at a takeover only while RESTORED still
says unheard or the taker's own key was released by goal continue.

**8. The takeover's confirmation is bound to who has answered. The yes can
fail in the ordinary case, and any member can keep it failing. (serious)**

1. A is lost. Ben runs goal takeover. The command polls for 20 seconds. The
   dial to A's dead endpoint is still open, or is between two retries of the
   driver's backoff (1 s doubling to 60 s in
   crates/locust-core/src/sync/driver.rs).
2. The plan is shown. Its id covers answered, refused, unreached and waiting;
   B1 has a test that the id changes with them.
3. Ben reads for half a minute and types y. Meanwhile A's dial moved between
   waiting and failed, or Birch's computer answered, or a member's exchange
   brought a record. goal.takeover computes the plan again: 'conflict: the
   plan changed'.
4. Ben runs it again and waits another 20 seconds. A member who wants to block
   the seat opens and drops exchanges, or signs a record every few seconds,
   and the id never holds still.
I could not tell from the text whether a state, once answered, can go back, or
whether waiting and failed are hashed apart. If either is so, this is the main
flow.

Fix: Bind the id to what the yes decides: the base, and the set of computers
that refuse this one. Who answered is shown and not hashed. A pinned test: the
id is the same before and after a dial to a dead endpoint fails.

**9. The automatic withdrawal reads what happens to be held, so batch
boundaries can decide which of two takeovers survives. A withdrawal is for
good, and the computed key then makes that base unusable for that person.
(minor)**

Arrival order:
1. B took over: T1, base (7,s7), 43 records by K1, held by C and by member M.
2. B's computer is put back from a copy at g6, nobody reachable. Ben: goal
   continue, goal takeover (T2, base (6,g6), K2), goal continue again; the
   daemon signs h1' with K2.
3. B reaches M first. M sends s7, then T1 alone in a batch, and stalls.
4. On B: two live changes name local agents; T2 has a record of its key after
   it held here, T1 has none held here. Exactly one has, so the daemon signs
   TakeoverWithdrawn(T1).
5. h1..h43 arrive. T1 stays withdrawn. The week is set aside everywhere the
   marker goes. With T1 and its records in one batch the person would have
   been asked.
Dead base:
1. Two live changes of one term by two people. Each sees B-7 and politely runs
   the printed --keep for the other's. Both are withdrawn.
2. The old log has one tip. goal takeover computes the same key and the same
   bytes, 'answers' the held, withdrawn record, and prints success. Nothing
   changes and nobody can take over from that tip again.

Fix: Withdraw by itself only a change this store signed (local record T) that
has no record of its key after it; that is known locally and does not depend
on delivery. goal.takeover refuses, with the reason, when the record it would
sign is held and not live. Let a withdrawal name the change it gives way to,
so two mutual withdrawals cancel.

**10. The stated limit on change records made by a once-named member is wrong
in size and in reach. It is not one per base: the author key is free. A
removed member can deliver them itself over the notice route, and the goal
then stops syncing for good. (minor)**

1. g6 = BackupSet(Juniper). g7 removes Juniper.
2. A modified daemon makes 4,097 keypairs and signs, with each,
   HostChanged(base (6,g6), host Juniper) with an endorsement over (goal,
   base, that key, Juniper). Header::check passes. Each is authorized at g6
   and revoked by g7.
3. It opens notice exchanges with C. Its endpoint is in held admission g3, so
   each is accepted. C's screen keeps each claim: path held, authorized,
   'whatever voids it'.
4. Ordinary sync spreads the kept records. Each is one author.
   crates/locust-proto/src/sync.rs refuses a Frontier above
   MAX_FRONTIER_AUTHORS (4,096) with LimitExceeded. No two members can
   exchange a frontier again.
Each such key is also a known governance key: what it admits, and what those
keys sign, is kept too.
The fault model takes a named backup as honest. A host that names someone,
finds them hostile and removes them has no defence, and the summary's 'one per
base' understates it.

Fix: Correct the limit and the owner question. A pure bound needs the new key
to be checkable from the held set: one key per naming, either named in
BackupSet or derived from the member's public key by a public tweak, so a
second base is a fork of one author and not a new author. If that is left for
later, at least refuse a notice claim that this computer already holds a
revoking record for, when the sender is not a member.

**11. Meeting points with ENDING, the guard and publication that the design
does not list. (minor)**

a. E2 removes a member whose agent signed a leave, unattended. B1's rule says
no unattended signature removes the backup host. If the backup's agent leaves,
one of the two must give, and an unattended removal of the backup would void a
takeover made from a copy older than the leave. The change is not in
changes_to_other_plans.
b. A new host may publish with no consent from the earlier host's agent. That
also holds when the earlier host was only asleep and the goal was never
public: its work goes on a public page it was never asked about. Owner
question 10 does not say this.
c. `backup clear`, `backup set` to another member and `goal keep` print an
`Undo:` line. When a takeover is in flight their effect is to drop it and
everything under it, and no command undoes that. By the roles plan's own rule
they belong with the commands that cannot be undone with one command, at least
while the backup's computer has not reported holding the latest records.
d. On an undo, members the dropped host removed are members again and are
served the current key; nothing repeats those removals. In the other direction
the daemon repeats them by itself.

Fix: a: add to E2 that a leave by the backup host is shown and not acted on.
b: put the sentence to the owner. c: print no `Undo:` line for these three
while backup_holds.behind is not 0, and say that the effect on a takeover
cannot be undone. d: state it in B-6's caption as a choice, or repeat them
under the same exceptions.

**12. Smaller gaps in texts and rules. (minor)**

a. No status line for a goal being handed over. While g exists nobody is
admitted and nothing is recorded, joiners are told host_changing for ever, and
plain status says nothing.
b. Every computer dials a removed earlier host until its yield is held. A lost
or stolen computer never yields, so every member's daemon contacts that
endpoint for good.
c. A computer told of the first change only (S8's D after a second change) is
told to ask a host who is no longer the host.
d. B-7's second form does not say that the backup who gives way to a host's
continuation can take over again from it (a continuation keeps the backup).
e. The yield's at_ms is not fixed. A yield signed twice from one state should
be one record, as the change record is.
f. A change record's author is not required to be new: it may be a key already
admitted as a member.
g. 'A handoff drops nothing' holds only if the giver's computer answers during
the taker's 20 seconds.
h. The plan says the computer continues from the last record it holds; with a
gap in the old log it holds later records it cannot place, and drops them.
i. Whether receive_notice screens the claim before it tests the sender
matters: a computer admitted under a term the responder has not seen is
accepted only if the claim is kept first.

Fix: One sentence or one condition each: a `Handing over to NAME` fact line
with the keep command; only the new host's computer tells, and it stops after
`kept: true` plus a held yield or a fixed number of answers; at_ms 0 for
markers; a live change's author is no member and no earlier governance key at
the base; say 'if this computer answers' in B-8; count unplaced later records
in the plan; screen the claim first.

What the reader could not break:

- A member the host never named cannot make a change record that any computer
  keeps. The endorsement must verify under a key that is the backup or the
  host's agent in the snapshot folded along the record's own path, and screen
  applies that exact test. I tried a self-naming log, a forged endorsement and
  a base before the naming.
- Whatever the old key signed outside the base's ancestry is set aside in any
  arrival order, and a fork of the old key at or below the base does not move
  the kept log. The kept log is read by `prev`, which is signed.
- S4a, one level: with the void change and its term kept, A and C give every
  record they both hold the same standing whichever of T and g7 came first,
  and Maple's next record at g7 counts. The walk in authorize_base needs only
  the skip rule.
- S4b and S4c: a removed backup that takes over is told by the first computer
  that holds the removal; two computers that each removed the other end with
  one host, with three computers, with two, and with the backup lost. The
  'told' rule reaches the earlier host in each.
- A revoked change never comes back. The revoking record's path is fixed by
  hashes and must be fully held, so no later delivery can put a yield between
  the base and it.
- The yield: a restored old host with its marks kept is behind until the yield
  is back, signs no second yield while held, and a record it signs after a
  yield in its own history does not void the takeover.
- Taking over twice from one base is one record. Read in the code: sign_at
  sets no parents, sealing derives its nonce from the content, the event id is
  the hash of the header bytes only, and the signature is outside the id.
- The takeover opens a content key the new host holds by construction, so S7
  holds: members write text again when the old host held the only copy of the
  newest key.
- During a dispute no key opened under either takeover is served and a member
  removed under either is refused. After an undo, keys opened under the
  dropped term are not served.
- The end rule, all three cases of S9, including an end set aside by a
  takeover and a takeover whose kept log holds an end.
- Recordings across a takeover: a set-aside decision is not a successor
  (decision() in fold.rs counts only effective candidates), the new key's
  decision follows the last kept one, and under roles Phase 4's sort it is the
  head.
- A hostile member cannot forge a yield, a withdrawal, or a removal for the
  new host's daemon to repeat; each needs a governance key of a term on the
  chain.
- The skip rule gives a member no way to anchor further back than its own
  effective records allow.
- Protocol 8 and the texts of B-9: a home, a peer and a ticket from 7 are
  refused and the home is not touched.

The reader's recommendation:

Smallest changes that make it sound, in the order I would apply them.

1. Definitions (B1, and HostChange.tla first). Undecided covers every record
   of every key whose path holds a disputed change. Add the two-term dispute
   scenario, settled each way.
2. Forked old log. Decide whether a removal of the backup on any held branch
   revokes (my recommendation, with the yield read on the removal's own kept
   log) or only by descent, and write the losing sequence as a limit either
   way: S18 or its mirror. In both cases the command refuses on any held
   removal or un-naming outside the base's kept log, and a host whose term log
   is forked can continue although a change from the other branch is held.
3. Notice. Let the refused side hand over its reason, two records per
   exchange, accepted when with them it speaks for a member. Put the
   consequence to the owner together with question 1: this and S17 are the
   same fact, that answer 18 has no end when the host never returns. If the
   owner prefers the takeover to stand in that case, the rule to offer is: the
   removal wins only when the host's computer itself says so after learning of
   the takeover (a marker that mirrors the yield), with the cost that a
   removed backup hosts until the host returns.
4. Unattended acts. One guard for all of them: passed over unless the record
   would be effective here and a place exists. Never repeat a removal of the
   term's host's agent. Fixed time 0 on markers.
5. `hosts()` and `key_for` do not ask that the host's agent is still
   connected.
6. `goal keep` signs BackupSet(none) first when the earlier backup is the
   person given to.
7. The hold on a new host's key ends by G1's host row, every other computer on
   the current chain; it is written at a takeover only while the restore is
   still unresolved.
8. The plan id covers the base and the refusals, not who answered.
9. The daemon withdraws by itself only a change this store signed that has
   nothing after it. `goal.takeover` refuses when the record it would sign is
   held and not live.
10. Correct the stated limit on change records by a once-named member
    (unbounded, deliverable by a removed member) and the owner question with
    it.
11. Add to changes_to_other_plans: E2's leave of a backup host, E2's revoke
    plan, and the tier of `backup clear`, `backup set` to another and `goal
    keep`.

For the owner, beyond the designer's sixteen questions: whether a new host may
publish a goal with the sleeping earlier host's work on the page and no
consent from them; and, under question 8, that a door member who was named and
then removed can, until the removal reaches a member's computer, write tasks
that run without asking there. That window argues for keeping the door mark.

To the models: add a holder of the revoking record that is not the earlier
host; the mirror of S18; the two-term dispute; a restored new host whose later
records sit with a member admitted after the copy; and a step for each
unattended signature that checks it cannot fail.

Not verified by the reader:

- Break 8 rests on how Node::asked moves between waiting, failed and answered,
  which the design does not say. If a state never goes back once set and
  waiting is not hashed apart from failed, only the hostile variant remains.
  One sentence in B1 settles it.
- Break 3 rests on reading the definition of 'set aside' by its letter. The
  phrase 'the walk has decided against it' may be meant to exclude a path with
  a disputed change. The model scenario settles it.
- Break 6: that the repeat is signed without a check that the removal would be
  effective. The design lists two exceptions and no general test. If B2 routes
  it through member_remove's refusals and treats a refusal as a skip, only the
  yield case remains.
- Break 2, step 3: that the fork proof for a governance key of the last term
  is pushed to the removed backup's endpoint. Read in halt_proofs today; B1
  keeps it for the last term's key.
- That honest sync never delivers a change record without the later records of
  its key. One author's log travels in order, so I expect a batch boundary
  only on a log longer than 256 events or from a member who cuts it on
  purpose.
- Whether a Notice from an endpoint the responder does not know fits the 4 KiB
  limit that applies before the sender is identified. A change record and an
  admission are a few hundred bytes each, so it should; a long member name or
  a large header would not.
- The consent rule for a page published after a change of host. I did not read
  farm.rs.
- Whether roles Phase 4 lands the decision sort by anchor position first. The
  order of decisions across terms rests on it.
- How members' connected folders behave when the head of the shared files
  moves back at an undo. The design leaves it open and I did not read
  workspace.rs.
- Everything about K1, G1, G2, E1 and R1 to R10 is plan text. I checked the
  design against that text and against today's code, not against a tree that
  has them.

## The choices

One for each problem the first round left open.

### Who picks the base, and how a renamed or removed backup is stopped (research section 7, first problem; items 1 and 4)

**Chosen.** The taker's daemon picks the base: a branch tip of the last term's
governance log, the only tip when that log has one branch. Validity is a pure
function of the held set. A change record is authorized when, folding its own
path up to the base, the member it names is the backup host (or the host's
agent, for a host continuing its own history). It is revoked by a record of
the old key that descends from the base through held `prev` links, takes
effect when folded from the base, and removes that member or is a `BackupSet`
naming anyone else or nobody. It is sealed against such a record when a
`HostYielded` for it lies between the base and that record. The old host's
daemon signs the yield by itself when it holds the change in force and holds
nothing that revokes it.

**Cost.** A takeover every computer treated as in force is undone when the old
host's record surfaces, until the old host's computer has received the
takeover. With a host that never returns it is never final. Descent needs the
links, so a computer that sync does not reach gets them two records per
exchange. One more record kind and one unattended signature by the old key. A
whole-computer restore of the old host from a copy older than its yield, with
nobody reachable and the guard overridden, followed by an un-naming typed
there, still voids the takeover.

**Rejected.** The takeover stands (the owner decided the removal wins). Read
the backup only at the base (a stale copy of any once-named key acts for
ever). Refuse to remove a member while it is named (un-naming has the same
race). Acknowledgement by electors or a second signature (set aside by the
owner). Revocation by position, the first draft: a record on a branch of a
forked old log that never held the base voided a takeover in force and left no
host and no way to take over (the code reviewer's sequence). No yield: a host
restored from a copy older than its receipt of the takeover voided three
months with `backup clear` (the safety reviewer's sequence). Any yield seals,
whatever branch it is on: a revoked takeover could then come back into force
on a later delivery, so revoked would not be stable.

### How the new governance key is made (item 3)

**Chosen.** Derived: seed = BLAKE3 derive_key('locust v0 governance key',
agent seed, goal, base position, base id), in the form of the existing domain
strings. The content key the record opens is derived from that seed. The
payload is the empty text, as a removal's is. `at_ms` is 0. Sealing is
deterministic in today's code and the event id is the hash of the header bytes
only, so the same agent taking over from the same base makes the same record
on any copy of its data. No seed is stored for a later term: `Node::key_for`
computes it from the local agent's seed and the held change record. K1's
record `K` and `Local.governance` stay as K1 writes them.

**Cost.** The governance key of a later term is as strong as its host agent's
seed and no stronger; both sit in the same data folder with no passphrase. The
record carries no time, so others show when they received it. Byte identity of
the header rests on the endorsement, an Ed25519 signature, being the same on
every run; a frozen vector and a two-store test pin it.

**Rejected.** A fresh random key: a backup restored from a copy older than its
takeover has lost the key for good, and every repeated takeover is a second,
different record. One key per member and term without the base: two takeovers
from two bases would be two records at position 0 of one log, and the author
log cannot hold two branches from position 0.

### A dispute that must not be permanent (research section 7, second problem; item 5)

**Chosen.** Same base gives one record. Two different live change records on
one term dispute it: the chain stops at the part both keep, no governance key
signs, and every member who signed under either one waits. It ends when all
but one are withdrawn. `TakeoverWithdrawn { change }` is a marker signed by
the change's own key: its anchor is the change's anchor, it is signed through
`Goal::marker_place`, not `Goal::next`, and it is effective when the change it
names is held, placed and by the same key. A daemon that can compute the keys
of two live changes of one term, where exactly one has records signed after
it, withdraws the others by itself. Otherwise status prints one command.
Separately, a fork inside the last term's own log is repaired by the same
change record: the host's agent, or the backup, continues from one branch.

**Cost.** One marker kind and one unattended signature. During a dispute
nobody who signed under either term can sign anything, and members admitted
under either are refused until it settles. Between two different people one
must give way; if that person is gone it stays halted. A member once named
backup who takes over years later from an old copy, past two overrides,
suspends every later term until they withdraw.

**Rejected.** The earlier base wins (a late record voids a settled week,
silently). The deeper base wins (an honest stale copy flips a settled change).
A takeover that lists the ones it supersedes (a third copy disputes it again).
Let members go on signing at the common part during a dispute (after it
settles, their earlier record sits at a higher anchor than their later one,
which is the measured stranding). A host's own change outranks a backup's on
the same term (a stale restored host could then void a backup's week with a
repair).

### Removals a takeover has not seen (research section 7, third problem; item 6)

**Chosen.** Four parts. Before the plan is shown, the backup's daemon asks
every other computer, the host's included, and continues from the union of
what they hold. The host is told what its backup holds, read from the frontier
the backup's computer itself reported. After a takeover, when the new host's
daemon holds a set-aside removal signed by an earlier term's key on its chain
and the member is still in under that admission, it signs a removal itself,
unasked, with a cutoff it computes on its own chain as `member_remove` does
today. It does not when the member is the backup host or holds a role given in
the current term; that case is shown as a fact with the remove command. The
takeover record opens a new content key.

**Cost.** A removal that only the lost computer held never arrives: that
member is in again and reads everything, and nobody is told. What the member
did between the takeover and the repeat stays. A host that has been replaced
and has not learned it can still remove members, and the new host's computer
repeats those removals.

**Rejected.** A removal is final only once acknowledged (a second signature).
Refuse a takeover until every member is reached. Offer the removal under
Waiting for you (a prompt, and reading stays open meanwhile). Copy the old
removal's cutoff, the first draft: its cutoff record can be anchored at a
set-aside record, the cutoff is then judged malformed and every record the
member ever signed is dropped (both reviewers). Limit the new cutoff to the
old cutoff's ancestry (it would retract approvals that the new host's computer
had already recorded a file change on).

### The cutoff being any record of the old signer; what a takeover sets aside (research section 7, fourth problem; item 11)

**Chosen.** `base` is an `AuthorPoint` in the old governance key's log and may
be a host step. The anchor is the last governance record at or before it. Set
aside is every record of the old key not reached from the base by `prev`. A
host step counts only if its signer is the governance key of the term its
anchor is in and it is in that key's kept log. A set-aside decision holds no
place after its predecessor. A stream's decisions are ordered by anchor
position first, which roles Phase 4 already gives `project`; a new key's step
is anchored at its change record or later, so it sorts after every kept
decision of the old key. `current_round` picks a derived task's opening step
by anchor position first for the same reason. The new host signs nothing but
the change record.

**Cost.** A plan text recorded after the base is recorded again by itself. Of
file changes recorded after the base, the first lands again by itself; one
built on it names the dropped acceptance as its parent and must be rebuilt and
approved again. A task opened by a set-aside stage step is opened again as a
new round and work on the old round is lost.

**Rejected.** The base must be a governance record (every recording since the
last admission is lost at every takeover). A second cutoff for the host
agent's key (K1 put host steps in the governance log). A mandatory rules
binding and tree epoch as the new host's first acts (every open revision and
proposal would be posted again; the ordering they were meant to fix is fixed
by the sort). The usable tip as the only base on a forked log (one stray
record signed by a restored host would drop the whole branch beside it).

### Telling the deposed host and whoever it admitted afterwards (research section 7, fifth problem; item 8)

**Chosen.** A notice exchange on the fork-proof route: the dialer sends one
change record it treats as in force (or none), the responder keeps it if it
passes the exact test and answers with at most two records that are its reason
to disagree, or with the next change on its chain, or with the yield. Two
cases open one. A computer that an endpoint it counts as a member's refuses as
not a member asks that endpoint. Every computer that holds a change in force
with no yield tells the computer of the earlier host's agent when that agent
is no longer a member. A computer the responder does not know may show its own
admission record. A computer none of whose agents is a member or joining stops
dialing.

**Cost.** Two new frames and one refusal value. A computer admitted under a
takeover that was later dropped is shown the old host's records between the
base and the record that dropped it. Anyone ever admitted can learn the keys
in the change records. A computer that missed two changes learns the first,
which is enough to stop it. A computer that is off learns nothing until it is
on. A removed member that no change concerns is still never told.

**Rejected.** Push the change record to every endpoint ever admitted, once,
with a delivered mark (the first draft): it reached a responder that held the
voiding record and stopped there, so two hosts never converged; it marked
delivery when the receiver had dropped the record; and it opened standing
pairs to every removed stranger. Serve whole governance logs to every computer
ever admitted. Refuse to remove the previous host until it was told (a lost
computer never answers).

### Catching up before signing after a restore (research section 7, sixth problem; item 3)

**Chosen.** G1's guard as built, with five uses. The takeover command refuses
while `Node::hold` holds the agent's key. It refuses when the marks name, in
this goal, a governance key whose change record the store does not hold: this
computer signed as host here before. A daemon that finds itself host through a
change record it has no local record of signing writes G1's `UNHEARD` for that
key. A change signed in a goal that holds G1's `RESTORED` record gets
`UNHEARD` for its key in the same commit. The yield gives the old key a mark,
so a restored old host is behind until the yield is back. `guard_sources`
leaves out the computer of an earlier host's agent that has not yielded.

**Cost.** After a whole-computer restore with nobody reachable, a person who
overrides twice can still take over from another base, and one who overrides a
third time can fork the new key's log; the first is the dispute, the second is
repaired by continuing from one branch. A wake from sleep still costs nothing,
so a host that was asleep can sign before its first exchange.

**Rejected.** A timer that delays signing. A new hold reason for handing over
(the code reviewer: G1's readers would print the restore sentence). Emptying
G1's heard set for the plan (it set back holds waiting on it).

### Content keys across two signers; a fresh key after a takeover (research section 7, seventh problem; item 7)

**Chosen.** Keys are stored by the id of the record that opened them (first
record, removal, or change of host), and `KeyRequest` and `Key` name that
record. `HostChanged` opens an epoch like a removal. A key is served and
wanted only when its opening record is effective on this computer's chain.
Keys opened by set-aside or undecided records stay where they are held and
readable there. While a term is disputed, an endpoint speaks for a member only
if no held record of a disputed change's key removes that member.

**Cost.** Two sync frames and the key store's layout change. Every takeover
costs one epoch. Rules whose text was sealed under a key only the old host
held cannot be read by anyone; the plan says so and the new host binds rules
again.

**Rejected.** Drop a set-aside epoch's key (authors lose their own text). Keep
the bare epoch number on the wire (after an undo two keys share a number).
Serve any held key to any current member, as today (during a dispute a member
removed under a disputed term is a member again and would get the keys opened
after its removal).

### Public page, door and invitations (research section 7, eighth problem; item 10)

**Chosen.** Nothing moves. A change of host to another person clears the
publication on the chain; a host continuing its own history keeps it. A ticket
gains `issuer`. The old host's daemon, on learning it was replaced, revokes
its pending invitations; its page goes blank and it can delete it. The new
host invites, opens a door and publishes again. For a page under today's
consent rule, an agent that was the host's agent of an earlier term is not
asked for consent to a publication record signed in a later term and is shown
as 'earlier host' unless it consents. For a joinable page under the
public-goals contract the new host's publication record is the consent and
nobody consents again.

**Cost.** A new address; under today's rule every other member consents again;
the gallery listing and the alias are asked for again. The page of a host that
is truly gone keeps its last content, with a Join band that reaches nobody,
until the farm service removes it.

**Rejected.** Derive the upload seed from the governance key (that key does
not move either). Carry the seed in a bundle with the backup. Let the farm
service move a page from the signed chain (a later step). Keep today's consent
rule unchanged (the lost host's agent wrote the first files and can never
consent, so the new host could never publish; the code reviewer's sequence).

### The planned handoff (research sections 6 and 8; item 2)

**Chosen.** The same record and the same rule. `goal give` applies at once: it
names the other member as backup if they are not, and writes one durable local
record, so this computer's governance key signs nothing but markers and `goal
keep` until the other person takes over. The other person runs `goal
takeover`; the giver's computer answers, so nothing is dropped; it then yields
by itself. `goal keep` signs `BackupSet` back to the earlier value, which
voids a takeover in flight.

**Cost.** Two more commands and one local record. If the other person never
acts the goal admits nobody and records nothing until `goal keep`. A giver
restored from an older copy loses the local record and signs again; what it
signs after the base is then dropped at the takeover.

**Rejected.** A handoff record in the old host's log (a restored old host
forks at it). The old host's signature inside the change record (a second way
to be valid). No quiet (an admission in the last second is dropped). `goal
keep` that only ends the quiet, the first draft (the other person could still
take over later, both reviewers).

### The dedicated signing key across terms (research section 7, ninth problem; items 13 and the three plugs)

**Chosen.** The governance key is a function of the chain: `Chain::build`
walks terms, and a snapshot carries the governance key, the host's agent and
the backup of its term. K1's two replay rules read the term. After a change to
another person the old host's agent is an ordinary member and can be removed.
A role list whose only holder was the old host's agent passes to the new
host's agent. The same record lets a host continue from one branch of its own
forked log, which K1 left as a halt for good.

**Cost.** The chain builder is rewritten, with the tests that pin today's fork
behaviour. Each kept change record adds one author to every frontier.

**Rejected.** Copying the key to another computer. Keeping roles with a lost
host's agent.

### What a computer keeps (research sections 2 and 9: retention decides what is held, and what is held decides every standing)

**Chosen.** `screen` keeps a change record exactly when its path is held and
it is authorized along that path, whether or not something voids it. It keeps
the records of every key such a record starts a term for, and of every key
those keys admitted. A change record whose path is not yet held is dropped and
comes again with the next exchange. Retention is in the replay model.

**Cost.** One fold of a held log per new change record. A void takeover's
whole term stays on every member's disk. Only a member the host really named
can make a kept change record, but such a member can make one per base and
each is an author in every frontier, which holds 4,096; removing the member
does not stop it. The fault model names a named party as honest; no rule
bounds it in this version.

**Rejected.** Drop a change record that a held record already revokes (the
first draft: whether a computer held the void term then depended on arrival
order, members who worked under it were pending for good on the old host's
computer, and that computer had nothing to tell the taker). Keep any change
record whose host some held `BackupSet` names (a hostile member could name
itself in its own log and stop all sync at the frontier limit). A count limit
per member and term (which records survive would be decided by arrival order
or by lowest id).

## Safety and liveness as the design claims them

The check found the claim "over connected computers" false as stated (its first break).

**Safety.** Over held sets. Who hosts, the chain, every standing and the
current content key are one function of the set of signed events a computer
holds. Two computers that hold the same set compute the same answer, in any
arrival order. No clock, timeout, arrival order or identifier comparison
enters. Revocation is read by `prev` descent, which is signed.

Over what is kept. Whether `screen` keeps a record depends only on the record
and on the held set, and a record it once kept it would keep at any later
time. A change record is kept when its path is held and it is authorized along
it, void or not, with every record of its key and of the keys that key
admitted. So two computers that have sent each other all they are willing to
send hold the same records of every term, and give every record they both hold
the same standing. The one known gap in this claim: it is argued, and modelled
for the scenarios listed, not proven for the code.

Over deliveries. Let a computer hold a set in which change record c is in
force, and let one more event arrive.
1. c is still in force, unless the event completes one of four things: (a) a
   record of the previous term's key that revokes c, with every link from it
   back to c's base, and no yield for c between; (b) a `TakeoverWithdrawn` for
   c by c's own key; (c) a second live change record of the same term, which
   makes the term disputed and puts nothing else in force; (d) any of these
   for a change of an earlier term that c is built on.
2. No different change record of the same term comes into force while c is
   live.
3. While c is in force, no record of the previous key outside c's kept log is
   effective, whenever it was delivered, and no record anchored at such a
   record is effective or pending.
4. A record of the previous key that does not descend from the base, at any
   position, changes no standing of c.
5. A change record whose kept log holds an end is never in force. An end
   outside the kept log does not end the goal while the change is in force.
6. Only the member the old key named as backup at the base, or the term's own
   host's agent, can produce a change record that any computer keeps or
   counts.
7. A change that was revoked or withdrawn is never in force again, whatever
   arrives.
8. If the held set holds a yield for c, a revoking record can only be one that
   the old key signed on another branch than the yield's, at or after the
   base. An honest old host's computer signs such a record only after a
   restore that lost its marks, or with the guard overridden.

Over connected computers. Let two honest computers each be named in an
admission the other holds or is shown, and let them exchange until nothing
changes. Then they compute the same last term. The ordinary exchange does it
when each counts the other a member. When one refuses the other, the refused
one asks and is answered with the reason. When neither dials the other, the
one that holds a change in force tells the earlier host, and is answered with
what revokes it. This is a property of the node model, not of the replay rule.

Exactly when a takeover a daemon treated as in force is undone: when that
daemon comes to hold (a), (b) or (d). Under (c) it is suspended, not undone.
Nothing else undoes it: not a fork of any earlier key, not an old-key record
on another branch, not any other late record of the old host, not a restart.
Once the old host's computer has received the takeover and yielded, (a) needs
that computer to have been restored.

What an undo does. Every record of the void change's key and of every later
term's key is set aside: the admissions (those members are not members), the
removals (those members are members again; status on the returning host lists
each with the remove command), the rule and role changes, the plan and file
recordings. Work anchored at those records is excluded; its authors' next
records at kept anchors count. The old key is the last term's key again and
its records after the base are effective again, an end among them. Content
keys opened under the void term stay on the computers that hold them and are
no longer served. A member's local working files when the files head moves
back are not designed here.

What is not safe, and is said in the texts. A removal held only by a lost
computer is lost. A takeover is not final while the old host's computer has
not received it, and never if that computer does not return. The daemon rules
(ask the other computers; the guard; never withdraw the last live change;
never an unattended removal of the backup) are aids that a modified daemon can
skip; validity does not rest on them.

**Liveness.** Naming, changing and clearing the backup: only the host's
computer must be on; one command each, no question.

Taking over: the backup's computer must be on and its person runs one command
and says yes once. The command waits until each other computer has answered,
refused or failed once, at most 20 seconds of its own clock. Nobody need
answer: the plan says who did. The old host need not be reachable. A member's
computer follows the new host as soon as it syncs with any computer that holds
the record.

After a takeover: members fetch the new content key from the new host's
computer or from any member that has it. The new host's computer must stay on
for admissions and recordings, as the old one had to. The old host's computer,
when it is next on, yields by itself.

When the goal is stuck, and how it gets unstuck.
- No backup named and the host's computer lost: nobody joins or is removed and
  the rules never change. Tasks, results and approvals go on, unless the last
  removal's content key reached no other computer, in which case nobody can
  write text either (measured today). Nothing unsticks it. This is today's
  behaviour and the price of naming being optional.
- The backup's computer is lost too: the same.
- A takeover was voided by the old host's record and that host's computer is
  lost: the same, and what was done under the takeover is dropped.
- The backup's computer is catching up after a restore: it waits for its own
  records, or for `goal continue` (G2).
- A new host's key is waiting to hear after a restore: it has heard once any
  other member's computer answers twice; an earlier host's computer that has
  not yielded is not waited for; with nobody to ask, `goal continue`.
- Two takeovers dispute a term: governance is halted and everyone who signed
  under either waits, on every computer that holds both. When both are one
  person's and one is empty, that person's daemon withdraws it with no
  command. Otherwise that person runs the command status prints. Between two
  different people the one who gives way runs it; if that person is gone it
  stays halted.
- The last term's governance log is forked (a restored host that continued):
  halted until that term's host, or its backup, continues from one branch with
  `goal takeover`. Today and under K1 this is for good.
- The current rules cannot be read after a takeover: the new host binds rules.
- A handoff that the other person never completes: the giver runs `goal keep`.
- A computer that the others refuse: it asks why on its next exchange and is
  answered, if a change of host is the reason. If it was simply removed, it is
  not told, as today.

## Scenarios

| | Sequence | Outcome |
| --- | --- | --- |
| S0-cast | Used by every scenario. Computer A: Ana, the host; governance key G0; host's agent Harbor. Computer B: Ben; agent Juniper. Computer C: Cleo; agent Maple. Computer D: Dee; agent Birch. G0's log: g0 first record, g1 admits Harbor, g2 rules, g3 admits Juniper, g4 admits Maple, g5 admits Wren, each anchored at the one before. A, B and C hold g0 to g5. | Chain g0..g5. Governance key G0, host's agent Harbor, backup none. `goal status` on every computer prints `Backup host: none`; plain `status` says nothing about it and nothing is listed under Waiting for you. |
| S1-naming | Ana runs `backup set --member Juniper`: no question; g6 = BackupSet(Juniper); the result ends `Undo: locust --owner backup clear`. Later `backup set --member Maple`: g7 = BackupSet(Maple). Later `backup clear`: g8 = BackupSet(none). In another run Ana removes Juniper while named: g7' = MemberRemoved(Juniper). Refused before signing, and excluded in replay if signed anyway: naming Harbor, naming an agent on computer A, naming a key that is not a member. | After g6 every holder computes backup = Juniper; after g7, Maple; after g8, none. After g7' the backup is none and Juniper is not a member. A change record by Juniper with base g6 is authorized at g6 and revoked by g7, by g8 or by g7' on any computer that holds that record: each is a G0 record whose `prev` reaches g6 and that names another, nobody, or removes Juniper. |
| S2-handoff | Ana runs `goal give --member Juniper`: no question. Her daemon signs g6 = BackupSet(Juniper) if Juniper is not the backup and writes the local record Giving(to Juniper, earlier none). G0 signs nothing more here: a joiner is answered `host_changing`, recordings wait. Ben runs `goal takeover`: his daemon asks; A answers, the second exchange brings nothing; base (6,g6); T = HostChanged(base (6,g6), host Juniper), signed by K1 = key computed from Juniper's seed, the goal and (6,g6), position 0, anchor g6, at_ms 0, empty text sealed at epoch 1. A receives T, computes it in force, signs y = HostYielded(T) with G0 at position 7 by itself and deletes the Giving record. | A, B and C compute chain g0..g6, T. Nothing is set aside, because G0 signed nothing after the base but the yield, which is a marker. T is final: no later G0 record can revoke it. Ana keeps Harbor's membership and name, roles Harbor shares, her levels, folders and copy. She loses `Host: you`, roles only Harbor held, her invitations (revoked) and her page (blank; she can delete it). If Ben never acts, `goal keep` signs g7 = BackupSet(none) and Ana hosts on; a takeover Ben then signs from base g6 without having received g7 is revoked when g7 arrives. |
| S3-takeover-of-a-lost-host | g6 = BackupSet(Juniper); A, B, C hold g0..g6. A recorded one file change s7 (a host step, anchor g6) that reached C and not B. A is lost. Ben runs `goal takeover`. `goal.takeover.ask` marks every pair due. B's exchange with C brings s7; the next brings nothing: C answered. A fails. The plan lists C as answered and A as not answering, base (7,s7). Ben says yes. Checks: no end held; Juniper local, active, the backup at the tip; no held G0 record after the base removes or un-names Juniper; no hold; the marks name no governance key here whose change record is missing; the plan id matches. T is signed by K1 = computed(Juniper, goal, (7,s7)), anchor g6. | B and C: chain g0..g6, T at position 7; governance K1; host's agent Juniper; backup none; epoch 1 under the key T opened; publication none. s7 is in the kept log and stays the head of the files. G0 answers nothing from `Goal::next` on any computer that holds T. Harbor is still a member, so no notice pair exists; B's status keeps one line: Harbor's computer has not received the takeover. It stays for good if A never returns. |
| S4a-the-hosts-removal-wins | g6 = BackupSet(Juniper). A signs g7 = MemberRemoved(Juniper), epoch 1; it reaches nobody. A sleeps. B hears from C and takes over: T, base (6,g6). As host Ben admits Robin (h1), binds new rules (h2) and his daemon records one file change (s3); Maple posts m1 anchored at h2. A wakes and syncs with C (Harbor is a member on both sides). C sends T, h1, h2, s3, Robin's records and m1; A sends g7. | On A: `screen` keeps T, because its path g0..g6 is held and Juniper is the backup at g6; K1 is then a known governance key, so h1, h2, s3 and Robin's records are kept. g7's `prev` is g6, the base, so T is `Revoked`. No live change: G0 is the last term, chain g0..g7, host Ana, Juniper and Robin not members, rules g2. h1, h2, s3 are `SetAside`; m1 is `Excluded(SetAside)`. A signs no yield. On C after g7 arrives: the same set, so the same standings, whichever of T and g7 came first. Maple's next record m2 anchors at g7: the walk skips m1 and m2 is effective on A and on C. B dials C and is refused as not a member; B asks with claim T; C answers g7; B computes T `Revoked` and prints that its takeover was dropped. Robin's computer asks B or C the same way and stops dialing. A's status lists what was dropped (text B-6). Until A woke, B and C treated T as in force: that is the window. |
| S4b-a-removed-backup-takes-over | As S4a, but g7 reached C before A slept. B dials C: `NotAMember`. Ben runs `goal takeover`. The plan lists Maple's computer under 'Refuses this computer: Juniper is not a member there' and nobody as answered. B holds no removal, so the command does not refuse. Ben says yes: T, base (6,g6). B's commit makes its pairs due; the ordinary exchange with C is refused; the next is a notice with claim T. | C: `screen` keeps T (placed, authorized at g6); C holds g7, so T is `Revoked` there and C's chain does not change. C answers with g7. B lands g7 and computes T `Revoked`: status reads `not a member`, your takeover was dropped. One exchange after the first refusal. If no computer is on, B believes it hosts until one is; anyone B admits meanwhile is told the same way when they reach a member. |
| S4c-two-hosts-that-each-removed-the-other | g6 = BackupSet(Juniper). A signs g7 = MemberRemoved(Juniper); it reaches nobody; A sleeps. B hears from C and takes over: T, base (6,g6). Ben removes Harbor: h1 by K1. A wakes. A's view: host Ana; members Harbor, Maple, Wren. B's and C's view: host Ben; members Juniper, Maple, Wren. | A dials C and is refused. A asks with no claim and its admission g1; C answers T; A keeps T as `Revoked`. C holds a change in force with no yield, and Harbor's endpoint is bound to no member there, so C tells A: notice with claim T. A answers g7. C lands g7: T `Revoked`, h1 set aside, Harbor a member, Juniper not. C now refuses B; B asks with claim T and gets g7. All three end with host Ana. With only A and B in the goal: B tells A by the same rule, A answers g7, B's takeover is dropped. With B lost after h1: C still tells A and converges; no computer ends alone. |
| S5a-taking-over-twice-from-one-base | B took over in S3: T, base (7,s7), then signed h1..h43 with K1. B's data folder is put back from a copy made before the takeover. Marks kept: the marks name K1 in this goal and the store holds no change record by it, so `goal takeover` is refused as catching up. B syncs with C and receives T and h1..h43: T names the local agent Juniper and no local record says this store signed it, so the daemon writes that record and `UNHEARD` for K1; when an exchange with C ends having brought nothing, the hold ends and B signs h44. Whole computer restored, marks lost, nobody reachable: the goal holds `RESTORED`; Ben runs `goal continue`, then `goal takeover`, yes. | With the marks: no second record is signed. Without them: the daemon signs the same key, base, anchor, payload and at_ms 0, so the same bytes and id as T; in the same commit K1 gets `UNHEARD`, because the goal holds `RESTORED`. B signs nothing as host until it has heard from C, by which time it holds h1..h43. Only a second `goal continue` lets it sign h1' at a used position; that is a fork in the last term's log, and Ben continues from the branch with 43 records as in S15. |
| S5b-two-takeovers-from-two-bases | B took over with T1, base (7,s7), and hosted for a week: 43 records by K1; Maple posted m1 anchored at h30; h10 removed Wren; h20 admitted Robin. The whole computer is restored from a copy that holds G0 only to g6. Nobody is reachable. Ben runs `goal continue` and `goal takeover`: T2, base (6,g6), key K2; K2 gets `UNHEARD`. B syncs with C. | Both are placed and authorized: two live changes on term 0, `Halt::HostDispute`, chain g0..g6. T1 and T2 are `Disputed`; K1's and K2's records are undecided. No governance key signs. m1's anchor is undecided, so anything Maple signs now is pending and her daemon refuses to sign it: she waits. Wren is a member at g6, but h10 is held and removes Wren, so C refuses Wren's computer and serves no key opened under K1. Robin's computer is refused, asks with claim T1 and is answered T2, so it shows the conflict. B's daemon can compute K1 and K2; K1 has records after its change and K2 has none, so it signs W = TakeoverWithdrawn(T2) with K2 by itself. On every computer that holds W: T2 `Withdrawn`, T1 in force, the week is back, Maple signs again, Robin is a member. Had Ben signed anything with K2 first, status would list both with the line `goal takeover --keep T1`. Had the outcome been T2: K1's records are set aside, m1 is skipped and Maple's next record at the head counts. |
| S6-a-removal-the-backup-never-received | g6 = BackupSet(Juniper). A signs g7 = MemberRemoved(Wren, admission g5, last_accepted w3). Ana's result line says Juniper's computer has not received this yet. A is lost before any exchange with B; C received g7. Case 1: B's ask reaches C, which brings g7; base (7,g7). Case 2: C is off; the plan says no other computer answered; Ben says yes; base (6,g6); T. Wren's computer dials B and is a member there; Wren approves Maple's change (w4) and B's daemon records it. Two days later C comes on and syncs with B. | Case 1: g7 is kept; Wren stays removed. Case 2: g7 is `SetAside`; Wren is a member and receives the key T opened. When B holds g7 its daemon sees a set-aside removal by an earlier term's key whose member is still in under admission g5, and signs h1 = MemberRemoved(Wren, g5, last_accepted w4) with K1, unasked: the cutoff is Wren's last effective record on B's chain. Epoch 2, a new key. w4 stays effective, so the recorded change keeps its evidence. Records Wren signs after w4 are `PastRemoval`. `goal status` on B says the removal was repeated. Had Ben named Wren his backup or given Wren a role, no removal is signed; status states the fact with the remove command. Had no computer but A held g7, Wren would stay in and nobody would know. |
| S7-the-newest-key-was-only-on-the-old-host | A signs g7 = MemberRemoved(Wren), epoch 1, key k1. The record reaches B and C; the key reaches nobody. A is lost. B and C cannot write text (measured today). B takes over, base (7,g7): T's payload is sealed under k2, computed from K1's seed, at epoch 2. | B holds k2 by construction. C asks `KeyRequest { opened_by: T }`, checks k2 by opening T's payload and writes text at epoch 2. k1 is never needed. If A had also bound rules at epoch 1, their text cannot be opened by anyone: the plan says the current rules cannot be read here, and Ben binds rules again. |
| S8-a-host-that-was-only-asleep | g6 = BackupSet(Juniper). A sleeps. B hears from C and takes over: T, base (6,g6). A wakes holding g0..g6. Before its first exchange Dee presents an old ticket: A's daemon signs g7 = admits Birch. D syncs with A and holds g0..g7; Birch posts b0 anchored at g7. A then syncs with C. | A receives T by ordinary sync, because Harbor is a member on C's chain. On A, B and C: T is in force, g7 and b0 are `SetAside`, Birch is not a member. A's daemon signs y = HostYielded(T) with G0 at position 8 by itself; from then no G0 record can revoke T, and a copy of A's data older than y is held by the marks until y is back. A revokes Ana's pending invitations, its page goes blank, and it prints the replaced block: dropped: admitted Birch. D dials A and C and is refused by both. D asks with no claim and its admission g7; A, or C once it holds g7 or is shown it, answers T. D holds g0..g6, so T is in force there: D sees its own admission set aside, prints that Birch must ask Juniper's owner for a new ticket, and stops dialing. Next for Ana: she works on as a member; Ben can run `goal give --member Harbor` and she takes over, which makes term 2 with a new key on computer A; G0 is not used again and her old page address does not come back. |
| S9-an-end-and-a-takeover | g6 = BackupSet(Juniper). Case 1: Ana ends the goal, g7 = GoalEnded, and B holds it. Case 2: g7 reaches nobody; B takes over, T base (6,g6); g7 arrives a day later. Case 3: a change record whose base is g7 or a later G0 record. | Case 1: `goal takeover` answers `conflict(ENDED)` and signs nothing. Case 2: g7 is a G0 record outside the kept log, `SetAside`; it removes nobody and names no backup, so T stands; `State.ended` is unset and `cut_end` does not see it, because it reads the last term's log only; the goal goes on; A yields, and Ana's status says her end was dropped. Case 3: the kept log holds an effective end, so the change is `Excluded(AfterEnd)` on every computer and never in force. `Chain::build` enforces all three; the command enforces the first. |
| S10-a-public-goal | The goal has a page (g7 = PublicationSet, farm id F, upload key on A's disk), an open door and two tickets out. Harbor shared the first files. A is lost. B takes over, base (7,g7). | On every member's computer the publication is none after T; `farm show` says the goal has no page. The plan told Ben: invitations, the door and the page do not move. Page F keeps its last content; its Join band reaches a computer that does not answer; the farm service removes it by its rule for a page that stopped checking in. A ticket of Ana's presented to B is refused. Ben runs `farm on`: a new upload key and a new address. Under today's consent rule every member but Harbor consents again; Harbor, the earlier host's agent, is not asked and is shown as 'earlier host'. On a joinable page Ben's publication record is the consent. If A was only asleep: on waking its page goes blank and Ana can delete it with `farm off`. |
| S11-recordings-across-a-takeover | G0's log: g6 = BackupSet(Juniper), s7 accepts file change P1 (previous none), s8 accepts P2 (parent s7), s9 records plan revision R1, s10 records R2 (base R1). B holds through s7 and takes over: T base (7,s7), anchor g6, chain position 7. C holds s8 to s10; they arrive at B later. Cleo's P3 has parent s8. | Kept: s7. Set aside whenever delivered: s8, s9, s10. Files: P2 is effective, counts and builds on the head s7, so B's daemon signs s8' accepting P2 with K1, previous s7, anchored at T. In the files scope the decisions are s7 (anchor position 6) and s8' (anchor position 7): by R4's sort s8' is last and is the head, though s7 has the higher position in its own author's log. P3 names s8 as parent; s8 is set aside and the previous decision is s8', so P3 can never land: Cleo is told it is behind and rebuilds it. Plan: R1 is next and is recorded; then R2, whose base is R1, is next and is recorded. s8 does not block the stream, because a set-aside decision holds no place after s7. After T every admission, stage step, plan recording and file acceptance is signed by K1; a step signed by G0 with an anchor at or after T is `SetAside`. |
| S12-a-backup-who-came-through-the-door | Juniper joined through the public door. Ana runs `backup set --member Juniper`; the result adds two lines: Juniper came through the public door; if Juniper's owner takes over they are the host. Later Ben takes over. | The same record and the same rule as for any backup; no extra limit. Joining gave nothing; the one command did. While Juniper is an ordinary member its tasks still ask first on every member's computer. Once the change is in force Juniper is the host's agent of the term: tasks it writes and stage tasks opened in that term are the host's, and members Ben invites are members the host invited. This reading of the owner's decisions 12 and 22 is put to the owner. |
| S13-second-and-third-change | T1 is in force (K1, Juniper); A never returned and never yielded. Ben runs `backup set --member Maple`: h1 = BackupSet(Maple) in K1's log. B is lost. Cleo takes over: T2 = HostChanged(base in K1's log, host Maple), signed by K2. Cleo names Robin; later Robin takes over: T3. A year later a record g7 = MemberRemoved(Juniper), which A had signed with `prev` g6 and which only one member held, is delivered. | The walk goes G0, T1, K1's kept log, T2, K2's kept log, T3. After each change the backup is none until the new host names one, with the same command, and nothing asks. When g7 is held, T1 is `Revoked`; T2 and T3 are built on a change that is not in force, so every record of K1, K2 and K3 is `SetAside` and the goal is back under G0, whose computer is lost. That is the owner's rule carried through. Had A returned even once after T1 it would have yielded, and g7 could not exist beside a yield unless A's own computer had been restored. The texts say a takeover is final only once the earlier host's computer has received it. |
| S14-upgrade | A person installs Locust 8 over a home written by 7 and starts it. Another person on 8 is sent a ticket made by 7. | The daemon refuses to open the home and changes nothing in it: text B-9. The ticket is refused: it was made by an earlier Locust; goals from before version 8 cannot be joined. No goal exists on both sides of the version line. |
| S15-a-forked-old-log | A is restored from a copy that holds g0..g6, its owner runs `goal continue`, and A signs g7' where members hold g7, g8, g9. Halt::Fork at 7. Case 1: A is present. Ana's status prints the conflict and one command. `goal takeover` on A: `takeover_bases(Harbor)` gives (9,g9) with 3 records after the fork and (7,g7') with 1; the plan takes the first; yes: Tc = HostChanged(base (9,g9), host Harbor) by Kc = computed(Harbor, goal, (9,g9)). Case 2: g6 = BackupSet(Juniper), A is lost after the fork; Ben runs `goal takeover`: the same two tips, Juniper is the backup on both; base (9,g9). Case 3: the fork is at 5 and BackupSet(Juniper) is g6 on one branch only; A is lost. `takeover_bases(Juniper)` gives the tip of that branch alone. | Case 1: Tc is authorized because Harbor is the host's agent at g9. It is a continuation: backup, publication and roles stay; the key and the epoch change; g7' is `SetAside`; the halt is gone; invitations Ana issued before stop working. Case 2: T's kept log is g0..g9 by `prev`; g7' is set aside; members whose records were anchored at g7' are excluded there and their next records count. Case 3: the change is authorized along its own path g0..g4, g5, g6, although the usable prefix stops at g4 where nobody is named. In every case a second G0 record at or below the base that arrives later changes nothing, because the kept log is read by ancestry. |
| S16-a-restored-old-host-that-had-received-the-takeover | S8 happened: A holds T and signed y at G0 position 8. Three months and two more changes of host pass. A's data folder is put back from a copy made before A received T; the marks are kept. Before any exchange Ana runs `backup clear`. | G0's mark is (8,y); the store's tip is below it, so G0 is behind and `backup clear` answers that this computer is catching up. A syncs, receives y and T, and is a member. Nothing is voided. Whole computer restored with the marks lost: every key waits to hear from the other computers and A learns T first. Only with nobody reachable and `goal continue` run can Ana sign g7'' = BackupSet(none) at position 7; its `prev` is g6 and no yield lies between, so T is revoked on every computer that receives it. That is the stated limit. |
| S17-a-lost-host-and-a-record-someone-kept | g6 = BackupSet(Juniper). Ana removes Juniper: g7, epoch 1, key k1. Record and key reach only member M. A is lost for good. B hears from C and takes over; months pass. M, still a member, syncs g7 to C. | g7 descends from the base and no yield exists, so T is `Revoked` on every computer that receives g7. Chain g0..g7: host Ana, whose computer is lost; no backup; epoch 1 under k1, which only M holds. No honest computer can write text and nobody can take over. Everything done under T is set aside. This is the owner's decision 18 with a host that never returns. It is a stated limit and a question for the owner. After M is removed it has no route to deliver g7 through an honest daemon: a notice answers only a claim, and nobody claims to M. |
| S18-an-unnaming-on-the-other-branch | g6 = BackupSet(Juniper). A sleeps. B takes over: T, base (6,g6). A week passes under K1. A's whole computer is put back from a copy that holds G0 to g4; the marks are lost; nobody is reachable; Ana runs `goal continue` and repeats from memory: g5' admits someone, g6' rules, g7' = BackupSet(Maple). A reconnects. | g7' is a G0 record at position 7 that names another backup, but its `prev` chain is g6', g5', g4: it does not reach the base g6. T is not revoked and stays in force; K1's week stands. g5', g6', g7' are outside T's kept log and `SetAside`. A receives T, is a member, and yields. Under the first draft's rule by position T was voided, G0 was the last term with a fork at 5, and nobody could take over. |

## Draft phases

As revised after the two attacks, before the last check. Phase names B1 to B3.

### B1: A backup host, taking over, and one host on every computer

**Goal.** A host can name one member as backup host, name another, or clear
the name. The backup host's owner can take over with one command and one yes,
alone. From then on their computer signs who is in, the rules and the host's
steps with a new governance key. Every computer sets aside what the old key
signed that the backup's computer had not received, whenever it arrives, and
keeps what it needs to judge that as every other computer does. The host's
removal or un-naming of the backup wins over a takeover until the host's
computer has received the takeover; from then the takeover is final. Two
computers that can reach each other end with the same host, also when each
removed the other. A host whose own history has two conflicting branches
continues from one with the same command. Naming is optional and nothing asks
for it. This phase takes protocol 8, and no goal made before it loads.

**Depends on.** The whole build order through R10 and the first public
release. By name: K1 (`State.governance`, `State.host`, `Local.governance`,
`Node::key_for`, `Node::hosts`, `Body::host_may_sign`, the record `K`); G1 and
G2 (`Node::hold`, `Node::guard_sources`, `Host::reconciled`, the marks, the
records `UNHEARD` and `RESTORED`, `Why::ThisComputer`); E1 (`Body::GoalEnded`,
`Exclusion::AfterEnd`, `State.ended`, `Chain.cut_end`, `ENDED`, the comment
`PLUG host-replacement`); R2 (`cli/confirm.rs` with plan ids,
`cli/only_you.rs`, the `Undo:` line); R3 (`Refused`, `Why`, `Act`); R4 (member
names, role lists, `resolve` with an anchor, the only-member part, the
first-files rule, and `project` sorting a scope's decisions by `(anchor
position, author, seq, id)`); R5 (the status view, `WaitingKind`); R8 and R9
(`ScopeDecided` signed by the governance key, `desired_selections`). All of
these are plan text, not code. The three models named under "Model first" are
written and run before `chain.rs` is touched. Nothing is released between B1
and B3.

**Changes.** Terms, each with one meaning below.

- A *term* is the part of a goal's history signed by one governance key. Term
  0 is signed by the key the first record names.
- The *backup host* is the member the latest `BackupSet` in force names.
  Removing that member clears it. A change of host to another person clears
  it.
- A *change record* is `HostChanged`. It sits at position 0 of a new key's log
  and starts the next term. Its *base* is the record of the previous term's
  key that it names. Its *kept log* is the base and everything before it in
  that key's log, following `prev`. Its *path* is the path of the change
  record at position 0 of that kept log (none for the first key), then the
  kept log, then itself.
- A change record is *placed* when every record on its path is held and each
  kept log's governance records form a chain. It is *authorized* when every
  earlier change record on its path is authorized and, in the snapshot got by
  folding its path up to the base, the member it names is the backup host or
  the host's agent. Naming the host's agent is a *continuation*: a host going
  on from one branch of its own history.
- A record `v` of the previous term's key *revokes* a change record when the
  base is reached from `v` by `prev` through held records; `v` takes effect
  when those records are folded in order from the base; `v` removes the member
  the change names, or the change was authorized through the backup and `v` is
  a `BackupSet` naming anyone else or nobody; and no `HostYielded` for that
  change lies between the base and `v`.
- A change record is *withdrawn* when a held `TakeoverWithdrawn` by its own
  key names it.
- A change record is *live* when it is placed and authorized, its anchor is
  the last governance record of its kept log, its payload epoch is one more
  than there, its kept log holds no effective end, and it is neither revoked
  nor withdrawn.
- A term with exactly one live change record: that record is *in force*. With
  two or more the term is *disputed*. With none the term is the last.
- A record is *set aside* when a governance key signed it and the walk has
  decided against it: a record of a term's key outside the kept log of the
  change in force that ended the term; or any record of the key of a change
  record that is not live, or whose path holds a change that is not in force.
  It is *undecided* when it is past a fork in the last term's log, or is a
  change record of a disputed term or a record of its key.
- A *marker* is `TakeoverWithdrawn` or `HostYielded`. A governance key signs
  it, it takes no position on the chain, nothing may anchor at it, and it is
  never set aside.
- A *notice* is one question and one answer on the evidence exchange that
  today carries a fork proof.

Every definition above is a function of the signed events held. No clock,
arrival order or identifier comparison enters.

- [lib.rs](../crates/locust-proto/src/lib.rs): `PROTOCOL_VERSION` and
  `API_VERSION` 7 to 8. [schema.rs](../crates/locust-store/src/schema.rs):
  `VERSION` 7 to 8. [site.json](../docs/site.json) follows. The refusal a store of
  marker 7 gets is text B-9.
- [event.rs](../crates/locust-proto/src/event.rs): four last variants, after
  whatever the tree holds (indices 27 to 30 if the public-goals phases add no
  record kind first). `Body::BackupSet { backup: Option<PublicKey> }`
  (`backup_set`, governance). `Body::HostChanged { base: AuthorPoint, host:
  PublicKey, endorsement: Signature }` (`host_changed`, governance).
  `Body::TakeoverWithdrawn { change: EventId }` (`takeover_withdrawn`) and
  `Body::HostYielded { change: EventId }` (`host_yielded`); for both
  `is_governance` is false and new `Body::is_marker` is true. `Header::check`:
  a `HostChanged` has `seq` 0, a payload, `host` different from its author,
  and an `endorsement` that verifies under `host` over the digest of (goal,
  base, author, host) in the new domain `HOST_CHANGE_SIGNATURE = "locust v0
  host change"`; a `BackupSet` and a marker carry no payload; a marker has
  `seq` above 0. `dependencies()` of `HostChanged` is the base's id, and of a
  marker the change it names.
- [crypto.rs](../crates/locust-proto/src/crypto.rs):
  `Keypair::governance_for(agent: &Keypair, goal: GoalId, base: AuthorPoint)
  -> Keypair`, whose seed is the digest of (agent seed, goal, base.seq,
  base.id) in the new domain `"locust v0 governance key"`, and
  `ContentKey::opened_by_change(governance: &Keypair)` in `"locust v0 change
  content key"`. Both are frozen in
  [vectors.rs](../crates/locust-proto/src/vectors.rs), with a vector for a
  whole signed `HostChanged`.
- [invite.rs](../crates/locust-proto/src/invite.rs): `Invitation` gains
  `issuer: PublicKey` after `governance`, in the signing digest; the ticket is
  signed by `issuer`. `governance` stays the goal's first key, so the joiner's
  check of the first record in
  [replica.rs](../crates/locust-core/src/node/replica.rs) is unchanged.
- [sync.rs](../crates/locust-proto/src/sync.rs): `KeyRequest { opened_by:
  EventId }` and `Key { opened_by, key }` replace the epoch number. Two last
  variants: `Notice { claim: Option<WireEvent>, admission: Option<WireEvent>,
  after: Option<EventId> }` and `NoticeAnswer { kept: bool, records:
  Vec<WireEvent> }` with at most two records. The evidence exchange is
  `Hello`, then `HaltProof` or `Notice`; a `Notice` is answered by one
  `NoticeAnswer`; then `Done`. `Refusal` gains `Replaced` (`"replaced"`): the
  responder no longer hosts this goal.
- [standing.rs](../crates/locust-core/src/goal/standing.rs): `Exclusion` gains
  `SetAside`, `Revoked`, `Withdrawn`, `NotBackup` and `Unplaced`. `Halt` gains
  `HostDispute { changes: Vec<EventId> }`. A change record of a disputed term
  has the existing `Standing::Disputed`. No change record and no marker is
  ever `Pending`.
- [state.rs](../crates/locust-core/src/goal/state.rs): `State` gains `backup:
  Option<PublicKey>` and `terms: Vec<Term { governance, host, change:
  Option<EventId>, base: Option<AuthorPoint>, yielded: bool }>`; `governance`
  and `host` are the last term's.
- [history.rs](../crates/locust-core/src/goal/history.rs):
  `History::kept(point) -> Option<&[Slot]>`, the `prev` walk from a point to
  position 0, cached, none when a link is not held; `History::tips(author) ->
  Vec<AuthorPoint>`, the held records of one author that no held record of
  that author names as `prev` and whose links are all held;
  `History::descends(record, from) -> Option<bool>`, none when a link is
  missing.
- [chain.rs](../crates/locust-core/src/goal/chain.rs): `Snapshot` gains
  `governance`, `host`, `backup` and `key: EventId` (the record that opened
  the current content key). The loop body of `Chain::build` becomes
  `step(snapshot, event) -> Standing`. New `Chain::fold_path(history, change)
  -> Option<Snapshot>` folds `step` along a change record's path; `screen` and
  the walk both use it. `build` becomes a walk over terms:
  1. Start at the first key with an empty snapshot.
  2. Collect the held change records whose base is in this key's log and whose
     kept log starts at the change record this walk came through. Test each
     for placed, the anchor, authorized, the epoch and the end; a failure is
     `Unplaced`, `Unplaced`, `NotBackup`, `BadEpoch` or `AfterEnd`. This is
     the place E1 marked `PLUG host-replacement`.
  3. Mark the revoked and the withdrawn ones `Revoked` and `Withdrawn`.
  4. None live: this is the last term. Fold its usable prefix as today; a fork
     is `Halt::Fork`; E1's `cut_end` reads this key's log only.
  5. One live: fold the governance records of its kept log, then the change
     record, which takes the next position. Its snapshot has `governance` its
     author, `host` its `host`, the epoch plus one, `key` its own id. For a
     change to another person the backup and the publication are cleared, and
     every role list whose only holder was the old host's agent holds the new
     one. A continuation keeps backup, publication and roles. Go to 2 with its
     author.
  6. Two or more live: fold the governance records every one of their kept
     logs holds, set `Halt::HostDispute`, stop.
  `step` gains two arms. `BackupSet`: with a member, it must be a current
  member, not the term's host's agent, and bound to another endpoint than the
  host's agent, else `Precondition`; it sets `snapshot.backup`.
  `MemberRemoved` also clears `backup` when it removes that member. K1's two
  rules read the term: an admission of any known governance key is excluded,
  and a removal of `snapshot.host` is excluded.
  `authorize_base`, in order. A marker is effective when the change it names
  is held and placed and the marker's author is that change's author (a
  withdrawal) or the key of the log its base is in (a yield); else `Unplaced`.
  A governance record with no standing from the walk is `SetAside` or, when
  undecided, `AfterHalt`. A host step is effective only when
  `snapshot(anchor).governance` is its author and it is in that key's kept log
  (the usable prefix for the last term); else `SetAside`, or `Pending(Anchor)`
  when undecided. For every other author: an anchor that is held and set aside
  gives `Excluded(SetAside)`; an anchor that is held and undecided gives
  `Pending(Anchor)`, as a retracted anchor does today. In the walk back along
  the author's log, an ancestor whose anchor is set aside is skipped: it
  neither makes the record pending nor counts for `AnchorRegressed`. An
  ancestor whose anchor is undecided keeps the record pending, as today.
- [fold.rs](../crates/locust-core/src/goal/fold.rs),
  [rules.rs](../crates/locust-core/src/goal/rules.rs),
  [goal/flow.rs](../crates/locust-core/src/goal/flow.rs),
  [projection.rs](../crates/locust-core/src/goal/projection.rs): wherever R4,
  R8, R9 and K1 read `State.governance` or `State.host` for a judged record,
  they read `snapshot(anchor)`: the authority of an agreed document and of the
  shared files, the stage runner, the first-files author, the role fallback.
  In `Verifier::new` the decision index uses one stand-in author for every
  governance key, so two host decisions after one predecessor dispute that
  scope. `desired_selections` ignores set-aside decisions. `project` keeps
  R4's sort; no change, and a replay pins that a new key's decision follows a
  kept one. `current_round` picks, among the effective opening steps of one
  derived task, the lowest `(anchor position, author position, id)`.
- [goal/mod.rs](../crates/locust-core/src/goal/mod.rs): `Goal::next` answers
  nothing for a governance key that is not the last term's, and for every
  governance key under `HostDispute`. New `Goal::marker_place(key) ->
  Option<Next>`: the key's next position, none when its log is forked or has a
  gap; the caller supplies the anchor, which is the named change's. New
  `Goal::takeover_bases(agent) -> Vec<TakeoverBase { base, anchor,
  continuation, after_fork: u64 }>`: each tip of the last term's governance
  log at which, folded along its own kept log, `agent` is the backup host or
  the host's agent. A log with one branch gives at most one.
- [screen.rs](../crates/locust-core/src/goal/screen.rs): a *known governance
  key* is the first key or the author of a kept change record, held or earlier
  in the batch. Kept are records by a known governance key, records by any key
  a known governance key admitted, and a `HostChanged` for which `fold_path`
  answers a snapshot in which it is authorized, whatever voids it. A
  `HostChanged` whose path is not held is dropped; its sender's frontier
  brings it again.
- [node/local.rs](../crates/locust-core/src/node/local.rs): two records in
  `Space::Goal`. `T` (goal, change): this daemon signed this change record.
  `q` (goal): the takeover question was asked, and when.
- [entry.rs](../crates/locust-core/src/node/entry.rs) and
  [replica.rs](../crates/locust-core/src/node/replica.rs): `Space::Key` is
  keyed by goal and opening record and holds the epoch number and the key.
  Sealing uses `snapshot(head).key`. Reading tries the keys held for the
  blob's epoch number. `key`, `wanted_keys` and `offer_key` take the opening
  record; `offer_key` accepts a `HostChanged` as proof like a removal; `key`
  answers, and `wanted_keys` lists, only a key whose opening record is
  effective here.
- `crates/locust-core/src/node/requests/hosts.rs` (new) and
  [api.rs](../crates/locust-proto/src/api.rs). The word for the person is
  "take over the goal"; every request sits under `goal.` so it never reads
  like R3's `attempt.takeover`.
  - `backup.set { goal, backup: Option<PublicKey>, expected }` (goal-scoped,
    `Host`, not a tool): `host()`, then the checks of `step`, then one
    `BackupSet`.
  - `goal.takeover.ask { goal }` (`Owner`): writes `q`. Its commit sets new
    `Tx::reach`, which marks the goal changed for sync though no record was
    added, so every pair of the goal is due.
  - `goal.takeover.plan { goal }` (`Owner`, read only): answers `TakeoverPlan
    { bases, host_name, host_last_sync_ms, answered, refused, unreached,
    waiting, sole_roles, rules_readable, published }` from new memory-only
    `Node::asked`, which `exchange_ended` and G1's `reconciled` fill per
    endpoint since the ask: answered (an exchange ran to its end and brought
    nothing new), refused as not a member, failed, or still waiting. The
    host's computer counts like any other. G1's heard set is not touched.
  - `goal.takeover { goal, agent, base, plan }` (`Owner`, not a tool). In
    order: the goal holds an end, `conflict(ENDED)`; `agent` is local, active
    and has not left; `base` is in `takeover_bases(agent)`, else `denied("only
    the backup host takes over")`, and a continuation is allowed only where
    the last term's log is forked; the store holds a record of the last term's
    key at a position after the base that removes `agent` or is a `BackupSet`
    that does not name it, `conflict` with that fact; `hold(entry, agent)`
    answers a reason, or the marks name in this goal a governance key whose
    change record the store does not hold, then G2's `read_only`; `plan` is
    not the id of the plan as it would be shown now, `conflict("the plan
    changed")`. Then it computes the key, signs `HostChanged` at position 0
    with `at_ms` 0 and the empty text as payload through `sign_at`, and writes
    the content key and `T` in the same commit, with G1's `UNHEARD` for the
    new key when the goal holds `RESTORED`. Signing the same record again
    answers it.
  - `goal.takeover.withdraw { goal, keep }` (`Owner`): under `HostDispute`,
    for every live change other than `keep` whose key this daemon can compute,
    one `TakeoverWithdrawn` through `marker_place`.
  - `GoalStatus` gains `backup`, `terms`, `takeover: Option<TakeoverView {
    change, received_by_earlier_host, earlier_host_agent }>` on the host's
    computer, and `Halt::HostConflict`. K1's `GoalStatus.governance` is the
    last term's key.
- [commit.rs](../crates/locust-core/src/node/commit.rs): `Tx::reach`. After
  `advance`, for a change record in force that names a local active agent and
  has no `T` record here: write `T` and `UNHEARD` for its key. Another copy of
  this data signed it, so this computer waits to hear before it signs as host.
- [access.rs](../crates/locust-core/src/node/access.rs): `hosts(entry)` is
  true where a local active agent is the last term's host's agent and
  `key_for` answers its key. `key_for` answers K1's stored key for the first
  term and computes a later one from the agent's seed and the held change
  record.
- [node/flow.rs](../crates/locust-core/src/node/flow.rs): `drive_flow` gains
  two automatic acts; like every automatic act they read no level. (1) Where
  this daemon holds the key of a term that a change in force ended, holds no
  yield for that change, and the guard holds nothing for that key: sign
  `HostYielded` for it. (2) Where two or more live change records of one term
  name local agents and exactly one of them has a record of its key after it
  held here: sign `TakeoverWithdrawn` for each of the others. The daemon never
  withdraws the only live change record, one that has records after it, or
  another person's. Rule for every phase: no unattended signature is ever a
  `BackupSet` or a removal of the backup host.
- [peers.rs](../crates/locust-core/src/node/peers.rs) and
  [invitations.rs](../crates/locust-core/src/node/requests/invitations.rs):
  `plan_join` refuses an invitation whose `issuer` is not the last term's key,
  with `Refusal::Replaced` where this daemon holds that issuer's key.
  `goal_join` answers `conflict("this ticket was issued by a host that has
  since been replaced")` when the held goal's last term has another key.
  `historical_endpoints` reads the admissions of every known governance key.
  `speaks_for_member`, while a term is disputed, also asks that no held record
  of a disputed change's key removes that member. `peers()` lists nothing for
  a goal in which no local agent is a member or joining. `Host::halt_proofs`
  becomes `Host::evidence() -> Vec<(GoalId, EndpointId, Evidence)>` with
  `Evidence::{Fork([WireEvent; 2]), Notice { .. }}`:
  - No fork proof is offered for a governance key that is not the last term's.
  - *Asked why.* For an endpoint this chain binds to a current member whose
    last ordinary exchange with this daemon ended `Refused(NotAMember)`: a
    notice whose claim is each change record in force on this chain in turn,
    last first, or none when there is none, with the held admission of a local
    agent. It stops for that endpoint once an answer agreed and brought
    nothing, until this chain's changes differ.
  - *Told.* For each change in force with no yield held, when the admission
    endpoint of the previous term's host's agent is bound to no current member
    and is not this daemon's: a notice whose claim is that change.
  - `receive_notice(goal, remote, notice) -> Result<NoticeAnswer, Refusal>`
    accepts from an endpoint named in a held admission by a known governance
    key, from a join's endpoint, or when `admission` is a `MemberAdmitted` of
    `remote` signed by a known governance key. It passes the claim through
    `screen` and lands it when kept. It answers from its own chain: no claim,
    the first change in force; a claim in force here, the next change in
    force, else the held yield for it, else nothing; a claim whose term is
    disputed here, another live change of that term; a revoked claim, the next
    two records on the path from its base to the record that revokes it, after
    `after`; a withdrawn claim, the withdrawal; anything else, nothing. `kept`
    says whether the claim is held now.
  - The dialer passes the answer's records through `screen` and lands them.
- `crates/locust-core/src/node/guard.rs` (from G1): `guard_sources` reads the
  host's agent of the last term, and leaves out of `all` the computer of an
  earlier term's host's agent whose yield is not held.
- [driver.rs](../crates/locust-core/src/sync/driver.rs) and
  [responder.rs](../crates/locust-core/src/sync/responder.rs): `Dialed.proof`
  carries either kind. After a `Notice` the dialer waits for the one
  `NoticeAnswer`, hands it to new `Host::notice_answered`, then sends `Done`.
  A pair that was refused as not a member alternates one ordinary and one
  notice exchange under today's backoff; a finished notice exchange does not
  clear the backoff or count as heard. The responder hands a `Notice` to
  `receive_notice` before its membership test, as it does a fork proof.
- `crates/locust/src/cli/only_you.rs`:

  | Command, after `locust --owner` | Sends | Asks | Prints |
  | --- | --- | --- | --- |
  | `backup set --goal G --member M` | `backup.set` | no | text B-1, ending in `Undo:` |
  | `backup clear --goal G` | `backup.set` with none | no | text B-1, ending in `Undo:` |
  | `goal takeover --goal G [--base ID]` | `goal.takeover.ask`, `goal.takeover.plan` until nothing waits or 20 seconds of its own clock pass, then `goal.takeover` | yes | text B-3 or B-10 |
  | `goal takeover --goal G --keep CHANGE` | `goal.takeover.withdraw` | yes | `Kept the takeover CHANGE of "T". N others withdrawn.` |

  With several bases the command takes the one with the most records after the
  fork and shows the others; `--base` names another. That choice is this
  computer's; every computer then follows the record.
- [presentation.rs](../crates/locust/src/cli/presentation.rs): `goal status`
  prints `Backup host: none` or `Backup host: NAME (KEY)`; plain `status`
  prints nothing about a goal with none. A record signed by a governance key
  that is set aside prints `host (dropped)`. On the host's computer, the lines
  of text B-11. `Halt::HostConflict` is text B-7 and a forked last term on its
  host's computer is text B-10. A goal whose join ended `Replaced` prints the
  last lines of text B-5. An agent whose chain says it is no member after a
  change prints `not a member`.
- [check_tla.py](../scripts/check_tla.py) and its unit tests: the suite
  `hostchange` joins the fixed list.
- Documents this phase owns: [guide/sharing.md](../docs/guide/sharing.md) gains "A
  backup host", built from the explanation;
  [guide/operations.md](../docs/guide/operations.md) gains "Upgrading to 8";
  [status.md](../docs/status.md) loses the sentence that a goal's host cannot be
  replaced.

**Tests.**
- New in [goal/tests.rs](../crates/locust-core/src/goal/tests.rs), each a
  signed replay run forward, reversed and reloaded, through `screen`:
  `a_backup_is_named_changed_and_cleared_and_removal_clears_it`;
  `naming_a_non_member_the_hosts_agent_or_an_agent_on_the_hosts_computer_is_excluded`;
  `a_change_by_the_named_backup_starts_a_term_and_one_by_anyone_else_is_not_kept`;
  `a_member_that_names_itself_in_its_own_log_cannot_make_a_kept_change`;
  `what_the_old_key_signed_after_the_base_is_set_aside_in_any_arrival_order`
  (an admission, a removal, a rules binding, a stage step, a plan recording
  and a file acceptance, each delivered before and after the change);
  `a_fork_of_the_old_key_at_or_below_the_base_changes_nothing`;
  `a_removal_on_another_branch_of_a_forked_old_log_voids_nothing`;
  `a_host_continues_from_one_branch_and_the_halt_is_lifted`;
  `a_backup_named_above_a_fork_takes_over_from_that_branch`;
  `work_built_on_a_set_aside_record_is_excluded_and_its_authors_next_record_counts`;
  `a_void_change_and_its_terms_records_are_kept_and_set_aside_whichever_arrives_first`;
  `a_removal_or_unnaming_that_descends_from_the_base_voids_the_takeover_whenever_it_arrives`;
  `a_yield_between_the_base_and_an_unnaming_seals_the_takeover`;
  `a_voided_takeover_never_comes_back`;
  `voiding_the_first_change_voids_the_second_and_third`;
  `two_live_changes_dispute_the_term_and_a_withdrawal_settles_it_either_way`;
  `a_member_who_signed_under_a_disputed_term_waits_and_signs_again_after_either_outcome`;
  `a_change_based_at_or_after_an_end_never_counts_and_an_end_after_the_base_is_set_aside`;
  `a_change_opens_a_key_epoch_and_clears_backup_and_publication_and_a_continuation_keeps_them`;
  `roles_only_the_old_hosts_agent_held_pass_to_the_new_one`;
  `a_host_step_is_judged_by_the_term_of_its_anchor`;
  `a_new_terms_decision_after_a_kept_one_is_the_head`;
  `the_old_hosts_agent_can_be_removed_after_a_change_and_the_new_one_cannot`.
- Rewritten there: K1's
  `a_fork_in_the_governance_log_retracts_later_governance_and_preserves_prefix_work`
  also shows the halt lifted by a continuation;
  `retracted_anchor_keeps_same_goal_author_descendants_pending_even_at_surviving_head`
  keeps its halt half and gains the half where a change sets the anchor aside
  and the descendants count;
  `decision_successors_keep_author_scope_purpose_and_predecessor_separate`
  gains two governance keys after one predecessor, disputed.
- New `crates/locust-core/src/node/tests/takeover.rs`, on `Network::with(n)`:
  `taking_over_twice_from_one_base_signs_one_record` (two stores, one seed);
  `a_restored_new_host_waits_to_hear_before_it_signs_as_host`;
  `a_takeover_in_a_restored_goal_holds_its_new_key`;
  `takeover_refuses_with_an_end_a_hold_or_a_held_removal_of_its_agent`;
  `the_plan_lists_who_answered_who_refused_and_who_did_not_and_changes_its_id_with_them`;
  `a_removed_backup_that_takes_over_is_told_by_the_first_computer_it_reaches`;
  `two_hosts_that_each_removed_the_other_end_with_one_host` (three computers,
  and two);
  `the_earlier_host_yields_by_itself_and_a_restored_copy_of_it_is_behind`;
  `a_removed_earlier_host_is_told_and_its_yield_comes_back`;
  `a_joiner_the_old_host_admitted_after_the_base_asks_and_stops_dialing`;
  `a_join_that_did_not_finish_is_told_the_host_was_replaced`;
  `a_notice_claim_that_is_not_authorized_is_not_kept`;
  `an_empty_duplicate_takeover_is_withdrawn_without_a_command`;
  `during_a_dispute_no_key_opened_under_it_is_served_and_a_member_it_removed_is_refused`;
  `after_a_takeover_members_write_text_when_the_last_key_reached_nobody`;
  `keys_are_stored_and_asked_for_by_the_record_that_opened_them`;
  `the_old_host_signs_nothing_as_host_once_it_holds_the_change`;
  `a_ticket_names_its_issuer_and_a_replaced_hosts_ticket_is_refused`;
  `landed_changes_after_the_base_land_again_or_are_reported_behind`.
- In [sync/tests/driver.rs](../crates/locust-core/src/sync/tests/driver.rs):
  `a_notice_is_answered_before_done_and_alternates_with_the_refused_exchange`;
  `no_fork_proof_is_pushed_for_an_earlier_terms_key`.
- In [cli.rs](../crates/locust/tests/cli.rs) and presentation.rs:
  `backup_set_and_clear_apply_at_once_and_print_undo`;
  `goal_status_states_the_backup_or_none_and_status_says_nothing`; one test
  per text of this phase; `every_printed_command_parses_as_printed` gains the
  new lines.
- Regenerated: `signed_current_protocol_vectors_are_frozen`,
  `body_indices_and_bytes_are_current_contract`. Unedited and passing:
  `incompatible_event_protocol_refuses_open_before_collecting_or_rewriting_state`,
  `another_protocol_version_is_refused`.

**Exit criteria.**
- The three cargo commands, the four site commands, `python3
  scripts/check_formations.py`, `scripts/check_docs.py`, `python3 -m unittest
  discover -s scripts/tests` and `python3 scripts/check_tla.py --suite
  hostchange` pass.
- `locust --json contract` reports protocol 8 and lists `backup.set`,
  `goal.takeover.ask`, `goal.takeover.plan`, `goal.takeover` and
  `goal.takeover.withdraw`, none as a tool.
- On three throwaway daemons A (host), B and C in one goal: `backup set
  --member` for B's agent on A prints an `Undo:` line and asks nothing; stop
  A; `goal takeover` on B lists C under "Answered just now" and proceeds on
  yes. `goal status` on B and C names B's agent's owner as host, `events`
  shows one `host_changed`, and a post with text by C's agent succeeds. `goal
  invite` on B issues a ticket that a fourth agent joins with.
- Start A again: its `goal status` names the new host, its own `rules bind`
  exits with `conflict`, `events` on B shows one `host_yielded`, B's status
  loses the line of text B-11 that the earlier host's computer has not
  received the takeover, and no daemon reports a halt.
- A removes B's agent, C receives it, A stops; `goal takeover` on B shows C
  under "Refuses this computer" and proceeds on yes. Within two exchanges B's
  `status` reads `not a member`; C's `events` shows the change record as
  dropped.
- A removes B's agent and stops before anyone receives it; B takes over and
  removes A's agent; A starts. All three end with A's person as host and B's
  agent as no member, with no command run.
- A copy of A's home is started beside nothing, continued with `goal
  continue`, and signs one admission; the original then returns its records.
  A's `status` prints text B-10; `goal takeover` on A proceeds on yes and no
  daemon reports a halt afterwards.
- A home written by protocol 7 is refused with text B-9 and is not modified.

**Risks and notes.**
- This is the phase that replaces the chain builder. The models' properties
  are its first check; the Rust replays run every transcript forward, reversed
  and reloaded, through `screen`.
- Limit, accepted because a hostile host is not assumed and a named party is
  honest: only a member the host really named can make a kept change record,
  but one per base, and each is one author in every frontier, which holds
  4,096. Removing that member does not stop it.
- Limit, the owner's decision 18 carried through: a takeover is undone by a
  removal or un-naming of the backup that the old host signed before its
  computer received the takeover, whenever that record arrives. If that
  computer never returns, any member that holds such a record can deliver it
  at any time. The goal then returns to the lost host's term: nobody can take
  over, and if the record was a removal whose content key reached no honest
  computer, nobody can write text.
- Limit: a whole-computer restore of the old host from a copy older than its
  yield, with nobody reachable and `goal continue` run, followed by an
  un-naming or removal of the backup typed there, voids the takeover although
  that computer had once received it.
- Limit: a computer admitted under a takeover that was later dropped is shown
  the old host's records between the base and the record that dropped it,
  admissions included. Anyone ever admitted can ask which change records are
  in force and learns their keys, no names and no endpoints.
- Limit: two different change records at position 0 of one new key cannot be
  withdrawn, because that key's log is forked at 0. Honest software signs one,
  since the key and the bytes follow from the base; a modified daemon of a
  named member could freeze a term this way.
- A named backup can take over a host that is present. That is the owner's
  decision; the plan says whether the host's computer answered.
- During a dispute every member who signed under either takeover waits, and so
  does every host step under them.
- A removed member that no change of host concerns is still never told of its
  removal.
- Read in the code at `1a7c332`: `Chain::build`, `traverse_cutoff`,
  `authorize` and the ancestry walk; `AuthorLog::insert`; `screen`; the
  decision index, `decision`, `current_round` and `check` in fold.rs; the
  decision sort and the files head in projection.rs; `Goal::next`; `land_once`
  and `advance` in commit.rs; `halt_proofs`, `accepts_halt_proof`,
  `receive_halt_proof`, `speaks_for_member`, `plan_join`,
  `historical_endpoints`; `receive`, `key`, `wanted_keys`, `offer_key`; the
  responder whole; `poll`, `dialed_step`, `end_dialed` and `dialed_writable`
  in the driver; `Body`, `Header::check`, `Event::sign`; the domain strings
  and `Keypair::sign`; `eligible` in farm.rs. Inferred and not run: every
  behaviour after the change; that the transport lets a responder's frame
  reach the dialer on an evidence exchange before `Done`; that ordinary sync
  carries both records of a fork in a governance key's log to every member.
- Size, a judgement: 3,600 to 5,000 production lines and 5,500 to 7,000 lines
  of tests.

### B2: Handing over, the host's view of its backup, the old host's removals, and what a replaced host is shown

**Goal.** A host hands a goal over with one command and nothing is dropped. A
host sees which of its changes its backup has received, and is told when a
command of its own could drop a takeover. The new host's computer repeats a
removal the old host had signed when it receives it, unless that would remove
its own backup host or a role holder. A host that was replaced, a member whose
admission was dropped and a backup whose takeover was dropped each see what
happened and what they can do. A replaced host's invitations are revoked and
its page can be deleted. A new host can publish.

**Depends on.** B1. G1 (`revoke_pending`). E1 (the delete-only path of
`FarmOff`). R5.

**Changes.**
- `requests/hosts.rs`, [api.rs](../crates/locust-proto/src/api.rs) and
  [node/local.rs](../crates/locust-core/src/node/local.rs): `goal.give { goal,
  member, expected }` (`Host`): `backup.set` when the member is not the
  backup, then new local record `g` (goal) holding `Giving { to, earlier:
  Option<PublicKey> }`. `goal.keep { goal }` (`Host`): one `BackupSet` naming
  `earlier`, then `g` is deleted. `g` is durable. It is deleted when a change
  comes into force.
- [authoring.rs](../crates/locust-core/src/node/authoring.rs),
  [peers.rs](../crates/locust-core/src/node/peers.rs),
  [node/flow.rs](../crates/locust-core/src/node/flow.rs) and
  [sync.rs](../crates/locust-proto/src/sync.rs): while `g` exists,
  `next_place` refuses every signature with the governance key but the
  `BackupSet` of `goal.keep`, with `conflict("this goal is being handed to
  NAME")` and the keep command; markers are not refused; `drive_flow` passes
  over host steps; `plan_join` answers new `Refusal::HostChanging`
  (`"host_changing"`), which uses up nothing and which a joiner asks again
  after.
- [replica.rs](../crates/locust-core/src/node/replica.rs) and local.rs: new
  local record `b` (goal): the position of this daemon's governance log that
  the backup's computer last reported holding, taken from that computer's own
  frontier when it is a prefix of this log. It is never ahead of the truth and
  can be one exchange behind.
- [node/flow.rs](../crates/locust-core/src/node/flow.rs): where this daemon
  hosts, `drive_flow` signs a `MemberRemoved` for each held set-aside removal
  signed by the key of an earlier term on the chain whose member is still
  active under the admission that removal names. Its cutoff is the member's
  last effective record here, computed as `member_remove` does today, and it
  draws a new content key. It signs none when the member is the backup host or
  is in a role list set by a record of the current term; that removal is
  reported in `not_repeated`.
- [commit.rs](../crates/locust-core/src/node/commit.rs), in `advance`, when
  the last term's key changes and this daemon holds the key of an earlier
  term: `revoke_pending` for the goal, and local record `p` (goal) with the
  change's id, so status can say it once.
- [node/farm.rs](../crates/locust-core/src/node/farm.rs): the delete-only path
  of `FarmOff` also serves an entry whose page this daemon published and no
  longer hosts. In `eligible`, for a publication record signed in a later
  term, an agent that was the host's agent of an earlier term is left out of
  the members and authors whose consent is required; the page shows it as
  "earlier host" unless its latest effective consent accepts. On a joinable
  page (public-goals contract) the host's publication record is the consent,
  and J7's freshness test reads the kept chain's first joinable publication
  record, which a change of host does not remove.
- api.rs and [views.rs](../crates/locust-core/src/node/views.rs): `GoalStatus`
  gains `backup_holds: Option<{ behind: u32, removals: u32 }>` on the host's
  computer, `giving: Option<PublicKey>`, `replaced: Option<Replaced { by,
  refused_by: Vec<PublicKey>, admitted, removed, rules, recorded, ended }>` on
  a computer that hosted, `dropped: Option<Dropped { by }>` for an agent whose
  admission is set aside, `voided: Option<Voided { cause, admitted, removed,
  rules, recorded }>` for a change that is not in force and that this daemon
  signed or was replaced by, `repeated: Vec<PublicKey>` and `not_repeated:
  Vec<PublicKey>`. `Membership` gains `Dropped`. `refused_by` is local: the
  members whose computers refused this one since its last completed exchange.
- `only_you.rs`: `goal give --goal G --member M` and `goal keep --goal G`
  apply at once and end in `Undo:` (text B-8). While `backup_holds.behind` is
  not 0, the results of `member remove`, of `backup clear`, of `backup set`
  for another member, of `goal give` to another member and of `goal keep` gain
  the fact line of text B-2.
- [presentation.rs](../crates/locust/src/cli/presentation.rs): the blocks of
  texts B-2, B-4, B-5 and B-6 and the repeat lines of B-11. None is listed
  under "Waiting for you".
- Documents: sharing.md gains "If you were replaced" and "Handing a goal
  over"; [guide/farm-publication.md](../docs/guide/farm-publication.md) says a page
  does not move with the host and who is asked for consent after a change.

**Tests.**
- In `node/tests/takeover.rs`:
  `giving_quiets_the_hosts_key_across_a_restart_until_the_takeover_or_keep`;
  `a_handoff_drops_nothing_and_the_giver_yields`;
  `keep_names_the_earlier_backup_again_and_voids_a_takeover_in_flight`;
  `a_joiner_during_a_handoff_is_asked_to_try_again`;
  `the_host_knows_which_changes_its_backup_reported_and_is_never_ahead`;
  `the_new_host_repeats_a_set_aside_removal_with_its_own_cutoff_and_keeps_what_counted`;
  `a_set_aside_removal_of_the_backup_or_a_role_holder_is_reported_not_signed`;
  `a_removal_from_a_void_term_is_not_repeated`;
  `a_member_admitted_again_is_not_removed_by_an_old_removal`;
  `a_replaced_host_revokes_its_invitations_and_can_delete_its_page`;
  `a_new_host_publishes_without_the_lost_hosts_consent`.
- In presentation.rs and cli.rs: one test per text;
  `every_printed_command_parses_as_printed` gains `goal give`, `goal keep` and
  the `farm off` line.

**Exit criteria.**
- The three cargo commands, the four site commands and the two check scripts
  pass.
- On the daemons of B1's drill, with a fourth daemon D: stop A's network, take
  over on B, let A admit D on an old ticket, reconnect A. A's `goal status`
  prints the block of text B-4 naming D; D's `status` prints text B-5 within
  three exchanges and D opens no further exchange for the goal.
- A removes a member while B is stopped: the result on A says B's computer has
  not received it. B takes over without it; when A returns, B's `events` shows
  a second `member_removed` signed as host with no command run, and that
  member's earlier approvals still count.
- `goal give` on A, `goal takeover` on B: `events` on every daemon shows no
  record of A's key after the base but one `host_yielded`. In a second run
  `goal keep` on A before B acts: B's `goal takeover` is refused once B has
  synced, and dropped if B had not.
- With A stopped for good and a page published by A, `farm on` on B publishes
  a page that names A's agent as "earlier host".

**Risks and notes.**
- A giver restored from a copy older than its `goal give` loses the quiet with
  the local record; what it then signs after the base is dropped at the
  takeover.
- A host that was replaced and has not learned it can still remove members;
  the new host's computer repeats those removals. A hostile host is not
  assumed.
- What a member did between a takeover and the repeat of its removal stays.
- Size, a judgement: 900 to 1,400 production lines and 1,500 to 2,200 lines of
  tests.

### B3: Qualification and release

**Goal.** The finished behaviour is run on real daemons, the one sentence a
person must understand is tested on people, and the release says what ends.

**Depends on.** B1 and B2.

**Changes.** No product code. Ten recipes, each with its expected output kept
beside it under `research/`:
- A host on one machine is switched off; the backup on a second machine takes
  over; a third joins on a new ticket; the first machine returns and yields.
- The same with the first machine asleep, not off, and admitting a joiner
  after it wakes.
- The host removes the backup, which takes over without having heard.
- The host removes the backup; the backup takes over and removes the host's
  agent; both machines return.
- The backup's data folder is put back from a copy made before its takeover,
  once with the marks kept and once with the whole home copied; it is host
  again without a second takeover.
- A takeover is made twice from two bases by overriding twice; the empty one
  is withdrawn with no command. Then with records signed after both; status
  prints the conflict and the printed command settles it.
- The old host's data folder is put back from a copy made before it received
  the takeover; it signs nothing until it has the yield back.
- A host forks its own history from a copy and continues from one branch.
- A public goal: the page, the door and the gallery row after a takeover, on
  the new host and on the old.
- A home from protocol 7 started under 8.

**Tests.** None beyond those of B1 and B2.

**Exit criteria.**
- Every recipe passes on fresh homes on two machines. This is the first
  recorded run of Locust with sleep and wake between two machines.
- Five people who have not used Locust read the explanation and answer: what
  happens to someone the old host admitted last; what a host who was asleep
  finds; what wins if the host had removed the backup, and until when.
- The release notes, the site and operations.md carry the sentences of text
  B-9.

**Risks and notes.**
- The takeover of a present host cannot be tested for intent, only for
  outcome.

## Terminal texts

Proposed output, not captured output.

**B-1. Naming and clearing a backup host**

On the host's computer. Both apply at once and ask nothing; the last line
undoes them. The lines in brackets print only for a member who came through
the public door (first command) and, from B2, while the backup's computer has
not reported holding this computer's latest records (second command). Planned
output.

```text
$ locust --owner backup set --goal "Static site search" --member Juniper
Juniper (8d03f2b6) is the backup host of "Static site search".
Juniper's owner can take over this goal at any time, alone, without asking you. If they do,
  whatever this computer signed that theirs had not received is dropped.
If you clear this or remove Juniper before this computer learns of a takeover, that wins and
  the takeover is dropped.
[Juniper came through the public door. If Juniper's owner takes over they are the host: tasks
  they write run without asking on members' computers set to auto, as yours do.]
Undo: locust --owner backup clear --goal 7f3a9c1e

$ locust --owner backup clear --goal 7f3a9c1e
"Static site search" has no backup host.
[Juniper's computer has not received this yet. If Juniper's owner has already taken over,
  this drops that takeover and what was done under it.]
Undo: locust --owner backup set --goal 7f3a9c1e --member 8d03f2b6
```

**B-2. What the host sees about its backup**

Lines of `goal status` on the host's computer, and the fact line added to the
result of `member remove`. The same fact line follows `backup clear`, `backup
set` for another member, `goal give` to another member and `goal keep`. Facts
only; nothing is listed under Waiting for you. With no backup the line is the
first one and plain `status` prints nothing.

```text
Backup host: none

Backup host: Juniper (8d03f2b6) · has received everything this computer signed

Backup host: Juniper (8d03f2b6) · has not received your last 3 changes, 1 of them a removal

$ locust --owner member remove --goal 7f3a9c1e --member Wren
…
Removed Wren from "Static site search". Copies already received cannot be retracted.
Juniper's computer (your backup host) has not received this yet. If it took over first, Wren
  would be a member again until it received this removal.
```

**B-3. Taking over**

On the backup host's computer. The command asks the other computers first, for
at most 20 seconds, then shows what it found and asks once. The two lines
after the first plan replace its 'Answered' lines in the two other cases; the
same yes proceeds. Planned output.

```text
$ locust --owner goal takeover --goal "Static site search"
Asking the goal's other computers… done.
Take over "Static site search" (7f3a9c1e) as host. You are its backup host, through Juniper.
Host now: Harbor's owner. Harbor's computer did not answer; last heard from 9 days ago.
Answered just now: Maple's computer.
Did not answer: Birch's computer (last seen 3 days ago).
This computer continues from the last thing Harbor's computer signed that it holds
  (2026-10-21 14:03 UTC). Anything signed there after that is dropped, even if it appears
  later: someone it admitted joins again; a member it removed is a member again until this
  computer receives that removal and repeats it.
If Harbor's owner removed you or named another backup host before their computer learns of
  this, the takeover and everything after it is dropped when that arrives.
Does not move: Harbor's owner's invitations and public page. Roles only Harbor holds (lead)
  pass to Juniper.
Plan id: plan-91c4e7a05d3b2f68
Proceed? [y/N] y
You host "Static site search" now. This computer keeps who is in and the rules; keep it on.
Harbor is still a member. Remove: locust --owner member remove --goal 7f3a9c1e --member Harbor
Invite: locust --owner goal invite --goal 7f3a9c1e

When nobody answered:
No other computer answered. This computer may be behind; what it has not received is dropped.

When a computer refuses this one:
Refuses this computer: Maple's computer. Juniper is not a member there. If Harbor's owner
  removed Juniper, this takeover is dropped as soon as it reaches Maple's computer.
```

**B-4. A host that was only asleep**

`status` on the old host's computer after it has received the takeover.
Nothing here waits for the person; the last line is the one thing they may
want to do. The second form of the first lines prints when the new host has
removed this computer's agent and the other computers refuse it.

```text
Static site search (7f3a9c1e) · host: Juniper's owner, on another computer
  Juniper's owner took over as host while this computer was away. You are a member.
  Dropped, because Juniper's computer had not received them:
    admitted Birch (3c5d7e9f): Birch joins again with a ticket from Juniper's owner
    removed Wren (a1b2c3d4): Juniper's computer repeats this when it receives it
    2 recorded file changes: the first lands again by itself; the one built on it must be
      rebuilt and approved again
  Your 1 open invitation was revoked. Your public page is blank now, and its address does not
    come back even if the goal is handed back to you. Delete it:
    locust --owner farm off --goal 7f3a9c1e

Static site search (7f3a9c1e) · host: Juniper's owner, on another computer
  Juniper's owner took over as host while this computer was away. Juniper's and Maple's
  computers refuse this one: Harbor is not a member there. Ask Juniper's owner.
```

**B-5. Someone the old host admitted too late**

`status` on the computer of a member whose admission a takeover set aside,
after it asked and was answered. The last block prints when the join had not
finished.

```text
Static site search (7f3a9c1e) · not a member
  Birch was admitted by a host that had already been replaced, so the admission was dropped.
  What Birch posted here does not count. Ask the host, Juniper's owner, for a new ticket.

Static site search (7f3a9c1e) · not joined
  The host that issued this ticket was replaced before the join finished. Ask whoever
  invited you for a new ticket.
```

**B-6. A takeover that the old host's removal voided**

`status` on the backup's computer after the removal reached it, and on the old
host's computer. The old host sees each removal the dropped takeover made,
with the command to make it again; nothing is repeated by itself.

```text
On the backup's computer:
Static site search (7f3a9c1e) · not a member
  Your takeover was dropped: Harbor's owner had removed Juniper before their computer
  learned of it. What this computer signed as host since then does not count.

On the old host's computer:
Static site search (7f3a9c1e) · host: you
  Juniper's owner took over while this computer was away, after you had removed Juniper.
  That takeover is dropped, with what it did:
    admitted Robin: Robin is not a member
    removed Wren: Wren is a member again. Remove:
      locust --owner member remove --goal 7f3a9c1e --member Wren
    changed the rules; 1 recorded file change
```

**B-7. Two takeovers that conflict**

`status`. The first block is for one person who signed both from two copies of
their data and signed something after each; when one of the two is empty the
daemon withdraws it and this never prints. The second block is for two
different people. The printed command runs as printed and asks.

```text
Waiting for you
  Two takeovers of "Static site search" conflict. Nobody can join or be removed, the rules
  cannot change, and members who worked under either one wait. Both were made from copies of
  this computer's data.
    4be19a02  continues from Harbor's record of 2026-10-21 · 43 records since
    9c07d1e3  continues from Harbor's record of 2026-10-19 · 2 records since
  Keep the first and withdraw the other:
    locust --owner goal takeover --goal 7f3a9c1e --keep 4be19a02

Waiting for you
  Two takeovers of "Static site search" conflict: yours and Maple's owner's. Nobody can join
  or be removed, the rules cannot change, and members who worked under either one wait.
    4be19a02  yours · 3 records since
    9c07d1e3  Maple's owner's · 43 records since
  Keep theirs and withdraw yours:
    locust --owner goal takeover --goal 7f3a9c1e --keep 9c07d1e3
  To keep yours, Maple's owner must withdraw theirs.
```

**B-8. Handing a goal over, and keeping it**

On the host's computer. Both apply at once and ask nothing. Until the other
person takes over, the goal admits nobody and records nothing new.

```text
$ locust --owner goal give --goal "Parser cleanup" --member Juniper
Handing "Parser cleanup" (3d9b6f20) to Juniper (8d03f2b6), on another computer.
Until Juniper's owner takes over, this computer admits nobody and records nothing new here.
Afterwards you are a member. Your invitations and public page do not move. Roles only Harbor
  holds (lead) pass to Juniper.
Juniper's owner runs: locust --owner goal takeover --goal 3d9b6f20
Undo: locust --owner goal keep --goal 3d9b6f20

$ locust --owner goal keep --goal 3d9b6f20
You keep hosting "Parser cleanup". Juniper is no longer its backup host. A takeover Juniper's
  owner made without having received this is dropped.
Undo: locust --owner goal give --goal 3d9b6f20 --member 8d03f2b6
```

**B-9. What version 8 tells people**

The refusal on a home written before version 8, the refusal of an old ticket,
and the release sentences. The home is not changed.

```text
$ locust --owner status
locust: unsupported_version: this Locust data was written by an earlier version.
Goals made before version 8 do not continue: nothing new can be recorded in them.
They are still in ~/.locust and can be read with the Locust that wrote them.
To start fresh with this version, move ~/.locust and ~/.locust.marks aside, then run:
  locust --owner up

$ locust --owner goal join --ticket-file old.ticket
locust: unsupported_version: this ticket was made by an earlier Locust. Goals from before
version 8 cannot be joined. Ask the host to start the goal again and send a new ticket.

Release notes and site:
Locust 8 adds backup hosts. Goals made before it do not continue: every member keeps their
copy and can read it with the earlier version, and nothing new is recorded in them. Before
you upgrade, end your public goals and take their pages down with the version that made them;
after the upgrade this computer can do neither. A later version that changes the signed
format again will end the goals made under this one.
```

**B-10. A host continuing from one of two histories**

`status` and the command on the computer of a host whose own governance log
has two branches, after a restore that was continued. The same command and one
yes; `--base` picks the other branch. Planned output.

```text
Static site search (7f3a9c1e) · host: you
  This computer signed two different histories as host here: it was started from an older
  copy of its data and continued. Nobody can join or be removed and the rules cannot change.
  Continue from one of them: locust --owner goal takeover --goal 7f3a9c1e

$ locust --owner goal takeover --goal 7f3a9c1e
Continue "Static site search" (7f3a9c1e) from one of two histories this computer signed.
  4be19a02  43 records after the point where they part · continue from this one
  9c07d1e3  1 record after it · dropped, with work built on it
To continue from the other: add --base 9c07d1e3
Plan id: plan-3e77a0c41b9d5f26
Proceed? [y/N] y
You host "Static site search" again. Invitations sent before this no longer work.
Invite: locust --owner goal invite --goal 7f3a9c1e
```

**B-11. What a new host sees afterwards**

Fact lines of `goal status` on the new host's computer. The first stays until
the earlier host's computer has received the takeover and is then replaced by
the second. The others print when they apply. None is listed under Waiting for
you.

```text
Host: you, since 2026-10-30 · Harbor's computer has not received your takeover yet. If
  Harbor's owner removed you or named another backup host before it does, the takeover and
  everything since is dropped.

Host: you, since 2026-10-30 · Harbor's computer has received your takeover.

Harbor (the earlier host's agent) is still a member. Remove:
  locust --owner member remove --goal 7f3a9c1e --member Harbor

Repeated a removal Harbor's computer had signed before you took over: Wren.

Harbor's computer had removed Maple before you took over. Maple is your backup host, so this
  computer did not repeat it. Remove:
  locust --owner member remove --goal 7f3a9c1e --member Maple
```

## Models to write first

Three models under research/tla, written and run before chain.rs is touched,
registered in research/tla/cases.json in the fast and extended suites and as
the suite `hostchange`, which B1 adds to scripts/check_tla.py and its tests.

1. `HostChange.tla`, a replay model in the form of Organization.tla: a fixed
   signed transcript per scenario, delivered in every order with duplicates.
   New here: delivery goes through a `Keep` function that restates `screen`,
   so retention is modelled. A delivered record that is not kept is offered
   again later. The signer is a function of position: governance keys 0
   (first), 6, 7, 8 and one key for a continuation; agents 5 (the first
   host's), 1, 2, 3. Kinds: genesis, admit, remove, rules, backup, change,
   withdraw, yield, end, a host step, and member work with an anchor and typed
   dependencies. A key epoch is a counter in the snapshot.
   Properties over held sets: `ReplayMatchesHeld`; `OneHostPerTerm`;
   `NothingAfterBase`; `NoPendingFromSetAside`; `RevocationByDescent` (a
   record that does not reach the base changes nothing); `YieldSeals`;
   `EndRule`; `KeptLogIgnoresForks`; `EpochFollowsChain`.
   Properties over retention: `KeepIsMonotone` (what was kept would be kept at
   any later held set); `RetentionConverges` (every order, run until no offer
   changes anything, ends with the same held set); `SameStandingWhenQuiet`;
   `NoPendingWhenQuiet` except for an undecided anchor.
   Properties over a delivery step: `UndoHasACause` (a change leaves force
   only on a step that completes one of the four named things);
   `NoSilentSwap`; `VoidStaysVoid`.
   Scenarios, each with a config: one change; old-key records after the base
   delivered before and after; an old-key fork below the base, at the base and
   above it; a removal of the backup that descends from the base, before and
   after the change is delivered, with work by a member under the void term
   and that member's next record; the same removal on another branch of a
   forked old log; an un-naming with a yield between; a yield and an un-naming
   on two branches; a rename with both the old and the new backup taking; a
   never-named member's change record; a member that names itself in its own
   log; two changes on two bases of one branch, then a withdrawal of each in
   turn, with a member's work under one of them; two changes on two branches
   of a forked log; a continuation from each branch of a forked last term; a
   backup named above a fork; a fork at position 0 of a new key's log and one
   above 0; an end at, before and after the base; a host step after the base
   and a decision after one predecessor by two keys; a second and a third
   change, then the first one revoked; a change with its base withheld.
   Mutations that must each fail their property: revocation ignored;
   revocation by position; set aside judged by anchor in place of log
   position; kept log read as the usable prefix; `Keep` drops a revoked change
   record; `Keep` accepts any change whose host some held backup record names;
   the walk lets a record through an ancestor anchored in a disputed term.

2. `HostChangeNodes.tla`, a small node model, because a restored honest daemon
   is a transition and not an assumption. Three computers (host, backup, one
   member) and a two-computer config; bounded logs. Actions: Sign, Name,
   Clear, Remove, RemoveOldHostsAgent, Sync(a, b) with the membership test of
   each side, Notice(a, b) with its answer, Takeover(b), Yield, Repeat,
   Withdraw, AutoWithdraw, Give, Keep, Continue (the repair), Copy,
   RestoreStore, RestoreAll, Override, Lose.
   Invariants and properties: `OneHostWhenQuiet` (any two computers that are
   not lost and where no Sync or Notice step between them changes anything
   compute the same last term); an ordinary start and a wake never block a
   takeover; a takeover repeated from the same base adds no record; no state
   reached without Override has two live changes or a fork in a new key's log;
   no unattended step signs a backup record or a removal of the backup; after
   a dispute settles either way every member can sign again; with the marks
   kept, a restored old host that had yielded signs nothing as host.
   Named traces kept as witnesses: a removed backup taking over from before
   its removal, then told by the first computer it reaches; two hosts that
   each removed the other, with three computers and with two; a restored
   backup with the marks kept; a restored backup with the marks lost taking
   over from the same base and from another, then the automatic withdrawal; a
   host asleep admitting after the base, the joiner asking; a handoff with
   nothing set aside; a keep after a give with a takeover in flight; a
   repeated removal whose old cutoff was anchored at a set-aside record; a
   repeated removal that would remove the new host's backup, not signed; and,
   left open on purpose, a removal held only by the lost host, a restored
   giver whose quiet was lost, and a restored old host that overrides and
   un-names after having yielded.

3. Cases added to existing models: in Workspace.tla an old-key acceptance
   after the base is not selected, a new-key acceptance after the same
   predecessor as a kept old-key one is disputed, and a new-key acceptance
   after a kept one is the head; in FlowEffects.tla a set-aside stage step
   never defines a task's round, and of two effective opening steps the one at
   the lower anchor does.

Not modelled, and said so in the model's page: the content of keys and which
key is served; frame sizes and the two-record limit of an answer; publication
and consent; that the Rust code refines the model. Workspace.tla does not sort
a stream, so the order of decisions across terms is pinned by the Rust replay
`a_new_terms_decision_after_a_kept_one_is_the_head`.

## Changes it asks of other plans

| Plan | Statement today | Becomes |
| --- | --- | --- |
| Master plan, Versions | Replacing a host takes protocol 8. | Replacing a host takes protocol 8, API 8 and store marker 8, all in B1. B2 adds one refusal value and no signed byte. Nothing is released between B1 and B3. A home, a peer or a ticket from 7 is refused; goals made under 7 do not load. A later step that lets several named people replace a host changes two of these records and ends the goals made under 8. The farm format and the formation schema do not change. |
| Master plan, Not designed yet | How a takeover works. The reviewers of the earlier design round agreed on a skeleton and left competing fixes for its hardest problems. | Designed as phases B1 to B3. Still not designed: a threshold among several named people, a standalone change of content key, moving a public page to a new key, telling a removed computer that no change of host concerns, a bound on change records made by a member that was once named. |
| Master plan, Assumed until the owner objects | The host cannot yet remove the agent they started a goal with. It can be disconnected and the goal keeps running. | The host's agent of the current term cannot leave or be removed. After a change of host to another person the old host's agent is an ordinary member. |
| Master plan, Decided by the owner, 12 | If a host removes their backup host and the backup takes over without having heard, the host's removal wins. The takeover is void when the removal surfaces. | Unchanged as a decision. The plan adds how it is read: naming another backup or none counts like a removal; the record counts only if it follows from the record the takeover continued from; and once the host's computer has received the takeover it says so by itself and the takeover is final. The owner is asked to confirm the last part. |
| Roles plan, Design still open | Decided: the record a takeover writes, what it sets aside, and what happens to work built on records it sets aside. Phases 8 and 9 are then written against it. | Decided. Phases 8 and 9 take three sentences. The authority of an agreed document and of the shared files is the governance key of the term the decision's anchor is in. A decision that a change of host set aside holds no place after its predecessor, so the stream does not stop. Two decisions by governance keys after one predecessor dispute that scope. |
| Roles plan, Phase 4, Changes | In projection.rs, `project` sorts a scope's decisions by `(anchor position, author, seq, id)`. | Unchanged, and B1 depends on it: a later term's decision is anchored at its change record or later, so it sorts after every kept decision of an earlier key. Phase 4 adds one sentence saying so, so the sort is not simplified back to the author's position. |
| Roles plan, Phase 8, Risks and notes | An excluded host decision still holds the place after its predecessor, so that stream stops until the next `rules bind`; an honest daemon signs none. | A host decision excluded on its content still holds the place. One that a change of host set aside holds none: the new host's daemon records after the last kept decision. |
| Roles plan, Phases 8 and 9, Changes | `resolve` sets `decisions.selection` to the host's agent (the governance key after K1). | `resolve` names the host through one accessor, `Chain::governance_at(anchor)`, which in protocol 7 answers `State.governance`. B1 changes that one function to read the snapshot at the anchor. |
| Roles plan, Phase 4, the first-files rule and the role fallback | The first files are a change by the host's agent (`History.host`); a declared role with no list, or an emptied list, gets the host agent. | Both read the host's agent of the term at the judged record's anchor. At a change of host to another person a list whose only holder was the old host's agent holds the new host's agent. |
| Roles plan, Phase 2, the two tiers | No command in this plan ends a goal; one that does will ask. | Added to the commands that ask: `goal takeover`, with and without `--keep`. Added to those that apply at once and print an `Undo:` line: `backup set`, `backup clear`, `goal give`, `goal keep`. |
| KEY (K1), Risks and notes | A fork in the governance log is permanent until a takeover exists. The host's agent cannot leave or be removed; lifting this belongs with replacing a host. | A fork in the last term's governance log halts governance until that term's host or its backup continues from one branch with `goal takeover`. K1's two replay rules read the term: an admission of any known governance key is excluded; a removal of the host's agent of the current term is excluded. |
| KEY (K1), node/local.rs and access.rs | New record tag `K` in `Space::Goal`, keyed by the goal, holding the 32-byte seed; `Local.governance: Option<Keypair>`; `Node::key_for`. | K1 lands as written and B1 does not change the record or the field. `key_for` also answers a later term's key by computing it from a local agent's seed and the held change record. `GoalStatus.governance` is the last term's key. |
| GUARD (G1), guard.rs | `Node::guard_sources`: `all` is every other computer; `host` is the host's computer when this daemon does not host the goal. | `host` is the endpoint in the admission of the host's agent of the last term. `all` leaves out the computer of an earlier term's host's agent whose yield is not held. `Hold` is unchanged. B1 writes G1's `UNHEARD` for a new governance key in two more cases: the change was signed in a goal that holds `RESTORED`, or this daemon finds itself host through a change record it has no local record of signing. `Guard::heard` is read by nobody new; the takeover plan keeps its own set. |
| END (E1), chain.rs and farm.rs | `Chain.cut_end` is the first `GoalEnded` of that log when `state.ended` is unset. The delete-only path of `FarmOff` serves an entry that holds an end, or whose governance key the restore guard holds. | `cut_end` reads the last term's log only. The arm marked `PLUG host-replacement` gains the test that a change record whose kept log holds an effective end is `AfterEnd`. The delete-only path also serves a page this daemon published in a goal it no longer hosts. |
| END, the explanation | Only the host can end a goal, so if the host's computer is lost the goal can never be ended. | Only the host can end a goal. If the host's computer is lost, a backup host who takes over can. If the host had ended the goal and the backup had not received the end, the takeover continues the goal and the end is dropped. |
| Public goals contract and J7 | A door member can never be named backup host; the chain excludes a naming record or a takeover whose backup's admission at the named base is `via: Door`. | A host may name someone who came through the door, with `backup set`, whose result says so. The rule is the same as for any backup. While an ordinary member, that agent's tasks ask first. Once it hosts, its tasks and its term's stage tasks are the host's; the owner is asked to confirm. J7 takes from this design: the door record, the tickets and the page do not move; the publication is none after a change to another person; `Refusal::Replaced` and the joiner's sentences; the consent rule below. |
| Public goals, consent and the page | A public goal is lost with its host's computer. Publishing needs a consent from every active member and every author of effective work. | A public goal is lost with its host's computer unless the host named a backup host. A backup host who takes over publishes a new page at a new address. For a publication record signed in a later term, the agents of earlier hosts are not asked for consent and are shown as 'earlier host' unless they consent. On a joinable page the new host's publication record is the consent, and the freshness test reads the kept chain's first joinable publication record. |

## What it owns, removes and checks

Owns:

- The terms: term, backup host, change record, base, kept log, path, placed, authorized, continuation, revokes, withdrawn, live, in force, disputed, set aside, undecided, marker, notice; the person's words host, backup host, take over, hand over, dropped
- The record kinds `BackupSet`, `HostChanged`, `TakeoverWithdrawn` and `HostYielded`, `Body::is_marker`, their `Header::check` rules and the domain `HOST_CHANGE_SIGNATURE`
- `Keypair::governance_for` and `ContentKey::opened_by_change`, and the rule that a later term's key is computed, never stored
- The rewrite of `Chain::build` as a walk over terms, `step`, `Chain::fold_path`, and `Snapshot.governance`, `Snapshot.host`, `Snapshot.backup`, `Snapshot.key`
- `State.backup`, `State.terms` and `Term`
- `Exclusion::SetAside`, `Revoked`, `Withdrawn`, `NotBackup`, `Unplaced`; `Halt::HostDispute`; the rule that no change record and no marker is ever pending
- The standing of a marker, `Goal::marker_place`, and the rule that a marker is never set aside
- The ancestry-walk rule: an ancestor anchored at a set-aside record is skipped; one anchored at an undecided record keeps the record pending
- The rule that a host step is judged by the term of its anchor, the one index entry for all governance keys in the decision index, and the order of `current_round`
- What a change of host clears and passes, and what a continuation keeps
- The end rule at E1's plug: `AfterEnd` for a change whose kept log holds an end; `cut_end` reads the last term only
- `History::kept`, `History::tips`, `History::descends`, `Goal::takeover_bases`, and what `Goal::next` answers for a governance key that is not the last term's or is under a dispute
- What `screen` keeps: known governance keys, the keys they admitted, and a change record by the exact test
- Content keys stored and requested by the record that opened them, and the rule that a key is served and wanted only when its opening record is effective
- `Invitation.issuer` and the checks that read it; `Refusal::Replaced` and `Refusal::HostChanging`
- The local records `T`, `q`, `g`, `b` and `p` in `Space::Goal`, `Tx::reach` and `Node::asked`
- The requests `backup.set`, `goal.takeover.ask`, `goal.takeover.plan`, `goal.takeover`, `goal.takeover.withdraw`, `goal.give`, `goal.keep`
- The commands `backup set`, `backup clear`, `goal takeover` with `--base` and `--keep`, `goal give`, `goal keep`; which ask
- The automatic acts: the yield, the withdrawal of an empty duplicate, the repeat of a set-aside removal; and the rule that no unattended signature is a `BackupSet` or a removal of the backup host
- `SyncMessage::Notice` and `NoticeAnswer`, `Host::evidence`, `Evidence`, `Host::receive_notice`, `Host::notice_answered`, who asks and who is told, and what a responder answers
- The rule for `speaks_for_member` while a term is disputed, and that `peers()` is empty where no local agent is a member or joining
- The consent rule for a page published after a change of host
- `GoalStatus.backup`, `terms`, `takeover`, `backup_holds`, `giving`, `replaced`, `dropped`, `voided`, `repeated`, `not_repeated`; `Halt::HostConflict`; `Membership::Dropped`
- The texts B-1 to B-11
- The models `research/tla/HostChange.tla`, `HostChangeNodes.tla` and their cases, and the suite `hostchange` in `scripts/check_tla.py`
- The protocol, API and store step from 7 to 8 and the sentences that say goals made before it do not continue

Removed or rewritten:

- `History.administrator` and `State.administrator` as the one signer (already renamed by R1 and K1): every remaining reader goes through `snapshot(anchor)` or `State.terms`
- The body of `Chain::build` that reads one key's usable prefix: replaced by the walk over terms and `step`
- `screen` reading one administrator: replaced by known governance keys and the exact test for a change record
- `Entry.keys: BTreeMap<u32, ContentKey>`, `key_write(goal, epoch, key)` and `key_subject`: replaced by the record-keyed store
- `SyncMessage::KeyRequest { epoch }` and `Key { epoch, key }`: replaced; the paragraph of sync.rs that defines a key by its epoch number is rewritten, and the line that describes the evidence exchange gains the notice
- `Host::halt_proofs` and `Dialed.proof` as a pair of events: replaced by `Host::evidence` and `Evidence`
- `historical_endpoints` reading one key's log
- The eleven characterization tests of the host-key note, in the form K1 and G1 leave them, are each re-read; the three named under B1's tests are rewritten
- K1's risk sentence that a governance fork is permanent until a takeover exists; K1's and the master plan's sentence that the host's agent can never be removed
- The roles plan's `Design still open` section: the third bullet and the paragraph `Still open` go
- END's sentence that a lost host's goal can never be ended; the public-goals sentence that a goal is lost with its host's computer, without the clause about a backup
- docs/status.md and docs/guide/operations.md: every sentence that says a goal's host cannot be replaced
- The research note's sections 7, 8 and 10 gain a line pointing at the plan that settled them; the note itself is kept
- `python3 scripts/check_formations.py --write` regenerates the runtime contract; vectors.rs is regenerated in B1
- From the first draft and not built: `Hold::Giving`, `Node::reach`, the local record `n`, a stored seed for later terms, `--alone`, `SyncMessage::Notice(WireEvent)` as a one-way push

| Behavior | Shown by |
| --- | --- |
| Naming, changing and clearing a backup are governance records that apply at once; a removal clears the name; `goal status` says `Backup host: none` and plain status says nothing | Replay `a_backup_is_named_changed_and_cleared_and_removal_clears_it`; cli test `backup_set_and_clear_apply_at_once_and_print_undo`; presentation test `goal_status_states_the_backup_or_none_and_status_says_nothing`; no entry under Waiting for you in any fixture with no backup |
| Only the member named at the base, or the term's own host, can start a term, and nobody else's change record is even kept | Replays `a_change_by_the_named_backup_starts_a_term_and_one_by_anyone_else_is_not_kept` and `a_member_that_names_itself_in_its_own_log_cannot_make_a_kept_change`; node test `a_notice_claim_that_is_not_authorized_is_not_kept`; model mutation `Keep accepts any change whose host some held backup record names` must fail |
| Whatever the old key signed after the base is set aside in any arrival order | Replay `what_the_old_key_signed_after_the_base_is_set_aside_in_any_arrival_order`, forward, reversed, reloaded; model property `NothingAfterBase` |
| Work built on set-aside records is excluded, never pending, and its author is not stranded, on every computer whichever record arrived first | Replays `work_built_on_a_set_aside_record_is_excluded_and_its_authors_next_record_counts` and `a_void_change_and_its_terms_records_are_kept_and_set_aside_whichever_arrives_first`; model properties `NoPendingFromSetAside`, `RetentionConverges`, `SameStandingWhenQuiet`; mutation `Keep drops a revoked change record` must fail |
| The host's removal or un-naming of the backup voids a takeover when it descends from the base, and not when it sits on another branch | Replays `a_removal_or_unnaming_that_descends_from_the_base_voids_the_takeover_whenever_it_arrives`, `a_removal_on_another_branch_of_a_forked_old_log_voids_nothing`, `voiding_the_first_change_voids_the_second_and_third`; model property `RevocationByDescent`; mutation `revocation by position` must fail |
| A takeover is final once the earlier host's computer has received it | Replays `a_yield_between_the_base_and_an_unnaming_seals_the_takeover` and `a_voided_takeover_never_comes_back`; node test `the_earlier_host_yields_by_itself_and_a_restored_copy_of_it_is_behind`; model properties `YieldSeals` and `VoidStaysVoid`; B1's fourth exit drill; B3's restored-old-host recipe |
| A takeover leaves force only for one of four named causes | Model property `UndoHasACause` over every delivery step of every scenario |
| Two computers that can reach each other end with one host, also when each removed the other | Node tests `two_hosts_that_each_removed_the_other_end_with_one_host`, `a_removed_backup_that_takes_over_is_told_by_the_first_computer_it_reaches`, `a_removed_earlier_host_is_told_and_its_yield_comes_back`; model property `OneHostWhenQuiet`; B1's fifth and sixth exit drills |
| Taking over twice from one base is one record, and a restored new host waits to hear before it signs as host | Node tests `taking_over_twice_from_one_base_signs_one_record`, `a_restored_new_host_waits_to_hear_before_it_signs_as_host`, `a_takeover_in_a_restored_goal_holds_its_new_key`; frozen vector for a signed `HostChanged`; B3 recipe with the marks kept and with the whole home copied |
| Two takeovers from two bases halt governance; an empty duplicate is withdrawn without a command; one command settles the rest; members sign again after either outcome | Replays `two_live_changes_dispute_the_term_and_a_withdrawal_settles_it_either_way` and `a_member_who_signed_under_a_disputed_term_waits_and_signs_again_after_either_outcome`; node tests `an_empty_duplicate_takeover_is_withdrawn_without_a_command` and `during_a_dispute_no_key_opened_under_it_is_served_and_a_member_it_removed_is_refused`; presentation test for text B-7; B3 recipe |
| A fork of the old key at or below the base changes nothing; a host or a backup continues from one branch of a forked last term | Replays `a_fork_of_the_old_key_at_or_below_the_base_changes_nothing`, `a_host_continues_from_one_branch_and_the_halt_is_lifted`, `a_backup_named_above_a_fork_takes_over_from_that_branch`; model property `KeptLogIgnoresForks`; B1's seventh exit drill |
| After a takeover members can write text even when the old host held the only copy of the newest key | Node tests `after_a_takeover_members_write_text_when_the_last_key_reached_nobody` and `keys_are_stored_and_asked_for_by_the_record_that_opened_them` |
| An end at or before the base stands; an end after it is dropped | Replay `a_change_based_at_or_after_an_end_never_counts_and_an_end_after_the_base_is_set_aside`; model property `EndRule`; node test `takeover_refuses_with_an_end_a_hold_or_a_held_removal_of_its_agent` |
| Recordings after the base are set aside, the new host's daemon records again, and its decision is the head | Node test `landed_changes_after_the_base_land_again_or_are_reported_behind`; replays `a_host_step_is_judged_by_the_term_of_its_anchor` and `a_new_terms_decision_after_a_kept_one_is_the_head`; the Workspace and FlowEffects cases |
| A computer the change drops asks why, is answered and stops dialing; a join that did not finish is told the host was replaced | Node tests `a_joiner_the_old_host_admitted_after_the_base_asks_and_stops_dialing` and `a_join_that_did_not_finish_is_told_the_host_was_replaced`; driver test `a_notice_is_answered_before_done_and_alternates_with_the_refused_exchange`; B2's first exit drill |
| The new host's computer repeats a removal the old host had signed, keeps what counted, and never removes its own backup or a role holder unasked | Node tests `the_new_host_repeats_a_set_aside_removal_with_its_own_cutoff_and_keeps_what_counted`, `a_set_aside_removal_of_the_backup_or_a_role_holder_is_reported_not_signed`, `a_removal_from_a_void_term_is_not_repeated`, `a_member_admitted_again_is_not_removed_by_an_old_removal`; node model invariant on unattended steps; B2's second exit drill |
| The host sees which of its changes the backup reported holding and is never told more than is true | Node test `the_host_knows_which_changes_its_backup_reported_and_is_never_ahead`; presentation test for text B-2 |
| A handoff drops nothing; keeping takes the offer back | Node tests `giving_quiets_the_hosts_key_across_a_restart_until_the_takeover_or_keep`, `a_handoff_drops_nothing_and_the_giver_yields`, `keep_names_the_earlier_backup_again_and_voids_a_takeover_in_flight`; B2's third exit drill |
| Invitations and the page do not move, and a new host can publish | Node tests `a_ticket_names_its_issuer_and_a_replaced_hosts_ticket_is_refused`, `a_replaced_host_revokes_its_invitations_and_can_delete_its_page`, `a_new_host_publishes_without_the_lost_hosts_consent`; replay `a_change_opens_a_key_epoch_and_clears_backup_and_publication_and_a_continuation_keeps_them`; B2's fourth exit drill; B3's public-goal recipe |
| A goal made before protocol 8 does not load and its home is not touched | `incompatible_event_protocol_refuses_open_before_collecting_or_rewriting_state` and `another_protocol_version_is_refused`, unedited; B1's last exit criterion; B3's last recipe |
| People can predict what a sleeping host finds and until when its word wins | B3: five people read the explanation and answer three questions |

## Left for later

- A threshold among several named people. The slot is the two fields this
  version fills with one value: `BackupSet.backup` becomes a list of members
  with a count, and `HostChanged.endorsement` becomes a list of approvals.
  Nothing is reserved now; a reserved field would be dormant code. Under the
  no-migration rule that step takes its own protocol number and ends the goals
  made under 8. To decide first, from the research: whether naming an elector
  can be undone by a later host record (it can here, by descent, until the
  yield); that an approval has no end date; what two certificates mean; the
  sizes allowed; whether the farm service may be a named elector for a public
  goal.
- A bound on the change records a once-named member can make, as a pure rule.
- Another end for the owner's decision 18 when the old host never returns, if
  the owner wants one.
- Telling a removed computer of its removal when no change of host is
  involved, and telling a removed backup before it acts. The notice answer is
  the place: one more case in what a responder answers.
- Moving a public page to the new host's key, by letting the farm service read
  the signed chain. Until then a takeover means a new address.
- A standalone record that changes the content key without removing anyone or
  changing the host.
- One exchange with the backup's computer before a host's first unattended
  signature after a wake.
- Showing an author the text of their own set-aside records for posting again,
  and what a connected folder does when the files head moves back.

## The designer's questions for the owner

Recorded as the designer wrote them. They have not been put to the owner.
Several are about internal choices that a person using Locust would not
notice; those are the plan author's to settle.

1. Your answer 18 has no end as written. This design gives it one: the removal
   or un-naming wins until the host's computer has received the takeover; that
   computer then says so by itself and the takeover is final. If the host's
   computer never returns, the takeover is never final: any member who holds
   an unsent removal of the backup can deliver it at any time, the later
   history is dropped, the goal returns to the lost host, and members may be
   unable to write text. Accept both halves? Recommended: yes. The first half
   is new; the second is your rule carried through.
2. `backup set` and `goal give` apply at once and end in an `Undo:` line, like
   giving a role. The first draft asked first; both reviewers read your
   answers 9 and 22 the other way. Confirm? Recommended: yes.
3. `goal takeover` has no `--alone`. The plan lists who answered, who refused
   this computer and who did not answer, and one yes covers all three. A
   backup that was removed and never told can so sign a takeover; it is
   dropped as soon as it reaches any computer that holds the removal. Accept?
   Recommended: yes; it asks least, and nothing rests on the refusal.
4. The new host's signing key is computed from their agent's key, the goal and
   the point they continue from. A repeated takeover is then the same record
   and a restore cannot lose the key. The cost: that key is no stronger than
   the agent's. Accept? Recommended: yes; both already sit in the same folder
   with no passphrase.
5. When the new host's computer later receives a removal the old host had
   signed, it repeats it by itself, except for its own backup host or someone
   it gave a role, where it states the fact and prints the command. What the
   member did in between stays. Accept? Recommended: yes.
6. A host whose own history has two conflicting branches, after a restore that
   was continued, can go on from one of them with `goal takeover` and one yes.
   Today and under K1 that goal is frozen for good. Accept? Recommended: yes.
7. When one person has made two takeovers from two copies and one of them has
   nothing signed after it, their computer withdraws that one by itself.
   Accept the unattended step? Recommended: yes; it removes a prompt and drops
   nothing.
8. A backup host who came through the public door and takes over is the host
   like any other: tasks they write run without asking on members' computers
   set to auto, and people they invite count as invited by the host. The
   result of `backup set` tells the naming host. This reads your answers 12
   and 22 together. Confirm, or should the door mark stay with them and with
   whoever they admit? Recommended: confirm; the other reading makes every
   stage of their goal ask on every computer.
9. Roles that only the old host's agent held pass to the new host's agent at a
   takeover. Accept? Recommended: yes.
10. After a takeover the public page gets a new address. Under today's consent
    rule every member but the earlier host's agent consents again; that agent
    is shown as 'earlier host'. The old page of a lost host keeps its last
    content until the farm service removes it. Accept for this version?
    Recommended: yes.
11. If two different people hold live takeovers of one term, one must
    withdraw; if that person is gone, members and rules stay frozen and those
    who worked under either wait. It takes a forked history that named a
    different backup on each side, or a host's repair made on a stale copy.
    Accept as a stated limit? Recommended: accept.
12. A member you once named backup host and who turns hostile can fill the
    goal's sync with dropped takeovers, and removing them does not stop it.
    This version has no rule against it, because a named person is taken as
    honest. Accept for this version? Recommended: accept, and bound it in a
    later step.
13. A named backup can take over while the host's computer is on and
    answering; the plan says it answered and does not refuse. Confirm?
    Recommended: yes.
14. The step after this one, several named people replacing a host together,
    changes two signed records and will again end the goals made before it.
    The release text of version 8 says that a later change of signed format
    ends its goals. Confirm the sentence?
15. On a home from before version 8, Locust 8 refuses to start and prints
    where the old goals are; it does not move the folder. Confirm?
    Recommended: yes.
16. Are the words right: backup host, take over, hand over, dropped, earlier
    host?

## How the 30 earlier breaks were answered

- Safety 1 (fatal, two hosts never converge): holds. Changed: a void change
  record is never dropped; the notice is a question with an answer; a computer
  refused by one it counts a member's asks why; every computer tells an
  earlier host that has not yielded and is answered with what revokes the
  takeover; the notice moved into B1. S4c traces the three-computer,
  two-computer and lost-backup variants; `OneHostWhenQuiet` is in the node
  model.
- Safety 2 (fatal, S4a strands members by arrival order): holds. Changed:
  `screen` keeps a change record whenever its path is held and it is
  authorized there, void or not, with its key's records and those of the keys
  it admitted. S4a, S4b and text B-6 are rewritten against that one rule;
  retention is in the replay model.
- Safety 3 (the repeated removal undoes too much): holds, all three parts.
  Changed: the repeat computes its own cutoff on the new chain as
  `member_remove` does; it is not signed against the backup host or a role
  holder of the current term, which is shown as a fact; the rule that no
  unattended signature is a `BackupSet` or a removal of the backup is stated;
  the repeat is in the node model.
- Safety 4 (a takeover never becomes final): the restored-old-host sequence
  holds and is closed by `HostYielded`: the yield seals the takeover against
  later old-key records and puts G0's mark ahead of any older copy, so G1
  holds a restored old host with no change to G1. The withheld-record sequence
  with a host that never returns holds and stands as a stated limit; the
  liveness line is corrected and the owner is asked. The notice route no
  longer accepts a bare removal, so a removed member cannot deliver one.
- Safety 5 (same-base restore freezes the goal): holds. Changed: a change
  signed in a goal that holds `RESTORED`, and a key this daemon finds itself
  host by without having signed, get G1's `UNHEARD`; a term's host can
  continue from one branch of its own forked log with the same record, so the
  fork is no longer final.
- Safety 6 (forked log cannot be repaired through the command): holds.
  Changed: `Goal::takeover_bases` gives the branch tips at which the agent is
  backup or host along that branch's own kept log; the request checks
  membership in that list, not `state().backup`; `--base` is specified and the
  default is shown in the plan as this computer's choice.
- Safety 7 (a dispute stops more and leaks keys): holds. Changed: the texts
  and liveness say everyone who signed under either takeover waits; a key is
  served only when its opening record is effective; under a dispute a member
  removed by a disputed term's record is refused; the walk rule for an
  undecided anchor is stated and modelled with the property that every member
  signs again after either outcome; a host step under a dispute is undecided;
  an empty duplicate is withdrawn by the daemon. That a once-named member can
  suspend later terms years later from an old copy, past two overrides, stands
  as a limit.
- Safety 8 (receipt test lets one member stop sync): holds for the first
  draft's test. Changed: the exact test, so only a member the host named can
  make a kept change record. The residue, a once-named member minting one per
  base, is a stated limit under the fault model and an owner question.
- Safety 9 (`goal keep` does not cancel): holds. Changed: `goal keep` signs
  `BackupSet` back to the earlier value, which voids a takeover in flight by
  descent; the result line says so.
- Safety 10 (host told too much): holds. Changed: the host notes the position
  the backup's computer itself reported in its frontier, not its own tip at
  the end of an exchange.
- Safety 11 (notice marked delivered; removed old host told it is a member):
  holds. Changed: there is no delivered mark; telling an earlier host ends
  when its yield is held; a join that did not finish gets `Refusal::Replaced`
  and its own sentence; text B-4 has a second form for a computer the others
  refuse.
- Safety 12 (three open points of the command): holds. Changed: the command
  refuses on a held record that removes or un-names its agent after the base,
  also past a gap; the payload is the empty text; the command asks, then polls
  a read-only plan until nothing waits or 20 seconds pass; the host's computer
  counts as answering.
- Safety 13 (two friction points): holds. Changed: `backup set` applies at
  once; a door member who hosts is the host like any other, with the question
  put to the owner.
- Code 1 (fatal, screen contradicts S4a and B-6): the same break as Safety 2
  and the same change.
- Code 2 (streams stop at the old key's last decision): holds on today's code
  and not on the tree B1 lands on. Roles Phase 4 already sorts a scope's
  decisions by anchor position first, and a new key's step is anchored at its
  change record or later, so it follows every kept decision. B1 names
  projection.rs, pins the order with a replay and a Workspace case, adds a
  sentence to Phase 4, and gives `current_round` the same order.
- Code 3 (revocation by position): holds. Changed to descent from the base,
  with the links carried in notice answers; S18 is the reviewer's sequence
  with the new outcome.
- Code 4 (a removed backup using --alone is never told): holds. Changed: the
  refused computer asks with its change record as claim, the responder keeps
  it as void and answers with the removal; B1 holds the notice and its drill.
- Code 5 (members wait in a dispute; the withdrawal cannot land): holds.
  Changed: liveness and text B-7 say who waits; `TakeoverWithdrawn` names its
  change, takes that change's anchor, is signed through `Goal::marker_place`
  and is effective as a marker, so the write path accepts it; a member's Sign
  action under dispute is in the node model.
- Code 6 (the new host can never publish): holds under today's rule. Changed:
  for a publication record signed in a later term the earlier hosts' agents
  are not asked and are shown as 'earlier host'; on a joinable page the host's
  record is the consent; the cost sentence is corrected; J7 gets the rule and
  a test with a lost old host.
- Code 7 (the rule is wider than the explanation says): holds. The rule is
  kept and bounded by the yield; the explanation says 'before their computer
  learned of the takeover'; the old host's commands that could drop a takeover
  print a fact line while the backup's computer has not reported holding them;
  the owner is told a lost host's takeover is never final.
- Code 8 (copied cutoff drops the member's history): holds. Changed: the
  repeat computes its own cutoff. It is not limited to the old cutoff's
  ancestry, because that would retract approvals the new host's computer had
  already recorded on.
- Code 9 (notice goes too wide, retries, marks delivery wrongly): holds.
  Changed: only two cases open a notice, a refused computer asking and telling
  an earlier host; no fork proof is pushed for an earlier term's key; no
  delivered mark; the answer says whether the claim was kept.
- Code 10 (the plan does not fit the node or the guard): holds. Changed:
  `goal.takeover.ask` is a small write that marks pairs due through
  `Tx::reach`; `goal.takeover.plan` is read-only over its own set; the command
  stops on its own 20-second deadline; G1's heard set is untouched; the host's
  computer counts; `--alone` is folded into the yes.
- Code 11 (`goal give` and `goal keep`): holds. Changed: keep un-names; giving
  is its own durable local record with its own refusal and words, not a guard
  hold; text B-8 names the roles that pass; a restored giver that loses the
  quiet is a stated limit and a model trace.
- Code 12 (S15 and --base contradict the request check): the contradiction
  holds and is fixed by specifying `takeover_bases`. `--base` is kept, not
  dropped: with the usable tip as the only base, one stray record signed by a
  restored host would drop the whole branch beside it. The person types no
  flag unless they want the other branch.
- Code 13 (confirmations and Undo lines): holds. Changed: `backup set`,
  `backup clear`, `goal give` and `goal keep` apply at once and end in
  `Undo:`; `--alone` is gone; `goal takeover` and `--keep` still ask.
- Code 14 (a lost old host stays a member and keeps rules waiting): holds.
  Changed: the takeover result and the new host's status state that the
  earlier host's agent is still a member, with the remove command;
  `guard_sources` does not wait for an earlier host's computer that has not
  yielded.
- Code 15 (the door limit is friction without protection): holds. Changed: no
  extra limit once the door member hosts; the result of `backup set` says so;
  the owner is asked.
- Code 16 (four sentences that are not what happens): holds. All four are
  rewritten: the lost host's page keeps its content; of chained file changes
  the first lands again and the rest are rebuilt; a handed-back goal gets a
  new address; text B-7 has a form for two different people.
- Code 17 (form): holds. Changed: the suite is added to check_tla.py and its
  tests in B1; requests are `goal.takeover.ask`, `.plan`, `goal.takeover` and
  `goal.takeover.withdraw`; the indices are stated as an assumption; the
  release text and the owner are told the next step ends goals again; domain
  strings follow `locust v0`.

## Not settled

- Nothing was built or run. K1, G1, G2, E1 to E3 and R1 to R10 are plan text;
  every function, field and test name taken from them may differ on the tree
  B1 lands on, and B1 must be read again against that tree. All sequences are
  hand traces.
- That an Ed25519 signature made by this code is the same on every run.
  crypto.rs calls the library's plain `sign`; the workspace pins ed25519-dalek
  3.0. I did not run it. The event id does not depend on it, but the
  endorsement inside the header does.
- That the transport lets a responder's frame reach the dialer on an evidence
  exchange before `Done`, and what limit it applies to that frame. Today the
  responder already sends `Refused` on that exchange and the dialer reads it;
  I read the driver and the responder, not the shell code that applies the
  two-header limit. A notice with a claim and an admission is two headers plus
  about 40 bytes.
- That ordinary sync carries both records of a fork in a governance key's log
  to every member. It is measured for a member's log only. The branch tips of
  S15 and the convergence of S18 rest on it.
- That a fresh joiner converges after a second or third change. Frames carry
  one author each, so a change record whose base arrives in a later frame is
  dropped once and needs another exchange per term. I expect it converges; not
  traced through the initiator and outbox.
- The claim that what is kept converges is argued and is modelled for the
  listed scenarios. It is not proven for the code, and `screen` now folds a
  path, which is more than it does today.
- Whether roles Phase 4 lands the decision sort as its text says, by anchor
  position first. The order of decisions across terms rests on it; B1 pins it
  with a replay.
- The leak in a notice answer: a computer admitted under a takeover that was
  later dropped is shown the old host's records between the base and the
  record that dropped it. I judged it acceptable and stated it. An alternative
  is one record and a local doubt, with no exact answer for that computer.
- The yield as a rule of validity. A yield and an un-naming on two branches of
  the old key's log can only come from a restore of the old host's computer; I
  chose that the un-naming then wins, so that a voided takeover never comes
  back. The other choice keeps the takeover but lets a revoked one return on a
  later delivery.
- Whether the own cutoff of a repeated removal always passes `Chain::cutoff`
  on the new chain. It is computed as `member_remove` does today, from the
  member's last effective record there, so it should; not run.
- The automatic withdrawal reads 'has a record of its key after it held here'.
  On a restored copy that signed one host step before syncing, both takeovers
  have records and the person is asked. I did not find a sound rule that picks
  without asking in that case.
- Passing sole-held roles to the new host's agent treats a list of exactly the
  old host's agent as the fallback. A host who gave itself a role on purpose
  loses it at a takeover.
- A continuation keeps the publication on the chain. I did not check the
  publisher's behaviour when the governance key changes under an unchanged
  publication record, or whether members' held consents then still match.
- What `eligible` should show for an earlier host's agent on a page under the
  public-goals rewrite; I read lines 280 to 305 of the contract only. Whether
  an alias can be reclaimed while the old page lives is open.
- That the farm service removes a page that stopped checking in rests on the
  planned quiet retention; the service today expires only closed pages.
- How connected folders on members' disks behave when the head of the files
  moves back, at an undo or when an acceptance is set aside. Not designed and
  not read.
- The 20-second wait of the takeover command is the command's own clock and
  decides nothing shared. Whether 20 seconds is long enough for two exchanges
  with a computer behind a relay is a guess.
- Whether a daemon that cannot open its store can still answer `status` with
  text B-9. Not read.
- Every size is a judgement. B1 is larger than K1, G1 and G2 together.
