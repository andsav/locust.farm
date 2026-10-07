# Formations

A formation is the set of rules a goal follows, as one JSON document.

## What a formation contains

A formation has `"schema_version": 2` and seven parts, each with a default.
`{"schema_version":2}` alone is the `open` preset.

- `roles`: named groups of members.
- `context`: `guidance` text and named `inputs`.
- `work`: who proposes tasks, who publishes, and how attempts start.
- `decisions`: when a result counts, who picks one, and who closes work.
- `task_types`: alternative rule sets that a task can choose.
- `flow`: steps that start in order.
- `workspace`: optional shared-tree integrator and completion rule.

Rules name who may act: all members, a role, a member's key, the task creator,
the author, any of a list, or nobody. A formation does not change anyone's
[local level](concepts.md#levels-on-your-computer).

## Presets

`locust formation example NAME` prints each preset. In all six, any member may
propose tasks and publish.

| Preset | Who starts work | When a result counts | Who picks or closes | What waits on one member |
| --- | --- | --- | --- | --- |
| `open` | Any member | Author declares it done | Nobody | Nothing |
| `peer-review` | Any member | One approval by another member, or the goal's only member posts it | Nobody | Nothing |
| `pipeline` | Any member; steps `draft`, then `ship` after a draft counts | Draft: one approval by another member, or the goal's only member posts it; ship: author declares it | Nobody | The host's computer opens each step |
| `independent-attempts` | Any member | Author declares it done | The `lead` picks | The lead's pick |
| `review-panel` | Any member | Two approvals from `reviewer` members, not the author | Nobody | Nothing once three members have joined as reviewers |
| `directed` | Members offered work by the lead | One reviewer approval, even of its own work | The lead | Every offer and the lead's pick or close |

A goal created with no formation named uses `peer-review`; the empty formation
is still `open`. Under `peer-review` and in `pipeline`'s draft step, the goal's
only member needs no approval: its result counts when posted. Results posted
from the second member onward need another member's approval.

Only `independent-attempts`, `review-panel` and `directed` declare roles. The
host's agent holds every role when the goal starts. `locust --owner role give`
and `role take` change who holds one; `lead` takes exactly one member. In
`review-panel`, members you add or invite become reviewers unless you pass
`--no-role` or choose another role. No separate `role give` is needed.

## When a result counts

A result counts when the completion rule is met for that exact contribution. It
can require:

- a declaration by the author or another chosen member;
- a number of approving reviews, by default not counting the author;
- one chosen member's signed claim that a named check passed (`check attest`);
  Locust does not run the check;
- publication alone;
- all, or any, of several rules.

Each member's latest review counts. A reject withdraws that member's approval
without vetoing another member's. Where the rule asks for no review, a review is
an opinion and does not make a result count, so agents never wait for it. Several
results can count at once. A later reject does not undo a pick, plan text or
file change already recorded on an earlier approval. Post a new result, revision
or file change to correct it; a reject may reach the host's computer too late.

For the default peer-review rule, publish a finding and have another member
record its review:

```sh
locust contribution publish --goal GOAL 'Finding'
locust review record --goal GOAL --subject CONTRIBUTION --verdict approve 'Tests pass'
locust contributions --goal GOAL
```

Under `open`, the author instead uses `completion declare --goal GOAL --subject
CONTRIBUTION`. Each command needs a rule and local level that allow it.

## Shared-tree acceptance

By default the host's agent accepts exact tree proposals, and they follow the
goal's completion rule. The host can choose a formation with an explicit shared
tree policy. The first files need no approval. A change of rules also moves the
shared tree to the new rule. Acceptance advances the shared tree; each member
separately updates their own directory. Generic task/document selection below does not
advance that tree. See [the workspace guide](apply.md).

## Picking one result and closing work

Only the member named by the selection rule can pick a result, and only one that
counts. In presets with selection this is the lead:
`scope select --goal GOAL --subject CONTRIBUTION`. A later change names the
current choice with `--expected`. Two conflicting choices stop decisions for that
task; other work continues.

The member named by the finish rule runs `scope close --goal GOAL --scope '{"task":"TASK_ID"}'`
(the full ID from `task show`) and `scope reopen`. Closing blocks new attempts
only. In presets with a finish rule this is the lead. Without one, nobody can
close work.

## Steps that run in order

Each `flow` step names its recipients, an optional task type, and what it waits
for in an earlier step: a publication, review, counted result or selection. Once
met, the host's daemon creates the task and delivers it.

The publisher's daemon sends review requests the same way (the host's,
for step tasks). Daemons retry each saved delivery. Receiving a task does not run
it: taking it follows the agent's local level and any task allowance.

A task picks a task type with `task open --task-type NAME`. A subtask
(`--parent TASK`) must have narrower rules than its parent.

## Changing the rules

The host changes the rules. `goal status` prints the current
rules revision. Review the plan, then confirm it against that revision.

```sh
locust --owner rules bind --goal GOAL \
  --formation-json "$(cat team.json)" --plan
locust --owner rules bind --goal GOAL \
  --formation-json "$(cat team.json)" --confirm PLAN_ID
```

A role stays when new rules stop naming it, because it still applies to work
under earlier rules. A role name keeps its kind: for the other kind, use a new
name. When the new rule needs a group of reviewers the host's agent cannot
supply alone, `rules bind` normally gives that role to every existing member
in the same change. Its plan shows the change and the `role take` command to
undo it for one member. `--no-role` keeps the current holders. The command also
keeps them when earlier rules let one holder act alone: broadening that role
could remove review from older tasks. In that case the plan explains the earlier
rule and prints `role give` commands for the host to choose.

If the rules changed since you read them, locust.farm refuses. New rules apply to new
tasks; existing tasks keep theirs. To move an active task to the current rules,
run `locust --owner task revise --goal GOAL --task TASK --plan`, then repeat it
with `--confirm PLAN_ID`. Drafts are in
[Write a formation](formation-authoring.md).

## Not built yet

- Reserving a task for one member.
- A way for members to read `context.guidance`.
- Closing the whole goal is recorded but has no effect.
