# Crates and workstreams

Date: 2026-10-03. **Status: accepted layout and working rules for the October 4 push. `locust-proto` holds the contract; `locust-net`, `locust-adapter` and `locust-workspace` have behavior and tests; `locust-core` and `locust-store` are being built; the `locust` binary still only prints its name.** The repository owner approved the crate split on 2026-10-03, chose to keep all streams in one checkout without worktrees, and divided the work between two orchestrating sessions that review each other. This supersedes the earlier guidance in the [implementation plan](implementation-plan.md) to keep every module inside one crate.

## Why the workspace is split

Several agent sessions edit this checkout at once. In a single crate, one stream's half-written file breaks every other stream's build and tests. With one crate per stream, unfinished code breaks only its own crate and the binary that links it. The split also fixes the dependency direction, which the single crate could not enforce: every library crate depends on the contract crate and on no other workspace crate.

```mermaid
flowchart BT
    P[locust-proto: contract]
    C[locust-core] --> P
    S[locust-store] --> P
    N[locust-net] --> P
    W[locust-workspace] --> P
    A[locust-adapter] --> P
    B[locust: daemon, CLI, MCP bridge] --> C & S & N & W & A & P
```

## Ownership

Two orchestrating sessions run the streams. **Lane A** owns the contract, the daemon's core path and every integration commit. **Lane B** owns everything that faces the coding clients, the network and the release. Each lane may run its streams as sub-agents; the lane's orchestrator is answerable for them.

| Crate or path | Lane | Responsibility | Tests alone against |
|---|---|---|---|
| [`crates/locust-proto`](../crates/locust-proto/src/lib.rs), root [`Cargo.toml`](../Cargo.toml), `Cargo.lock` | A | The contract: identifiers, signed events, invitations, sync frames, local API types, storage seam, limits, golden vectors; all dependency pins | Its own unit tests |
| [`crates/locust-core`](../crates/locust-core/src/lib.rs) | A | State machine with no I/O: validation, authority, tasks, claims, projections, what to send a peer | `MemStore` and the test kit |
| [`crates/locust-store`](../crates/locust-store/src/lib.rs) | A | SQLite implementation of the storage seam | The store conformance suite plus crash tests |
| [`crates/locust`](../crates/locust/src/main.rs) | A | Daemon process, socket server, CLI, `locust mcp`; wires the other crates | `MemStore`, then the real store |
| [`crates/locust-workspace`](../crates/locust-workspace/src/lib.rs) | A | Manifest export, materialization, patches; runs in the CLI, never in the daemon | Plain directories and Git object reads |
| [`crates/locust-net`](../crates/locust-net/src/lib.rs) | B | Endpoint, relay and address configuration, framed links, in-memory link; the two-machine transport probe | An in-memory link; then two machines |
| [`crates/locust-adapter`](../crates/locust-adapter/src/lib.rs) | B | Per-client configuration, launch, session binding, hooks, diagnostics | Scripted provider fixtures |
| Real-client qualification harness | B | Default-profile Codex, Claude Code, Factory Droid and Pi runs: tool reach, approvals, wait, interruption, resume, hook delivery | A stand-in stdio MCP server until `locust mcp` exists |
| `scripts/`, skill and installer files | B | Installer, operating skill, client configuration, user-service units, install prompt | The development binary |
| `.github/`, release manifest and signing | B | macOS and Linux CI, release-build smoke test on a fresh database, artifacts | — |
| [`release-evidence.md`](release-evidence.md) | B | Keeps the gate ledger; lane A supplies records for its own gates | — |
| [`sites/locust.farm/`](../sites/locust.farm/README.md) | C | The website and its short `/start` guide with the entry prompt | The site's `npm` lint, check, test and build |
| [`first-contact.md`](first-contact.md), [`lane-c-log.md`](lane-c-log.md), [first-contact research](../research/first-contact-integrations.md) | C | First-contact experience contract, harness routing, Polaris handoff and requests to other owners | `python3 scripts/check_docs.py` |

Lane A makes each integration commit that wires crates together, including the one that connects `locust-net` to the daemon.

**Lane C** owns the first-contact experience: the website and the documents above. It implements no runtime, setup or client integration. It asks lanes A and B, and the separate Polaris work, for what it needs in its own [log](lane-c-log.md), and updates its copy when their records show new behavior.

### Starting lane B

Read, in order: this document, the [version 0 contract](protocol-v0.md), plan sections 6 to 8 of the [implementation plan](implementation-plan.md), the [hcom assessment](../research/hcom-dissection.md) and the client findings in the [independent review](../research/implementation-plan-independent-review.md). Then:

1. **Transport probe.** Two machines on separate networks connect with Iroh 1.3 at the pinned version; the direct and the forced-relay path are both observed; an alternate relay works; the default relay and address-lookup operator and what each observes are written down. Deliver in `locust-net` a framed link that carries `locust_proto::sync::SyncMessage` frames using `locust_proto::codec`, exposes the authenticated remote `EndpointId` of each link, and has an in-memory twin for tests. Evaluate the blob layer separately at pinned versions, including unauthorized fetch and push and crash durability. The transport decides nothing about membership: the daemon feeds received frames to the state machine and writes back what it returns.
2. **Default-profile client qualification.** In isolated, default-configured profiles (never the owner's own, which are permissive), establish for Codex, Claude Code, Factory Droid and Pi: whether a registered stdio MCP server reaches a Unix socket under `$LOCUST_HOME`, which tool calls prompt, how a blocking wait behaves at each client's limits, what interruption and explicit resume look like, and which hooks deliver between tool calls. Run scripted-provider checks without real credentials where the selected client supports that path; real-account checks need the owner to sign in to those isolated profiles. Record client/version, effective permissions, provider/authentication mode and any missing capability separately in the [client matrix](release-evidence.md).
3. **Client lifecycle adapters**, following the adapter table in plan section 6. Codex, Claude Code, Factory Droid and Pi use active sessions and explicit resume. Automatic wake is scoped only to Merak for now, with its own qualification; unattended closed-session startup remains deferred.
4. **Installer, skill and client configuration**, against the operation names in the contract.
5. **CI and release packaging.**

Lane B never edits lane A's paths. A needed contract change or new dependency is written as a request in the [lane B log](lane-b-log.md); lane A answers in the [lane A log](lane-a-log.md) and makes the change.

## Cross-review

Each orchestrator reviews the other's work. This is the scoped review the plan requires before a change counts as reviewed, and the release ledger's review reference points at these entries.

- **When.** Before an integration checkpoint that depends on the commits, and without waiting for one when a commit touches authorization, signing or verification, the wire format, export or materialization, the installer, or the transport's admission of peers.
- **How.** Read the diff, run that crate's checks, and try to break it. A reviewer does not edit the other lane's files.
- **Record.** The reviewer writes the commit range and numbered findings in its own log. The owning lane fixes each finding or declines it with a reason, and replies in its own log by finding number. Each log has exactly one writer, so the two sessions never edit the same file.

## Rules for one shared checkout

These are conventions; nothing enforces them except the checks named below.

1. **Stay inside your paths.** A stream edits only its crate or paths from the table. A change to `locust-proto`, the root `Cargo.toml` or `Cargo.lock`, including any new dependency, is requested from lane A.
2. **Commit by path.** Use `git commit -- <your paths>` so another session's staged files are never swept in. Do not amend, rebase or otherwise rewrite history.
3. **Build in your own directory.** Set `CARGO_TARGET_DIR` to a per-stream directory under `target/` so concurrent builds do not queue on Cargo's lock.
4. **Contract changes are versioned.** A change that alters a golden vector in [`vectors.rs`](../crates/locust-proto/src/vectors.rs) changes the wire format and needs a new protocol version once a release exists.
5. **Checks.** Approved by the owner on 2026-10-03: a stream commit runs `cargo fmt`, `cargo clippy` and `cargo test` for its own package (`-p <crate>`), so a neighbour's unfinished crate does not block it. Lane A runs the same three checks across the whole workspace at each integration commit, as does any change to `locust-proto`, the root `Cargo.toml` or `Cargo.lock`. [AGENTS.md](../AGENTS.md) carries the commands.

## Integration order

The owner's priority, set on 2026-10-03: as soon as the scaffolding links into a daemon that can found, join and synchronize a goal, cut a binary and test it on the owner's three machines. Work that the first three-machine test does not need waits until that test has run.

**Scaffolding** is done when these exist together: contract revision 2 complete (lane A); the core state machine and the SQLite store (lane A); the daemon shell with its socket and a CLI for the operations below (lane A); a transport that the daemon can bind and dial with contract types, with the hello-then-peer frame limit and delivery of a final frame (lane B; findings A-R1, A-R2 and A-R4 in the [lane A log](lane-a-log.md)).

### T1: the first binary on three machines

- **Binary.** One commit built for each machine in release mode. `locust --version` prints the version and the commit, so every record names exactly what ran. Lane B provides the build step for the owner's machines; lane A provides the daemon and CLI.
- **Operations in the binary.** `daemon run`, `status`, `agent enroll`, `goal create`, `goal invite`, `goal join`, `goal status`, `note add`, `notes`, `task propose`, `task assign`, `task claim`, `task submit`, `event show`, `result accept`, `board`, `pending`. Content is sealed, so key exchange and single-chunk content transfer between daemons are included.
- **Run, driven from the CLI on each machine.**
  1. Machine 1 founds a goal and issues two invitations; machines 2 and 3 join. All three list three members and show the route to each peer.
  2. Machine 1 proposes and assigns a task to machine 2, which claims and submits; machine 1 reads the result and accepts it; machine 3 holds the whole history without having taken part.
  3. Machine 1's daemon is stopped. Machines 2 and 3 exchange notes with each other. Machine 1 starts again and catches up.
  4. Each daemon is restarted in turn and keeps its state; a sleeping laptop wakes and reconnects.
- **Evidence.** Commit and binary hash per machine, operating system and architecture, the commands, event identifiers, routes and every failure, recorded in the [release evidence ledger](release-evidence.md). A failure is recorded and fixed before anything else is added.
- **Not in T1.** The MCP bridge with real coding clients, workspace snapshots in the task flow, member removal and key rotation, document revisions, managed client sessions, content larger than one chunk, the installer and signed artifacts.

### T2: coding agents on the same machines

`locust mcp` and the operating skill in default-profile client sessions on the three machines, a real task with a workspace snapshot and a patch, and wait, interruption and explicit resume. Baseline clients are Codex, Claude Code, Factory Droid and Pi; start with a runnable pair.

### After T2

Member removal and key rotation, cancellation delivery, document revisions, content larger than one chunk, managed client sessions, then packaging and the one-prompt install against the release candidate.

Deterministic tests do not wait for the machines: the core's shuffled-delivery and multi-node tests and the store's conformance and crash tests run as their crates land.
