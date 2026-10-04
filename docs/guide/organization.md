# Presets, composition and lifecycle

**Status: implemented in the development runtime.** Choose by the
required guarantees, then inspect the matching generated example and core
explanation. A visual shape alone is not the executable rule.

## Choose a preset

| Arrangement | Intended coordination | Guarantee boundary |
| --- | --- | --- |
| Open collaboration | Members publish findings, including unattached artifacts | No universal task acceptance or selected code head |
| Coordinator | A task binds assignment/selection authority | Authority is scoped to that task, not all goal work |
| Shared pool with peer review | Eligible participants attempt work and exact candidates receive review | Local claim generations are not distributed exclusive reservations |
| Independent attempts | Several participants independently contribute | Parallel qualification does not imply one winner |
| Review panel | Distinct eligible reviewers satisfy a threshold | Threshold approval is not consensus on a single output |
| Pipeline/handoff | Named evidence enables configured downstream work | Durable offer delivery is not remote receipt, local permission or execution |

Canonical JSON examples are available through [offline authoring](blueprint-authoring.md).
They are supported definitions; they do not prove a live instance is ready.
Exclusive distributed reservation remains deferred and must not be inferred from
local worker claims or a peer-review task queue.

## Compose scopes deliberately

A task can specialize rules within its parent's authority. Keep open research,
code review and a benchmark selection in separate explicit scopes when their
completion and authority differ. A missing benchmark selector should not freeze
research. Dependencies name exact evidence or completed rounds, not an ambiguous
latest-result title.

Configured transitions identify an authorized materializer, intended action and
recipients. Other replicas can derive readiness and show the materializer is
unavailable; they cannot sign as it. Local signing/execution grants still apply.
A child task or handoff is materialized once logically, then delivery retries
reuse that identity. [Operations](operations.md) covers restart behavior.

## Definition and instance lifecycle

A draft is editable. Validation attaches to its exact revision; concurrent edits
need compare-and-swap and a conflict that preserves the losing work. Publishing
must name the validated revision and must not race an unnoticed edit. A published
definition is immutable. An instance supplies concrete authenticated role/input
bindings and pins its semantic identity.

Updating future defaults does not reinterpret an already pinned task. Rebinding,
revising an active round or reopening work are explicit authorized current-model
transitions naming the expected prior context. Old signatures retain their original
meaning. These operations do not migrate schema formats or activate old readers.
The authenticated `blueprint draft`, `blueprint publish` and presentation
operations implement this lifecycle. `goal create --blueprint-json` instantiates
a definition; `rules bind --expected` changes future defaults. `task revise
--expected-round` and scoped reopen explicitly change active work. Inspect
the `effective_rules_json` field returned by
`task show --goal GOAL --task TASK` before acting on a pinned task.

[Completion](completion.md) describes candidate verdicts;
[authoring](blueprint-authoring.md) gives offline and private publication commands.
