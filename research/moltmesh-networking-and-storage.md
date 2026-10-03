# MoltMesh networking, artifacts, and offline behavior

Research date: 2026-10-03. Inspected `sahilpohare/MoltMesh` default `actor-model` commit `707c870e3188df243e5aea3c4662daa3253270bc` and pinned dependency source. Method: read-only code tracing, comparison against documentation, and inspection of existing tests. No tests or network probes were executed for this note. Findings distinguish source-established behavior, inferred consequences, and scenarios still requiring runtime validation. These are research findings, not an accepted Locust implementation plan.

See the [research index](README.md) for complementary task, security and consensus investigations, and the [validation report](moltmesh-validation.md) for the separately executed local demo and other runtime checks.

## Executive assessment

The project has real building blocks: libp2p transport, a custom Kademlia DHT, signed discovery cards, GossipSub, SQLite inbox/outbox, IPFS Bitswap plus a persistent flatfs blockstore, and an archive worker. It is useful implementation prior art, but **not evidence that internet-wide, offline-capable, private shared workspaces are solved**. The major gaps are recovery authenticity, bootstrapping/reachability, storage/discovery lifecycle, and application-level file replication. Documentation frequently describes an older or more capable implementation than the current path.

## Actual network path

- `daemon/node/node.go:96-118`: Ed25519 host identity; explicit QUIC + TCP transports; NAT port mapping, NAT service, hole-punching; custom DHT protocol prefix `/a2a`, `ModeAutoServer`, validators for agents/names/threads. This is **not the public IPFS DHT** despite the `New` comment and README language.
- `node.go:123-126`: GossipSub uses StrictSign. Transport/source authentication does not itself imply per-topic authorization.
- `node.go:169-209`: mDNS `moltmesh` LAN discovery, plus DHT rendezvous on `moltmesh`, retry every 30 seconds.
- `node.go:260-296`: configured IPFS bootstrap is expressly ignored; only explicit bootstrap multiaddrs dialed. No successful bootstrap logs a warning and still allows the daemon to start. A pair on the same LAN may connect through mDNS; internet nodes need reachable MoltMesh bootstrap contacts or another explicit connection route.
- `cmd/moltmesh/daemon.go:241-257`: canonical daemon binds IPv4 QUIC/TCP and passes only the listed options. `pkg/config/config.go:67-68` exposes `announce_addrs`, but no consumer was found. No AutoRelay peer source/static relays or relay-service configuration was found in source.
- Dependency nuance: libp2p v0.48.0 **does enable relay transport by default**. Thus do not claim no relay protocol support. It does not automatically supply relay discovery/reservation infrastructure. Current default path does not demonstrate two NATed remote nodes successfully establishing their prerequisite relayed connection. Cross-NAT failure is a test gap/likely deployment limitation, not a tested universal failure.

Permalinks: [host construction](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/node/node.go#L96-L118), [bootstrap behavior](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/node/node.go#L260-L296), [daemon configuration](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/cmd/moltmesh/daemon.go#L241-L257), [libp2p defaults](https://github.com/libp2p/go-libp2p/blob/v0.48.0/defaults.go#L94-L97).

## Definite recovery authenticity defect: hashes are accepted as authorship/finality

`ThreadHead` is intentionally unsigned (`daemon/thread/publisher.go:35-41`). `pkg/a2avalidator/thread_head.go:28-83` validates basic shape/CID/height and matching ID, and `Select:91-95` prefers the greatest height. It does not bind a pointer to a creator or a committed quorum.

`daemon/thread/verify.go:86-131` only checks block hash recomputation, parent linkage, thread ID, and height. It does **not** verify entry signatures, proposer signatures, authorized membership, or a consensus commitment certificate. `daemon/thread/raft.go:837-849` shows the hash is publicly computable from ordinary fields and entry bytes.

Both production read paths then trust this result:

- `daemon/thread/archive_worker.go:227-269`: verify a signed public descriptor, hash-walk the chain, directly `Store.ImportHistory`.
- `daemon/rpc/server_threads.go:638-670`: same verification then `ImportHistory` via manager; `ActorManager` embeds `*Store` (`actor_manager.go:23-26`), so there is no hidden validating wrapper.
- `store_blocks.go:31-54` transactionally inserts/replaces descriptor and blocks without more validation.

A valid descriptor authenticates the descriptor itself, but the reviewed path does not establish that its creator or an authorized consensus group approved the recovered history. Consequently, a self-consistent chain can pass without evidence of authorized authorship or finality. Payload tamper-after-hashing tests do not cover this missing validation. Existing positive fixture uses `did:key:zProposer` and no signatures (`verify_test.go:11-30,154-160`), demonstrating that authentication is absent from the verifier's contract. This is a **certain source-level authenticity flaw**, not a demonstrated exploit against a deployed network.

Permalinks: [unsigned head](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/thread/publisher.go#L35-L49), [verification](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/thread/verify.go#L86-L131), [import](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/thread/store_blocks.go#L28-L54), [positive test fixture](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/thread/verify_test.go#L11-L30).

### Related definite resource-exhaustion path

The unsigned head accepts any positive `int64` height. `verify.go:91` immediately allocates `make([]*pb.ThreadBlock, head.Height)` before fetching or authenticating the chain. This creates a source-established risk of allocation failure, process memory exhaustion, or panic from untrusted discovery metadata. Preferring the greatest unverified height also lets unusable metadata displace valid lower records. Defensive design should authenticate checkpoints, process history incrementally, and account for local resources before allocation. No adversarial allocation probe was run.

[Validator](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/pkg/a2avalidator/thread_head.go#L74-L95), [allocation](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/thread/verify.go#L86-L107).

## Artifacts are immutable blocks, not workspace synchronization

`SendFile` stores the entire unary request as one raw CIDv1 SHA-256 block (`server_files.go:22-55`, `p2putil.go:39-48`), always in local blockstore, with inline response at <=64 KiB. Bitswap is notified. There is no directory manifest, file watcher, version graph, concurrent path resolution, chunk-level resume, or workspace materialization layer in this path. `FetchFile:92-125` checks local storage, optionally resolves/connects a supplied DID, otherwise asks Bitswap, then slices the already-materialized block into 32 KiB gRPC messages. Those chunks are **not** chunked P2P file transfer.

Canonical gRPC server has no receive-size override (`daemon.go:403-410`), so unary upload inherits gRPC's default receive limit (4 MiB), including protobuf overhead. Do not describe this implementation as supporting arbitrary large files. No large-file boundary probe was run. The one-block approach also limits practical memory use and Bitswap interoperability regardless of RPC streaming cosmetics.

**Definite remote-cache gap:** after successful `Bitswap.GetBlock`, `FetchFile` never puts the fetched block in local blockstore. Boxo v0.39.0's receive path only dispatches received blocks to sessions/notifiers; it does not persist them. A downloaded file therefore does not make that daemon a retained replica. Retrying after the sole source disappears is not served from this daemon's CAS (unless some independent path stored it). Source: `server_files.go:104-109`; Boxo `bitswap/client/client.go:526-568` and `server/internal/decision/engine.go:1004-1013`.

**Definite discoverability gap:** `SendFile` never calls DHT `Provide` for the file CID. `NotifyNewBlocks` only informs Bitswap peers/sessions; it is not a DHT publication or replication request. `rg` found app DHT `Provide` only for agent capability and thread archive rendezvous. Consequently CID-only lookup from a peer that is not connected to a holder has no guaranteed provider record. On a small fully connected mesh it can work, masking this gap.

**Privacy boundary:** raw file bytes are placed in the default Bitswap-served store with no per-file encryption or authorization callback configured (`node.go:151-156`). Thread encryption does not automatically encrypt separately uploaded attachments. A CID is a content identifier, not an access policy; caller-side encryption would be necessary for confidential files shared through this path. Also no app block deletion/GC/pinning policy found.

Permalinks: [file operations](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/rpc/server_files.go#L20-L125), [Bitswap setup](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/node/node.go#L132-L156), [Boxo receive path](https://github.com/ipfs/boxo/blob/v0.39.0/bitswap/client/client.go#L526-L568), [Boxo notification](https://github.com/ipfs/boxo/blob/v0.39.0/bitswap/server/server.go#L382-L396).

## Archive durability is materially weaker than local persistence

Useful implemented pattern: committing a block persists it locally before asynchronous publishing; the archive worker independently checks bytes before import and issues signed receipts; ADR-0019 explicitly admits commits are not gated on an archive quorum and all-offline holders mean unavailable content.

However:

1. `Publisher:122-171` writes local blob, best-effort DHT height/head records, then advertises a thread rendezvous. It does not push bytes to a remote durable store. Failure is logged and not retried by this actor; `EnableActor:85-86` installs no scheduled replay.
2. `ArchiveWorker.Run/syncKnown:52-84` enumerates only locally known threads. It does not discover arbitrary new threads or automatically recruit independent archival hosts. Additional peers must first learn/save the thread.
3. **Local-read bypass:** `archive_worker.go:252`, `publisher.go:226` and `archive.go:132` always use Bitswap network `GetBlock`. Boxo `client.go:427-457` starts a network session and does not check local CAS. The normal file RPC already documents/fixes this same issue (`server_files.go:84-93`). A sole surviving holder can fail/block refreshing its own archive despite locally having the bytes. Archive worker uses daemon lifetime context for this fetch and processes known threads sequentially, so one missing source can stall the remaining sweep. Exact timing requires runtime test.
4. Each sync walks **every height back to genesis**, sequentially doing one DHT lookup per height plus a fetch per block (`verify.go:91-127`), holding decoded blocks and raw fetched blocks until import (`archive_worker.go:241-275`). One-minute sweep does not make this incremental. For Locust this is an obvious scaling/availability bottleneck.
5. DHT records have no persisted datastore configured (`node.go:107-115`). Dependency v0.40.0 defaults to an in-memory map (`internal/config/config.go:107-121`) and a 48-hour maximum record age (`amino/defaults.go:40-43`). No replay/republication of all historical per-height pointers found. Archive sync refreshes a provider record, not historical DHT heads. Cold restart of all DHT participants can therefore lose discoverability even with intact SQLite/flatfs data; old height pointers can age out. The chain uses `ParentHash`, not `ParentCid`, making every old mutable lookup a recovery dependency.
6. Publisher comment says publishing height first means head walkers always find it, but after a failed height `PutValue` it still attempts head `PutValue` (`publisher.go:161-165`). The stated ordering invariant is false under partial failure.
7. Recovery envelopes are persisted by archive workers only when seen on live gossip (`archive_worker.go:87-129`), while `Sync` fetches thread blocks only. A late archive host does not backfill missing historical envelopes during this sweep. DHT envelope pointers exist, but losing the sole envelope blob holder is still possible.

Permalinks: [publisher](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/thread/publisher.go#L96-L172), [worker](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/thread/archive_worker.go#L200-L291), [DHT default datastore](https://github.com/libp2p/go-libp2p-kad-dht/blob/v0.40.0/internal/config/config.go#L107-L121), [record age](https://github.com/libp2p/go-libp2p-kad-dht/blob/v0.40.0/amino/defaults.go#L40-L43), [honest ADR boundary](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/docs/adr/0019-verified-archive-replication-and-recovery-discovery.md#L31-L41).

## Offline messaging semantics

Actual durable retry is a sound primitive: `SendMessage` returns queued only after SQLite enqueue (`server_messaging.go:12-29`); recipient acknowledges after inbox insertion/application callback (`deliver.go:442-459`); outbox stores terminal state and reclaims processing leases after restart (`outbox.go:175-255,311-315`). This is at-least-once delivery; work/side-effects still need idempotency.

Scope: **sender-retained retry**, not third-party mailbox. Sender and recipient must overlap online. For SDK agents, a recipient must have a currently live authenticated session (`deliver.go:393-405`), even when its daemon is online, so daemon uptime does not equal agent addressability.

Ordinary messages expire at 72 hours and reach dead-letter after ten attempts (`outbox.go:18-21,178-180,243-244`). Current retry uses fixed 5-second eligibility with a 10-second periodic worker (and enqueue flushes), not documented exponential backoff. A rapidly rejected offline/sessionless destination can burn ten attempts long before the nominal TTL. Thread invites/task requests/results/cancels are a separate durable class exempt from these attempt/TTL limits (`outbox.go:347-353`). Do not generalize unlimited retry to every message.

Agent discovery has a further lifecycle concern: daemon's card is republished every 45 minutes, but SDK-signed cards in `r.cards` are not handled by that loop (`registry.go:37-44,60-80,259-279`). Local capability results return cached cards without expiry check (`156-164`), while remote resolution rejects expired cards (`135-136`). SDK refresh policy needs joint review before labeling it an end-to-end defect.

Permalinks: [enqueue/ack](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/deliver/deliver.go#L393-L459), [retry rules](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/outbox/outbox.go#L175-L255), [durable kinds](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/daemon/outbox/outbox.go#L347-L355).

## Evidence limitations and test gaps

- `e2e/e2e_test.go:40-67` uses in-process localhost TCP nodes, explicitly no DHT/bootstrap. File tests at 304-361 upload only; the "large" fixture is 128 KiB and does not prove remote fetch.
- TypeScript real-daemon integration exists and previously surfaced real flatfs/local-fetch bugs (node.go:141-150, server_files.go:84-90 comments). This is a positive testing choice. Inspect/run results separately; reading tests is not a pass.
- `verify_test.go:83-143` uses fake CIDs/map lookups; tests establish self-consistent hash traversal, not DHT lifetime, availability, remote replication or authenticated history.
- `archive_test.go` only checks stable/scoped rendezvous CID derivation. No full archive worker cold-recovery test found in inspected tests.
- Need WAN NAT matrix, mutually non-overlapping clients, source-offline-after-fetch, cold restart of all nodes, aged DHT records, crash immediately after ack, partial DHT publication failure, archive envelope backfill, and genuine multi-MiB files before strong availability claims.

## Locust lessons

1. Preserve the clean local daemon/API boundary, immutable artifact handles, SQLite transactional queues, signed identities, and explicit archive receipts.
2. Specify independently: transport identity, workspace membership, content integrity, author authorization, task finality, and retention. A CID/hash chain proves only the bytes/links under a chosen root.
3. Keep public discovery hints untrusted and bound work to authenticated incremental state. Recover to a trusted genesis/checkpoint with author/capability signatures and any needed finality proof; avoid allocating from advertised remote height.
4. Make immutable manifests include parent CIDs directly. DHT is a replaceable hint/rendezvous mechanism, not the sole index needed for every historical object.
5. Implement transfer -> verify -> persist -> fsync/transaction -> receipt as explicit stages; local upload, cached retrieval, replication and durable availability need separate statuses.
6. Encrypt attachments independently before CAS publication; bind encryption epoch/capability to a manifest. Track retention/pinning and resource usage locally.
7. A directory/worktree is a materialized view of accepted immutable changes, not arbitrary latest bytes mirrored across machines. Begin with snapshots/patches per participant and explicit integration rules.
8. Use a transport with tested relay fallback, make relay/bootstrap dependencies configurable and replaceable, and measure two real remote networks early.
9. State offline contract precisely: sender retry requires overlap; asynchronous collaboration across non-overlapping laptops needs an opt-in durable mailbox/archive peer or equivalent replication arrangement.
