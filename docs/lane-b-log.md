# Lane B log

**Status: working log, written only by lane B (transport, client adapters, real-client qualification, installation, CI and release).** It carries requests for contract or dependency changes, reviews of lane A's commits and replies to lane A's findings. Lane A replies in its own [log](lane-a-log.md). The roles and review rules are in [workstreams](workstreams.md).

The owner has assigned lane B the agent-coordination product area. Implementation progress, product observations and verification boundaries are maintained in the [implementation log](lane-b-implementation-log.md); this file remains the cross-lane request/review channel.

## Scope notice — October 3

The owner made **Codex, Claude Code, Factory Droid and Pi** the required first-release client baseline. Lane B updated the [plan](implementation-plan.md), [workstreams](workstreams.md) and [qualification matrix](release-evidence.md). All four must complete the common task/review/recovery flow; automatic wake is restricted to Merak for now, while all four baseline clients use active sessions and explicit resume. This needs four client adapters and evidence rows, not a client-specific daemon authority or protocol. No contract change is requested by this scope update.

## Requests to lane A

- **B-1 — Complete the local session/claim and inspection seam before client integration depends on it.** The review below identifies missing claim proof/generation and result/revision reads (B-R1/B-R2). In addition, the [adapter contract](../crates/locust-adapter/src/lib.rs) requires API-only daemon access and persistence in `Space::Session`, but [the API](../crates/locust-proto/src/api.rs) has no session or launch-intent operations. Please define narrowly scoped register/recover/update operations, including ownership and protected proof handling, or explicitly host adapter lifecycle work inside the daemon behind an injected interface. A raw arbitrary-key store API or a second adapter database would violate the intended boundary. Lane B can build pure configuration and notification parsing while this is settled.
- **B-2 — Make reconciliation identify divergent histories and paginate them completely.** B-R3 affects the protocol messages and storage seam, both owned by lane A. Please settle the divergence exchange and cursor contract before lane B treats the sync frames as stable. Transport framing, authenticated endpoint handling and connectivity experiments can proceed independently.
- **B-3 — Decide encrypted-object identity and resumable transfer before freezing the artifact wire/storage contract.** The [protocol](protocol-v0.md) explicitly leaves encryption/key distribution and objects larger than one frame undecided, while [workstreams](workstreams.md) puts encryption work after the first local flow. That is reasonable for implementation order, but the wire decision should happen now: identify what is hashed (plaintext or ciphertext), where epoch/nonce/authentication metadata lives for manifests and file blobs, how keys reach admitted peers, and how offset/chunk verification and restart work. `PayloadRef::key_epoch` alone is not that design. Record a concrete choice and fixtures; do not hold up the local CLI/MCP slice on implementing the entire encrypted-transfer path.
- **B-4 — First independent lane B slices ready for cross-review.** Please review `48cf79b` (authenticated framed Iroh links and in-memory twin) and `7d1a207` (per-run MCP registration, socket probe and installed-Codex configuration test) before wiring them into the daemon. The [implementation log](lane-b-implementation-log.md) records APIs, verification and limitations. Endpoint builders and frame limits are caller-selected; membership and reconciliation remain in core. The adapter prepares configuration only: it does not provision credentials, launch workers or infer readiness. No lane-A source or shared dependency files were changed in these commits.
- **B-5 — Transport qualification probe ready for cross-review.** Please review `b4daf3f` (path observations, relay readiness and acknowledged stream shutdown) and `905f31a` (runnable probe and process checks). The [runbook](transport-probe.md) and [retained findings](../research/iroh-transport-probe.md) record behavior and checks. Default and custom n0 relay exchanges passed using two processes on one host; separate-machine/network qualification is still open. Local review caught and fixed disabled HTTPS relay selection and blocking diagnostic output. The final scoped checks pass; whole-workspace checks encountered the in-progress workspace crate's formatting and `materialize.rs:165` lifetime error, which lane B left untouched. No contract or dependency files changed.

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

## Replies to lane A transport findings — October 3

Implemented in `28dcfdd`; the [implementation log](lane-b-implementation-log.md) and [runbook](transport-probe.md) record verification and integration requirements.

- **A-R1:** `usize` limits, contract encode/admission helpers and mutable sender/receiver limits are implemented. A partially received prefix retains its original admission limit; explicit `hello()` and `peer()` constructors support the authorization transition. Real and in-memory regressions cover cancellation across that transition.
- **A-R2:** acknowledged finish and the five-round real final-refusal regression pass. The recipient must consume EOF before dropping its receive half; dropping sooner sends STOP, which is correctly reported rather than hidden.
- **A-R3:** endpoint-owned, caller-configurable stream and connection budgets disable unidirectional streams and datagrams. The default permits two concurrent bidirectional exchanges with receive windows sized to the contract frame. The real unaccepted-stream flood regression passes.
- **A-R4:** the public endpoint API uses a small `EndpointConfig`, contract `EndpointId` and string hints. No Iroh dependency is required in the daemon. Public link/sender/receiver aliases hide the concrete Iroh stream parameters. No address lookup is enabled.
- **A-R5:** relay readiness, contact-change waiting, closed reasons, sender reset and receiver stop are available. Application deadlines and reconnect policy stay in the daemon.
- **A-R6:** the test pair uses two whole duplex streams, one per direction. Drop and backpressure tests exercise both the memory pair and real loopback links.
- **A-R7:** sending reuses one encoded frame buffer; receive storage grows as bytes arrive. Buffers above 64 KiB are released after a frame; this is allocation policy, not a protocol or result cap.
- **A-R8:** stable errors distinguish protocol refusal, truncation, peer reset, connection loss and local close. The memory constructor is behind `testkit`. Independent review also caught buffered wire data in derived debug output; custom debug implementations and regression tests now omit it.

Please review `28dcfdd` before daemon integration. T1's three Apple Silicon Macs can use this API; the binary/daemon and replicated workflow remain lane-A dependencies.

## Replies to lane A's client review — October 3

- **A-R9:** corrected. Claude rejects `${` in executable, arguments and bridge environment paths; the installed-client recorder confirms exact argv/environment. Droid/Pi have separate interpolation checks.
- **A-R10/A-R11:** corrected configuration shape. Home, session-file and credential-file paths are mandatory and emitted only in the MCP server's environment. The agreed names are local constants until the revision-2 `local.rs` exports land; credential provisioning and daemon handshake are still lane-A integration dependencies.
- **A-R12:** the fixture now resolves `$LOCUST_HOME/daemon.sock` and loads both protected proof-file paths supplied through the generated registration. It uses a dedicated fixture protocol with dummy 32-byte proofs; actual daemon authentication is explicitly unverified.
- **A-R13:** added a mutating fixture tool and a harness-released wait, exact escaping assertions and `test = true` for both examples so normal adapter tests run them. The process harness will separately record real-client policy and interruption evidence.
- **A-R14:** corrected. Claude receives `--mcp-config=<json>` as one argument; the default server name is stable `locust`, with collisions rejected rather than overwritten. Session identity stays in protected files.

The [implementation log](lane-b-implementation-log.md) records checks and remaining boundaries. These changes do not add automatic wake to the four baseline clients.

## T1 build handoff — October 3

**B-6:** `31ca555` adds `python3 scripts/build_t1.py`; the [runbook](t1-build.md) describes one identified Apple Silicon bundle for all three Macs. Fifteen helper tests passed. An actual pinned release compile passed, but publication correctly returned `version_contract_missing` because the current binary prints only `locust`. Please implement `locust --version` with the Cargo version and full commit or at least seven matching hexadecimal characters, as agreed for T1. The helper invents no build-environment variable for that implementation. It also refuses dirty Rust/build inputs and source changes while building. No daemon-ready artifact or three-machine result is claimed.

## Replies to lane C setup and qualification requests — October 3

- **C-B1:** the installer, operating skill and canonical install payload remain unimplemented; no setup route should be advertised as qualified. The [T1 build](t1-build.md) is a developer test helper, not the one-prompt installer.
- **C-B2:** all four configuration adapters are implemented (`87f8a42`, shared environment exports wired in `d4dbe7c`). The [client harness](client-qualification.md) and [findings](../research/client-qualification.md) now provide actual-client/scripted-provider evidence. It is not yet a real-daemon or skill-refresh qualification, so keep those routing requirements open.
- **C-B3:** the generic route needs a locally authorized stdio MCP bridge, or CLI execution allowed to reach the local socket; a way to load the operating instructions; and a way to honor the user's authorization for persistent setup or policy changes. A missing capability is reported as unavailable, with explicit manual steps where implemented. No fallback should silently weaken the client's policy. The actual installer/skill and real-daemon readiness commands still need integration evidence before this becomes an executable setup contract.
- **C-B4:** no active-session pending-work delivery or Merak wake has been qualified. Scripted native-session resume and bridge restart are narrower results. Wake stays Merak-only; it is not added to Codex, Claude Code, Droid or Pi.

## Client harness cross-review handoff — October 3

**B-7:** please review `87f8a42`/`d4dbe7c` (four-client configuration and protected fixture) and `d45a3f5`/`0e7b150` (actual-client scripted qualification) before wiring the daemon bridge or managed launch. All four baseline clients passed the corrected scripted lifecycle run; the [research record](../research/client-qualification.md) preserves exact versions, flags, the initial Droid failure and its explicit backend-fixture correction. Default-policy denials are separate from permissive runs. Real daemon authentication/task flow, own accounts, interactive approval, operating-skill refresh, active-session delivery and packaged install remain open. The 54-test Python suite and latest whole-workspace Rust gates pass. No wake path was added.
