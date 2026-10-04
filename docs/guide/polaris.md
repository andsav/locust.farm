# Polaris authoring and observation

**Status: implemented development slice; native package and release qualification
remain open.** Polaris has a forms-first organization library/editor at
`/organizations` and a typed Rust adapter to the local Locust daemon. The verified
local source commit is
[`aaaf62ae3bbdd3f280eb7fd1db343b690ebb330e`](https://github.com/33CCFF/dreamcolor10/commit/aaaf62ae3bbdd3f280eb7fd1db343b690ebb330e).
It has not been pushed or published; the link identifies the configured repository
and locally verified commit, not a publicly available build. Locust remains
independently usable. This page does not establish a packaged Polaris visual
editor, download, installed native UI, or Merak execution worker.

## Connect with separate credentials

The Organizations page takes an absolute Locust home and private credential-file
path. Native Rust opens and checks the credential file, authenticates a fresh
local typed client, and keeps credential bytes outside the renderer. The file
must be owned by the native process's user, regular, and mode `0600`; symlink
files are refused. The daemon's state directory remains private (`0700`).

Use distinct connections for private authoring, read-only goal viewing and goal
creation by an enrolled agent. Owner credentials are refused. The author
connection accesses its own drafts and publications; it does not grant goal
access or execution authority. Native request-channel checks reject external,
voice and devserver requests, and the shell restricts this adapter to the main
native window/webview. A browser without a desktop backend shows the
desktop-required interstitial.

The adapter pins immutable remote `locust-proto` Git revision
`1b81bef7a3caafa219f5a4096a01b3a49d505c56`, including causal closure and durable
delivery receipts. Generated request, response and Blueprint schemas match that
revision exactly. It was tested from Cargo's locally seeded Git cache; remote
fetchability has not been established. No sibling path dependency or copied Rust
protocol is used.

## Use one shared executable definition

Create a draft from a bundled arrangement preset or open an existing private
draft. Forms cover participants, context, work, completion and decisions, task
variations, and optional flow. They use Locust's generated JSON Schema; the
source tab shows the same JSON. The source inventory detects unsupported fields
and newer schema versions, preserves the complete source, and refuses visual
editing rather than dropping rules. This is a forms-first editor, not a canvas.

Validate and explain the exact source through Locust. Diagnostics retain code,
phase, path, message and correction; matching form fields show their diagnostics.
The effective-rules view uses Locust's explanation. Publication review shows
Locust-normalized definitions, semantic hashes and a structural comparison with
the original draft. A stale explanation is identified and cannot authorize
publication. Polaris does not implement a second policy evaluator.

## Round-trip and unsupported input

Source edits update draft revisions. Saving editor presentation uses its own
revision and does not change semantic identity. Saving a draft, saving a view,
publishing an immutable definition and creating a goal are distinct actions.
Goal creation takes the published normalized definition, explicit role member
bindings and contextual input hashes through the separate agent connection. It
does not start a Merak run or grant local execution permission.

## Refresh and reconcile the private catalog

There is no global catalog revision or catalog notification stream. Explicit
refresh, focus and reconnect read the private draft/publication catalogs. Each
draft supplies its own revision and source hash; presentation has an independent
revision. Goal `wait` is not used as a catalog watch.

A draft update supplies its expected revision. Publishing supplies the expected
revision and exact saved source hash and requires a current valid inspection.
Dirty source, stale replies and external revisions cannot silently publish.
External source appears for reconciliation while local edits remain intact.
Changing author identity/home or disconnecting invalidates the editing scope and
preserves local source; the person must explicitly open a current catalog draft
before saving in the new scope.

Every operation opens a fresh authenticated connection, so daemon restart,
revocation and candidate mismatch are observed by the authoritative API. An
uncertain mutation is not automatically replayed. Refresh and reconcile before
retrying. The isolated native adapter test verified exact draft readback after a
daemon restart and refusal after credential revocation.

## Observe authoritative work

Connect a viewer or agent to refresh goal status, board, contributions and pending
obligations. These are currently expandable evidence panels, not the proposed
swarm map. Contributions, completion, selection, delivery and actual local
application remain distinct facts; an empty obligation list does not mean the
goal is complete. The observer does not start work or move a feed cursor.

## Verification boundaries

Three focused native adapter/channel tests passed. A separate isolated test used
a real local Locust daemon and typed native clients to exercise agent-authored
source, person edits, stale-revision refusal, explanation, exact-source
publication, separate agent goal creation, author goal-creation refusal,
restart/readback and revocation. Its daemon identified itself as
`0.1.0 (9d0487fae38b-dirty)`: this is local component evidence, not an installed
immutable candidate. The agent in this fixture is an API caller, not an AI
provider acceptance run.

Frontend type checking passed with zero errors/warnings, the production build
passed, and eight focused organization tests passed. Browser checks of the real
route used synthetic IPC and exercised library opening, form-to-source edits,
dirty publication gating and explanation; they prove browser UI behavior only.
The broader frontend suite reported 132 failures outside the organization tests
and has not been qualified against a clean baseline. Workspace-wide Rust gates
also retain failures in unchanged files. See the Merak commit's
[implementation and evidence record](https://github.com/33CCFF/dreamcolor10/blob/aaaf62ae3bbdd3f280eb7fd1db343b690ebb330e/docs/LOCUST_ORGANIZATION_AUTHORING_2026-10-04.md).

Installed/package native visual acceptance, public artifact fetchability and
release qualification remain required. No public deployment or release occurred.
