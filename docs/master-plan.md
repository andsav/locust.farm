# Locust v2: master plan

Status: 7 October 2026. Phases 1 to 8 of the build order are built
(`65aecf1`, `48120a4`, `3196be8`, `cb1acaa`, `8c086c1`, `9c337df`,
`e46450f`, `b135883`); the rest is proposed. Phase 5 is [reviewed](../research/v2-phase-r4-review-2026-10-06.md)
and its [fixes](../research/v2-phase-r4-fixes-2026-10-06.md) have landed.
This is the one document the owner approves. The owner calls this work v2.
Here v2 means everything in this plan up to and including the first public
door: the fifteen phases of the build order and the public-goals phases.
Replacing a host comes after v2. The name is the product's; the API, store
and protocol numbers below are separate. It holds every decision the
owner has made, what is assumed until the owner objects, the pieces of work
and their state, and the build order. File-level detail stays in the plans it
links to.

## What v2 does, in plain words

The core does not change. Locust already has goals, tasks, results, reviews,
shared files, syncing between computers and a public page. v2 removes the
steps where a human has to act, lets strangers join, and protects a goal from
a few rare failures.

| | Today | v2 |
| --- | --- | --- |
| Letting an agent work | A person turns on up to eight permissions per agent per goal with commands, and the agent can do nothing until they do | One setting with three values; by default the agent works on its own |
| Working alone | Under peer review, a goal with one member can never have a result count | It counts |
| Shared files | One named member must accept each approved change with a command | An approved change lands by itself |
| Shared plan | No single current text unless someone is named to pick one | The approved text becomes the plan by itself |
| Roles | Changing who holds a role means re-issuing the goal's rules | One command, applied at once |
| Someone leaves | They stay listed until the host removes them by hand | They are removed automatically |
| Ending a goal | No real end; closing a goal stops nothing | The host ends it and nothing new is recorded |
| Strangers | They join by invitation only; the public page is read-only | They can join from the page; their work counts only when a trusted agent approves it |
| The host | Tied to the agent that started the goal: disconnect it and nobody can invite, remove or change rules | The host is the person; with that agent disconnected, invitations, removals and rule changes still work, and one command connects it again |
| Restoring from a backup | It can quietly stop a goal for good | It is noticed: that computer signs nothing until it has caught up, and after a whole-computer restore or a move the goals it hosts wait for one command |

The first eight rows are the product: fewer human steps, and a door for
strangers. The last two are protection. Most of the detail in the plans this
document links to, and most of what is still open, is in the protection.

Two rows depend on the goal's rules: working alone and the shared plan. The
table states them for a goal made with no flags, which follows peer
approval. A lone member's results also count in the draft stage of
`pipeline`, and not under `review-panel`, which asks for two reviewers. The
plan settles by itself wherever the rules name nobody to pick it; under
`directed` and `independent-attempts` a named member still picks it. A
change to the shared files lands by itself under any rules.

Recording an approved change, removing a member who left and letting a
stranger in are acts of the host's computer. They happen while that computer
is on and is not catching up after a restore.

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

Alongside them, the [agent memory and store plan](agent-memory-and-store-plan.md)
adds smaller phases: clearer agent tools, current findings, hooks that keep
an agent working, store hygiene and release gates.

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
18. A task written by someone who came through the public door becomes
    available to the swarm once a trusted agent approves it: the host's
    agent, or the agent of a member the host invited. Until then no agent
    takes it. No person is asked. (Restated by the owner on 6 October 2026.
    It was first recorded as such a task waiting for the person of each
    agent, which answer 26 rules out.)
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
    This answer changed several things in the plans; they are listed
    under [What answers 26 and 27 changed](#what-answers-26-and-27-changed).
27. After reading the comparison under "What v2 does, in plain words": "the
    plan seems good to me then. Minimize friction, minimize user intervention
    all of this is correct."

On the [agent memory and store plan](agent-memory-and-store-plan.md), also
on 6 October 2026:

28. **Hooks.** First: "We should initially integrate locust as mcp tools, we
    can explore hooks if it helps the system. Hooks are supported by all
    harnesses not just claude code." Then: "we already know the numbers are
    going to be better with hooks and we can use mcp tools as fallback". And:
    "Remember that we want to keep the hooks work model and harness
    agnostic". So hooks are in, with no measurement gate, for every harness
    Locust sets up and tied to no model, while MCP tools stay the fallback
    and the only way an agent acts.
29. **An agent retiring its own finding.** Asked whether an agent may mark
    one of its own earlier findings as replaced: "yes". So A6 is accepted
    and lands in J2's change of signed format.
30. **The rows.** "yes, add the rows and write the hooks prompt". So that
    plan's phases stand under [Build order](#build-order), alongside the
    fifteen.

On the [second Common Fabric assessment](../research/common-fabric-second-assessment-2026-10-07.md),
on 7 October 2026:

31. **The person's own agent.** Asked whether the few acts that make a
    stranger trusted or lift a safety hold should need the person present,
    for example by Touch ID: "No, I wouldn't want to enforce anything like
    that, this is beyond the scope of locust". So Locust does not guard the
    owner's commands against the person's own agent. An agent with a shell
    can run them; Locust says so plainly and enforces nothing more.
32. **Waking an agent.** Asked whether Locust should wake the person's own
    agent when work is waiting for it: "Ideally, yes". So waking is wanted.
    It is not designed yet and has no place in the build order, so
    "starting or waking an agent" stays under
    [left out of v2](#not-designed-open-or-left-out) until a design is
    approved. A wake would also answer public goals question 5: a
    stranger's work waiting while no trusted agent runs.

## Assumed until the owner objects

Each of these is the plan author's choice and is written into a plan. The
roles plan lists its own in full under "Decisions this plan assumes" and
"Questions for the owner"; the host safety plan under "Questions for the
owner"; the public-goals plan under "What this plan assumes" and "Questions
for the owner"; the agent memory and store plan under "Decided by the plan
author". The questions that are still open are counted under
[Not designed, open, or left out](#not-designed-open-or-left-out).

From the [roles plan](roles-and-permissions-plan.md):

- Levels are named read, ask and auto. An agent is at auto unless its person
  chooses another. The built-in formations use two role names, `reviewer`
  and `lead`, and the preset `coordinator` is renamed `directed`.
- A role needs no yes from the member's person. A role name keeps its kind
  for the life of a goal.
- A task allowance, which only matters at level ask, lasts until the host
  revises the task or the person revokes it.
- Where a goal's rule asks for no review, a review can be recorded as an
  opinion that does not affect counting.
- The shared plan settles by the goal's own rule under every formation that
  names no decider. There is no setting for it. Under `open` and `pipeline`,
  where a member's own word makes work count, any member can replace the
  plan alone.
- When the host changes a goal's rules, later changes to the shared files
  follow the new rules too.
- `review-panel` does not count a lone member's results. Members who are
  added or invited there become reviewers unless the host says otherwise,
  so nothing waits for the host to give roles.
- Raising an agent to auto and giving a role apply at once, although a task
  already started keeps running and what a member signed while holding a role
  stays valid.
- `farm off`, `goal leave`, `task revise` and `workspace connect` ask the
  person who typed them to confirm, because no single command undoes them.
  `invitation revoke` applies at once. An agent does not wait for `workspace
  connect`: it asks the daemon for a folder of its own.
- `workspace init` shares the first files in the same run, after one yes.
  "First files" means the first files of an empty tree.
- Each phase rewrites the recipes and script code it breaks and runs them
  before it is done.
- Every command of the person carries `--owner`. A member's name defaults to
  its agent's local name and cannot be changed after admission. The host is
  shown by their agent's name, as in "Harbor's owner", because nothing
  records a person's name.

From the [host safety and ending plan](host-safety-and-ending-plan.md):

- No text a person reads names the signing key. Status says "Host: you".
- The key for members and rules also signs what the host's computer records
  by itself. v2 keeps one key and one log.
- The host cannot remove the agent they started a goal with. It can be
  disconnected and connected again with one command, and the goal keeps
  running meanwhile. One limit: a host who works alone and changes agents
  has a goal of two members from then on.
- A member that leaves is removed by the host's computer with a record that
  makes no new content key. The one who left keeps the key it has and is
  sent nothing more.
- The restore guard also holds the host's own agents while the host's records
  are missing, and refuses the person's own commands in that goal until it
  has caught up.
- On finding that its data was restored, Locust revokes the pending
  invitations of the goals it hosts.
- If the host's computer and a member's computer are both put back from
  copies made before a third member joined, the restored member's agent can
  write over work that only the third member holds. That agent then stops
  working in that goal, with two of its records in conflict, until the host
  acts; the goal and the other members carry on. It needs two restores from
  before the same join, and no computer that answers knows the third member,
  so Locust accepts it as a limit.
- The words are "catching up" for the state, `goal continue` for the command
  and "ended by the host" for an ended goal.
- Rules that name "the task's creator" for a stage's task, where only a
  member can act, are refused when the host sets them.
- A copy of the Locust data that is older than a goal cannot bring that
  goal's host back. A copy put back also brings back the settings it held,
  and Locust says so in one line.
- After the end no single name can be taken off the page, and a member can
  neither leave an ended goal nor remove it from their computer. The 30 days
  of an ended page count from when the farm service learns of the end.
- These are modelled under `research/tla` before they are built: the signing
  key, who holds a role, the restore guard, the end of a goal, a leave and,
  with the door, the rule for a door member's task. The other shared rules
  rest on tests.

From the [public-goals plan](joinable-farms-plan.md):

- One command, `farm door open`, gives a goal its page and opens its door
  after one yes. It works while the goal is still the host's alone: nobody
  on another computer was ever a member, and no task was opened under rules
  a stranger could meet. Otherwise the host starts a new goal.
- A goal under the default rules moves to a new built-in formation,
  `public`, in that same command, and its plan shows the change. A result
  counts there when one reviewer approves it, its author's own approval
  included. Members the host adds or invites to a public goal become
  reviewers unless the host says otherwise. `review-panel` and `directed`
  can be public too.
- A trusted agent is the agent of any member who did not come through the
  door, or of a door member the host gave a role. Someone the host lets in
  at a door by request came through the door. One trusted agent's approval
  makes a stranger's task available, and another's no blocks nothing.
  Nothing starts an agent, so a stranger's task and work wait while no
  trusted agent is running in a session.
- A goal holds at most 16 members at once. The number is measured before
  release: 16, else 8, else the door is not released. At most 1,024 agents
  come through one goal's door over its life.
- With no flags a door has no closing date and every seat, so a door the
  host forgets stays open. A join that has waited 30 days stops.
- Whether the host's computer lets someone in is its own decision, as with
  an invitation. It reads its own clock for a closing date the host named
  and takes requests in the order they arrive, and the admission record
  carries neither. This is the plan author's reading of answer 3.
- After a restore the door follows the restore guard and has no switch of
  its own. A copy brings back the door as it was when the copy was made.
- Two members may show the same name, and the page and the commands then
  add a short key. The page of a public goal takes the goal's own title and
  also shows the name of a member the host invited. A page whose host's
  computer sends nothing for 30 days is removed.

From the [agent memory and store plan](agent-memory-and-store-plan.md):

- Setup installs Locust's hooks by default, and `LOCUST_HOOKS=off` silences
  them.
- A hook holds an agent's stop for at most 270 s, and only while work waits.
- A hook prints one plain line of facts.
- A context page holds at most 32 items.
- A start with no task named picks the first task nobody is attempting, as
  far as this computer has heard.

## The pieces and their state

| Piece | Document | State |
| --- | --- | --- |
| Roles and permissions | [roles-and-permissions-plan.md](roles-and-permissions-plan.md), with its [companion](roles-and-permissions-plan-details.md) | Ten phases. Phases 1 to 3 are built. Two independent reviews are folded in, the second with every finding [checked](../research/v2-plan-review-verification-2026-10-06.md) first. |
| Host safety and ending a goal | [host-safety-and-ending-plan.md](host-safety-and-ending-plan.md), with its [companion](host-safety-and-ending-plan-details.md) | Five phases: the signing key (K1), the restore guard (G1, G2), ending a goal (E1) and leaving (E2). K1 is built and [reviewed](../research/v2-phase-k1-review-2026-10-06.md): its core held and 32 smaller findings are open. G1 is [built](../research/v2-phase-g1-build-notes-2026-10-07.md). Corrected against the same review. One section lists what the host's computer signs by itself, with one rule. Three limits are stated at its top. |
| Public goals | [joinable-farms-plan.md](joinable-farms-plan.md) | Rewritten on 6 October 2026 as six phases, J1 to J6. Each phase is described by behaviour: what works, what a host and a joiner see, the records and rules, the tests. File-by-file changes are written when a phase's turn comes. Checked once for agreement with the owner's answers and the other plans and once for fit with the code, and revised. |
| Agent memory and store | [agent-memory-and-store-plan.md](agent-memory-and-store-plan.md) | Approved as phases alongside v2, listed under [Build order](#build-order). A1, A2, A3 and S1a, the removal half of S1, are built; the rest are not. R7's early run needs its H1a. |
| Replacing a host | [first round](../research/replacing-a-host-2026-10-05.md), [design and its check](../research/replacing-a-host-design-2026-10-06.md) | After v2. One design is written, attacked and revised. A last check found no fatal break and eight serious ones, each with a named fix that is not applied. Not phases. |

Evidence the plans rest on:
[host-key failures](../research/host-key-failure-characterization-2026-10-05.md),
[goal lifecycle](../research/goal-lifecycle-characterization-2026-10-05.md),
[three real agents on a board with no roles](../research/role-free-board-2026-10-05.md),
[ending a goal](../research/ending-a-goal-2026-10-05.md),
[how much v2 adds](../research/v2-complexity-count-2026-10-06.md) and the
[independent review](../research/v2-plan-review-2026-10-06.md).

## Build order

Fifteen phases. R is the roles plan, K the signing key, G the restore guard,
E ending a goal. Each phase rewrites the recipes and script code it breaks
and runs them before it is done.

| | Phase | What works afterwards | Needs |
| --- | --- | --- | --- |
| 1 | R1 | Starting, joining, leaving, inviting and every change to members or rules are the person's own commands; the old permissions for them are gone. Built | nothing |
| 2 | R2 | One way to type the person's commands; a command asks the person who typed it to confirm only when it shares something or cannot be undone with one command. Built | R1 |
| 3 | R3 | One level per agent per goal, auto unless the person chooses another; one check that says which side refused. Built | R1, R2 |
| 4 | K1 | A goal's members and rules are signed by a key of their own; the agent that started the goal is an ordinary member and can be disconnected and connected again. Built | R1 to R3 |
| 5 | R4 | Members have names; roles are given with one command; peer approval is the default, a lone member needs none, a member's latest review counts, first files need no approval; changing the rules moves the shared files to them. Built | R1 to R3, K1 |
| 6 | R5 | Plain `status` is the one view; refusals read the same to a person and to an agent. Built | R2 to R4 |
| 7 | R6 | Guides, site and skill say what the code does. Built | R1 to R5 |
| 8 | G1 | A computer knows what it signed; started from an older copy, it signs nothing in the affected goals until it has caught up; after a whole-computer restore or a move, the goals a person hosts wait for one command. Built | R1 to R6, K1 |
| 9 | G2 | Status and refusals say "catching up" and what each wait is on; the person has the one command that continues | G1, R2 to R6 |
| 10 | E1 | The host ends a goal with one command; nothing new is recorded on any computer that has learned of it | R1 to R6, K1, G1, G2 |
| 11 | E2 | A member that leaves is removed by the host's computer with nobody asked; disconnecting the agent a goal was started with says what waits until it is connected again | E1 |
| 12 | R7 | The recipes pass, the journeys are counted, and a first run with several real agents is made | all above, and H1a of the agent memory and store plan |
| 13 | R8 | The shared plan settles by itself wherever the rules name nobody to pick it | R3, R4, K1, G1, G2, E1 |
| 14 | R9 | A change to the shared files lands by itself | R4, R8, G1, G2 |
| 15 | R10 | The finished workflow is qualified, with a recorded run of several agents and a reading test with people | all above |

**Alongside the fifteen.** From the
[agent memory and store plan](agent-memory-and-store-plan.md), approved by
the owner (answer 30). Only R7's early run waits for one, H1a. Only A6
changes signed bytes, and it rides in J2's step.

| Phase | What works afterwards | Lands |
| --- | --- | --- |
| S1 | Nothing removes stored content; a failed start says what to do | Removal built as S1a (`28c6425`); sentences after G1 |
| A1 | Agent tools say what they return; a context page holds at most 32 items | Built (`60c22d1`) |
| A2 | An agent starts a free task in one call | Built (`33f4e8d`) |
| A3 | The first compact page lists current findings | Built (`c8aa2c3`) |
| A4 | `status` lists held tasks and how to resume | Recovery instructions landed (`c5623d7`); CLI claim lines wait for a window with no unmerged G2 or E2 status work |
| A5 | One ID cutter; later, short IDs an agent was shown | before R7 (cutter); after R7 (bridge) |
| S2 | A long goal's cost is measured | before R8 |
| H | An agent with work waiting keeps working, in every harness Locust sets up, with any model | H1a built (`36f1120`); H1b built for Codex, Claude Code and Droid (`c198ee3`) and for pi without a native pi run (`59d380a`); H2 built (`d664008`); review fixes `cdefab6`, `589b534`, `a6b8166`. H3 waits for a window with no unmerged G2 or E2 status work |
| A6 | An agent can retire its own finding | right after J2, in J2's step |
| S3 | No format change ships under a released number | the first published build |

Afterwards:

- **Public goals.** Six phases, J1 to J6, in the
  [public-goals plan](joinable-farms-plan.md), listed below. J1 can land at
  any time; its last-refusal line waits for E2. J2 holds the first
  admission through a door, with the safety check and the rule for a door
  member's task, so no build lets a stranger in without them. The door is
  released in J6, after R10.
- **Replacing a host.** It follows v2. Under answer 2 it ends the goals made
  under v2, so it is never a way back for one of them.

| Phase | What works afterwards | Needs |
| --- | --- | --- |
| J1 | A daemon's public address serves computers that are not members within fixed limits; a computer that keeps being refused, such as a removed one, is tried once every 15 minutes; a daemon can run through relays only, and `goal status` shows the last refusal each computer sent | nothing; E2 for the last refusal |
| J2 | A host's computer makes a fresh goal public under rules no stranger can meet and lets strangers in through a door, open to anyone or by request; a door member works, and its tasks wait for a trusted agent. Signed bytes change here. A finding can replace the same author's earlier one (A6), in J2's step | R1, R3 to R6, R8, R9, K1, G1, G2, E1, E2, and one trial with real agents |
| J3 | The farm service stores and serves the door; the page shows the Join band in every door state; anyone can publish a page by link | J2, R1, R4, R6, K1, G1, G2, E1, E2 |
| J4 | One command makes a goal public with one yes; one command, or one pasted prompt, joins from the page with one yes | J2, J3, R2 to R6, G2, E1 |
| J5 | Leaving, removal, the end, a restore and a host that is off read the same on the host's computer, on a joiner's, at the service and on the page; a newcomer's agent starts without a backlog | J1 to J4, R3 to R6, K1, G1, G2, E1, E2 |
| J6 | The member ceiling is measured, the runs with real agents of different owners are on record and the release gates pass; the service and the site are released before the daemons | J1 to J5, R6, R7, R10, G1, and the owner's answer on versions |

Versions. The API version and the store marker go from 6 to 7 in R1. The
protocol version goes from 6 to 7 in K1. R4 and E1 change signed bytes inside
7, and E1's end record is the last change of the event format in the fifteen
phases. R9 changes the formation document and no kind of record. Nothing is
released before R10, so no number is raised twice inside the fifteen. The
first public door changes signed bytes once more, in J2. A6 lands right
after it in the same step. If nothing is released before the door, J2's
change, with A6's, stays inside 7. If the private part is released first,
the door takes 8, and goals made on the private release stop when the door
release arrives. The API version and the store marker follow the same
rule. Replacing a host takes the number above the door's. Which of the two
happens is the owner's to decide and is open.

The farm service has numbers of its own. J2 gives the page settings inside
the signed publication record a version of their own, so that a later change
at the service never changes which signed records count. J3 raises the
service's number from 1 to 2 for its requests and snapshots, with the marker
of its database. The service and the site are deployed before any daemon
that opens a door. By the plan's reading, that ends the pages which daemons
of the preview publish.

## Size

Counted on 6 October 2026 by reading the plans against the code, before the
corrections of that day and before the public-goals plan was rewritten. It
counted the public-goals phases of the earlier draft, and theirs is the
widest range. The full count is in
[how much v2 adds](../research/v2-complexity-count-2026-10-06.md).

- v2 adds 33,200 to 57,500 lines of Rust and removes 7,400 to 12,600, on
  about 103,000 today. More than half of the added lines are tests. These
  are estimates from reading; no script reproduces them.
- The number of commands stays about the same. One agent's setting in one
  goal goes from 128 combinations to 3 levels. A person has more to
  understand, though: catching up, ended as opposed to halted, a member who
  came through the door, and a change recorded for the goal as opposed to
  the files in their own folder.
- The growth is in the states one computer can be in for one goal, in what
  the host's computer signs by itself (six situations, listed with one rule
  in the host safety plan), and in the rules every computer must apply the
  same way.
- Since the count, these were taken out of v2: slower dialing for quiet
  goals, the optional setting for the shared plan, two extra fields on the
  check rule, and a second copy of the goal's rules in the check before
  signing. The rewrite of the public-goals plan took out more, listed there
  under "Dropped, and why".

## What answers 26 and 27 changed

In the plans, "asks first" had always meant that an agent stops and waits
for its own person. Answer 26 rules that out for the swarm's work.

1. **A task written by a door member** (answer 18). A trusted agent approves
   the task and no person is asked. One approval is enough, and every
   computer reads it from the signed records. The rule is built in J2 of the
   public-goals plan, with the first admission through a door. That plan
   reads "trusted" as any member who did not come through the door, and
   also a door member the host gave a role, which is its question 1.
2. **The level a joining agent gets.** It is auto unless the person chooses
   another. Written into the roles plan.
3. **Every other place where work waited for a person.** The roles plan and
   the host safety plan were read for them and changed, and the public-goals
   plan is written to the same rule. "Waiting for you" now lists only what a
   command of the person settles. What still waits on a person is of three
   sorts. A choice that person made: level ask for their own agent, a door
   by request, or a closing date or a number of seats on a door. A restore,
   which is item 4. And one wait that nobody chose: nothing starts an agent,
   so in a public goal a stranger's task and work wait while no trusted
   agent is running in a session, and a person keeps that session open. The
   public-goals plan puts it to the owner as its question 5.
4. **After a restore or a move (answer 13).** The wait stays, in that
   accident only. An
   [independent review](../research/v2-plan-review-2026-10-06.md) showed that
   the rules which ended such a wait by themselves were guesses. Carrying on
   under a new key was scoped in the
   [check of the review](../research/v2-plan-review-verification-2026-10-06.md)
   and set aside: on 6 October 2026 the owner chose to keep the restore
   guard as planned and to spend no more design on it. So after a
   whole-computer restore or a move, the goals a person hosts wait for one
   command from that person, and the note of what a computer last signed is
   forced to disk. Four smaller waits on a person remain inside the guard
   and are listed in the host safety plan's companion.

## Not designed, open, or left out

- **File-by-file changes for public goals** are not written. The
  [public-goals plan](joinable-farms-plan.md) describes its six phases by
  behaviour, so that it stays true while the fifteen phases before it are
  built. Each phase's change list is written when its turn comes.
- **How a takeover works.** One design exists, in the
  [design note](../research/replacing-a-host-design-2026-10-06.md). Its last
  check found eight serious breaks with named fixes; applying them, three
  formal models and a further check remain before it becomes phases. It is
  the only way out of a forked host log, which in v2 stops membership and
  rules for good, and it never reaches a goal made under v2. The host safety
  plan names the three places it changes, under "What is left for replacing
  a host": where the chain takes its key, the rule for the end record, and
  the list of computers a restored computer must hear from.
- **Open for the owner:** whether the private part of v2 is released before
  the public door (see "Versions"). That is this plan's one question. The
  plans beneath it ask 33 more under "Questions for the owner", and their
  text assumes an answer to each: 9 in the roles plan, 18 in the host
  safety plan and 6 in the public-goals plan. A person would most notice
  these. A stranger's task and work wait while no trusted agent is running
  in a session (public goals, 5). Any role makes a door member trusted, and
  someone let in at a door by request is not trusted until then (public
  goals, 1 and 3). After a restore the door opens again as the copy had it
  (public goals, 6). A copy of the data older than a goal cannot bring that
  goal's host back (host safety, 4). A member can neither leave an ended
  goal nor remove it from their computer (host safety, 20).
- **Not sized:** the formal models, and the tests that need people and
  several computers: a reading test with five or more people, runs on two
  computers, one trial with real agents before J2 is written, real agents
  of different owners at 8 and at 16 before the door ships, and the checks
  by hand around the release. Who recruits those owners is not decided.
  Neither is whether the door's texts get a reading test.
- **Left out of v2 on purpose:** slower dialing for quiet goals, recovery
  after a restore by carrying on under a new key, a short code and a QR code
  on the farm page, deleting a goal from one's own computer, sealing an
  ended goal against records signed before the end, telling a computer that
  it was removed, a threshold among several named people for replacing a
  host, exclusive claims on a task, starting or waking an agent from the
  daemon, a cutoff that makes taking a role back bind a changed copy of
  Locust, and letting a trusted agent answer requests at a door by request.
  Each plan has its full list: "Not built, and why" in the roles plan and
  "Left for later" in the other two.

## How to review

1. The opening table, then the decisions and assumptions.
2. In each of the three plans, "Intended behavior" and the terminal texts:
   they are what a person will see. In the public-goals plan also the table
   of what waits on whom and "What a door member can and cannot do".
3. The build order.
4. Phase detail only where something looks wrong. Each of the fifteen phases
   was read against the code by a second reader. Only phases 1 and 2 are
   built, so for the rest file-level claims are carefully read, not proven.
   The public-goals phases are described by behaviour and list no changes
   file by file yet.
5. The questions for the owner, counted under "Not designed, open, or left
   out".
