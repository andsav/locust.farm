# Local API, MCP and event reference

**Status: implemented development API 5 / protocol 5.** The published terminal preview is a separate API-4 artifact; see [availability](status.md).
The tables below are generated from the Rust request, response and event types,
operation registry and actual CLI command builder. Download the
[full runtime contract](../reference/generated/runtime.contract.json), or run
`locust --json contract` without a daemon or credentials. Offline formation
inspection has a separate [schema reference](schema-reference.md).

## Local API transport and effects

The local API uses authenticated local transport and structured envelopes.
It is not a public HTTP API; no OpenAPI facade is implied. The current operation
registry is the authority for names, audience and effects. Owner administration,
agent work and read-only viewer calls have different audiences. A viewer read
cannot acquire a claim or acknowledge shared context. All event and context reads
are observational; acknowledgment is an explicit session-bound write.

Requests and replies use the current canonical local framing and typed response
variants. CLI JSON output wraps results as `{ "ok": true, "result": ... }` and
errors as `{ "ok": false, "error": ... }`. Error details retain current draft
revisions and structured diagnostics where applicable. `events` takes a feed
position and an explicit page limit; `wait` takes a known revision and explicit
timeout. These are observation controls, not work execution budgets.

## MCP and native tools

The stdio bridge exposes agent operations under the session's authenticated
principal. Configured transport, instruction discovery and a model's actual tool
call are separate observations. Native policy can refuse a call; configuration or
an organization rule must not weaken it. An owner-only operation cannot be
advertised as an ordinary agent tool. Read-only pending inspection does not accept
work or acknowledge cancellation.

Invitation issuance, inspection, inventory, revocation and redemption are not
model tools. The person-facing `invitation inspect` command verifies a signed
preview offline; `invitation join` requires direct owner authority, an existing
local principal and the exact review identifier. It grants membership without
changing local execution permissions or workspace bindings. `invitation list`
exposes capability-free issuer inventory; `invitation revoke` durably refuses
unused tickets and cannot undo a redeemed membership. Existing authorized
CLI/API callers retain their explicit authority; hiding these operations from
MCP does not revoke a principal's grants. See the [invitation journey](collaboration.md#invite-a-person).

## Signed event and proof context

Events bind goal, signer, author sequence/predecessor, causal parents, governance
anchor, diagnostic time and sealed payload. Clock time grants no rights. Genesis
pins the administrator and initial definition identity; admission and rule binding
are explicit governance. Work binds exact subjects, rules/rounds and eligible
identities. Content epoch differs from authority and rule revisions.

Approval attaches to an exact contribution. Scoped selection requires its named
authority and predecessor context. A conflicting successor halts that scope;
missing ancestry is pending verification. Retained proof closure cannot bypass
membership removal, wrong epoch, invalid subject or the authority stream's fork.
See the [signed semantics](../formations-semantics.md) for the exact
implementation contract and [recovery](recovery.md) for observable states.

## Errors, versions and reference coverage

Distinguish invalid evidence, missing proof, unsatisfied criteria, satisfied
criteria and disputed authority. Readiness diagnostics also separate unavailable
actors and missing local grants. Preserve structured code, phase/path where
applicable, scope and subject identity. Do not treat missing proof as rejection.

The displayed development API/protocol markers identify source, not a qualified
running release. Schema versions are independent. Unsupported protocol markers
and persistent formats must be rejected before using an old decoder or mutating
state. Definition normalization and semantic hash use the current core model;
layout metadata does not define executable identity.

The generated tables document executable source. They do not qualify a client,
platform, public download or remote transport path; see [availability](status.md).

## Status, local permissions and shared context

`locust status`, `goal status`, `board`, `task show`, `pending`, `sessions` and
`session show` render names, states, reasons and suggested actions for people.
`--json` retains structured machine responses; context receipt presentation is
described below. Session reports include the last
reported state and timestamp; connection attachment and a held claim do not prove
that an external client is running. `locust --owner inbox` collects local
participants needing attention without accepting work or acknowledging content.
`locust watch --goal GOAL` prints the current pending view, waits for a changed
revision, then prints the new view; `--timeout-ms` is an explicit observation wait.

Use `locust --owner permission inspect --goal GOAL --agent NAME` to inspect
membership, all seven local permission categories and task-specific authorizations.
An agent or its read-only viewer can inspect its own key with the same command
or `locust_permission_inspect`; inspecting another principal and changing any
permission require the owner.
`permission allow` and `permission revoke` take named categories and change only
those categories. For example, `permission allow --goal GOAL --agent NAME review`
permits eligible reviews without enabling execution. Add `--task TASK execute`
to `permission allow` for a task-specific execution authorization. Use
`permission revoke --goal GOAL --agent NAME --task TASK` to remove that task's
local authorizations. Revoking a standing category leaves task exceptions visible;
neither permission edit terminates an external process or cancels an attempt.
Membership and organization eligibility remain separate requirements.

`context.read` takes a goal, optional task, explicit `view` (`full` or `compact`),
positive page `limit`, optional `preview_chars`, `unread_only`, and the previous
`next` as `after`. Full context includes pinned rules, inputs, task state, shared
document references and complete pending work. Compact context keeps current
scope/rule/document identities, all pending category counts and session news.
The first page carries `summary.full` or `summary.compact`; continuations omit
that summary. Both views return the same attributed event text. Full available
text is the default; a compact summary never truncates a finding.

`pending.page` returns detailed obligations with an explicit positive `limit`,
optional category `kind`, and `next`/`after` continuation. Every page includes
counts for all seven categories, including categories outside the filter. The
existing `pending` operation returns the complete view. Follow continuations;
page size is not an execution budget or a completeness claim. Both pagers bind
goal revision, reader, session and query controls. A changed revision or query
requires restarting the read. Acknowledgments do not shift event page offsets.

Named CLI commands and MCP return a short `ctx:` receipt reference. Pass that
exact string to `context acknowledge --receipt` or `locust_context_acknowledge`.
The client saves the daemon-signed receipt under the private Locust home, scoped
to its credential and execution session. References work across CLI/MCP process
restarts in that same scope; they are not transferable to another credential or
session. Removing a receipt file requires reading the page again. There is no
automatic expiration or acknowledgment on read. The native typed API and raw
`locust call` retain the full signed receipt object, as declared in the runtime
contract; the named CLI and MCP schemas describe their reference argument.

Only complete content in the receipt becomes read for its principal and session.
Previews, missing text, lost read responses and another session remain unread.
Later text availability or changed event standing becomes new context again.
Acknowledgments survive daemon restart, do not change the goal revision, and do
not wake goal waiters. Pending reads reflect session `context_news`. Reads without
a session and viewer reads have no receipt.

`contribution.publish` accepts `sources`, an explicit array of source event IDs.
`patch submit --source EVENT` is repeatable. These references are signed author
declarations. Local publishing requires each referenced event to be held in the
same goal; historical and excluded sources can still be cited. Declarations do
not grant authority, change approval criteria, or introduce causal dependencies.

`contribution inspect --goal GOAL --contribution EVENT` (MCP:
`locust_contribution_inspect`) shows the contribution, its declared source events,
and exact attempt/task-round chain. It retains author, current standing, text
availability and content status; unavailable event headers remain explicit empty
details. A signed citation or acknowledgment does not prove comprehension, actual
use or approval. Agents read useful findings, declare sources when publishing,
and let the resulting artifact and independent review provide that evidence.
