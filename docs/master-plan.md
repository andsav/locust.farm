# Locust v2: master plan

Status: being assembled, 6 October 2026. Proposed; nothing here is built.
This is the one document the owner approves. The owner calls this work v2.
Here v2 means everything in this plan up to and including the first public
door: the sixteen phases of the build order and the public-goals phases.
Replacing a host comes after v2. The name is the product's; the API, store
and protocol numbers below are separate. It holds every decision the
owner has made, what is assumed until the owner objects, the pieces of work
and their state, and the build order. File-level detail stays in the plans it
links to. Two parts are still to come and are marked: the summary of the
public-goals phases, and the final check of this document against all the
plans.

## What is being built

Locust lets several people's coding agents work on one goal. Each person runs
Locust on their own computer, and every member's computer keeps a full signed
copy of the goal. Four pieces of work change how that behaves.

1. **Roles and permissions.** A goal is a shared board that agents organize
   themselves on. A result counts when the goal's rule is met. The host
   decides who is in and what the rules are. Each person sets one level per
   agent per goal. A short list of commands is the person's alone.
2. **Host safety and ending a goal.** A goal's membership and rules are
   signed by a key of their own. A computer started from an old copy of its
   data catches up before it signs. A host can end a goal.
3. **Public goals.** A host can give a goal a page and open a door that
   strangers join through. They can work; they cannot make work count.
4. **Replacing a host.** A host may name one backup host who can take over if
   the host's computer is lost.

## Decided by the owner

These are the owner's answers of 5 and 6 October 2026. Nothing else in any
plan is accepted.

Principles:

1. "User ergonomics with no friction are the most important." Where two
   sound designs differ in what they ask of the person, the one that asks
   less wins. Protective features are optional and nothing prompts for them.
2. No migration. No release reads goals made under an earlier signed format.
   A later change of signed format ends the goals made before it, and the
   texts say so.
3. No computer decides anything shared from its own clock, a timeout, the
   order records arrived in or a comparison of identifiers. The host's
   computer may choose among changes that each already count, by signing one
   record that every other computer follows: when several approved changes
   build on the same version it records the one with the lowest identifier
   among those it holds, and the other authors rebuild. (Narrowed by the
   owner on 6 October 2026. It first read: "Nothing every computer must
   agree on is decided by clock, timeout, arrival order or lowest hash.")
4. One master plan that the owner approves, with detail in the plans it
   links to.

When work counts:

5. A new goal follows peer approval: a result counts when another member
   approves it. While a goal has only one member, that member needs no
   approval.
6. A member's latest review of a result is the one that counts. What was
   already recorded on an earlier approval is not undone.

The person's commands:

7. A person-only command asks for confirmation only when it shares something
   or is hard to undo: starting or joining a goal, inviting, removing a
   member, changing rules, publishing, ending. Setting a level, allowing a
   task and giving or taking a role apply at once.

The host:

8. The failure to design for is a host that disappears. A hostile host is not
   assumed. Hostile members are the host's to remove.
9. A goal's membership and rules are signed by a key that does nothing else,
   separate from the host's working agent. It is kept in the Locust data
   folder with no passphrase.
10. Agreement among several people is used only when a host is replaced.
    Ordinary changes to members and rules stay one signature with no waiting.
    A takeover fixes the last host record its signers hold; whatever the old
    host signed after it is void, even if it appears later.
11. Who may replace a host is a rule the host writes in advance. First
    version: the host may name one backup host, who can take over alone.
    Naming one is optional and nothing asks for it. A threshold among several
    named people is a later step.
12. If a host removes their backup host and the backup takes over without
    having heard, the host's removal wins. The takeover is void when the
    removal surfaces.
13. After a whole-computer restore or a move, for a goal with nobody else to
    ask, Locust signs nothing there until the person runs one command.
14. A member whose agent signs a leave is removed by the host's computer
    automatically.

Shared files:

15. There is no integrator role. An approved change is recorded by the host's
    computer by itself. File acceptance rests on the host's computer before a
    backup host exists.
16. The first files a host shares need no approval. Every later change
    follows the goal's rule.

Public goals:

17. The name a joiner chooses at the door is the name the page shows. It is
    required at a public door. There is no separate consent step.
18. A task written by someone who came through the public door always asks
    first. Level auto covers tasks written by the host and by members the
    host invited, never a door member's.
19. A host may give a deciding role to, or name as backup host, someone who
    came through the door, with one explicit command. Joining alone gives
    nothing.
20. A farm listed in the public gallery may be open or by request. The host
    chooses.
21. The farm service may help a host's computer notice that it was restored
    from an old copy. Private goals never contact the service.
22. The first public door is released after the restore guard and after the
    first step of ending a goal, and before the backup host. The page and the
    host's plan say that goals made now end at the next change of signed
    format.
23. When the host ends a public goal, its page stays, marked ended, for 30
    days and is then removed by the farm service. The host can take it down
    sooner.
24. On the farm page, the Join band folds away once a visitor has joined or
    dismissed it.

The name:

25. What this plan builds is called v2.

Added on 6 October 2026:

26. "I don't want the swarm to stop to ask a human for anything." Agents
    decide among themselves. What a person does is start, join, invite,
    remove, set rules, publish and end, and the work never waits on those.
    This answer puts three things in the plans in question; they are listed
    under [Decisions still needed](#decisions-still-needed).

## Assumed until the owner objects

Each of these is the plan author's recommendation and is written into a plan.
The first group is listed in full in the
[roles plan](roles-and-permissions-plan.md) under "Decisions this plan
assumes" and "Questions for the owner".

- Levels are named read, ask and auto. The built-in formations use two role
  names, `reviewer` and `lead`, and the preset `coordinator` is renamed
  `directed`.
- A role needs no yes from the member's person. A role name keeps its kind
  for the life of a goal.
- A task allowance ends when the task is finished or the host revises it.
- A review can be recorded under `open` as an opinion that does not affect
  counting.
- The shared plan settles by the same rule as the shared files, under every
  formation that names no decider.
- `review-panel` does not count a lone member's results; it asks for two
  named reviewers.
- Raising an agent to auto and giving a role apply at once, although a task
  already started keeps running and what a member signed while holding a role
  stays valid.
- `farm off`, `goal leave`, `task revise`, `agent revoke` and `workspace
  connect` ask first, because no single command undoes them. `invitation
  revoke` applies at once.
- `workspace init` shares the first files in the same run, after one yes.

From the [host safety and ending plan](host-safety-and-ending-plan.md):

- No text a person reads names the signing key. Status says "Host: you".
- The host cannot yet remove the agent they started a goal with. It can be
  disconnected and the goal keeps running.
- The restore guard also holds the host's own agents while the host's records
  are missing, and refuses the person's own commands in that goal until it
  has caught up.
- On finding that its data was restored, Locust revokes the pending
  invitations of the goals it hosts.
- The words are "catching up" for the state, `goal continue` for the command
  and "ended by the host" for an ended goal.
- A goal with no new records dials and checks in less often, down to once an
  hour, by the computer's own clock.
- Each piece is modelled under `research/tla` before it is built.

For public goals, the remaining recommendations are in the
[rewrite contract](../research/joinable-farms-rewrite-contract-2026-10-05.md)
and its
[review](../research/joinable-farms-rewrite-contract-review-2026-10-05.md).
The contract is being revised against that review and the answers above.

## The pieces and their state

| Piece | Document | State |
| --- | --- | --- |
| Roles and permissions | [roles-and-permissions-plan.md](roles-and-permissions-plan.md), with its [companion](roles-and-permissions-plan-details.md) | Ten phases written. An [independent review](../research/roles-and-permissions-plan-review-2026-10-05.md) is folded in. Each phase was checked against the code by a second reader. |
| Host safety and ending a goal | [host-safety-and-ending-plan.md](host-safety-and-ending-plan.md), with its [companion](host-safety-and-ending-plan-details.md) | Six phases written and checked: the signing key (K1), the restore guard (G1, G2) and ending a goal (E1, E2, E3). A reader found 28 seams between them; the fixes are applied, and their consequences are written into the roles plan. E2 is being revised for answer 14. Four known gaps are listed at its top. |
| Public goals | [joinable-farms-plan.md](joinable-farms-plan.md) | The old plan. About half its text survives. It is rewritten as nine phases, J0 to J8, once the contract is revised. |
| Replacing a host | [research note](../research/replacing-a-host-2026-10-05.md) | The first version is decided (answers 11, 12 and 19). How a takeover works needs one more design round before it becomes phases. |

Evidence the plans rest on:
[host-key failures](../research/host-key-failure-characterization-2026-10-05.md),
[goal lifecycle](../research/goal-lifecycle-characterization-2026-10-05.md),
[three real agents on a board with no roles](../research/role-free-board-2026-10-05.md)
and [ending a goal](../research/ending-a-goal-2026-10-05.md).

## Build order

Proposed by the reader who compared the pieces. Nothing is released before
the last of these sixteen phases. R is the roles plan, K the signing key, G
the restore guard, E ending a goal.

| | Phase | What works afterwards | Needs |
| --- | --- | --- | --- |
| 1 | R1 | Starting, joining, leaving, inviting and every change to members or rules are the person's requests and need no grant | nothing |
| 2 | R2 | One grammar for the person's commands; a command asks first only when it shares something or cannot be undone with one command | R1 |
| 3 | R3 | One level per agent per goal, one task allowance, one check that says which side refused | R1, R2 |
| 4 | K1 | A goal's members and rules are signed by a key of their own; the agent that started the goal is an ordinary member | R1 to R3 |
| 5 | R4 | Members have names; roles are read when an act happens; peer approval is the default, a lone member needs none, a member's latest review counts, first files need no approval | R1 to R3, K1 |
| 6 | R5 | Plain `status` is the one view; refusals read the same to a person and to an agent | R2 to R4 |
| 7 | R6 | Guides, site, skill and scripts say what the code does | R1 to R5 |
| 8 | G1 | A computer knows what it signed; started from an older copy, it signs nothing in the affected goals until it has caught up | R1, K1 |
| 9 | G2 | Status and refusals say "catching up"; the person has one command to continue | G1, R2 to R6 |
| 10 | E1 | The host ends a goal with one command; nothing new is recorded on any computer that has learned of it | R1 to R6, K1, G1, G2 |
| 11 | E2 | The host sees who asked to leave; disconnecting the host's agent is explained, not refused | E1 |
| 12 | E3 | A goal with no new records dials and checks in less often | G1 |
| 13 | R7 | The recipes pass, the journeys are counted, an unprompted swarm run is recorded, people are tested on the explanation | all above |
| 14 | R8 | The shared plan settles by itself | R3, R4, K1, G1, G2, E1, and the takeover record's shape decided |
| 15 | R9 | A change to the shared files lands by itself | R4, R8, G1, G2 |
| 16 | R10 | The finished workflow is qualified | all above |

Afterwards:

- **Public goals.** J0 can land at any time. The first working admission
  needs R5, K1, G1 and G2. The first door is released after R10 and E1.
- **Replacing a host.** It follows v2. Under answer 2 it ends the goals made
  under v2.

Versions. The API version and the store marker go from 6 to 7 in R1. The
protocol version goes from 6 to 7 in K1. R4 and E1 change signed bytes inside
7, and E1's end record is the last change of the event format. Nothing is
released before R10, so no number is raised twice. Replacing a host takes
protocol 8.

## Size

Counted on 6 October 2026 by reading the plans against the code; nothing was
built. The full count is in
[how much v2 adds](../research/v2-complexity-count-2026-10-06.md).

- v2 adds 33,200 to 57,500 lines of Rust and removes 7,400 to 12,600, on
  about 103,000 today. More than half of the added lines are tests. These
  are estimates from reading; no script reproduces them.
- The number of commands stays about the same: 97 API requests before and
  after, 137 commands before and 140 after. One agent's setting in one goal
  goes from 128 combinations to 3 levels. A person has more to understand,
  though: catching up, ended as opposed to halted, a member who came through
  the door, and a change recorded for the goal as opposed to the files in
  their own folder.
- The growth is in the states one computer can be in for one goal, in what
  the host's computer signs by itself (seven situations across the plans),
  and in the rules every computer must apply the same way.
- The count names seven cuts and deferrals at the edges, worth about a
  tenth of the added code. They are proposed, not applied. An independent
  reader of the count judged that bringing the seven unattended signings
  under one written contract matters more than removing lines.

## Decisions still needed

Answer 26 puts these in question. In the plans, "asks first" has always
meant that an agent stops and waits for its own person.

1. **A task written by a door member (answer 18).** As recorded, such a task
   waits for the person of each agent that would take it, at every level.
   That stops the swarm for a human. What replaces it is not decided. Blocks
   the public preset and the check that R3 and R5 lack.
2. **The level a joining agent gets.** `goal join` requires `--level`, and
   its examples use ask, where the agent waits for its person before each
   task. Under answers 1 and 26 the level would default to auto, with ask and
   read as choices a person makes.
3. **After a whole-computer restore with nobody else to ask (answer 13).**
   Locust waits for one command from the person. The owner chose this on 6
   October 2026 before giving answer 26; it is rare and stays unless the
   owner says otherwise.

Not the owner's to decide, and no longer put to the owner: whether the key
for members and rules may also sign what the host's computer records by
itself. A person sees no difference. It is settled with the contract for
unattended signing, and the wording of answer 9 follows what that settles.

## Not designed yet

- **How a takeover works.** The reviewers of the earlier design round agreed
  on a skeleton and left competing fixes for its hardest problems: someone
  once named backup starting from before their own removal, two takeovers
  freezing a goal, removals the backup never received, telling the old host,
  and issuing a fresh content key. Answer 12 settles the first. The rest need
  a design round, an attack on its result and a formal model.
- **The public-goals phases.** The contract must be revised first; its review
  found that it contradicted answers 18 and 22 and admitted strangers one
  phase before the safety check.
- **Found by the count, with no phase that owns them.** The restore guard's
  release rule does not work for a public goal, because a door member who
  never returns blocks it. Answer 14 has no design and E2 says the opposite.
  Answer 18 covers tasks only: under the public preset the host's agent at
  auto reviews a stranger's result and the change is then recorded with no
  person asked. The check behind answer 18 is in no phase. The default rule
  of answer 5 fails the door's safety check, so a goal made with no flags
  cannot be made public. R4, R8 and R9 still describe the host's agent as
  the signer. R8, R9, R10 and the door wait on the takeover record's shape.
  The models and the qualification runs that need people and several
  computers are named and not sized.
- **Left out on purpose for now:** deleting a goal from one's own computer,
  sealing an ended goal against records signed before the end, telling a
  computer that it was removed, a threshold among several named people for
  replacing a host, and exclusive claims on a task.

## How to review

Two of the four pieces are written as phases and can be reviewed now: roles
and permissions, and host safety and ending a goal. In the second, phase E2
is being rewritten for answer 14. Public goals exists only as a contract
that is marked for revision, so it can be reviewed for scope and not for
detail. Replacing a host follows v2 and is a research note.

1. The decisions and assumptions above.
2. In the roles plan and in the host safety plan, "Intended behavior" and the
   terminal texts: they are what a person will see.
3. The build order and the size.
4. Phase detail only where something looks wrong. Each phase was read
   against the code by a second reader, but nothing was built or run, so
   file-level claims are carefully read, not proven.
