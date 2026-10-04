# Polaris authoring and observation

**Status: proposed integration.** No Locust connector, bundle, download or verified
native visual editor is established by this manual. Locust remains standalone.
The website's documentation build does not establish packaged Polaris behavior.

## Use one shared executable definition

The intended visual editor reads the same current schema and core validator used
by the offline CLI. Nodes and edges explain rules; they do not introduce another
policy language. Presentation coordinates and labels remain distinct from
semantic identity. Unsupported input must be refused visibly without silently
rewriting or dropping rules.

An agent can draft JSON and a person can inspect it visually. Import should show
all supported semantics, diagnostics and required contextual bindings. Export
should preserve normalized semantic identity. A canvas rearrangement alone does
not revise authority or a pinned task round.

## Review the draft lifecycle

Read a specific revision, edit it and compare its effective-rule explanation.
Concurrent edits need the same compare-and-swap contract as other authoring
clients; losing edits are preserved for reconciliation. Validation names exact
revision/bytes. Publish and bind are explicit actions, distinct from saving layout,
and neither implicitly starts local work.

When active rules change, the UI must identify whether the change updates future
defaults, rebinds roles, revises a current round or explicitly reopens it. Existing
signatures retain their old context; no schema migration or compatibility reader
is offered.

## Observe authoritative work

The native connector authenticates through the local daemon. Credentials stay on
the Rust/native side, not in the webview. A read-only viewer can inspect what its
principal can read without claiming work or moving a feed cursor. Revocation and
write refusal belong to the authoritative API, not button hiding.

The display identifies the source/contract version, latest observed revision,
missing content/proofs and stale/offline state. Contributions, approval,
criterion satisfaction, unique selection, delivery and actual execution are
separate visible states. A graph must provide text explaining the same rules.

## Qualification needed

A browser mock proves rendering only. Native qualification must identify app
build, connector authentication, actual edits/import/export, concurrent revision
failure, unsupported-input preservation and fresh/stale daemon observations.
Screenshots must name their verified build. Until those checks exist, this page
is a concrete interaction contract, not an executable visual quickstart.

## Round-trip and unsupported input

A valid round-trip preserves normalized semantic identity through JSON import,
visual edits and export. Layout-only changes should leave that identity unchanged.
Semantic edits require core validation and explanation. Unsupported constructs
remain intact for inspection while editing is refused; the UI must not discard
unknown rules and announce a successful import. Native and browser checks need
separate evidence for these guarantees.
