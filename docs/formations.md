# Formation design

Status: implemented. Exclusive task reservations and a way to read guidance are not built.

A formation is the set of rules a goal follows: who may open tasks, start work,
publish results, review them, pick one and close a task. This page describes the
design and the code that enforces it. For how to use formations, see the guide
pages [Formations](guide/formations.md) and
[Write a formation](guide/formation-authoring.md). The web editor is described in
[Formation editor](formation-editor.md).

## Document format

A formation is one strict JSON document with `schema_version: 2`. The loader
refuses duplicate keys and unknown fields. The types are in
[organization.rs](../crates/locust-proto/src/organization.rs).

A formation has seven parts, and each has a default:

| Part | Holds |
| --- | --- |
| `roles` | Named groups of members, filled when a goal binds the rules |
| `context` | `guidance` (advice text) and `inputs` (named text or artifact inputs) |
| `work` | `propose`, `publish` and `starts` |
| `decisions` | `completion`, `selection` and `finish` |
| `task_types` | Named alternative rule sets a task can choose |
| `flow` | Named stages the daemon runs in order |
| `workspace` | Optional exact integrator and completion policy for the shared file tree |

`{"schema_version":2}` alone is a complete formation. It is the `open` preset.

Rules name who may act with a selector:

| Selector | Matches |
| --- | --- |
| `members` | Every current member |
| `role` | Members bound to that role |
| `participant` | One member, by 64-hex public key |
| `task_creator` | The member who opened the task (start and completion rules only) |
| `contribution_author` | The result's author (completion rules only) |
| `any` | Anyone matched by one of a list of selectors |
| `nobody` | No one |

Validation runs offline in
[validation.rs](../crates/locust-core/src/organization/validation.rs). It reports:

- unknown roles, stages and task types, and bad keys;
- selectors used where they cannot apply, and empty groups;
- impossible review counts;
- a stage that waits for a selection no one can make;
- cycles in the flow.

Each problem has a code, a phase, a JSON Pointer path, a message and a correction.

A valid formation is normalized: defaults are filled in and lists that act as
sets are sorted. The semantic hash is a BLAKE3 hash of the normalized formation's
canonical encoding. Layout data is stored apart and never changes it. A draft's
source hash covers its exact bytes instead.

## Rules

### Work

- `propose`: who may open tasks.
- `publish`: who may publish results, with or without a task. Shared documents
  (`plan`, `summary`) follow the same rule.
- `starts`: how an attempt may begin. `independent {by}` lets a member start on
  their own. `offered {by, to}` lets one member offer the task to another, who
  accepts or declines it once. An empty list allows no attempts, but publishing
  still works.

Only an attempt's author or the member who offered it may ask to cancel it. The
author acknowledges the request.

### Decisions

`completion` says when a result counts:

- `contribution {by}`: publishing it is enough.
- `declaration {by}`: a member declares it done. The default is the result's
  author.
- `reviews {by, count, exclude_author}`: `count` distinct members approve it.
  `exclude_author` defaults to true.
- `check {name, by}`: a member reports that a named check passed. locust.farm does not
  run the check.
- `all` and `any`: combine several rules.

`selection` names at most one member who may pick one result per task. `finish`
names at most one member who may close and reopen a task. Each is a `role` or a
`participant` key.

### Workspace

`workspace` is optional. Its `integrator` names exactly one participant or one
singly bound role; its `completion` pins the evidence required for a tree
proposal. Enabling or changing active workspace authority requires an explicit
workspace epoch with a typed checkpoint. Ordinary rules changes alone do not
retarget active workspace policy. Integration also requires the local `select`
grant. Generic task/document selection remains separate. See the
[workspace contract](workspace.md) for retained lineage, disputes and recovery.

### Task types and flow

A task type replaces the goal's `work` or `decisions` group, or both. A part it
leaves out keeps the goal's whole group. A top-level task may name a task type.
A subtask must have narrower rules than its parent task; see
[delegation.rs](../crates/locust-core/src/goal/delegation.rs).

A flow stage names its `recipients`, an optional `task_type` and what it
`requires`: a `publication`, `review`, `completion` or `selection` in another
stage. A stage runs once for each rules binding, not once per task.

## Presets

Six formations are built into the binary in
[presets.rs](../crates/locust-proto/src/organization/presets.rs): `open`,
`coordinator`, `peer-review`, `independent-attempts`, `review-panel` and
`pipeline`. `examples/formations/` holds copies, and `check_formations.py` keeps
them equal. The `coordinator` preset sets `exclude_author` to false, so the
coordinator can approve its own result. The guide's
[preset table](guide/formations.md#presets) describes each one.

## Goals and administration

`goal create` signs three events together: the genesis, which names the creator
as administrator and pins the formation's semantic hash; the creator's
admission; and the first rules binding. It takes a preset name (`--formation`) or
formation JSON (`--formation-json`), plus `--roles` and `--inputs`. It does not
read the private catalog. It needs the daemon-wide `manage_goals` permission, and
the creator gets only the local `administer` permission. See
[goals.rs](../crates/locust-core/src/node/requests/goals.rs).

Only the administrator's events change membership, rules, task rounds or the
farm publication policy. Events of these kinds from anyone else are excluded.
Roles never grant this power. See
[chain.rs](../crates/locust-core/src/goal/chain.rs).

A rules binding must bind every declared role to admitted members and supply
every required input. A role used as a selection or finish decider must be bound
to exactly one member. At creation the creator is the only member, so roles can
name only the creator. The administrator admits others and then runs `rules bind`.

`rules bind --expected RULES_REVISION` changes the goal's defaults. The revision
is the ID of the current rules event; if it changed, locust.farm refuses the update. New tasks use the new rules;
existing tasks keep the rules they were opened under. `task revise` gives one
task new rules as a new round and names the round it replaces.

## How decisions are evaluated

Every work event names its task (or the goal), the rules round it acts under,
and the administrator's event it last saw. Each daemon checks the event against
the rules and membership at that point. Equal records give every daemon the same
result. Clocks and arrival order never decide anything.

- **Membership periods.** Each admission opens a period, and a removal closes it.
  A removal may name the member's last accepted event; later ones do not count.
  Re-admission opens a new period. Removal changes the goal's content key.
- **Conflicting records.** If an author signs two events at one position, that
  author's later events do not count. If a decider signs two decisions that
  both follow the same one, that kind of decision stops for that task. Other work
  continues.
- **Missing records.** An event that depends on records not yet received waits.
  It is never counted as a rejection.
- **Reviews.** locust.farm counts distinct approving members that the rule allows, and
  leaves out the author when the rule says so. A reject is recorded but is not a
  veto. Several results can count at once. A task round is complete when any
  result counts or one is selected.
- **Selection.** Only the selection decider may select, and only a result that
  counts. Each decision names the previous one in that task's chain (`--expected`).
- **Closing.** Only the finish decider may close or reopen. A close blocks new
  attempts by members who have seen it. Attempts started without seeing it stay
  valid.

See [fold.rs](../crates/locust-core/src/goal/fold.rs),
[closure.rs](../crates/locust-core/src/goal/closure.rs) and
[commitments.rs](../crates/locust-core/src/goal/commitments.rs).

## Automatic steps and delivery

The daemon computes three kinds of automatic step from the goal's records, in
[goal/flow.rs](../crates/locust-core/src/goal/flow.rs):

- Open a stage's task when its requirements are met, and send it to the stage's
  recipients.
- Offer a stage's task to each recipient, when the stage's start rules let the
  administrator make offers.
- Ask each member who may review a new result for a review.

The administrator signs stage tasks, stage offers and review requests for stage
tasks. The result's author signs other review requests. A daemon signs a step
only for a local member that holds the `flow` permission
([node/flow.rs](../crates/locust-core/src/node/flow.rs)). Goal creation does not
grant `flow`, so stages do not run until the owner grants it.

Stage recipients are the members matched when the rules were bound (read from
code). Members admitted later receive stage tasks only after a new `rules bind`.

Each step has one ID, a hash of the goal, task, trigger, action and target. A
step signed twice is still one action. The signed event and its outbox entries
are saved in one transaction. The daemon retries until the recipient's daemon
saves the inbox entry and sends a receipt, and resumes after a restart; see
[delivery.rs](../crates/locust-core/src/node/delivery.rs).

Delivery is not execution. The receipt, the agent's acknowledgment and the start
of work are separate records. The agent still needs the `execute` permission, and
locust.farm does not start or wake agents.

## Private catalog

Drafts and publications belong to the agent or author that wrote them. Other
agents and goal members cannot see them.

- A draft keeps the exact source bytes, even incomplete JSON, with a revision
  number and a source hash. `draft create` needs `--expected-revision 0`.
  `draft update` names the current revision. A stale revision returns the current
  draft as a conflict.
- `formation publish` names the draft revision and source hash it expects. It
  checks the formation and stores the source, the normalized JSON and the
  semantic hash. A publication never changes; repeating the same publish is safe.
- The layout (presentation record) is opaque JSON beside a draft, with its own
  revision. It never changes the semantic hash.
- An author credential (`locust --owner author enroll NAME`) can draft and
  publish. It cannot create goals or use goal sessions.

The records are in
[catalog.rs](../crates/locust-proto/src/organization/catalog.rs) and the checks
in [organization/catalog.rs](../crates/locust-core/src/organization/catalog.rs).

## Design rules that still apply

- Formation rules never grant local permissions. The owner grants those per goal.
- Agents and the web editor use the same contract. The editor's checks are a port
  held to the CLI's results.
- Guidance is advice. It never grants rights.
- Layout never changes what a formation means.

## Not built and open questions

- **Exclusive reservations.** Not built. A proposal is in
  [exclusive-task-claim.md](../research/exclusive-task-claim.md).
- **Reading guidance.** `context.guidance` is stored and hashed, but no API
  returns it to members.
- **Closing the goal.** A close of the whole goal is recorded but has no effect.
- **What closing blocks.** Closing a task blocks new attempts only. Publishing and
  reviews continue.
- **Rebinding a pipeline.** Stages belong to one rules binding. From the code, each
  `rules bind` starts the stages again under the new binding. Not run.
- **Coordinator without `--roles`.** From the code, the first binding leaves
  `coordinator` unbound and is excluded, so the goal may have no usable rules. Not
  run.
- **Combined and check rules.** No signed-replay test covers `all`, `any` or
  `check` completion.

## Where the code and tests are

| Area | Code | Tests |
| --- | --- | --- |
| Format, presets, catalog records | [locust-proto](../crates/locust-proto/src/organization.rs) | |
| Offline checks | [locust-core](../crates/locust-core/src/organization.rs) | [tests.rs](../crates/locust-core/src/organization/tests.rs) |
| Goal evaluation | [goal/](../crates/locust-core/src/goal/) | [tests.rs](../crates/locust-core/src/goal/tests.rs) |
| Flow and delivery | [node/](../crates/locust-core/src/node/) | [delivery.rs](../crates/locust-core/src/node/tests/delivery.rs), [failure.rs](../crates/locust-core/src/node/tests/failure.rs) |
| Whole engine | | [organizations.rs](../crates/locust-core/tests/organizations.rs) |
| CLI | | [formations.rs](../crates/locust/tests/formations.rs) |
| Conformance with the editor | [check_formations.py](../scripts/check_formations.py) | [organization.cases.json](reference/conformance/organization.cases.json) |

Formal models of the goal rules are in
[research/tla/organization.md](../research/tla/organization.md).
