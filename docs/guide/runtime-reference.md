# Local API, MCP and event reference boundaries

**Status: replacement runtime contract in development; generated runtime operation
coverage is not yet qualified.** Offline blueprint operations have a separate
[generated reference](schema-reference.md). This page identifies the authoritative
runtime sources and interpretation rules without inventing an HTTP interface or
hand-maintained wire signatures.

## Local API transport and effects

The local API uses authenticated local transport and structured envelopes.
It is not a public HTTP API; no OpenAPI facade is implied. The current operation
registry is the authority for names, audience and effects. Owner administration,
agent work and read-only viewer calls have different audiences. A viewer read
cannot acquire a claim or move an agent's feed cursor.

The replacement exporter must derive commands from actual CLI builders and MCP
input schemas from actual bridge metadata. It must include success/error response
coverage and pagination, not only input field names. A generated reference must
match those executable sources before being labeled verified. Unknown/unsupported
operations must fail explicitly instead of entering a previous coordinator-only
runtime path.

## MCP and native tools

The stdio bridge exposes agent operations under the session's authenticated
principal. Configured transport, instruction discovery and a model's actual tool
call are separate observations. Native policy can refuse a call; configuration or
an organization rule must not weaken it. An owner-only operation cannot be
advertised as an ordinary agent tool. Read-only pending inspection does not accept
work or acknowledge cancellation.

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

Runtime command/API/MCP response and error tables will be rendered from the
regenerated operation exports when they pass source parity. Until then this
substantive transport/effect/proof contract is a reference boundary, not complete
machine signature coverage or a verified runtime quickstart.
