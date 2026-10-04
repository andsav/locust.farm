# Exclusive claim on a task: proposal

Date: 2026-10-04. **Status: proposal. Nothing in it is built or accepted.** It
was written against Locust at commit `d07d202`. No code was written or run for
this note.

Labels: **[V]** checked by the author in the source or in an accepted document;
**[I]** inference or recommendation.

## Question

The owner said on 2026-10-04 that an exclusive claim "could be a good
addition": one member takes a task, and other members cannot work on it until
it is released or replaced. It is decision 13 in the
[formation editor plan](../docs/formation-authoring-plan.md) and "wanted, not
built" in the [editor review](formation-editor-review.md). The accepted
documents defer it as package O13.

What do the accepted documents already require of it, and what is the smallest
change to the format, the runtime and the editor that meets those
requirements?

## Short answers

1. The accepted documents already fix the hard parts: one named writer per
   task, a chain of generations, explicit release and replacement, waiting when
   the writer is unreachable, no reassignment by clock, and a halt on a fork.
2. Format: one new start rule kind, `exclusive`, which must be the only start
   rule of its task rules. No other field.
3. Authority: the goal's administrator, as for stages. This departs from the
   accepted wording "named authority" and needs the owner's decision.
4. Runtime: two new member events (request, release), one new purpose for the
   existing scoped decision stream (grant, revoke), and one new field on
   `AttemptStarted`. A result for the task must come from an attempt that names
   the member's grant.
5. A result from a member who does not hold the task is refused locally and
   does not count on replay. A result the old holder wrote after being replaced
   does not count either; it is kept and shown, and the member can still post
   it as a finding that is not attached to the task.
6. Editor: one more answer under "Who works on a task?": "One member at a
   time", with who can take it and three printed limits.
7. Building it means selecting O13 before the M3 review that was meant to
   decide it. That and nine other questions are for the owner.

## Words

- **Claim** is the owner's word and the word in this note's title.
- **Reservation** is the accepted documents' word for the same thing.
- Locust already uses `Claim` for something else: one session's hold on one
  attempt on one daemon, with its own `generation`
  ([api.rs](../crates/locust-proto/src/api.rs) line 1389,
  [claims.rs](../crates/locust-core/src/node/requests/claims.rs)). [V] O13
  requires the two to stay separate.

To keep them apart this note uses `exclusive` in the format, `reservation` in
event names, **grant** for one confirmed reservation, and "takes a task" on the
page. Naming is open question 8.

## Today

| Fact | Source |
| --- | --- |
| `work.starts` holds `independent` and `offered` rules. Neither locks a task. | [organization.rs](../crates/locust-proto/src/organization.rs) lines 147 to 158 [V] |
| Starting an attempt checks the start rules. Any number of members can start. | [fold.rs](../crates/locust-core/src/goal/fold.rs) lines 223 to 239, [goal/mod.rs](../crates/locust-core/src/goal/mod.rs) lines 339 to 361 [V] |
| Posting a result checks only the publish rule. An attempt is optional. | [fold.rs](../crates/locust-core/src/goal/fold.rs) lines 277 to 303 [V] |
| A unique decision already has a home: `ScopeDecided`, one writer, each entry naming its predecessor, with a halt when one predecessor gets two successors. It has two purposes, selection and closure. | [event.rs](../crates/locust-proto/src/event.rs) lines 227 to 248 and 453 to 458, [fold.rs](../crates/locust-core/src/goal/fold.rs) lines 561 to 669 [V] |
| The goal's administrator signs every stage effect. Its daemon does so only with the local `flow` grant, which goal creation does not give. | [goal/flow.rs](../crates/locust-core/src/goal/flow.rs) lines 39 to 43, [node/flow.rs](../crates/locust-core/src/node/flow.rs) lines 11 to 30 [V] |
| Daemons exchange signed events, content, keys and delivery notices for effects that are already signed. One daemon can ask another for a decision only by sending it a signed event. | [sync.rs](../crates/locust-proto/src/sync.rs) module text and `SyncMessage` [V] |
| The read model is built only from effective events. An event that replay rejects is not shown. | [projection.rs](../crates/locust-core/src/goal/projection.rs) lines 50 and 102 [V] |
| The codec identifies enum variants by declaration index, so variants are only appended. | [codec.rs](../crates/locust-proto/src/codec.rs) module text [V] |
| A fork in the administrator's own log halts the administrator chain. | [chain.rs](../crates/locust-core/src/goal/chain.rs) lines 193 to 203 [V] |
| The editor says "Anyone. No lock: two members can work on the same task." | [words.ts](../sites/locust.farm/src/lib/formation-editor/model/words.ts) line 131, [PointBox.svelte](../sites/locust.farm/src/lib/formation-editor/ui/PointBox.svelte) line 252 [V] |
| O13 is deferred and not selected. V04 is conditional on it. | [status](../docs/formations-status.md) lines 32, 177 and 184 [V] |

## What the accepted documents already require

All [V], quoted or closely paraphrased.

From the [accepted direction](../docs/formations.md), section
"Accepted governance and scoped decisions":

- A scope may name an optional authority for exclusive reservation. It need not
  be the goal administrator.
- If that authority is unavailable, only the decision that needs it waits.
  Independent work and non-exclusive evidence continue.
- No reservation authority is implicit in open work.

From the [implementation plan](../docs/formations-implementation-plan.md),
decision D5 and package O13:

- A named, task-scoped, single-writer reservation authority.
- Durable generations, with explicit release and replacement.
- New exclusive claims wait when the authority is unreachable. No reassignment
  by clock alone. The validator rejects automatic lease expiry (section 4.2).
- The reservation and its retry receipt are saved together before ownership is
  confirmed. An uncertain acquisition is not retried as a fresh request.
- Never two effective generations at once. A write that uses a stale generation
  is rejected.
- The reservation generation is separate from local session fencing.
- Disconnected peers may keep an old confirmation or keep running. The feature
  promises neither that everyone sees one owner nor that a process stopped.
- The authority's daemon answers requests under explicit authority, as in D14.
  The contract and the manual state that dependency and its cost.
- Exit: V04 passes; unavailable and forked-authority states are explicit. V12
  stays true: a distributed reservation does not say the old process stopped.
- O0's exit rule also applies: every exclusive operation names its conflict
  rule, and finite models cover the main races.
- The gate: the M3 review decides from observed pooled work whether duplicate
  attempts cost enough to build this.

From the [semantics](../docs/formations-semantics.md):

- "YAML and exclusive reservations remain deferred". D14 does not bring O13
  forward. Reservation events are absent until O13 is selected.
- A decision stream that must be unique names its authority, scope,
  predecessor and generation.
- Four scenario rows, which the proposal answers one by one below: "Exclusive
  pickup", "Lost reservation reply", "Replacement and stale worker" and
  "Authority fork".

## Proposed format change

Three shapes were compared. [I]

| Shape | Example | Assessment |
| --- | --- | --- |
| A third start rule kind | `{ "kind": "exclusive", "by": … }` | Recommended. Reads as the opposite of `independent`. One new variant, no new field. |
| An option on `independent` | `{ "kind": "independent", "by": …, "exclusive": true }` | The two words contradict each other. Locust's own explanation of `independent` says "concurrent attempts may coexist". |
| A field on the work rules | `"work": { "exclusive": true, … }` | Says correctly that the lock belongs to the task, not to one rule. But it has to define what an offer means under a lock, which is more than the smallest change. |

Recommended:

```json
"work": {
  "propose": { "kind": "members" },
  "publish": { "kind": "members" },
  "starts": [{ "kind": "exclusive", "by": { "kind": "members" } }]
}
```

Meaning: a member matching `by` may ask for the task. One member holds it at a
time. Only the holder may start an attempt on it or post a result for it.

Rules for the validator: [I]

- `exclusive` must be the only entry in its `starts` list. Mixed with
  `independent` it would not lock anything. Mixed with `offered` it would need
  a rule for offers under a lock (open question 7).
- `by` takes the same selectors as `independent`.
- In a task type, `exclusive` by a group counts as narrower than `independent`
  or `exclusive` by the same or a larger group. `independent` under an
  `exclusive` parent is wider and is refused.
  ([delegation.rs](../crates/locust-core/src/goal/delegation.rs), `narrows`)
- `publish` keeps its meaning: who may post at all, including findings that are
  not attached to a task. The lock adds a second condition for results that are
  attached to the task.
- `schema_version` stays 1. The kind is added after the existing two, so
  formations without it keep their meaning and their hash.

## Who hands tasks out

Two options. [I]

| Option | For | Against |
| --- | --- | --- |
| The goal's administrator, always. No field in the formation. | Same answer as stages (decision 10; the review records the owner's reason, that a separate runner was confusing). Nothing to fill in when a goal starts. One computer to keep on, and a goal with steps already needs it on. | Departs from "named authority" in D5 and from "no reservation authority is implicit". The administrator's key signs every grant, and that key's log also carries membership and rules. |
| A role or member named in the formation, as selection and finish are today. | Matches D5 as written. Keeps reservation signing out of the administrator's log. | A new role to explain and to bind. At goal creation the only member is the creator, so the role is the administrator anyway until rules are bound again (editor review, finding 1). |

Recommended: the administrator, with no field. A named authority can be added
later as an optional field without changing what the first form means. The
departure from D5 is the owner's to accept (open question 2).

The administrator does not choose a winner by judgement. Its daemon answers
requests on its own, as D14 requires, under the local `flow` grant. If that
grant is missing, no task is ever handed out. The review found the same trap
for stages (open question 9).

## Proposed runtime

All [I]. Names are illustrative.

### Events

| Event | Signer | Meaning |
| --- | --- | --- |
| `ReservationRequested { context, after }` | A member matching the rule's `by` | Asks for the task in this round. `after` names the grant this request would follow, or nothing if the task was never taken. |
| `ScopeDecided` with a new purpose `Reservation` and action `Grant { request }` | The administrator | Gives the task to the request's author. This is one generation. |
| `ScopeDecided` with purpose `Reservation` and action `Revoke { last_accepted }` | The administrator | Takes the task back. `last_accepted` is nothing, or one exact point in the holder's log, as in `MemberRemoved`. |
| `ReservationReleased { grant }` | The holder | Gives the task up. Needs no administrator. |
| `AttemptStarted` gains `reservation: Option<EventId>` | The holder | Names the grant the attempt runs under. |

The stream key is the existing one: goal, task, round and purpose. So each
round of each task has its own chain of grants and revokes, written by one
key, each entry naming its predecessor. A generation's identity is its grant
event. A generation number for people is the count of grants in the verified
chain. No counter is signed.

The request is an event because a signed event is the only way one daemon can
ask another for a decision. That also gives the request a stable identity,
which the lost-reply case needs.

A grant is a decision, not an `EffectMaterialized`. An effect must be the one
configured consequence of its trigger, so every peer can predict it. When two
members ask at once, nobody can predict which the administrator's daemon saw
first. Peers read the signed grant; they never work the winner out.

### Rules checked on replay

The conflict rule, named as O0 requires: **one successor per predecessor in the
task round's reservation stream; two successors halt that stream.** This is the
rule `ScopeDecided` already enforces.

- The task is **free** at the head of the stream when there is no entry yet,
  when the head is a revoke, or when the head is a grant whose holder has
  released it.
- A grant is valid when the administrator signed it, the head it names is free,
  the request is valid and for this round, the request's `after` is the latest
  grant before it, and the requester is a member the rule's `by` selects. A
  grant after a release cites the release as evidence.
- A request whose `after` grant already has a later grant for someone else is
  over. Every peer that holds the later grant sees that. There is no refusal
  event and no waiting list (open question 6).
- A release is valid once per grant, from its holder.
- Under an `exclusive` rule an attempt must name a grant held by its author. A
  result for the task must name such an attempt. Under any other rule
  `reservation` must be empty.
- A holder's attempt or result counts only if it comes before the holder's own
  release in the holder's log, and, when a revoke exists, only if it is at or
  before the revoke's `last_accepted` point.
- A new round of the task starts a new stream. Grants do not carry over.
- Reviews, approvals, picking and closing are unchanged. They are not "work on
  the task" and need no grant.

### A result from a member who does not hold the task

| Case | Outcome |
| --- | --- |
| The member never held the task | Their daemon refuses to write the result and says who holds the task. If a result arrives anyway, replay rejects it like any result the publish rule forbids today. |
| The holder posts, then releases | Counts. The holder's own log orders the two. |
| The holder posts after their own release | Does not count. |
| The holder was replaced and posts at or before the cutoff | Counts. |
| The holder was replaced and posts after the cutoff, for example while offline | Does not count for the task. A peer that showed it as counting changes its status when the revoke arrives. The signed result is kept and shown under the task as posted without the task. |
| Any member posts a finding that is not attached to the task | Allowed under `publish`, as today. This is how work done without the task is still shared. |

Today the read model leaves out events that are not effective, so "kept and
shown" needs a small addition there. Without it, work by a replaced member disappears
from view, which the accepted documents say must not happen.

A task can still end up with results from two members, one before a
replacement and one after. The lock stops work at the same time. Picking one
result stays the job of the "Is one result picked?" point.

### Release and replacement

- **Release.** The holder signs it. It takes effect from the holder's log
  alone, so a holder can let go while the administrator is offline. The next
  grant still needs the administrator.
- **Replacement.** The administrator signs a revoke, then grants the next
  valid request. The revoke is a deliberate act by whoever acts as the
  administrator, through an API operation. It is never started by a timer.
  Its daemon fills `last_accepted` with the latest point it holds from the old
  holder, so everything it has seen still counts.
- **A removed member.** Removal does not free a task by itself. The
  administrator's daemon writes a revoke for each task the removed member
  holds, with the removal's cutoff.
- **Cancellation.** A revoke does not stop a process. Asking the old holder to
  stop is the existing `CancelRequested`, which today only the worker or its
  offerer may send. The administrator should be able to send it for an attempt
  under a grant it revoked.

### The four accepted scenarios

| Scenario | In this proposal |
| --- | --- |
| Exclusive pickup | Bea and Chen each sign a request that follows the same grant, or none. The administrator's daemon signs one grant, for Bea. Until a grant reaches Chen, Chen's daemon shows "waiting for the administrator". After it, "Bea has it". A timeout changes neither. |
| Lost reservation reply | The grant is an event in the administrator's log, saved before anything is sent. After a restart the daemon finds it at the head of its stream and signs nothing new. Bea's daemon receives it by ordinary sync. Bea's repeated API call returns her existing request, as a repeated attempt start returns its claim today. |
| Replacement and stale worker | A revoke with a cutoff, then a grant to Chen. Bea's later results do not count for the task. Her process may still run and her local session claim is untouched. She can still post a finding. |
| Authority fork | Two entries naming one predecessor give the existing `Halt::Successors` on that task round only. Both are disputed, nobody holds the task, and no result under either counts. Other tasks continue. The administrator continues the task with a new round, as the semantics already describe for a halted scope. |

### Offline and fork cases

- **Administrator offline.** No new grants. Requests wait and say so. Holders
  keep working, their results count, and they can release. Findings, reviews,
  picking and tasks under other rules are not affected.
- **Holder offline or gone.** The task stays held. Nothing expires. The
  administrator takes it back.
- **Requester gone before the grant.** The task is granted to a member who is
  no longer there. The administrator takes it back. A waiting list would make
  this worse, which is one reason not to have one.
- **Members cut off from the administrator.** They cannot take tasks. They can
  post findings. A holder among them keeps working on an old grant and may be
  replaced without knowing; see the cutoff rule above.
- **A fork of the administrator's own log.** This is not new, but it is worse
  than a stream halt: it stops everything the administrator signs after the
  fork, including membership. Signing every grant with that key gives one more
  kind of event that a restored backup or a second daemon could sign twice. A
  named authority would keep that risk in another member's log.
- **A fork of the holder's log.** Events past the fork give no authority, as
  today. A release past the fork cannot be relied on; the administrator
  revokes instead. A grant would pin its request's exact ancestry, as scoped
  decisions pin their evidence today, so it survives a later fork in the
  requester's log.

### Local sessions

Unchanged. A grant belongs to a member. A local claim belongs to one session on
one daemon for one attempt. The chain is grant, attempt, local claim, result.
Replacing a local session does not touch the grant, and revoking a grant does
not stop or fence the local session.

### What the daemon and API need

- Operations to take a task, release it, and, for the administrator, take it
  back. Task views show the holder, since which grant, and the states
  "waiting for the administrator", "posted without the task" and "disputed".
- A loop in the administrator's daemon, next to stage materialization, that
  grants the first valid request it holds for each free task. The grant is
  committed before it is sent. The stream head read from its own saved state
  is what prevents a second grant.
- Delivery of the grant to the requester, so the agent is told it has the
  task.
- New variants are appended, as the codec requires. The new field changes the
  encoding of `AttemptStarted`, so the protocol marker changes
  (`PROTOCOL_VERSION` is 3 today), and so does the API version. Under D3 a
  goal made before the change is refused, not converted.

### Checks before building

- A finite model next to the [organization models](tla/organization.md) for:
  two requests for one free task; restart after the grant is saved and before
  it is sent; a revoke racing a result written offline; two successors.
- Replay tests with reversed arrival for each row of the two tables above, and
  the V04 transcript through the public Engine.
- The validator, explanation, normalization and diff for the new kind, with
  conformance vectors shared with the editor's TypeScript checks.

## Proposed editor change

The point "Who works on a task?" gets one more answer. [I]

```
Who works on a task?

( ) Anyone
    No lock: two members can work on the same task.
(•) One member at a time
    A member takes the task. Others cannot work on it until it is released.
      Who can take it?  [ Any member ▾ ]
    The Locust of whoever started the goal hands the task out, so that
    computer has to be on.
    A task is not released by a timer. The member releases it, or whoever
    started the goal takes it back.
    Locust does not stop an agent that is already running.
( ) Only one role
    Only they can post results.
( ) A member is asked to do it
    The member asked can say no. Other members can still post results.
```

- **Short answer under the point:** "One member at a time. Others wait until
  it is released." With a role: "One "builder" at a time."
- **In words:** "One member works on a task at a time. A member takes the
  task. Until it is released, other members cannot start work on it or post
  results for it. The Locust of whoever started the goal hands tasks out. A
  task is not released by a timer."
- **Writes:** `starts` becomes one `exclusive` rule with the chosen `by`, and
  `publish` becomes the same selector, as "Only one role" does today.
- **Picture:** the Work drawing with one member joined to the task and the
  others dimmed.
- "No lock" stays beside "Anyone". It is still true there and now points at
  the alternative.
- "Takes" was rejected in the review because it promised ownership Locust did
  not give. With this built it is accurate.
- The six ways of working stay as they are. None of them uses the new answer
  (open question 10).
- The page offers the answer only when the released Locust accepts the new
  kind. Otherwise the copied prompt fails its own check.

## What this does not give

- It does not stop a process that is already running, on any machine.
- It does not promise one result per task.
- It does not promise that every member sees the same holder at the same
  moment. A member that is cut off may act on an old grant.
- It does not work without the administrator's Locust running.
- It does not protect against an administrator who signs two grants. That is
  detected and halts the task; it is not prevented.

## Considered and not recommended

- **Counting a replaced holder's results if they had not yet seen the
  revoke.** This is the rule closure uses for attempt starts. Here it would
  let an old generation use the holder's rights, which the "Replacement and
  stale worker" row forbids.
- **A waiting list.** A member that loses the race wants another task, not a
  place in line, and a queued member may be gone when its turn comes.
- **Expiry by clock.** Rejected by D5 and by the validator rules in the plan.
- **First start wins by timestamp, arrival or hash.** The accepted documents
  rule this out for every unique decision.
- **Locking only the start and leaving results open.** Simpler, but the review
  found that "who works" does not gate who delivers, and this would repeat it.

## Where it would land

Not a plan, only the files a build would touch. [I]

| Area | Files |
| --- | --- |
| Format | [organization.rs](../crates/locust-proto/src/organization.rs), [validation.rs](../crates/locust-core/src/organization/validation.rs), [normalize.rs](../crates/locust-core/src/organization/normalize.rs), [explanation.rs](../crates/locust-core/src/organization/explanation.rs), the generated contract and schema |
| Events | [event.rs](../crates/locust-proto/src/event.rs), the protocol and API versions in [lib.rs](../crates/locust-proto/src/lib.rs) |
| Replay | [fold.rs](../crates/locust-core/src/goal/fold.rs), [projection.rs](../crates/locust-core/src/goal/projection.rs), [state.rs](../crates/locust-core/src/goal/state.rs), [delegation.rs](../crates/locust-core/src/goal/delegation.rs), [goal/mod.rs](../crates/locust-core/src/goal/mod.rs) |
| Daemon | [node/flow.rs](../crates/locust-core/src/node/flow.rs), [claims.rs](../crates/locust-core/src/node/requests/claims.rs), [api.rs](../crates/locust-proto/src/api.rs) |
| Editor | [line.ts](../sites/locust.farm/src/lib/formation-editor/model/line.ts), [words.ts](../sites/locust.farm/src/lib/formation-editor/model/words.ts), [checks.ts](../sites/locust.farm/src/lib/formation-editor/model/checks.ts), [PointBox.svelte](../sites/locust.farm/src/lib/formation-editor/ui/PointBox.svelte), [diagrams.ts](../sites/locust.farm/src/lib/formation-editor/ui/diagrams.ts) |
| Documents | The semantics' deferred lines and event table, O13 and V04 in the plan and the status ledger, decision 13 in the editor plan, the guide |

## Open questions for the owner

1. Build this now, or wait for the M3 review? O13 says the review decides from
   observed pooled work. "Could be a good addition" is recorded as wanted, not
   as selected.
2. Who hands tasks out: always the member who started the goal, or a role
   named in the formation as D5 says? Recommended: the first, with the named
   role as a later option.
3. What is locked: starting work and posting a result for the task, or only
   starting work? Recommended: both. Findings not attached to the task stay
   open to anyone who may post.
4. When a holder goes quiet, is an explicit take-back by the administrator
   enough? The accepted documents rule out a timer. Should a member be able to
   ask for a take-back through Locust, or is that said outside it?
5. A replaced holder's late result: not counted, but kept and shown.
   Acceptable, given that it can change a status a peer has already shown?
6. No waiting list: a member that loses the race picks another task.
   Acceptable?
7. Should "A member is asked to do it" be able to lock as well, so that an
   accepted ask is the grant? Not in the smallest change.
8. Words. The code has `Claim` for a local session. Recommended: `exclusive`
   in the format, `reservation` in events, "takes" and "one member at a time"
   on the page. Should the page say "claim" or "lock" anywhere?
9. Should starting a goal whose formation has exclusive work, or steps, give
   the starter's Locust the `flow` grant? Without it no task is handed out and
   nothing says why.
10. Should one of the six ways of working use the new answer, for example
    Peer review, or should it stay an option only?
