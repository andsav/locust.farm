# Organization blueprints: accepted direction

Date: 2026-10-04. **Status: accepted product direction and authoring requirements;
not implemented.** The [implementation plan](organization-blueprints-implementation-plan.md)
now sequences the runtime, agent authoring, Polaris, and complete public manual.
The concrete schema, distributed decision semantics, and migration strategy
remain proposed choices with explicit decision gates. The current
[protocol-1 coordinator behavior](protocol-v1.md) remains enforced until replaced.

This decision follows the [organization blueprint research](../research/organization-blueprints.md)
and the owner's agreement that agents must easily author blueprints and Polaris
should offer a visual authoring experience. It does not adopt every tentative
protocol mechanism or example field name in that research or the conversation.

The [public documentation plan](public-documentation-plan.md) specifies the full
locust.farm manual required alongside implementation, including versioned human
and agent references, tested tutorials, and publication verification.

## Accepted model

An organization blueprint is a reusable, declarative agreement about how a
group works together. A goal selects and pins a version; tasks inherit defaults
or select an allowed variation. The coordinator workflow becomes one arrangement
among others. Open collaboration must permit useful findings and contributions
without mandatory assignments, reviews, or a single accepted result.

The model separates participants/roles, shared context, work/attempts/contributions,
flow, and decisions. Completion follows a task's explicit rule: what evidence is
required and whose judgment counts. Submission, approval, selection, and applying
a change locally remain distinct. Organization rules do not grant local tool,
filesystem, spending, or sharing permissions.

## One contract, two authoring experiences

Agents and Polaris edit the same organization definition. There must not be a
second visual-only organization language or a restricted agent-only format.
Locust owns the schema, validation, and runtime semantics. Polaris provides a
visual editor over that contract; Locust remains usable without Polaris.

The design must support a short path from a user's description to a valid
blueprint with a readable explanation. Agent authoring is a primary use case,
not an import/export feature added after the visual editor.

### Agent authoring requirements

- Small declarative documents with optional structure and reusable examples.
  A simple arrangement must not require building a graph or enumerating all
  future work. YAML and JSON are candidate authoring representations, not yet
  a frozen wire schema.
- A discoverable machine-readable contract and concise authoring instructions,
  so agents do not guess field names or rely on long prompt conventions.
- Structured validation errors identifying the field, violated rule, and useful
  correction. Distinguish invalid syntax, unsupported semantics, missing role
  bindings, and unavailable runtime capabilities.
- Operations for reading, drafting, validating, comparing, and publishing a
  definition, with a normalized effective-rules view. Exact CLI/MCP names remain
  to be designed. Every default affecting authority or completion must be visible.
- A short explanation of who may act, how work moves, what completes a task,
  and which instructions are advisory. Guidance cannot silently become authority.

### Polaris visual authoring requirements

- Present participants/roles, shared context, work choices, completion rules,
  and optional flow in terms people can understand. Simple arrangements should
  be editable without drawing an execution graph.
- Use the same validator and effective-rule explanation as agent authoring.
  Unsupported combinations must be visible rather than accepted only by one UI.
- An agent-authored blueprint must open for visual editing, and visual edits
  must remain readable and editable by agents without loss of meaning.
- Keep layout, colors, and other presentation state separate from organization
  semantics. Moving a visual element must not change authority or execution.
- Preserve supported semantics on a round trip. When an editor cannot understand
  a newer construct, show that limitation and preserve the definition rather
  than silently dropping the construct.

## Drafts, publication, and running goals

Keep an editable draft separate from an immutable published definition and a
goal/task instance that binds participants and inputs. Saving or validating a
draft does not launch agents, share data, or change an existing goal.

Human and agent edits to the same draft need revision checks and reviewable
diffs so one does not silently overwrite the other's work. Publishing pins exact
semantics. Later definition edits do not reinterpret existing work; changing an
active arrangement requires an explicit, authorized revision boundary.

Polaris integration remains a separate delivery step. These authoring requirements
constrain the Locust contract now without making the desktop editor a prerequisite
for standalone collaboration.

## Verification required before claiming this works

These are future acceptance criteria, not checks already run:

1. An agent creates Open collaboration and Coordinator arrangements from the
   same primitives, validates them, and explains their different completion rules.
2. An agent-authored definition opens in Polaris; a person edits a rule; the
   agent reads the changed definition and accurately explains its effect.
3. A semantic round trip preserves the definition; layout-only changes preserve
   executable identity. Invalid and unsupported rules get consistent diagnostics.
4. Concurrent draft edits report a revision conflict without losing either
   author's work. Saving and publishing do not mutate already pinned instances.
5. Task-specific variations preserve parent scope and local permissions, and
   the agent can explain exactly what remains before task completion.

The research's unresolved questions still need concrete answers: reservation and
decision finality, membership/rule epochs, proof retention, default templates,
protocol-1 compatibility, and the exact declarative schema. Acceptance of the
product direction does not establish protocol correctness or runtime readiness.
