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
| `roles` | Named groups of members; declared roles initially hold the host's agent |
| `context` | `guidance` (advice text) and `inputs` (named text or artifact inputs) |
| `work` | `propose`, `publish` and `starts` |
| `decisions` | `completion`, `selection` and `finish` |
| `task_types` | Named alternative rule sets a task can choose |
| `flow` | Named stages the daemon runs in order |
| `workspace` | Optional exact integrator and completion policy for the shared file tree |

`{"schema_version":2}` alone is a complete formation. It is the `open` preset.
A goal created without a formation follows `peer-review`.

Rules name who may act with a selector:

| Selector | Matches |
| --- | --- |
| `members` | Every member at the act's governance anchor |
| `role` | Holders of that role at the act's governance anchor |
| `participant` | One member, by 64-hex public key |
| `task_creator` | The member who opened the task (start and completion rules only). A stage's task is opened by the host's computer, which is no member, so in the rules a stage's task uses it may appear only in the `by` of an `offered` start |
| `contribution_author` | The result's author (completion rules only) |
| `any` | Anyone matched by one of a list of selectors |
| `nobody` | No one |
| `only_member` | The one member while the goal has exactly one member at the act's governance anchor |

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

`workspace` is optional. Its `integrator` names one participant directly;
a role is refused by [Rust validation](../crates/locust-core/src/organization/validation.rs)
and the [site's mirror](../sites/locust.farm/src/lib/formation-editor/contract/rules.ts).
Its `completion` pins the evidence required for a tree proposal. Enabling or changing active workspace authority requires a workspace
epoch with a typed checkpoint. The `rules bind` command binds the new rules and
moves an active tree to them in the same commit, carrying its accepted revision
forward. A formation's explicit `workspace` policy is used as written; otherwise
the command uses the host's agent as integrator and the goal's completion rule.
`workspace init` also preserves an existing workspace policy. An explicit
`--completion` changes its completion rule while retaining its integrator;
without a policy, initialization uses the host's agent and the goal's rule.
The [CLI regressions](../crates/locust/tests/workspace.rs) check both cases and
refusal of a task-creator rule before confirmation or capture.
The host's first files, with no parent and no composition source in an epoch
that starts empty, count when posted. Later changes follow the tree's rule.
Integration remains the host's explicit command in this phase. It also follows
the local level and formation eligibility. Generic task/document selection remains separate. See the
[workspace contract](workspace.md) for retained lineage, disputes and recovery.

### Task types and flow

A task type replaces the goal's `work` or `decisions` group, or both. A part it
leaves out keeps the goal's whole group. A top-level task may name a task type.
A subtask must have narrower rules than its parent task; see
[delegation.rs](../crates/locust-core/src/goal/delegation.rs).

A flow stage names its `recipients`, an optional `task_type` and what it
`requires`: a `publication`, `review`, `completion` or `selection` in another
stage. A stage runs once for each rules binding, not once per task. Automatic
stage offers name the task's current round, including after `task revise`.
A revision without `--task-type` keeps the current type; replay refuses a stage
revision whose start or completion rules require `task_creator` to
act. See [flow.rs](../crates/locust-core/src/goal/flow.rs) and
[fold.rs](../crates/locust-core/src/goal/fold.rs).

A step that cannot be signed or materialized is listed as `cannot_materialize`
in the goal's stalled steps. The daemon skips it for that pass and continues
other work; the next goal change or startup retries it. A store failure still
stops the daemon. Reconnecting an agent both receives waiting deliveries and
runs any step that waited for its signature. These paths are covered in the
[delivery](../crates/locust-core/src/node/tests/delivery.rs) and
[authorization](../crates/locust-core/src/node/tests/authorization.rs) tests.

## Presets

Six formations are built into the binary in
[presets.rs](../crates/locust-proto/src/organization/presets.rs): `open`,
`peer-review`, `pipeline`, `independent-attempts`, `review-panel` and
`directed`. `examples/formations/` holds copies, and `check_formations.py` keeps
them equal. `directed` gives its `lead` the choice of result and task closure;
its `reviewer` may approve its own result. `peer-review` and the pipeline's draft
also count the goal's only member's result when posted. `review-panel` always
requires two eligible reviewers. The guide's
[preset table](guide/formations.md#presets) describes each one.

## Goals, names and roles

The person's `goal create` command signs three events together: the genesis,
which names the host's agent and pins the formation's semantic hash; that agent's
admission; and the first rules binding. It takes a preset name (`--formation`) or
formation JSON (`--formation-json`), plus `--name` and `--inputs`. It does not
read the private catalog. The person names the enrolled agent who becomes the
host's agent. All three are signed by the goal's own signing key, which the
hosting computer makes with the goal and keeps; the goal identifier commits to
that key and to the host's agent. The key is never a member. See
[goals.rs](../crates/locust-core/src/node/requests/goals.rs) and
[event.rs](../crates/locust-proto/src/event.rs).

Only events signed by the goal's key change membership, role holders, rules, task rounds or
the farm publication policy, and only it signs a stage's steps. Events of these
kinds from anyone else are excluded, and anything else it signs is excluded as
a non-member's. Roles never grant this power. The host's agent is an ordinary
member: it cannot leave or be removed, and disconnecting it stops no host
command. See [chain.rs](../crates/locust-core/src/goal/chain.rs).

Every admission signs the member's name into the goal. A rules binding supplies
every required input and starts each newly declared role with the host's agent.
The host changes a role with `role give` and `role take`. A role used to pick a
result or close a task has one holder; the host's daemon keeps each role's kind
for the life of the goal. Taking the last holder or removing that member falls
back to the host's agent. A new binding keeps all role lists, even those it does
not declare, so work under earlier rules can still use them. Roles are resolved
at each act's own governance anchor, not where its task's rules were bound.
The host's daemon checks role kind before signing; replay requires one holder
when a deciding role is used.

Where the completion rule needs reviewers the host's agent cannot supply alone,
`goal add` and `goal invite` choose the reviewer role by default. `--no-role`
admits a member without that role. A role carried by a ticket becomes part of
the signed admission.

`rules bind` changes the goal's defaults. Its plan records the current rules
event; if it changes before confirmation, locust.farm refuses the update.
New tasks use the new rules;
existing tasks keep the rules they were opened under. `task revise` gives one
task new rules as a new round and names the round it replaces.

## How decisions are evaluated

Every work event names its task (or the goal), the rules round it acts under,
and the host's event it last saw. Each daemon checks the event against
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
  leaves out the author when the rule says so. Without pinned evidence, each
  member's latest effective review is read: a reject withdraws that member's
  approval, without vetoing another member's. A selection or materialized step
  keeps the exact reviews it pinned. A review, named check or declaration cannot
  name a host record earlier on the host's chain than its subject's. Under a rule that asks for no reviews, a
  member's review is an opinion and never contributes to completion. Several results can count at once. A task round is complete when any
  result counts or one is selected.
- **Selection.** Only the selection decider may select, and only a result that
  counts. Each decision names the previous one in that task's chain (`--expected`).
  Shared-document selections across rules rounds follow governance chronology,
  not the lexical order of event hashes; scoped historical selections remain.
- **Closing.** Only the finish decider may close or reopen. A close blocks new
  attempts by members who have seen it. Attempts started without seeing it stay
  valid.

See [fold.rs](../crates/locust-core/src/goal/fold.rs),
[closure.rs](../crates/locust-core/src/goal/closure.rs) and
[commitments.rs](../crates/locust-core/src/goal/commitments.rs).

Retained fork evidence does not replace ordinary synchronization with active
members. Historical-only contacts can receive evidence-only exchanges; active
peers continue exchanging subsequent work. The
[driver regression](../crates/locust-core/src/sync/tests/driver.rs) enforces this
distinction.

Proof event sets use persistent radix trees of bitmap words. Cached ancestor
closures share unchanged branches; they do not allocate one full-history bitmap
per ancestor. The tests in
[commitments.rs](../crates/locust-core/src/goal/commitments.rs) check exact sparse
unions, history growth and bounded node growth for interleaved author prefixes.

The [commit path](../crates/locust-core/src/node/commit.rs) advances signed
projections only for new event transactions or changed definition evidence.
Local revisions and delivery/join reconciliation still take effect
without refolding history. The
[definition index](../crates/locust-core/src/node/definitions.rs) retains signed
rule references and refreshes their interpretation when referenced objects or
keys change, including streamed arrivals. [Content regression tests](../crates/locust-core/src/node/content_graph_tests.rs)
check that ordinary uploads, downloads and local touches do not refold the goal.

## Automatic steps and delivery

The daemon computes three kinds of automatic step from the goal's records, in
[goal/flow.rs](../crates/locust-core/src/goal/flow.rs):

- Open a stage's task when its requirements are met, and send it to the stage's
  recipients.
- Offer a stage's task to each recipient, when the stage's start rules let the
  host make offers.
- Ask each member who may review a new result for a review.

The hosting computer signs stage tasks, stage offers and review requests for
stage tasks with the goal's key, whether or not the host's agent is connected.
The result's author signs other review requests. A daemon signs each eligible
step for its local member without a separate flow setting
([node/flow.rs](../crates/locust-core/src/node/flow.rs)).

Recipients are matched at the signed step's governance anchor. When deciding
what to send next, the daemon uses holders and members at the current head.
Review requests are sent only for results that do not yet count and have not
been selected on an open task; a new reviewer does not receive requests for all
past results. Automatic offers are wanted only for the current round of a stage
task that is not closed, completed or selected. An already-signed request or
offer keeps its historical authority. When an ordinary result's author has left
or been removed, its unsent review requests keep the recipients at the result's
own anchor; adding a member does not ask that author to sign another request.
Prerequisites use an upstream task's revised round at the materialization's
governance anchor. Already-materialized effects retain their historical evidence.
[Goal tests](../crates/locust-core/src/goal/tests.rs) cover revised prerequisites
and document selections across rules rounds.

Each step has one ID, a hash of the goal, task, trigger, action and target. A
step signed twice is still one action. The signed event and its outbox entries
are saved in one transaction. The daemon retries until the recipient's daemon
saves the inbox entry and sends a receipt, and resumes after a restart; see
[delivery.rs](../crates/locust-core/src/node/delivery.rs).

Delivery is not execution. The receipt, the agent's acknowledgment and the start
of work are separate records. The agent's local level and any task allowance
still govern taking a task, and
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

- Formation rules decide eligibility; the owner chooses each agent's local level
  and any task allowance.
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
