# Local API, MCP and event reference

**Status: implemented development API 3 / protocol 3; no public release.**
The tables below are generated from the Rust request, response and event types,
operation registry and actual CLI command builder. Download the
[full runtime contract](../reference/generated/runtime.contract.json), or run
`locust --json contract` without a daemon or credentials. Offline blueprint
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
See the [signed semantics](../organization-blueprints-semantics.md) for the exact
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
`--json` retains the typed machine response. Session reports include the last
reported state and timestamp; connection attachment and a held claim do not prove
that an external client is running. `locust --owner inbox` collects local
participants needing attention without accepting work or acknowledging content.
`locust watch --goal GOAL` prints the current pending view, waits for a changed
revision, then prints the new view; `--timeout-ms` is an explicit observation wait.

Use `locust --owner permission inspect --goal GOAL --agent NAME` to inspect
membership, all seven local permission categories and task-specific authorizations.
`permission allow` and `permission revoke` take named categories and change only
those categories. For example, `permission allow --goal GOAL --agent NAME review`
permits eligible reviews without enabling execution. Add `--task TASK execute`
to `permission allow` for a task-specific execution authorization. Use
`permission revoke --goal GOAL --agent NAME --task TASK` to remove that task's
local authorizations. Revoking a standing category leaves task exceptions visible;
neither permission edit terminates an external process or cancels an attempt.
Membership and organization eligibility remain separate requirements.

`context.read` takes a goal, optional task, positive page `limit`, optional
`preview_chars`, `unread_only`, and its previous `next` as `after`. It returns a
coherent brief with pinned rules, inputs, task state, shared document references,
pending work and attributed events, including finding text and review reasons.
Full available text is the default. Follow every page; no first-page completeness
is implied. A changed goal revision requires restarting pagination.

`context.acknowledge` accepts the exact signed receipt from a page. Only complete
content in that receipt becomes read for its principal and execution session.
Previews, missing text, lost read responses and another session remain unread.
Later text availability or changed event standing becomes new context again.
Acknowledgments survive restart, do not change the goal revision, and do not wake
goal waiters. Pending reads immediately reflect the session's `context_news`.
Reads without a session and viewer reads have no acknowledgment receipt. Agents
use the installed skill to read, reuse and cite findings, publish new findings,
and acknowledge what they actually read as part of ordinary work.
