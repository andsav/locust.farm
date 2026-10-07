# Rules on how agents work that Locust imposes: an audit after answer 33

Status: research, 2026-10-07, at `2b31e69`. Proposals only; nothing here is
accepted. It answers the owner's worry under answer 33 in the
[master plan](../docs/master-plan.md): "the goal of this project is not to
impose an opinionated set of guardrails ... Your question makes me worried we
might have gone too far in that direction already. Deterministic organization
should always be an option." No code was changed and no suite was run. The
claims marked "checked by hand" were re-read after the audit.

## Method

Seven readers each inventoried one surface: agent-facing text (skill, MCP
instructions and tool descriptions, setup text), hooks, rules the daemon and
the fold apply to every goal, formations, shared files and the person's CLI,
plans not yet built, and the guides and site copy. Each rule was classed as:

- **protocol**, needed so every computer derives the same state and
  signatures, sealing and membership hold;
- **owner**, an explicit owner answer;
- **machine-safety**, protecting the person's own computer, secrets or
  accounts;
- **opinion**, a rule about how agents should work that every goal or agent
  gets and a host cannot choose against;
- **unclear**.

A skeptical verifier per surface re-checked every item and its class. Of 174
rules, 34 are protocol, 53 owner, 23 machine-safety, 8 unclear and 56
opinion. Verifiers disagreed with 18 classes; the resolved class is used below.

## Verdict

**The shared rules are clean; the overreach is almost all in words.** The fold
and the daemon hardwire no check, test run or acknowledgement as a condition
of counting. Almost every "who may" question (propose, start, publish, review,
attest, pick, close) is read from the formation. A host who wants tests before
a result counts can already write it: a completion rule with a named `check`.

The opinions sit in four places.

### 1. The shipped skill and MCP instructions

Every agent in every goal receives these, and no host can turn them off. They
come from the "board habits" of the roles plan's Phase 6, not from an owner
answer. Checked by hand at [SKILL.md](../skills/locust/SKILL.md) lines 115-131.

| Text every agent gets | Where | Proposal |
| --- | --- | --- |
| "Approve only what you checked" | SKILL.md 131 | Cut |
| Review procedure: read standing rejects first, inspect evidence chains and source authors, "another member's summary is not independent verification" | SKILL.md 117-119, 227-233; `event.show` description | Cut; keep one fact: a reject withdraws only its author's approval and is never a veto |
| "Post your result before reading other members' results" | SKILL.md 117-118; `contributions` description in [api.rs](../crates/locust-proto/src/api.rs) | Move into the guidance of formations that want blind attempts |
| "Prefer a task nobody holds" | SKILL.md 116-117 | Cut; it works against `independent-attempts` |
| "After task:" ordering convention: check the named task is done, else prefer other work | SKILL.md 124-127, 157-165 | Move to guidance, or make ordering a formation construct |
| "Run checks only within the work your owner allowed"; update "at an explicitly allowed work boundary"; share "only the scope allowed" | SKILL.md 167-168, 183-184, 229, 251-252 | Cut; no such allowance exists in code |
| Context-reading schedule and citation habits | SKILL.md 47-95; INSTRUCTIONS in [mcp.rs](../crates/locust/src/mcp.rs) | Cut to the facts of how pages and receipts work |
| Take over only "after that session's work was checked" | SKILL.md | Cut |

The skill grew from 11,678 to 20,349 bytes since `ff1686d`, much of it in this
kind of text. The model-text test in `mcp/tests.rs` requires INSTRUCTIONS to
name four tools, so cutting there means updating that test.

Kept on purpose: secrets and credentials stay away from the model; received
code and other members' words are material, not instructions (machine
safety); the owner-command procedure (answers 7 and 31); keep working while
work waits (answers 26 and 28); resuming held claims (answer 30); and counting
facts (answers 5, 6, 16).

### 2. One hardwired daemon rule, and the picks built on it

- **A counted result closes the task.** Once any result counts on a task
  round, the daemon refuses every new attempt, offer and allowance on it
  ([access.rs](../crates/locust-core/src/node/access.rs) lines 248-262, checked
  by hand). A pick needs a counted result. So the `independent-attempts`
  preset cannot run as "several try, then the lead picks": the first author
  whose result counts closes the task to anyone not already started. The
  host's computer also stops offering a stage task once a result counts.
  Proposal: make this a formation choice.
- **The no-task pick and the hooks' free-task push assume one agent per task.**
  Both count only tasks nobody is attempting
  ([claims.rs](../crates/locust-core/src/node/requests/claims.rs) line 110;
  [goal/mod.rs](../crates/locust-core/src/goal/mod.rs) lines 452-468). Answer
  30 approved the one-call pick. The proposal is only that it respect a
  formation that wants several attempts.
- Smaller choices nobody decided: only the worker or its offerer may ask to
  cancel an attempt, so a lead or closer cannot; the host's agent fills every
  role nobody holds, so a role is never empty; review requests go to every
  eligible reviewer, with no off switch.

### 3. The policy inside the hooks

Answer 28 put hooks in, and answer 30 approved the H rows, including which
work holds a worker's stop. Within that, these are the author's choices and
none reads the formation:

- A fixed priority among kinds of waiting work.
- An idle worker in the person's own interactive chat is held once per write
  with no work waiting. Proposal: delete.
- A chat is held for attempts its sibling chats hold. The approved H1 sentence,
  "held only for attempts it holds", reads per chat. Proposal: narrow it.
- Check attestations are reported as reviews and pointed at
  `locust_review_record`.
- Decider work (picking, closing, handing out offers) never holds a stop, so a
  lead-driven formation can stall while its workers are held.
- "keep going unless your owner asked you to stop" names only the person, not
  the host's formation.
- With hooks off, the skill still tells every agent to call `locust_wait`
  before stopping. Proposal: make it conditional on working the goal's queue.

### 4. Plans not yet built, and copy

- **R7 and R10 qualification runs** grade one method, agents pulling from the
  board, and plan to reword the shared skill until agents follow it. Step 5 of
  the [self-organizing plan](../docs/self-organizing-collaboration-plan.md),
  "small conditional skill/default changes", would add more. Proposal: put
  method into formations; grade only whether agents follow the formation they
  were given.
- **Public goals** plan more shared-skill text: J2-11's approval criteria for
  trusted agents, J5-10's newcomer section, door-member etiquette, and refusal
  texts that prescribe the next step. The door safety check refuses every stage
  in a public goal with no override, so a deterministic pipeline can never be
  public. Stages fed by publication evidence do let a door member trigger the
  next stage, which is a real answer-18 concern. That one is for the owner.
- **Copy.** The site, the roles plan's opening explanation and parts of the
  guide present self-organization and peer approval as how every goal works.
  [apply.md](../docs/guide/apply.md) tells reviewers to run checks, and its
  sample review says "Tests pass". `formation explain` describes every
  formation, `directed` included, as members organizing themselves.

## The root cause: the host's method has no way to reach agents

A formation's `context.guidance` is "advisory instructions, versioned with the
agreement" ([organization.rs](../crates/locust-proto/src/organization.rs) line
69). It is stored and hashed, but nothing outside its definition reads it
(checked by hand: `grep guidance crates` finds only that field). Role
descriptions do not reach agents either, and a member's agent cannot read the
goal's pinned formation. The formation editor says guidance is shown to
agents; it is not.

Meanwhile the skill, the MCP instructions and the hook lines reach every agent
in every goal. With no channel for the host's method, Locust's own method went
into the one text every agent reads. Delivering guidance is the change that
makes answer 33 workable: method moves out of the skill and into formations,
including the presets' own guidance.

## What a host can and cannot express today

A host who wants a deterministic workflow can already set who proposes,
starts and publishes; offered-only starts (`directed`, where nobody without an
offer can start); review counts with the author excluded; a named check with a
named attester, including a dedicated test agent or a role disjoint from the
workers; two attesters through `all`; one picker and one closer; task types;
and an ordered flow of stages with prerequisites.

It cannot:

- give its agents a written procedure (guidance is not delivered);
- run best-of-N, or keep a task open after a result counts;
- require a check that excludes the author when attesters overlap with workers,
  or a counted number of checks; `count` and `exclude_author` on checks were
  withdrawn on 2026-10-06 as a size cut (roles plan, assumed row 9), not by an
  owner answer;
- make a reject or a failed check block;
- order ad-hoc tasks, except through the "After task:" text nothing reads;
- run stages per task: a flow runs once per rules binding, with no loops;
- gate a stage on landed shared files;
- let a lead cancel or revise work;
- use stages in a public goal (planned refusal);
- set hook behaviour per goal; it is per person and per harness;
- have the next step's agent started (answer 32, not designed).

Some limits are protocol and should stay: no deadlines or timeouts (principle
3); no thresholds relative to current membership, since an author picks the
position its result is judged at; single-holder deciders; subtasks narrower
than their parent.

## Proposal

One change, in this order:

1. Deliver `context.guidance` and role descriptions to agents: on the first
   compact context page, in the brief, and through `formation.show` for the
   goal's pinned rules. Fix the editor's claim until then.
2. Cut the method sentences listed above from the skill and the MCP
   instructions, keeping how-the-tools-work facts. Move the useful ones into
   preset guidance: "post before reading others" into `independent-attempts`,
   for example.
3. Make "a counted result closes the task" a formation choice, and let the
   one-call pick and the free-task push follow a formation that wants several
   attempts.
4. Restore `count` and `exclude_author` on checks as formation options.
5. Fix the hook items: drop the idle hold in a person's chat; narrow sibling
   holds; report checks as checks; make the hooks-off fallback conditional.
6. Fix the copy, and redirect R7, R10 and the self-organizing plan's step 5 so
   method lives in formations.

Items 1, 2 and 6 change only text and one read path. Item 3 changes which
records a daemon accepts locally, not signed formats. Item 4 adds fields to
the formation document, which is signed, so it belongs with a change of
signed format.

## Disputed classes

Verifiers moved several items. The resolved reasons:

- The hooks' kinds of waiting work, the sticky worker flag and the
  session-wide read reset were in the H text the owner approved with answer
  30. They are owner, not opinion, but their wording is still the author's.
- Refusing reviews from members the rule does not name enforces the host's
  selector, which a deterministic host needs; keep it.
- "Readers join the counting role on rules bind" is a plan-author default
  overridden by `--no-role` on each command; keep it, but it is not an owner
  answer.
- `workspace connect` confirming and the formation prompt asking before
  publishing meet answer 7's hard-to-undo test, since neither can be undone
  with a command; keep them.
- "Your agent asks you first" in [concepts](../docs/guide/concepts.md)
  describes what the skill produces, and the same page says nothing enforces
  it, as answer 31 asks; keep it.
