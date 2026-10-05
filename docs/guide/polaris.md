# Optional visual authoring

Polaris is a separate desktop integration for authoring and observing Locust
organizations. Locust is usable through its CLI and MCP bridge without it.
This guide describes the integration boundary; it does not advertise a public
Polaris download or qualify a packaged desktop application against API 5.

## Use the Locust contract

A visual editor should read Locust's generated schema, validate and explain the
exact source through the typed API, and publish the normalized definition.
See [the authoring reference](schema-reference.md) and
[formation authoring](formation-authoring.md). The renderer must not implement
another collaboration evaluator or silently discard unsupported rules.

Private draft source has a revision and semantic identity. Editor presentation
has a separate revision; moving a visual element does not change work authority.
Saving a draft, saving presentation, publishing a definition and creating a goal
remain separate actions. Goal creation does not start a model or grant execution
permissions.

## Keep credentials and authority separate

Use scoped authoring, viewer and agent credentials for their respective operations.
Private authoring is not goal membership, and viewing a goal does not grant
execution. Native connectors should retain credential bytes outside the renderer
and authenticate each operation through Locust. A browser mock cannot establish
that native credential handling works.

## Handle stale and uncertain state

Draft updates use an expected revision. Publication must refer to the exact saved
source and a current valid inspection. Preserve local edits on an external change,
reconcile before retrying an uncertain mutation, and recheck the connection after
daemon restart or credential revocation.

## Observe authoritative work

Views report authoritative contributions, completion, selection and pending
obligations separately. Empty pending work does not mean a goal is complete, and
receiving a delivery does not prove a process started. See
[recovery](recovery.md) and [runtime reference](runtime-reference.md).

Public support claims need an identified desktop artifact, matching API/schema,
actual native credential and round-trip checks, and a separately verified release.
The [availability record](../reference/availability.json) covers Locust's release.
