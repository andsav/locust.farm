# Host safety and ending a goal: implementation plan

Status: proposed plan of 2026-10-06, one of the plans under the
[Locust v2 master plan](master-plan.md). Not accepted and nothing here is
built. It holds five phases: K1 gives a goal's members and rules a signing key
of their own, G1 and G2 stop a computer started from an old copy of its data
from signing until it has caught up, and E1 and E2 let the host end a goal
and have the host's computer remove a member that leaves. A sixth phase, E3,
would let idle goals use the network less. It is deferred until after v2 and
named under [Left for later](#left-for-later). The research behind it is in
[replacing a host](../research/replacing-a-host-2026-10-05.md),
[ending a goal](../research/ending-a-goal-2026-10-05.md),
[host-key failures](../research/host-key-failure-characterization-2026-10-05.md)
and [goal lifecycle](../research/goal-lifecycle-characterization-2026-10-05.md).

Each of the three parts was written by one designer and checked against the
code by a second reader. A third reader then compared the three with each
other and with the [roles plan](roles-and-permissions-plan.md) and found 28
places where they disagreed; the fixes are applied in the text below. Code
was read at `c9c3b2c` and again at `48bb12c`. Nothing was built or run, so
every behavior after the change is inferred. The terminal texts are proposed
output, not captured output.

An [independent review](../research/v2-plan-review-2026-10-06.md) of 6
October 2026 then said not to build the phases unchanged. Its
[check](../research/v2-plan-review-verification-2026-10-06.md) refuted none
of its findings. On the same day the owner narrowed answer 3, restated answer
18, gave answers 26 and 27 and chose to keep the restore guard as planned.
The text below is corrected for all of this. In short: the note of what a
computer last signed is forced to disk; on a host's computer a copy of
unknown age waits for the person and for nothing else; a disconnected agent
can be connected again; a stage rule that nobody could meet is refused when
the rules are set; each phase repairs the recipes and scripts it breaks; and
the phase for idle goals is deferred.

What each phase owns and removes, the checks behind its behaviors, and what
its writers could not settle are in the
[companion list](host-safety-and-ending-plan-details.md).

## Known gaps

What the text below leaves open after the corrections of 6 October 2026. None
of these is fixed here.

1. **After a restore, a goal can wait for a person.** Answer 26 says the swarm
   never stops to wait for a person. The restore guard is the exception, and
   the owner kept it on 6 October 2026 (master plan, "What answer 26
   changes"). After a whole-computer restore or a move the host's computer
   cannot tell how old its copy is. It signs nothing in the goals it hosts
   until the person runs `locust --owner goal continue`, and nothing else
   ends that hold. Until then the other members' agents still post and
   review, but nobody joins, no stage step is taken and no approved change is
   recorded. After a data folder alone is put back, a hold ends by itself
   when the missing record returns from another computer. A computer can
   answer and still not send it: the model kept the case where the only
   other holder was removed since, and a removed computer's exchanges
   finish and bring nothing. Where no computer that answers sends the
   record back, only the same command ends the hold, and status lists the
   goal under "Waiting for you" with that command. Going on under a new
   key would need no person. It is scoped in the
   [check of the review](../research/v2-plan-review-verification-2026-10-06.md),
   is not built in v2 and belongs with the backup host.
2. **The page of an ended goal cannot be corrected from a host whose data went
   back.** E1 marks the page ended whenever the host's copy holds an end, and
   lets a blank page say so. A host whose data went back numbers its requests
   to the farm service below what the service holds, and the service refuses
   them. Such a page keeps what the service last held until the joinable plan
   lets a delete, and a suspend that says ended, be signed above the service's
   number.
3. **Nothing in v2 brings back a goal that has lost its host.** A host's
   computer that is lost, a copy of the data older than the goal, and two
   records at one position of the host's log each stop joining, removing,
   rule changes and the end of that goal for good. A conflict in the records
   of the agent a goal was started with does the same to that goal's first
   files: `workspace init` shares them for that agent only, and it cannot be
   removed. The later release in which a host can be replaced is no way back
   for any of these, because under answer 2 it does not read goals made under
   v2.

Closed since this list was first written:

- E2 had the host remove a member that left by hand, against answer 14. E2 is
  rewritten.
- K1 counted two records that the host's computer signs by itself. The section
  [What the host's computer signs by
  itself](#what-the-hosts-computer-signs-by-itself) lists all six, with one
  rule for all of them.
- An end that a fork cut had no page state. E1 now marks the page ended
  whenever the host's copy holds an end. Gap 2 is what is left of it.
- G1's release rule did not work for a public goal. After a whole-computer
  restore the host's computer waited to hear from every other computer, and a
  member who joined through a public door and never returned blocked that for
  good. The rule is removed. On the host's computer such a hold now waits for
  no other computer, and gap 1 says what that costs.
- The note of what a computer last signed was not forced to disk, so a power
  failure followed by a restore could pass the guard. G1 now syncs the note
  before the commit returns.
- Disconnecting the agent a goal was started with could not be undone, and it
  stranded the goal's first files and that agent's consent to a page. K1 adds
  the command that connects an agent again.
- A hand-written rule could name the creator of a stage's task where only a
  member can act, and nobody could meet it. K1 refuses such rules when they
  are set.

## Intended behavior

**The signing key.** The key itself brings nothing new to type or learn. The
host is the person who started the goal, and their computer keeps who is in
and the rules. Disconnecting the agent you started a goal with no longer
freezes the goal: invitations, removals and rule changes keep working from
your computer. That agent stays a member of the goal and cannot be removed
from it. A role nobody else holds is held by that agent, and one command
gives it to another member. Disconnecting an agent applies at once and prints
the command that connects it again, under the same name and with the same
key. No other computer learns of either. While that agent is disconnected it
does no work: nobody uses the roles only it holds, the goal's first files
cannot be shared, and it cannot give or change its consent to a public page.
Connecting it again ends all three, so disconnecting it strands nothing. A
second agent of yours in the goal is a second member. If you started a goal
alone, disconnected its agent and went on with another, a result under the
default rule needs an approval that the disconnected agent cannot give:
connect it again, add a third agent, or set rules that need no approval. A
copy of your Locust data directory holds everything needed to act as the host
of your goals, so keep copies private and never run a copy beside the
original. A copy made before you started a goal cannot bring that goal back.
Where no newer copy is left, nothing in v2 brings its host back: the later
release in which a host can be replaced does not read goals made under v2.

**The restore guard.** Locust keeps a small note beside its data folder, in
`~/.locust.marks`, of the last thing this computer signed in each goal, as
host and for each of your agents. Each change to the note is forced to disk
before the record it covers is sent anywhere. If the data folder is ever put
back from an older copy, Locust sees that the note is ahead, signs nothing in
the goals that are behind, and catches up from the other members' computers
by itself. In a goal you host, your agents wait too while the host's own
records are missing. If the whole computer was restored or moved, so the note
is a copy too, Locust cannot tell how old its data is. In a goal you host it
then signs nothing until you run one command, `locust --owner goal continue`.
Nothing else ends that wait, because an old copy cannot know which computers
joined after it was made, and status lists the goal under "Waiting for you".
In a goal someone else hosts, your agents wait until this computer has heard
from the host's computer or from every other member's computer, and need
nothing from you while one of those answers. An ordinary restart or waking
from sleep changes nothing and asks you nothing. A copy also brings back the
settings it held: levels, allowed tasks, folders, and which agents were
disconnected or had left a goal. Never run two copies of the same Locust data
at once; nothing can protect a goal from that.

**Ending a goal.** The host can end a goal with one command, which asks the
host for a yes first and cannot be undone. After that nothing new is recorded
in the goal on any computer that has learned of the end: no tasks, results or
approvals, no new members or rule changes, and nobody joins. Ending deletes
nothing: every member keeps their copy and can still read it, and work a
member signed before their computer learned of the end still counts when it
arrives. A public page stays up marked "Ended by the host" until the host
takes it down or the farm service removes it, by default 30 days after the
service learns of the end. If the page can no longer show the goal's work it
is blank and still reads "Ended by the host". No single name can be taken off
it after the end. Only the host can end a goal, so if the host's computer is
lost the goal can never be ended.

**Leaving a goal.** A person takes their agent out of a goal with one
command, which asks that person for a yes first. The host does nothing. The
host's computer removes that member by itself, as soon as it has the leave
and has had one chance to fetch what the member last posted. Work the member
posted before leaving stays and counts as before. Nothing it signs afterwards
counts. Its roles go back to the host's agent, and the other computers stop
exchanging with the member's computer, which keeps its copy. No new content
key is made for a leave. While the host's computer is off or catching up, the
member stays listed, marked as left. To come back, the person joins with a
new invitation.

## Decided by the owner

The numbers are those of the [master plan](master-plan.md).

- 2: no migration. No release reads goals made under an earlier signed
  format. So the later release in which a host can be replaced ends the goals
  made under v2 and is no way back for any of them.
- 3: no computer decides anything shared from its own clock, a timeout, the
  order records arrived in or a comparison of identifiers. The host's
  computer may choose among changes that each already count, by signing one
  record that every other computer follows: when several approved changes
  build on the same version it records the one with the lowest identifier
  among those it holds, and the other authors rebuild (narrowed on 6 October
  2026).
- 8: the failure to design for is a host that disappears, not a hostile one.
- 9: a goal's members and rules are signed by a key that does nothing else,
  separate from the host's working agent, kept in the Locust data folder
  with no passphrase. The master plan leaves to this plan what else that key
  may sign. It also signs what the host's computer records by itself, and
  never a member's work; see
  [What the host's computer signs by itself](#what-the-hosts-computer-signs-by-itself).
- 13: after a whole-computer restore or a move, for a goal with no other
  computer to ask, Locust signs nothing there until the person runs one
  command. On 6 October 2026 the owner kept the restore guard as planned, and
  the master plan records what follows: after such a restore or move every
  goal a person hosts waits for one command from that person.
- 14: a member whose agent signs a leave is removed by the host's computer
  automatically. E2 builds it.
- 15: an approved change is recorded by the host's computer by itself.
- 18: a task written by someone who came through the public door becomes
  available to the swarm once a trusted agent approves it, and no person is
  asked (restated on 6 October 2026). The public-goals phases build that
  rule. It adds nothing to what the host's computer signs: the approval is a
  member's record, and the restore guard and the end apply to it as to any
  other.
- 23: when the host ends a public goal its page stays, marked ended, for 30
  days and is then removed by the farm service.
- 26: the swarm never stops to ask a human for anything. In this plan a
  command asks only the person who typed it, and no agent's work waits for a
  yes from its own person. What still waits for a person follows a restore of
  old data or that person's own act. Known gap 1 and the companion's notes on
  the restore guard say what, and the guard is kept as planned.
- 27: minimize friction and what a person has to do. So disconnecting an
  agent applies at once and one command undoes it, a command that would be
  refused shows no plan first, and the phase for idle goals, which no answer
  asks for, is deferred.

Everything else in this plan is its writers' recommendation. The open ones
are listed under [Questions for the owner](#questions-for-the-owner).

## Implementation sequence

Five phases inside the fifteen of the master plan's build order, where R is a
phase of the roles plan: R1, R2, R3, **K1**, R4, R5, R6, **G1**, **G2**,
**E1**, **E2**, R7, R8, R9, R10. Each lands on a clean tree: a phase
rewrites, in the same phase, the executable recipes and the script harness
code that call an interface it removes or changes, and its exit criteria run
them. The protocol version goes from 6 to 7 in K1. E1's end record is the
last change of the event format within the fifteen phases. The first public
door changes signed bytes once more, and the master plan says under which
number.

| Phase | What works afterwards | Depends on |
| --- | --- | --- |
| K1 | A goal's members and rules are signed by a key of their own; the agent that started the goal is an ordinary member, and a disconnected agent can be connected again | R1 to R3 |
| G1 | A computer knows what it signed; started from an older copy, it signs nothing in the affected goals until it has caught up, and after a whole-computer restore or a move a goal it hosts waits for the person | R1, K1 |
| G2 | Status and refusals say "catching up"; the person has one command to continue | G1, R2 to R6 |
| E1 | The host ends a goal with one command; nothing new is recorded on any computer that has learned of it | R1 to R6, K1, G1, G2 |
| E2 | A member that leaves is removed by the host's computer with nobody asked; disconnecting the agent a goal was started with says what waits until it is connected again | E1 |

Deferred until after v2: E3, in which a goal with no new records dials and
checks in less often. No answer of the owner asks for it and no other phase
reads its code. Until it is built every goal keeps today's 30 seconds. E3
would also have slowed the retries of a computer that was removed. Until then
that rests on the failure backoff of the public-goals phase J1, so J1 keeps
it. [Left for later](#left-for-later) says what a person notices without E3.

## What the host's computer signs by itself

The governance key signs two sorts of record. Most are asked for by a command
the person typed. The rest the host's daemon signs with nobody present. This
table is the whole list of the second sort across every v2 plan: six
situations, in four kinds of record. Three of them change who is in. All six
go into the governance key's log. A second, different record at a used
position of that log stops members and rules for good, and v2 cannot replace a
host. So every row says what makes a second signature harmless, or why nothing
can.

"Twice" means signed again from the same inputs: the position, the record
before it, the anchor, the records the act names and, where the record answers
a request, the same request.

| | Record | Phase | Signed today | The same if signed twice | What stops a second, different record | What this plan asks |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | An admission on an invitation, with the role the ticket carries | K1, Phase 4 | Yes, with the creator's agent's key and a clock reading (`plan_join`) | Yes, from K1. It carries no clock reading. Member, endpoint, name and role come from the request and the ticket | The ticket's record, written in the commit that admits, answers a retry and signs nothing. After a found restore the guard holds the key and the copy's pending tickets are revoked (G1). The cost of that: an admission the copy lost is not answered again once its ticket is revoked (G1's notes) | K1 signs it with no clock reading. Phase 4 puts the role in this record and signs no second one beside it. Which joiner takes the next position is the order of asking, and nothing changes that |
| 2 | A stage's steps: opening its task, offering it, asking the reviewers' agents for a review of its result | K1 | Yes, with the creator's agent's key (`drive_flow`) | Yes. Measured: `restored_host_automatic_stage_replay_is_identical_unless_another_event_used_its_position` | A step this copy holds is not wanted again. The goal is read again before each step. The guard passes over a held key | Nothing. When two copies hold different records the step that takes a position can differ, and no change to the record removes that |
| 3 | An approved change to the shared plan | Phase 8 | No. A member selects by hand | Yes from the same records, once Phase 8 signs it with no clock reading | Nothing is wanted while a record by this key follows the last one. The goal is read again before each signing. The guard | Phase 8 signs it with no clock reading and adds a restore test. It cannot be the same across two copies: it records the lowest identifier among the revisions that have arrived and count, which answer 3 allows |
| 4 | An approved change to the shared files, the first files included | Phase 9 | No. The integrator accepts by hand | As row 3, and only with the same content on this computer | As row 3. At another position a second record disputes the files only | As row 3 |
| 5 | The removal of a member whose agent signed a leave | E2 | No. `member remove` is a command | Yes. It names the member, its admission and the leave, and carries no clock reading, no payload and no new content key | A member the copy shows as removed has not left, so no removal is wanted again. The goal is read again before each. The guard | Nothing more. A member whose log holds two leaves on two branches gets no such record, and the host removes it by hand |
| 6 | An admission through a public door | J2 | No | As row 1 | The member list answers a retry. While the guard holds the key the door is closed with that reason, and it opens again by itself when the hold ends | As row 1, and a restore test of its own. G1's test covers tickets only |

Outside the log the governance key signs one more thing with nobody present:
the description of an open door, each time it is derived (J1, J3). It has no
position and cannot conflict. It stays outside the log.

With other keys the host's computer also signs by itself. A review request for
a result on a task no stage opened is signed with the author's agent's key, as
on every member's computer; a conflict there costs that agent its key in the
goal. The page's requests to the farm service are signed with the page's key;
a conflict there costs the page. The daemon's endpoint key signs receipts, and
a joining agent's key signs its request again on each retry. Neither is a
record in a log.

**The rule.** Every later phase keeps these five points for a record the
host's daemon signs with the governance key while nobody is present.

1. **It is in this table.** A phase that adds such a record adds its row in
   the same change, with every column filled. A record with no row is signed
   only inside a command the person typed.
2. **It passes the one gate.** It is signed through `next_place`, after the
   goal was read again, and one act goes into one commit. So an end, a hold of
   the restore guard and a halt each stop it. Each of them is tested before
   the signature, so a record that cannot be signed yet is passed over and is
   never an error. No record is signed at a place worked out beside the gate.
3. **Its bytes come only from the records this copy holds and from the request
   it answers.** It carries no clock reading and no value drawn at the moment
   of signing. The same act from the same inputs is then the same record at
   the same position, and signing it twice does no harm.
4. **It is not a second record where the first can carry it.** What one record
   signed with nobody present can say, a second one does not say again.
5. **If it cannot be the same twice, its row says why, and it has a restore
   test.** A record that picks among what has arrived (rows 3 and 4, and the
   order of joiners in rows 1 and 6) cannot meet point 3 across two copies of
   the data. For those only the marks and the restore guard stand between a
   restore and a halt. A phase adds one only if no record that meets point 3
   would do, and it adds a test in which a host started from an older copy
   signs none of them before it has caught up. A record that needs a new
   secret is not signed with nobody present at all. That is why row 5 draws no
   content key.

What enforces the rule. Points 2 and 3 have one place in the code: K1's
`Node::author_alone`, which takes no clock reading and no text and signs
through `next_place`. `plan_join` and `drive_flow` are its only callers, and
K1's exit criteria search for any other. Each row that says yes has a test
that signs from two stores and compares the two records:
`a_stage_step_signed_twice_from_one_input_is_one_record` (G1),
`an_admission_signed_twice_for_one_request_is_one_record` (K1),
`a_removal_that_follows_a_leave_signed_twice_from_one_input_is_one_record`
(E2), and one each in Phases 8 and 9. Points 1 and 4 are conventions that a
reader of a phase checks.

**One key and one log, in v2.** Rows 2 to 4 are in this log because K1 keeps
one key for members, for rules and for what the host's computer records by
itself. The master plan leaves that choice to this section, not to the owner.
It is kept. A second key for rows 2 to 4 was laid out and not taken. It would
put them in a log of their own, where a second record at a used position stops
stages and recordings in that goal and leaves members, rules and the end. That
is a real gain. These are the records signed most often, they are signed at
once when a hold ends, and rows 3 and 4 cannot be the same across two copies.
It is not taken in v2 for three reasons. Stages, the shared plan and the
shared files would still stop for good in such a goal, so it could be ended
and not worked in; lifting that needs a record that replaces the second key,
which is a change of host in small. The one design for replacing a host cuts
off members, rules and recordings with one base in one log. Going on under a
new key after a restore uses the same record; it is scoped in the
[check of the review](../research/v2-plan-review-verification-2026-10-06.md)
and is not built in v2. And a second key adds a key, a log in every
frontier, a mark and one more state for a person to read, with no fewer
signatures. The choice is taken again when replacing a host becomes phases.
The five points apply to a second log unchanged.

What keeping one key costs, in plain words. A host's whole computer is
restored from a copy while the only member's computer that holds the host's
later records is off. The person runs `goal continue`. The daemon signs the
first due recording at once, at a position the lost log already used. When the
member's computer returns, both hold two records at that position: nobody
joins or is removed, the rules never change, and the goal cannot be ended. The
plan of `goal continue` says this before it asks the person for a yes. After
a whole-computer restore that command is the only way on in a goal the person
hosts (G1), so what protects the goal there is the person waiting until the
other members' computers have been on. The other ways to the same end are
those G1 lists under what can still fork.

Three changes follow from this section. K1 adds `author_alone` and signs an
admission with no clock reading. Phase 4 puts the role a ticket carries in the
admission record and signs no role record with an admission. Phases 8 and 9
sign their records with no clock reading, and each gains the restore test of
point 5; G1 already has it for admissions on a ticket and for a stage's steps.

## Terminal texts

The example is the roles plan's: a person with two agents, called Maple and
Juniper in goals, hosts "Parser cleanup".

**K1-1. Starting a goal**

The lines of P2-1 that this piece changes; every other line is the roles
plan's. Planned output. The words `through codex-maple-1a2b3c4d` are gone from
both Host lines.

```text
$ locust --owner goal create --title "Parser cleanup" --formation peer-review \
    --agent codex-maple-1a2b3c4d
Start a goal: Parser cleanup
...
Host: you. This computer keeps who is in and the rules.
Plan id: plan-5c0e91a7d2b44f18
Proceed? [y/N] y
Started "Parser cleanup" (3d9b6f20). Host: you. This computer keeps who is in and the rules.
```

**K1-2. Disconnecting the host's agent and connecting it again**

Planned output. From this piece `agent revoke` applies at once: it shows no
plan, asks nobody and prints the command that undoes it. E2 adds the lines it
prints for the agent a goal was started with. The lines marked ... are the
plan of `goal invite`, which Phase 2 words; that command asks the person
typing it for a yes. Before this piece it exits 3 with `the host agent is
disconnected; nothing can sign for this goal`. The third and fourth commands
are refused before any plan. The third prints Phase 1's sentence. The fourth
says that only the disconnected agent can share this goal's first files and
names the command that connects it again. The last command is that one: the
agent comes back with the name and the key it had, and `workspace init` then
shows its plan.

```text
$ locust --owner agent revoke --agent codex-maple-1a2b3c4d
codex-maple-1a2b3c4d is disconnected. The name stays taken.
Undo: locust --owner agent reconnect --agent codex-maple-1a2b3c4d

$ locust --owner goal invite --goal "Parser cleanup"
...
Proceed? [y/N] y
locust-invite-9f2c41d7…

$ locust --owner member remove --goal "Parser cleanup" --member codex-maple-1a2b3c4d
locust: conflict: the host's agent cannot be removed from its own goal

$ locust --owner workspace init --goal "Parser cleanup" --root ~/parser
locust: conflict: codex-maple-1a2b3c4d is disconnected; only codex-maple-1a2b3c4d can share this goal's first files
  Connect it again: locust --owner agent reconnect --agent codex-maple-1a2b3c4d

$ locust --owner agent reconnect --agent codex-maple-1a2b3c4d
codex-maple-1a2b3c4d is connected again.
Undo: locust --owner agent revoke --agent codex-maple-1a2b3c4d
```

**K1-3. Records the host's computer signed**

Planned output of `events`, in today's line form (identifier, kind, author,
standing), with Phase 4's names. Identifiers are cut here with …; the command
prints them whole. A record signed by the governance key prints `host` and no
key.

```text
$ locust --owner events --goal 3d9b6f20
5b1e09c2… · genesis · host · effective
0c44a7d1… · member_admitted · host · effective
9a03f6be… · rules_bound · host · effective
71d2c80a… · task_opened · Maple (e47b90d1) · effective
e6f0913b… · member_admitted · host · effective
2c7a55d4… · review_recorded · Juniper (8d03f2b6) · effective
```

**K1-4. Goal status for a script**

The three fields of `goal status --json` this piece sets, on a member's
computer. `governance` appears only here and in other JSON; the text form
prints `Host: Harbor (51c2e9aa) · another computer` as in P4-2. `host` is null
until the goal's first record has arrived. Keys are cut with ….

```text
$ locust --owner --json goal status --goal c01d5b7e
{
  "goal": "c01d5b7e…",
  "governance": "a94f03c7…",
  "host": "51c2e9aa…",
  "hosted_here": false,
  "host_name": "Harbor",
  ...
}
```

**K1-5. Joining, with no host key shown**

The Host line of P3-2 as this piece leaves it, after Phases 1 to 3. Phase 4
adds the host's agent's name. Planned output.

```text
Join "Static site search" as codex-maple-1a2b3c4d.
  Goal:    Static site search (7f3a9c1e)
  Host:    on another computer. The ticket's signature is verified.
  Expires: in 6 days (2026-10-12 14:03 UTC)
```

**G-1. Status after the data folder was put back from an older copy**

Proposed output of `locust --owner status` on the host's computer. The marks
survived, so Locust knows exactly what is missing and holds only this goal.
The missing records are the host's own, so the host's agents wait too. Cedar
is a friend's agent on another computer. The hold waits for a computer and for
no person: it ends when the missing records come back from any computer in the
goal, and the block names the computers not heard from yet. Nothing is listed
under 'Waiting for you', because a computer is left that can end the hold. The
second block is the same goal after Cedar's computer answered; the restored
line stays until the next ordinary start. No text names a key: the heading
reads `host: you` and the records read 'signed as host'. Names and identifiers
follow the roles plan's example.

```text
$ locust --owner status
Nothing is waiting for you.

Parser cleanup (c01d55aa) · host: you
  Catching up: this computer's Locust data is older than what it signed here.
    Missing: 2 records this computer signed as host. Nothing is signed here until they
    come back from another computer in the goal.
    Heard from since this start: nobody yet.
    Not yet: Cedar's computer (2f6b90c4), last seen 2 days ago.
    To continue without them: locust --owner goal continue --goal c01d55aa
  Restored from a copy: 1 invitation was revoked, because a copy cannot know whether it
    was used. Levels, allowed tasks, connected folders, and which agents are disconnected
    or have left are as they were in the copy.
  Maple (codex-maple-1a2b3c4d) · lead, reviewer · auto
      posts, reviews, decides; takes tasks on its own

After Cedar's computer answered:
Parser cleanup (c01d55aa) · host: you
  Restored from a copy: 1 invitation was revoked, because a copy cannot know whether it
    was used. Levels, allowed tasks, connected folders, and which agents are disconnected
    or have left are as they were in the copy.
  Maple (codex-maple-1a2b3c4d) · lead, reviewer · auto
      posts, reviews, decides; takes tasks on its own
```

**G-2. Status after a move to a new computer**

Proposed output. Data and marks were both copied, so Locust cannot tell how
old the copy is. In a goal this computer hosts only the person ends that hold,
so the person's own goal is listed under 'Waiting for you' from the start,
also after Cedar's computer has answered. The goal hosted by Ana waits for her
computer and needs nothing from the person. The restored line of G-1 is
printed under both goals and is left out here to fit.

```text
$ locust --owner status
Waiting for you
  "Parser cleanup" is catching up: this computer's Locust data may be an old copy, and only
  you can say it is the newest
    locust --owner goal continue --goal c01d55aa

Parser cleanup (c01d55aa) · host: you
  Catching up: this computer's Locust data may be an old copy.
    Waiting for you: only you can say this is the newest copy of this computer's data.
    Heard from since this start: Cedar's computer (2f6b90c4).
    To continue: locust --owner goal continue --goal c01d55aa
  Maple (codex-maple-1a2b3c4d) · lead, reviewer · auto

Static site search (7f3a9c1e) · host: Harbor's owner, on another computer
  Catching up: this computer's Locust data may be an old copy.
    Waiting to hear from the host's computer (Harbor), last seen 3 hours ago. Nothing is
    needed from you.
    To continue without it: locust --owner goal continue --goal 7f3a9c1e
  Maple (codex-maple-1a2b3c4d) · member · auto
```

**G-3. The override: goal continue**

Proposed output. The command asks the person who typed it: it shows a plan and
proceeds on a yes, because what is signed after it cannot be undone with one
command. It prints no Undo line. The line 'They may include ...' and the two
lines from 'Wait until' are printed when the held key is the host's. After a
copy of unknown age the line on missing records reads: This copy may be older
than what this computer signed here, and nothing on this computer can tell.
The plan's review holds no clock reading, so its id is the same on a second
run and stops matching once the hold is over. `--all` prints one such block
per goal that is catching up and asks that person once.

```text
$ locust --owner goal continue --goal "Parser cleanup"
Goal: Parser cleanup (c01d55aa) · host: you
Continue: sign in this goal from this computer's copy of the data.
  This copy is missing 2 records that this computer signed as host.
  They may include a removal, a rule change or the end of the goal.
  Not heard from since this start: Cedar's computer (2f6b90c4), last seen 2 days ago.
  Wait until the computers of the members you added most recently have been on. A member
  added after this copy was made is not listed here, and its computer may hold them.
  If another computer holds one of those records, the next record signed here
  conflicts with it. If these are the host's records, that stops joining, removing and
  rule changes in this goal for everyone, for good.
  Safe when this is the newest copy of this computer's Locust data and no other copy
  is running.
Plan id: plan-7d1e0c44a9b35f02
Proceed? [y/N] y
Continued "Parser cleanup". This computer signs here again.

$ locust --owner goal continue --goal "Parser cleanup"
"Parser cleanup" is not catching up. Nothing changed.
```

**G-4. The refusal in two voices, and what a joiner is told**

Proposed output. One refusal with the fifth side, 'this computer', rendered by
the roles plan's `render`: P for the owner with `--owner`, A as the daemon's
message in a tool result and from the launcher. The second P line is a host
command refused before any plan: it reads 'You' and names no key; the words of
the act are Phase 5's. The third P line and the second A line are the same
refusal after a copy of unknown age in a goal this computer hosts, where the
hold waits for the person and does not end by itself. The last block is the
sentence the joinable plan prints on a joiner's computer while the host's
computer answers `catching_up`; that plan owns the field it is read from.

```text
P  Maple can't post to "Parser cleanup": the Locust data here is older than what this
   computer signed in the goal (this computer). It catches up by itself. To go on
   without waiting: locust --owner goal continue --goal c01d55aa
P  You can't change the rules of "Parser cleanup": the Locust data here is older than
   what this computer signed in the goal (this computer). It catches up by itself. To go
   on without waiting: locust --owner goal continue --goal c01d55aa
A  codex-maple-1a2b3c4d can't post to this goal: the Locust data here is older than what
   this computer signed in the goal (this computer). It catches up by itself, or
   codex-maple-1a2b3c4d's owner can continue without waiting.
P  Maple can't post to "Parser cleanup": the Locust data here may be an old copy (this
   computer). It waits for you. To continue: locust --owner goal continue --goal c01d55aa
A  codex-maple-1a2b3c4d can't post to this goal: the Locust data here may be an old copy
   (this computer). It waits for codex-maple-1a2b3c4d's owner, who can continue with one
   command.

$ locust-cli contribution publish --goal c01d55aa --summary "notes"; echo $?
locust: read_only: codex-maple-1a2b3c4d can't post to this goal: the Locust data here is
older than what this computer signed in the goal (this computer). It catches up by itself,
or codex-maple-1a2b3c4d's owner can continue without waiting.
9

On a joiner's computer:
  Juniper (claude-juniper-77aa0c52) · joining
      The host's computer is catching up and admits nobody yet. Locust asks the host's
      computer again by itself.
```

**G-5. A copy older than the goal's first member**

Proposed output on the host's computer. The marks survived and say the goal
had members, but the copy lists none, so there is nobody to dial. The goal is
not listed under 'Waiting for you': continuing would break it, and it catches
up as soon as a member's computer connects.

```text
$ locust --owner status
Nothing is waiting for you.

Parser cleanup (c01d55aa) · host: you
  Catching up: this computer's Locust data is older than what it signed here.
    Missing: 3 records this computer signed as host. This copy is older than the goal's
    first member, so it knows no computer to ask. It catches up when a member's
    computer connects.
    To continue without them: locust --owner goal continue --goal c01d55aa
  Maple (codex-maple-1a2b3c4d) · lead, reviewer · auto
      posts, reviews, decides; takes tasks on its own
```

**G-6. Three short lines: a lost goal, a new member, an agent in conflict**

Proposed output, three separate cases in `locust --owner status`. First, after
the goals: the copy put back is older than goals this computer took part in;
the line lasts until the next ordinary start. Second: an agent that was just
admitted while the host's computer has not answered yet, or while the goal's
rules or current content key have not arrived; it prints one line, no block
and no command. Third: an agent with two records at one position in a
goal; the goal is not halted, and the host's agent gets the second sentence.

```text
This copy of the Locust data is older than 2 goals this computer took part in. A goal you
hosted cannot be brought back from it. A goal you joined needs its ticket again.

Static site search (7f3a9c1e) · host: Harbor's owner, on another computer
  Juniper (claude-juniper-77aa0c52) · member · auto
      Just admitted: checking with the host's computer and fetching the goal's rules.

Parser cleanup (c01d55aa) · host: you
  Juniper (claude-juniper-77aa0c52)
      Juniper can sign nothing more here: two of its records conflict. Join with another
      agent.
  Maple (codex-maple-1a2b3c4d)
      Maple can sign nothing more here: two of its records conflict. Give its roles to
      another member.
```

**E1-1. The host ends a goal**

Proposed output, typed by the host of "Parser cleanup". The first line's host
wording is K1's: `Host: you`, with no agent named. The page lines print only
when a page is on; with none the line reads: Public page: none. An ended goal
cannot be published later. With no open invitation the line reads: Open
invitations: none. With no member on another computer the last two lines of
the result are not printed. Lines are wrapped here to fit; the command prints
one line per sentence.

```text
$ locust --owner goal end --goal "Parser cleanup"
End "Parser cleanup" (c01d55aa) for everyone. Host: you.
Members: Juniper (8d03f2b6), Maple (e47b90d1).
After the end nothing new is recorded in this goal: no tasks, results, approvals, members or
  rule changes, and nobody joins. Automatic steps stop.
Nothing is deleted. Every member keeps their copy and can read it.
Still counts: what a member signed before their computer learned of the end, when it arrives.
Open invitations: 1. It is revoked.
Public page: on. It stays up marked "Ended by the host" until the farm service removes it
  (30 days after it learns of the end unless its operator set another period). No name can
  be taken off it after the end. Take the page down now or later:
  locust --owner farm off --goal c01d55aa
This cannot be undone.
Plan id: plan-7be2a90c41d6f3a8
Proceed? [y/N] y
Ended "Parser cleanup". Nothing new is recorded in it. Every member keeps a copy.
1 invitation revoked.
Members' computers learn of the end when they next reach this one; keep it on until then.
  locust --owner goal status --goal c01d55aa shows when each last synchronized.

$ locust --owner goal end --goal "Parser cleanup"
"Parser cleanup" has already ended.
```

**E1-2. Status after the end, and a refused post**

Proposed output. First a member's computer, then the host's, whose heading
reads `host: you` (K1). One sentence after a goal's agents stands in place of
each agent's level and standing line. P is the person's voice and A the
daemon's message to an agent, both from the roles plan's one refusal template.

```text
# on your computer, a member of Ana's goal
$ locust --owner status
Nothing is waiting for you.

Static site search (7f3a9c1e) · host: Harbor's owner, on another computer · ended by the host
  Maple (codex-maple-1a2b3c4d) · member
      Nothing new is recorded here. Your copy stays readable.

$ locust --owner goal status --goal "Static site search"
Static site search (7f3a9c1e)
Ended by the host (record 5a11c0de). Nothing new is recorded; your copy stays readable.
Host: Harbor (51c2e9aa) · another computer
...

# on the host's computer
Parser cleanup (c01d55aa) · host: you · ended by you
  Maple (codex-maple-1a2b3c4d) · lead, reviewer
  Juniper (claude-juniper-77aa0c52) · member
      Nothing new is recorded here. Every member keeps a copy.

P  Maple can't post to "Static site search": the host ended this goal (the goal's state).
   Nothing to change; pick other work.
A  codex-maple-1a2b3c4d can't post to this goal: the host ended this goal (the goal's state).
   Nothing to change; pick other work.

$ locust-cli pending --goal 7f3a9c1e
The host ended this goal. Nothing is pending.
```

**E1-3. Ended, then halted; a halted goal; a goal hosted elsewhere**

Proposed output. The first block is a member's computer that holds an end
record and a fork of the host's record at or before it. The second is the host
of a halted goal that holds no end. The third is a member trying to end a
goal; its sentence is the roles plan's first refusal of host().

```text
Static site search (7f3a9c1e) · host: Harbor's owner, on another computer · ended by the host, then halted
  Maple (codex-maple-1a2b3c4d) · member
      The host's record has two entries at one position, at or before the end.
      Nothing new is recorded here. Your copy stays readable.

$ locust --owner goal end --goal "Parser cleanup"
halted: this goal is halted; the host can sign nothing more for it, the end included

$ locust --owner goal end --goal "Static site search"
denied: this goal is hosted on another computer; its host decides
```

**E1-4. Taking the page down after the end**

Proposed output on the host's computer. After the end `farm off` signs
nothing, so members' copies of the goal keep the page's address. The same path
runs while this computer is catching up; the first line then ends `(catching
up)` and the second sentence reads `Nothing is signed, because this computer
is catching up`. The delete is one more numbered request to the farm service.
From a copy whose page number is behind the service's it is refused, until
the joinable plan lets a delete be signed above the service's number (E1's
note on a restored host).

```text
$ locust --owner farm off --goal c01d55aa
Take down the public page of "Parser cleanup" (ended).
The farm service deletes the page. Nothing is signed, because the goal has ended: members'
  computers keep the page's address, which stops answering.
Plan id: plan-19c4e07a5b3d2f66
Proceed? [y/N] y
Asked the farm service to delete the page of "Parser cleanup".
  locust --owner farm status shows the receipt.
```

**E2-1. A member leaves and the host's computer removes it**

Proposed output. The first block is the leaving member's computer; its first
result line is the roles plan's and the next three are new. Juniper is that
computer's only agent in the goal. Where another agent of the same computer
stays in the goal, the line that says to keep the computer on is not printed.
The second block is the same computer's status before and after the host's
computer has refused it as not a member. P and A are the refusal an agent
that left gets, from the roles plan's one template. The third block is the
host's computer: nothing waits at any point, `events` shows the leave and
the removal that the host's computer signed by itself, and the member is gone
from `goal status`. The last line is a third member's computer that holds
the leave and not yet the removal. One refusal is deleted with this phase and
replaced by nothing: `conflict: the departure must be acknowledged by removal
before rejoining`. An agent that left joins again with the roles plan's
ordinary `goal join` texts.

```text
# on the member's computer
$ locust --owner goal leave --goal "Static site search" --agent claude-juniper-77aa0c52
...
Juniper left "Static site search". Copies already received stay with the goal.
The host's computer removes Juniper by itself once it has the leave. Nobody has to do it.
Keep this computer on until locust --owner status says the host's computer has the leave.
To come back, join with a new invitation.

$ locust --owner status
Nothing is waiting for you.

Static site search (7f3a9c1e) · host: Harbor's owner, on another computer
  Juniper (claude-juniper-77aa0c52) · left
      The host's computer does not have the leave yet. Keep this computer on.

# the same agent afterwards
  Juniper (claude-juniper-77aa0c52) · left
      The host's computer has the leave. This computer gets nothing new from the goal.
      Its copy stays readable. To come back, join with a new invitation.

P  Juniper can't post to "Static site search": this agent left the goal (the goal's state).
   Nothing to change; pick other work.
A  claude-juniper-77aa0c52 can't post to this goal: this agent left the goal (the goal's
   state). Nothing to change; pick other work.

# on the host's computer
$ locust --owner status
Nothing is waiting for you.
...

$ locust --owner events --goal 7f3a9c1e
...
4e80b1c7… · leave_requested · Juniper (8d03f2b6) · effective
a17c3d02… · member_removed · host · effective

$ locust --owner goal status --goal "Static site search"
...
Member: Harbor (51c2e9aa) · local · endpoint 4be07a19 · lead, reviewer

# on a third member's computer that holds the leave and not yet the removal
Member: Juniper (8d03f2b6) · remote · endpoint 9d21c4e8 · left, not yet removed
```

**E2-2. Disconnecting the agent a goal was started with**

Proposed output on the host's computer, for the agent that started "Parser
cleanup". From K1 the command applies at once and shows no plan, because one
command connects the agent again. The first line is the roles plan's and the
last, the `Undo:` line, is K1's. The four lines from `Maple started` on are
this phase's. The second of them is printed only where Maple alone holds a
role, the third only while the goal's shared files are empty, the fourth only
while the goal's page is on. A goal that holds an end gets no lines. After K1
the result carries no warning that the goal freezes.

```text
$ locust --owner agent revoke --agent codex-maple-1a2b3c4d
codex-maple-1a2b3c4d is disconnected. The name stays taken.
Maple started "Parser cleanup" (c01d55aa). The goal keeps running: inviting, removing and rule
  changes need no agent. Maple stays a member and cannot be removed.
Waits until you connect Maple again or give them to another member: the roles only Maple
  holds (lead, reviewer).
Sharing this goal's first files needs Maple connected.
Maple's consent to this goal's public page cannot be given or withdrawn until it is connected
  again.
Undo: locust --owner agent reconnect --agent codex-maple-1a2b3c4d
```

## Phases

### K1: A goal's governance has its own key

**Goal.** Every goal gets a signing key of its own when it is started. That
key signs who is in, the rules, and the steps the host's daemon takes by
itself. It signs nothing else. Across the plans it signs with nobody present
in six situations, and three of them change who is in. They are listed, with
the one rule they keep, under
[What the host's computer signs by itself](#what-the-hosts-computer-signs-by-itself).
It is not a member and it is not one of the
person's agents. The agent the host started the goal with becomes an ordinary
member whose key signs only its own work. So a second record at a used
position of that agent's log costs what any member's fork costs, and
disconnecting that agent no longer stops invitations, removals or rule
changes. Disconnecting an agent applies at once, and one new command connects
it again. For the key itself a person types nothing new and reads no new
word.

**Depends on.** Roles-plan Phases 1, 2 and 3. It is fourth in the one build
order of fifteen phases: R1, R2, R3, K1, R4, R5, R6, G1, G2, E1, E2, R7, R8,
R9, R10, where R is a roles-plan phase. Its change to the Organization model
comes first. The protocol version goes from 6 to 7 in K1, which lands
directly before Phase 4. Phase 4 and E1 change signed bytes inside 7; E1's end
record, at index 26, is the last change of the event format within the
fifteen phases. The first public door changes signed bytes once more, and the
master plan says under which number. Nothing is released before Phase 10, so
no number is raised twice inside the fifteen. It needs nothing from
GUARD or END to land. GUARD's hooks are `Node::hold` and
`Node::admission_hold`; K1 leaves no hook of its own. END's end record is one
more governance kind signed through `Node::host()`. Later phases read this
one: R4 (`State.host` for the role fallback, the only-member part and the
first files; `host_name` after `governance`), G1, E1, E2, R8 (it adds
`ScopeDecided` to `Body::host_may_sign`) and the joinable plan's J1. Replacing
a host is not designed here; one place is left for it (see the notes).

**Changes.** Four terms, used the same way everywhere.
- The *governance key* is a signing key made for one goal when it is started
  and kept on the host's computer. In code it is `governance`. No text a
  person reads names it.
- The *host* is the person whose computer holds that key. `Node::hosts(entry)`
  says this daemon holds it.
- The *host's agent* is the agent the host started the goal with. The first
  record names it. It is a member like any other, with three extra facts: it
  is never removed, other computers reach the host's computer through its
  admission, and it holds a role nobody else holds (Phase 4). Its admission's
  endpoint is the host's computer. In code a key-typed field called `host`
  always means this agent.
- A *host step* is a record that is not governance and that the governance key
  may sign. Today that is a stage's steps (`EffectMaterialized` whose
  configured runner is the governance key). Phase 8 adds the plan's text and
  Phase 9 a landed file change (`ScopeDecided`).

Who signs what after this phase:

| Record | Signed by | Started by |
| --- | --- | --- |
| First record, admissions, removals, rules, tree epochs, task revisions, the publication policy; Phase 4's role holders; END's end record | the governance key | the person's command, except three that the host's daemon signs by itself: an admission on an invitation, which from Phase 4 carries the ticket's role; from E2 the removal of a member whose agent signed a leave; and from J1 an admission through a public door |
| A stage's steps: opening its task, offering it, asking the reviewers' agents for a review of a stage's result | the governance key | the host's daemon by itself |
| The plan's text (Phase 8) and a landed file change (Phase 9) | the governance key | the host's daemon by itself |
| A review request for a result on a task no stage opened | the result's author | that member's daemon by itself, as today |
| Tasks, attempts, results, reviews, checks, document revisions, file proposals, picks and closes by a role holder, page consent, leave requests, delivery and cancel records | a member's agent, the host's agent included | that agent or its person |
| An invitation | the governance key | the person's command |
| A join request | the joining agent | the person's command |

Until Phase 9 the tree's policy still names one member who accepts file
changes, and Phase 4 makes that the host's agent. That acceptance is member
work under the agent's own key. The goal's first files (Phase 4) are a file
proposal signed with the host's agent's key, before and after Phase 9; Phase 9
moves only their acceptance to the governance key. The host's daemon signs no
leave request for the host's agent.

- [Organization.tla](../research/tla/Organization.tla),
  [cases.json](../research/tla/cases.json) and
  [organization.md](../research/tla/organization.md), done first. Record 2 of
  the founding transcript admits identity 0 today. It admits a new identity 5
  instead, the host's agent. Every other id, position and anchor stays, so no
  scenario is renumbered. Two scenarios are added, each with a config under
  `research/tla/configs/` and a row in cases.json. `governance-key-work` (case
  `organization-governance-key-work`): a contribution and a review signed by
  identity 0 are in `view.ordinary` in no reachable state. `host-agent-fork`
  (case `organization-host-agent-fork`): two records at position 0 of identity
  5's log and a later admission by identity 0; every held governance record
  stays in `view.governance`. organization.md says identity 0 governs and is
  not a member, and identity 5 is the host's agent. This is the first of four
  changes under `research/tla`: the role holders of roles-plan Phase 4 come
  next, then G1's model, then E1's cases. All of them keep this transcript's
  identities.
- [lib.rs](../crates/locust-proto/src/lib.rs): `PROTOCOL_VERSION` 6 to 7, with
  `versions.protocol` in [site.json](site.json). It is not raised again.
  `API_VERSION` and the store marker stay 7 from Phase 1; this phase's new
  fields and its record `K` land inside them.
- [event.rs](../crates/locust-proto/src/event.rs):
  - `Genesis { governance: PublicKey, host: PublicKey, definition, salt }`.
    Today it is `{ administrator, definition, salt }`.
  - `Genesis::goal_id` hashes `governance`, `host`, `definition` and `salt`,
    in that order, under `domain::GOAL_ID`. So no second first record can name
    another host's agent under the same identifier.
  - `Header::check`, `Body::Genesis` arm: the author must equal
    `genesis.governance`, as it must equal `administrator` today, and new:
    `genesis.host != genesis.governance`. Either failure is
    `EventError::BadAnchor`.
  - New `Body::host_may_sign(&self) -> bool`: `is_governance()` or
    `EffectMaterialized`. It is the one list of kinds the governance key
    signs. Phase 4's `RoleHolders` and E1's `GoalEnded` join `is_governance()`
    and so this list; Phase 8 adds `ScopeDecided`.
  - The module comment and the comment on `GoalId` in
    [id.rs](../crates/locust-proto/src/id.rs) say governance key.
- [invite.rs](../crates/locust-proto/src/invite.rs): no field changes.
  `Invitation.governance` (Phase 1's name for today's `administrator`, see the
  list of changes to that plan) now holds the governance key, and
  `Invitation::signed` and `sign` take that keypair. An invitation commits to
  the goal identifier and the governance key and is signed by that key. The
  joiner's daemon stores the key and refuses a first record that names
  another, as `receive` in
  [replica.rs](../crates/locust-core/src/node/replica.rs) does today.
- [api.rs](../crates/locust-proto/src/api.rs) and
  [api/context.rs](../crates/locust-proto/src/api/context.rs): `GoalStatus`
  gains `governance: PublicKey`, filled from the first record or, before it
  arrives, from the ticket, and `hosted_here: bool`, which is
  `Node::hosts(entry)`. `GoalStatus.host` and `ContextBrief.host` become
  `Option<PublicKey>`: the host's agent, absent until the first record is
  held. `EventView` and `TaskView` gain `by_host: bool`, true when the
  record's author or the task's creator is the governance key. In
  `crates/locust-proto/src/api/level.rs` (new in Phase 3) `Abilities.host`
  becomes `Option<PublicKey>` for the same reason; `Why::Rules.host` stays a
  key, because a rules refusal exists only once the first record is held. One
  request is added: `Request::AgentReconnect { agent }` (`agent.reconnect`,
  audience `Owner`, not a tool, answered `Done`; daemon.rs below). The
  contract lists one more operation than the tree this phase starts from and
  the same number of tools.
- [testkit.rs](../crates/locust-proto/src/testkit.rs): `Author::genesis_with`
  and `Author::found_goal` take the host's agent's key and admit that key, not
  the signing key. The constants in
  [vectors.rs](../crates/locust-proto/src/vectors.rs) are regenerated.
- [history.rs](../crates/locust-core/src/goal/history.rs): `History` gains
  `host: Option<PublicKey>` beside `governance`; `insert` fills both from the
  first record. [state.rs](../crates/locust-core/src/goal/state.rs): `State`
  gains `host`.
- [chain.rs](../crates/locust-core/src/goal/chain.rs):
  - `Chain::build` sets `state.host` from `history.host`. A `MemberAdmitted`
    whose `member` is the governance key is `Excluded(Precondition("the goal's
    signing key is not a member"))`. A `MemberRemoved` whose `member` is the
    host's agent is `Excluded(Precondition("the host's agent is not
    removed"))`. Both still take a position and a snapshot, as every refused
    governance record does. Nothing else in the loop changes: the chain is
    still the governance records in the usable prefix of one key's log, and a
    second record at a used position of that log is still `Halt::Fork`.
  - `authorize_base`, for a record that is not governance and whose author is
    `state.governance`: `Excluded(NotAMember)` unless `body.host_may_sign()`.
    When it may be signed, the `tenure_at` test and the cutoff test are
    skipped; the anchor must be held, the payload epoch must match, and the
    walk back along the author's log runs as for any author. For every other
    author the code is as today. Every event passes this function before any
    rule is read (`Verifier::check` in
    [fold.rs](../crates/locust-core/src/goal/fold.rs)), so it is the one place
    replay learns that a signer is not a member. Once E1 has landed, the order
    in `authorize_base` for a record that is not governance is: E1's two
    `AfterEnd` tests, then K1's branch for the governance key, then today's
    tenure path. `authorize` beside it needs no change: `tenure_at` answers
    nothing for this key, so only the usable prefix or a pin admits its
    records.
  - The module comment says one governance stream.
- [goal/flow.rs](../crates/locust-core/src/goal/flow.rs): `stage_template` and
  `review_templates` take the runner from `self.chain.state.governance` in
  place of `self.history.administrator`. No rule changes: `validate_effect`
  already refuses a step whose signer is not its configured runner.
- [validation.rs](../crates/locust-core/src/organization/validation.rs): a
  stage's rules may not name `task_creator` where only a member can act. The
  creator of a stage's task is the governance key, which is no member, so
  such a rule could be met by nobody. In the stage loop of `Validator::run`,
  the check reads the work rules and the decision rules the stage's task gets:
  its task type's, or the formation's where the type sets none. It reports
  `selector_scope` for `task_creator` in the `by` of an `independent` start,
  in the `to` of an `offered` start and in any completion criterion, alone or
  inside an `any`. The diagnostic names the stage, says that a stage's task
  is opened by the host's computer, and tells the author to name `members`, a
  role or a participant. The `by` of an `offered` start may still name
  `task_creator`: the host's computer makes those offers by itself. Replay
  runs the same validation on every rules binding (`valid_definition` in
  [goal/mod.rs](../crates/locust-core/src/goal/mod.rs), called from
  `validate_binding` in chain.rs), so the check lands with this phase's
  protocol step, and a binding it refuses is `InvalidDefinition` on every
  computer. It is mirrored in
  [rules.ts](../sites/locust.farm/src/lib/formation-editor/contract/rules.ts),
  with one more case in
  [organization.cases.json](reference/conformance/organization.cases.json)
  and one clause in the selector table of [formations.md](formations.md). No
  preset names `task_creator`, so none changes. One limit stays, read and not
  run. A subtask opened under a stage's task inherits the stage's rules with
  `task_creator` fixed to the governance key (`inherit` in
  [delegation.rs](../crates/locust-core/src/goal/delegation.rs)), and the
  host's computer makes offers only for the stage's own task. So a start
  `offered by: task_creator` gives no offer in a subtask, and a stage that
  lets members open subtasks needs another start rule for them. The check
  does not test that.
- [node/local.rs](../crates/locust-core/src/node/local.rs): new record tag `K`
  in `Space::Goal`, keyed by the goal, holding the 32-byte seed. New
  `governance_write(goal, seed)`. `Local` gains `governance: Option<Keypair>`,
  built in `absorb`; `Keypair`'s `Debug` prints the public key only. The key
  lives in the daemon's store inside the data directory, beside the agents'
  seeds (`Space::Agent`), the content keys (`Space::Key`), the invitation
  records (`Space::Invite`) and the page's upload seed. No command shows or
  exports it.
- [authoring.rs](../crates/locust-core/src/node/authoring.rs): new
  `Node::key_for(&self, entry, author) -> Result<&Keypair, ApiError>`: the
  goal's governance key when `author` is its public key and this daemon holds
  it, else `signer(author)` as today. `author` uses it. `next_place` skips the
  member test and the `Local.part` test for the governance key and keeps both
  for every agent. The order three pieces build there, once G1, G2 and E1 have
  landed: `next_place(entry, author, body)`, in order: the copy holds an end,
  `conflict(ENDED)` (E1); member and `Local.part`, skipped for the governance
  key (K1); `hold` (G1); `Goal::next`. In K1 itself the function keeps today's
  two arguments; `body` is G2's. Every signature with the governance key
  passes `next_place`, with one exception: the three records `goal_create`
  signs through `sign_at` at positions 0, 1 and 2 of a new log, as today. New
  `Node::author_alone(entry, author, body, tx)` is `author` with no text and
  clock field 0. It is the one way a record is signed with nobody present:
  `plan_join` and `drive_flow` sign through it and nothing else calls it. So
  such a record carries no clock reading and passes the gate (the rule under
  "What the host's computer signs by itself"). The module comment no longer
  says that only a current member signs.
- [access.rs](../crates/locust-core/src/node/access.rs):
  - `Node::hosts(entry)`: `entry.local.governance` is held and its public key
    equals `entry.state().governance`. Phase 1 asked `Principals::holds`.
  - `Node::host(actor, goal)`: `readable`; the goal has a governance key, else
    `not_found`; `hosts(entry)`, else Phase 1's first sentence. It returns the
    governance key. Phase 1's second test and its sentence `the host agent is
    disconnected; nothing can sign for this goal` are deleted.
- [requests/goals.rs](../crates/locust-core/src/node/requests/goals.rs):
  `goal_create` draws a seed with `self.random()`, builds the keypair, signs
  the first record, the admission of the named agent and the first rules
  binding with it at positions 0, 1 and 2 of its log, and writes
  `governance_write` in the same commit. Today the creator's own key signs all
  three and admits itself. `goal_leave` and `member_remove` keep Phase 1's two
  refusals for the host's agent, compared with `state().host`. `member_remove`
  takes its keypair from `key_for`; today it calls `signer` itself.
  `rules_bind` and every other host request sign with the key `host()`
  returns. `goal_status` fills `governance`, `host` and `hosted_here`.
- [requests/invitations.rs](../crates/locust-core/src/node/requests/invitations.rs):
  `goal_invite` reads the title as the owner (`self.title(entry, None)`);
  today it reads as the administrator, which only a member can
  (`may_read_epoch`). It signs the invitation through `key_for` (two direct
  `signer` calls today) and stores the key in `InviteRecord.governance`. In
  `goal_join` the check that the ticket's endpoint equals "the held
  administrator admission" compares with the admission of the host's agent
  (`state().host`); today it looks the administrator key up among the members.
  That check, the check of the ticket's key beside it, and the answer `joined`
  that follows for an agent this copy lists as a member all read the copy this
  computer holds. That copy can be old: a removed computer that never received
  its removal answers `joined` to a fresh ticket and the host admits nobody
  (measured,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 1:
  `offline_removed_member_retries_refused_peers_and_fresh_ticket_returns_stale_membership`).
  K1 does not change that answer.
- [peers.rs](../crates/locust-core/src/node/peers.rs): `plan_join` tests that
  this daemon hosts the goal, that the invitation names the goal's governance
  key, and that the joiner's key is not that key. Phase 1's tests of the host
  agent (active, has not left) are deleted. It signs the admission with the
  governance key. `Host::join` calls `plan_join` as today. The order of
  today's checks is kept. A repeated join on an invitation the same member
  already redeemed at the same endpoint is answered from the invitation record
  and the member list, before these three tests and before anything is signed
  (the `invite.redeemed` branch;
  `redeemed_invitation_is_not_revoked_and_retries_recover_after_expiry` in
  [invitations.rs](../crates/locust-core/src/node/tests/invitations.rs) pins
  it and stays). G1 puts `Node::admission_hold` after that answer, at the one
  place `plan_join` is about to sign. It signs through `author_alone`, with
  no clock reading, where today it passes the caller's clock (read in
  `plan_join`). The same request from the same records is then the same
  record. The time of joining stays on the host's computer in the ticket's
  record (`redeemed_ms`); the only reader of the record's own clock field is
  the event view.
- [node/flow.rs](../crates/locust-core/src/node/flow.rs): `drive_flow` signs a
  desired step when its runner can sign here. For the governance key that is
  `hosts(entry)`, `desired.runner == state.governance` and
  `entry.goal.next(&runner)` answering a place. For a member's agent the tests
  are Phase 3's: active, has not left, a member, can sign next. Phase 3's
  `Node::stalled` (in `requests/levels.rs`) follows: `RunnerRevoked`,
  `RunnerLeft` and `RunnerNotMember` never apply to the governance key, and
  `Node::abilities` there fills `host` and `hosted_here`. In this phase the
  key's one stall is `Halted`; Phase 8 adds `RunnerElsewhere` and G2 adds
  `CatchingUp`.
- [farm.rs](../crates/locust-core/src/node/farm.rs): `eligible` leaves the
  governance key out of the authors whose consent it requires. Today every
  author of an effective record outside six kinds (first record, publication
  policy, consent, admission, removal, rules) must consent and must be in the
  member map, which would stop publication in any goal with a stage step, a
  tree epoch or a task revision. The key is left out where the set of required
  authors is built, not where consents are judged. `eligible` reads the
  highest consent it holds for each required author and stops publication when
  that one is pending or excluded, even beside an older acceptance (measured,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 3:
  `excluded_consent_arriving_after_removal_suspends_even_when_it_accepts`). A
  consent signed by the governance key is excluded by replay, and because the
  key is never a required author that record is never read and cannot suspend
  the page. `project` keeps records signed by the governance key in the page's
  list of changes, with no agent beside them (`FarmChange.agent` is already
  optional); today the list keeps only records by consenting agents.
  `farm_request` signs the publication policy with the key `host()` returns,
  through `author`. The page's address is unchanged: it is the hash of an
  upload key drawn for that publication, and the publication policy, signed by
  the governance key, ties it to the goal.
- [views.rs](../crates/locust-core/src/node/views.rs),
  [context_views.rs](../crates/locust-core/src/node/context_views.rs) and
  [sim/check.rs](../crates/locust-core/src/node/sim/check.rs): the event and
  task views fill `by_host`; the compact context fills `host` as an option;
  the simulator compares `status.host` with the creating agent.
- [presentation.rs](../crates/locust/src/cli/presentation.rs): where a view
  carries `by_host`, the author or creator prints as `host`, with no key. A
  stalled step (Phase 3) whose runner equals `GoalStatus.governance` prints
  `host` the same way. The `Host:` line of `goal status` prints `Host: you`
  when `hosted_here`; elsewhere it prints the host's agent as `label` does, or
  `Host: on another computer` before the first record is held. No rendered
  text prints the key or the words governance key.
- [cli/invitations.rs](../crates/locust/src/cli/invitations.rs) and
  [cli/mod.rs](../crates/locust/src/cli/mod.rs): the ticket review and the
  join result print no host key; the goal's identifier, which commits to the
  key, is already printed. In cli/mod.rs the usage error for `--agent` on a
  host command (Phase 2) reads `host commands are the host's own and name no
  agent; drop --agent`.
- [cli/workspace.rs](../crates/locust/src/cli/workspace.rs): `workspace init`
  reads `GoalStatus.host` as an option; absent, it answers Phase 1's first
  sentence of `host()`. On the computer that hosts the goal (`hosted_here`),
  it refuses while the host's agent is disconnected, before its plan and
  before it sends `rules.bind` or `workspace.epoch`: the agent
  `GoalStatus.host` names is not among the active agents of `status`, the read
  `local_members::run` makes today. The reason is that it captures on that
  agent's behalf (Phase 1), only that agent's change counts as first files
  (Phase 4), and an owner's request on behalf of a revoked agent is
  `not_found` (`resolve` in
  [callers.rs](../crates/locust-core/src/node/callers.rs)). The refusal is
  `conflict`: `AGENT is disconnected; only AGENT can share this goal's first
  files`, with a second line, `Connect it again: locust --owner agent
  reconnect --agent AGENT`. Until this phase `host()` refused such a run at
  `rules.bind` or `workspace.epoch`; without the new test those two would now
  succeed and start an epoch that can get no first files.
- `crates/locust/src/cli/only_you.rs` (from Phase 2): `goal add` refuses
  before its plan when `hosted_here` is false. The test it took over from
  [local_members.rs](../crates/locust/src/cli/local_members.rs), that the
  goal's signer is an active local agent and a local member, is deleted; its
  retry key and its ticket comparison read `governance`. `member remove`
  refuses before its plan when the member it names is `GoalStatus.host`, with
  Phase 1's sentence. Until this phase it showed its plan, asked the person
  typing it for a yes and was then refused by the daemon; the daemon's
  refusal stays. `agent revoke` loses the warning `Goals NAME hosts freeze
  for everyone` and, with it, its plan and its confirmation. From this phase
  it applies at once, like `level` and `allow`: it takes neither `--plan`
  nor `--confirm`, prints `NAME is disconnected. The name stays taken.` and,
  last, `Undo: locust --owner agent reconnect --agent NAME`. Phase 2 had it
  ask the person typing it for a yes because no command undid a revoke; one
  now does, so by Phase 2's own rule for the two tiers it applies at once. It
  still reads the goals in which this agent is the host's agent, and what it
  prints for such a goal is E2's. New command `locust --owner agent reconnect
  --agent NAME`: it sends `agent.reconnect`, applies at once and prints `NAME
  is connected again.` and `Undo: locust --owner agent revoke --agent NAME`.
  For an agent that is not disconnected it prints `NAME is not disconnected.
  Nothing changed.` and exits 0.
- [requests/daemon.rs](../crates/locust-core/src/node/requests/daemon.rs):
  new `agent_reconnect`, the handler of `agent.reconnect`. It clears the flag
  that `agent_revoke` set on the stored record and changes nothing else. A
  revoke deletes nothing: the record keeps the name, the signing seed and the
  credential digest (`PrincipalRecord` in
  [identity.rs](../crates/locust-core/src/node/identity.rs)), so the agent
  comes back under the same key and the credential its client stored works
  again. No goal record is signed and no other computer learns of it. Like
  `agent_revoke` it marks every goal the agent is a member of as changed, so
  a step that waited for the agent is taken. An agent that is not
  disconnected answers `Done` and writes nothing; an unknown key is
  `not_found`. `agent_revoke` itself is unchanged.
- Recipes and scripts. This phase rewrites, in the same change, every marked
  recipe under `docs/guide` and every harness under `scripts/` that calls
  something it changes, and its exit criteria run both. What they can meet:
  `agent revoke` run with `--plan` and `--confirm`; `host` in `goal status
  --json`, which is null until the first record is held; and a hand-written
  formation in a fixture whose stage rules name `task_creator`. A search at
  `d3ad6b7` finds no recipe and no script that calls `agent revoke`.
- Unchanged after Phase 1's rename, and checked: `screen` in
  [screen.rs](../crates/locust-core/src/goal/screen.rs) keeps records by the
  governance key and by keys it admitted; `Goal::next` in
  [goal/mod.rs](../crates/locust-core/src/goal/mod.rs) refuses the governance
  key at a halt and any key whose own log has a fork or a gap, and tests no
  membership; `historical_endpoints`, `receive_halt_proof`, `peers` and
  `speaks_for_member` in peers.rs; `resolve` and `task_creator(history, task,
  round)` in [rules.rs](../crates/locust-core/src/goal/rules.rs), read again
  after `c5201dd`: the function follows the round back to the opening record
  and names its signer, so a stage task's creator is the signer of its opening
  step; [delivery.rs](../crates/locust-core/src/node/delivery.rs), where
  deliveries are per recipient member; the sync code, which filters no author
  by membership; `offer_key` and `wanted_keys` in replica.rs, where the proof
  of a content key is the payload of the first record or of the removal that
  began its epoch, whoever signed it. So a survivor that holds a removal and
  not its new key still cannot write text until the key arrives, and the
  host's computer, which made the key, can (measured,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 9:
  `undelivered_removal_key_blocks_survivor_text_but_host_can_write_and_survivor_can_leave`).
  Delivering that key is the sync code's work and needs no agent.
- `python3 scripts/check_formations.py --write` regenerates the contract files
  under `docs/reference/generated/`.

**Tests.**
- New in event.rs:
  `a_first_record_is_signed_by_the_governance_key_and_names_another_key_as_the_hosts_agent`
  (a first record whose author is not `governance`, or whose `host` equals
  `governance`, is refused) and `the_goal_identifier_commits_to_both_keys`
  (changing either key changes the identifier).
- New in [goal/tests.rs](../crates/locust-core/src/goal/tests.rs), each a
  signed replay run forward, reversed and reloaded:
  - `the_governance_key_is_never_a_member_and_its_ordinary_work_is_excluded`:
    its own admission is excluded, and a task, a result, a review and a pick
    signed by it are `NotAMember`.
  - `a_stage_step_signed_by_the_governance_key_is_effective_and_one_by_the_hosts_agent_is_not`.
  - `removing_the_hosts_agent_is_excluded_and_it_stays_a_member`.
  - `a_fork_in_the_governance_log_retracts_later_governance_and_preserves_prefix_work`:
    once with two different admissions at one position and once with a stage
    step against an admission; the assertions of today's
    `host_review_fork_retracts_later_governance_but_preserves_prefix_work`.
  - `a_stage_task_names_the_governance_key_as_its_creator`: an offer rule `by:
    task_creator` or `by: members` yields offers signed by the governance key,
    and one `by` a role yields none. It binds no rule that lets `task_creator`
    act in a stage task, because such rules are refused (the next two tests).
  - `rules_whose_stage_task_names_the_task_creator_are_excluded_in_replay`: a
    rules binding whose formation the new check refuses is
    `InvalidDefinition`, and no stage task opens under it.
- New in
  [organization/tests.rs](../crates/locust-core/src/organization/tests.rs):
  `a_stage_rule_that_names_the_task_creator_where_a_member_must_act_is_refused`.
  `selector_scope` is reported for an independent start `by: task_creator`,
  for an offered start `to: task_creator`, and for a declaration, a review, a
  check and a posted result `by: task_creator`, alone and inside an `any`,
  whether the rules are the stage's task type's or the formation's own. A
  stage whose start is `offered by: task_creator` passes, and so does every
  preset. The new case in organization.cases.json and the site's conformance
  test assert the same diagnostic.
- Rewritten, because the measured behaviour changes:
  - `host_review_fork_retracts_later_governance_but_preserves_prefix_work`
    becomes
    `a_review_fork_by_the_hosts_agent_costs_what_a_members_fork_costs`: the
    later admissions stay effective, there is no halt, the governance key can
    sign next, the agent cannot, and its records from the forked position on
    are pending.
  - `retracted_anchor_keeps_same_goal_author_descendants_pending_even_at_surviving_head`
    keeps its name and assertions; the fork is made between two governance
    records.
  - In [workspace_tests.rs](../crates/locust-core/src/goal/workspace_tests.rs)
    the two `host_integrator_acceptances_*` tests become
    `acceptances_by_the_hosts_agent_at_one_log_position_dispute_only_the_files`
    and
    `acceptances_by_the_hosts_agent_at_distinct_log_positions_dispute_only_the_files`:
    in both the later admission stays effective.
  - In [delivery.rs](../crates/locust-core/src/node/tests/delivery.rs):
    `restored_host_signing_before_peer_recovery_forks_but_recovery_first_extends`
    becomes `a_restored_hosts_agent_forks_only_its_own_log`, and the restored
    daemon still admits a joiner without a fork.
    `restored_host_redeems_outstanding_invitation_before_recovery_at_an_already_used_position`
    loses an admission, not a result, before the restore, and still halts.
    `restored_host_automatic_stage_replay_is_identical_unless_another_event_used_its_position`
    uses an admission as the other record.
    `restored_host_known_gap_blocks_signing_until_missing_predecessor_arrives`
    puts the gap in the governance log.
  - In [durable_tests.rs](../crates/locust/src/daemon/durable_tests.rs):
    `sqlite_older_directory_signs_at_used_host_position_unless_later_events_are_recovered_first`
    becomes two.
    `sqlite_older_directory_forks_only_the_hosts_agent_when_its_own_work_was_lost`:
    status reports no halt, `goal invite` succeeds and the agent's next
    request is `unavailable`.
    `sqlite_older_directory_halts_governance_when_a_governance_record_was_lost`:
    status reports the halt.
  - In
    [lifecycle_characterization.rs](../crates/locust-core/src/node/tests/lifecycle_characterization.rs)
    (added in `c9c3b2c`, after Phase 1 was written):
    `revoked_host_cannot_resume_governance_through_grants_or_enrollment_but_old_store_can`,
    in whatever form Phase 1 leaves it, becomes
    `revoking_the_hosts_agent_stops_that_agent_and_not_governance`: after the
    revoke and after a restart, `member.remove` and `goal.invite` succeed, the
    agent's own requests fail, and its name cannot be enrolled again. It keeps
    the other two facts the old test measured: the revoke adds no goal record
    and no halt, and a copy of the store taken before the revoke still has the
    agent connected. It then sends `agent.reconnect`: the agent's next request
    succeeds under the same key, and still no goal record was added.
  - These restore tests state what the code does between this phase and G1. G1
    starts from these names and rewrites what they expect.
  - Phase 1's tests:
    `host_operations_need_no_grant_and_sign_as_the_host_agent` becomes
    `host_operations_need_no_grant_and_sign_with_the_governance_key`;
    `a_disconnected_host_agent_signs_nothing_but_its_invitations_are_listed_and_revoked`
    becomes `disconnecting_the_hosts_agent_stops_no_host_command`;
    `admission_stops_when_the_host_agent_is_revoked` becomes
    `admission_continues_when_the_hosts_agent_is_revoked`;
    `goal_create_is_the_owners_act_and_names_the_host_agent` also asserts that
    the first three records are signed by a key that is no agent and no
    member; `host_operations_refuse_a_daemon_that_does_not_hold_the_host_key`
    keeps its name and also reads `hosted_here` false on the member's daemon
    and true on the host's.
  - Phase 3's `goal_status_reports_stalled_effects` stalls a review request
    whose author's agent is disconnected, and a stage step only by a halt.
- New in the node's tests:
  - `the_governance_key_is_stored_with_the_goal_and_signs_after_a_restart`
    ([lifecycle.rs](../crates/locust-core/src/node/tests/lifecycle.rs)).
  - `an_admission_signed_twice_for_one_request_is_one_record`
    ([invitations.rs](../crates/locust-core/src/node/tests/invitations.rs)):
    the same join request admitted from two copies of one store gives one
    identifier, and a request by another joiner gives another.
  - `no_credential_and_no_on_behalf_reaches_the_governance_key`
    ([authorization.rs](../crates/locust-core/src/node/tests/authorization.rs)):
    an agent credential is `denied` on every host request, and an agent
    operation sent `on_behalf` of the governance key is `not_found`.
  - `the_hosts_agent_cannot_leave_or_be_removed_and_can_be_disconnected`
    (authorization.rs).
  - `a_disconnected_agent_is_connected_again_with_its_name_and_key`
    (authorization.rs): after `agent.revoke` and `agent.reconnect`, also
    across a restart between them, the agent's stored credential works, its
    key is the same, a step that waited for it is signed, and neither request
    added a goal record. `agent.reconnect` is `denied` to an agent's
    credential, `not_found` for an unknown key, and `Done` with no write for
    an agent that is connected.
  - `a_ticket_names_the_governance_key_and_the_first_record_must_agree` and
    `a_tickets_endpoint_is_checked_against_the_hosts_agents_admission`
    ([invitations.rs](../crates/locust-core/src/node/tests/invitations.rs)).
  - `stage_steps_are_signed_while_the_hosts_agent_is_disconnected`
    ([organizations.rs](../crates/locust-core/tests/organizations.rs)).
  - `publication_needs_no_consent_from_the_governance_key` and
    `the_page_keeps_changes_signed_by_the_governance_key`
    ([farm.rs](../crates/locust-core/src/node/tests/farm.rs)). The first also
    holds a consent signed by the governance key, which is excluded, and the
    page stays eligible. E1 adds an end record to it.
- New in presentation.rs: `a_record_signed_by_the_governance_key_prints_host`
  (an event, a task and a stalled step) and
  `rendered_text_never_names_the_governance_key`, which renders every response
  fixture as text and finds neither the key nor the words governance key.
- New in [cli.rs](../crates/locust/tests/cli.rs), against its stub server:
  `goal_add_needs_this_computer_to_host_the_goal`: with `hosted_here` true and
  the host's agent revoked the plan is shown; with `hosted_here` false the
  command refuses before any plan and sends no write. New in
  [tests/workspace.rs](../crates/locust/tests/workspace.rs):
  `init_refuses_while_the_hosts_agent_is_disconnected`: the command exits 7
  before any plan and sends neither `rules.bind` nor `workspace.epoch`; its
  second line names `agent reconnect`, and after that command the plan is
  shown. Also new in cli.rs:
  `agent_revoke_applies_at_once_and_prints_the_command_that_undoes_it` (no
  plan, no confirmation, one `agent.revoke`, the `Undo:` line last, and that
  line run as printed sends one `agent.reconnect`) and
  `member_remove_naming_the_hosts_agent_refuses_before_any_plan` (exit 7, no
  plan and no write).
- Adapted with no change of assertion: every fixture that founds a goal signs
  governance with one key and the host's own work with another (`Fixture` in
  goal/tests.rs, `Author::found_goal`, the sync, store and replica fixtures,
  the two `same_key_rejoin_*` tests,
  `a_diverged_author_log_reconciles_between_two_real_daemons`,
  `decision_successors_keep_author_scope_purpose_and_predecessor_separate`,
  and the other tests of `c9c3b2c` in
  `node/sim/lifecycle_characterization.rs`,
  `node/tests/farm_characterization.rs` and
  `node/tests/delivery_characterization.rs`, where the host's own work is
  signed by the host's agent and its admissions, removals and policy by the
  governance key). A node test that forges a governance record takes the
  keypair from `Local.governance`; it took it from `signer` until now.
  `signed_current_protocol_vectors_are_frozen` and
  `body_indices_and_bytes_are_current_contract` are regenerated.
  `incompatible_event_protocol_refuses_open_before_collecting_or_rewriting_state`
  in the store's tests and `another_protocol_version_is_refused` in the sync
  tests are unchanged and are the check of the last exit criterion.

**Exit criteria.**
- The three cargo commands of `AGENTS.md`, the four npm commands of
  `AGENTS.md` in `sites/locust.farm`, `python3 scripts/check_formations.py`,
  `python3 scripts/check_docs.py` and `python3 scripts/check_tla.py --suite
  organization` pass.
- `python3 scripts/check_documentation.py --binary target/debug/locust
  --timeout 60` and `python3 -m unittest discover -s scripts/tests` pass.
  They are the two CI steps that run the guide's recipes and the script
  harnesses.
- `locust --json contract` reports protocol 7. `GoalStatus` has `governance`,
  `hosted_here` and an optional `host`.
- On a fresh home with maple and juniper enrolled, after `goal create --agent
  maple`: `goal status --json` gives a `governance` key that is none of
  `members[].member` and none of the agents in `status --json`, and
  `hosted_here` true; `events` lists the first record, the admission and the
  rules binding with `host` as their author. `goal leave --agent maple` exits
  7 with `conflict`.
- Then `agent revoke --agent maple`, which shows no plan and prints its
  `Undo:` line last. `goal invite`, `goal add --agent juniper` and `rules
  bind` still succeed. `member remove --member maple` and `workspace init
  --empty` each exit 7 with `conflict` before any plan, and neither adds a
  record to `events`.
- Then the `Undo:` line as printed. `workspace init --empty` now shows its
  plan, maple's next post succeeds under the key it had, and `events` shows
  no record for the revoke or for the reconnect.
- `rules bind` with a formation whose stage's task lets `task_creator`
  declare is refused with the diagnostic `selector_scope`, and `events` shows
  no new record.
- In a `pipeline` goal whose host's agent is disconnected, the first stage's
  task opens and `events` shows its `effect_materialized` by `host`.
- `git grep -n 'host agent is disconnected' -- crates` and `git grep -n
  'freeze for everyone' -- crates` find nothing. `git grep -n 'author_alone('
  -- crates/locust-core/src/node` finds the definition and calls in peers.rs
  and node/flow.rs only.
- A store or a peer from before this phase is refused as unsupported.

**Risks and notes.**
- A copy of the data directory holds the governance key of every goal that
  computer hosts, the seed of every agent, every content key, the invitation
  records and the page's upload seed. A daemon started on a copy is a second
  host. Two hosts that both sign halt governance for good. Nothing in this
  phase prevents that; the restore guard is GUARD's. The operations guide must
  say what a copy holds; the Backups section is G2's.
- A copy older than a goal holds neither the goal nor its key. Put back, it
  loses that goal's host seat for good: nobody joins or is removed, the rules
  never change and the goal cannot be ended. No later release changes that
  for a goal made under v2: the one in which a host can be replaced does not
  read these goals (answer 2).
- A fork in the governance log is judged as the administrator's fork is today:
  the chain is cut at that position, later governance is excluded, and the key
  signs nothing more. It is permanent. v2 has no takeover, and the release
  that brings one does not read goals made under v2. What changes is what
  can cause it. Only governance records and host steps sit in that log, so a
  review, a result or a task can no longer halt a goal.
- A host step forked at one position halts membership and rules too, because
  both live in one log. The daemon signs in this log with nobody present from
  K1 on: every admission on an invitation and every stage step. Phases 8 and 9
  add every plan change and landed file change, E2 the removal that follows a
  leave, and J1 every admission through a public door. GUARD must exist before
  Phases 8 and 9 and before the first door, and nothing is released between K1
  and G1. One key and one log are kept in v2. The reasons, the cost and the
  second key that was not taken are under "What the host's computer signs by
  itself".
- A fork in the host's agent's log costs what a member's fork costs: its
  records from that position on wait, what a pick or a recording pinned stays,
  and the agent signs nothing more in that goal. One thing is specific to it:
  it cannot be removed, and Phase 4 gives it any role nobody else holds, so
  the host gives those roles to another member with `role give`. It can then
  share no first files either. Connecting it again does not help, because the
  conflict is in its records, and no later release helps a goal made under
  v2. What the person is told is G1's
  `Halt::SignerConflict` and G2's line for it.
- The host's agent cannot leave or be removed. Sync runs only between
  endpoints bound to current members (`peers` and `speaks_for_member` in
  peers.rs), so its admission is how members reach the host's computer. Replay
  keeps it once admitted. "Cannot leave" is the host daemon's refusal to sign
  the request, not a replay rule, and it needs none: a leave request removes
  nobody. Measured on today's code, a leave request shows in the event list
  and changes no member list and no status line, and the leaver stays a member
  until a removal
  (`leave_is_visible_in_events_and_replication_and_rotated_keys_continue_until_removal`,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 7).
  The removal is what replay excludes. From E2 the host's computer answers a
  leave with a removal by itself, never for the host's agent, and a leave
  signed with that agent's key is invalid in replay. That it is admitted
  first is the
  daemon's act, in the same commit as the first record, not a replay rule.
  Lifting this needs the host's endpoint in a signed record; it belongs with
  replacing a host, which names a new host's agent. A goal made under v2
  never gets it, because that release does not read these goals.
- New limit for Phase 4's rule that a goal's only member needs no approval.
  That rule is judged at the result's anchor, and an author's anchors never go
  backward along its own log. Until this phase the host's agent's log held
  every governance record, which kept it near the head. From this phase its
  log holds only its own work. A host's agent that has signed nothing since
  the second member joined can still anchor where it was alone, and that
  result counts when posted. An honest, current daemon anchors at its head. So
  this needs a modified daemon, or a host restored from a copy older than the
  second admission. A hostile host is not assumed, and the host could bind
  `open` openly. G1 closes the restored case while the marks are kept. What is
  left: a modified daemon, or a person who continues from an old copy.
- A stage's task has the governance key as its creator. `task_creator` in a
  stage task's rules therefore names a key that signs only that stage's steps.
  An offer rule `by: task_creator` or `by: members` still gives automatic
  offers (`rules::matches` answers true for `members` without a membership
  test). One `by` a role gives none, because the key holds no role; the agent
  that holds the role signs the offer itself, with no person asked when its
  level is auto. A rule that lets `task_creator` start a stage task, be
  offered it, or declare, review, attest or post in it could be met by
  nobody. Such rules are refused when the rules are checked (validation.rs
  above), at binding and in replay, so a formation never opens a stage task
  that cannot be finished. No preset does any of this. What the check
  leaves: a subtask under a stage task inherits `offered by: task_creator`
  as an offer that only the governance key could make, and the host's
  computer makes none there. The agent that was offered work can still sign
  a request to cancel its own attempt, and it acknowledges that request
  itself. The offerer of an automatic offer is the key, which signs no such
  request. No person is asked in either case.
- A formation cannot name the governance key as an authority:
  `validate_binding` accepts only admitted members there. The key holds no
  role, so it never picks a result or closes a scope. Measured today, a close
  on the goal scope follows the finish role and not the host
  (`goal_close_follows_finish_role_instead_of_host_identity`), and Phase 4
  gives a role nobody holds to the host's agent, never to the key. When Phase
  8 lets the key sign `ScopeDecided`, such a record for anything but the
  plan's text or a landed file change is refused by the rule (`decision` in
  fold.rs: the signer is not the named authority).
- Disconnecting the host's agent stops no host command. It is kept on this
  computer only and it lasts until the agent is connected again: the revoke
  adds no goal record and no halt, and other members' computers still list
  the agent as a member. On today's code no command brings it back (measured,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 5:
  `revoked_host_cannot_resume_governance_through_grants_or_enrollment_but_old_store_can`).
  That is a choice and not a fact about the data: a revoke keeps the agent's
  seed and only sets a flag, and this phase adds the request that clears it.
  What waits while the agent is disconnected is what only that agent's key
  can sign: its own work; the roles only it holds, until the host gives them
  to another member; the goal's first files, which only it can share (Phase
  4); and its consent to a public page. The last is read in farm.rs and not
  measured for a revoked agent: `eligible` requires every current member's
  consent and `farm_request` needs the agent's key to sign one. The nearest
  measurement is of an agent that left, whose consent can no longer be
  withdrawn
  (`departed_member_cannot_sign_consent_withdrawal_even_for_owner_after_restart`,
  claim 2). So while a host's agent is disconnected, a page it had not
  consented to is not published and a consent it gave cannot be withdrawn.
  Connecting the agent again ends each of the four, so none of them lasts
  longer than the person wants. What `agent revoke` prints for such a goal is
  E2's, and it names the roles, the first files and the page.
- A host who works alone and changes agents. Read from the plans, not
  measured. After `agent revoke --agent maple` and `goal add --agent juniper`
  the goal's record holds two members: a second agent of the host's person is
  a second member, and Maple cannot be removed. Phase 4's rule that a goal's
  only member needs no approval then no longer applies. Under the default
  rule Juniper's results need an approval, and the only other member signs
  nothing while it is disconnected. The ways on are the person's: connect
  Maple again, so that it approves; add a third agent; or bind rules that
  need no approval, which later changes to the shared files then follow too.
  A goal that goes on with one working agent and no approvals, after its
  first agent is retired, is not to be had in v2. It needs a signed
  replacement of the starting agent, which belongs with replacing a host and
  never reaches a goal made under v2.
- `goal.status` has no `host` until the first record is held. Phase 4's
  `host_name` comes from the ticket in that window.
- The governance key is one more author in every sync frontier, which holds at
  most 4,096. A quiet goal of three opens 720 exchanges in an hour (measured,
  `idle_three_member_goal_keeps_exchanging_for_a_simulated_hour`); each now
  carries one more author entry.
- What public goals ask of this phase (the [review of the joinable
  contract](../research/joinable-farms-rewrite-contract-review-2026-10-05.md),
  findings 4, 5 and 12). One contract that says when a daemon may sign: K1 has
  none and leaves no hook. After K1 alone a daemon signs with the governance
  key whenever it holds the key and `Goal::next` answers a place. The contract
  is GUARD's `Node::hold` and `Node::admission_hold`, and it is GUARD that
  says how an ordinary private goal with only local members passes with no
  peer to ask (G1: an ordinary restart holds nothing). A retry of an admission
  already committed: K1 keeps it ahead of its own tests and of any signature
  (peers.rs above), so G1's hold comes after it. The first end: K1 builds
  none. E1's end record is signed with the governance key through
  `Node::host()`, has no frontier, and E1 says what records signed earlier can
  still do. A ticket, and so a public door, names the governance key and is
  signed by it; `host_name` is the name of the host's agent. Because the
  host's daemon admits the host's agent with the first record and replay keeps
  it, a member who came through a door is never a goal's only member.
- The reviewers' list, checked against the code. Confirmed: the first record
  and the goal identifier (event.rs); `signer` and `next_place`
  (authoring.rs); `Node::administrator` (access.rs); `plan_join` (peers.rs);
  `drive_flow` (node/flow.rs); the stage runner (goal/flow.rs); farm
  eligibility (farm.rs); `task_creator` (rules.rs). Wider than they said: farm
  eligibility also fails on a tree epoch or a task revision, not only on
  stages. Not on their list and found here: the member check in
  `authorize_base` is the only one replay makes of an author; `member_remove`,
  `goal_invite` and `farm_request` call `signer` themselves and not through
  `author`; `goal_invite` reads the title as a member; `goal_join` looks the
  administrator up among the members; `goal add-local` tests that the
  administrator is an active local agent; the page's change list keeps only
  records by consenting agents; `label` in presentation.rs would print the raw
  key. Their count of 211 uses of `administrator` in 45 files is 230 matching
  lines in 46 files at `2209b1b` and 233 in 48 at `c9c3b2c`; Phase 1 does that
  rename, so it is not this phase's cost.
- Size, an estimate: 700 to 1,000 changed production lines and 2,000 to 3,000
  lines of tests and fixtures. The test kit's `Author` founds a goal at about
  thirty call sites in twelve files and is built 61 times; `Fixture.admin`
  signs in about eighty places in goal/tests.rs and workspace_tests.rs.
  Splitting the fixtures into a governance signer and a host's agent is the
  first commit and changes no behaviour. The corrections of 6 October 2026
  are not in these numbers: the reconnect request and command, `agent revoke`
  applying at once, the refusal of `member remove` before its plan and the
  check on a stage's rules. They are small beside them and were not sized.
- The place left for replacing a host: the first lines of `Chain::build`,
  where the chain takes its key and its host's agent from the first record.
  Readers in replay and on the node go through `State.governance` and
  `State.host`. Two readers run before a chain exists and read the first
  record itself: `screen` and the join check in replica.rs.
- Read again for the final text, at `48bb12c`. Whole: authoring.rs, access.rs,
  peers.rs, node/flow.rs, chain.rs, screen.rs, delivery.rs, requests/goals.rs,
  requests/invitations.rs. In part: `eligible`, `project` and `farm_request`
  in farm.rs; `task_creator` and `matches` in rules.rs; `resolve` in
  callers.rs; `agent_revoke` in requests/daemon.rs; `receive`, `wanted_keys`
  and `offer_key` in replica.rs; `Goal::next` in goal/mod.rs; `decision` in
  fold.rs; `stage_template` and `review_templates` in goal/flow.rs; `init` and
  `initial_epoch` in cli/workspace.rs; `run` in local_members.rs; the test
  `redeemed_invitation_is_not_revoked_and_retries_recover_after_expiry`. The
  characterization tests are cited by name from the two research notes; their
  files were found and not opened. Inferred and not run: the `workspace init`
  refusal and its words, that a consent signed by the governance key cannot
  suspend a page, and what a disconnected host's agent does to a page.
- Read for this phase, at `c9c3b2c` in a checkout other sessions were editing.
  Whole: chain.rs, history.rs, screen.rs, rules.rs, state.rs, goal/flow.rs,
  authoring.rs, access.rs, node/local.rs, entry.rs, identity.rs, peers.rs,
  node/flow.rs, delivery.rs, requests/goals.rs, requests/invitations.rs,
  Organization.tla, lifecycle_characterization.rs. In part: event.rs,
  invite.rs, goal/mod.rs, fold.rs, farm.rs, commit.rs, callers.rs, testkit.rs,
  local_members.rs, requests/daemon.rs. By search only: api.rs, views.rs,
  context_views.rs, presentation.rs, the other command-line files, replica.rs,
  requests/tasks.rs, requests/workspace.rs, goal/workspace.rs, delegation.rs,
  projection.rs, presets.rs, the sync code, cases.json and the farm page.
  Inferred and not run: every behaviour after the change, the rewritten tests'
  outcomes, and the size.
- Read for the corrections of 6 October 2026, at `d3ad6b7`, whose crates are
  those of `51d50b8`. In part: `Validator` in validation.rs, to the end of
  `run`; `inherit`, `subset`, `implies` and `narrows` in delegation.rs;
  `agent_enroll` and `agent_revoke` in requests/daemon.rs; `PrincipalRecord`
  and `Principals` in identity.rs; `resolve`, `resolve_binding`,
  `task_creator` and `matches` in rules.rs; `stage_template`,
  `offer_templates` and `review_templates` in goal/flow.rs; the arms for an
  opened task, an offer, an attempt and a cancel request in fold.rs;
  `valid_definition` in goal/mod.rs and `validate_binding` in chain.rs.
  Inferred and not run: that clearing the flag makes the stored credential
  work again, the check on a stage's rules, what a subtask inherits, and
  every line the two commands print. The name `agent reconnect` is this
  text's.

### G1: A daemon knows what it signed

**Goal.** A daemon keeps, beside its data directory and never inside it, the
last record each of its keys signed in each goal. At a start it can tell
whether its data is the same files it last wrote, a copy older than what it
signed, or a copy of unknown age. A key whose records are missing signs
nothing in that goal until this daemon has caught up. After a copy of unknown
age no key signs: in a goal hosted on another computer until this daemon has
heard from that goal's other computers, and in a goal this daemon hosts until
the person runs one command, because no other computer can show that the copy
holds everything the host's computer signed. Where this daemon hosts the goal
and the host's own records are missing, its agents sign nothing there either.
That covers what the daemon signs by itself and what the person or an agent
asks the daemon to sign. An ordinary restart holds nothing and waits for no
person and no computer; waking from sleep is not a start. The
guard is this daemon's own behaviour. It changes no signed byte and no rule by
which any daemon judges a record.

**Depends on.** K1 and roles-plan Phases 1 to 6; it lands after Phase 6 and
before Phases 8 and 9. It is the eighth phase of the build
order, and the first public door is released after it, G2 and E1, as the owner
decided. What it calls is Phase 1's and K1's. From Phase 1: `Node::hosts()`,
`plan_join` without grants, the scan `invitation_revoke` uses for every
pending invitation, and the `denied` an agent gets on an `Owner` operation.
From K1: the governance key (`state().governance`), the host's agent
(`state().host`), and that every signature with the governance key passes
`Node::next_place` and `sign_at`. K1 leaves no hook of its own; the guard's
hooks are `Node::hold` and `Node::admission_hold`. The guard holds by key and
reads nothing else about a key. Phases 2 to 6 are on the tree when it lands,
so G2 can own the documents it makes stale. Nothing from END. The two
things named under [Models written first](#models-written-first) are
written: the model, with its page
[restore-guard.md](../research/tla/restore-guard.md), and the file-identity
note,
[restore-file-identity-2026-10-06.md](../research/restore-file-identity-2026-10-06.md).
What they found is folded into this phase's tests, notes and exit
criteria.

**Changes.** Terms, each with one meaning below.

- A *start* is `Node::open`. Waking from sleep is not a start.
- A *mark* is the last record one key signed in one goal on this daemon: its
  position and id. The *marks* are the file of all marks. It lives in a
  directory beside the data directory, so a copy of the data directory does
  not carry it.
- *Another computer* in a goal is an endpoint, other than this daemon's, that
  a current member is bound to. A member whose agent signed a leave stays a
  current member until it is removed, and its computer keeps exchanging until
  then (measured:
  `leave_is_visible_in_events_and_replication_and_rotated_keys_continue_until_removal`),
  so it counts. From E2 the host's computer signs that removal by itself, and
  never while its governance key is held, so a restored host keeps hearing
  from the computer of a member that left. The *host's computer* is the
  endpoint in the admission of the
  host's agent (`state().members[&state().host]`, K1's). Before the goal's
  first record is held it is `JoinRecord.endpoint`.
- This daemon has *heard from* a computer in a goal when, since this start, an
  exchange with it ran its record stage to the end and brought this daemon no
  record it did not hold. Either side may have opened it. The daemon then
  holds every record that computer was willing to send, and no more can be
  known. An exchange that brought records does not count; the next one does,
  and the driver opens it at once because the goal changed. The reason is in
  today's code: `screen` drops a member's records that arrive before the
  admission that names it, and a responder sends authors in key order, so one
  exchange can end with this daemon's own records still missing. A refused
  exchange never counts.
- A key is *held* in a goal while `Node::hold` answers a reason. *Behind*: the
  marks name a record the store does not hold. *Unheard*: the data is a copy
  of unknown age. *Admitted*: the key was just admitted here and this daemon
  has not yet heard from the host's computer, or cannot yet read the goal's
  current rules or seal under its current content key. Where this daemon
  hosts the goal, an agent's key is also held whenever the governance key
  is, for the governance key's reason.

What a start finds, from two facts: are the marks the file this installation
wrote, and is the database the file last used.

| Marks | Database | The start is | What follows |
| --- | --- | --- | --- |
| kept | the file last used | ordinary | nothing is held. If a mark is ahead of the store all the same, the database was overwritten in place; one file holds every goal, so every goal is treated as in the next row, not only the one whose mark is ahead. Only a mark of a key this computer signs with, in a goal that holds no `RESTORED` record, counts: a goal already found restored may still be catching up |
| kept | another file | the data directory was replaced by a copy | each key whose mark is ahead of the store is behind; where that key is the governance key, every agent's key in that goal on this computer is held with it; every other key signs at once |
| lost, or a copy | the file last used | ordinary | nothing is held; the marks are written again from the store. One exception: in a goal that still holds its `RESTORED` record, every local key is unheard, as in the row below |
| lost, or a copy | another file | a copy of unknown age | every key this daemon holds is unheard in every goal; the marks are written again from the store |

The third row's exception exists because a behind hold is remembered only
by the marks: losing them while a restore is still being caught up must not
make that goal ordinary. Where this daemon hosts the goal it then waits
for the person's `goal.continue`, as every unheard hold there does; on a
member's computer the hold ends when the host's computer is heard. The marks
can keep that a goal is a copy of unknown age only in a mark, and a goal in
which no key of this computer has a record in the copy has none. On a
member's computer the data directory alone can then go back to the same copy
before the hold ends, and that start is ordinary for the goal: an agent that
signed after the copy was taken signs again at a position another member may
hold. It is a limit of the same kind as risk (5) in the notes: it costs
that agent's key in one goal and leaves the goal unharmed. The simulator
leaves it out of its claim, and the run
`a_member_whose_agent_had_no_record_in_the_copy_signs_again_after_a_second_restore`
must end in that reused position. In the
second and fourth rows, and for every goal when the database was
overwritten in place, `restore_found` runs once per goal (below): the
overwrite rolled the whole file back, not only the goal whose mark is
ahead. A mark of a key this computer does not sign with was written by a
data directory this one replaced. It is kept, never lowered and never read:
it holds that directory's key if the directory is put back from a copy.
The marks are also lost when
their file holds a record whose checksum fails. A torn write then leads to the
third or the fourth row, never to a key with no mark that signs at once.

When a hold ends by itself:

| Hold | Ends when |
| --- | --- |
| Behind, any key | the marked record is held again, from any computer |
| Behind, the governance key | also at once when its mark says the goal never had a member on another computer and the copy shows none |
| Behind, an agent's key | also when this daemon has heard from every other computer in the goal and its own governance key, if it holds one there, is not held. No computer that can be asked has the record, and the mark is lowered |
| Any agent's key on the host's computer, held because the governance key is | when the governance key's hold ends |
| Unheard after a copy of unknown age, on a member's computer | it has heard from the host's computer; or from every other computer that is not the host's, when there is at least one |
| Admitted | it has heard from the host's computer and can read the goal's current rules and seal under its current content key. A new member's records arrive before the content they name, and the host's computer can be heard through an exchange it opened, which brings no content; until both have arrived most records the key signs would be refused as `unavailable`, since they cannot be judged or sealed yet. The hold ends by itself when that content lands |

Two holds end only on the person's command (`goal.continue`, below).

- An unheard hold in a goal this daemon hosts, from the start. The copy lists
  the computers that were members when it was made. A computer admitted since
  may hold the host's later records and is not on that list, and a computer
  removed since answers and brings nothing. So hearing from every listed
  computer shows nothing about what the host's computer signed last. While the
  hold lasts the daemon still dials every computer the copy lists and calls
  back a caller it does not know (below), so records keep returning. Every
  agent's key of this daemon in that goal is held with the governance key, by
  the fourth row.
- A behind hold on the governance key, for as long as no computer that
  answers sends the marked record. A computer can answer and send nothing:
  the model kept the case where the only other holder of the record was
  removed since, and a removed computer's exchanges finish and bring
  nothing (`restore-finding-removed-keeper`). That hold does not end by
  itself either. Once every other computer has answered, status lists the
  goal under "Waiting for you" with the command that ends it (G2). The
  daemon never gives up a governance record by itself: the records it is
  missing are the ones that say who the other computers are.

Every other hold waits for the computers its row names and for no person. If
one of them never answers the hold lasts, and the person can end it with the
same command. No clock ends a hold.

The two tables and `Node::hold` are the one signing-readiness contract.
`Node::hold(entry, key)` answers whether this daemon may sign with this key in
this goal now: none means it may. `Node::admission_hold(entry)` is the same
answer for the key that signs admissions. Every gate reads one of the two and
adds no condition of its own: `next_place`, `drive_flow`, `plan_join`, and
later the joinable plan's door and its farm-service check (J1 and J3). A join
this daemon already admitted is answered before the contract is asked, because
it signs nothing. These are its answers in the cases the [review of the
joinable
contract](../research/joinable-farms-rewrite-contract-review-2026-10-05.md)
names in finding 4.

| Case | What the contract answers |
| --- | --- |
| A private goal whose members are all on this computer | After an ordinary start nothing is held, and no person and no other computer is asked for anything. After the data directory is put back from an older copy the hold ends at once: the mark says the goal was never shared. After a copy of unknown age the goal waits for the person, once, as every goal this computer hosts does. The copy cannot show that the goal was never shared (answer 13) |
| A copy older than every member on another computer | Marks kept: the governance key is behind, its mark says shared, and an empty list of computers never ends that hold. A member's computer that calls is called back. Marks lost: unheard in a goal this computer hosts, so it waits for the person. The copy lists no other computer, and a member's computer that calls meanwhile is called back |
| A computer that answers but holds less, withholds, or sends nothing because it was removed since | Marks kept: only the marked record ends the hold, so none of the three can end it. The third is honest: a removed computer's exchanges finish and bring nothing, and the hold lasts although every computer answered (the model's `restore-finding-removed-keeper`). Marks lost, in a goal this computer hosts: no answer ends the hold, and the person decides when to continue. Marks lost, on a member's computer: not detected (residual 5 in the notes) |
| Records this daemon signed that reached no other computer | Marks kept: an agent's key gives them up once every other computer has answered. The governance key never does by itself; once every other computer has answered, G2 lists the goal for the person. Marks lost: not known, and harmless, since no other computer holds them |
| No member's computer can be reached | Nothing is signed and no clock ends the hold. With the marks kept it waits for a computer that holds the missing record, and status names the computers not heard from; the person's command is the only other way out. After a copy of unknown age a goal this computer hosts waits for the person in any case |
| A public goal | The same answers. The farm-service check can add a hold through `guard_attest`, where a start looked ordinary and the service shows that the copy is behind. It never ends a hold, and it does not show that the governance log is whole |

A hold ends in one of three ways, and only the first is proof. The first: the
marked record is held again, so this daemon's own log is whole up to its mark;
or the mark says the goal was never shared and the copy shows no other
computer. Both rest on the mark being the key's true last signature, which the
synced write below gives. The second: the computers a row of the second table
names were heard from, and no other computer can be asked for more. That is
not proof. It ends only holds on an agent's key: a key that is behind, an
unheard hold on a member's computer, a key just admitted. A wrong guess there
costs one agent its key in one goal. The third: the person continued, an
override, which proves nothing. A hold on the governance key ends by the first
or the third only. No view records afterwards which of the three it was.
`Response::Continued` tells the caller at the time, and no text calls the
second or the third "caught up".

- [store.rs](../crates/locust-proto/src/store.rs): new types beside `Commit`.
  ```rust
  /// Identity of a file on this computer. A copy of the file has another.
  pub struct FileId { pub ino: u64, pub created_ms: Option<u64> }
  /// The last record `key` signed in `goal` on this daemon.
  pub struct Mark { pub goal: GoalId, pub key: PublicKey, pub point: AuthorPoint,
                    /// The governance log held here admits a member on another computer.
                    pub shared: bool }
  pub enum MarkWrite { Set(Mark), Clear { goal: GoalId, key: PublicKey } }
  /// What was found beside the data directory when the store was opened.
  pub struct Marks { pub file: FileId, pub kept: Option<Vec<Mark>> }
  ```
  `Commit` gains `marks: Vec<MarkWrite>`, applied in order. They are written,
  and the marks file is synced, after the events, objects and local writes of
  the commit are durable and before `commit` returns. So no record reaches a
  peer before its mark is on disk, and a failed commit writes none. A commit
  with no mark pays nothing. A mark that cannot be written or synced is
  `StoreError::Failed`, and the store refuses further calls as after a failed
  sync. `Store` gains `fn marks(&self) -> Result<Marks, StoreError>`: `file`
  identifies where the store keeps its records now; `kept` is `None` when no
  marks were found, they did not read, or the file that holds them is not
  the one they were written in. `MemStore`: `MemState` gains a `FileId`
  drawn from a process-wide counter in `MemStore::new`, so `reopen` keeps
  it and a new
  store has another; the marks live in new `MemMarks` (a shared cell that
  reads as lost until first written), with `MemStore::with_marks(MemMarks)`
  and `MemStore::marks_handle()`. The `conformance` module gains one case. The
  comment on `Space::Identity` loses "signing watermarks" and names the file
  record below. The third implementor, `Failing` in
  [tests/failure.rs](../crates/locust-core/src/node/tests/failure.rs),
  forwards `marks`.
- `crates/locust-store/src/marks.rs` (new),
  [store.rs](../crates/locust-store/src/store.rs) and the crate comment in
  [lib.rs](../crates/locust-store/src/lib.rs): the file. One header (a magic
  value and the marks file's own `FileId`, read after the file is created)
  and fixed records of 112 bytes: goal, key, position, event id, `shared`,
  a 4-byte checksum. A changed mark is one positional write in place, and a
  new
  record is appended. Every commit that carries marks writes them all and then
  syncs the file once, before `commit` returns, so no signature can lose its
  mark to a power failure. That is one more flush for a commit that signs,
  whatever the number of its marks, and none for a commit that carries no
  mark. Creating or replacing the file also syncs its directory, as
  `Files::create` does today for the object directory
  ([files.rs](../crates/locust-store/src/files.rs)). Both syncs pass the fault
  points of `faults.rs`, as the syncs of files.rs do. `read(dir)` answers
  `None` when the file is missing, the header does not read, the file's own
  `FileId` differs from the header's, or any record's checksum fails; such
  a file is replaced by an empty one with a fresh header. So a torn record
  makes the marks lost, which the first table handles, and never leaves a
  key with no mark that signs at once. Identifying the file by its own
  identity, not its directory's, is what makes an older marks file restored
  into the same directory read as lost: a directory keeps its inode and
  creation time when a file inside it is replaced. The marks a start writes
  (below) follow the same rule and are on disk before the transport starts.
  `FileId::of(path)` takes
  the inode number and the creation time from the file's metadata; the device
  number is left out because it can change between boots. Where the file
  system reports no creation time, `created_ms` is `None` (measured on
  tmpfs) and the inode number alone decides; so it does when one of the two
  identities compared has a creation time and the other has none
  (`FileId::same`), so that a sandbox or file system that starts or stops
  reporting it does not make an untouched file look copied.
  `SqliteStore::open(dir, marks)` creates the marks directory with mode 0700,
  and refuses a marks directory or file that anyone but its owner may read;
  `commit` applies `commit.marks` after the SQLite transaction and syncs the
  marks file; `marks()` answers `FileId::of` the database file and
  `read(marks)`. The 25 call sites of `SqliteStore::open` (one in
  `daemon/mod.rs`, the rest in tests and one example) pass a directory.
- [local.rs](../crates/locust-proto/src/local.rs): `MARKS_SUFFIX = ".marks"`
  and `marks_dir(home: &Path) -> Result<PathBuf, LocalError>`, the home's
  path with the suffix added to its last part: `~/.locust.marks` for the
  default home. A home that ends in `..` or is `/` has no last part to add it
  to, and `home_dir` refuses it. There is no
  environment variable. [daemon/mod.rs](../crates/locust/src/daemon/mod.rs):
  `run` computes it and `run_networked_with(home, marks, ..)` passes it to
  `SqliteStore::open`.
- [sync.rs](../crates/locust-proto/src/sync.rs): `Refusal` gains a last
  variant `CatchingUp` (`"catching_up"`): the responder is catching up after a
  restore and admits nobody yet, and the joiner's computer asks it again. It
  is the one wire addition: one unsigned sync frame, inside protocol 7, which
  K1 raised from 6. It changes no signed byte, and no number is raised for the
  guard.
- `crates/locust-proto/src/api/guard.rs` (new) and
  [api.rs](../crates/locust-proto/src/api.rs):
  ```rust
  pub struct GuardView { pub key: PublicKey, pub by_host: bool, pub reason: GuardReason,
                         pub heard: Vec<EndpointId>, pub waiting: Vec<EndpointId> }
  #[serde(rename_all = "snake_case")]
  pub enum GuardReason { Behind { held: u64, signed: u64 }, Unheard, Admitted }
  ```
  `by_host` is K1's name for "this is the governance key". `GuardReason` is
  tagged the ordinary way: a response travels in postcard, which cannot read
  an enum tagged inside its fields. `waiting` lists the other computers not
  yet heard from. A view is about a key's own hold. An agent that is held only
  because the governance key is held gets no view of its own: the governance
  key's view is its reason. `GoalStatus` gains `guard: Vec<GuardView>` (for
  the owner, every key of this daemon with a hold of its own in the goal; for
  an agent, its own and, where this daemon hosts the goal, the governance
  key's) and `restored: Option<u32>` (set while this daemon holds the goal's
  `RESTORED` record; the number is the invitations revoked). `GoalSummary`
  gains `guard: Vec<GuardView>` (the hold on its agent and, where this daemon
  hosts the goal, the hold on the governance key) and `restored: Option<u32>`,
  both repeated on each of the goal's summaries as `halted` is. `status` is
  one `GoalSummary` per local agent in a goal (read in api.rs and views.rs),
  and after K1 the governance key is no agent, so this is how `status` carries
  the host's hold. `Halt::SignerRecovery`, declared and never set today, is
  set from here on for a `Behind` or an `Unheard` hold, and never over
  `AuthorityConflict`. `Halt` gains `SignerConflict`: this agent has two
  records at one position in this goal and signs nothing more in it; the goal
  is not halted. `ErrorCode::ReadOnly` is what a signature held as `Behind` or
  `Unheard` answers; `failure.rs` already maps it to exit 9. `Admitted` is the
  reason of a key that was just admitted. A signature held for it is refused
  as `unavailable` with `admission has just arrived; Locust is checking with
  the host's computer and fetching the goal's rules and content key`. It sets
  no halt. New `Request::GoalContinue { goal }`
  (`goal.continue`; goal-scoped; audience `Owner`; not a tool), answered by
  new `Response::Continued { keys: u32 }`, the number of keys released. The
  contract lists one more operation than the tree this phase starts from and
  the same number of tools. All of it rides inside API version 7, which Phase
  1 raised.
- `crates/locust-core/src/node/guard.rs` (new):
  ```rust
  pub(super) enum Hold { Behind { mark: AuthorPoint }, Unheard, Admitted }
  pub(super) struct Guard {
      marks: BTreeMap<(GoalId, PublicKey), Mark>,       // read at the start, then every signature
      heard: BTreeMap<GoalId, BTreeSet<EndpointId>>,    // memory only; empty at every start
      callers: BTreeMap<GoalId, VecDeque<EndpointId>>,  // memory only; at most CALLERS = 8 a goal
  }
  pub(super) struct Sources { pub all: BTreeSet<EndpointId>, pub host: Option<EndpointId> }
  ```
  - `Node::hold(&self, entry, key) -> Option<Hold>`: none when the key's log
    is forked (`Goal::fork_point`): such a key signs nothing anyway and status
    says conflict. `Behind` when a mark exists for (goal, key) and its point
    is not in the usable prefix of the key's log (`Goal::points`,
    `Goal::usable` in [goal/mod.rs](../crates/locust-core/src/goal/mod.rs)).
    Else, where this daemon hosts the goal and `key` is not the governance
    key, whatever `hold` answers for the governance key. This computer's copy
    of who is in, the rules and whether the goal has ended is then known to be
    old. Else `Unheard` when the goal's `RESTORED` record says so. Else
    `Admitted` when `entry.local.unheard` holds the key. Else none.
  - `Node::guard_admissions(&mut self, goal)`: deletes the goal's `UNHEARD`
    records of keys admitted here once the host's computer has been heard and
    `ready(entry)` holds: the goal's current rules are readable and the
    content key of its current epoch is held. `guard_settle` applies the same
    test when it hears the host's computer. `Node::land` runs it for every
    goal it touches, because content and keys land outside any hearing.
  - `Node::admission_hold(&self, entry) -> Option<Hold>`: `hold` for the key
    that signs admissions in this goal, the governance key. This is the hook
    `plan_join` calls, and the one a public door reads for its state. A join
    this daemon already admitted never reaches it.
  - `Node::guard_sources(&self, entry) -> Sources`: `all` is every other
    computer; `host` is the host's computer, as the terms define it, when this
    daemon does not host the goal. It is the one function that names whom this
    daemon must hear from, and the place host replacement plugs into.
  - `Node::heard(&self, goal) -> &BTreeSet<EndpointId>`.
  - `Node::guard_settle(&self, goal, tx)`: applies the second table. It
    deletes `UNHEARD` records and clears `unheard` in the `RESTORED` record
    when their rule is met, and lowers a mark (`MarkWrite`) where the table
    gives up a record. In a goal this daemon hosts it never clears `unheard`:
    only `goal_continue` does. A settle that ends a hold touches the goal, so
    `land` runs `drive_flow` and a waiting step is signed at once. Called at
    the start and after every `reconciled`. A `Behind` hold needs no settle:
    `hold` reads it from the marks and the log each time.
  - `Node::restore_found(&self, entry, unheard: bool, now_ms, tx)`: writes the
    goal's `RESTORED` record. Where this daemon hosts the goal it also revokes
    every pending invitation of the goal, through `revoke_pending` (below),
    and stores their count. A copy cannot know which tickets were used or
    revoked after it was taken. For a goal found overwritten in place it does
    nothing when the record already exists, so a restart while still behind
    does not revoke an invitation issued since. The joinable plan adds its
    door line here.
  - `Node::halt(&self, entry, key: Option<&PublicKey>) -> Option<Halt>`:
    `Entry::halted()` first (`AuthorityConflict`); else `SignerConflict` when
    `Goal::fork_point(key)` is set and `key` is not the governance key; else
    `SignerRecovery` when `key` is held as `Behind` or `Unheard`.
    `Entry::halted` in [views.rs](../crates/locust-core/src/node/views.rs)
    stays as it is; it cannot see the marks. For a goal whose copy holds an
    end (`Goal::end_held`, END's), `Node::halt`, `GoalStatus.guard` and
    `GoalSummary.guard` answer nothing, and G2's `waiting_for` lists no
    `CatchingUp` entry. `Node::hold` and the stored records are unchanged, so
    a voided end would be held again. `Node::halt` still answers
    `AuthorityConflict` there, which END prints as `, then halted`.
    `Goal::end_held` arrives with E1, so E1 writes this test into the guard's
    functions; the rule is stated here because the functions are the guard's.
  - Specified here and built with its first caller, the joinable plan's
    service check, because a function nothing calls fails `clippy -D
    warnings`: `Node::guard_attest(&mut self, goal, now_ms)`. Its caller calls
    it when the farm service's answer shows that this copy is behind: the
    service refused a request at a number it had already used. It calls
    `restore_found` with `unheard: true`, so it catches a rollback the marks
    cannot see, and the goal then waits for the person like any goal this
    daemon hosts after a copy of unknown age. It does that only where this
    start found nothing for the goal: the first table called the start
    ordinary, no mark was ahead of the store, and the goal holds no `RESTORED`
    record. Otherwise it does nothing. With the marks kept they say exactly
    what is missing, and a page number that is behind adds nothing to that
    proof; after a copy of unknown age the goal is unheard already. Private
    goals never call it. No answer of the service ends a hold. A request the
    service accepts shows that the page's number is current, not that the
    governance log is whole: records this daemon signed after the last request
    the service saw are outside it. And the service answers an identical old
    request with the receipt it stored, also after newer requests and a
    restart (measured:
    `old_identical_check_in_returns_its_receipt_after_newer_requests_and_restart`).
    So the service's answers are used only as evidence that a copy is behind,
    which is what answer 21 allows. One limit binds its caller. A copy's page
    number stays behind the service's at every later start, until the joinable
    plan lets the publisher catch up. The caller reports one such finding
    once, or a goal the person continued would be held again at the next
    start.
- [node/local.rs](../crates/locust-core/src/node/local.rs): two records, with
  rows in the table at the top of the file and arms in `absorb` for writing
  and deleting. `RESTORED = b'R'`: goal; value new `Restored { revoked: u32,
  unheard: bool }`; `Local.restored: Option<Restored>`. `UNHEARD = b'u'`:
  goal, key; value `()`; `Local.unheard: BTreeSet<PublicKey>`. Both are
  durable, so a restart while held stays held. Neither tag is used in
  `Space::Goal` today, by roles-plan Phase 3 or by K1, whose key record takes
  `K`. With the file record and `Commit.marks` they ride inside store marker
  7, which Phase 1 raised; the marks file has its own header and no number.
- [identity.rs](../crates/locust-core/src/node/identity.rs): `Identity` gains
  `file: Option<FileId>`, the database file this daemon last ran on, under a
  new key, with `Identity::file_write` and an arm in `absorb`.
- [node/mod.rs](../crates/locust-core/src/node/mod.rs): `Node::open` reads
  `store.marks()` after loading the goals and before its last loop, which
  calls `drive_flow` and today signs before any peer is heard. It builds
  `Guard` by the first table and writes one commit: `restore_found` for each
  goal the table names; marks from the store's own tips where they are lost,
  except in a goal that still holds its `RESTORED` record, whose keys become
  unheard instead (the first table's exception, decided before the marks are
  rewritten and before any `RESTORED` record is deleted);
  any mark below the store's tip raised to it; the `RESTORED` record deleted
  where the start is ordinary and no key of this daemon is held in the goal;
  and `file` when it changed. That commit syncs the marks like any other, so a
  mark raised here is on disk before the first exchange. A store that holds no
  goal and no file record is new: `file` is written, nothing is held, and the
  marks found beside it are kept. The parameter `_now_ms` becomes `now_ms`.
- [authoring.rs](../crates/locust-core/src/node/authoring.rs): `next_place`
  asks `hold` after its membership test, which K1 skips for the governance
  key, and before `Goal::next`. For `Behind` and `Unheard` it answers
  `ErrorCode::ReadOnly` with one of two sentences: "this computer's Locust
  data is older than what it signed in this goal; it is catching up" or "this
  computer's Locust data may be an old copy; it is catching up, or waiting for
  its owner to continue". For `Admitted` it answers `unavailable` with the
  sentence above. An author whose own log is forked, and that is not the
  governance key, gets `ErrorCode::Halted` with "this agent has two records at
  one position in this goal and signs nothing more in it; the goal is not
  halted". Today that author gets `unavailable` and "the goal's history has
  not arrived yet" (read in authoring.rs and `Goal::next`). The hold answers
  before a missing content key does: `author` calls `next_place` first and
  looks for the epoch's key after. The order of the whole function once every
  piece has landed: the copy holds an end, `conflict(ENDED)` (E1); member and
  `Local.part`, skipped for the governance key (K1); `hold` (G1);
  `Goal::next`. Every signature after a goal's first three records passes
  `next_place` (read: `author`, `member_remove`, `cancel_acknowledge`;
  `goal_create` signs its three records directly), so this is the one gate.
  `sign_at` adds a `MarkWrite::Set` for each event it signs. An invitation is
  signed but is not a record at a position: issuing one is not held, admission
  on it is.
- [commit.rs](../crates/locust-core/src/node/commit.rs): `land_once` sets
  `shared` on the commit's marks after `advance`, from the admissions in the
  governance log held here, and copies the marks into `Guard::marks` once the
  store returned. The bit rides the commit of the admission that first sets
  it, so the synced write puts it on disk before that admission reaches the
  joiner. `finish_joins` also writes `UNHEARD` for the admitted key
  unless `hosts(entry)`; that key is then held as `Admitted`. In a healthy
  join that hold lasts until the next exchange with the host's computer, about
  a second, and until the goal's current rules and content key have arrived
  (`guard_admissions`), which is later when the goal holds many records. The
  second is by reading, not measured: the admission changes the
  goal, so the driver opens the next exchange at once, and the shell polls the
  driver every second. Where that exchange does not come, the next is the
  ordinary one: in a quiet goal each pair exchanges every 30 seconds
  (measured: 120 exchanges an hour for each ordered pair,
  `idle_three_member_goal_keeps_exchanging_for_a_simulated_hour`).
- [node/flow.rs](../crates/locust-core/src/node/flow.rs): the `find` in
  `drive_flow` also requires `self.hold(entry, &desired.runner).is_none()`. A
  held step is passed over, not an error. This test is required: without it
  `author` fails inside `land`, a received batch is refused as a protocol
  error and `Node::open` fails. With K1 the rule reads: a step whose runner is
  the governance key is signed when this daemon hosts the goal, the guard does
  not hold the key and the key can sign next. A step whose runner is a
  member's agent passes Phase 3's four tests and is not held by the guard.
- [peers.rs](../crates/locust-core/src/node/peers.rs): `plan_join` asks
  `admission_hold` at the one place it is about to sign: after every check
  that refuses for good, before `author`. A held host answers
  `Refusal::CatchingUp` and uses up nothing. A retry of an admission this
  daemon already holds is answered before the guard is asked, as today: the
  branch of `plan_join` for a ticket redeemed by this member at this endpoint
  returns before `author` is reached and signs nothing. `exchange_ended`
  already marks a join refused only for `InvitationRefused`, so the joiner's
  computer asks the host's computer again with the driver's backoff, one
  second doubling to sixty. `Host
  for Node` gains `reconciled(goal, endpoint)`, which adds to `Guard::heard`
  and lands `guard_settle`, and `note_caller(goal, endpoint)`, which remembers
  the endpoint (newest eight) when this daemon hosts the goal, its governance
  key is held there and the endpoint is bound to no current member. `peers()`
  also lists a goal's callers; they are dropped when the hold ends.
- [driver.rs](../crates/locust-core/src/sync/driver.rs),
  [initiator.rs](../crates/locust-core/src/sync/initiator.rs),
  [responder.rs](../crates/locust-core/src/sync/responder.rs): the trait
  `Host` gains `reconciled` and `note_caller`, both with empty defaults.
  `Initiator` gains `received: bool`, set when `Replica::receive` kept a
  record, and `reconciled()`, true once `stage` is past `Reconcile` with
  `outbound_authorized` and nothing received. `Dialed` gains `heard: bool`;
  `dialed_step` and `dialed_writable` call `host.reconciled` once per exchange
  when it turns true, before content is fetched. `accepted_step` calls it
  where it calls `peer_completed`, when `Responder::received()` is false.
  `Responder::hello` takes `&mut dyn Host` and calls `note_caller` when the
  remote does not speak for a member. A caller is dialed like a peer.
  `authorize_outbound(false)` before the first frame makes the initiator send
  an empty frontier, as a joiner's exchange does, so the caller is sent
  `Hello` and nothing else until this daemon's own records name it a member.
- [requests/goals.rs](../crates/locust-core/src/node/requests/goals.rs),
  [views.rs](../crates/locust-core/src/node/views.rs),
  [context_views.rs](../crates/locust-core/src/node/context_views.rs),
  [requests/invitations.rs](../crates/locust-core/src/node/requests/invitations.rs):
  new `goal_continue`: the owner only; in the goal it deletes every `UNHEARD`
  record, clears `unheard` in the `RESTORED` record, and sets every mark that
  is ahead to the tip the store holds (`MarkWrite::Clear` when the store holds
  none); it touches the goal and answers the number of keys released.
  `goal_status` and `goal_summaries` fill `guard` and `restored`;
  `GoalStatus.halted` is `Node::halt` for the governance key held here, and
  `GoalSummary.halted` and `ContextBrief.halted` for the agent they are about.
  In `goal_join`, the local branch turns `Refusal::CatchingUp` from
  `plan_join` into `ReadOnly` with the hold's sentence, so a person adding
  their own agent to a held goal reads the real reason. The scan Phase 1 gives
  `invitation_revoke` for no id becomes `revoke_pending(&self, goal, now_ms,
  tx: &mut Tx) -> Result<u32, ApiError>`; `invitation_revoke` and
  `restore_found` call it.
- [presentation.rs](../crates/locust/src/cli/presentation.rs):
  `halt(Halt::SignerRecovery)` reads `Catching up: nothing is signed here
  until this computer has caught up.` and `halt(Halt::SignerConflict)` reads
  `Conflict: two of this agent's records sit at one position. It signs nothing
  more here.` The full words are G2's.
- Test harnesses. `snapshot` in
  [tests/mod.rs](../crates/locust-core/src/node/tests/mod.rs) gives the copy
  the original's marks, as a replaced data directory has, and new
  `snapshot_all` gives it lost marks, as a whole-computer restore has; its
  callers in delivery.rs, replica_tests.rs and the characterization files say
  which they mean. The three callers that came with the lifecycle measurements
  (two in tests/farm_characterization.rs, one in
  tests/lifecycle_characterization.rs) keep `snapshot`. By reading, none of
  them signs after its restore with a key the guard holds: each goal has all
  its members on one computer, so a missing agent record, where there is one,
  is given up at the start. Not run. `Running` in
  [durable_tests.rs](../crates/locust/src/daemon/durable_tests.rs) gains
  `start_with_marks(home, marks)`. Every helper that hands out a daemon home
  (`short_dir` in [testdir.rs](../crates/locust/src/testdir.rs) and the
  helpers of the command tests under `crates/locust/tests`) hands out a
  directory one level inside its temporary directory, so the marks directory
  beside the home is removed with it.
- `python3 scripts/check_formations.py --write` regenerates
  `docs/reference/generated/runtime.contract.json`.

**Tests.**
- In the store's [tests.rs](../crates/locust-store/src/tests.rs):
  `marks_survive_reopen_and_a_torn_record_makes_them_lost` (one record's
  checksum fails and `kept` is `None`);
  `marks_in_a_copied_directory_are_not_kept` (the directory is copied file by
  file and `kept` is `None`);
  `an_older_marks_file_put_back_into_its_directory_reads_as_lost` (the marks
  directory survives, an older marks file is restored into it, and `kept` is
  `None`); `a_failed_commit_writes_no_mark`;
  `a_failed_mark_sync_breaks_the_store_and_nothing_is_released` (the sync of
  the marks file fails at the existing fault point `Point::FileSync`; `commit`
  answers `StoreError::Failed` and the store refuses further calls);
  `a_commit_with_marks_syncs_the_marks_file_once_and_one_without_syncs_none`
  and `creating_the_marks_file_syncs_its_directory`, both read from the fault
  trace; `a_copied_database_file_has_another_identity`. In the `conformance`
  module: `marks_follow_their_commit_and_survive_reopen`. The crash tests in
  [tests/crash.rs](../crates/locust-store/tests/crash.rs) kill a process and
  cannot simulate a power loss, so a mark's durability rests on the sync and
  is tested at its fault point.
- In `locust-proto`: `refusals_render_in_snake_case` gains `catching_up`;
  `the_published_names_and_modes_are_stable` gains the suffix; new
  `the_marks_directory_is_beside_the_home_and_never_inside_it`.
- New `crates/locust-core/src/node/tests/guard.rs`, on `Network` from
  [delivery.rs](../crates/locust-core/src/node/tests/delivery.rs), which is
  two daemons today and gains `Network::with(n)`:
  - `an_ordinary_restart_holds_nothing_and_signs_at_once`: a host alone, and a
    host whose only member is unreachable, each sign a waiting step inside
    `Node::open`.
  - `only_the_goals_that_are_behind_are_held`: after the data is put back from
    an older copy, a goal with no signature since the copy signs at once.
  - `a_goal_never_shared_is_not_held_by_a_missing_record`: a host alone put
    back from an older copy signs at once, and so does a host whose second
    agent is on the same computer.
  - `on_the_hosts_computer_no_agent_signs_while_the_hosts_own_records_are_missing`:
    the copy is older than a second admission; the host's agent's post is
    `read_only` until the admission returns and then needs the other member's
    approval.
  - `a_member_that_lacks_the_later_records_does_not_open_the_guard`: the
    member that answers holds less than the mark; the key stays behind until
    the other member answers.
  - `a_copy_of_unknown_age_on_the_hosts_computer_waits_for_the_person`: store
    and marks both restored on the host's computer; after both other computers
    were heard from nothing is signed, and a waiting step is signed after
    `goal.continue`. Its second half is the trace the rule answers: after the
    copy one member was removed and another admitted; the removed member's
    computer answers and brings nothing, the host stays held, and the admitted
    member's computer, which the copy does not list, calls, is called back and
    returns the later records.
  - `a_mark_short_of_the_store_is_raised_at_the_start_before_any_exchange`:
    the marks are put back as they were before the last commit, as after a
    crash between the database commit and the sync of the marks; the start
    raises the mark to the store's tip in its own commit, and no exchange
    opens before that commit returns.
  - `an_exchange_that_brought_records_does_not_count_as_hearing`: the hold
    ends on the second exchange, not the first.
  - `a_key_admitted_while_catching_up_is_held_with_the_rest`: a local agent
    the copy does not list as a member signs nothing after the admission
    arrives until its own log is back.
  - `on_a_members_computer_the_hosts_computer_or_all_the_others_end_the_hold`.
  - `with_every_member_unreachable_nothing_is_signed_until_the_person_continues`.
  - `a_copy_from_before_the_first_admission_catches_up_when_a_member_calls`:
    the copy shows nobody, the mark says shared; the later member's `Hello`
    makes the host dial back, its own log returns, the hold ends.
  - `an_unknown_copy_that_shows_nobody_waits_for_the_person`: nothing is
    signed until `goal.continue`.
  - `a_record_no_other_computer_holds_is_given_up_for_an_agent_key_and_kept_for_the_governance_key`.
  - `a_forked_key_is_reported_as_a_conflict_and_not_as_catching_up`: an
    agent's forked key reads `signer_conflict`, its request answers `halted`,
    and the goal is not halted; the governance key's fork reads
    `authority_conflict` as today.
  - `the_persons_own_command_is_held_like_any_signature`: `rules.bind` and a
    local `goal.join` answer `read_only` on a held host and pass after
    `goal.continue`.
  - `continuing_is_the_owners_alone`: an agent's credential gets `denied`.
  - `a_caller_is_sent_nothing_until_this_daemons_own_records_name_it`;
    `callers_are_bounded_and_forgotten_when_the_hold_ends`.
  - `a_held_join_is_asked_again_and_not_refused_for_good`.
  - `a_join_already_admitted_is_answered_while_the_host_is_held`: the member's
    retry on its redeemed ticket passes and signs nothing.
  - `a_restore_revokes_pending_invitations_once_and_only_where_this_daemon_hosts`:
    a restart while still behind leaves an invitation issued since.
  - `a_record_signed_after_continuing_is_judged_like_any_other`: the person
    continues too early, the host signs at the used position, both daemons
    report the fork as today. This is the proof that the guard is not
    validity.
- Rewritten in delivery.rs, each from the form K1 left of a test that measured
  today's behaviour: `a_restored_hosts_agent_forks_only_its_own_log` becomes
  `a_restored_hosts_agent_is_held_until_its_own_records_return_and_then_extends`;
  K1's form of
  `restored_host_redeems_outstanding_invitation_before_recovery_at_an_already_used_position`
  (it loses an admission) becomes
  `a_restored_host_admits_nobody_until_caught_up_and_its_old_invitation_is_revoked`
  (the old ticket is refused for good; a ticket issued after the restore is
  answered `catching_up` and then admitted at the next free position);
  `restored_host_automatic_stage_replay_is_identical_unless_another_event_used_its_position`
  becomes `a_restored_host_signs_no_stage_step_until_caught_up`, and its
  identical-replay half stays as
  `a_stage_step_signed_twice_from_one_input_is_one_record`, signed from two
  stores;
  `restored_host_known_gap_blocks_signing_until_missing_predecessor_arrives`
  becomes
  `a_restored_host_with_a_gap_stays_held_until_the_missing_record_arrives` and
  expects `read_only`.
- In [replica_tests.rs](../crates/locust-core/src/node/replica_tests.rs),
  whose `Peer` harness carries the marks: the body of
  `same_key_rejoin_recovers_own_log_before_signing_when_sync_finishes` is
  unedited.
  `same_key_rejoin_can_sign_at_zero_if_membership_and_content_arrive_before_own_log`
  becomes `a_rejoined_key_signs_nothing_until_its_own_log_returns` (marks
  kept: behind at position 0) and gains a twin,
  `a_newly_admitted_key_waits_for_the_hosts_computer_once` (marks lost: held
  as `admitted`, and its request answers `unavailable`, until an exchange with
  the host's computer brings nothing new).
- In [sync/tests/driver.rs](../crates/locust-core/src/sync/tests/driver.rs):
  `a_dialed_exchange_is_heard_only_when_it_brought_nothing`;
  `an_accepted_exchange_is_heard_at_done_when_nothing_was_pushed`;
  `an_unknown_caller_is_dialed_back_with_an_empty_frontier`.
- In [durable_tests.rs](../crates/locust/src/daemon/durable_tests.rs): K1's
  two tests,
  `sqlite_older_directory_forks_only_the_hosts_agent_when_its_own_work_was_lost`
  and
  `sqlite_older_directory_halts_governance_when_a_governance_record_was_lost`,
  become `an_older_directory_is_held_until_its_later_events_return`, run once
  for a lost agent record and once for a lost governance record (real
  directories; the copy is started with the original's marks; the request
  exits `read_only`, and after the events return it signs at N+1). New
  `a_lost_marks_directory_is_an_ordinary_start_and_a_copy_of_both_is_not` and
  `a_database_overwritten_in_place_is_found_by_its_marks`. The first holds
  only where no goal still holds its `RESTORED` record; with one held, the
  lost marks make that goal's keys unheard instead, and
  `lost_marks_while_a_restore_is_caught_up_make_the_goal_unheard` covers it
  (on the host the goal then waits for `goal.continue`; on a member's
  computer the hold ends when the host's computer is heard). The second now
  treats every goal in the file as restored: a second goal with no mark
  ahead gets its `RESTORED` record and, where this daemon hosts it, its
  pending invitations revoked, and no key of it is behind.
- In [reconcile_tests.rs](../crates/locust/src/daemon/reconcile_tests.rs):
  `a_diverged_author_log_reconciles_between_two_real_daemons` sends
  `goal.continue` on the owner's connection before the restored member signs,
  since nothing else makes it diverge now.
- In the simulator ([chaos.rs](../crates/locust-core/src/node/sim/chaos.rs),
  [check.rs](../crates/locust-core/src/node/sim/check.rs)): new
  `Kind::Restored { m, marks }` beside `Down` and `Asleep`: a stopped
  machine's store goes back to an earlier snapshot, with its marks kept or
  lost. New invariant
  `a_restored_machine_signs_at_no_used_position_unless_its_owner_continued`,
  claimed where it holds and nowhere else. A used position is one at which a
  machine still holds a record, so what the invariant says is the model's
  `StoreNoFork`: no two machines ever hold different records at one
  position. A position whose record was lost everywhere can be signed again
  under the two give-up rules, and the claims below leave those runs out.
  For the governance key the claim holds in every run whose goal had a
  member on another computer, with the marks kept or lost and with members
  admitted or removed after the snapshot: a mark is on disk before its
  record can leave, and with the marks lost the hold ends only on the
  owner's command. In a goal never shared the mark is lowered at the start
  and the position can be signed again; the lost record reached no other
  machine (the model's `restore-finding-private-reuse`). For an agent's key
  on the machine that hosts the goal it holds whenever the marks are lost,
  because the key is held with the governance key. With the marks kept it
  holds for an agent's key in runs with no removal that the restored copy
  does not hold, where the missing record is held by another machine that
  can still be asked. Where the record reached no other machine the mark
  is given up once every other computer has answered, and the position can
  be signed again (the model's `restore-finding-unseen-agent-reuse`).
  It is not claimed for an agent's key on a member's machine whose marks are
  lost, and that stays so after a later restore of the same machine with its
  marks kept, at a position the marks have not named since the marks were
  lost: they were written again from a copy without that record, so the
  later restore finds nothing missing. Nor is it claimed for an agent's key
  on a member's machine restored with its marks kept to a copy older than an
  admission, when the host's computer held that admission and was itself
  restored to a copy without it:
  no computer that answers knows the admitted member, so the mark is given
  up once every computer the copy lists has answered, and a record only the
  admitted member holds can be signed over. A host copy that lacks the
  admission only because it was taken before the admission existed does not
  count: the host's computer learns of the admission again and answers for
  that member. Four runs built step by step are kept that must end in a
  reused position, all under risk (5) in the notes (the limit of a goal with
  no mark, under the start table above, has its own): a member's machine
  restored with its marks lost, whose agent's last record reached another
  member's machine and not the host's; the same machine, once caught up by
  the host's computer, restored again with its marks kept before its agent
  signs; an agent's
  record that reached only a machine whose removal the restored copy does
  not hold, given up with the marks kept; and a member's machine and the
  host's each restored to a copy from before the same admission, the
  member's with its marks kept. A seeded run that fails counts as one of
  these residuals only when every invariant it breaks follows from the
  reused key: that key's halt, or records standing on its reused records
  that are not effective. These bounds are from
  reading. A seed that breaks the invariant inside them is a finding. The
  invariant that every machine holds every acknowledged record leaves out a
  record that only the restored machine held. Sleep stays what it is there,
  and no test treats a wake as a start.
- `operations_that_are_only_the_persons_are_never_tools` in `api.rs` gains
  `goal.continue`. `signed_current_protocol_vectors_are_frozen` passes
  unedited. Phase 1's tests of `invitation revoke` with no id pass unedited
  over `revoke_pending`.

**Exit criteria.**
- The three cargo commands and `python3 scripts/check_formations.py` pass.
  `git grep -n "signing watermarks" -- crates` finds nothing. The executable
  recipes and the helper tests pass as CI runs them: `python3
  scripts/check_documentation.py --binary target/debug/locust --timeout 60`
  and `python3 -m unittest discover -s scripts/tests`. This phase removes no
  interface they call.
- `python3 scripts/check_tla.py --suite restore` passes with the outcomes
  the registry declares. Passing: the safety cases `restore-p1-store`,
  `restore-p2-all`, `restore-p3-ordinary`, `restore-p4-all-held` and
  `restore-p5-agent-fenced`, the two extended twins, and the recovery cases
  `restore-p4-recovery`, `restore-p4-alone`, `restore-p4-agent-give-up` and
  `restore-p4-willing-general`. Declared violations, kept as traces: the
  six rule removals `restore-lacking-peer`, `restore-empty-shared`,
  `restore-hear-with-data`, `restore-give-agent-held`,
  `restore-agent-signs-held` and `restore-all-listed`, with their guarded
  twins passing; the two residuals `restore-residual-removed` and
  `restore-residual-member`; the two give-up traces
  `restore-finding-private-reuse` and `restore-finding-unseen-agent-reuse`;
  `restore-finding-removed-keeper`, the named wait on the person; and
  `restore-p4-fairness-removed`.
- `cargo run --release -p locust-store --example commit_latency` measures a
  commit that carries a mark beside one that carries none, and both numbers
  stand in the table of the crate comment in
  [lib.rs](../crates/locust-store/src/lib.rs). This phase is not done until
  they do. The estimate, not measured: one more flush, so a commit that signs
  goes from about 3.9 ms to about 7.5 to 8 ms on the machine of that table.
- The drill, in two forms, with two daemons A (host) and B on one computer,
  each on its own home. A lost post: stop A and copy its data directory; start
  A, post one record with A's agent, let B receive it, stop A; move A's data
  directory away and put the copy in its place; start A with B stopped.
  `locust --owner --json goal status` on A shows one `guard` entry, for the
  agent's key, whose reason is `behind`, `"halted": null`, and `restored`; the
  post exits 9 with `read_only`; `goal invite` and admission still work. Start
  B: within two exchanges the entry is gone, the post lands at the next
  position, and neither daemon reports a halt.
- A lost rule change: the same with a `rules bind` in place of the post.
  Status shows `"halted": "signer_recovery"` and one `guard` entry with
  `by_host` true; `rules bind` exits 9 with `read_only`, and so does a post by
  A's agent. After B starts the entry is gone and `rules bind` lands at the
  next position.
- The first form with A's marks directory copied and put back too: every local
  key shows `unheard`, and still does after B has answered twice, because A
  hosts the goal. `locust --owner call goal.continue` with the goal's
  identifier clears it. The same copy and put-back on B, a member's computer:
  every local key shows `unheard`, and it clears with no command after A has
  answered twice.
- Both folders restored into the surviving home with `rsync -a`: the older
  marks file lands in its own directory and still reads as lost, every local
  key shows `unheard`, and the goal A hosts waits for `goal.continue`.
- Delete A's marks directory and restart A: no `guard` entry. Then the same
  with a restore still being caught up: stop A after the first drill's
  put-back, before B has answered, delete A's marks directory and restart A.
  The goal still shows `unheard`, not an ordinary start, and waits for
  `goal.continue`.
- A host alone in its goal signs a waiting step during start, with no peer and
  no command.
- The simulator's seeded runs with `Kind::Restored` pass the new invariant
  where the tests above claim it, and the two kept runs end in a reused
  position.
- The recorded note exists:
  [restore-file-identity-2026-10-06.md](../research/restore-file-identity-2026-10-06.md),
  with its script and full output. Measured on macOS (APFS) and Linux
  (Docker's overlayfs, plus a tmpfs probe): `FileId` of the database and of
  the marks directory stays equal across a stop and start, a checkpoint, a
  `VACUUM` and a rename; every copy tool measured gives the copy a new
  inode number; a data directory replaced by a copy is caught on both
  systems, on overlayfs by the creation time alone; and `cp` over the
  existing path and `rsync --inplace` keep both values and are caught only
  where a mark is ahead. Still missing, and this phase may be built before
  each: a Time Machine restore and a Migration Assistant transfer (both
  need a second machine; by judgement they write restored files as new
  files, the shape every measured copy tool shows, and a restore that kept
  the identity would be the in-place case the marks catch); a reboot (a
  rename is measured to keep both values, and the device number is left out
  because it can change between boots); and a plain Linux disk (the limit a
  file system with no reported creation time and reused inode numbers adds
  is stated in the notes).

**Risks and notes.**
- What the guard can know. With its marks kept it knows exactly the last
  record each key signed, so it knows when its store is behind and when it has
  caught up. It can never know whether a computer that answered sent
  everything, whether a computer it cannot reach or does not know holds more,
  or whether another copy of itself is running.
- What can still fork. (1) Two running copies of one data directory, for
  example an old computer left on after a move. No single daemon can see this.
  (2) A rollback that keeps every file's identity and carries the marks: a
  disk image, a snapshot of a virtual machine or a volume, a tool that writes
  the database and the marks in place. Ordinary where machines are restored
  from snapshots, and a restore that writes both folders in place is the same
  shape on a laptop; for private goals nothing in v2 detects it, and for
  public goals the service check can
  catch it through `guard_attest`, except for records signed after the last
  request the service saw. (3) A database overwritten in place while the marks
  are lost: two faults at once. (4) With the marks kept, a member that is the
  only holder of the host's later records and withholds them keeps the host
  held; if the person then continues, the member can show the records and the
  host is forked. In a goal of a host and one hostile member there is no
  defence: v2 cannot replace a host. (5) An agent's key can still fork in
  four ways, each at the cost of that agent's key in one goal and with the
  goal unharmed. On a member's computer after a copy of unknown age, the
  host's computer never received that agent's last records and another member
  did. The hold ends on hearing the host's computer, and the marks written
  again from the copy do not name those records, so this fork can also come
  later: if the data directory alone is then put back with the marks kept,
  that start finds nothing missing and the agent signs at the same position.
  Holding it would mean waiting for every member's computer after every copy
  of unknown age, which the second table does not ask. A key is admitted
  again and the host's computer lacks its later records. With the marks
  kept, a record reached only a computer removed since: it is given up once
  every other computer has answered, and the
  removed computer brings it back if it is ever admitted again. For the host's
  agent this third way also stops the goal's first files. Or, on a member's
  computer with the marks kept, the copy is older than an admission and the
  host's computer, which held that admission, was itself put back to a copy
  without it: a record that reached only the member admitted since is given
  up once every computer the copy lists has answered. This fourth way is
  accepted as a limit because it needs two separate restores, of two
  computers, from before the same admission, and because no computer that
  answers then knows the admitted member, so nothing could tell the guard
  to wait for it. (6) The person continues
  when they should not. After a whole-computer restore or a move this is the
  only way a host's hold ends, so a release that was a guess is now the
  person's choice. The risky case is a copy older than a change of members:
  every computer the copy lists can have answered while a member admitted
  since holds the later records. That is why the plan of `goal continue` says
  to wait for the computers of the members added most recently. By judgement
  (1), (6) and, on virtual machines, (2) are the likely ones. None was
  measured.
- A crash between the database commit and the sync of the marks leaves a
  record with no mark. `commit` had not returned, so that record never left
  this computer. An ordinary start raises the mark before any exchange, and a
  data directory put back from an older copy loses only a record that nobody
  else holds.
- Storage that reports a sync it did not do can, after a power failure, leave
  a mark ahead of the store or short of it. Ahead: that key is behind after an
  ordinary restart, and because one such mark treats every goal in the file
  as restored, it can also revoke the pending invitations of every goal this
  computer hosts, not only that key's goal. An agent's key is released once
  every other computer has
  answered. A governance key in a shared goal waits for the record to come
  back from a computer that received it and, where none did, for the person's
  `goal continue`. Short: an ordinary start raises the mark, but a data
  directory put back from an older copy before that start passes the guard.
  The guard assumes storage that keeps what it reports as synced.
- The first table rests on file identity, and that is now measured
  ([restore-file-identity-2026-10-06.md](../research/restore-file-identity-2026-10-06.md),
  macOS APFS and Linux overlayfs, with a tmpfs probe). Noticed by the
  identity: every copy tool measured (`cp -R`, `cp -Rp`, `cp -c`, `ditto`,
  `rsync -a` and tar) gives the copy a new inode number, so a data
  directory replaced by a copy reads as another file on both systems.
  `cp -c`, an APFS clone, keeps the creation time exactly and is caught on
  the inode alone. On overlayfs the freed inode numbers were handed to the
  new files at once, and only the changed creation time kept the restore
  visible. A rename keeps both values, so moving the home aside and back on
  one volume is an ordinary start. Caught only because a mark is ahead:
  `cp` over the existing path and `rsync --inplace`, which keep the inode
  and the creation time while the content verifiably rolls back, on both
  systems. Row 1's caveat is the whole defence against them: one mark ahead
  treats every goal in the file as restored, and with the marks lost too
  they are residual 3. Where no goal in the file has a mark ahead, such an
  overwrite passes as an ordinary start. Nothing this daemon signed was
  lost and records it received return by exchange; what silently comes back
  is local: invitations revoked since the copy are pending again, and
  settings changed since are as the copy held them. A false alarm has one
  measured path on APFS: anything that sets an earlier modification time
  lowers the same file's creation time, so `FileId` can change with no copy
  at all, and the start then reads as row 2. A plain limit, measured in
  its two halves: a file system can report no creation time (tmpfs, where
  `created_ms` is `None`) and an allocator can hand a replaced file its old
  number back (overlayfs). A file system with both would make even a
  replaced directory read as the same file, and the marks would be the
  whole defence. On a file system that does not keep file identity across
  a remount (some network and removable-disk formats), every start looks
  like a copy: pending invitations are revoked at each start, and with the
  marks on such a disk too every goal waits. The default home is on the
  system disk, where this does not happen.
- A copy older than the goal itself: the marks know the goal and the store
  does not. This phase does not bring such a goal back; G2 prints one line. A
  member joins again with its original ticket and is then behind until its log
  returns (a redeemed ticket is answered again for the same member on the same
  endpoint; measured in
  `same_key_rejoin_recovers_own_log_before_signing_when_sync_finishes`). A
  host has no way back in this plan: K1 keeps the governance key in the data
  directory from the commit that founds the goal, so such a copy holds neither
  the goal nor its key.
- A copy also brings back older local settings: an agent's level, task
  allowances, connected folders, whether an agent is disconnected and whether
  it has left a goal. An agent disconnected after the copy was taken is
  connected again (measured:
  `revoked_host_cannot_resume_governance_through_grants_or_enrollment_but_old_store_can`;
  `agent_revoke` writes one local record and no goal record). An agent that
  left after the copy is behind until its leave returns. Until E2 it then
  reads as a member here again, because leaving is a local record beside the
  signed request (read: `goal_leave` writes `Local.part`, and
  `Entry::membership` reads only that). From E2 it reads as left again,
  because the standing is read from the signed record. The leave returns only
  from a computer that still exchanges with this one, and after the removal
  that follows a leave none does (E2's notes). The guard does not cover
  them. `GoalStatus.restored` lasts until the first ordinary start with
  nothing held, and G2 prints it.
- Phase 4's rule that a goal's only member needs no approval. K1 found that a
  host restored from a copy older than the second admission could post a
  result that counts unapproved. With the marks kept this is closed: the
  missing admission puts the governance key behind, and the host's agents are
  held with it. What is left is a person who continues from an old copy (6
  above). Where the second member is on the same computer and no other
  computer ever held the goal, the lost admission is lost for everyone and the
  one copy again shows one member; that is by reading, not run.
- An end. E1's first end has no frontier: a record signed before its signer's
  computer learned of the end still counts when it arrives. On the host's
  computer a copy older than the end is behind, the host's agents are held
  with it, and once the end is back nothing is signed there. On a member's
  computer put back from a copy older than the end, a key with no record of
  its own missing is not held: it signs on its old view until the end arrives
  again, and those records count like any late record. The guard does not
  close this. A frontier would, and E1 builds none. Until E1 removes it, a
  close on the goal scope is not an end: it stays in shared decisions, can be
  reopened and stops no work and no admission (measured:
  `goal_close_is_shared_and_publicly_ended_but_does_not_stop_work_or_admission`),
  so the guard treats a closed goal like any other.
- Catching up brings back records, not content keys. After a hold has ended a
  text write can still be refused `unavailable`: when the records that came
  back include a removal, this computer holds the removal and not the content
  key made with it, and it signs nothing that carries text until a member that
  holds the key delivers it (measured:
  `undelivered_removal_key_blocks_survivor_text_but_host_can_write_and_survivor_can_leave`,
  `verified_delivery_of_withheld_removal_key_restores_survivor_text_writes`).
  Records without text, a leave request among them, still sign. If the
  restored host made that key and no member received it, the key is gone, and
  by reading nobody writes text in the goal until a later removal makes a new
  key.
- The guard does not hold the page publisher. Its requests are signed with the
  page's own key and numbered by a counter in the data directory
  (`FarmLocal.seed` and `next_sequence`, read in node/farm.rs); they are not
  records at a position in a goal. After a restore that counter is behind the
  service's. The service answers an identical old request with its stored
  receipt and a different request at a used number with `409` (measured:
  `old_identical_check_in_returns_its_receipt_after_newer_requests_and_restart`).
  While a host catches up, its publisher also reads the highest consent it
  holds for a member, even one whose predecessor is missing or that is
  excluded, so the page can be suspended until the gap fills (measured:
  `later_consent_with_missing_predecessor_suspends_but_replayed_old_consent_does_not`,
  `excluded_consent_arriving_after_removal_suspends_even_when_it_accepts`).
  What the publisher does while its goal is catching up is the joinable
  plan's; `restore_found` is where its line goes.
- A computer that was removed while it was away. Every peer refuses it
  `NotAMember` (measured: 39 of 39 exchanges in ten simulated minutes,
  `offline_removed_member_retries_refused_peers_and_fresh_ticket_returns_stale_membership`),
  and a refused exchange is not hearing. With its data intact nothing is held
  and nothing forks: it signs records nobody takes. After a copy of unknown
  age it stays catching up until its owner continues, and nothing tells it
  why, because a notice to removed computers is not built (END). Given a fresh
  invitation, such a computer answers `joined` from its own stale records and
  the host signs nothing (the same test), so no admission arrives and the
  `Admitted` hold never starts. The same test supports the call-back: a
  computer the other side does not know keeps calling, 19 and 20 times in
  those ten minutes.
- An admission the copy lost is not answered before the guard. If the host
  signed an admission after the copy was taken, the copy still lists that
  ticket as pending and `restore_found` revokes it. A retry on it is refused
  for good, also when the admission later returns from another member. It
  needs a join cut off between the host's signature and the joiner's receipt,
  and then a restore. The joiner is then a member by the records and refused
  on its own computer; the host removes it and invites it again.
- A callback asks the caller for the goal's whole record history, because the
  frontier sent is empty. It is bounded by the transport's limits for a
  connection not yet admitted and by eight callers a goal. Someone who knows a
  goal's id can cause one only while that goal's governance key is held.
- In a goal with constant traffic a hold can outlast several exchanges,
  because only an exchange that brings nothing counts. In a quiet goal the
  first finished exchange with each computer counts: over one quiet hour three
  computers ran 720 exchanges, 120 for each ordered pair, and no stored record
  count changed (measured:
  `idle_three_member_goal_keeps_exchanging_for_a_simulated_hour`).
- If `goal.continue` lowers a mark while the store holds later records past a
  gap, the key still cannot sign: `Goal::next` refuses a log with a gap, as
  today.
- A home written by a build before this phase has no file record, so it starts
  as a copy of unknown age. Use a fresh home, as for every phase.
- The marks reveal goal ids, public keys and counts. The directory is private
  (0700). Uninstalling never deletes it, as it never deletes the data
  directory.
- Between this phase and G2 a held step is passed over by `drive_flow` and is
  not yet listed by Phase 3's `Node::stalled`; `GoalStatus.guard` says why
  until G2 adds the entry.
- Read in the code at `f12eec8`: every signature after a goal's first three
  records passes `next_place`; `Node::open` runs `drive_flow` for every goal
  before the transport exists; a received batch lands through `land`, which
  runs `drive_flow`, so a restored daemon can sign in the middle of an
  exchange; an endpoint the copy does not know is refused `NotAMember` and can
  never return this daemon's own records; `screen` keeps only records by the
  administrator or by keys it admitted, across batches; a frontier lists
  authors in key order; an initiator not authorized before its first frame
  sends an empty frontier; a dialed exchange reads frames at the peer limit
  from its first frame; a member's contact hints are stored only from a
  ticket, so dials to other members already rely on the transport finding an
  endpoint by its id; the shell polls the driver every second; the store
  commits with `synchronous=FULL` and `fullfsync`. Inferred and not run: that
  a call-back exchange behaves like a join from its first frame and finishes
  before the transport closes a connection not yet admitted; everything about
  `FileId` on real file systems; that three rounds of the rejoin test contain
  an exchange that brings nothing.
- Read again at `48bb12c` for the sentences this revision changed: `plan_join`
  returns for a ticket redeemed by the same member at the same endpoint before
  it reaches `author`; `next_place` tests membership and `Local.part`, then
  `Goal::next`, and a forked author gets `unavailable` there; `author` looks
  for the epoch's content key after `next_place`; `goal_leave` signs through
  `author` and writes `Local.part`; `goal_join` answers `Joined` at once when
  its own records say the agent is a member; `GoalSummary` is one per goal and
  local agent and repeats `Entry::halted()`; `Halt::SignerRecovery` and
  `ErrorCode::ReadOnly` are set nowhere; a dialed exchange that did not
  complete backs off from one second to sixty; `Refusal` has six variants; the
  publisher signs with `FarmLocal.seed` and counts with `next_sequence`;
  `snapshot` has eight callers. Since `f12eec8` the production code changed
  only in goal/fold.rs and goal/rules.rs, which this phase does not touch. The
  names taken from K1 and from roles-plan Phases 1 to 6 are plan text, so this
  phase must be read again against the tree it lands on.
- Size, a judgement: 800 to 1,100 lines of production code and about 1,200 of
  tests.

### G2: Catching up in the person's words

**Goal.** A person sees which goal is catching up, why, and whether it waits
for other computers or for them, with the one command that continues. An
agent's tool gets the same facts as data. The override is one command in the
roles plan's grammar. It asks the person who typed it for a yes, with a plan
and a confirmation, because what is signed after it cannot be undone with one
command, which is Phase 2's rule for a command that asks for a yes. The guides
say what a restore does.

**Depends on.** G1. Roles-plan Phases 2 (`cli/confirm.rs`, `cli/only_you.rs`,
the rule for which commands ask the person typing them for a yes), 3
(`Refused`, `Why`, `Node::refuse`, `Attempted`, `Stalled`, `Stall`,
`Node::stalled`), 4 (members' names), 5
(`WaitingForYou`, `WaitingKind`, `render`, `Voice`, `short`, the status view)
and Phase 6 (the guides as rewritten; after Phase 6 a phase owns the documents
it makes stale). K1 for the words: text says host and never names the
governance key. It is the ninth phase of the build order: after G1, before E1,
which tests the override and reads `GoalStatus.guard`, before Phase 7, so the
explanation is tested once, and before Phases 8 and 9, which extend `Stall`.

**Changes.**
- `crates/locust-proto/src/api/level.rs` (new in Phase 3): `Why` gains a fifth
  side, `ThisComputer { hold: GuardView }` (`"side": "this_computer"`).
  `Stall` gains `CatchingUp`. `render` words it. The reason is "the Locust
  data here is older than what this computer signed in the goal" for `Behind`,
  "the Locust data here may be an old copy" for `Unheard` and "admission has
  just arrived; Locust is checking with the host's computer and fetching the
  goal's rules and content key" for `Admitted`;
  the side is "(this computer)". The fix says what the hold waits for. Where
  it waits for a computer, the fix in the person's voice is "It catches up by
  itself. To go on without waiting: LINE", with LINE from new
  `continue_command(goal)`; in the agent's voice it is "It catches up by
  itself, or WHO's owner can continue without waiting." and no command is
  printed. Where only the person ends it, the person's voice reads "It waits
  for you. To continue: LINE" and the agent's "It waits for WHO's owner, who
  can continue with one command." That is an `Unheard` hold whose view has
  `by_host`, and also a `Behind` hold on the governance key whose `waiting`
  is empty: every other computer has answered and none sent the marked
  record. For `Admitted` the fix in both voices is "It ends by itself." and
  no command is printed.
- [authoring.rs](../crates/locust-core/src/node/authoring.rs) and
  [access.rs](../crates/locust-core/src/node/access.rs): `next_place` takes
  the body it is asked for, `next_place(entry, author, body: &Body)`, at its
  three callers. On a hold it answers through Phase 3's `Node::refuse` with
  `Attempted::Sign(body)`, `Why::ThisComputer` and `ErrorCode::ReadOnly`, or
  `unavailable` for `Admitted`; the message is `render(.., Voice::Agent)`, and
  G1's fixed sentences for a hold go. For an agent held because the governance
  key is, the `hold` in the refusal is the governance key's view. For a
  refusal whose key is the governance key, `Refused.agent_name` is `host`, and
  the person's voice prints `You can't ...`. The order inside
  `next_place(entry, author, body)` once every piece has landed: the copy
  holds an end, `conflict(ENDED)` (E1); member and `Local.part`, skipped for
  the governance key (K1); `hold` (G1); `Goal::next`. The act of a body that
  Phase 3's table does not list is: `MemberAdmitted` `Invite`, `MemberRemoved`
  `RemoveMember`, `RulesBound` `ChangeRules`, `TaskRevised` `Revise`, a leave
  request `Leave`, a publication record `Publish`, a role record `GiveRole`,
  any other `PersonCommand`. E1 adds `GoalEnded` to this list, as `Act::End`.
  The `goal.join` handler builds the same refusal with `Act::Join` for the
  joining agent.
- [views.rs](../crates/locust-core/src/node/views.rs) and
  [api.rs](../crates/locust-proto/src/api.rs): `Node::stalled` tests, after
  "is a member" and before "can sign next", that `hold` answers none; failing
  it is `Stall::CatchingUp`. With K1 the whole rule reads: a step whose runner
  is the governance key is signed when this daemon hosts the goal, the guard
  does not hold the key and the key can sign next. Its stalls are
  `RunnerElsewhere`, `CatchingUp` and `Halted`. A step whose runner is a
  member's agent passes Phase 3's four tests and is not held by the guard. Its
  stalls are Phase 3's four and `CatchingUp`. (`RunnerElsewhere` arrives with
  Phase 8.) `WaitingKind` gains `CatchingUp { holds: Vec<GuardView> }`.
  `Node::waiting_for` lists a goal under it only when no computer can end the
  hold as far as this daemon can tell: an `Unheard` hold in a goal this daemon
  hosts, from the start, because only the person ends it; or a `Behind` hold
  on the governance key after every other computer was heard from. Its
  `command` is `continue_command`. A hold that still waits for a computer is
  not listed and prompts no person; the block under the goal names the
  computers it waits for. A
  goal that waits for a member's computer to call (behind, the mark says
  shared, the copy shows nobody) is not listed there: continuing would fork
  it. A `Halted` entry is listed for `Halt::AuthorityConflict` only. From E1,
  a goal whose copy holds an end gets no `CatchingUp` entry (G1's rule for an
  ended goal). `DaemonStatus` gains `lost_goals: u32`, for the owner: the
  goals the marks name and the store does not hold. `Node::open` counts them
  at a start that is not ordinary and keeps the number in memory (`Guard`
  gains `lost: u32`), so the line below lasts until the next ordinary start,
  as the restored line does.
- `crates/locust/src/cli/only_you.rs` (new in Phase 2): one more row, and
  `goal continue` joins the commands that ask the person typing them for a
  yes: what is signed after it cannot be undone with one command. It prints no
  `Undo:` line.

  | Command, after `locust --owner` | Sends | `review` adds | Prints |
  | --- | --- | --- | --- |
  | `goal continue (--goal G \| --all)` | `goal.continue`, once per goal that is catching up | per goal: each held key, its reason, the computers heard from and not | `Continued "T". This computer signs here again.` |

  The plan is the text of mockup G-3. It names the computers not heard from
  with their last-seen time (in `human` only, so the plan id holds no clock
  reading). When the held key is the governance key it says, after the line on
  the missing records, `They may include a removal, a rule change or the end
  of the goal.` and `Wait until the computers of the members you added most
  recently have been on.`, with the reason: a member added after the copy was
  made is not listed, and its computer may hold what the copy lacks. For an
  `Unheard` hold the line on the missing records reads `This copy may be older
  than what this computer signed here, and nothing on this computer can
  tell.` The plan says what a conflict does when the records are the host's,
  and ends "Safe when this is the newest copy of this computer's Locust data
  and no other copy is running." With nothing held it prints `"T" is not
  catching up. Nothing changed.` and exits 0 with no plan. The plan of `goal
  invite` gains the warning `This computer is catching up; nobody is admitted
  until it has.` while `GoalStatus.guard` holds the governance key.

  A command of this file that would sign with a key that `GoalStatus.guard`
  lists computes no plan and prints the refusal with the continue line. `goal
  invite` signs no record, so it keeps its plan and the warning. In a goal
  this computer hosts, a listed governance key stands for every agent's key
  there, because G1 holds them with it. The same holds for the two tiers of
  Phase 2. A command that asks the person typing it for a yes (`goal add`,
  `goal leave`, `member remove`, `rules bind`, `task revise`, `workspace
  init`, `farm on`, `farm off`, `farm consent`) shows no plan it could not
  carry out. From E1, `farm off` in a
  held goal takes E1's delete-only path, which signs nothing, and keeps its
  plan. A command that applies at once and signs (`role give`, `role take`)
  prints the refusal and no `Undo:` line, because nothing changed. `level`,
  `allow` and `invitation revoke` sign nothing and apply as always.
- [presentation.rs](../crates/locust/src/cli/presentation.rs): under a goal's
  heading in `status` and in `goal status`, one block per goal that is
  catching up, as in mockups G-1, G-2 and G-5: the reason; for `Behind` the
  number of missing records and whose they are, `this computer signed as host`
  for the governance key and `NAME signed` for an agent; `Heard from since
  this start:` and `Not yet:` with each computer as the names of the members
  bound to it (`member_label`) and the last-seen time from
  `PeerView.last_sync_ms`; then `To continue without them: ` with the continue
  line. In `status` the block is built from `GoalSummary.guard` and printed
  once per goal. No text names the governance key, and a heading reads `host:
  you` (K1's rule on words). An `Admitted` hold prints no block; under the
  agent it prints `Just admitted: checking with the host's computer and fetching
  the goal's rules.` Under
  any goal with `restored`, the line of mockup G-1 on revoked invitations and
  local settings, whether or not anything is held. An agent whose `halted` is
  `signer_conflict` gets the standing line `NAME can sign nothing more here:
  two of its records conflict. Join with another agent.`, and for the host's
  agent `... Give its roles to another member.` After the goals in `status`,
  when the marks name goals the store does not hold: `This copy of the Locust
  data is older than N goals this computer took part in. A goal you hosted
  cannot be brought back from it. A goal you joined needs its ticket again.`
  `halt` keeps its short sentences for where only a `Halt` is at hand. Every
  block says who or what the hold waits for. A `Behind` hold waits for the
  missing records from another computer in the goal, and the computers not
  heard from stand under `Not yet:`. When every other computer has answered
  and the governance key is still behind, the block adds `No computer that
  answered sent them. Waiting for you.` One that answered can still hold
  them: a computer removed since finishes its exchanges and sends nothing
  (G1's notes). An `Unheard` hold in a goal this
  computer hosts reads `Waiting for you: only you can say this is the newest
  copy of this computer's data.` and its last line reads `To continue: ` with
  the continue line. An `Unheard` hold on a member's computer reads `Waiting
  to hear from the host's computer (NAME), last seen T. Nothing is needed from
  you.`
- [doctor.rs](../crates/locust/src/cli/doctor.rs): one more check, `marks`:
  its detail is the directory's path; it fails with a recovery sentence when
  the directory cannot be created or written.
- [mcp.rs](../crates/locust/src/mcp.rs),
  [SKILL.md](../skills/locust/SKILL.md), [sharing.md](guide/sharing.md): the
  same short text in each. `read_only` means this computer is catching up
  after its data was restored. The refusal says what the hold waits for:
  other computers, and then it ends by itself, or your owner. Do not retry in
  a loop; work in other goals and read again later. `unavailable` keeps Phase
  5's instruction, read again and retry, which is right for a key that was
  just admitted. [concepts.md](guide/concepts.md): the "Only you" section
  that Phase 6 wrote lists `goal continue` among the commands that ask you
  for a yes.
- [operations.md](guide/operations.md): the Backups section loses "Restoring a
  copy is untested." It says: stop the daemon before copying; copy the data
  directory and not the marks directory beside it; a copy of the data
  directory holds the keys of every goal this computer hosts and of every
  agent on it; whoever starts a daemon on a copy can act as you in those
  goals; a copy put back is older than what this computer signed, Locust
  notices and catches up from the other computers by itself; pending
  invitations are revoked, and levels, allowed tasks, connected folders, and
  which agents are disconnected or have left a goal are as they were in the
  copy; after a whole-computer restore or a move Locust cannot tell how old
  the copy is, so a goal you host waits for your `goal continue`, and a goal
  hosted on another computer waits to hear from that goal's other computers;
  a copy older than a goal you host loses that goal; never run two copies.
  Phase 6 leaves this section to G2.
- `python3 scripts/check_formations.py --write` regenerates the runtime
  contract; `refusal_schema` gains the fifth side.

**Tests.**
- `this_computer_reads_the_same_facts_in_both_voices` in level.rs: the
  person's voice prints the continue line; the agent's prints none and quotes
  nothing a member wrote; a refused host command reads `You can't` and names
  no key; an unheard hold in a hosted goal reads `It waits for you` to the
  person and names the owner to the agent; an `admitted` hold prints no
  command in either voice.
- `status_shows_who_a_goal_catching_up_waits_for_and_the_continue_line` (one
  block for each thing a hold waits for: another computer, the host's
  computer, the person),
  `a_hosted_goal_of_unknown_age_is_listed_under_waiting_for_you` (from the
  start, and still after every other computer has answered) and
  `a_goal_waiting_for_a_call_is_not_listed_under_waiting_for_you` in
  presentation.rs. Beside them:
  `the_hosts_hold_is_printed_once_per_goal_from_the_summaries`,
  `a_just_admitted_agent_gets_one_line_and_no_block`,
  `an_agent_with_conflicting_records_reads_its_own_conflict` and
  `status_says_when_this_copy_is_older_than_goals_it_took_part_in`.
- `goal_continue_shows_its_plan_and_needs_a_goal_or_all`,
  `the_invite_plan_warns_while_the_host_is_catching_up` and
  `a_signing_command_in_a_goal_that_is_catching_up_shows_no_plan` in
  [cli.rs](../crates/locust/tests/cli.rs). The last runs `rules bind` and
  `role give` against a status whose `guard` lists the governance key: neither
  computes a plan, both print the refusal with the continue line, and `role
  give` prints no `Undo:` line.
- `doctor_names_the_marks_directory` in `cli/doctor/tests.rs`.
- Rewritten: Phase 3's `goal_status_reports_stalled_effects` gains a step held
  by the guard; `every_printed_command_parses_as_printed` gains the continue
  line.

**Exit criteria.**
- The three cargo commands, the four site commands, `python3
  scripts/check_formations.py` and `scripts/check_docs.py` pass. The
  executable recipes and the helper tests pass as CI runs them: `python3
  scripts/check_documentation.py --binary target/debug/locust --timeout 60`
  and `python3 -m unittest discover -s scripts/tests`.
- After the second form of G1's drill, the lost rule change, `locust --owner
  status` on A prints the block of mockup G-1 with B's members named under
  `Not yet:`, and nothing under "Waiting for you". `locust --owner rules bind`
  on A prints `You can't ...` with the continue line and no plan. After B
  answers, the block is gone and the restored line stays. After the first form
  the block names A's agent and its one missing record.
- On a host whose data and marks were both copied back, `status` lists the
  goal under "Waiting for you" with the continue line from the start, alone
  and also after B has answered. On B after the same, the block reads `Waiting
  to hear from the host's computer` and nothing is listed under "Waiting for
  you". `locust --owner goal continue --goal T` shows its plan and proceeds on
  yes; `--confirm` with a stale id exits with `conflict: the plan changed; run
  --plan again`.
- An agent's `locust-cli` post in a goal that is catching up exits 9 with
  `read_only` and the agent-voice sentence; its tool result carries `"side":
  "this_computer"`.
- `git grep -n "Restoring a copy is untested" -- docs` finds nothing.

**Risks and notes.**
- The state is "catching up" and the command is `goal continue`. The roles
  plan already uses "resume" for taking an attempt over (`Act::Resume`) and
  says a closed task is paused and resumed, so neither word is used here.
- The continue line is printed under every goal that is catching up, but a
  goal is listed under "Waiting for you" only when no computer can end the
  hold: a goal this computer hosts after a copy of unknown age, and a goal
  whose missing host records no computer that answered sent back, whether
  because none holds them or because the one that does was removed since and
  sends nothing. A person whose
  computer is catching up by itself is not asked to do anything. The limit: a
  hold that waits for a computer that never answers is not listed. It lasts
  until that computer answers or the person continues, and status names the
  computer and when it was last seen.
- After a whole-computer restore or a move every goal this computer hosts
  waits for the person's `goal continue`, and `goal continue --all` covers
  them with one yes. That is answer 13 applied to every hosted goal: hearing
  from the computers an old copy lists cannot show what the host's computer
  signed last (G1).
- No text names the governance key (K1's rule on words). A record it signed
  prints `host`, missing records read "signed as host", and a refused host
  command reads `You can't ...`.
- The plan of `goal continue` names the end of the goal. No command makes an
  end until E1, the next phase. Nothing is released between the two, so the
  sentence is written once.
- A newly admitted agent reads `unavailable`, not `read_only`, and gets one
  line and no continue command. `goal continue` still releases it, and `goal
  status` lists the hold.
- A computer that was removed while it was away and then restored shows its
  goal catching up with every computer under `Not yet:`. The block cannot say
  why (G1's notes).
- A joiner whose host answers `catching_up` keeps today's sentence for a
  joining agent until the joinable plan stores a join's last refusal; that
  plan owns the field and takes this sentence: "The host's computer is
  catching up and admits nobody yet. Locust asks the host's computer again by
  itself."
- The names taken from K1 and roles-plan Phases 2 to 6 are plan text, not
  code. This phase must be read again against the tree it lands on.
- Size, a judgement: 300 to 400 lines of production code and about 400 of
  tests.

### E1: The host ends a goal

**Goal.** The host can end a goal with one command, which first asks the
person typing it for a yes. The end is one signed governance record, signed
by the governance key. On every computer that holds it, nothing new is signed
in that goal: no task, result, approval or file change, no change of members,
rules or roles, no admission on a ticket and no automatic step. Reading,
syncing what was signed earlier and taking the
public page down go on. Nothing is deleted on any computer. The public page
reads ended whenever the host's copy holds an end, by the function the signing
gate reads. The close on the goal scope is removed. Today it is a
decision of the goal's finish role, which the default formation does not have.
It is kept in the shared decisions and can be reopened, it stops no work and
no admission, and the page is the one reader that calls the goal "ended"
(measured on today's code,
[note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 6:
`goal_close_is_shared_and_publicly_ended_but_does_not_stop_work_or_admission`
and `default_formation_has_no_goal_finish_decider` in
[farm_characterization.rs](../crates/locust-core/src/node/tests/farm_characterization.rs)).
A record signed before a computer learned of the end still counts when it
arrives.

**Depends on.** Roles-plan Phases 1 to 6, K1, G1 and G2. In the build order
this phase follows G2 and comes before E2 and roles-plan Phases 7 to 10. From
the roles plan: `Audience::Host`, `Node::host()`, `Node::hosts()` and
`invitation.revoke` with no id (Phase 1); `cli/confirm.rs`,
`cli/only_you.rs`, the `--owner` grammar and the rule for which commands ask
the person typing them for a yes before they act, called the commands that
ask below (Phase 2); `Refused`, `Why::State`, `Act`, the check an agent's
request passes before it signs, and `Node::stalled` (Phase 3); member names
and `member_label` (Phase 4); the status view, `render`, `short` and
`WaitingForYou` (Phase 5); the documents rewritten (Phase 6), after which a
phase owns the documents it makes stale. From K1: the governance key, which
signs the end record and which `Node::host()` returns; `State.governance` and
`State.host`; `Node::key_for`; `next_place` without the member test for that
key; `hosted_here` and `by_host`; and the rule that a fork of that key's log
is judged by position, as the administrator's is today. From G1: `Node::hold`
inside `next_place`, so that a host started from an older copy recovers its
own later records, the end among them, before it or any agent on its computer
signs; `revoke_pending`; `GoalStatus.guard`. From G2: `next_place(entry,
author, body)` and its list of bodies and acts; the rule that a signing
command in a goal that is catching up shows no plan; and `goal continue`, the
override one test signs through. Roles-plan Phases 8 and 9 land after this
phase; a note below says what Phase 8 adds. The model cases named under "Exit
criteria" are written before the chain rule is built, on the founding
transcript the model has by then
([Models written first](#models-written-first)).

**Changes.** Five terms, used the same way below. The *end record* is the new
governance record `GoalEnded`, signed by the governance key, with no payload.
A copy *holds an end* when the governance key's log, as that copy holds it,
has an end record. The end is *in force* on a copy when its governance chain
reaches the end record; the copy is then *ended*. A copy is *ended, then
halted* when it holds an end that a fork of the governance key's log, at or
before the end's position, cut out of the chain. A copy that holds an end
while a governance record before it has not arrived reads as ended, and the
end comes into force when that record arrives. Each of these is a function of
the signed events held and of nothing else: no clock, arrival order or local
flag enters.
- [event.rs](../crates/locust-proto/src/event.rs): new last variant
  `Body::GoalEnded`, with no fields. `kind()` gains the arm `goal_ended`, and
  the variant joins `is_governance()`. `context()` and `dependencies()`
  already answer nothing for it through their wildcard arms. Its index is 26,
  after Phase 4's `RoleHolders` at 25. The record has no frontier, no note and
  no payload. With no payload it needs no content key. The key is looked up
  only for a record that carries text (read in
  [authoring.rs](../crates/locust-core/src/node/authoring.rs); measured for a
  leave request, which carries none,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 9).
  So a computer that lacks the key of the current epoch still reads the end.
- [testkit.rs](../crates/locust-proto/src/testkit.rs) and
  [vectors.rs](../crates/locust-proto/src/vectors.rs): `every_body()` gains
  the body. `transcript()` gains one end record as its last event, with one
  new id constant and one new signature constant.
  `body_indices_and_bytes_are_current_contract` gains the row `goal_ended`,
  and `BODY_DIGEST` is regenerated. No constant of an earlier event changes
  because of this record.
- [standing.rs](../crates/locust-core/src/goal/standing.rs): new
  `Exclusion::AfterEnd` (`"after_end"`). `Evaluation` gains `cut_end:
  Option<EventId>`. [state.rs](../crates/locust-core/src/goal/state.rs):
  `State` gains `ended: Option<EventId>`, the end record in force.
- [chain.rs](../crates/locust-core/src/goal/chain.rs): `Chain::build` reads
  governance from the usable prefix of the governance key's log, as today
  (`log.slots[..log.usable]`; that key is `history.administrator` in today's
  code and `history.governance` on the tree this phase starts from). New arm
  for `Body::GoalEnded`. With a payload it is `Excluded(Precondition("an end
  record carries no payload"))`: it keeps its position, like any excluded
  governance record, and ends nothing. Otherwise it is effective, takes its
  position and snapshot, sets `chain.state.ended`, and the loop stops there.
  This arm carries the comment `PLUG host-replacement: takeover versus end`.
  After the loop:
  - A fork of that log is reported as `Halt::Fork` only when `state.ended` is
    unset, because nothing above the end counts.
  - When `state.ended` is unset, new `Chain.cut_end` is the first record of
    that log, in the log's own order (position, then identifier), whose body
    is `GoalEnded` and that carries no payload. One exists when a fork at or
    before the end's position cut the end out of the prefix, and while a
    record before the end has not arrived. The order only picks which
    identifier is shown. The gate asks only whether one exists.
  - In `authorize_base`, a governance record of that key that the loop gave no
    standing is `Excluded(AfterEnd)` when `state.ended` is set, and
    `Excluded(AfterHalt)` otherwise, as today. Such a record has no position
    and no snapshot.
  - In `authorize_base`, while `state.ended` is set, a record that is not
    governance is `Excluded(AfterEnd)` in two cases. First, when its anchor,
    or the anchor of a record its ancestry walk visits, is the end record or a
    held governance record of that key with no place in the chain. It is
    excluded, never pending, and adds no missing dependency. Second, when its
    author is the governance key and its position in that key's log is above
    the end's. Every other record is judged as today, so one anchored before
    the end is effective whenever it arrives.
  - The order, as the master plan gives it: `authorize_base` for a record that
    is not governance: E1's two `AfterEnd` tests, then K1's branch for the
    governance key, then today's tenure path.
- [fold.rs](../crates/locust-core/src/goal/fold.rs): `Verifier::check` already
  returns for governance bodies other than `TaskRevised` and `WorkspaceEpoch`;
  `GoalEnded` joins the bodies of its closing `unreachable!()` arm.
  `Verifier::decision` refuses a `Close` or `Reopen` at `Scope::Goal` with
  `invalid("a goal is ended by its host's end record; a close on the goal
  scope is invalid")`, beside the test it has for the workspace scope.
  `evaluate` copies `chain.cut_end` into the `Evaluation`.
  [goal/flow.rs](../crates/locust-core/src/goal/flow.rs): `desired_effects`
  returns nothing when the copy holds an end, so nothing is called for and
  nothing shows as stalled.
- [goal/mod.rs](../crates/locust-core/src/goal/mod.rs): new
  `Goal::end_held(&self) -> Option<EventId>`, `state().ended` or else
  `evaluation().cut_end`. `Goal::next` returns `None` for every author when
  `end_held()` is set; this is the one signing gate. `Goal::can_start` is
  false then. New `pub const ENDED: &str = "the host ended this goal"`, the
  one sentence every refusal below uses. The check an agent's request passes
  before it signs (roles-plan Phase 3) tests the end first, for every body,
  before it tries the candidate record and before the level check, and
  answers `Why::State { reason: ENDED }`. A close or reopen at goal scope
  does not reach that check: `Request::check` refuses the request (below).
- [authoring.rs](../crates/locust-core/src/node/authoring.rs):
  `next_place(entry, author, body)`, in order: the copy holds an end,
  `conflict(ENDED)` (E1); member and `Local.part`, skipped for the governance
  key (K1); `hold` (G1); `Goal::next`. So `next_place` answers
  `conflict(ENDED)` first, before its membership test, when the entry holds an
  end. Its other sentence, about history that has not arrived, stays for that
  case. The module comment says "halted or ended". Every signature in a held
  goal passes `next_place`; only `goal_create`, which founds a new goal, signs
  without it (read in
  [goals.rs](../crates/locust-core/src/node/requests/goals.rs),
  [claims.rs](../crates/locust-core/src/node/requests/claims.rs),
  [peers.rs](../crates/locust-core/src/node/peers.rs) and
  [node/flow.rs](../crates/locust-core/src/node/flow.rs)).
- [api.rs](../crates/locust-proto/src/api.rs): new `Request::GoalEnd { goal }`
  with the row `("goal.end", false, true, Host, false, "Ends the goal for
  every member. It cannot be undone.")`, its arms in `goal()` and
  `is_answered_by`, and `Response::GoalEnded { end: EventId,
  invitations_revoked: u32 }`. The request names no expected head. The host's
  computer signs admissions and removals with nobody present, each of them
  moves the head, and none changes what an end does. A head the plan showed
  would only make the one command that must work fail and be typed again.
  `GoalStatus`, `GoalSummary` and `PendingWork` each gain `ended:
  Option<EventId>`, the end this copy holds (`end_held()`); with `halted` it
  tells apart not ended, ended, halted, and ended then halted, which is
  `ended` set and `halted` equal to `authority_conflict`. `Request::check`
  answers `invalid` for `scope.close` and `scope.reopen` with `Scope::Goal`:
  "a goal is ended by its host: locust --owner goal end". The summaries of
  those two operations say they close and reopen a task. In `api/level.rs`
  (roles-plan Phase 3) `Act` gains `End`; `callers::resolve` maps `goal.end`
  to it, and `render` (Phase 5) reads it as "end "GOAL"". G2's list of bodies
  gains `GoalEnded`, which reads `Act::End`.
- [goals.rs](../crates/locust-core/src/node/requests/goals.rs) and the
  dispatch in
  [requests/mod.rs](../crates/locust-core/src/node/requests/mod.rs): new
  `goal_end(actor, goal, now_ms)`. In order: `host()`; the entry holds an
  end, then `conflict(ENDED)`; the goal is halted, then `ErrorCode::Halted`
  with "this goal is halted; the host can sign nothing more for it, the end
  included"; the governance key is held by the restore guard, then GUARD's
  `read_only` refusal, which `next_place` gives. The handler compares no
  head: it asks `next_place` for the governance key and signs at the place it
  answers with `sign_at`, as `member_remove` does today (read in this file).
  It signs `Body::GoalEnded` with the governance key, which `host()` returns
  (K1), and no text, and `revoke_pending` (next bullet) runs on the same `Tx`.
  It answers `GoalEnded`. `goal_status` fills `ended`.
- [invitations.rs](../crates/locust-core/src/node/requests/invitations.rs):
  `goal_end` calls `revoke_pending` (from G1) on the same `Tx`. `goal_invite`
  refuses with `conflict(ENDED)` when the entry holds an end, beside its test
  for a halt; it signs a ticket and not an event, so the gate does not reach
  it. `goal_join`, on the joiner's daemon, answers `conflict("this goal has
  ended; nobody can join it")` for an agent that is not already a member when
  the daemon holds the goal with an end.
  [peers.rs](../crates/locust-core/src/node/peers.rs) is not changed:
  `plan_join` refuses a revoked invitation and signs through `author`, which
  now fails, so a ticket presented after the end gets
  `Refusal::InvitationRefused`, and `exchange_ended` already stops the
  joiner's retries on that refusal. A retry of an admission that was committed
  before the end is answered as today. `plan_join` answers a redeemed
  invitation, presented again by the same key from the same endpoint, before
  it reaches `author`. That is before the ended test and before G1's
  `admission_hold`. `revoke_pending` revokes only pending invitations, so a
  redeemed one keeps that answer after the end (read in peers.rs and
  invitations.rs).
- [node/farm.rs](../crates/locust-core/src/node/farm.rs): in `project`, the
  block that looks up the closure decisions of the goal scope is replaced by
  one test: `entry.goal.end_held()` set gives `FarmGoalState::Ended`. The page
  then reads what the signing gate and the terminal read, also while the end
  waits for an earlier record. Nothing sets `Disputed` any more, and nothing
  ever showed it: `project` fails in `eligible` before it reaches that test,
  and the service refuses such an upload (read in node/farm.rs and in
  `mutate_inner`). The change list reads an end as
  `(FarmChangeKind::Closure, "The host ended the goal")`. K1 keeps the
  governance key's records in that list, so the line always shows, with no
  agent beside it; the page's state does not depend on it. In `farm_request`,
  `FarmOff` on an entry that holds an end, or whose governance key the restore
  guard holds, sets the delete as today and signs no `PublicationSet`. It asks
  only `readable` and `hosts(entry)`, as `invitation.revoke` does in
  roles-plan Phase 1. Today the handler takes the signer first and then signs.
  `FarmOn` and `FarmConsent` fail at `next_place` with `ENDED`. Check-ins
  already stop for a snapshot that is not `Open`; an upload still goes out
  when the snapshot changes. A page that cannot show its board says ended
  too. A suspend carries `ended` when the copy holds an end. A copy that is
  ended, then halted still fails `eligible` on the halt and suspends the
  page, as today, and that suspend says ended. A page that was already blank
  when the end arrived is sent one suspend that says so. The page's local
  record gains one flag for that: set when the service acknowledges a suspend
  that said ended, cleared when it acknowledges an upload (`farm_poll_local`
  and `farm_complete_local`).
- [proto farm.rs](../crates/locust-proto/src/farm.rs): `FarmGoalState` loses
  `Disputed`. `FarmControlBody` gains `ended: bool`, left out when false, so a
  body with nothing to say stays `{}` and a stored receipt still matches.
  `FarmServiceView` gains `removal_ms: Option<u64>`, the time the service will
  remove the page. [locust-farm lib.rs](../crates/locust-farm/src/lib.rs):
  `mutate_inner` refuses only `Unavailable`. A suspend that says ended sets
  the page's stored end time (`closed_at`) when it is unset. `ended` on a
  check-in or a delete is refused, so the flag has one meaning. `current` and
  `unavailable` answer `removal_ms`, the stored end time plus `retention_ms`,
  for a page that is shown or blank. `run_expire`, the gallery query and the
  upload arm are not changed: the service already keeps the end time across a
  suspend and removes every page past its period, shown or blank (read in
  `mutate_inner` and `run_expire`; test
  `suspended_ended_farm_expires_at_original_deadline`). The service reads one
  flag in a request signed with the page's key, as it reads `goal_state` in a
  snapshot today. It judges no signed history. So the 30 days of answer 23
  start when the service first accepts a request that says the goal ended: an
  upload whose snapshot reads ended, or a suspend that says so. That is the
  service's clock, and no computer has to agree on it.
- [views.rs](../crates/locust-core/src/node/views.rs): `goal_summaries` fills
  `ended`. `pending_work_with_news` returns every list empty for a goal that
  holds an end and sets `PendingWork.ended`; `pending`, `wait` and the context
  views all read it. `waiting_for` (roles-plan Phase 5) lists no `AllowTask`
  and no `Halted` entry for such a goal. G1's rule for a goal that holds an
  end takes effect in this phase, because `Goal::end_held` exists only from
  here: `Node::halt` answers no `signer_recovery` for such a goal,
  `GoalStatus.guard` and `GoalSummary.guard` are empty, and `waiting_for`
  lists no `CatchingUp` entry. `Node::hold` and the stored records are
  unchanged.
- `crates/locust/src/cli/only_you.rs` (roles-plan Phase 2): new `locust
  --owner goal end --goal G`, among the commands that ask the person typing
  them for a yes;
  [args.rs](../crates/locust/src/cli/args.rs) attaches it and skips the
  generated `goal end`. Its `review` adds the members' keys, the ids of
  pending invitations and the page's `desired`, and it sends `goal.end` with
  the goal alone. An admission on a ticket, a removal that follows a leave
  and an invitation that runs out all happen with nobody present. Between
  `--plan` and `--confirm` each of them changes one of those fields, so the
  plan id no longer matches, the command answers `conflict: the plan changed;
  run --plan again`, and the person runs it again. The daemon itself compares
  no head, so nothing the host's computer signs after the yes refuses the
  end. The plan and result are text E1-1. On a goal whose
  status has `ended`, `goal end` computes no plan, prints `"T" has already
  ended.`, sends nothing and exits 0, with or without `--confirm`. Every other
  command of this file that would sign in such a goal computes no plan and
  prints `conflict: the host ended "T"; nothing new is recorded in it`. That
  covers both tiers of the roles plan's confirmation rule. A command that asks
  shows no plan. A command that applies at once and would sign (`role give`,
  `role take`) sends nothing and prints no `Undo:` line, because nothing
  changed. `level`, `allow` and `invitation revoke` sign nothing and work as
  before. The commands in other files that ask and would sign follow the same
  rule: `workspace init`, which shares the first files
  ([cli/workspace.rs](../crates/locust/src/cli/workspace.rs)), and `farm on`
  and `farm consent` ([cli/farm.rs](../crates/locust/src/cli/farm.rs)). `goal
  end` itself asks, so it prints no `Undo:` line. On a goal that is catching
  up G2's rule comes first for `goal end`, as for every signing command: no
  plan, and G2's refusal with the continue line.
- [presentation.rs](../crates/locust/src/cli/presentation.rs): the goal
  heading of `status` ends `· ended by the host`, or `· ended by you` where
  this daemon hosts the goal, and adds `, then halted` when `halted` is
  `authority_conflict`. `Voice::Agent` reads "NAME's owner" for "you", as
  elsewhere. After the goal's agents one sentence stands in place of every
  agent's level and standing line (text E1-2). `goal status` prints the same
  fact as its second line with the first eight characters of the end record.
  `pending` prints `The host ended this goal. Nothing is pending.`
  [cli/farm.rs](../crates/locust/src/cli/farm.rs): the `farm off` plan and
  result on an ended goal are text E1-4; on a goal that is catching up the
  plan gives that as its reason.
- The site: in [model.ts](../sites/locust.farm/src/lib/farm/model.ts)
  `modeName('ended')` is `Ended by the host`. `FarmGoalState` in `types.ts`
  beside it loses `'disputed'`, and the farm page
  (`routes/farm/[id]/+page.svelte`) loses its "Goal disputed" branch.
  `FarmState` in `client.ts`, the site's type for the service's answer, gains
  `removal_ms`. `farmMode` answers `ended` for a state with no snapshot and
  `removal_ms` set, and the blank branch of the farm page then prints the
  heading `Ended by the host` over `The host ended this goal. Nothing else is
  shown here.` A blank page with no `removal_ms` reads `Farm unavailable`, as
  today. The
  legend of the farms page (`routes/farms/+page.svelte`) reads `Ended: the
  host ended the goal.` in place of `Ended: the goal was closed.`
- [check_farm.py](../scripts/check_farm.py): the two steps that close and
  reopen the goal scope go. After its last consent step the script ends the
  goal with `goal end`, waits for the page to read `ended`, and its `farm off`
  then takes the delete-only path. `finish_farm` in
  [live_farm.py](../scripts/client_qualification/live_farm.py) closes the goal
  scope today so that a demo's page reads ended. It ends the goal with `goal
  end` under the owner's credential, the plan and then `--confirm`, and
  `test_finish_waits_for_new_receipt_not_old_success` in
  [test_live_farm.py](../scripts/tests/test_live_farm.py) counts its calls
  again. The prompt in [live_farm_demo.py](../scripts/live_farm_demo.py) loses
  the words "or close", because no agent can end a goal. These are the
  only callers of a goal-scope close outside the crates (searched at
  `541f29b`), so this phase leaves no recipe and no script failing.
- Documents this phase owns: [status.md](status.md) loses "Closing the whole
  goal"; [guide/formations.md](guide/formations.md) loses "Closing the whole
  goal is recorded but has no effect."; [guide/sharing.md](guide/sharing.md)
  gains a section on ending, built from the explanation in this plan;
  [guide/farm-publication.md](guide/farm-publication.md) says what an ended
  page shows, the blank page included, and that the farm service removes it
  30 days after it is told of the end;
  [swarm-visualization-plan.md](swarm-visualization-plan.md) says "an end
  record" where it says "explicit closure";
  [SKILL.md](../skills/locust/SKILL.md) gains one instruction: when a goal has
  ended, stop working in it and tell your owner. `python3
  scripts/check_formations.py --write` regenerates `runtime.contract.json` and
  `farm.schema.json` under `docs/reference/generated/`.
- [Organization.tla](../research/tla/Organization.tla), its configs under
  `research/tla/configs/`, [cases.json](../research/tla/cases.json) and
  [organization.md](../research/tla/organization.md): the governance kind
  `end`, six cases and one witness, written first.

**Tests.**
- [goal/tests.rs](../crates/locust-core/src/goal/tests.rs), signed replays,
  each forward, reversed and after a reload:
  - `an_end_marks_the_goal_ended_and_later_governance_is_excluded`: an
    admission and a rules binding signed after the end are `AfterEnd`, members
    and rules are those before it, and `next` is `None` for the host and for a
    member.
  - `work_signed_before_the_end_counts_whenever_it_arrives`: a result and
    another member's approval anchored before the end arrive after it, both
    are effective and the result counts.
  - `a_record_anchored_at_the_end_or_signed_by_its_key_above_it_is_excluded`:
    both kinds are `AfterEnd`, and neither is pending or listed as missing.
  - `a_fork_at_or_before_the_end_cuts_it_and_the_copy_reads_ended_then_halted`:
    modelled on K1's
    `a_fork_in_the_governance_log_retracts_later_governance_and_preserves_prefix_work`;
    `state().ended` is unset, the halt is a fork, `end_held()` is set, `next`
    is `None` for every key and earlier work stays effective.
  - `a_fork_above_the_end_changes_nothing`: the copy stays ended and is not
    halted.
  - `an_end_behind_a_missing_record_is_held_and_not_in_force`: with the
    governance record before the end withheld, `end_held()` is set,
    `state().ended` is not, and `next` is `None` for a member; the end is in
    force once the record arrives.
  - `an_end_with_a_payload_or_from_another_key_ends_nothing`: the first is
    excluded on its content and the second as not the host's.
  - `a_close_on_the_goal_scope_is_invalid_and_a_task_still_closes`: the
    goal-scope decision is excluded and a task close is effective.
  - `an_ended_goal_calls_for_no_automatic_step`: on the `pipeline()` fixture,
    `desired_effects` is empty once the end is held.
- New `crates/locust-core/src/node/tests/ending.rs`, declared in
  [tests/mod.rs](../crates/locust-core/src/node/tests/mod.rs):
  - `goal_end_signs_one_record_and_revokes_pending_invitations_in_one_commit`:
    one event and the invitation writes land together, and a redeemed or
    expired invitation is untouched.
  - `goal_end_needs_the_host_and_a_goal_that_is_not_halted_or_held`: a
    member's daemon is `denied`, a halted goal is `halted`, a host whose
    governance key the restore guard holds is `read_only`, and a second end is
    `conflict`.
  - `goal_end_follows_an_admission_the_hosts_daemon_signed_meanwhile`: a
    joiner is admitted on a pending ticket after the status the command read;
    the end is signed at the next position, and the redeemed invitation is
    not counted as revoked.
  - `after_the_end_every_signing_request_is_refused_as_the_goals_state`: post,
    review, start, open a task, bind rules, remove, invite, leave, `farm.on`
    and `farm.consent` are all `conflict` with `ENDED`, while board, events,
    context and document reads answer.
  - `an_ended_goal_lists_no_pending_work_and_nothing_waits`: the pending lists
    and `waiting` are empty and `PendingWork.ended` is set.
  - `the_end_reaches_a_member_by_ordinary_sync_and_stops_its_signing`: on
    `Network` from
    [delivery.rs](../crates/locust-core/src/node/tests/delivery.rs), the
    member's daemon holds the end after one round and its agent's post is
    refused.
  - `a_ticket_presented_after_the_end_is_refused_and_the_joiner_stops`: the
    join gets `InvitationRefused` and the joiner's record is marked refused.
  - `a_retry_of_an_admission_committed_before_the_end_is_answered_and_signs_nothing`:
    a joiner admitted before the end presents its request again after it; the
    answer is the admission already made, no record is signed, and the
    joiner's daemon holds the end after its next exchange.
  - `a_record_signed_before_a_member_learned_of_the_end_still_counts`: a
    member offline at the end posts, then syncs, and both daemons hold the
    post as effective.
  - `a_host_restored_from_before_the_end_signs_nothing_once_it_recovers_it`:
    modelled on G1's
    `a_restored_hosts_agent_is_held_until_its_own_records_return_and_then_extends`;
    before recovery the host's agent's post is `read_only`; after recovery the
    gate is shut and there is no fork.
  - `a_restored_host_that_signs_before_recovery_cuts_the_end_on_both_computers`:
    the same test's other ordering, signed through GUARD's override; the
    member's daemon reads ended, then halted. After one exchange in each
    direction the host's daemon holds the end record and reads the same, and
    neither signs.
  - `an_ended_goal_is_not_shown_as_catching_up`: on a copy of unknown age that
    holds an end, `halted` and `guard` are empty and nothing waits, while
    `Node::hold` still answers.
- [tests/farm.rs](../crates/locust-core/src/node/tests/farm.rs):
  `the_page_reads_ended_from_the_end_record_and_check_ins_stop`;
  `the_page_reads_ended_while_the_end_waits_for_an_earlier_record`;
  `a_page_that_cannot_show_its_board_is_told_the_goal_ended_once` (a fork at
  or before the end, and a page that was blank before the end; no second
  suspend after a restart);
  `farm_off_after_the_end_sends_the_delete_and_signs_nothing`;
  `farm_off_while_the_host_is_catching_up_sends_the_delete_and_signs_nothing`;
  `a_decline_signed_before_the_end_still_blanks_the_page_when_it_arrives`,
  which also asserts that the suspend says ended;
  `a_late_record_updates_an_ended_page_and_keeps_it_ended`. Each asserts what
  its name says on the publisher's next request, with receipts the harness
  supplies. K1's `publication_needs_no_consent_from_the_governance_key` gains
  an end record.
- In the farm service ([lib.rs](../crates/locust-farm/src/lib.rs)):
  `a_suspend_that_says_ended_starts_retention_and_a_plain_one_does_not`;
  `a_blank_ended_farm_answers_its_removal_time_and_is_removed_after_retention`;
  `an_ended_request_at_a_used_number_is_refused_and_changes_nothing`, which
  pins what a host whose data went back cannot do.
  `suspended_ended_farm_expires_at_original_deadline` passes unedited. In
  proto farm.rs: `a_control_body_that_says_nothing_is_the_empty_object`.
- Existing tests in
  [farm_characterization.rs](../crates/locust-core/src/node/tests/farm_characterization.rs)
  that pin the goal-scope close.
  `goal_close_is_shared_and_publicly_ended_but_does_not_stop_work_or_admission`
  is deleted; the page test above takes its place.
  `default_formation_has_no_goal_finish_decider` and
  `goal_close_follows_finish_role_instead_of_host_identity` are adapted to
  close a task, where the same two rules still hold. The three consent tests
  of that file stay.
- [api.rs](../crates/locust-proto/src/api.rs):
  `goal_end_is_the_hosts_and_never_a_tool`;
  `a_close_or_reopen_of_the_goal_scope_is_invalid`.
  `composed_scope_is_json_and_task_identity_remains_explicit` in
  [args.rs](../crates/locust/src/cli/args.rs) is the existing parse test,
  kept; it already names a task.
- [cli.rs](../crates/locust/tests/cli.rs):
  `goal_end_binds_to_the_members_invitations_and_page_its_plan_showed` (a
  change of one of them between plan and confirm is `conflict: the plan
  changed` and no write is sent; the request that is sent carries no head);
  `goal_end_on_an_ended_goal_says_so_and_sends_nothing`;
  `a_signing_command_on_an_ended_goal_shows_no_plan` (`rules bind`, `workspace
  init` and `farm on` show no plan; `role give` sends nothing and prints no
  `Undo:` line; `level` still applies). G2's
  `a_signing_command_in_a_goal_that_is_catching_up_shows_no_plan` gains `goal
  end`.
- [presentation.rs](../crates/locust/src/cli/presentation.rs):
  `status_and_goal_status_say_who_ended_the_goal`;
  `an_ended_then_halted_goal_reads_as_both` (with `halted` equal to
  `authority_conflict`); `every_printed_command_parses_as_printed` gains `goal
  end` and the `farm off` line of the end plan.
- The site's `model.test.ts` beside model.ts: `an ended farm reads ended by
  the host`; `a blank ended farm reads ended by the host`; `a blank farm with
  no end reads unavailable`.
- Rewritten: `signed_current_protocol_vectors_are_frozen` and
  `body_indices_and_bytes_are_current_contract`. Unedited and passing:
  `decision_successors_keep_author_scope_purpose_and_predecessor_separate` and
  `closure_gates_authoring_and_reopened_starts_record_the_exact_position`,
  which close tasks only, and
  `closure_retention_tombstones_and_reopen_clears_timer` in the farm service.

**Exit criteria.**
- The three cargo commands and the four site commands of `AGENTS.md` pass,
  with `python3 scripts/check_formations.py` after its `--write`, `python3
  scripts/check_docs.py` and `python3 scripts/check_tla.py --suite fast`,
  which now runs the end cases. The two steps CI runs on every push pass too:
  the guide's recipes, `python3 scripts/check_documentation.py --binary
  target/debug/locust --timeout 60`, and the scripts' own tests, `python3 -m
  unittest discover -s scripts/tests`.
- `locust --json contract` lists `goal.end` with audience `host` and not as a
  tool. `locust --owner call scope.close` with `"scope": "goal"` exits 6 with
  `invalid`.
- `git grep -n -e 'scope.reopen' -e 'scope.close' -- scripts/check_farm.py`
  finds nothing, `git grep -n scope --
  scripts/client_qualification/live_farm.py` finds nothing, and `git grep -n
  Disputed -- crates/locust-proto/src/farm.rs` finds only task states.
- On two throwaway daemons in one goal with a page on and one open
  invitation: `locust --owner goal end --goal G --plan` twice prints the same
  plan id and changes nothing; `--confirm` prints `Ended "T".` and `1
  invitation revoked.`; the same command again prints `"T" has already
  ended.` and exits 0. The member's `status` then reads `ended by the host`,
  its agent's `contribution publish` exits 7 with the ended sentence, and
  `events` on both lists one `goal_ended`, with `host` as its author. The old
  ticket is refused. The page reads `Ended by the host` and receives no
  further check-in. `locust --owner farm off --goal G` then removes the page,
  and `events` shows no new record.
- With the member's daemon stopped before the end, a result it posts while
  stopped is effective on both daemons after they sync.
- After a debug build, `python3 scripts/check_farm.py --output DIR` passes
  with its new end step.

**Risks and notes.**
- What the end stops, on a computer that holds it: every new signature in the
  goal by any key. That covers tasks, attempts, results, reviews, checks,
  picks, plan revisions, file proposals, the first files a host shares
  (`workspace init`), publication consent, leave requests, admissions,
  removals, rules, roles, task revisions, tree epochs, stage steps, review
  requests and the recordings of roles-plan Phases 8 and 9. It also stops page
  check-ins.
- What it does not stop: reading the board, events, documents and files;
  syncing records and content signed earlier, and fetching content keys;
  `invitation list`; and `farm off`. Requests that sign nothing are not
  refused either: storing or withdrawing content, taking over an attempt,
  setting a level, connecting a folder. They change only that computer.
  Connected folders are not touched. Nothing is deleted on any computer. A
  member's computer that holds a removal but not its new content key could not
  write text before the end either (measured on today's code,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 9:
  `undelivered_removal_key_blocks_survivor_text_but_host_can_write_and_survivor_can_leave`
  in
  [delivery_characterization.rs](../crates/locust-core/src/node/tests/delivery_characterization.rs)).
  After the end it still asks for that key on the exchanges it opens, and text
  of that epoch opens when the key arrives.
- A name cannot be taken off the page after the end, because a decline is a
  signed record. A name leaves with the whole page: the host runs `farm off`,
  or the service deletes the ended page after its retention period, 30 days
  by default (answer 23).
- The page can still be blanked after the end. For each covered author the
  publisher reads the consent record with the highest position that it holds,
  also when that record is excluded and an older acceptance is still the
  effective one. It suspends the page when that record declines, is pending or
  is excluded, even if it accepts (measured on today's code,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 3:
  `later_consent_with_missing_predecessor_suspends_but_replayed_old_consent_does_not`
  and `excluded_consent_arriving_after_removal_suspends_even_when_it_accepts`
  in
  [farm_characterization.rs](../crates/locust-core/src/node/tests/farm_characterization.rs);
  read in `eligible` in
  [node/farm.rs](../crates/locust-core/src/node/farm.rs)). A decline signed
  before its computer learned of the end does this when it arrives. So does
  any consent record a changed client sends later: one anchored at the end is
  `AfterEnd`, and excluded is enough. A page suspended over a missing earlier
  record comes back when that record arrives. Otherwise the host cannot bring
  the page back, because that needs a new signature. It can still delete the
  page. A blanked page shows no names and reads "Ended by the host". The
  service keeps the first end time across a suspend and removes every page
  past its period, shown or blank (read in `mutate_inner` and `run_expire` in
  [the farm service](../crates/locust-farm/src/lib.rs); test
  `suspended_ended_farm_expires_at_original_deadline`). So a page blanked
  after it read ended is removed on its first date, and a page that was blank
  when the goal ended gets its date from the suspend that says ended. Only a
  page whose publisher's requests the service refuses stays as the service
  last held it (the note on a restored host, below).
- The catch, pinned by two tests: the end has no frontier, so it does not seal
  the board. A record signed before its signer's computer learned of the end
  counts whenever it arrives, as for a task close. What such a late record can
  still do:
  - Count as work. A result, an approval, a check or a pick by a role holder
    is effective when it arrives. A late approval can make a result count. A
    late reject can stop one counting, because a member's latest review is the
    one that counts (roles plan); what was already recorded on the earlier
    approval is not undone. Under the roles plan's one-member rule a late
    result needs no approval when its author was the goal's only member at its
    anchor, and first files count when posted. Only the host's agent can be in
    either position. It signs on the host's computer, which holds the end from
    the moment it is signed, so this needs a changed client or a restored
    host, and G1 holds a restored host's agents.
  - Change what the page shows. The publisher sends an upload when the
    snapshot changes, and the page stays "Ended". The publisher is not frozen
    after one last upload. The service keeps the time of the first upload that
    said ended while the page stays up (read in `mutate_inner` in [the farm
    service](../crates/locust-farm/src/lib.rs)), so a late upload does not
    push the deletion back.
  - Blank the page, as the note above says.
  - Mark its author as having left (E2). Nothing is signed after the end, so
    no removal follows and the mark stays.

  What a late record cannot do: change members, rules or roles, because only
  the governance key signs those and its records above the end are `AfterEnd`;
  reopen the goal; or make a computer that holds the end sign again. A changed
  client can keep adding records anchored before the end. The owner's
  assumption covers this: no hostile host, and hostile members are removed
  before the end.
- A restored host. Governance is read only from the usable prefix of the
  signer's log, so a second record of the governance key at or before the
  end's position takes the end out of force on every computer that holds both.
  GUARD prevents the ordinary case: a daemon started from an older copy
  recovers its own later records before it signs, the end is one of them, and
  once held the gate never opens. While the governance key is held, G1 also
  holds every agent on the host's computer, so the host's agent posts nothing
  in a goal its host ended. What remains: the person continues before recovery
  (`goal continue`); or the end reached no other computer before the host's
  copy was lost, and then no copy is ended. After a copy of unknown age the
  hold on a host's computer ends only on that command (G1), so a person who
  continues before a computer that holds the end has answered is the first
  case. The second is always so in a goal whose members are all on the host's
  computer. There G1 releases the key at once when its marks are kept, or
  after `goal continue` when they are not, and the goal is open again as the
  copy had it. In the first case, every computer that holds the end reads
  "ended, then halted" and signs nothing. The host's
  publisher then wants a blank page that reads "Ended by the host". Whether
  the page changes is a further step. A copy that was put back numbers its
  requests to the farm service from an old count, and the service answers a
  different request at a used number with `409`, a delete included (read in
  `mutate_inner`). So the page of such a host keeps what the service last
  held, and `farm off` there is refused the same way, until the joinable
  plan lets a delete, and a suspend that says ended, be signed above the
  service's number. Where the service was told of the end before the data
  was lost, the page already reads ended and is removed on time. A computer
  that holds the fork but never received the end reads plain "halted"; its
  agents can still sign work on what remains, as in any halted goal, and
  that work counts everywhere. One kind of computer can stay in that state.
  Sync passes the records beyond a fork on to every computer that is still a
  member on the copy that serves them. A member admitted at or after the
  forked position is no member on a copy that holds both records, so it is
  refused there and may never receive the end.
- Replacing a host is not designed here. This phase assumes the rule the
  research gives: an end at or before a takeover's base stands, and a takeover
  built on it is excluded, so an ended goal cannot be taken over; an end after
  the base is void like any other record the old host signed late, and the
  goal goes on under the new host. The one place to change is the `GoalEnded`
  arm of `Chain::build`, with the `cut_end` rule beside it. Because "ended" is
  derived and this phase sweeps nothing on members' computers, a voided end
  needs no local undo there. E1 builds none of the takeover rule. Its
  `AfterEnd` tests cover only records of the governance key, and a takeover is
  signed by another key, so the rule that excludes a takeover based at or
  after an end is the host-replacement plan's to add.
- Deliberately not built: the frontier that seals out late records; local
  delete and its tombstone; a notice to computers that are no longer members,
  so a removed member's computer is not told of the end and keeps asking; any
  change to what that computer answers, so `goal join` there still says
  "joined" from its old copy (measured for a removal today,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 1:
  39 `NotAMember` refusals in ten simulated minutes and a local "joined" with
  no new admission,
  `offline_removed_member_retries_refused_peers_and_fresh_ticket_returns_stale_membership`
  in
  [sim/lifecycle_characterization.rs](../crates/locust-core/src/node/sim/lifecycle_characterization.rs));
  taking the page down as part of the end; a publisher that stops after one
  last upload; permanent ended pages; a refusal that tells a joiner "ended"
  apart from "revoked"; a count of the members whose computers hold the end.
  Until local delete exists, a member has no command that removes an ended
  goal from their computer, and `goal leave` is refused there because it would
  have to sign.
- A host that ends a goal and switches the computer off before any member
  synchronizes leaves the goal open for everyone else. The result line says to
  keep the computer on.
- A halted goal cannot be ended, because the host can sign nothing more for
  it. `farm off` on a halted goal that holds no end still fails at signing;
  the delete-only path for that case is the joinable plan's. Whichever lands
  second adds its condition to the one path this phase builds. That path
  already covers a goal that is catching up.
- Which sentence a raw `call` gets. Agent requests go through the check of
  roles-plan Phase 3 and get the ended refusal first. A host request checks
  its own arguments first, so a `rules.bind` with a stale `expected` on an
  ended goal answers that conflict; with correct arguments it answers `ENDED`
  from `next_place`.
  On a copy that is ended, then halted, a handler that tests the halt first
  answers `halted`. On a host that is catching up and holds no end, `goal.end`
  answers `read_only` (G1). A text write on a computer that lacks the current
  content key answers `unavailable` today; once that computer holds the end it
  answers `ENDED`, because `next_place` runs before the key is looked up (read
  in authoring.rs). Nothing is signed in any of these cases.
- An invitation that is used or expires, or a member that is admitted or
  removed, between `--plan` and `--confirm` changes the plan id, and the
  person runs `--plan` again. Nothing else the host's computer signs in
  between refuses the end, because `goal.end` carries no head. On a busy
  public goal that can happen more than once. Whether the plan id should
  then leave the members out is the joinable plan's to weigh.
- Versions. The API version and the store marker go from 6 to 7 in Phase 1.
  The protocol version goes from 6 to 7 in K1, which lands directly before
  Phase 4. Phase 4 and E1 change signed bytes inside 7; E1's end record, at
  index 26, is the last change of the event format in the phases of the build
  order. It is not the last in v2. The public door changes signed bytes once
  more in its first admission phase: its contract names
  `MemberAdmitted.via`, `PublicationSet.goal_proof` and
  `DisclosurePolicy.joining`. The number that change takes is the master
  plan's to give. It stays inside 7 if nothing is released before the door,
  and takes 8 otherwise, with replacing a host one above. Nothing is released
  before Phase 10, so no number is raised twice. Inside API 7 this phase adds
  one operation and its response, `ended` on three views and `Act::End`, makes
  a close or reopen of the goal scope invalid, and removes the farm state
  `disputed`. One flag is added to the page's local record, and
  `FarmControlBody` and `FarmServiceView` gain one optional field each; no
  other stored layout changes. The frozen vectors are regenerated
  here, as in K1 and Phase 4. `FARM_VERSION` stays 1; because a farm state
  goes, the farm service and the site are deployed from this phase's commit or
  later.
- Roles-plan Phases 8 and 9 land after this phase.
  `Evaluation.desired_selections` must be empty when the copy holds an end.
  Phase 8 writes that test when it adds the function, and Phase 9's entries
  for the shared files pass through it. So a plan text that had settled, or a
  file change that counted, first files included, is not recorded after the
  end if it was not recorded before.
- `Undo:` lines. `goal end` asks the person typing it for a yes, so it prints
  none, and no command undoes it. An `Undo:` line printed before the end for
  a role is refused after it with the ended sentence, because it would sign.
  One for a level or an allowed task still runs, because it changes only
  that computer.
- The words. The texts say the end cannot be undone: no command reopens an
  ended goal. They do not say the board is sealed, because late records still
  count. They do not say the end is final against a later change of host: once
  a host can be replaced, an end signed after a takeover's base is void like
  any late host record, and the host-replacement plan changes the explanation
  then. The first release has no takeover.
- What public goals need from this phase ([review of the joinable
  contract](../research/joinable-farms-rewrite-contract-review-2026-10-05.md),
  findings 4, 5 and 12), and what it gives.
  - Signing readiness. This phase defines none. `goal end` and every other
    signature ask GUARD's one contract, `Node::hold` inside `next_place`, and
    admission asks `Node::admission_hold`. The end gate is a different thing:
    it reads signed records only and never opens again. Take an ordinary
    private goal whose members are all on the host's computer. After an
    ordinary restart nothing is held and `goal end` signs at once, with no
    other computer to ask. After the data directory was replaced by an older
    copy with its marks kept, G1 releases the governance key at once. After a
    copy of unknown age G1 waits for the person's `goal continue`; that wait
    is GUARD's rule and the owner's answer 13, not this phase's. `farm
    off` needs no signature in a goal that is ended or catching up.
  - A retry of an admission that is already committed. It is answered before
    the ended test and before the guard is asked, by the order `plan_join` has
    today, which this phase keeps. The joiner's computer then syncs as a
    member and receives the end.
  - The first end. It is one record with no frontier, and the catch above
    lists what late records can still do. The end does not take the page down
    and does not freeze the publisher. The publisher uploads when the snapshot
    changes and sends no check-in, and the service deletes the page after its
    retention period. The owner chose that the page stays, marked ended, and
    is not taken down by the end (answer 23).
  - Not given: a notice to computers that are no longer members; a refusal
    that says "ended" to a joiner; anything about a public door. The joinable
    plan closes a door in the commit of `goal_end`, beside `revoke_pending`,
    and its admission path signs through `next_place`, where the gate stops
    it. E1 gives that plan `State.ended`, `goal.end` and the terminal
    `InvitationRefused`. The first public door needs this phase. E2 lands
    before the door in the build order too, and what happens when a door
    member leaves is E2's.
- Read and inferred. Read in the code: the chain builder, its usable prefix
  and its default standing; the ancestry walk; `Goal::next` and every caller
  of `next_place` and `sign_at`; the fold's check, decision and evaluate; the
  projection of decisions; the page projection, its eligibility test and its
  check-in rule; `plan_join`, `goal_invite`, `goal_join` and the invitation
  store; `pending` and `wait`; the service's upload rule and retention; the
  body list and the vector tests; the site's model and its two pages;
  `check_farm.py`. Read for the check of the plan review and not run: a
  record beyond a fork of the governance key's log reaches every computer
  that is still a member on the copy that serves it. The frontier digest
  covers every held point of a log, a digest that differs sends an inventory
  of all of them, and `screen` keeps the host key's records whatever their
  standing (outbox.rs, initiator.rs and screen.rs). Inferred and not run:
  every behaviour of the end record, since it does not exist; that the
  governance key's log will hold host steps (K1) after KEY. Read again at
  `48bb12c`, when the three pieces were joined: `author`, `next_place` and
  `sign_at`;
  `Chain::build` and `authorize_base`; `Goal::next`; the body list, which has
  25 kinds; `eligible`, `project`, `farm_request` and the check-in rule;
  `plan_join` and `exchange_ended`; `invitation_revoke`, `goal_invite` and
  `goal_join`; `member_remove`; `mutate_inner` and `run_expire` in the farm
  service; and the characterization tests named above.

### E2: A member that leaves is removed, and disconnecting a host's agent

**Goal.** A member whose agent signs a leave is removed from the goal by the
host's computer, with nobody asked (answer 14). The removal is one record
signed with the governance key. It names the member, its admission and the
leave. It carries no new content key, no clock reading and nothing drawn when
it is signed, so signed again from the same records it is the same record.
From its position everything a removal means holds: the member's work up to
its leave stays, nothing it signs after the leave counts, its roles go back to
the host's agent, its place is free, and the other computers stop exchanging
with its computer. Until the host's computer has signed it the member stays in
the goal's record, as today, marked as left. The agent can come back with a
new invitation. Disconnecting a host's agent ends nothing, freezes nothing
and strands nothing, because one command connects the agent again. The
command applies at once, and its result says so in plain facts: it names the
goals that agent started, says that each keeps running and what waits until
the agent is connected again, leaves out goals that are already ended, and
ends with the line that undoes it.

**Depends on.** E1. Roles-plan Phases 1 to 5: `goal leave` as the person's
command and its result line (1, 2); `Node::stalled` with `RunnerLeft`, and
the check an agent's request passes before it signs (3); names, the role
lists, the rule that a removal empties a list to the host's agent, and the
only-member part (4); the status view (5). K1: `State.host`; the host's
agent, which cannot leave or be removed and can be disconnected and connected
again; the governance key, which signs removals; `Node::hosts`;
`Node::author_alone`; the list `agent revoke` reads, the goals in which this
agent is the host's agent; and `agent revoke` as a command that applies at
once and prints `agent reconnect` as its `Undo:` line. G1: `Node::hold`,
which `drive_flow` asks before it signs. In the build order this phase
follows E1 and comes before roles-plan Phase 7. It changes no signed byte: a
leave and a removal keep today's bodies. It changes one rule of replay, for a
removal that carries no payload, inside protocol 7, so E1's end record stays
the last change of the event format in the phases of the build order. The
model cases named under Changes are written before chain.rs is touched.

**Changes.** Three terms, each with one meaning below. A *leave* is the record
a member's agent signs to leave a goal (`LeaveRequested`). It names the
admission it ends and carries no text, so it needs no content key. A member
*has left* on a copy when it is a current member there and the usable prefix
of its own log holds an effective leave that names its current admission.
Where that prefix holds two, the one at the lower position is meant. The
*removal that follows a leave* is a `MemberRemoved` that names that member,
that admission and that leave as its last accepted record, with no payload and
clock field 0. Whether a member has left is a function of the signed events
held and of nothing else: no clock, arrival order or local flag enters. A
member that has left is still a current member until a removal of it is in the
host's record.

Today a leave removes nobody. Measured on today's code
([note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 7,
`leave_is_visible_in_events_and_replication_and_rotated_keys_continue_until_removal`
in
[sim/lifecycle_characterization.rs](../crates/locust-core/src/node/sim/lifecycle_characterization.rs)):
after a leave the member count is unchanged, and the leaver's computer still
receives later records, a rotated key and readable text until a removal. Read
in the code: `goal_leave` signs the request and sets a local flag, and that
flag is all that stops the agent signing; `goal_join` refuses a ticket for an
agent that left until its removal has arrived; every `MemberRemoved` starts a
new content-key epoch, and the proof of that epoch's key is the removal's
payload (`offer_key` in
[replica.rs](../crates/locust-core/src/node/replica.rs)), so a removal with no
payload would start an epoch whose key nobody can check. By reading and not
measured, the removal never arrives on the leaver's computer, because every
peer that holds it refuses that computer.

- [Organization.tla](../research/tla/Organization.tla),
  [cases.json](../research/tla/cases.json) and
  [organization.md](../research/tla/organization.md), done first, on the
  transcript E1's cases use. The model gains the ordinary kind `leave`. Four
  cases, each a
  fixed signed transcript delivered in every order, each with a config under
  `research/tla/configs/`: `organization-leave-removal` (a leave, the removal
  that names it, one record of the member below the leave and one above; the
  first is in `view.ordinary` in every state that holds it and the records
  below it, the second in none that holds the removal),
  `organization-leave-fork` (a second record at a position below the leave,
  delivered after the removal; the leave and the records it builds on stay),
  `organization-leave-readmission` (a leave, its removal, an admission of the
  same identity and one record under each admission) and
  `organization-leave-host-agent` (a leave signed by identity 5 marks nobody,
  and identity 5 stays a member). `ReplayMatchesHeld` holds for all four.
  `research/tla/RestoreGuard.tla` (G1's model) gains the action Leave and one
  named trace: a restored host whose only other computer belongs to a member
  that left signs no removal while it is held, the exchange with that computer
  runs to its end, and the removal the host signs afterwards is the record it
  signed before.
- [chain.rs](../crates/locust-core/src/goal/chain.rs): in `Chain::build`, the
  content-key epoch a `MemberRemoved` is judged at is `snapshot.epoch + 1`
  when the record carries a payload and `snapshot.epoch` when it carries none.
  The removal arm raises `snapshot.epoch`, and the `read_epoch` of the members
  that stay, only for a record with a payload. Nothing else in the arm
  changes: the admission ends at this position, the cutoff is `last_accepted`,
  and Phase 4 drops the member from every role list and gives an emptied list
  to the host's agent. So a removal with no payload ends an admission and
  starts no epoch. The module comment says so.
  [sync.rs](../crates/locust-proto/src/sync.rs): the module comment's sentence
  on the key epoch reads: the number of `MemberRemoved` decisions that carry a
  payload, at or before its anchor.
- [fold.rs](../crates/locust-core/src/goal/fold.rs): the arm of
  `Verifier::check` for a leave keeps its test, that the record names its
  author's admission at its anchor, and adds `invalid("the host's agent cannot
  leave its own goal")` when the author is `state.host`. So a leave signed
  with the host's agent's key marks nobody, as K1 excludes a removal of that
  agent.
- [state.rs](../crates/locust-core/src/goal/state.rs) and
  [projection.rs](../crates/locust-core/src/goal/projection.rs):
  `State.leave_requests`, which projection.rs writes and only one
  characterization test reads, is deleted with its arm. `Member` gains `left:
  Option<AuthorPoint>`. The `LeaveRequested` arm of `project` sets it on the
  author's entry when the leave names that entry's admission, and keeps the
  lower position when the entry has one already. A leave of an earlier
  admission of a member admitted again marks nobody. The entry keeps `left`
  after its removal, so a copy can tell a member that left from one the host
  removed.
- [goal/mod.rs](../crates/locust-core/src/goal/mod.rs): new `Goal::left(&self)
  -> Vec<(PublicKey, EventId, AuthorPoint)>`: each current member that has
  left, with its admission and its leave, in key order. New `pub const LEFT:
  &str = "this agent left the goal"`. The check an agent's request passes
  before it signs (Phase 3) answers `Why::State { reason: LEFT }` for an agent
  that has left, after E1's test for an end. That is a refusal on the goal's
  state, so its code is `conflict`, where such an agent gets `denied` today.
- [node/local.rs](../crates/locust-core/src/node/local.rs): the `m` record
  keeps its key and loses its flag. `Local.part` becomes a set and
  `part_write` takes no `left`.
  [views.rs](../crates/locust-core/src/node/views.rs): `Entry::membership`
  reads the record for a principal that took part: `Left` when its entry has
  `left`, else `Member` when it is a current member, else `Removed`. So an
  agent that left reads as left after a restart and after its removal has
  arrived, with no local record.
- [authoring.rs](../crates/locust-core/src/node/authoring.rs): in `next_place`
  the `Local.part` half of the member test becomes a reading of the record: an
  author that has left is refused `denied` with `LEFT`. It is skipped for the
  governance key with the member test, as K1 has it, and the order that K1, G1
  and E1 give the function is unchanged. In `requests/levels.rs` (Phase 3)
  `RunnerLeft` is read the same way.
- [node/flow.rs](../crates/locust-core/src/node/flow.rs): `drive_flow` signs
  the removal that follows a leave. Before it looks for a step, it takes the
  first entry of `Goal::left()` that passes every test below and signs
  `Body::MemberRemoved { member, admission, last_accepted: Some(leave) }` with
  the governance key through `author_alone`, which gives it no text and clock
  field 0. Then it reads the goal again. The tests: this daemon hosts the goal
  (`hosts(entry)`); `hold` answers none for the governance key (G1);
  `entry.goal.next` answers a place for that key, which it does not in a goal
  that holds an end or is halted; and this daemon has had its one chance to
  fetch what the member posted (next bullet). All run before the signature, so
  a removal that cannot be signed yet is passed over and is never an error. An
  error inside `land` would refuse a received batch and fail `Node::open`, as
  G1 found for a held step. The record is effective on this copy by
  construction: it names a current member and its current admission, that
  member is not the host's agent, and it carries no payload. `drive_flow` also
  loses its `Local.part` test for a member's agent and reads `Member.left`.
- [driver.rs](../crates/locust-core/src/sync/driver.rs),
  [peers.rs](../crates/locust-core/src/node/peers.rs) and
  [node/mod.rs](../crates/locust-core/src/node/mod.rs): one exchange first.
  The trait `Host` gains `opened(goal, endpoint)` with an empty default, and
  the driver calls it when it opens an exchange that presents no join. `Node`
  gains `asking` and `asked`, two sets of (goal, endpoint) kept in memory
  only. `opened` adds the pair to `asking` when this daemon hosts the goal and
  a member that has left is bound to the endpoint. `exchange_ended` moves a
  pair from `asking` to `asked` when the exchange this daemon opened has
  ended, completed or not, and touches the goal so that `drive_flow` runs. The
  test in `drive_flow` passes when the member's endpoint is this daemon's own,
  when another current member that has not left is bound to it, or when
  `asked` holds the pair. A pair leaves both sets when no member that has left
  is bound to its endpoint. The reason: content and keys travel only on
  exchanges a daemon opens itself, and once this daemon holds the removal it
  refuses the member's computer. So it opens one exchange first. Its record
  stage takes every record that computer holds, and its content stage fetches
  the text and files of the member's last work. An exchange that was already
  open when the leave arrived does not count, because its content stage may
  have passed. A computer that cannot be reached ends the exchange too, and
  the removal is then signed without that content. This decides only when this
  daemon signs. What it signs is the same either way.
- [goals.rs](../crates/locust-core/src/node/requests/goals.rs): `goal_leave`
  signs the leave and writes no local record. It keeps K1's refusal for the
  host's agent. `member_remove` is not changed: a member that has left is
  still a member, so the host can remove it by hand before this daemon does,
  with today's removal and a new content key. `goal_status` fills new
  `MemberView.left: bool`.
- [invitations.rs](../crates/locust-core/src/node/requests/invitations.rs):
  `goal_join` loses the refusal `the departure must be acknowledged by removal
  before rejoining`. An agent that has left joins like a new one: the join is
  written and the agent reads `joining`.
  [commit.rs](../crates/locust-core/src/node/commit.rs): `finish_joins` ends a
  join when the agent is a current member that has not left, so the admission
  it left never ends it.
- peers.rs, coming back. `Node` gains `refused`, a set of (goal, endpoint)
  kept in memory only. `exchange_ended` adds a pair when an exchange this
  daemon opened, that presented no join, ended `Refused(NotAMember)`, and
  removes it when one completes. `Host::joins` leaves out the join of an agent
  this copy still shows as a current member, which is an agent that left and
  whose removal has not arrived here, until `refused` holds the pair of the
  ticket's endpoint. Until then that pair runs the ordinary exchange, which
  delivers the leave. So a ticket is not presented to a host's computer that
  still lists the agent. `plan_join` refuses a current member for good, as
  today, and that refusal would use up the invitation. `plan_join` itself is
  not changed.
- [farm.rs](../crates/locust-core/src/node/farm.rs): in `eligible`, a leave
  joins the kinds whose author needs no consent. Today a member that joined,
  signed nothing else and left stays a required author through its leave and
  can no longer consent, so the page could never be published.
- [api.rs](../crates/locust-proto/src/api.rs): `MemberView.left` as above.
  `GoalSummary` gains `leave_taken: bool`, for an agent that has left: this
  copy shows it removed, or the host's computer has refused this daemon as not
  a member since the last start (`refused`).
- [presentation.rs](../crates/locust/src/cli/presentation.rs): the `Member:`
  line of `goal status` ends `· left, not yet removed` for a member that has
  left. Status prints, for an agent that left, one of two sentences from
  `leave_taken` (text E2-1) in place of today's sentence in
  `membership_action`. Nothing is listed under "Waiting for you" for a leave,
  on any computer.
- `crates/locust/src/cli/only_you.rs`: the result of `goal leave` gains the
  lines of text E2-1. Where another agent of this computer stays in the goal,
  the line that says to keep the computer on is not printed. The `member
  remove` plan prints `M already left; this computer removes it by itself.
  Removing it now also gives the goal a new content key.` when so.
- `agent revoke`. From K1 the command applies at once, shows no plan and
  ends with the `Undo:` line that names `agent reconnect`. This phase adds
  what its result says for a host's agent. The command reads K1's list, the
  goals in which this agent is the host's agent, and leaves out goals that
  hold an end. For each of the others it prints: `NAME started "T" (ID). The
  goal keeps running: inviting, removing and rule changes need no agent. NAME
  stays a member and cannot be removed.` and, where they apply, `Waits until
  you connect NAME again or give them to another member: the roles only NAME
  holds (ROLES).`, `Sharing this goal's first files needs NAME connected.`
  and `NAME's consent to this goal's public page cannot be given or
  withdrawn until it is connected again.` Text E2-2 shows the result.
  `agent_revoke` in
  [daemon.rs](../crates/locust-core/src/node/requests/daemon.rs) is not
  changed. The roles line applies where the agent alone holds a role
  (roles-plan Phase 4). The first-files line applies while the goal's shared
  files are empty, because only the host's agent shares first files (Phase 4's
  first-files rule). The page line applies while the goal's page is on,
  because publication needs the consent of every current member and signing
  one needs the agent's key (K1's notes).
- [check_operations.py](../scripts/check_operations.py): its
  `withdrawal_leave` step expects the refusal this phase gives an agent that
  left. Read at `541f29b` it expects `denied` twice, before and after a
  restart. From this phase the post is refused as the goal's state, which is
  `conflict`.
- Documents this phase owns: the paragraph on `goal leave` that roles-plan
  Phase 6 adds to the guide, which now says that the host's computer removes
  the member by itself, what the member's status shows until then and how to
  come back; and the explanation under "Leaving a goal" in this plan's
  "Intended behavior". `python3 scripts/check_formations.py --write`
  regenerates the runtime contract.

**Tests.**
- [goal/tests.rs](../crates/locust-core/src/goal/tests.rs), signed replays,
  each run forward, reversed and after a reload:
  - `a_removal_with_no_payload_ends_the_admission_and_starts_no_epoch`: the
    epoch and every other member's `read_epoch` are unchanged, and a text
    record sealed under the old epoch after it is effective.
  - `a_removal_that_names_a_leave_keeps_the_work_before_it_and_nothing_after`,
    on the fixture of
    `removal_retains_only_exact_cutoff_ancestry_and_readmission_does_not_backdate`:
    records below the leave stay effective, a record that builds on the leave
    is `PastRemoval`, and a second record at a position below the leave,
    delivered afterwards, takes nothing back.
  - `a_member_has_left_only_by_a_usable_leave_of_its_current_admission`:
    `Goal::left` lists it; it does not for a leave of an earlier admission,
    while a record below the leave is missing, or while the log is forked
    below it; of two leaves the lower position is named.
  - `a_leave_signed_by_the_hosts_agent_is_invalid_and_marks_nobody`.
  - `a_removal_that_follows_a_leave_gives_the_members_roles_to_the_hosts_agent`:
    Phase 4's rule for a removal holds for one with no payload. A result the
    host's agent anchors after it counts when posted, and one posted before it
    does not start counting.
- [lifecycle.rs](../crates/locust-core/src/node/tests/lifecycle.rs):
  - `the_hosts_daemon_removes_a_member_that_left_with_nobody_asked`: after
    `goal.leave` on the member's daemon and the exchange the host's daemon
    opens, the host's log holds one `MemberRemoved` with no payload, clock
    field 0 and the leave as `last_accepted`; the epoch is unchanged, no
    content key was written, and nothing waits.
  - `a_removal_that_follows_a_leave_signed_twice_from_one_input_is_one_record`,
    signed from two stores, in the form of G1's
    `a_stage_step_signed_twice_from_one_input_is_one_record`.
  - `the_removal_waits_for_one_exchange_and_takes_the_leavers_last_content`:
    the member posts a result with text while cut off, leaves and reconnects;
    the host's daemon holds the text before it signs the removal. With the
    member's computer unreachable the removal is signed after the failed
    exchange.
  - `no_removal_follows_a_leave_while_the_goal_is_ended_or_halted_and_nothing_fails`:
    the batch that brings the leave lands, and `Node::open` succeeds.
  - `an_agent_that_left_reads_left_from_the_record_and_signs_nothing`, also
    after a restart and after its removal has arrived.
  - `a_member_that_left_comes_back_with_the_same_agent_on_a_fresh_ticket`, in
    two orders: the ticket is presented after the removal, and the ticket is
    taken before the leave has reached the host's computer. In the second the
    join waits, the ordinary exchange delivers the leave, and the agent is
    admitted on the same ticket.
  - Phase 4's `a_second_agent_of_the_same_person_is_a_second_member` changes
    its last assertion: an agent on the host's own computer that sent
    `goal.leave` is removed by the host's daemon in the next commit.
- `crates/locust-core/src/node/tests/guard.rs` (G1's):
  `a_host_that_is_catching_up_signs_no_removal_and_keeps_exchanging_with_the_leavers_computer`.
  The copy is older than a rule change that only the leaver's computer holds.
  The leave arrives first, the exchange runs on, the rule change returns, and
  the removal is signed once the hold has ended.
- [sim/lifecycle_characterization.rs](../crates/locust-core/src/node/sim/lifecycle_characterization.rs):
  `leave_is_visible_in_events_and_replication_and_rotated_keys_continue_until_removal`
  becomes
  `a_leave_is_followed_by_the_hosts_removal_and_replication_to_the_leaver_stops`:
  within a minute of machine 2's leave the host's log holds the removal, the
  epoch is unchanged, and a record the host publishes afterwards does not
  reach machine 2. Run again with the host's machine stopped at the leave,
  machine 2 keeps receiving from machine 3 until the host is back.
- [farm_characterization.rs](../crates/locust-core/src/node/tests/farm_characterization.rs):
  `departed_member_cannot_sign_consent_withdrawal_even_for_owner_after_restart`
  has both agents on one daemon, so the removal follows the leave in the next
  commit. It asserts that, and keeps its other assertions: the owner's
  withdrawal and a contribution for that agent are refused, also after a
  restart, and the agent's consent and public profile stay.
  [tests/farm.rs](../crates/locust-core/src/node/tests/farm.rs):
  `a_member_that_only_left_is_not_asked_for_consent`.
- [sync/tests/driver.rs](../crates/locust-core/src/sync/tests/driver.rs):
  `the_host_is_told_when_an_exchange_is_opened_and_never_for_a_join`.
- [presentation.rs](../crates/locust/src/cli/presentation.rs):
  `goal_status_marks_a_member_that_left`;
  `status_says_whether_the_hosts_computer_has_the_leave`.
- [authorization.rs](../crates/locust-core/src/node/tests/authorization.rs):
  `revoking_the_hosts_agent_leaves_goal_end_working`: after the revoke and
  after a restart, `goal.end` succeeds.
- [cli.rs](../crates/locust/tests/cli.rs):
  `agent_revoke_names_only_goals_that_are_not_ended_and_says_what_waits`: for
  a host's agent the result prints the first line for each goal it started
  that holds no end, the roles line only where it alone holds a role, the
  first-files line only while the shared files are empty, the page line only
  while the page is on, and the `Undo:` line last. No plan is shown.
- `signed_current_protocol_vectors_are_frozen` and
  `body_indices_and_bytes_are_current_contract` pass unedited.

**Exit criteria.**
- The three cargo commands pass, with `python3 scripts/check_formations.py`
  after its `--write`, `python3 scripts/check_docs.py` and `python3
  scripts/check_tla.py --suite organization`. The two steps CI runs on every
  push pass too: the guide's recipes, `python3 scripts/check_documentation.py
  --binary target/debug/locust --timeout 60`, and the scripts' own tests,
  `python3 -m unittest discover -s scripts/tests`.
- On two daemons, A the host and B: after `locust --owner goal leave` on B,
  and within a minute, `locust --owner events` on A lists one
  `leave_requested` by the member and after it one `member_removed` by `host`,
  and A's `goal status` no longer lists the member. A's `status` says "Nothing
  is waiting for you." before and after. A result A's agent then posts counts
  with no approval, and its text is written at once: no new content key was
  needed. After B's next exchange with A, B's `status` reads `left` with the
  second sentence of text E2-1.
- With A stopped during the leave, B's `goal status` prints `left, not yet
  removed` on the member's line. After A starts, the removal follows.
- A fresh invitation presented by the same agent is admitted. `events` on A
  shows a second `member_admitted` for that key, and the agent's next result
  is effective on both daemons.
- A member removed and admitted again shows no mark from a leave of its
  earlier admission.
- On the host's computer, `locust --owner agent revoke --agent NAME` for the
  agent that started a goal shows no plan. It prints the line that begins
  `NAME started "T"` and, last, `Undo: locust --owner agent reconnect --agent
  NAME`. `goal invite` still works. That line, run as printed, connects the
  agent again, and `workspace init` in that goal then shows its plan. After
  `goal end` on that goal the same `agent revoke` prints no line for it.
- With `--binary` set to a debug build, one run of `python3
  scripts/check_operations.py` passes with its changed leave step.
- `git grep -n -e leave_requests -e 'acknowledged by removal' -- crates` finds
  nothing.

**Risks and notes.**
- Why this record may be signed with nobody present. It is row 5 of [What the
  host's computer signs by itself](#what-the-hosts-computer-signs-by-itself)
  and meets that section's five points. Its bytes are the member, its
  admission and its leave. It carries no clock reading, no payload and no new
  key, and it passes `next_place`. A host started from an older copy that
  signs it again at the same position signs the same record. A removal signed
  by hand does not: it draws a new content key, carries a clock reading and
  names whatever record of the member the host's computer holds last (read in
  `member_remove`). What is left is a position that another record used in
  between, which the restore guard covers as for every record in this log.
- One restore ends at the person. The leaver's computer is never sent the
  removal. Where it was the goal's only other computer, the removal reached
  no computer at all. A host's computer put back from a copy older than that
  removal, with its marks kept, is then behind by a record nobody can give
  back, and G1 never gives up a host record by itself: the goal waits for
  `goal continue`. Once the earlier host records have come back, the removal
  signed after that command is the record signed before.
- The member stays in the goal's record until the host's computer has signed.
  While that computer is off or catching up, the member is listed with the
  mark, its computer keeps exchanging, a role it holds stays with it, and
  under peer approval a goal of two still waits for its approval. In an ended
  goal nothing is signed and nothing needs to be. In a halted goal `goal
  leave` works, because a member's key can still sign, and no removal follows:
  the host can sign nothing there. A member's own daemon signs nothing for an
  agent that left in any of these states.
- The content key. A leave draws none. The member that left keeps the key it
  holds and is sent nothing more, because every computer that holds the
  removal refuses its computer. So it reads nothing new unless a member hands
  it the sealed bytes, and a member could hand over the plain text as well.
  The next removal by hand draws a new key, as today. After the automatic
  removal no command gives the goal a new key by itself. That is put to the
  owner.
- A member admitted moments ago. G1 holds its key as `Admitted` until this
  computer has heard from the host's computer and can read the goal's current
  rules and holds its current content key. A leave needs neither, but every
  gate reads only `Node::hold`, so such a member cannot leave until that
  content arrives from any member's computer, or until its owner runs `goal
  continue`. Before G1's follow-up fix (`a0dff5e`) the hold ended on hearing
  the host's computer alone. Kept so: one gate, and the wait covers only the
  first moments after admission.
- One exchange first. It is this daemon's own timing and nothing shared
  depends on it. In the ordinary case the exchange opens at once, because the
  leave changed the goal, and the removal follows within seconds. After a
  failed exchange the driver retries within a minute, and the first exchange
  that ends is enough. What it cannot save: content on a computer that is
  switched off right after the leave. The result line of `goal leave` says to
  keep the computer on.
- Other members' computers hold the leave before they hold the removal. Until
  then they exchange with the leaver's computer as with any member. Content
  the host's computer fetched before it signed is fetched from the host's
  computer afterwards.
- A leave that reaches no other computer changes nothing elsewhere. The
  leaver's daemon keeps opening exchanges, and its status says the host's
  computer does not have the leave yet.
- The leaver's computer afterwards. Every peer that holds the removal refuses
  it, and nothing tells it why: a notice to computers that are no longer
  members is not built (E1's notes). Its daemon knows from its own leave. It
  keeps asking each peer about twice a minute, as a removed computer does
  today (measured,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 1).
  Asking less often is [left for later](#left-for-later). The joinable
  plan's J1 backs off after a long run of failed exchanges, and the driver
  counts a refused exchange as a failed one. The second status sentence
  rests on that refusal by the host's computer. A host's
  computer started from a copy older than the member's admission refuses the
  same way, so in that rare case the sentence is early; the leaver's daemon
  keeps asking, and the leave is delivered once that host has caught up.
- A leaver's computer put back from a copy older than its leave reads as a
  member there, and every peer refuses it. With its marks kept G1 holds its
  key for good, and status offers `goal continue`; what it signs after that
  reaches nobody. `goal join` on such a copy answers `joined` from the copy
  and presents nothing (measured for a removal,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 1).
  This is the state of any computer that was removed while it was away, and no
  phase of this plan lifts it.
- Coming back. The same agent joins again with a new invitation, on the same
  computer. Its daemon presents the ticket only once the host's computer has
  refused it as not a member, or once its own copy shows the removal, which it
  does where another agent of that computer stayed in the goal. One order is
  left. A host's computer restored from a copy older than the agent's
  admission refuses the leaver's computer, then catches up from another member
  and lists the agent again without its leave. A ticket presented in that
  window is refused for good. The ordinary exchange then delivers the leave,
  the removal follows, and the person needs a second invitation.
- A member whose own log holds two leaves on two branches, or a second record
  at a position below its leave, has not left by this phase's definition, and
  no removal is signed for it. Only a changed daemon produces that. The host
  removes such a member by hand (answer 8).
- A member's accepted work cannot be taken back after the removal. An ended
  admission keeps exactly the leave and the records it builds on, whatever
  that member's log holds later (read in `Chain::cutoff`;
  `removal_retains_only_exact_cutoff_ancestry_and_readmission_does_not_backdate`).
  Before the removal a member that left can lose its records from a forked
  position on, as any member can.
- Results posted before the removal do not start counting when the host's
  agent is alone again. The agent posts them again (Phase 4). The only-member
  part reads the host's record alone, so no leave changes what counted at an
  earlier anchor.
- A public door. A stranger's join and leave cost two records signed with
  nobody present, the admission and this removal, and no content key. Nobody's
  writing pauses. The place under the ceiling is free from the removal's
  position; a door seat is not returned. The record keeps one entry and one
  ended admission for each key ever admitted, as after any removal. The
  joinable plan measures that with its ceiling.
- A removal that follows a leave moves the head of the host's record with
  nobody present, as an admission does. No command carries that head any
  more (roles-plan Phase 2, and E1 for `goal end`). A `rules bind` whose plan
  was shown before it still applies, because it is bound to the current
  rules. A `goal end` whose plan was shown before it answers `conflict: the
  plan changed`, because its plan shows the members, and the person runs
  `--plan` again.
- "Cannot leave" for the host's agent is its daemon's refusal, and from this
  phase a leave signed with that key is invalid in replay. Other computers
  reach the host's computer through that agent's admission (K1).
- In an ended goal `goal leave` is refused with the ended sentence (E1): it
  would have to sign. A leave signed before its computer learned of the end
  marks the member when it arrives, and the mark stays.
- A member that left cannot take its name off a public page afterwards. Its
  own daemon signs nothing more for it, the owner's request included (measured
  on today's code,
  [note](../research/goal-lifecycle-characterization-2026-10-05.md), claim 2:
  `departed_member_cannot_sign_consent_withdrawal_even_for_owner_after_restart`),
  and a consent record signed after the leave is past the removal. A member
  who wants its name off declines publication before it leaves. What a page
  shows for a member that left is the joinable plan's.
- Disconnecting an agent is local, and it is no shared halt. It is kept on
  this computer only, it survives a restart, and it signs no record, so no
  other computer learns of it. Today no command undoes it. K1 adds the one
  that connects the agent again, which changes the same local record and
  signs nothing either (the revoke is measured on today's
  code, [note](../research/goal-lifecycle-characterization-2026-10-05.md),
  claim 5:
  `revoked_host_cannot_resume_governance_through_grants_or_enrollment_but_old_store_can`
  in
  [lifecycle_characterization.rs](../crates/locust-core/src/node/tests/lifecycle_characterization.rs)).
  A data directory put back from before the revoke holds the agent connected
  again. From K1 the governance key signs without the agent, so this phase
  words what is left to say and leaves out goals that are ended. While the
  agent is disconnected it approves nothing, so under peer approval a goal
  whose only other member it is counts no new result until it is connected
  again. Connecting it again does not help a host's agent whose own log holds
  two records at one position; that case stays as K1 and G2 describe it.
- Read in the code at `986c18d`, where the crates are those of `6944de4`:
  `goal_leave`, `member_remove`, `goal_join`, `finish_joins`; `Chain::build`,
  `cutoff`, `authorize` and `authorize_base`; the leave arm of the fold and of
  the projection; `offer_key`; `next_place`, `author` and `sign_at`;
  `drive_flow`; `peers`, `speaks_for_member`, `joins`, `plan_join` and
  `exchange_ended`; the responder's refusal; `poll` and `end_dialed` in the
  driver; the initiator's stages; `Entry::membership`; `eligible`;
  `agent_revoke`. Inferred and not run: every behaviour after the change; that
  the first exchange opened after a leave ends within a minute; everything
  about the role lists, the only-member part, the check before signing,
  `Node::stalled` and the commands' texts, which the roles plan has not
  built; `Node::hold`, `author_alone` and `agent reconnect`, which are plan
  text; and that a post by an agent that left is refused as `conflict` from
  this phase.
- Size, a judgement: 250 to 400 lines of production code and about 800 of
  tests.

## Left for later

**Idle goals ask less often.** This plan had a third ending phase, E3. In it a
goal with no new records opened exchanges less and less often, from every 30
seconds toward once an hour, and the host's computer checked in with the farm
service at the same slower pace. It is deferred out of v2: the plan author's
decision of 6 October 2026, after the
[independent review](../research/v2-plan-review-2026-10-06.md) and the
[check of it](../research/v2-plan-review-verification-2026-10-06.md). No other
phase reads its code, no owner answer asks for it, and it changes no signed
record.

What a person notices without it is what they see today. Every goal keeps
asking its other computers every 30 seconds, quiet or not, so an idle goal
uses a little more network. A public page reads "Receiving updates" steadily
while the host's computer is on, and no record arrives late. With E3 an idle
page would have read "Quiet" most of the time, and a computer that was asleep
could have waited up to an hour for the first new record. A computer that was
removed from a goal, or whose agent left it, keeps asking each other computer
about twice a minute, as today.

What waits with it: `idle_interval_ms` and `IDLE_ANTI_ENTROPY_MS`, the
driver's note of each goal's last news, `Host::catching_up`, the check-in pace
in `farm_poll_local`, and new legend sentences for "Receiving updates" and
"Quiet". The measured test
`idle_three_member_goal_keeps_exchanging_for_a_simulated_hour` stays as it is.
The phase as it was written, with its tests, is in this file at commit
`541f29b`, and its three unsettled points are in the companion. The joinable
plan meant to change `idle_interval_ms` for an open door. It now writes its
failure backoff (J1) and its check-in rule without that function. One fact
from G1 goes with the phase: a goal that is catching up keeps the 30-second
interval, because a hold ends only on exchanges.

## What is left for replacing a host

Replacing a host comes after v2. Each part leaves one place for it.

**The signing key (K1).** One place: the first lines of `Chain::build` in
crates/locust-core/src/goal/chain.rs, where the chain takes its key
(`history.governance`) and its host's agent (`history.host`) from the goal's
first record and follows that one key's log. Readers in replay and on the node
go through `State.governance` and `State.host`, and the node keeps one keypair
per goal in `Local.governance`. Replacing a host makes that choice a function
of the chain: follow the first key's log up to a takeover's base, then the
next key's log. It is one of three named places; the other two are END's
`GoalEnded` arm with `cut_end` and GUARD's `Node::guard_sources`, and the
master plan lists all three. What K1 assumes about it: (1) a takeover is
signed by a fresh governance key made on the backup host's computer, and the
old key is never copied or exported; (2) the takeover names a new host's agent
that is a member at its base, which takes over the four things the host's
agent does (it is how members reach the host's computer, it carries the host's
name, it holds a role nobody holds, and under Phase 4's first-files rule only
its change with no parent counts as first files), and only then may the old
host's agent be removed, so the replay rule that excludes its removal and the
first-files rule both read the host's agent of the current term; (3) host
steps sit in the governance key's log, so one cutoff by position in that one
log covers governance and recordings alike, and no second cutoff for an
agent's key is needed; (4) `Invitation.governance` then has to tell the goal's
first key from the key of the current term, and that field is where it is
added (the version ladder lists the invitation's issuer field with the
takeover record, outside this plan's ladder); (5) a fork in the governance log
stays a permanent halt until a takeover exists, and K1 adds nothing that lifts
it, so a goal made under v2 keeps it for good (answer 2); (6) two readers run
before a chain exists and read the first record
itself, `screen` in crates/locust-core/src/goal/screen.rs (which records are
kept) and the join check in crates/locust-core/src/node/replica.rs; a takeover
design must say what each keeps for a later key.

**The restore guard (G1, G2).** The named place is `Node::guard_sources(entry)
-> Sources` in `crates/locust-core/src/node/guard.rs`: the one function that
says which computers a daemon must hear from in a goal. Today `all` is every
other computer and `host` is the host's computer, the endpoint in the
admission of the host's agent (`state().members[&state().host]`, K1's), when
that is not this one. Host replacement changes what it reads: after a takeover
the host's computer is the new host's, the old host follows the member rule,
and that plan may add the named backup host as a computer a restored host must
hear from, since the backup's copy is what a takeover keeps. Beside it,
`Node::heard(goal)` is what a takeover's plan reads to show which members were
heard from before it chooses its base. It is one of three named places; the
other two are KEY's (the first lines of `Chain::build`) and END's (the
`GoalEnded` arm with `cut_end`). GUARD assumes five things about host
replacement. (1) A takeover is signed by a fresh key at position 0 of its own
log, so it never reuses a position of the old governance key and the marks
stay per key. (2) The new governance key signs through `next_place` and
`sign_at`, so it gets a mark and is held like any key, and the agents on its
computer are held with it; a new host restored from a copy taken while it was
host is then behind, not forked. (3) What a woken or restarted old host must
learn before it signs, that it was replaced, is a rule of the chain that host
replacement adds to `Goal::next`; it is not a hold, and the guard does nothing
on a wake. (4) The backup's daemon applies the guard to its own keys before it
signs a takeover, and its takeover command refuses or warns from
`Node::heard`; that command is host replacement's. (5) The guard does not make
the last host record a takeover's signers hold any newer; it only keeps a
restored daemon from signing below what it already signed. One idea is
recorded here and not built in v2: after a restore the host's computer goes
on by itself under a new key, from the last record it holds, and what the old
key signed later is void. It needs the takeover record, so it belongs with
replacing a host. It is scoped in the
[check of the review](../research/v2-plan-review-verification-2026-10-06.md).
Until then a copy of unknown age on a host's computer waits for `goal
continue`.

**Ending a goal (E1, E2).** One named place: the `GoalEnded` arm of
`Chain::build` in crates/locust-core/src/goal/chain.rs, marked with the
comment `PLUG host-replacement: takeover versus end`, together with the
`cut_end` rule that follows the loop in the same function. Everything else END
builds reads what that function produces (`State.ended`, `cut_end`,
`Goal::end_held`), so nothing else changes when a takeover exists. Assumptions
about host replacement, all taken from the research and none designed here:
(1) a takeover names a base, a record of the old governance key, and whatever
that key signed after the base is void whenever it arrives; (2) the end record
is an ordinary record of that key under this rule; (3) an end at or before the
base stands, and a takeover whose base is the end or anything after it is
excluded, so an ended goal cannot be taken over or continued under its id. The
host-replacement plan adds that rule and E1 does not: E1's `AfterEnd` tests
cover only records of the governance key, and a takeover is signed by another
key; (4) an end after the base is void: `State.ended` stays unset and
`cut_end` does not count it on any copy that holds the takeover, so the gate
opens again there and the goal goes on under the new host; (5) the new host
may end the goal by the same record, signed by the governance key then in
force; (6) the old page, tickets and invitations do not carry over, so the
invitations E1 revoked on the old host's disk, and the end time the farm
service stored for the old page, need no undo. E1 keeps one
property for this plug on purpose: "ended" is derived from the records held,
and no member's computer writes any local state when an end arrives, so a
voided end is undone by replay alone. G1's holds are untouched by an end for
the same reason, so a key that was held is held again once an end is void. If
host replacement later reads the kept chain by ancestry from the base and not
by the usable prefix, the same reading can make an end survive a fork of the
old key below it; that would retire "ended, then halted" and is the upgrade
path, not part of this piece. The other two plugs are the first lines of
`Chain::build` (KEY) and `Node::guard_sources` (GUARD). E2 adds no place of
its own. Its removal is an ordinary record of the governance key, so
assumption (1) covers it: one signed after a takeover's base is void like any
other. What then removes a member that left, and what a backup host's own
leave means, are the host-replacement plan's to say.

## Models written first

Each part asks for a model under `research/tla` before its code. The restore guard's is written; the rest are not, and the work is not sized.

**The signing key (K1).** A small change to research/tla/Organization.tla,
made and run before chain.rs is touched, as the first step of K1. It is the
first of four changes under research/tla: K1's change, the role holders of
roles-plan Phase 4, G1's model, then E1's cases. All of them keep K1's
identities (the host is identity 0, the host's agent identity 5). The model
already treats identity 0 as the only signer of governance, but record 2 of
its founding transcript admits identity
0 as a member. Change that record to admit a new identity 5, the host's agent,
and keep every other id, position and anchor, so no existing scenario is
renumbered. Add two scenarios, each with a config under research/tla/configs
and a row in cases.json: `governance-key-work` (a contribution and a review
signed by identity 0 are in `view.ordinary` in no reachable state) and
`host-agent-fork` (two records at position 0 of identity 5's log, then an
admission by identity 0; every held governance record stays in
`view.governance`). Run `python3 scripts/check_tla.py --suite organization`.
Nothing larger is needed for this piece: the key is constant for the life of a
goal. The signer as a function of position, delivery-step properties and a
restore action belong to replacing a host and to GUARD. The model does not
cover anchors chosen by the host's agent under Phase 4's only-member part;
that limit is stated in prose and pinned by Phase 4's test. It does not cover
the first-files rule either; Phase 4 says so in a dated note in
research/tla/workspace.md and rests that rule on its Rust tests.

**The restore guard (G1, G2).** Written on 6 October 2026:
[RestoreGuard.tla](../research/tla/RestoreGuard.tla), with its
configurations under `research/tla/configs`, its entries in
`research/tla/cases.json` (its cases join the `fast`, `extended` and
`restore` suites) and its page
[restore-guard.md](../research/tla/restore-guard.md), run by `python3
scripts/check_tla.py --suite restore`. It is a node model, not a change to
Organization.tla: the guard is not a validity rule. Two keys on one daemon
(the governance key and one agent key), that daemon and two peers, one
admission and one removal allowed (a second only after Continue, which
only the override trace and E2's leave trace enable), bounded log length.
Actions: Sign,
Admit, Remove, Sync(a, b) with an order in which a member's records can
arrive before the admission that names it and are then dropped, Copy,
RestoreStore (store back, marks kept), RestoreAll (both back), LoseMarks,
Start (classify by the first table), Hear (per peer, only on a sync that
brought nothing), Settle (the second table, with the row that holds the
agent key while the governance key is held), Continue. Its properties, by
the names the model gives them:

- `StoreNoFork` (cases `restore-p1-store`, `restore-p1-store-extended`):
  after RestoreStore without Continue, no two computers hold different
  records at one position, with the plan's exemption for the agent key: a
  removal that the restored copy does not hold, signed after the copy or
  signed before it and never delivered to it. This is what the guard
  guarantees. The stronger
  `StoreNoReuse` fails, by design: a record that was lost everywhere can
  have its position used again under the plan's two rules for that, the
  never-shared release of the governance mark and the give-up of an agent
  key's record once every other computer has answered. The model keeps
  both traces, `restore-finding-private-reuse` and
  `restore-finding-unseen-agent-reuse`: finding F1 of
  [restore-guard-model-2026-10-06.md](../research/restore-guard-model-2026-10-06.md).
- `AllNoFork` (cases `restore-p2-all`, `restore-p2-all-extended`): after
  RestoreAll on the host without Continue, no fork, whatever was admitted
  or removed after the copy, because that hold ends only on Continue.
- `OrdinaryStart` (case `restore-p3-ordinary`): an ordinary healthy Start
  sets no hold.
- `HoldsEnd` (cases `restore-p4-recovery`, `restore-p4-alone`,
  `restore-p4-agent-give-up`) and `WillingHoldsEnd` (case
  `restore-p4-willing-general`): after RestoreStore, with every peer
  eventually reachable and honest, the holds end when the goal was never
  shared or a peer that may still send holds the marked record. The
  stronger "a peer holds it" does not hold:
  `restore-finding-removed-keeper` keeps the case where the only other
  holder was removed since and answers with nothing, and that wait does
  not end by itself (finding F2). G1 names the case, and G2 lists the goal
  under "Waiting for you" with the command that ends it. `AllStayHeld`
  (cases `restore-p4-all-held`, `restore-p2-all`): after RestoreAll on the
  host neither key's hold ends without Continue.
  `restore-p4-fairness-removed` shows the holds end only when settling is
  scheduled.
- `AgentFenced` (case `restore-p5-agent-fenced`): on the host's daemon the
  agent key never signs while the governance key is held.
- The six named rule removals, each a kept trace with a guarded twin that
  passes: `restore-lacking-peer` (the hold released after the first
  exchange with any one peer), `restore-empty-shared` (an empty peer list
  ending a hold while the mark says shared), `restore-hear-with-data`
  (hearing counted on an exchange that brought records),
  `restore-give-agent-held` (an agent key given up while the governance key
  is still behind; `NoGiveWhileHeld`), `restore-agent-signs-held` (the
  agent key signing while the governance key is behind) and
  `restore-all-listed` (the host's hold after RestoreAll released once
  every peer the copy lists was heard from). The two planned residuals
  reproduce with every rule present: `restore-residual-removed` (the agent
  key given up after RestoreStore when its record reached only a peer
  removed since) and `restore-residual-member` (RestoreAll on a member's
  daemon when the host's daemon never received the agent key's last record
  and the other peer did). `restore-continue-override` shows that the
  person's command can cause a retained fork.

Also written, and not a model: the file-identity note named in G1's exit
criteria,
[restore-file-identity-2026-10-06.md](../research/restore-file-identity-2026-10-06.md).
Its measurements are folded into G1's notes and exit criteria.

**Ending a goal (E1, E2).** Before the chain rule of E1 is built, add the
governance kind `end` to research/tla/Organization.tla (its `GovKinds` today
is genesis, admit, remove, rules) and register the cases in
research/tla/cases.json in the registry's own form: one entry per case, named
`organization-end-...`, suite fast, extended and organization, with a config
under research/tla/configs/ and a `Scenario` in the model. Written on the
founding transcript as roles-plan Phase 4 leaves it: K1's identities (the
host is identity 0, the host's agent identity 5) and the two role events. The
model changes land in this order: K1's change, Phase 4's role holders, G1's
model, then E1's cases. Each case is a fixed signed transcript delivered in
every order. (1) organization-end-late-record: a result and an approval
anchored before the end
are delivered after it and become effective. (2)
organization-end-later-governance: an admission and a rules binding signed
after the end are never effective. (3) organization-end-fork-at-or-before: a
second host record at the end's position, and one at an earlier position,
leave no end in force and report the fork. (4) organization-end-fork-above: a
second host record above the end leaves the goal ended with no halt. (5)
organization-end-anchored-at-end: a work record that names the end as its
anchor is never effective. Case 5 keeps only the record anchored at the end;
the record by the end's signer above it is pinned by the Rust test. (6)
organization-end-after-readmission: an end after a member was removed and
admitted again changes neither tenure nor the removal's cutoff. Keep
`ReplayMatchesHeld` for all six and add one safety property, EndIsTerminal: in
any held set with an end in force, no governance record after the end and no
record anchored at or after it is effective. Add one witness case,
organization-end-late-record-witness: a record signed before the end is
effective in a state reached after the end was held. Not modelled, and said so
in research/tla/organization.md: the signing gate and `cut_end`, which are
daemon behaviour and not replay; the frontier, which is not built; and an end
against a takeover base, which waits for the takeover record to have a shape.
The registry's baseline says the model covers a subset of the organization
rules at an older protocol number; these cases extend that subset and do not
move the baseline. E2 adds the ordinary kind `leave` and four cases named
`organization-leave-...` on the same transcript, and one trace to G1's model.
E2 lists them.

## Questions for the owner

Each question carries its writers' recommendation, which the text above
assumes. Those the owner has answered are marked, and so are those the plan
author settled on 6 October 2026 under the owner's answers. Four questions
about choices a person never sees were taken out, and each choice is stated
where it is built: one key for everything the host's computer signs (under
"What the host's computer signs by itself"), the build order (the master
plan), calling back an unknown computer while catching up (G1), and the
models (Models written first). The two questions about idle goals went with
the phase that is [left for later](#left-for-later). One question from the
check of the review is the master plan's to put: whether anything is released
before the public door, which decides the number the door's change of signed
bytes takes (E1's note on versions).

**The signing key (K1)**

1. May a host remove the agent it started a goal with? Recommended: not in
   v2. That agent can be disconnected and connected again, or set to read,
   and the goal keeps running either way. Removing it needs the release that
   brings a backup host, which names a new host's agent, and under answer 2
   that release ends the goals made under v2.
2. Should anything a person reads name the key that signs members and rules?
   Recommended: no. Text says host, and a record that key signed prints as
   host. On the host's own computer status and plans say `Host: you` with no
   agent named, because disconnecting an agent no longer changes who hosts.
   Only JSON for scripts carries a `governance` field.
3. A copy of the data directory holds the key of every goal that computer
   hosts, with no passphrase and no separate file. Accept? **Answered: yes
   (answer 9).**
4. A copy of your data that is older than a goal holds neither the goal nor
   its key. Putting it back loses that goal's host seat for good: nobody
   joins or is removed, the rules cannot change and the goal cannot be ended.
   A backup host will not bring such a goal back either, because the release
   that brings one ends the goals made under v2. Accept, with one line in
   `status` that says so? Recommended: yes; the key belongs with the data.
5. On the host's own computer a changed Locust, or a person who continues
   from an old copy against the warning, can make the host's own agent's
   result count with no approval. Accept? Recommended: yes. A hostile host is
   not assumed (answer 8), continuing is the person's own act, and the other
   members see the result.
6. Disconnecting the agent you started a goal with: should the command only
   say what waits, or refuse while the goal still lacks its first files?
   **Settled by the plan author on 6 October 2026, under answers 7 and 27: a
   disconnected agent can be connected again with one command. So
   disconnecting applies at once, prints the command that undoes it and
   refuses nothing.** For the agent a goal was started with it also says what
   waits until then: the roles only that agent holds, sharing the goal's
   first files, and its consent to a public page.

**The restore guard (G1, G2)**

7. Should the guard hold your own agents as well as the host's records: an
   agent whose own records are missing, and every agent of yours in a goal
   you host while the host's own records are missing? Recommended: yes. It
   costs nothing in ordinary running, and it stops a restored host from
   posting work that counts unapproved or from working in a goal it ended.
   Other members' agents are not held and keep working.
8. After a whole-computer restore or a move, Locust cannot tell how old its
   data is. Should a goal you host wait for one command from you, `locust
   --owner goal continue`, or carry on by itself? **Answered: wait for the
   command. Answer 13 says so for a goal with no other computer to ask. On 6
   October 2026 the owner chose to keep the restore guard as planned, and
   that covers every goal a person hosts (the master plan, "What answer 26
   changes", item 4).** Hearing from the computers an old copy lists cannot
   show that nothing newer exists, so on a host's computer this wait ends
   only on the command. One command with `--all` covers every goal. Until it
   is typed the host's computer keeps collecting what the other computers
   hold, admits nobody and records nothing, and the other members' agents
   keep working. A restore of the data folder alone, with the note beside it
   kept, still ends by itself once the missing records are back.
9. While a goal is catching up, are your own commands in it (remove, change
   rules, add your agent, end) refused with the continue line, before any
   plan is shown? Recommended: refused; the damage is permanent and a
   warning is easy to miss.
10. When Locust finds its data was restored it revokes the pending
    invitations of the goals it hosts. Accept? Recommended: yes; status says
    how many, and you send new ones to the people who had not joined yet.
11. An ordinary restart admits and records at once and waits for no other
    computer. Accept? Recommended: yes; it is what makes a restart cost
    nothing, and the joinable plan's gate follows it.
12. On a member's computer after a whole-computer restore, an agent goes on
    once the host's computer has answered, or all the other members'
    computers. A rare order of events can then cost that agent its key in
    that goal, and the person joins with another agent. Accept? **Settled by
    the plan author on 6 October 2026: the rule stays, with this limit,
    because the restore guard is kept as planned.**
13. Are the words right: "catching up" for the state and `goal continue` for
    the command? `resume` was dropped because the roles plan uses it for
    taking an attempt over. Recommended: yes.
14. Locust keeps its note of what it last signed in a folder beside the data
    folder, `~/.locust.marks`, and not in the system's application-state
    folder. Accept? Recommended: yes; a copy of the data folder then does
    not carry the note, which is how Locust notices an old copy.
15. A copy put back also undoes what you set on this computer since it was
    taken: an agent you disconnected is connected again, and an agent that
    left a goal after the copy was taken reads as a member on this computer
    again, although the other computers have removed it. Locust says so in
    one line and does nothing more. Accept? Recommended: yes for now;
    undoing it would mean keeping those settings beside the data directory
    too.

**Ending a goal (E1, E2)**

16. After the end, may a member still take their name off the page by a
    signed decline that is exempt from the stop? Recommended: no in this
    step. The name leaves with the page, by the host's `farm off` or by the
    service's deletion, and a decline signed before that computer learned of
    the end still blanks the page.
17. Should `goal end` take the public page down by itself? **Answered: no.
    The page stays 30 days, marked ended, and is then removed. The host can
    take it down sooner (answer 23).**
18. When a second record at one position of the host's record cuts an end
    out, should every computer that holds the end keep signing nothing and
    read "ended, then halted"? Recommended: yes; otherwise a host's restored
    copy would restart agents' work in a goal the host ended. Should the page
    of such a goal be blank, read "Ended by the host" and be removed after
    the same 30 days? Recommended: yes.
19. Should `goal end` take a short public note? Recommended: no; the record
    stays bare, and a note can be a later field.
20. Until deleting a goal from one's own computer is built, a member has no
    command that removes an ended goal from their computer, and cannot leave
    it. Acceptable for the first public door? Recommended: yes, said plainly
    in the guide.
21. Should the result of `goal end` say how many members' computers already
    have the end? Recommended: not in this step; the result says to keep the
    computer on, and `goal status` shows when each other computer last
    synchronized.
22. When a member leaves, the host's computer removes it with a record that
    makes no new content key. The member keeps the key it has and is sent
    nothing more, and no command gives the goal a new key until the host
    next removes someone by hand. Accept? Recommended: yes; a member that
    leaves is not an adversary, and a new key at every leave would pause
    every member's writing each time a stranger left a public goal.
23. The 30 days an ended page stays up are counted from the moment the farm
    service is told of the end. That can be later than the end, when the
    host's computer was off or the page was blank. Accept? Recommended: yes;
    only the service can count them, and the plan of `goal end` says so.
