# Formation editor specification

The `/formations` page lets someone choose an arrangement, edit its rules and
copy a prompt for their coding agent. The implemented editor uses a compact
rules matrix under six ways of working. It does not create goals or launch agents.
Read [formation authoring](guide/formation-authoring.md) for Locust operations and
[the prompt contract](formation-prompt.md) for the exact handoff text.

## Supported editing

The editor offers Open collaboration, Coordinator, shared pool, independent
attempts, review panel and pipeline examples. A preset is an editable definition,
not another runtime mode. The four shared questions are who adds tasks, who works
on them, when results count and whether one result is picked. Steps and task types
can specialize those answers within the contract.

Roles can be named and edited in place. Stage/task-type references must stay
consistent after renaming or removal. A step is a task added by the goal's
administrator; flow readiness and local execution permission remain separate.
Independent attempts do not claim an exclusive distributed lock.

Source JSON, copied prompts and saved formations can be opened without silently
losing unsupported fields. The [document model](../sites/locust.farm/src/lib/formation-editor/model/document.ts)
retains source, while [edits](../sites/locust.farm/src/lib/formation-editor/model/edit.ts)
maintain references. Undo, file actions, saved local formations and share links
remain explicit user actions. The editor uses browser storage; it does not read
private daemon catalogs or credentials.

## Validation contract

Offline checks run in TypeScript. The
[contract port](../sites/locust.farm/src/lib/formation-editor/contract/rules.ts)
checks only what can be established without goal membership, artifacts, machine
permissions or live state. Locust remains authoritative: the receiving agent
checks the exact definition through its installed CLI before saving it.

[check_formations.py](../scripts/check_formations.py) exports conformance cases
using Locust. [The site conformance test](../sites/locust.farm/src/lib/formation-editor/contract/conformance.test.ts)
requires matching diagnostic codes, phases, paths, messages and corrections.
JSON/parser failure messages use the page's plain wording. Semantic explanation
and normalization follow the same generated contract; new rules need conformance
coverage.

## Prompt and authority

[The prompt builder](../sites/locust.farm/src/lib/formation-editor/prompt/prompt.ts)
uses the [fixed prompt contract](formation-prompt.md). Data blocks preserve the
exact formation source and integrity values. The agent validates, explains and
can save a private draft; publishing is a separate choice. Creating a goal needs
actual members, input artifacts and local authority, so it is outside this prompt.

The prompt authorizes no owner administration, work grants or sharing. Delimiters
and hashes detect accidental corruption; they are not a security boundary around
a shell-capable coding agent. Locust's authenticated operations and the person's
native client policy govern what may execute.

## Verification

The [browser scenarios](../sites/locust.farm/e2e/formations.spec.ts) cover initial
selection, rules, role/stage edits, diagnostics, prompt copying, source round-trip,
sharing, phone layout and the page without JavaScript. When a current Locust binary
is available, they also validate the copied formation through the CLI.
Run the [site gates](../sites/locust.farm/README.md) for a change to this editor.
Browser fixtures and conformance cases do not establish first-time-user
understanding or a real-model handoff; those require separate experiments.
