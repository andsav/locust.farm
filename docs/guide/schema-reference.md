# Authoring schema and command reference

**Status: implemented offline authoring contract, formation schema version 1.**
The tables on this page are rendered from committed exports of the Rust model and
CLI catalog. They are not separately maintained signatures. Download the
[exact schema](../reference/generated/organization.schema.json),
[contract](../reference/generated/organization.contract.json) and examples below.

## Scope and versions

This reference covers offline JSON discovery and inspection only. It does not
provide runtime publication, binding, goal mutation, execution or flow delivery.
API/protocol identifiers shown in the page context identify the source contract;
they do not claim that offline validation exercises the daemon or wire protocol.
Unsupported formation schema versions are rejected; no conversion is offered.

## Inspect the current definition

Use the generated schema for field/type shape and the core validator for supported
semantics. The report provides structured diagnostic codes, phases, paths, messages
and corrections. Explanation resolves declared rules and required bindings, but
cannot observe actual membership, authority availability, input values or local
permissions. Presentation is not executable identity.

## Machine discovery and examples

The inventory lists exact artifact URLs, hashes and source identity. Download an
example matching this schema, validate it using the locally built CLI, and inspect
its effective-rule explanation before changing it. Keep the exact checked file
and its source/schema identity together. These artifacts contain public test data,
not credentials or invitations.

The generated sections below expose every offline operation, root schema field
and shipped example from the same current export. The [runtime reference](runtime-reference.md) separately generates the current
CLI/API/MCP and signed-event surfaces. Use [authoring](formation-authoring.md)
for private publication and binding procedures.
