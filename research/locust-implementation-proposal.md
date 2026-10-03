# Proposed Locust implementation approach

Research date: 2026-10-03. **Status: proposal for review, not an accepted architecture or implemented feature.** This synthesizes the [ecosystem survey](landscape.md), [MoltMesh architecture review](moltmesh-architecture-and-consensus.md), and its [security](moltmesh-security.md), [network/storage](moltmesh-networking-and-storage.md), and [task](moltmesh-tasks-and-sdk.md) lessons.

## Product contract

Locust should let independently operated agents collaborate on a common goal through small local Rust daemons, without requiring a vendor-operated coordinator, subscription message board, or cloud account. Participants retain control of execution, sandboxing, keys, storage, and which data they export.

The daemon should own identity, workspace admission, durable exchange, artifact transfer, and validation of collaboration state. An agent adapter should own model invocation and execution. A remote request offers work; it does not directly authorize a command on somebody else's machine.

Bootstrap contacts, relays and optional storage peers should be replaceable and independently operable. “No mandatory central authority” is achievable. “No infrastructure and guaranteed asynchronous availability while every participant is offline” is not. The UI should make the current connection route, available holders, and replication status inspectable.

## Start with one narrow vertical slice

Two people invite their agents into a goal. Each daemon has its own keys, persistent database, content store, and local working directory. One participant offers a task against an immutable input snapshot. The other accepts, works in its own sandbox, and returns a signed result plus an artifact/patch. The goal owner reviews and accepts that contribution. Either participant can disconnect and later reconcile missing state without losing authorship or silently overwriting files.

This proves the collaboration contract with very little scheduling machinery. Add unattended delegation only once the acceptance and permission policies are clear. Start with invitations rather than an open global marketplace; open discovery adds admission, spam, Sybil and trust-ranking problems that are not prerequisites for useful collaboration.

## Three data planes

| Plane | Proposed representation | Required semantics |
|---|---|---|
| Collaboration events | Signed immutable events with workspace ID, author sequence, causal parents and payload references | Durable reconciliation; deduplication; authority checks; explicit conflicts |
| Artifacts | Encrypted immutable blobs and manifests addressed by content hash | Verified transfer, retained local storage, resumability, explicit retention |
| Local execution | Agent adapters and isolated local worktrees | Host-approved capabilities; bounded authority; explicit external side-effect handling |

Gossip should announce that new durable state exists. Peers then reconcile what they are missing. DHT records and routing hints help find peers or objects; they must not be the only durable index linking historical data. A manifest should reference immutable parent objects directly.

An event might include `workspace_id`, protocol version, `author_key`, per-author sequence, causal parent hashes, membership epoch, event type, task/input/artifact references, and a signature over a canonical encoding with domain separation. Specify encoding and signature test vectors before supporting multiple implementations. A deterministic serialization convention is not automatically canonical across languages.

## Ordering and authority

Avoid making every message depend on a quorum. Per-author logs and causal reconciliation are sufficient for messages, observations, proposals, and immutable result contributions. A mergeable task board is useful as a view of events, but a CRDT cannot by itself promise exclusive execution of a scarce task or external side effect.

For an initial workspace, use an explicitly designated owner/coordinator key for decisions such as accepting a result, changing membership, or advancing the accepted workspace head. That key belongs to a participant's daemon; it is not a mandatory service. Disconnected participants may continue drafting contributions, while authoritative acceptance waits for its authority to become available. Display that condition instead of pretending finality.

If automatic failover becomes necessary, define an authority epoch and a handoff/quorum mechanism before claiming safe reassignment. Do not elect a replacement merely because another peer's local clock says the owner timed out: the old owner may still be active across a partition. Under partitions, choose which operations can remain available and which must pause. If Byzantine agreement is a real requirement, adopt a suitable reviewed implementation and establish membership/Sybil assumptions; do not handwrite consensus as a side project.

## Task semantics

Use a protocol lifecycle such as `offered → accepted → running → result_submitted → accepted_result`, with explicit rejection, failure, cancellation request, and cancellation acknowledgment events. Distinguish “worker finished” from “the goal accepted its contribution.” Store deadlines/retry policy in the signed task request so both sides receive the same contract.

Every attempt needs its own ID, input snapshot hash and authority epoch. Separate durable agent identity from worker-instance identity and lease ownership. A reconnecting worker should prove possession of its existing attempt, rather than allowing every process sharing one DID to obtain the same active lease. A persisted subscription cursor must not move past work before its claim/acknowledgment is durably represented.

Use at-least-once delivery with idempotent state transitions. Bind an idempotency key to the canonical request digest and scope; reject reuse with different contents. Write state changes and pending outbound notifications atomically in one local transaction, then deliver from that outbox. Preserve enough state to resume notifications after a crash.

A lease alone cannot guarantee exactly-once external effects. For an API or resource that supports fencing, pass an attempt/authority token that the resource validates; otherwise use the resource's idempotency API or require explicit acceptance before the irreversible action. A stale worker must not gain finalization rights merely because it continues executing after its lease expires. Receiving cancellation is also distinct from successfully stopping a running process or undoing effects.

## Workspace and artifact semantics

Begin with snapshots or Git commits plus patches, rather than synchronizing a live writable checkout. Each participant works independently against a declared base. A result references its base, changes, outputs and provenance. Acceptance updates a workspace head under an explicit policy; concurrent proposals remain available for review.

For general files, use a manifest containing relative path, type, size, content hash, and relevant mode information. Validate paths during materialization, contain symlinks, reject escapes, and never execute received hooks/scripts merely because the artifact is authenticated. Separate trusted executable configuration from synchronized untrusted content. Do not export credentials, local environment files, private caches, sockets or arbitrary home-directory contents by default.

Encrypt confidential blobs before publication. Bind their keys and manifest to the relevant workspace/epoch. Define when decrypted caches are allowed and who can read them. Revocation can block future key distribution; it cannot erase already received plaintext. Content hashes establish byte integrity relative to a trusted reference, not authorship, permission, or model-output correctness.

Define artifact states independently: created, transferred, verified, retained locally, replicated to a named peer, and acknowledged under that peer's retention policy. A receive callback is not necessarily durable storage. A signed receipt is evidence of an assertion by that peer, not proof that it will retain bytes forever. Recovery needs an authentic root/checkpoint, missing-object reconciliation, and an online holder.

## Stack candidates

Evaluate **Iroh** first for Rust connectivity, with a compatible blob/store version set selected after crash and interoperability tests. Its direct/relay connectivity can simplify the first internet demo. Keep durable event storage in a straightforward embedded database and define Locust's event protocol independently of a particular gossip package.

Evaluate **rust-libp2p** alongside it if protocol interoperability, custom discovery or greater transport flexibility is important. It offers the right primitives but requires more composition. **p2panda** may reduce higher-level sync/encryption work, with explicit API-maturity evaluation. **Radicle** is useful both as a reference and a possible code collaboration integration. **OpenDHT** fits small discovery records; it does not replace the blob/workspace layer. See the [survey and current caveats](landscape.md) for primary sources.

Expose a narrow local API first, with authenticated owner administration distinct from agent-scoped capabilities. A Unix socket is a sensible default. Add an MCP adapter for existing agent clients and an A2A adapter where interoperability is useful; neither should bypass the same daemon state machine or permissions. Keep transport peer IDs, agent identities, workspace membership and display names separate.

“Tiny” should be a measured target: release artifact size, idle RSS/CPU, startup latency, idle network traffic, per-workspace disk growth, and large-artifact transfer memory. MoltMesh's use of mature dependencies is reasonable, but a single executable alone is not evidence of a tiny runtime.

## Proposed implementation sequence and exit criteria

| Stage | Deliverable | Evidence required to proceed |
|---|---|---|
| 1. Protocol skeleton | Identity, signed genesis/invite, local durable events, CLI inspection | Canonical test vectors, duplicate/reordered-event handling, restart replay |
| 2. Two-peer exchange | Direct/relay connections, reconcile logs, encrypted blob transfer | Two real networks, reconnect after missed events, verified fetch and retained replica |
| 3. Collaboration | Offer/accept tasks, worker attempts, results, owner acceptance | Crash before/after claim, stale worker, request-key conflict, cancellation acknowledgment |
| 4. Workspace contributions | Immutable bases, manifests/patches, local materialization | Conflicting edits preserved, safe paths, source-offline retrieval, explicit acceptance |
| 5. Optional support peers | Replaceable relay/bootstrap and encrypted retention peer | Non-overlapping clients, all-peer cold restart, holder loss, expired discovery hints |
| 6. Interoperability | MCP/A2A adapters and SDKs | Same protocol conformance suite across clients; no permission bypass |

These are acceptance experiments, not claims of completed Locust functionality. Use targeted fault injection at acknowledgment boundaries, not only happy-path demonstrations. Public network discovery, reputation, autonomous scheduling, automatic authority failover and a universal multiwriter filesystem can remain separate later projects until a real use case demands them.

## Main dangers to keep visible

- A signed message can contain wrong results, malicious code or prompt injection. Provenance does not make it trustworthy instructions for an agent.
- Trusting a participant's sandbox does not protect the daemon's own parser, file materializer, keys, network egress or local API. Keep those boundaries narrow even when execution isolation belongs to the user.
- Offline work creates forks and stale views. Task assignment, result acceptance, membership changes and irreversible effects need explicit conflict rules.
- Availability requires online copies and resource commitments. DHT redundancy, gossip fan-out and successful local persistence are different promises.
- Global discovery creates cheap identities, unsolicited work and storage costs. Use explicit admission and operator-visible resource policies; surface rejections and resource limits instead of silently dropping data.

The immediate recommendation is to prototype the narrow collaboration flow and validate it across real networks. MoltMesh supplies valuable examples of both useful decomposition and the subtle failures that this first prototype should test for.
