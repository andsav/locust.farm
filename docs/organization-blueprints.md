# Organization blueprints: accepted direction

Date: 2026-10-04. **Status: accepted product direction and authoring requirements.**
API 3 / protocol 3 now implements the standalone organization runtime, private
catalog and agent operations. The [implementation plan](organization-blueprints-implementation-plan.md)
remains the frozen delivery scope. The separate
[execution ledger](organization-blueprints-status.md) records completed checks
and remaining formal, native/client, transport and publication boundaries. The
historical [protocol-1 coordinator behavior](protocol-v1.md) has been replaced;
it is not available as a fallback.

The installed `locust blueprint contract` command exports the schema, command
catalog and examples without connecting to a daemon. Validation rejects unknown
fields, duplicate keys, unresolved role references, invalid selector scopes,
impossible explicit thresholds and static flow cycles. It normalizes defaults
and unordered rule sets before deriving semantic identity. These are offline
definition checks; required member/input bindings, local permissions and runtime
proofs remain contextual checks. The enforcing implementation is
[the pure validator](../crates/locust-core/src/organization.rs), with
[behavioral tests](../crates/locust-core/src/organization/tests.rs) and
[installed CLI tests](../crates/locust/tests/blueprints.rs).

This decision follows the [organization blueprint research](../research/organization-blueprints.md)
and the owner's agreement that agents must easily author blueprints and Polaris
should offer a visual authoring experience. It does not adopt every tentative
protocol mechanism or example field name in that research or the conversation.

The [public documentation plan](public-documentation-plan.md) specifies the full
locust.farm manual required alongside implementation, including versioned human
and agent references, tested tutorials, and publication verification.

## Greenfield implementation constraint

The owner explicitly selected a clean replacement on 2026-10-04: no migrations,
backward compatibility, retained legacy runtime, or dead code. Implement the new
contract directly and remove superseded code, interfaces, flags, dependencies,
fixtures and active documentation in the same completed logical changes.
Existing goals/data do not require conversion or continued runtime support.
Initialize fresh state for the new design; unsupported formats must fail clearly
before mutation rather than being interpreted through compatibility paths.

Historical evidence can remain labeled in research or Git history. Definition
revisions and evidence retention within the supported current model remain product
features; neither requires retaining obsolete schema/protocol implementations.
This is an accepted implementation requirement; code enforcement and removal
checks are specified in the implementation plan and have not run yet.

## Accepted model

An organization blueprint is a reusable, declarative agreement about how a
group works together. A goal selects and pins a version; tasks inherit defaults
or select an allowed task type. The coordinator workflow becomes one arrangement
among others. Open collaboration must permit useful findings and contributions
without mandatory assignments, reviews, or a single accepted result.

The model separates participants/roles, shared context, work/attempts/contributions,
flow, and decisions. Completion follows a task's explicit rule: what evidence is
required and whose judgment counts. Submission, approval, selection, and applying
a change locally remain distinct. Organization rules do not grant local tool,
filesystem, spending, or sharing permissions.

## Accepted governance and scoped decisions

The owner resolved D2 on 2026-10-04: each goal has one explicit administrator
for membership and rules. Administration is separate from work organization,
review, and local execution. A work event authorized by the pinned rules does
not need a fresh administrator signature.

The D4/D5 approach is also accepted: a scope may name an optional authority for
exclusive reservation or selecting one output. These identities need not be the
goal administrator. If an authority is unavailable, only the decision requiring
it waits; authorized independent contributions and non-exclusive evidence can
continue. No reservation or selection authority is implicit in open work.

Acceptance fixes the product contract, not a signed-event format or finality
proof. The [semantics and removal inventory](organization-blueprints-semantics.md)
records concrete scenarios and the remaining cutoff, fork, proof-retention and
revision obligations. The administrator must not become a hidden finalizer to
avoid engineering those obligations.

## Daemon-driven transitions and delivery

The accepted D14 correction requires the daemon to advance configured transitions
and durably deliver ready work without waiting for agents to request each step.
Deterministic readiness feeds one authorized signer: the goal's administrator
for configured stages (owner decision, 2026-10-04) and a result's author for
review requests outside a stage. Stable
logical effect IDs and atomic event/outbox/deduplication writes make retry and
restart resume the same work. Delivery, acknowledgment and execution start are
separate facts. Local execution requires the existing local permission grants;
flow rules do not grant consent or promise to wake a closed client.

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
5. Task types preserve parent scope and local permissions, and
   the agent can explain exactly what remains before task completion.

The remaining engineering questions include exact reservation and decision
finality, membership/rule cutoffs, proof retention, current-model revision
transitions, and the signed binding of the declarative schema. D2 and the
D4/D5 authority approach are accepted; their detailed protocol proofs remain
open. Earlier compatibility/migration proposals are
superseded by the greenfield constraint above. Acceptance of the
product direction does not establish protocol correctness or runtime readiness.
