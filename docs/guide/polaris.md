# Polaris authoring and observation

**Status: API-4 source integration with verified native adapter components;
the earlier isolated native package remains API 2.** Polaris has a forms-first
organization library/editor at
`/organizations` and a typed Rust adapter to the local Locust daemon. The verified
local source commit is
[`4917caf3963900c5ebaa6819eaa034663e589d64`](https://github.com/33CCFF/dreamcolor10/commit/4917caf3963900c5ebaa6819eaa034663e589d64).
It has not been pushed or published; the link identifies the configured repository
and locally verified commit, not a publicly available build. Locust remains
independently usable. The native qualification uses a separately identified QA
package and private state; it does not establish a public download, an installed
canonical release or a Merak execution worker.

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
`c50a43f54a6168a631351c6500b44ccd1167d27c`, including API-4 context views,
acknowledgments, signed contribution sources and own-permission inspection.
Generated request, response and Blueprint schemas match that
revision exactly. It was tested from Cargo's locally seeded Git cache; remote
fetchability has not been established. No sibling path dependency or copied Rust
protocol is used.

## Use one shared executable definition

Create a draft from a bundled arrangement preset or open an existing private
draft. Forms cover participants, context, work, completion and decisions, task
task types, and optional flow. They use Locust's generated JSON Schema; the
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

## Current source verification

The API-4 refresh passed three focused native adapter/channel tests and an
isolated real-daemon author/edit/publish/goal/restart/revocation test. A standalone
Cargo harness compiled the actual native adapter module against the exact Git
SDK pin, with no path override; the complete Tauri host was not rebuilt.
Workspace dependency resolution passed with `cargo metadata --locked --offline`.
Frontend type checking reported zero errors/warnings, facade boundary lint and
eight organization tests passed, and generated contract drift checks passed.
The diagnostic observer passed fifteen tests for current wire shapes. Agent
connections now explicitly reject owner-only reads such as the inbox.

These checks used the API-4 development daemon, whose runtime source is committed
as `c50a43f54a6168a631351c6500b44ccd1167d27c`. They do not update the package
described below. The [current source evidence record](https://github.com/33CCFF/dreamcolor10/blob/eb3d28e7f6a9d7e4c4288b839ac62cc91d0346ae/docs/LOCUST_ORGANIZATION_AUTHORING_2026-10-04.md)
retains exact schema hashes and the historical results. Both source commits are
local; no push, public release or packaged-app rebuild occurred in this refresh.

## Historical API-2 package verification

Three focused native adapter/channel tests passed. A separate isolated test used
a real local Locust daemon and typed native clients to exercise agent-authored
source, person edits, stale-revision refusal, explanation, exact-source
publication, separate agent goal creation, author goal-creation refusal,
restart/readback and revocation. It passed against immutable Locust candidate
`0a295cdabe6a878cc733c791ca73863933cfa45a`, binary SHA-256
`b577709eb981544b4bcf5bdcc5efd677a6133d24e474ab274a1fe500215450b3`.
The agent in this fixture is an API caller, not an AI provider acceptance run.

The signed local Polaris package from Merak commit
`aaaf62ae3bbdd3f280eb7fd1db343b690ebb330e` also passed an automated native UI journey
against those exact daemon bytes: create a fresh draft, edit context through
forms, save revision 2, explain, review and publish, create a goal through the
separate agent, and read back its state. After daemon restart, the draft,
publication and goal remained readable. Revoking the author caused the next
library refresh to show the authoritative refusal. The actual packaged
`tauri://localhost` window used the native adapter with private credentials;
no synthetic IPC participated in that journey.

The package used identifier `com.merak10.polaris.locust-o10-qa`, product name
`PolarisLocustQA` and title `Polaris Locust QA` to avoid the existing app's
single-instance lock. Production native channel guards remained enabled. The
QA archive SHA-256 is
`5341acf14bedf545dbe75f5e7bb007aa02eef8797d94657976bb34000b801dc7`.
This verifies the isolated Developer ID signed local package. Canonical-identity
installed release, notarization and public fetchability remain separate.

At that earlier checkpoint, frontend type checking passed with zero errors/warnings, the production build
passed, and eight focused organization tests passed. Additional browser checks
used synthetic IPC for dirty edits, external revisions and scope invalidation.
The broader frontend suite reported 132 failures outside the organization tests
and has not been qualified against a clean baseline. Workspace-wide Rust gates
also retain failures in unchanged files; full lint ran out of heap. The current
[implementation and native evidence record](https://github.com/33CCFF/dreamcolor10/blob/b0fb5d9d5/docs/LOCUST_ORGANIZATION_AUTHORING_2026-10-04.md)
retains exact package identities and these unresolved broader gates. The commit
is local and has not been pushed. No public deployment or release occurred.
