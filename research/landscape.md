# Distributed agent collaboration landscape

Research date: 2026-10-03. Question: which existing protocols and open-source components could support Locust, a small Rust daemon for collaboration among independently operated agents without a mandatory central coordinator?

Method: primary documentation and repository review, followed by a deeper source review of MoltMesh at commit `707c870e3188df243e5aea3c4662daa3253270bc`. This is an exploratory survey, not an exhaustive market analysis, performance benchmark, or adoption decision. Online documentation and dependency maturity can change; recheck the selected versions before implementation.

## Assessment

The transport, file-transfer and synchronization building blocks already exist. Locust's main design work is the collaboration contract: who can join a goal, authorize an operation, claim work, publish a result, accept a change, retain shared evidence, and resolve competing changes after disconnection.

A practical first candidate is **Iroh networking plus a durable, signed collaboration log and immutable artifact storage**, with the higher-level storage dependencies selected and tested independently. **rust-libp2p** is a strong alternative where protocol interoperability or routing flexibility outweighs composition cost. **Radicle** is particularly relevant to the code/workspace layer. **A2A and MCP** are useful integration boundaries. None should be treated as supplying Locust's entire trust and execution model.

The achievable promise is no mandatory vendor-controlled coordinator or message board. Reliable internet connectivity may still require replaceable relays/bootstrap contacts; availability across non-overlapping participants may require an opt-in peer retaining encrypted data. Supporting peers can be independently operated without becoming an authority over the workspace.

## Comparison

| Component | Main layer | Relevant capability | What Locust still needs |
|---|---|---|---|
| Iroh | Connectivity and composable protocols | Rust QUIC, public-key peer addressing, hole punching, relays; gossip/blobs/docs ecosystem | Collaboration semantics, authorization, retention, version compatibility |
| rust-libp2p | Modular P2P networking | Multiple transports, peer/content discovery, pub/sub, relay/hole-punch protocols | Durable application state and more integration work |
| OpenDHT | Distributed lookup/small records | Kademlia KV, signed/encrypted values, change listeners | Durable storage and large-artifact transfer |
| p2panda | Local-first application stack | Log sync, blobs, group encryption and access-control components | Maturity evaluation and agent-specific behavior |
| Radicle | Distributed code collaboration | Git replication, signed references, issues/patches and explicit seeding | Agent execution, scheduling and non-code artifacts |
| A2A | Agent service interoperability | Agent Cards, messages, tasks, artifacts and transport bindings | Peer replication, workspace state and execution policy |
| MCP | Agent-to-tools/resources integration | Standard interface for exposing local daemon operations to existing clients | Distributed storage and coordination protocol |
| MoltMesh | Integrated agent-daemon prototype | Discovery, messaging, task APIs, thread consensus and artifact transport | Significant correctness, recovery and deployment work identified in source review |

The rows describe different layers, so these are often complementary rather than substitutes. Primary evidence and caveats follow.

## Iroh

Iroh connects peers by public key over QUIC, attempts direct connectivity and can use relay fallback. Transport authenticates a peer; application authorization remains the application's responsibility. The project includes relay code that operators can run themselves. This makes it an attractive Rust-first connectivity candidate. [Repository](https://github.com/n0-computer/iroh), [Rust API and connection behavior](https://docs.rs/iroh/latest/iroh/).

Its higher layers are separate choices:

- `iroh-gossip` broadcasts by topic using epidemic dissemination. Use notifications to prompt durable synchronization; a live broadcast is not itself an offline message archive. [Gossip implementation](https://github.com/n0-computer/iroh-gossip).
- `iroh-blobs` supports BLAKE3-verified transfer, range requests and sequences of blobs. Provider discovery is outside its transfer protocol. The repository reviewed currently warns that its current development version is not production quality and points production users to 0.35. This is a reason to verify a compatible version set and storage behavior, not to combine that older crate blindly with latest Iroh. [Blob repository](https://github.com/n0-computer/iroh-blobs), [wire protocol](https://docs.rs/iroh-blobs/latest/iroh_blobs/protocol/index.html).
- `iroh-docs` synchronizes signed namespace/author/key entries referring to content hashes, with set reconciliation and persistent storage. It composes gossip and blobs. A replicated KV document does not define task claims, acceptance authority or correct source-code merges. [Docs repository](https://github.com/n0-computer/iroh-docs).

Durability needs an explicit application contract: the documented blob filesystem store can lose recent writes on unclean shutdown, and clean shutdown has stronger behavior than dropping handles. Test what an acknowledgment actually guarantees. [Filesystem store](https://docs.rs/iroh-blobs/latest/iroh_blobs/store/fs/index.html).

Public Iroh community relays currently have rate limits and no uptime guarantee. Self-hostable infrastructure avoids a compulsory vendor but does not remove operational bandwidth and availability costs. [Service description](https://www.iroh.computer/pricing).

## rust-libp2p

The Rust implementation is a broad, modular stack with transport, multiplexing, swarm and protocol layers; the repository lists substantial downstream users. It is the candidate to evaluate when existing libp2p interoperability, custom routing or transport flexibility is important. More choices also mean more application wiring and operational validation than a narrower connectivity API. The comparative engineering cost is our assessment, not a measured benchmark. [Repository](https://github.com/libp2p/rust-libp2p).

Circuit relays and DCUtR support connecting peers behind NAT and upgrading a relayed connection to a direct one. They still require reachable relay infrastructure and do not guarantee direct connectivity through every network. [DCUtR specification](https://github.com/libp2p/specs/blob/master/relay/DCUtR.md).

Gossipsub depends on discovery supplied by the application and maintains recent-message caches. Locust should reconcile a persistent log after reconnect, using gossip for timely notification. [Pub/sub documentation](https://docs.libp2p.io/concepts/pubsub/).

## OpenDHT

OpenDHT is a C++17 Kademlia implementation with Rust bindings, redundant volatile KV storage, listeners and optional cryptographic signing/encryption. Its wiki documents values up to 64 KiB and expiration policies. It fits small rendezvous records or discovery hints more naturally than a durable artifact/workspace store. The Rust bindings also bring C++ and cryptographic-library packaging considerations into an otherwise Rust daemon. [Repository and dependencies](https://github.com/savoirfairelinux/opendht), [data model](https://github.com/savoirfairelinux/opendht/wiki).

The BitTorrent/OpenDHT analogy is useful for discovery and immutable bytes. It does not answer mutable workspace history, authorization, replication commitments or task execution semantics.

## p2panda and Willow

p2panda is especially relevant prior art because it combines Rust libraries for networking, append-only-log sync, blobs, confidential discovery, group management and encryption, using Iroh among its dependencies. Its APIs and core types are explicitly still considered unstable for production. Evaluate whether adopting the whole stack reduces risk compared with composing selected parts; do not infer that a listed encryption feature has been independently audited for Locust's threat model. [Repository and component descriptions](https://github.com/p2panda/p2panda).

Willow provides protocol ideas for selective synchronization, mutable offline data and capability-based access through Meadowcap. Its official Rust status page currently describes implemented data-model/capability components with additional storage and transfer work forthcoming. It is valuable design material; check exact implementation completeness before making it the MVP's critical dependency. [Protocol overview](https://willowprotocol.org/), [Rust status](https://willowprotocol.org/rust/).

## Radicle

Radicle is the closest mature architectural reference for the code-collaboration part of the idea: Rust nodes, Git data replication, signed references and collaborative objects, with participant-specific repository namespaces and explicit seeding policies. Gossip advertises updates; peers fetch actual objects. It distinguishes the working copy from stored replicated data, which is a useful model for avoiding concurrent agents overwriting one shared checkout.

Availability requires an online holder; dedicated seed peers improve it. Private repositories use access-controlled replication and are not encrypted at rest according to the reviewed guide. Radicle does not itself provide agent task execution. Locust could learn from its signed identity/ownership and patch acceptance model or integrate it for repositories, while using separate protocols for tasks and general artifacts. [Official protocol guide](https://radicle.dev/guides/protocol).

## A2A

A2A defines Agent Cards, message/task/artifact semantics, streaming and error behavior. Current standard bindings include JSON-RPC, gRPC and HTTP+JSON. The specification also permits custom bindings, provided their data model, operations, security and semantics conform. It therefore does not categorically prohibit a P2P binding, but an A2A-shaped protobuf alone does not establish conformance. [Specification](https://a2a-protocol.org/latest/specification/).

The [focused A2A assessment](a2a-assessment.md) reviews the released specification and official Rust SDK. Keep A2A as an optional interoperability adapter when a concrete external agent or client needs it; no such integration is required for the October 4 release. Define exactly which task state is authoritative and how retries/cancellation map to the external API. A2A interoperability should be tested against the selected version; it does not provide replicated workspace storage by itself.

## MCP

MCP standardizes how AI applications connect to tools and external context/resources. It is a natural adapter for exposing a local Locust daemon to existing agent clients: inspect a goal, read synchronized evidence, offer work, submit a result, or retrieve an artifact. [Official introduction](https://modelcontextprotocol.io/docs/2026-07-28/getting-started/intro).

Our architectural distinction: the MCP interface can present Locust operations, while the daemon enforces workspace capabilities and durable state transitions. MCP access must not implicitly grant arbitrary remote execution. Existing agent clients can remain responsible for their execution environment and sandbox, while Locust governs what can be requested or shared across peers.

## MoltMesh

MoltMesh is an unusually close implementation reference. The reviewed commit contains a Go daemon with libp2p, custom Kademlia discovery, GossipSub, persistent queues, task lifecycle APIs, actor-managed consensus threads, Bitswap artifacts and archive recovery. It shows how these concerns can be divided into concrete modules, and supplies useful failure cases for Locust's design. [Reviewed source](https://github.com/sahilpohare/MoltMesh/tree/707c870e3188df243e5aea3c4662daa3253270bc).

It is not a validated foundation for Locust's security or availability promises. Source review found a gap between hash consistency and authenticated recovered history, incomplete artifact caching/provider publication, public-IPFS bootstrap claims inconsistent with the custom DHT, and recovery dependencies on ephemeral discovery records. Those findings are source-based unless a companion test report says otherwise; they do not constitute a deployed-system penetration test.

See [networking, storage and offline behavior](moltmesh-networking-and-storage.md), and the [research index](README.md) for the complementary task, consensus, security and runtime studies.

## What to prove before selecting a stack

These are proposed acceptance scenarios, not completed experiments:

1. Two participants on separate home/mobile networks connect; relay fallback and the selected relay are visible.
2. A disconnected participant rejoins and retrieves all authorized missed events without relying on gossip replay.
3. A received artifact remains available after its original author goes offline, according to an explicit retention policy.
4. Restarting all peers preserves data and restores discovery from durable local state.
5. Concurrent changes remain attributable and reviewable; synchronization never implies permission to execute them.
6. Revoked members cannot obtain future plaintext, while the interface accurately states that already received data cannot be recalled.
7. Duplicate task delivery and crashed workers do not duplicate externally visible side effects without an explicit application decision.
8. Measured binary size, idle memory, disk growth and background network traffic support the promise of a small daemon.

The initial stack choice should optimize these proofs. A broad feature list or a successful localhost demo is weaker evidence than a small implementation surviving disconnection, restart and permission changes.
