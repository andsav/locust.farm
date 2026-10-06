# How much v2 adds: a count

Status: research finding, 6 October 2026. A count made by reading the plans
and the code. Nothing was built or run, so every line figure is an estimate
with a low and a high end.

The owner asked how much complexity the [v2 plan](../docs/master-plan.md)
introduces. Five readers each counted one piece of the plan against the code
as it stood, and a sixth took an inventory of the code. Two more then
attacked the count: one looked for complexity it had missed, one for
complexity that could be cut. What they returned is kept in the
[evidence folder](evidence/v2-complexity-count-2026-10-06/README.md).

An independent reader reviewed this note the same day. It found the design
findings sound and the size figures weaker than they looked, and it found two
accounting slips. This version is corrected for all of it. The section
[How far to trust the numbers](#how-far-to-trust-the-numbers) says what the
figures can carry.

## Answer

v2 is a large change. It adds a fifth to a half again to the Rust in the
workspace, and more than half of the added lines are tests.

A person has fewer settings and about the same number of commands. A person
has more to understand: a computer that is catching up, a goal that is ended
as opposed to halted, a member who came through a public door, and a change
that is recorded for the goal as opposed to the files in their own folder.

Most of the new difficulty is inside the daemon, in three places:

1. the states one computer can be in for one goal;
2. what the host's computer signs by itself with nobody present;
3. the rules that every computer must apply the same way.

The most useful result is not the size. It is the list of problems that sit
between the pieces of the plan, under
[What the count missed](#what-the-count-missed), and the two decisions under
[Two decisions the count exposes](#two-decisions-the-count-exposes).

## How far to trust the numbers

- **Item counts** can be recomputed from the ledgers in the evidence folder.
  What counts as one item was each reader's judgement.
- **Line figures** are estimates. Each reader took the files a phase names,
  their present size, and the size of comparable code. No script reproduces
  them.
- **The baseline moved while it was counted.** Other sessions were changing
  the code. The inventory gives two snapshots: the working tree at the time,
  103,092 lines of Rust, and commit `43f03b4`, 101,443 lines. The plans were
  at `ae85baf`.
- **Test and non-test lines were split two ways.** The readers of the pieces
  counted a line as test code when its file is a test file, which gives
  34,478 test lines today. The inventory also counted test modules inside
  source files, which gives 50,424. So the totals below are firmer than the
  split between test and non-test.
- **The state figures are estimates, not an enumeration.** The dimensions are
  listed below. Nobody listed the reachable combinations.
- **How much is unavoidable is a judgement.** One reader put about half of
  what v2 adds down to the owner's decisions, three tenths to safety work
  those decisions imply, and two tenths to the plan author's choices. The
  readers tagged many items "owner decision" where a decision fixes that a
  thing exists and the author chose its shape. The split should not be read
  as showing that most of the complexity cannot be avoided.

## Today

| Measure | Today |
| --- | --- |
| Rust in the workspace | about 103,000 lines, roughly half of it tests when test modules inside source files are counted |
| Kinds of signed record | 25 |
| Keys that sign in a goal | one kind: each agent's key. The creator's agent key also signs the 7 kinds that set members and rules |
| API requests | 97, of which 63 are offered to agents as tools |
| Commands | 137 |
| Settings for one agent in one goal | 7 switches, so 128 combinations, plus one switch for the whole computer |
| Formal models | 4 (923 lines, 106 cases). None covers sync, invitations, joining or content keys |

## What each piece adds and removes

Lines are Rust only. "Test" here means lines in test files. "Items" counts
things on six measures: signed format, state kept on disk, states, surface
(commands, requests, refusals, words), things the daemon does by itself, and
properties that need a model or a test family.

| Piece | Items added / removed / changed | Non-test added | Non-test removed | Test added | Test removed |
| --- | --- | --- | --- | --- | --- |
| Roles, the person's side (R1 to R3, R5 to R7) | 32 / 10 / 18 | 4,520 to 6,030 | 2,310 to 3,100 | 3,250 to 4,830 | 1,410 to 2,320 |
| Roles, the goal's side (R4, R8 to R10) | 40 / 10 / 15 | 1,900 to 3,300 | 550 to 900 | 4,200 to 7,800 | 500 to 1,050 |
| Signing key, restore guard, ending (K1, G1, G2, E1 to E3) | 55 / 4 / 19 | 2,300 to 3,600 | 350 to 700 | 6,000 to 10,400 | 1,200 to 1,800 |
| Public goals (J0 to J8) | 67 / 5 / 16 | 5,500 to 11,000 | 700 to 1,400 | 5,500 to 10,500 | 400 to 1,300 |
| **v2 in all** | **194 / 29 / 68** | **14,200 to 23,900** | **3,900 to 6,100** | **19,000 to 33,500** | **3,500 to 6,500** |
| Replacing a host (after v2, provisional) | about 54 items | 1,800 to 3,800 | 250 to 700 | 2,200 to 4,500 | 300 to 1,000 |

In all, v2 adds 33,200 to 57,500 lines of Rust and removes 7,400 to 12,600.
The net is 20,600 to 50,000 lines on about 103,000. The public-goals range is
the widest because its phases are not written.

The 291 items by measure:

| Measure | Added | Removed | Changed |
| --- | --- | --- | --- |
| Signed format | 20 | 2 | 15 |
| State kept on disk | 16 | 7 | 6 |
| States | 39 | 6 | 9 |
| Surface | 53 | 12 | 25 |
| Done by the daemon itself | 25 | 1 | 11 |
| Properties to model or test | 41 | 1 | 2 |
| **All** | **194** | **29** | **68** |

About a dozen items appear in two ledgers (the end shown on a page, idle
check-in, the catch-up check before admission, the version numbers), and
renames are counted as lines both added and removed. Both push the totals up
a little.

## Where the complexity lands

**The number of commands stays flat, and that understates what a person must
learn.** The roles phases take the API from 97 requests to 89. The later
phases bring it back to 97: two for roles, one to continue after a restore,
one to end a goal, five for the door, less one for the integrator. Commands
go from 137 to 140. Agent tools fall from 63 to 55. One agent's setting in
one goal falls from 128 combinations to 3 levels, which is a real
simplification.

A count of commands does not measure what a person must understand. v2 asks a
person to understand that a computer can be catching up and what continuing
after a restore risks, that a goal can be ended or halted, that a member came
in by invitation or through the door, and that a change recorded for the goal
is not yet in their own folder. A better measure is the number of decisions
and recovery steps in the journeys people take. The roles plan's Phase 7
already counts journeys; it should also count a restore, an ending and a
public join.

**The signed format grows modestly.** Kinds of signed record go from 25 to 27
(role holders, the end of a goal). One new kind of key signs members and
rules. The door adds one signed description outside the event log and about
six fields. Replacing a host would add two more kinds of record.

**States grow the most.** One reader listed what decides whether one computer
signs in one goal.

- Today: membership (5 values), goal halted or not, own log usable or not,
  creator's key here or not, local agent disconnected or not. That is 80
  combinations, of which the reader judged about 20 behave differently.
- After v2, private goal: membership (5), halt (4), end (4: none, in force,
  held behind a missing record, ended then halted), hold on the host's key
  (3), hold on the agent's key (4), hosted here or not, host's agent
  connected or not, level (3). That is 11,520 combinations. The reader judged
  about 100 reachable on a member's computer and on the order of 1,000 on the
  host's.
- A public goal multiplies the host's side by the door's 12 readings and the
  publisher's 6 states.

The plans define behavior for about 20 pairs of these states and for one
triple.

**The host's computer signs by itself in seven situations.** The signing-key
phase says two. Across all the plans the key for members and rules signs with
nobody present when it:

1. admits a joiner on an invitation;
2. records a role given with an invitation (R4);
3. advances a stage;
4. records an approved change to the shared plan (R8);
5. records an approved change to the shared files (R9);
6. removes a member whose agent signed a leave (decision 14);
7. admits a joiner through the public door (J1).

Three of these do not come out the same if done twice: a recording picks the
lowest identifier among the proposals that have arrived, a removal draws a
fresh content key, and an admission depends on who asked. All seven write to
the one log where a second record at a used position stops membership and
rules for good, and v2 has no way to replace a host.

## Two decisions the count exposes

Both need the owner's explicit answer before the phases they touch are built.

### Arrival order at the host's computer

Decision 3 of the master plan says: "Nothing every computer must agree on is
decided by clock, timeout, arrival order or lowest hash."

The roles plan's Phase 9 says that when several approved changes build on
the same files, the host's computer lands "the one with the lowest identifier
among those it holds complete at that moment, so arrival there can decide."
Phase 8 says the same of two revisions of the shared plan.

Every computer still reaches the same answer, because each follows the one
record the host's computer signed. But arrival order and a lowest identifier
did help choose that record, and the wording of decision 3 forbids both. The
roles plan puts the plan case to the owner as its question 9 and lists the
file case as an assumption. Neither is prominent enough for a conflict with a
numbered decision.

Three ways to settle it:

1. **Accept it and narrow decision 3.** The decision would say that no
   computer decides anything shared from its own clock, arrival order or a
   hash comparison, and that the host's computer may choose among changes
   that each already count, by signing one record. This is what the plans do
   now.
2. **Do not choose.** When two approved changes build on the same version,
   the host's computer records neither. Both authors are told, and one must
   rebuild on the other, or a person picks. Arrival decides nothing. Two
   agents can then block each other until someone acts.
3. **Record both when they do not touch the same files,** where the order
   does not change the result, and fall back to 1 or 2 when they do.

The author of the plans recommends 1, with 3 as a later refinement. It is the
owner's decision.

### What a level allows for work that came through the door

Decision 18 says a task written by a door member always asks first. It
covers taking a task. The plans leave three other steps undefined for a door
member's work:

- **Recorded for the goal.** At level ask or auto an agent reviews results
  without asking: in the roles plan's Phase 3 an agent at ask posts, reviews
  and decides, and asks only before each task. Under the public preset the host's agent
  is the only reviewer. Once it approves a door member's change, the host's
  computer records the change by itself (R9). No person is asked.
- **Applied to a folder.** Bringing recorded files into a person's own folder
  is an explicit command today (`workspace update`). The v2 plans do not say
  which level may run it.
- **Run.** Whatever is in the folder runs when an agent works on a task
  there, including a task the host wrote.

This may be exactly the intended workflow: the host's agent is trusted to
review, and that is what a reviewer is for. Or the owner may want a person
asked at one of these three steps when the work came through the door. The
plans should state, for each step, what read, ask and auto allow.

## What the count missed

The reader who attacked the count found sixteen gaps. Each sits between
pieces, which is why no single ledger lists it. The larger ones:

1. **The restore guard's release rule does not work for a public goal.**
   After a whole-computer restore the host's computer signs again once it has
   heard from every other computer in the goal. A door member who uninstalled
   without leaving never answers, so the hold never ends and the override
   becomes the usual path.
2. **Decision 14 has no design, and E2 says the opposite.** Removing a member
   automatically means drawing a new content key in the background. No phase
   says what the restore guard, an ended goal or a halt do to that, or what a
   restored host does when it repeats the removal with a different key. With
   a door, strangers can drive it: each join and leave costs one admission,
   one removal and one new content key for everyone.
3. **A door member's work can be recorded with no person asked.** See the
   second decision above.
4. **The always-ask check for door members' tasks has no phase.** R3 and R5
   were written without it, and the rule for subtasks and revised rounds is
   unwritten. R3 also writes a record for each refused start and never clears
   it, so strangers opening tasks can grow that record without bound.
5. **The owner's default rule fails the door's own safety check.** A goal
   made with no flags cannot be made public. The lone-member and first-files
   exceptions that R4 builds get a new condition (how the member came in)
   that arrives with the door, after the record the master plan calls the
   last change of the event format.
6. **R8, R9, R10 and the door wait on the takeover record's shape,** which
   belongs to the piece outside v2 and still needs a design round.
7. **R4, R8 and R9 are stale against K1.** They still describe the host's
   agent as the signer. R4's bound on how far back the host's agent can
   anchor an unapproved result rests on that agent's log holding every record
   for members and rules, which K1 moves to another key one step earlier.
8. **R3 writes the replay rules a second time** so that a refusal can be
   given before signing, guarded by one test of seven rows. The replay code
   has 51 places that exclude or refuse. Every later phase that adds a replay
   rule must add its twin, and no phase lists that duty.
9. **"Each piece is modelled before it is built" is not sized.** One new
   model is planned, estimated at a day, and it needs the repository's first
   liveness property. First files, one recording per version across restarts,
   door admission, the join safety check and automatic removal get tests
   only.
10. **Qualification that needs people and machines is not sized:** a
    comprehension test with five or more people, restores with Time Machine
    and Migration Assistant on two file systems, real agents of different
    owners on 8 and on 16 computers, and a measured member ceiling that
    decides whether the door ships.
11. **The host safety plan was not yet a document** when the count was made.
    It was assembled later the same day as the
    [host safety and ending plan](../docs/host-safety-and-ending-plan.md),
    with its 28 fixes applied. Gap 7 was partly closed at the same time, when
    that plan's consequences were written into the roles plan.

The smaller ones:

- An end cut by a fork, or held behind a missing record, has no page state:
  the public page reads open while nothing is signed, and the 30 days of
  decision 23 never start.
- The contract still closes the door when the host's agent is disconnected;
  K1 keeps admitting.
- A joiner can get two different answers for one state, with two retry
  speeds.
- A computer that was removed is never told, so after a restore it waits on a
  host that refuses it.
- A restored copy brings back old levels and task allowances. An agent the
  person had lowered to read can come back at auto.

The count was corrected in these places: rule lookup by position has 22 call
sites, not ten; state kept per goal has ten kinds today, not eight; cutting
the `public` preset would leave a lone host only `directed`, so it is not
cuttable; the freeze when the host's agent is disconnected is narrowed, not
removed.

## What would reduce the complexity that matters

The independent reader's recommendation, which the author of the plans
accepts: keep the simplifications for the person, and bring the seven
unattended signings under one written contract. For each of the seven the
contract states:

- the authority: which key signs, under which rule;
- the retry identity: what makes a second attempt produce the same record at
  the same position;
- the durable writes: what is written before and after signing, and in which
  order;
- the behavior while the computer is catching up, while the goal is halted,
  and after the goal is ended.

Every later phase that adds an unattended signing then has one rule to meet.
A design round on the seven and on what protects each was started on 6
October 2026. Settling the two decisions above belongs with it. Removing
lines is secondary to this.

## What can be cut or deferred

These cuts are at the edges. None contradicts an owner decision. Together
they take out roughly 1,500 to 2,500 non-test lines and 2,000 to 3,800 test
lines, about a tenth of what v2 adds. None has been applied.

| | Change | What a person notices |
| --- | --- | --- |
| Defer | R8, the shared plan settling by itself | In a goal with no lead the plan has no single current text, as today. Shared files still land by themselves |
| Defer | J6, the brief for someone joining a goal in progress | A newcomer's agent reads what every member reads; on a long history its first useful answer takes longer |
| Defer | E3, idle goals dialing less often | Nothing, except a little more network use |
| Defer | The 8-letter short code on the farm page | No code to type by hand; the link and the QR code remain |
| Cut | R3's second copy of the rules: use the trial run the daemon already makes before saving | Nothing |
| Cut | E2's "asked to leave" line under what waits for the host | Nothing: decision 14 removes the member automatically |
| Cut | The two new fields on the check rule | Nothing in any built-in formation |

Two of these carry conditions.

- **Deferring R8 does not remove all of it.** R9 uses four things R8 builds:
  the function that works out which recordings are due, the step type, the
  stall reported when the recorder is elsewhere, and the rule that an
  automatic act reads no level. They would move into R9. The saving is what
  is left after that, and it has not been sized separately.
- **Using the one rules check has two conditions.** The refusal must still
  say why, which means tagging the replay code's exclusions with reasons a
  person can read. And nothing rejected may escape: the trial must run on the
  record before it is signed, so that a rejected record is never signed,
  marked or sent. This is the most promising structural simplification of
  the seven.

Examined and kept: the separate record for role holders (decision 7 needs
it), the plan identifier behind `--confirm`, the restore guard's file of
marks (the smaller designs make ordinary restarts ask), and letting the key
for members and rules also sign what the host's computer records by itself.
The last stretches the wording of decision 9, "a key that does nothing else",
and needs the owner's explicit yes.

One more place where the plans sit against a decision and must change
whatever is cut: E2 against 14.

## Where v2 makes Locust simpler

- Eight switches and a per-round task authorization become one level and one
  task allowance.
- Two overlapping ways to set permissions become one.
- Three views (`inbox`, `permission inspect`, the grant rows) become plain
  `status`.
- Joining has one path and one way to confirm.
- The integrator role, its command, request and tool go.
- The viewer credential goes.
- A goal has one way to be over, where today a close stops nothing.
- A forked review by the creator's agent no longer halts the goal.
- A public page needs no separate consent record per member.

## Risks, most serious first

1. A permanent halt caused by the host's own computer: seven unattended
   signings, three of them not repeatable, protected by a guard whose release
   rule rests on sync behavior that was read and not run.
2. Computers disagreeing on what counts: six phases from four documents edit
   the same few functions, each written against a different commit.
3. The door opening before the two decisions above are made and before the
   always-ask check has a phase (gaps 3, 4 and 5).
4. Building on text that is stale or undecided (gaps 6 and 7).
5. Proof that does not arrive (gaps 9 and 10).

## Limits

The readers were read-only and ran nothing. Other sessions changed the code
while they counted, and the plans were written against earlier commits, so
line-level figures are of that age. The public-goals and replacing-a-host
figures rest on a contract and a research note, not on phases. The
independent reader checked the surrounding plans and the signing and commit
code; it did not reproduce the inventory.
