# Lane C log

**Status: working log, written only by lane C (first-contact experience and website).** It carries lane C's requests to the other owners and the source reviews behind its public copy. Lane A and B answer in their own logs; the roles are in [workstreams](workstreams.md).

Lane C owns the [first-contact contract](first-contact.md), the [locust.farm site](../sites/locust.farm/README.md) and its research notes. It does not implement runtime, setup or Polaris behavior. Each request below names the input lane C needs and what lane C will change once it lands.

## Requests

### To lane A (runtime, local API, permissions)

- **C-A1: a read-only viewer.** *Need:* a way for a native connector, such as Polaris's, to read goal status, board, task detail, pending work and events without being able to write. *Gap:* `Caller` is the owner or an enrolled agent with grants; no read-only principal exists, and an operation being read-only is not a viewer permission. *Then C:* names the connector's credential and scope in the Polaris handoff.
- **C-A2: an "applied" record.** *Need:* a read that tells accepted from applied locally, from the CLI's integration step. The plan already reports submitted, accepted and integrated separately. *Gap:* the API has `TaskState::Accepted` only. *Then C:* shows the third outcome in the guide and the Polaris inputs.
- **C-A3: readiness answers.** *Need:* `status` or `doctor` output that separates daemon not answering, version mismatch and a working harmless call, with stable error codes. *Gap:* no daemon or CLI yet. *Then C:* links each readiness state to the actual check.
- **C-A4: stale status line.** The header of [workstreams](workstreams.md) says every crate except `locust-proto` is a stub. At `9fbbf74` the net, adapter and workspace crates have behavior and tests. Please update it when convenient; lane C did not edit it.

### To lane B (setup, harness routes, qualification)

- **C-B1: one canonical setup artifact.** *Need:* a stable location for the install prompt or payload that the guide can link to, and for each qualified harness: the files and configuration scopes it changes, approvals it needs, the refresh step, and the readiness check. *Gap:* no installer, prompt or skill exists. *Then C:* replaces the guide's "no qualified route" stop with a link to that artifact. The guide's entry prompt only points to the guide; it will not become a second installer.
- **C-B2: qualification records per harness.** *Need:* a ledger record per harness and version for tool reach, approvals, skill refresh, interruption and explicit resume. *Then C:* moves that row of the routing table from "None" to the recorded result, and nothing more.
- **C-B3: the generic route.** *Need:* the minimum capabilities for a harness outside the four-client baseline (stdio MCP, or a command line that can reach the socket under its policy; skill loading; asking before changes), and what such a harness is told when it lacks one. *Then C:* turns the guide's three routing questions into that exact check.
- **C-B4: delivery and wake.** *Need:* per baseline client and version, whether active-session delivery passes, and the Merak wake record if one is made. *Then C:* names them. Until then the guide promises active sessions and explicit resume only.

### To the Polaris work (separate implementation)

- **C-P1: native connector.** *Need:* a Rust-side connector that holds the daemon credential, makes the typed reads in the [Polaris handoff](first-contact.md#polaris), checks the API version and reconnects from the last feed position. *Depends on:* C-A1 and the running daemon.
- **C-P2: packaging and lifecycle.** *Need:* a decision on whether Polaris attaches to a user's daemon or manages its own, with only one daemon per state directory. *Then C:* describes how Polaris and the own-harness path coexist.
- **C-P3: download and navigation.** *Need:* a real download location and link format. *Then C:* adds them to the site. Until then the site names Polaris as the complete offering and links nowhere.

## Source reviews

### 2026-10-03 at `9fbbf74`

Read: the [`locust` binary](../crates/locust/src/main.rs), the [local API](../crates/locust-proto/src/api.rs), the [workspace crate](../crates/locust-workspace/src/lib.rs), the [adapter crate](../crates/locust-adapter/src/lib.rs), both lane logs, the [lane B implementation log](lane-b-implementation-log.md) and the [release ledger](release-evidence.md), including `9fbbf74`'s four-client baseline. Official client documentation for Claude Code, Codex, pi and Droid, and the Polaris source tree, are summarized in [first-contact integrations](../research/first-contact-integrations.md).

Result: no daemon, CLI, MCP bridge, installer or skill; typed contract with revision 2 pending; workspace export and materialization implemented; Codex and Claude configuration and peer links with component and same-host evidence; Droid and pi adapters not implemented; automatic wake scoped to Merak only; no release gate passed. The [first-contact contract](first-contact.md) is written against this state.

### 2026-10-03 at `ce0ece6`

Read lane B's four-client configuration change (`87f8a42`) in the [lane B implementation log](lane-b-implementation-log.md) and [lane B log](lane-b-log.md), the [release ledger](release-evidence.md) and the [`locust` binary](../crates/locust/src/main.rs).

Result: lane B now prepares configuration for all four baseline clients, with component tests and installed-client configuration checks for Codex 0.153.4 and Claude Code 2.1.280 that used no model or account. Lane B's four-client qualification harness (`client-qualification.md`) is in progress and not yet committed; it states that no complete Locust client workflow is qualified. No daemon, `locust mcp`, installer, skill or qualification record exists, and no release gate passed. The routing table in the [first-contact contract](first-contact.md) and on the site's [`/start` guide](../sites/locust.farm/src/lib/onboarding/guide.ts) now say so. The guide and its entry prompt are available locally, not deployed; C-B1 to C-B4 stay open.
