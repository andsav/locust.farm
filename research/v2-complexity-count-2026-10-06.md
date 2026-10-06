# How much v2 adds: a count

Status: research finding, 6 October 2026. A count made by reading the plans
and the code. Nothing was built or run, so every line figure is an estimate
with a low and a high end.

The owner asked how much complexity the [v2 plan](../docs/master-plan.md)
introduces. Five readers each counted one piece of the plan against the code
as it stands, and a sixth took an inventory of the code today. Two more then
attacked the count: one looked for complexity it had missed, one for
complexity that could be cut. Each reader cited the plan or the code for
every item.

## Answer

v2 is a large change. It grows the non-test Rust by about a sixth to two
fifths and the test Rust by a quarter to three fifths.

The growth is not in what a person types. By the first public door Locust has
the same number of API requests as today and three more commands. The growth
is in three places:

1. the states one computer can be in for one goal;
2. what the host's computer signs by itself with nobody present;
3. the rules that every computer must apply the same way.

About half of what v2 adds is fixed by the owner's 25 decisions. About three
tenths is safety work those decisions imply. About two tenths is the plan
author's choice, and roughly half of that can be cut or deferred.

## Today

| Measure | Today |
| --- | --- |
| Rust, non-test | 50,174 lines |
| Rust, test | 50,424 lines |
| Kinds of signed record | 25 |
| Keys that sign in a goal | one kind: each agent's key. The creator's agent key also signs the 7 kinds that set members and rules |
| API requests | 97, of which 63 are offered to agents as tools |
| Commands | 137 |
| Settings for one agent in one goal | 7 switches, so 128 combinations, plus one switch for the whole computer |
| Formal models | 4 (923 lines, 106 cases). None covers sync, invitations, joining or content keys |

Test code here includes test modules inside source files. An earlier figure in
this session, "about a third is tests", counted only test files and was too
low.

## What each piece adds and removes

Lines are Rust only. "Items" counts things on six measures: signed format,
state kept on disk, states, surface (commands, requests, refusals, words),
things the daemon does by itself, and properties that need a model or a test
family.

| Piece | Items added / removed / changed | Non-test lines added | Non-test lines removed | Test lines added |
| --- | --- | --- | --- | --- |
| Roles, the person's side (R1 to R3, R5 to R7) | 32 / 10 / 18 | 4,520 to 6,030 | 2,310 to 3,100 | 3,250 to 4,830 |
| Roles, the goal's side (R4, R8 to R10) | 40 / 10 / 15 | 1,900 to 3,300 | 550 to 900 | 4,200 to 7,800 |
| Signing key, restore guard, ending (K1, G1, G2, E1 to E3) | 55 / 4 / 19 | 2,300 to 3,600 | 350 to 700 | 6,000 to 10,400 |
| Public goals (J0 to J8) | 67 / 5 / 16 | 5,500 to 11,000 | 700 to 1,400 | 5,500 to 10,500 |
| **v2 in all** | **194 / 29 / 68** | **14,200 to 23,900** | **3,900 to 6,100** | **19,000 to 33,500** |
| Replacing a host (after v2, provisional) | about 54 items | 1,800 to 3,800 | 250 to 700 | 2,200 to 4,500 |

Net of removals, v2 adds 8,100 to 20,000 non-test lines to 50,174 and 12,500
to 30,000 test lines to 50,424. The public-goals range is the widest because
its phases are not written.

The same 291 items by measure, added and removed:

| Measure | Added | Removed |
| --- | --- | --- |
| Signed format | 20 | 2 |
| State kept on disk | 16 | 7 |
| States | 39 | 6 |
| Surface | 53 | 12 |
| Done by the daemon itself | 25 | 1 |
| Properties to model or test | 41 | 1 |

About a dozen items appear in two ledgers (the end shown on a page, idle
check-in, the catch-up check before admission, the version numbers), and
renames are counted as lines both added and removed. Both push the totals up
a little.

## Where the complexity lands

**The surface stays flat.** The roles phases take the API from 97 requests to
89. The later phases bring it back to 97: two for roles, one to continue
after a restore, one to end a goal, five for the door, less one for the
integrator. Commands go from 137 to 140. Agent tools fall from 63 to 55. One
agent's setting in one goal falls from 128 combinations to 3 levels.

**The signed format grows modestly.** Kinds of signed record go from 25 to 27
(role holders, the end of a goal). One new kind of key signs members and
rules. The door adds one signed description outside the event log and about
six fields. Replacing a host would add two more kinds of record.

**States grow the most.** A reader estimated the combinations that behave
differently for one computer in one goal: about 20 today; after v2 about 100
on a member's computer and on the order of 1,000 on the host's, before the
door multiplies the host's side again. The plans define behavior for about 20
pairs of states (ended and catching up, halted and ended, and so on) and for
one triple.

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
3. **Decision 18 protects tasks only.** Under the public preset the host's
   agent is the only reviewer and starts at auto. It reviews a stranger's
   result unasked, and R9 then records the change by itself. From a
   stranger's post to a landed file no person is asked.
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
11. **The host safety plan is not yet a document.** Its text is a file in a
    session's scratch folder with 28 fixes still to apply.

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

## What can be cut or deferred

None of these contradicts an owner decision. Together they take out roughly
1,500 to 2,500 non-test lines and 2,000 to 3,800 test lines, about a tenth of
what v2 adds.

| | Change | What a person notices |
| --- | --- | --- |
| Defer | R8, the shared plan settling by itself | In a goal with no lead the plan has no single current text, as today. Shared files still land by themselves |
| Defer | J6, the brief for someone joining a goal in progress | A newcomer's agent reads what every member reads; on a long history its first useful answer takes longer |
| Defer | E3, idle goals dialing less often | Nothing, except a little more network use |
| Defer | The 8-letter short code on the farm page | No code to type by hand; the link and the QR code remain |
| Cut | R3's second copy of the rules: use the trial run the daemon already makes before saving | Nothing |
| Cut | E2's "asked to leave" line under what waits for the host | Nothing: decision 14 removes the member automatically |
| Cut | The two new fields on the check rule | Nothing in any built-in formation |

Examined and kept: the separate record for role holders (decision 7 needs
it), the plan identifier behind `--confirm`, the restore guard's file of
marks (the smaller designs make ordinary restarts ask), and letting the key
for members and rules also sign what the host's computer records by itself.
The last stretches the wording of decision 9, "a key that does nothing else",
and needs the owner's explicit yes.

Two more places where the plans sit against a decision and must change
whatever is cut: E2 against 14, and R8 and R9 near 3, because when two
approved changes build on one version the order of arrival at the host's
computer helps pick the winner.

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
3. The door opening before its protections have an owner (gaps 3, 4 and 5).
4. Building on text that is missing, stale or undecided (gaps 6, 7 and 11).
5. Proof that does not arrive (gaps 9 and 10).

## Limits

The readers were read-only and ran nothing. Other sessions changed the code
while they counted, and the plans were written against earlier commits, so
line-level figures are of that age. The public-goals and replacing-a-host
figures rest on a contract and a research note, not on phases. The shares
(half, three tenths, two tenths) are one reader's weighting.
