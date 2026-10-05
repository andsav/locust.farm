# Roles and permissions: what exists and a simpler model

Status: investigation and proposal of 2026-10-05 at `49241fb`. Nothing here is
implemented, built or run. Statements about today's code were read in the
source by investigating agents; the author re-read the grant record, the
creator's default grant, the stage check and the admission check. The proposal
is under discussion with the owner and none of its choices is accepted.

The question was how to give Locust a permission model that a person with
little context understands at once.

## Method

1. Five agents mapped what exists: every operation and the checks it passes,
   the actors, the rules a goal carries, the words the documents and command
   line use, and the steps eight journeys cost. A sixth read how sixteen other
   products keep this simple.
2. A first model was written. Four agents attacked it (protection,
   completeness against all operations, the journeys redone, the words). Two
   more designed their own models without seeing it.
3. Readers given only an explanation answered twelve scenario questions, first
   from the first model and from today's documentation, then from the revised
   model.

Limits: the readers were language models, three per text at most, so their
scores are a rough signal and not a user study. Counts of commands under the
proposal are inferred from its text, because the commands do not exist.

## What exists today

A newcomer meets twelve separate things that can each refuse an action: what
the setup prompt allowed, the coding agent's own approvals, the owner or agent
credential, a daemon-wide grant, membership, the administrator, the
formation's roles and selectors, single deciders, seven per-goal permissions,
per-task authorization, sessions and claims, and farm consent.

### Three yeses for every write

Every write by an agent needs three independent conditions: the agent is a
member, the goal's formation makes it eligible, and the agent's own owner
granted a local permission. They are checked in a different order for
different actions and reported with three different error codes. The commonest
refusal, "this operation requires a local grant", names no grant.

### Nine local switches

- One daemon-wide grant, `manage_goals`: create, join, leave and invite.
- Seven per goal ([api.rs](../crates/locust-proto/src/api.rs), `GoalGrants`):
  administer, contribute, execute, review, select, flow, takeover.
- A per-task authorization that stands in for execute.

Of 97 operations, 24 consult a grant. Every way an agent comes to exist gives
it none, except that a goal's creator gets `administer`
([goals.rs](../crates/locust-core/src/node/requests/goals.rs)).

### The same question asked twice

| The question | Asked in the goal's rules | Asked again locally |
| --- | --- | --- |
| May it start work | the start rule | `execute`, or one task's authorization |
| May it approve | the completion rule or a reviewer role | `review` |
| May it pick, close or accept files | the selection, finish or integrator authority | `select` |
| May it hand out work | the coordinator role | `flow` |
| May it run the goal | being the administrator | `administer`, and `manage_goals` to admit remote people |

Holding a role therefore does nothing until the member's own owner also grants
the matching permission, and nothing says which one. Granting a permission the
rules do not back succeeds and has no effect.

### Two checks with nobody present

- The daemon signs a formation's stage tasks and review requests only for a
  local member that holds `flow`
  ([flow.rs](../crates/locust-core/src/node/flow.rs)). Without it nothing
  happens and nothing says so.
- It admits a remote joiner only if the administrator agent holds `administer`
  and `manage_goals` ([peers.rs](../crates/locust-core/src/node/peers.rs)).
  Without them the invited person is refused for good and the host sees
  nothing.

### The person and the agent

- The owner is whoever can read one credential file. It has no key and never
  appears in a goal. Every signed record carries an agent's key. A person is
  never a member; the guide says "an agent or a person".
- `--owner --as NAME` signs with that agent's key and skips every local grant.
  It never skips membership, the administrator check or the goal's rules.
- The line between owner and agent is real at the daemon and for agent tools.
  It is not an operating-system boundary: the agent runs as the same user and
  the launcher only refuses five flag spellings
  ([launcher.rs](../crates/locust/src/installation/setup/launcher.rs),
  [installation](../docs/installation.md)). The setup prompt already has the
  agent run owner commands.

### What it costs

- Two agents on one computer with review: 7 documented owner commands, of which
  6 answer no question a person would recognise as a choice.
- Inviting a friend: 3 to 4 commands on the host and 2 to 4 on the friend.
- The four-agent live demo needed at least 25 permission-related calls.
- About nine decisions across all journeys are real: connecting an agent;
  starting a goal and how results are decided; who is invited; accepting an
  invitation and which agent joins; whether tasks other people wrote may run
  on this computer; whether the agent may post into the goal; which files are
  shared; a public page and name; removing a member.

### Words

The per-goal right is called permission, grant, authorization, allow, right
and approval. The person is owner, local participant, person and user. The
agent is agent, principal, participant, member, client and harness; one
enrolled agent is selected by six different flags. "Participant" names the
person in some places and the agent in others. `administer` is a permission
that any agent can be given and that does nothing unless that agent is the
administrator.

## Proposed model

### The explanation a new user gets

> A goal is work shared by agents. People are never in a goal; their agents
> are, one member each.
>
> The **host** is the person whose agent started the goal. The host decides
> which agents are in, sets the goal's rules, and gives **roles** such as
> reviewer.
>
> For each of your agents in each goal you choose one of three **levels**.
> **Read**: it only reads. **Ask**: it takes part, posting results, reviews
> and whatever its role allows, and takes a task only when you allow that
> task. **Auto**: it also takes tasks on its own.
>
> An agent can do something when the goal's rules allow it and its level
> allows it. A refusal says which one said no and who can change it. The host
> cannot see or change your levels.
>
> Connecting an agent puts it in no goal. Starting, joining or leaving a goal,
> who is in it, its rules and roles, levels, and anything public are yours:
> your agent asks you first.

### Three ideas

1. **In the goal: host, members, rules, roles.** Shared and signed. Set by the
   host. The rules protect the goal from everyone's agents.
2. **On your computer: one level per agent per goal.** Local. Set by you. The
   level protects your computer from your own agent.
3. **Only you.** A short fixed list that no agent tool can perform.

### The levels

| Level | The agent | What it means for you |
| --- | --- | --- |
| read | reads the goal, and can finish or drop what it already holds | nothing new leaves or runs |
| ask | also posts to the goal: findings, results, tasks, reviews and any decision its role allows. It takes a task only when you allow that task | asks before each task |
| auto | also takes tasks on its own | tasks other members wrote run on this computer without asking |

The levels are cut by the two decisions that are really local: whether the
agent may post, and whether it may take tasks unasked. Acting in a role is
posting, so a role never needs a second yes. An agent that only reviews is a
reviewer at level ask whose tasks are never allowed.

Defaults: an agent you start a goal with, or add to your own goal, gets auto.
Joining someone else's goal asks for the level in the join step, shows what
each level means under that goal's rules, and has no silent default.

Allowing one task is today's per-task authorization. It should last until the
task ends, through a revision of the task and a restarted session.

### Only you

- Connect or disconnect an agent.
- Start a goal and choose its rules. Join a goal. Leave a goal.
- Who is in: invite, open or close a door, remove a member.
- The goal's rules, and who holds which role.
- An agent's level, and allowing one task.
- Connecting a folder on this computer to a goal, which is the only way local
  files can be shared.
- A public page, and the name your agent appears under.

The honest statement of this boundary: Locust refuses these to every agent
tool. An agent with a shell can still run the command, so the coding agent's
own approval prompt is the real guard. Each of these commands carries
`--owner`, so a person can recognise it in that prompt, and each shows a plan
and proceeds only on a confirmation bound to that plan.

A command a person runs themselves is recorded as one of their agents. The
level limits the agent, never the person.

### What the daemon does by itself

It carries out decisions already made and consults no permission: it admits
whoever presents an invitation the host issued or comes through a door the
host opened; it runs the stages of the rules the host set; it asks for reviews
of results the agent posted; it syncs and uploads the public page. It acts for
an agent only while that agent is connected and a member.

### The goal side

- **Host** replaces "administrator". The host is the person; the host's agent
  holds the goal's key. The goal is run from that computer: while it is off,
  nobody joins and the rules cannot change.
- **Member**: an agent in the goal. Every member has a name in the goal,
  chosen by its person at start or join. A name is a label, not an identity.
- **Role**: the rules name roles; the host says who holds them. The built-in
  formations would use two names: **reviewer** (approves results) and **lead**
  (hands out work, picks the result, closes tasks). The host runs the goal; a
  lead runs the work.
- The host's agent holds every role when a goal starts. Giving a role is one
  command that names a member.

### Refusals and the one view

One template: who, what, why, which side said no, who can change it. Three
sides only: your setting, the goal's rules, the goal's state.

- "Maple can't take this task: its level here is ask (your setting). Allow
  this task, or set Maple to auto."
- "Maple can't approve this result: it needs a reviewer and Maple is not one
  (the goal's rules). The host, Ana, gives roles."
- "Maple can't take this task: it is already finished (the goal's state)."

The rules are checked first, and a level change is never suggested when the
rules would still refuse. Text shown to an agent says "Maple's owner" where
text shown to the person says "you".

One view answers "what may my agent do here" and "what is waiting for me":

```text
Waiting for you
  Maple wants to take "Fix the parser" in Static site search
Static site search · host Ana
  Maple    reviewer · ask   posts and reviews; asks before each task
  Juniper  member   · auto  posts; takes tasks on its own
```

### How today's checks map

| Today | Proposed |
| --- | --- |
| `manage_goals` for create, join, leave, invite | only you; no grant |
| `administer` for rules, removal, tree policy, invitations | only you, as host; no grant |
| `contribute` for opening tasks, posting, declaring done, proposing files | level ask, plus the goal's rules |
| `review`, `select`, `flow` for approving, picking, closing, offering | level ask, plus the goal's rules |
| `execute` or one task's authorization for starting | level auto, or ask plus allowing that task |
| `takeover` | part of taking the task: an agent may resume its own attempt |
| the daemon needs `flow` to run stages | nothing |
| the daemon needs `administer` and `manage_goals` to admit | nothing |

### What disappears

- The daemon-wide grant, the seven per-goal grants and the two silent checks.
- Every double question: a role is enough.
- The words principal, participant, grant, authorization and administrator
  from what a person reads; `goal grant` and `task authorize` as second ways to
  set one thing.
- `--as`, `--principal`, `--recipient` and `--integrator`. Two selectors
  remain: `--agent` for one of yours, `--member` for someone in the goal.
- Six tools from the agent's tool list: create goal, leave goal, remove member,
  bind rules, tree policy, revise task. The agent's unreviewed way to join.

## What the review changed

| First model | Correction |
| --- | --- |
| The top level let an agent start tasks and use any role | Those are unrelated. Acting in a role moved to the middle level; the top means only "takes tasks on its own". A review-only agent became expressible |
| Levels named read, contribute, work | "Work" already names three other things in the product and does not say "without asking". Renamed read, ask, auto |
| "Never an agent's call" | True of agent tools, false of a shell. Replaced by the honest statement above |
| Drop `--owner`; plain `locust` is you | Kept. It is the one word a person can recognise in the coding agent's approval prompt |
| Refusals name the host and members | Members on other computers have no names today. Every member gets a name in the goal |
| "Give a role with one command" | A role is a list inside the rules, and a task keeps the rules it was opened under. Needs a real operation, and a decision about open tasks |
| Read is read-only | Eleven agent writes check no grant today. Read is defined; sharing a file needs a connected folder |
| The daemon consults nothing to admit | Invitations never expire by default. They should, and one command should stop all admission |
| "Which files are shared" is your decision | Nothing asked it. Connecting a folder to a goal becomes the person's act |
| The host cannot see levels (unstated) | Stated, with the sentence that rules protect the goal |

## Newcomer test

Twelve scenario questions, answered from an explanation alone.

| Explanation | Readers | Right out of 12 |
| --- | --- | --- |
| Today's concepts page | 1 | about 7 |
| First model | 2 | 9 and 10 |
| Revised model | 3 | 10, 12 and 11 |

The first model failed on "what may a freshly connected agent do", "which
setting decides whether strangers' tasks run unasked" and "an agent that only
reviews". The revised text fixed the first two. The third still drew low
confidence, so the explanation now says "takes a task only when you allow that
task".

## Alternatives considered

- **In or out.** No levels at all: once an agent is in a goal it may do what
  the rules allow, and the coding agent's approvals guard the computer. It is
  the smallest model and it is honest that Locust sandboxes nothing. It cannot
  say "ask me before each task", which is the one local decision that matters
  in a goal run by strangers.
- **Ask once.** Nothing set in advance; the first time an agent wants to post,
  work or decide in a joined goal, the person is asked: this once, always
  here, or no. Closest to phones and coding agents. It needs a reliable way to
  reach a person while the agent runs unattended, and its three asked things
  are the same three the levels cover.
- **Four levels.** A separate top level for using roles. It puts approving
  above taking tasks, which is backwards for the commonest wish.
- **Accepting a role.** The member's person is asked once when a host gives
  their agent a role. It protects the member's signature and tokens, and adds a
  concept that the model otherwise avoids.

## What it needs underneath

1. One level in place of eight flags, and a task allowance that survives
   revision and restart.
2. About 50 of the 97 operations change their check, audience or tool flag.
3. One function, "may this agent do this here, and if not why", run before
   signing and used by refusals, the view and the pending list.
4. A name for every member, carried in the goal.
5. Roles: the creator's agent fills every role at start; giving and taking a
   role is one host operation. Today a new reviewer cannot review a task that
   already exists, because each task keeps the role holders it was opened with
   ([rules.rs](../crates/locust-core/src/goal/rules.rs)). Either role holders
   are looked up when the action happens, or giving a role offers to move open
   tasks.
6. Invitations expire by default; one command stops all admission; open
   invitations and doors appear in the view.
7. Connecting a folder to a goal becomes a person's act.
8. Every "only you" command gets plan-then-confirm bound to the plan shown.

By one reviewer's count this touches about 50 Rust files, 28 of them tests or
simulation, and about 40 documents, scripts and site files.

## Effect on the joinable farms plan

The [plan](../docs/joinable-farms-plan.md) is written against the grants this
proposal removes and would be rewritten first:

- The join's `--allow contribute[,execute]` becomes a level, ask or auto.
- `farm on --joinable` no longer grants `manage_goals` and `administer`; its
  first open question dissolves.
- The unreviewed agent join goes, which settles its fourth question.
- Its `maintainer` and `lead` roles become `reviewer` and `lead`.
- The public name at join becomes the member's name in every goal.

## Decisions for the owner

1. The level names: read, ask, auto (recommended), or read, contribute, work.
2. Whether a role needs a yes from the member's person. Recommended: no.
3. Whether `--owner` stays as the visible mark of a person's command.
   Recommended: yes.
4. Whether the built-in formations standardize on reviewer and lead.
   Recommended: yes.
5. Whether to go further and drop levels entirely. Recommended: not while
   goals run by strangers are planned.
6. Whether a real wall between a person and their agent is wanted, such as a
   separate operating-system user. Today there is none, and no wording should
   imply one.

## Appendix: every operation and the checks it passes today

Read from the code by one investigating agent and not re-checked line by line.
Evidence uses short paths: `api` is crates/locust-proto/src/api.rs, `req/` is
crates/locust-core/src/node/requests/, `node/` is crates/locust-core/src/node/ and
`goal/` is crates/locust-core/src/goal/.

Who may call: OWNER = owner credential only, never with `--as` (an agent gets `denied: this request is the owner's to make`, node/callers.rs:135-137). AGENT = the agent's own credential, or the owner with `--as NAME` (owner without `--as` gets `invalid: the owner makes this request on behalf of a principal`, node/callers.rs:115-125). ANY = read-only: agent, a viewer of that agent, the owner directly, or the owner with `--as`. ADMIN = as AGENT, and the handler also requires the caller to be the goal's one administrator (node/access.rs:81-96). AUTHOR = an agent or an author-only credential (or owner with `--as`).

Membership words: "visible" = the goal exists for the caller: a current or past member (removed or left keeps old history; joining answers `unavailable`; refused answers `denied`; never a member answers `not_found`; the owner directly sees every goal) (node/access.rs:40-58). "member" = a current member that has not left (node/access.rs:62-77).

"Skip" column: does the owner acting with `--as` skip the local grant? It never skips membership, administrator identity, sessions or goal rules.

| Operation | Who may call | Model tool | Local grant checked | Also depends on (membership, session, goal rules) | Owner `--as` skips grant | Evidence |
|---|---|---|---|---|---|---|
| **READING** | | | | | | |
| status | ANY | yes | none | none; an agent sees only itself and its own goals | n/a | api:898; req/daemon.rs:17-36 |
| goal.status | ANY | yes | none | visible | n/a | api:914; req/goals.rs:171-172 |
| board | ANY | yes | none | visible | n/a | api:934; req/reading.rs:16-17 |
| task.show | ANY | yes | none | visible | n/a | api:935; req/reading.rs:36-37 |
| event.show | ANY | yes | none | visible | n/a | api:936; req/reading.rs:139-140 |
| contributions | ANY | yes | none | visible | n/a | api:947; req/reading.rs:52-53 |
| contribution.inspect | ANY | yes | none | visible | n/a | api:948; req/reading.rs:96-104 |
| pending | ANY | yes | none (reads execute / task authorization only to sort work into "to start" or "to authorize") | visible | n/a | api:958; req/reading.rs:152-153; node/views.rs:296-302 |
| pending.page | ANY | yes | none | visible | n/a | api:957; node/context_views.rs:121-129 |
| wait | ANY | yes | none | visible | n/a | api:959; req/reading.rs:196-205 |
| events | ANY | yes | none | visible | n/a | api:960; req/reading.rs:159-166 |
| doc.read | ANY | yes | none | visible | n/a | api:961; req/documents.rs:14-15 |
| blob.get | ANY | no | none | visible; content must lie within the caller's membership period | n/a | api:964; req/content.rs:77-111 |
| blob.stat | ANY | yes | none | visible | n/a | api:965; req/content.rs:161-162 |
| context.read | ANY | yes | none | visible | n/a | api:978; node/context.rs:164-175 |
| context.acknowledge | AGENT (viewer refused) | yes | none | visible; needs a session; the receipt must belong to this agent and this session | n/a | api:979; req/mod.rs:75-83; node/context.rs:322-340 |
| workspace.head | ANY | yes | none | visible | n/a | api:926; req/workspace.rs:278-279 |
| workspace.tree | ANY | yes | none | visible | n/a | api:927; req/workspace.rs:283-289 |
| workspace.read | ANY | yes | none | visible | n/a | api:928; req/workspace.rs:334-351 |
| workspace.proposals | ANY | yes | none | visible | n/a | api:929; req/workspace.rs:446-447 |
| workspace.proposal | ANY | yes | none | visible | n/a | api:930; req/workspace.rs:439-440 |
| workspace.revision | ANY | yes | none | visible | n/a | api:931; req/workspace.rs:362-363 |
| permission.inspect | ANY; an agent only about itself, the owner directly about anyone | yes | none | visible | n/a | api:984; req/permissions.rs:20-40 |
| inbox | OWNER | no | none | none | n/a | api:989; req/permissions.rs:98-101 |
| **PUBLISHING** | | | | | | |
| task.open | AGENT | yes | contribute | member; rules `work.propose` (also the parent task's) | yes | api:937; req/tasks.rs:65-66; goal/fold.rs:232-278 |
| contribution.publish | AGENT | yes | contribute | member; rules `work.publish`; if it names an attempt, the session holding that claim generation | yes | api:946; req/claims.rs:257-270; goal/fold.rs:355-372 |
| doc.revise | AGENT | yes | contribute | member; rules `work.publish` | yes | api:962; req/documents.rs:40-41; goal/fold.rs:458-471 |
| blob.put | AGENT | no | none | member | n/a | api:963; req/content.rs:52-53 |
| blob.withdraw | AGENT | yes | none | member (any member; effect is local to this daemon) | n/a | api:966; req/content.rs:173-174 |
| workspace.operation.prepare | AGENT | no | none | member | n/a | api:921; req/workspace.rs:779-785 |
| workspace.publish | AGENT | yes | contribute (checked late, after the prepared operation and workspace state) | member; a prepared operation; workspace ready and enabled; rules `work.publish` and a workspace policy | yes | api:932; req/workspace.rs:459-513; goal/fold.rs:384-405 |
| **TAKING WORK** | | | | | | |
| attempt.start | AGENT | yes | execute, or a per-task authorization for the task's current round | member; session; goal rules are checked first (`work.starts`: independent start, or an offer addressed to this agent; task not closed, completed or selected) | yes | api:941; req/claims.rs:94-120; node/views.rs:47-53; goal/mod.rs:353-374 |
| attempt.takeover | AGENT | yes | takeover, or a per-task authorization whose takeover flag is set | member; session; the attempt was started by this same agent and is held by another of its sessions; no goal rule | yes | api:942; req/claims.rs:158-177 |
| attempt.report | AGENT | yes | none | member; the session holding the claim generation; `completed` needs a published contribution naming the attempt | n/a | api:945; req/claims.rs:217-232 |
| work.decline | AGENT | yes | none | member; rule: only the offer's recipient | n/a | api:943; req/tasks.rs:190-197; goal/fold.rs:326-331 |
| cancel.acknowledge | AGENT | yes | none | member; the attempt's author; the current claim holder | n/a | api:956; req/claims.rs:308-341 |
| delivery.acknowledge | AGENT | yes | none | member; rule: a recipient of that delivery | n/a | api:955; req/tasks.rs:407-414; goal/fold.rs:481-498 |
| **REVIEWING** | | | | | | |
| review.record | AGENT | yes | review | member; rules `decisions.completion` of kind reviews (`by`, `exclude_author`) | yes | api:950; req/tasks.rs:259-260; goal/fold.rs:425-438 |
| check.attest | AGENT | yes | review | member; rules `decisions.completion` of kind check (`name`, `by`) | yes | api:951; req/tasks.rs:288-289; goal/fold.rs:440-456 |
| completion.declare | AGENT | yes | contribute (not review) | member; rules `decisions.completion` of kind declaration (`by`) | yes | api:949; req/tasks.rs:236-237; goal/fold.rs:413-423 |
| **DECIDING** | | | | | | |
| scope.select | AGENT | yes | select | member; rules `decisions.selection` must name exactly this member; not usable for the workspace | yes | api:952; req/tasks.rs:307-337,387-388; goal/fold.rs:706-714 |
| scope.close | AGENT | yes | select | member; rules `decisions.finish` must name exactly this member | yes | api:953; req/tasks.rs:339-375,387-388; goal/fold.rs:706-714 |
| scope.reopen | AGENT | yes | select | member; rules `decisions.finish` must name exactly this member | yes | api:954; req/tasks.rs:339-375,387-388 |
| workspace.integrate | AGENT | yes | select (checked late) | member; a prepared operation; workspace policy `integrator` names this member; proposal approved; head unchanged | yes | api:933; req/workspace.rs:459-558 |
| **RUNNING THE GOAL** | | | | | | |
| goal.create | AGENT | yes | manage_goals (daemon-wide; refusal is `denied`, not `authorization_required`) | none; the creator becomes the administrator and receives `administer` | yes | api:909; req/goals.rs:41,111-118; node/access.rs:116-127 |
| rules.bind | ADMIN | yes | administer | member; must be the goal's administrator | yes (grant only) | api:916; req/goals.rs:136; node/access.rs:81-96 |
| task.revise | table says ADMIN; the handler admits any member | yes | contribute (not administer) | member; goal rules then accept it only from the administrator (`conflict: not_administrator`) | yes | api:938; req/tasks.rs:108-109; goal/chain.rs:349-351; node/commit.rs:294-314 |
| workspace.epoch | ADMIN | yes | administer | member; administrator | yes (grant only) | api:925; req/workspace.rs:122 |
| work.offer | AGENT | yes | flow | member; rules `work.starts` of kind offered (`by`, `to`) | yes | api:939; req/tasks.rs:150-151; goal/fold.rs:293-299 |
| attempt.cancel | AGENT | yes | flow | member; rule: only the worker itself or the member whose offer it accepted | yes | api:944; req/tasks.rs:216-217; goal/fold.rs:333-345 |
| **MEMBERSHIP** | | | | | | |
| goal.invite | ADMIN | no | manage_goals AND administer | member; administrator; goal not halted | yes (both) | api:911; req/invitations.rs:184-191 |
| invitation.list | ADMIN when an agent asks; the owner directly for any goal | no | administer (agent only) | administrator (agent only) | n/a (owner reads directly) | api:981; req/invitations.rs:68-73 |
| invitation.revoke | OWNER | no | none | goal known to this daemon; invitation not yet used | n/a | api:982; req/invitations.rs:86-117 |
| invitation.inspect | ANY | no | none | none (checks the ticket itself) | n/a | api:980; req/invitations.rs:61-66 |
| goal.join | AGENT | no | manage_goals (`denied`) on the joiner's daemon; admission then runs automatically on the inviter's daemon and needs the administrator agent's administer AND manage_goals there | ticket valid, unexpired, unused | yes on the joiner's side; a same-daemon join by the owner also skips the administrator's grants | api:910; req/invitations.rs:240-241,314-340; node/peers.rs:341-364 |
| invitation.join | OWNER (names the agent with `--principal`) | no | none (runs goal.join as an owner act) | review identifier must match the ticket; agent active and not author-only | n/a | api:983; req/invitations.rs:136-170 |
| goal.leave | AGENT | yes | manage_goals (`denied`) | member; not the administrator | yes | api:912; req/goals.rs:260-267 |
| member.remove | ADMIN | yes | administer | member; administrator; target is a member and not itself | yes (grant only) | api:915; req/goals.rs:290-300 |
| **LOCAL SETUP: daemon and identities** | | | | | | |
| daemon.stop | OWNER | no | none | none | n/a | api:899; req/daemon.rs:40-48 |
| agent.enroll | OWNER | no | none (sets manage_goals for the new agent) | name and credential unused | n/a | api:900; req/daemon.rs:52-112 |
| author.enroll | OWNER | no | none | name and credential unused | n/a | api:901; req/daemon.rs:55-57 |
| viewer.enroll | OWNER | no | none | the agent is active | n/a | api:904; req/daemon.rs:154-178 |
| agent.grant | OWNER | no | none (replaces the daemon-wide grants) | the agent is active | n/a | api:902; req/daemon.rs:117-131 |
| agent.revoke | OWNER | no | none | the agent exists | n/a | api:903; req/daemon.rs:133-152 |
| **LOCAL SETUP: permissions** | | | | | | |
| goal.grant | OWNER | no | none (replaces all seven goal grants at once) | goal known to this daemon; agent enrolled; membership not required | n/a | api:913; req/goals.rs:238-256 |
| permission.allow | OWNER | no | none (turns the named grants on, leaves the rest) | goal known; agent enrolled and not author-only; membership not required | n/a | api:985; req/permissions.rs:76-96 |
| permission.revoke | OWNER | no | none (turns the named grants off) | same | n/a | api:988; req/permissions.rs:76-96 |
| permission.task.allow | OWNER | no | none (writes a task authorization; keeps an earlier takeover flag) | agent is an active current member; tied to the task's current round | n/a | api:986; req/permissions.rs:180-229 |
| permission.task.revoke | OWNER | no | none (removes the task's authorizations in every round) | goal known; task exists | n/a | api:987; req/permissions.rs:152-178 |
| task.authorize | OWNER | no | none (writes the same record as permission.task.allow but overwrites the takeover flag) | agent is an active current member; tied to the task's current round | n/a | api:940; req/tasks.rs:164-189 |
| **LOCAL SETUP: sessions** | | | | | | |
| session.report | AGENT | no | none | needs a session; the session must belong to this agent | n/a | api:905; req/sessions.rs:48-64 |
| session.show | ANY | no | none | an agent sees only its own sessions | n/a | api:906; req/sessions.rs:66-85 |
| sessions | ANY | no | none | an agent sees only its own sessions | n/a | api:907; req/sessions.rs:87-99 |
| session.drop | the session itself, or the owner directly | no | none | the session holds no unfinished attempt | n/a | api:908; req/sessions.rs:101-114; node/callers.rs:116-117 |
| **LOCAL SETUP: checkouts** | | | | | | |
| checkout.register | AGENT | no | none | member | n/a | api:917; req/workspace.rs:622-628 |
| checkout.bind_session | AGENT | yes | none | member; session | n/a | api:918; req/workspace.rs:727-734 |
| checkouts | ANY | yes | none | visible | n/a | api:919; req/workspace.rs:767-777 |
| workspace.recovery.parent | ANY | no | none | visible | n/a | api:920; req/workspace.rs:91-97 |
| workspace.operation.show | ANY (the owner must use `--as`) | yes | none | visible; own operations only | n/a | api:922; req/workspace.rs:884-898 |
| workspace.operations | ANY (the owner must use `--as`) | yes | none | visible; own operations only | n/a | api:923; req/workspace.rs:900-912 |
| workspace.operation.complete | AGENT | no | none | visible (a removed member may still finish its local file update) | n/a | api:924; req/workspace.rs:914-922 |
| **LOCAL SETUP: private formation catalog** | | | | | | |
| formation.draft.create | AUTHOR | yes | none | none; each agent's or author's own private catalog | n/a | api:967; req/catalog.rs:50-60; node/callers.rs:140-153 |
| formation.draft.update | AUTHOR | yes | none | own catalog | n/a | api:968; req/catalog.rs:61-68 |
| formation.draft.show | AUTHOR | yes | none | own catalog | n/a | api:969; req/catalog.rs:69-71 |
| formation.drafts | AUTHOR | yes | none | own catalog | n/a | api:970; req/catalog.rs:72-74 |
| formation.publish | AUTHOR | yes | none | own catalog | n/a | api:971; req/catalog.rs:75-90 |
| formation.show | AUTHOR | yes | none | own catalog | n/a | api:972; req/catalog.rs:91-93 |
| formation.list | AUTHOR | yes | none | own catalog | n/a | api:973; req/catalog.rs:94-96 |
| formation.presentation.show | AUTHOR | yes | none | own catalog | n/a | api:974; req/catalog.rs:97-99 |
| formation.presentation.update | AUTHOR | yes | none | own catalog | n/a | api:975; req/catalog.rs:100-113 |
| formation.validate | AUTHOR | yes | none | none | n/a | api:976; req/catalog.rs:114-118 |
| formation.explain | AUTHOR | yes | none | none | n/a | api:977; req/catalog.rs:114-118 |
| **PUBLICATION (public farm page)** | | | | | | |
| farm.on | OWNER | no | none | this daemon must hold the goal administrator's key; the daemon signs as the administrator without consulting any grant | n/a | api:893; node/farm.rs:96-105,575-576,659-663,763-776 |
| farm.off | OWNER | no | none | same | n/a | api:894; node/farm.rs:745-776 |
| farm.show | OWNER | no | none | goal known to this daemon | n/a | api:895; node/farm.rs:592-609 |
| farm.status | OWNER | no | none | none | n/a | api:896; node/farm.rs:577-584 |
| farm.consent | OWNER | no | none | the daemon signs the consent as the named local agent, which must be a current member; no grant consulted | n/a | api:897; node/farm.rs:611-657 |

Totals: 97 rows. Grant-checked operations: manage_goals 4 (goal.create, goal.join, goal.invite, goal.leave); administer 5 (rules.bind, workspace.epoch, member.remove, goal.invite, invitation.list as an agent); contribute 6 (task.open, task.revise, contribution.publish, doc.revise, completion.declare, workspace.publish); select 4 (scope.select, scope.close, scope.reopen, workspace.integrate); review 2 (review.record, check.attest); flow 2 (work.offer, attempt.cancel); execute 1 (attempt.start); takeover 1 (attempt.takeover). Two more checks have no caller: automatic steps need flow (node/flow.rs:21-28); admitting a remote joiner needs the administrator agent's administer and manage_goals (node/peers.rs:351-358).
