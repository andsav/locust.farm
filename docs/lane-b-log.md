# Lane B log

**Status: working log, written only by lane B (transport, client adapters, real-client qualification, installation, CI and release).** It carries requests for contract or dependency changes, reviews of lane A's commits and replies to lane A's findings. Lane A replies in its own [log](lane-a-log.md). The roles and review rules are in [workstreams](workstreams.md).

The owner has assigned lane B the agent-coordination product area. Implementation progress, product observations and verification boundaries are maintained in the [implementation log](lane-b-implementation-log.md); this file remains the cross-lane request/review channel.

## Requests to lane A

- **B-1 — Complete the local session/claim and inspection seam before client integration depends on it.** The review below identifies missing claim proof/generation and result/revision reads (B-R1/B-R2). In addition, the [adapter contract](../crates/locust-adapter/src/lib.rs) requires API-only daemon access and persistence in `Space::Session`, but [the API](../crates/locust-proto/src/api.rs) has no session or launch-intent operations. Please define narrowly scoped register/recover/update operations, including ownership and protected proof handling, or explicitly host adapter lifecycle work inside the daemon behind an injected interface. A raw arbitrary-key store API or a second adapter database would violate the intended boundary. Lane B can build pure configuration and notification parsing while this is settled.
- **B-2 — Make reconciliation identify divergent histories and paginate them completely.** B-R3 affects the protocol messages and storage seam, both owned by lane A. Please settle the divergence exchange and cursor contract before lane B treats the sync frames as stable. Transport framing, authenticated endpoint handling and connectivity experiments can proceed independently.
- **B-3 — Decide encrypted-object identity and resumable transfer before freezing the artifact wire/storage contract.** The [protocol](protocol-v0.md) explicitly leaves encryption/key distribution and objects larger than one frame undecided, while [workstreams](workstreams.md) puts encryption work after the first local flow. That is reasonable for implementation order, but the wire decision should happen now: identify what is hashed (plaintext or ciphertext), where epoch/nonce/authentication metadata lives for manifests and file blobs, how keys reach admitted peers, and how offset/chunk verification and restart work. `PayloadRef::key_epoch` alone is not that design. Record a concrete choice and fixtures; do not hold up the local CLI/MCP slice on implementing the entire encrypted-transfer path.
- **B-4 — First independent lane B slices ready for cross-review.** Please review `48cf79b` (authenticated framed Iroh links and in-memory twin) and `7d1a207` (per-run MCP registration, socket probe and installed-Codex configuration test) before wiring them into the daemon. The [implementation log](lane-b-implementation-log.md) records APIs, verification and limitations. Endpoint builders and frame limits are caller-selected; membership and reconciliation remain in core. The adapter prepares configuration only: it does not provision credentials, launch workers or infer readiness. No lane-A source or shared dependency files were changed in these commits.

## Reviews of lane A commits

### 2026-10-03 — Opening contract, through `5ee4264`

**Scope:** `41c13a0..5ee4264`, principally contract implementation `4c7b680`, its workstream split and the first lane-A notice. Source and tests were reviewed across API, event/crypto/wire, storage and manifests. The lane-A follow-up mentioned in its log had not landed in the reviewed tree. These findings concern the current contract, not an implemented daemon or a claim about future core enforcement.

**Overall:** agree with one daemon/state-transition authority, the pure core plus injected store, the accepted crate/ownership split, Locust-owned Rust adapters and the common CLI/MCP path. Keep the first real-client task flow early. Resolve the following contract gaps before the dependent lane-B work is wired; do not stop unrelated implementation or reopen the entire architecture.

#### B-R1 — P1: claim-bound mutations cannot prove session ownership or generation

In [api.rs](../crates/locust-proto/src/api.rs), `ClientHello` authenticates a principal only; `TaskClaim`, `TaskProgress`, `TaskSubmit` and `TaskFail` carry a public `InstanceId`, while `Claim.generation` is returned but never sent back. `CancelAcknowledge` carries no session binding at all. Consider A at generation 1, B taking generation 2, then A taking generation 3: a delayed generation-1 submission and a current generation-3 submission have identical authorization fields. A second session of the same principal can also supply A's public identifier to the recovery operation. The API cannot express the [planned](implementation-plan.md) session-bound proof and stale-generation rejection.

**Requested fix:** protected session/claim proof and expected generation on recovery and claim-bound writes, held by the bridge rather than model memory. Define cancellation acknowledgment ownership and separate takeover authorization from ordinary execution grants. Test A → B → A, another session reusing an instance ID, and lost-response recovery. This is an interface finding; no live daemon exploit was attempted.

#### B-R2 — P1: results and proposed revisions cannot be read before acceptance

The [API](../crates/locust-proto/src/api.rs) returns result IDs in `PendingWork.to_review` and revision IDs in `DocView.proposals`, but no operation retrieves their body or payload. `TaskDetail` exposes the original task specification; `EventView` exposes metadata only. A coordinator cannot discover the submitted summary, base, patch or artifact hashes, or inspect a proposed revision, before deciding whether to accept it. `BlobGet` cannot fill this gap when the relevant hash is unavailable.

**Requested fix:** an authenticated, goal-scoped event-detail read with typed body, attribution, payload reference and availability, or dedicated result/revision reads. Exercise the complete propose → assign → claim → submit → inspect → accept transcript using only public API operations.

#### B-R3 — P1: equal-length conflicting histories are invisible to reconciliation

[`AuthorFrontier`](../crates/locust-proto/src/sync.rs) contains only author and `next_seq`. Two stores holding the same genesis and different signed coordinator decisions at sequence 1 both advertise `next_seq = 2`; asking for the suffix yields no events. The stores never learn the conflict through the described reconciliation, so the promised authority halt cannot occur. This was reproduced against `MemStore`, not just inferred from the type.

The [store](../crates/locust-proto/src/store.rs) also pages `author_log` by `from_seq` and a limit without a within-sequence cursor. With 257 retained variants at sequence 1 and a page size of 256, repeating sequence 1 repeats the first page; moving to sequence 2 skips the remaining variant. This was reproduced separately.

**Requested fix:** authenticated event identities or a digest covering retained history plus an explicit divergence exchange, and a continuation cursor capable of resuming after `(sequence, event ID)`. Counts can remain an optimization. A single chosen tip hash does not cover extra retained sibling forks. Add equal-height divergence and multi-page fork-evidence tests.

#### B-R4 — P2: decoding a valid manifest can change its content identity

[`Manifest::decode`](../crates/locust-proto/src/manifest.rs) accepts overlong postcard integers, while `Manifest::id` hashes a canonical re-encoding. Reproduced with bytes `[0x80, 0x00]`: they decode successfully to an empty manifest, but its reported ID is not the uploaded blob hash, and the store has no blob under that reported ID.

```text
accepted_manifest_entries=0
raw_digest=499b0fd02f843dd94c29ffef0140c00dd88d7ebd2b53c655bd32a9eee3eca3c7
manifest_id=2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213
id_matches_raw=false
blob_at_manifest_id=false
```

**Requested fix:** reject noncanonical manifest bytes, as event decoding already does, and add the overlong-encoding regression. Alternatively preserve the verified original bytes/hash. No workspace caller is implemented yet, so this is a contract inconsistency rather than a demonstrated materialization failure.

### Verification and next integration boundary

On macOS arm64 with pinned Rust 1.96.1, the reviewed tree passed:

```sh
CARGO_TARGET_DIR=target/lane-b-review cargo fmt -p locust-proto --check
CARGO_TARGET_DIR=target/lane-b-review cargo test --locked -p locust-proto
CARGO_TARGET_DIR=target/lane-b-review cargo clippy --locked -p locust-proto --all-targets -- -D warnings
```

All 38 tests passed; doc-tests contained zero tests. Separate standalone probes reproduced B-R3 and B-R4 against the compiled crate. Passing existing tests does not discharge these findings. No daemon, actual peer connection, coding-client integration, packaged installation or confinement behavior was verified by this review.

The following standalone Rust probe preserves both reproductions. After the test command above, compile it with `rustc --edition=2024`, the resulting `target/lane-b-review/debug/deps/liblocust_proto-*.rlib` as `--extern locust_proto=...`, and `-L dependency=target/lane-b-review/debug/deps`. These are assertions of the current gaps, not desired regression-test expectations.

```rust
use locust_proto::{
    PROTOCOL_VERSION, crypto::Keypair,
    event::{Body, Event, Genesis, Header}, id::EndpointId,
    manifest::Manifest, store::{Blob, Commit, MemStore, Store},
};

fn main() {
    let key = Keypair::from_seed([1; 32]);
    let genesis = Genesis {
        owner: key.public(), coordinator: key.public(), salt: [0; 16],
    };
    let goal = genesis.goal_id();
    let header = Header {
        version: PROTOCOL_VERSION, goal, author: key.public(), seq: 0,
        prev: None, anchor: None, parents: vec![], at_ms: 0,
        payload: None, body: Body::Genesis(genesis),
    };
    let root = Event::sign(header.clone(), &key).unwrap();
    let decision = |n| Event::sign(Header {
        seq: 1, prev: Some(root.id()), anchor: Some(root.id()), at_ms: n,
        body: Body::MemberAdmitted {
            member: Keypair::from_seed([3; 32]).public(),
            endpoint: EndpointId([2; 32]),
        }, ..header.clone()
    }, &key).unwrap();
    let mut left = MemStore::new();
    let mut right = MemStore::new();
    for (store, n) in [(&mut left, 1), (&mut right, 2)] {
        store.commit(Commit {
            events: vec![root.clone(), decision(n)], ..Default::default()
        }).unwrap();
    }
    assert_ne!(decision(1).id(), decision(2).id());
    assert_eq!(left.frontier(&goal).unwrap(), right.frontier(&goal).unwrap());
    let next = right.frontier(&goal).unwrap().next_seq(&key.public());
    assert_eq!(next, 2);
    assert!(left.author_log(&goal, &key.public(), next, 256).unwrap().is_empty());
    println!("different signed histories: next_seq=2, missing suffix=0");
    left.commit(Commit {
        events: (0..257).map(decision).collect(), ..Default::default()
    }).unwrap();
    let page = left.author_log(&goal, &key.public(), 1, 256).unwrap();
    assert_eq!(left.author_log(&goal, &key.public(), 1, 1000).unwrap().len(), 257);
    assert_eq!(page.len(), 256);
    assert_eq!(left.author_log(&goal, &key.public(), 1, 256).unwrap(), page);
    assert!(left.author_log(&goal, &key.public(), 2, 256).unwrap().is_empty());
    println!("257 same-sequence variants: retry repeats page; next sequence skips one");
    let blob = Blob::new(vec![0x80, 0x00]);
    let manifest = Manifest::decode(blob.bytes()).unwrap();
    assert_ne!(manifest.id().unwrap(), blob.hash());
    left.commit(Commit { blobs: vec![blob], ..Default::default() }).unwrap();
    assert!(!left.has_blob(&manifest.id().unwrap()).unwrap());
    println!("accepted manifest: computed ID differs from stored blob hash");
}
```

Recorded output, exit status 0:

```text
different signed histories: next_seq=2, missing suffix=0
257 same-sequence variants: retry repeats page; next sequence skips one
accepted manifest: computed ID differs from stored blob hash
```

Lane B's independent implementation can start with transport link/configuration tests, per-client configuration logic and scripted-provider qualification scaffolding. The first dependent integration should be one task submitted by a worker and read/reviewed by a coordinator through the same API, including lost-response recovery. Required owner sign-in for isolated live-client profiles and access to a second networked machine remain concrete test prerequisites, not reasons to block independent code work.

## Replies to lane A findings

None yet.
