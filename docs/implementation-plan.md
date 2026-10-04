# Locust implementation plan

Updated: 2026-10-03. **Current execution: T2 MCP, operating skill and snapshot/contribution flow are implemented and locally verified; the owner is handling live client and physical-machine testing separately. See [T2 workflow](t2-workflow.md). Managed sessions and installation follow T2.**

**Prior T1 checkpoint:** persistent daemon, CLI and peer synchronization implemented; exact release candidate verified with three processes on one Mac. The physical-machine qualification uses the owner's two available Macs. The earlier October 4 release target is retained as planning history; the owner has since deferred publication. Neither that date nor this plan authorizes publication. The two-Mac first pass remains a separate qualification activity; three-peer checks and later release gates retain their separate evidence requirements. This document combines accepted design and remaining work; section 2 and the [release ledger](release-evidence.md) distinguish implementation from qualification.

## 1. Outcome and scope

Locust is an open-source protocol and small Rust daemon that lets independently operated coding agents collaborate on a common goal. Participants install it locally, retain control of execution and sandboxing, and exchange work and context without a mandatory vendor-operated coordinator or paid cloud message board.

It targets collaborators who run different coding agents on machines they control and currently relay tasks, context and patches by hand. The useful distinction to prove is collaboration without a common forge account or hosted board, while each participant retains authority over local execution and credentials. This is a product hypothesis to test in the first real-agent run, not a claim that existing tools cannot exchange work.

The first complete experience:

1. A person pastes an installation prompt into a supported coding agent.
2. The agent installs a verified Locust release and its operating skill, starts the daemon, and checks that it can use it.
3. The person creates a goal or accepts an invitation, explicitly choosing which workspace content to share.
4. Two agents on different machines synchronize the goal's board and context. A designated coordinator assigns a task against a specific input snapshot.
5. A worker accepts, executes in its participant-managed environment, and publishes a result and patch or artifact.
6. The coordinator reviews and accepts the contribution. A disconnected participant can reconnect and recover authorized missed state from reachable retained copies.

The first release must prove this entire flow with real coding agents and a real network connection between separate machines. Exercise real agents as soon as the first runnable task flow exists, while protocol and persistence work continues in parallel. A local deterministic worker is a companion test fixture with a different evidence boundary; it is not a prerequisite for trying the product with real agents.

### User requirements

- A protocol and open-source tool, with a small daemon written in Rust.
- Distributed agent orchestration and collaboration without a mandatory central service.
- Shared goal context, notes and files; participants can run on independently managed machines.
- Participants remain responsible for their execution sandbox and local credentials.
- Installation starts from a prompt pasted into a coding agent and includes an operating skill.
- First-release client baseline: Codex, Claude Code, Factory Droid and Pi, as specified by the owner on October 3. Each must qualify the common collaboration workflow; automatic wake is restricted to Merak for now and requires separate evidence.
- Learn from MoltMesh and other projects without copying their implementation wholesale.

### Design defaults and current decisions

| Area | Default for this plan | Rationale |
|---|---|---|
| Collaboration | Invitation-only goals | Makes authority and admission explicit before public discovery |
| Coordination | One designated coordinator authority per goal | Gives assignments and accepted results an unambiguous owner |
| Shared board | Durable signed events, replicated peer to peer | Supports offline work, replay and attributable history |
| Scratchpad | Versioned documents and revisions on the same event system | Preserves concurrent contributions without silent overwrites |
| Workspace | Immutable snapshots and patches; one local working copy per participant | Keeps synchronization separate from integration and execution |
| Transport | Iroh 1.3 with local multicast and Mainline endpoint lookup; n0 relays by default | Implemented; separate-machine and alternate-operator qualification remain open |
| Local persistence | SQLite for events, projections and durable outbound intent; separate immutable blob files | The event log may supply reconciliation intent; other delivery obligations need durable pending records |
| Agent integration | Structured CLI, portable skill and thin local stdio MCP bridge | Uses supported client interfaces without assuming sandboxed shell commands can reach daemon IPC |
| Client lifecycle integration | Implement Locust-owned Rust adapters, using hcom's client-integration patterns as design references | Own the launch, hook, session and delivery contract; no hcom binary, fork or source transplant |
| Initial execution | Active sessions and explicit resume for all four baseline clients; automatic wake only for Merak for now | Wait/resume and active-session delivery are qualified per client; Merak wake requires its own evidence |
| Initial release targets | macOS arm64 and Linux x86_64; Codex, Claude Code, Factory Droid and Pi local clients | Four-client baseline required by the owner; exact versions and platform coverage require qualification, not a current support claim |

The [version 1 contract](protocol-v1.md) and [workstreams](workstreams.md) record settled interfaces and ownership. Table entries describe the design, not universal implementation or support claims; section 2 records what exists. Windows, more architectures and Cursor remain later qualification targets. The historical candidate reports API 0/protocol 0; current remediation source uses API 1/protocol 1 and requires fresh goal state; follow the workstream rules for wire-format changes and golden vectors.

### Deferred scope

Public global agent discovery, global names/reputation, a marketplace, Byzantine consensus, automatic coordinator failover, a universal multiwriter filesystem, built-in model inference and a universal sandbox are outside the first release. Unattended agent runners, optional retention peers, A2A and deeper execution/sandbox integrations are follow-on capabilities. Pi participation, configuration and locally initiated session integration are part of the four-client baseline. The thin stdio MCP bridge is included in the first release. Locally initiated client launch and hook/session integration are a parallel implementation workstream; automatic session wake is restricted to a Merak-specific adapter for now, while additional MCP transports remain optional qualification work. Merak wake is not a fifth baseline-client requirement or an implemented capability; unattended closed-session startup remains deferred. Relay configuration remains part of the initial connectivity work.

## 2. Current repository and evidence

The runtime was integrated in `885b372`. The [application](../crates/locust/src/daemon/mod.rs) serves an authenticated Unix API backed by the core state machine, SQLite and Iroh. The CLI creates and joins goals, assigns and claims tasks, transfers sealed text, submits and accepts results, exchanges notes and recovers after restart. Seven [workspace crates](../Cargo.toml) separate the binary, protocol, core, store, network, client configuration and workspace components under the accepted [ownership rules](workstreams.md).

| Area | Implemented and retained evidence | Still needed |
|---|---|---|
| Runtime, protocol and persistence | Integrated daemon and CLI; the integration record reports 391 passing Rust tests and five explicit ignores | Complete gate coverage and physical-machine qualification |
| Peer task and recovery flow | Exact Apple Silicon candidate `3422c7b` passed 21 checks with three local processes, including coordinator outage and restart | Two physical Macs, observed routes and OS sleep/wake; third-peer extension separately |
| Coding clients | Configuration generation for Codex, Claude Code, Droid and Pi; actual client binaries exercised with scripted providers and a fixture MCP server | Production `locust mcp`, operating skill, real-daemon/model task flow and managed client lifecycle |
| Workspace | Git snapshot export and safe materialization libraries | CLI/API integration of that workflow and manifest-bound patches |
| Later runtime features | Document revisions, member removal/key rotation and multi-chunk transfer have implementation and component tests | Their complete workflow and release qualification; inclusion in core tests does not close those gates |
| Delivery | Identified local arm64 candidate; local website and entry guide | Installer, signed platform artifacts and public-download qualification; publication is deferred |

The [integration findings](../research/t1-integration-2026-10-03.md), [candidate identity](t1-build.md) and [release ledger](release-evidence.md) retain exact evidence and limitations. Multicast-only discovery failed on the development host; daemon-default lookup and relay configuration passed the same-host workflow. No complete release gate is recorded as passed. The [first-contact document](first-contact.md) describes the target journey and an older source snapshot, not current daemon readiness.

**Immediate owner update:** start with two Macs, using the same verified binary and independent state directories. Run joining, task acceptance, offline catch-up, sequential restart and sleep/wake via the [T1 guide](t1-run.md). A third daemon can later run on either Mac to test two surviving peers exchanging while the coordinator is offline; record that topology as three daemons on two machines. A third physical Mac is not a prerequisite for the first pass. No two-peer run establishes that three-peer behavior.

The [landscape survey](../research/landscape.md) evaluates Iroh, rust-libp2p, OpenDHT, p2panda, Willow, Radicle, A2A and MCP. The MoltMesh review provides concrete lessons from its [architecture/consensus](../research/moltmesh-architecture-and-consensus.md), [network/storage](../research/moltmesh-networking-and-storage.md), [task/SDK](../research/moltmesh-tasks-and-sdk.md) and [security](../research/moltmesh-security.md) paths. The [validation report](../research/moltmesh-validation.md) separates executed tests from source review.

Useful MoltMesh ideas are the daemon/API boundary, durable queues, explicit task records and immutable artifact references. Its reproduced task/restart/release failures and source-established recovery/authorization gaps become Locust test cases. Its local multi-process demo is positive evidence for that implementation's happy path; it is not validation of Locust or of WAN/Byzantine behavior. The earlier [implementation proposal](../research/locust-implementation-proposal.md) remains research background; this document adds the execution sequence and installation contract.

The [second independent review](../research/implementation-plan-independent-review.md) motivates the client, recovery and release refinements below. The [review response](implementation-plan-review-response.md) records what was incorporated, qualified or rejected. Reviewer-reported probes are not Locust release evidence; retain exact commands and raw results when qualifying the implementation.

The [hcom assessment](../research/hcom-dissection.md) informs the client lifecycle workstream. **Accepted direction, October 3:** implement these ideas directly in Locust's Rust code, rather than adopt hcom as a runtime or fork. Its source and [characterization results](../research/evidence/hcom-validation.md) are references for behavior and failure cases, not implementation or verification of Locust adapters.

## 3. System shape

```mermaid
flowchart LR
    A[Coding agent] --> S[Locust skill]
    S --> C[Locust CLI]
    C --> API[Authenticated local API]
    A --> M[Locust stdio MCP bridge]
    M --> API
    API --> D[Local Rust daemon]
    D --> DB[SQLite events, views and pending delivery]
    D --> B[Encrypted immutable blob store]
    D <-->|Reconcile events and fetch blobs| P[Peer daemons]
    D -. Rendezvous and relayed transport .-> R[Replaceable relay]
    O[Local participant] -->|Authorized launch or resume| L[Locust client adapter]
    D -. Pending IDs and status .-> L
    L -->|Client lifecycle and qualified wake| A
    A --> W[Participant-managed sandbox and worktree]
    W -->|Explicit export or result submission| C
```

The peer-to-peer layer moves durable records and blobs between machines. The coordination board is a local queryable view of those records. The shared scratchpad is another view. They share replication infrastructure but have different state-transition rules.

The agent plans, negotiates and reviews. The daemon checks authority, validates transitions, retains state and transfers data. A remote task is an offer to an enrolled participant; it is never a direct instruction to the daemon to execute arbitrary shell commands.

Use one installed `locust` executable initially, with daemon, CLI and stdio MCP subcommands. Each MCP bridge connects to the existing daemon; closing a client does not discard daemon identity or history. Its code is split into one library crate per workstream behind a shared contract crate, so concurrent work in one checkout cannot break unrelated builds; see [crates and workstreams](workstreams.md) and the [version 1 contract](protocol-v1.md). Do not add a web UI, actor framework, CRDT framework or consensus engine merely to complete the diagram. A CLI board view and structured queries are sufficient for the initial product.

Implement the local client adapter in that same Rust application, outside the peer-ingress path. It composes client launch, configuration, hooks and session observation; it does not add another collaboration store or protocol. A participant-selected supervisor must cover the managed process tree and hooks if whole-worker confinement is claimed. Existing sessions can use CLI/MCP and explicit resume without being launched by Locust.

Without a retention peer, new state crosses between two participants only while their daemons can communicate. Display last successful synchronization and pending/unknown remote state; an offline agent session differs from an offline daemon. Name the selected relay/discovery operators and addressing metadata they observe. Iroh may use a relay for initial rendezvous before switching to a direct connection, so a relay is not merely an exceptional fallback. Carry usable relay/contact hints in invitations and test replacement.

**Owner direction, October 3: peer discovery works like BitTorrent.** A participant should not need to know or care which network a peer is on. A goal behaves like a private swarm: an invitation plays the part of a magnet link, naming the goal and the inviter by key; members learn each other's endpoint keys from the signed membership decisions; each daemon publishes its current addresses under its endpoint key and looks peers up by key on the BitTorrent Mainline DHT, and finds peers on the same local network by multicast discovery; any member synchronizes with any other member it can reach, not only with the coordinator. Contact hints in an invitation are an accelerator, never a requirement. Iroh ships both lookups as separate crates (`iroh-mainline-address-lookup`, `iroh-mdns-address-lookup`); the daemon now wires both as the transport default in its [network configuration](../crates/locust/src/daemon/network.rs). Three things remain unlike a public torrent and are stated to users rather than hidden: the DHT itself is reached through bootstrap nodes, as BitTorrent's is; two peers that are both behind address translation still need a third party to help them connect, which here is a replaceable relay; and a goal is private, so only invited members are admitted and content is sealed. Publishing an address under an endpoint key reveals that key's current addresses to anyone who knows the key, as joining a swarm does. [Iroh connection and relay behavior](https://docs.rs/iroh/1.3.0/iroh/).

## 4. Protocol and coordination contract

### Identity, invitation and admission

A goal has an immutable signed genesis record, whose digest identifies it and pins its initial owner/coordinator authority. Transport keys identify endpoints. Agent identities identify enrolled principals; friendly names are labels and cannot reassign key ownership.

For the initial local trust model, the daemon is a trusted signing delegate for its enrolled agents. It holds signing keys outside shared workspaces and issues scoped local credentials to agent clients. An event identifies the agent and its delegation. This proves the authorized signer/delegate, not that a particular model or human generated the text. Independently held agent keys can be added later if required.

Owner administration and agent operations use distinct credentials and permissions. A missing agent credential must never select an administrator or global namespace. Every local API operation and event-ingress path goes through the same authorization rules. Strong isolation from another process running as the same OS user still depends on the participant's sandbox and filesystem permissions; the daemon cannot manufacture that isolation on its own.

Retain scoped client credentials for least privilege and attribution; do not present them as protection against another process able to steal those credentials. Every goal-scoped operation, record and idempotency key explicitly binds its goal ID. Omission is an error, never an implicit current goal. Global lifecycle/enrollment operations are separately typed.

An invitation binds the goal/genesis, expected recipient or redeemable admission capability, assigned rights, issuer-selected expiry and contact hints. Redemption is single-use and binds the joining key; retrying the same request recovers its result, while another key is refused. Display peer key fingerprints and destination before joining. Receipt alone does not enroll a machine, share files or start work. After acceptance, the coordinator records membership in its decision chain and distributes the appropriate encrypted content keys. Possessing a peer address, topic name or artifact hash is insufficient authorization.

The M0 permission matrix names both the protocol signer and the local approval owner:

| Action | Local authority and protocol role |
|---|---|
| Share scope, invite or join | The participant authorizes the specific scope; the agent may act within an existing grant; the daemon validates enrollment/membership |
| Offer/assign work | A member may propose a task; the coordinator signs assignment, initially to a named assignee |
| Execute an assignment | The worker's local participant authorizes execution per task or through an explicit standing goal policy; coordinator assignment is not local consent |
| Accept a result/head | The coordinator principal decides under its locally authorized review policy; the daemon validates the decision |
| Apply a patch to a local repository | The repository's local participant authorizes integration; protocol acceptance alone is insufficient |

Existing explicit authorization persists; do not add a repeated confirmation for an action already covered. Without a standing execution grant, newly assigned work remains pending local authorization. Peer-authored task/context text remains attributed data and cannot widen host permissions.

### Two kinds of durable history

**Contribution events** include messages, observations, scratchpad proposals, progress and result submissions. They are signed, deduplicated, attributable and causally linked. They can be created while disconnected and reconciled later. Their arrival order is not a global order.

**Coordinator decisions** include membership changes, assignments/reassignments, accepted results and accepted document/workspace heads. They form one signed hash-linked sequence per goal, with a previous-decision reference and explicit preconditions. Only the current coordinator authority can advance it. Enforce a single writer locally and detect conflicting signed successors; conflicting authority histories halt authoritative projection and require explicit resolution rather than choosing the newest timestamp.

The coordinator agent proposes decisions through its scoped API; deterministic daemon rules validate them. The authority can live on any participating machine. Initial operation uses one coordinator key with no automatic leader election or timeout-based takeover. Losing it pauses authoritative changes; participants retain and can export their histories. A new goal fork must be identified as a new goal, not represented as recovered continuity. Planned handoff/key recovery requires a later explicit protocol and tests.

An exclusive local store lock prevents concurrent writers to one state directory; it does not protect copied directories on other machines. Supported restore/rollback operations enter read-only recovery: recover the complete own signed tail through an independently retained pre-restore signing watermark or an intact current signing store, and require the operator to retire the former writer before resuming. Agreement among reachable peers alone cannot rule out a lost, never-replicated tail. If continuity cannot be established, remain read-only for that signer/goal or create an explicitly new goal. Local checkpoints cannot detect every unmanaged copy; raw duplicated signing state is unsupported, and discovered equivocation is detected rather than retrospectively prevented. Conflicting successors, or a validated supported decision that contradicts its predecessor's semantic state, halt authoritative advancement for the affected goal and retain the evidence; unrelated goals remain usable. First-release conflict recovery is inspection/export and an explicit new-goal fork. Missing dependencies remain pending, unsupported versions report incompatibility, and local I/O failure does not become a fabricated protocol fork.

| While coordinator is unavailable | Allowed behavior |
|---|---|
| Messages, notes, drafts and results | Retain and exchange authorized contributions |
| Previously assigned work | Continue within its known authorization and host policy; report uncertain/expired authority |
| New authoritative assignment, result acceptance, membership or accepted-head change | Remain pending until coordinator is available |
| Peer observes a timeout | Report it; do not create a new coordinator or final assignment |

### Event envelope and replay

Specify a versioned envelope containing goal ID, event type, author/delegation, per-author sequence and prior hash, causal parents, referenced coordinator decision/membership epoch, payload or ciphertext hash, artifact references and signature. Wall-clock time is diagnostic metadata, not an authorization or conflict-resolution oracle.

Before implementation, select one versioned encoding, hash/signature scheme and authenticated-encryption composition using maintained libraries. Sign and retain exact envelope bytes; verification must not depend on decoding and re-serializing them. Specify unambiguous field/length handling, domain separation, the signed region and event-ID derivation; reject ambiguous encodings such as duplicate fields. Publish byte/signature test vectors and document algorithm/version negotiation. This narrows encoding work without removing its contract. Detect duplicate events by authenticated identifier and author equivocation instead of replacing records with the same author/sequence.

Reconciliation exchanges known frontiers/heads, retrieves missing ancestors and objects, validates them, and rebuilds deterministic local views. Unknown dependencies stay pending; invalid records do not enter accepted projections. Gossip is an optional wakeup hint. Missed gossip must not lose history. Snapshots/checkpoints are authenticated optimizations with sufficient ancestry and authority proofs; they cannot replace validation with an unsigned advertised height.

For replicated events, the retained log may itself be the durable outbound intent if reconciliation can always rediscover missing records after a crash. Commit events, projection changes and request-digest idempotency together. Persist separate pending work for obligations not recoverable from that log, such as invitation redemption before membership. A separate per-peer outbox is an implementation choice, not an additional protocol guarantee.

Apply published framing limits and operator-configured storage/transfer admission policy before allocating or retaining peer input, including from members. Report rejections explicitly; do not silently truncate content or invent agent token/tool limits. Private-goal user payloads, including notes, use the declared encryption envelope; reserved event kinds cannot bypass it. Membership checks apply to every event/blob request, not just initial connection admission.

### Revocation and offline membership

Revocation is a coordinator decision and rotates future content access as required. A stale membership epoch or claimed creation timestamp does not prove an offline event predates revocation. Validate the referenced authorization chain, not just an epoch number. The initial rule should record the revoked author's last accepted frontier in the revocation decision. Contributions beyond that frontier remain unaccepted after revocation; any intentional recovery of legitimate offline work requires a current authorized decision rather than retroactive timestamp trust. Member removal supersedes its outstanding attempts' rights to finalize and emits cancellation requests; it does not assert their processes have stopped.

Disconnected peers may temporarily display stale membership and cannot instantly withdraw plaintext or stop a disconnected worker. Surface the last known decision head and synchronize before new authoritative acceptance. Already received plaintext cannot be recalled; future key distribution and finalization authority can be withdrawn. Test this policy before representing a goal as private and revocable.

Distinguish causal references from semantic dependencies. A valid but excluded event may remain as authorized historical evidence and satisfy an ancestry link; it cannot supply valid membership, assignment, accepted base or finalization authority. An honest event referencing an excluded event is not automatically invalid, but its own semantic preconditions must still hold. Quarantine invalid records and bound their retention; do not relay arbitrary rejected input as accepted history.

## 5. Tasks, scratchpad and workspace

### Task lifecycle

The board must distinguish task intent, an execution attempt, and acceptance of a result. Suggested visible flow:

```text
offered → assigned → worker_accepted → running → result_submitted → result_accepted
                         ↘ rejected / failed / cancel_requested → cancel_acknowledged
```

This is an explanatory flow, not the final state machine. Work package M0 must define the complete transition table, including retries, superseded attempts, result rejection and late messages. Each transition lists its allowed signer, prior-state precondition and side effects. In particular, a completed worker attempt does not automatically mean that the goal accepted its result.

An immutable task specification binds the requested outcome, named assignee or eligible role, input snapshot, expected outputs, dependencies and authored execution/retry/deadline policy. Worker acceptance binds the exact assignment hash, input snapshot and required capabilities and is persisted before execution. Omitted policy means a documented explicit default or no deadline; remote receivers must not silently replace supplied policy. Any retry limit or resource policy is operator/task-authored and visible, not an arbitrary hidden model/tool cap.

Keep task, assignment, attempt, worker-instance and claim-request roles distinct; an immutable event hash may serve as an identifier where unambiguous. The first-release interactive worker uses a durable instance-bound claim plus generation, with explicit authenticated recovery/takeover. It does not require the model to heartbeat during a long tool call. The daemon grants the claim atomically; retrying the same acquisition recovers its result, while the agent credential alone cannot recover another execution session's claim. Recovery requires session-bound proof; takeover requires separate explicit authorization. Taking over increments the generation and fences the previous token. Reattaching the original instance after a daemon restart must prove its claim, not silently create a second worker.

A worker instance is an enrolled execution-session handle, not the PID of each short-lived CLI invocation. The adapter/daemon persists the handle and protected claim state; repeated commands from that session use it without requiring the model to remember a secret token.

Reconstruct available/owned/recovery-pending work and outstanding cancellation from task state; feed cursors are not the only record that work exists. Persist consumer cursors in the daemon by default. An adapter may retain a durable cursor too, but no correctness state is delegated to model memory, and acknowledgment cannot erase an unclaimed task.

Coordinator reassignment creates a new attempt and supersedes the old assignment; local takeover cannot assign work on another daemon. Stale generations/attempts cannot finalize, but their artifacts remain recoverable for review. Persisted terminal results survive later ownership changes. Preserve authored deadlines/retry budgets across restart and reassignment. Optional expiring leases require a named resident adapter to renew them and an explicit recovery policy; do not introduce an implicit TTL for interactive sessions. Scheduling/acceptance deadlines belong to the coordinator and execution duration to the local executor. Persist authored absolute deadlines, re-evaluate them and reconnect after sleep/wake, and test clock jumps; no clock observation alone grants takeover or global exclusivity.

Cancellation is a durable request followed by an executor acknowledgment. While the worker is offline, report `cancel_requested`; do not claim execution stopped. Proposed race rule: a coordinator cancellation/supersession decision removes future finalization rights; a result accepted earlier in the decision chain remains accepted. Late results remain attributable, unaccepted contributions. An executor reports stopped, already completed or outcome uncertain. Expose cancellation durably through worker wait/status operations and revoke the corresponding daemon completion authority. The skill checks that state between operations. Delivering an immediate process stop signal depends on a supported client/runner integration; a generic skill cannot necessarily interrupt a coding agent's ongoing tool invocation. Completion of an external effect may remain uncertain.

Claims and leases protect daemon state; they cannot guarantee exactly-once arbitrary external effects. Use destination-supported idempotency/fencing or a separate explicitly authorized effect step. When the outcome is unknown and safe retry cannot be established, retain an uncertain outcome requiring reconciliation. The v1 demo should use inspectable code/artifact production rather than irreversible external actions.

### Shared scratchpad and useful context

Provide goal summary, plan, decisions, findings and per-task notes as addressable documents/views. Notes/findings are append-only contributions with explicit correction/supersession references. Replaceable shared plan/summary documents use revisions naming their base and content hash: concurrent edits remain separate proposals, and the coordinator advances an accepted revision with an expected-base precondition. No last-writer-wins overwrite of accepted documents based on clock time.

Agents query relevant tasks and documents, subscribe to relevant events and retrieve referenced artifacts. They should not need to load the full conversation history for every action. Summaries are derived, versioned artifacts that link back to their supporting events; they do not erase source evidence or confer authority. Text in a scratchpad such as “I claim task X” has no scheduling effect without the typed task transition.

### Artifact and workspace handling

Blobs are immutable and encrypted before peer publication when confidential. Manifests refer directly to immutable parents and content objects; historical traversal cannot depend on a fresh DHT record for every revision. Use a verified, resumable transfer implementation and test its actual persistence semantics.

The storage sequence is receive → verify → persist → durable acknowledgment. Blob files are durably installed before a database transaction publishes a retained reference. A crash can leave collectable unreferenced blobs; it must not leave an acknowledged reference pointing to bytes that were never durable. Pin objects while accepted history, pending submissions or explicit retention promises reference them.

The initial workspace representation is a reviewed file manifest plus immutable content objects, with patches bound to that manifest's digest. Default export selects files from one named Git commit under the participant's recorded export root, without Git history or repository metadata. The actual manifest digest identifies the shared snapshot; the source Git commit is provenance, not proof that every file was shared. Uncommitted files require explicit inclusion and a distinct manifest. Default-deny export selection excludes credentials, private caches and unrelated content; tracked status and ignore rules alone are not secrecy boundaries. Result patches/artifacts follow the same declared export scope.

The trusted CLI/adapter performs workspace I/O and streams selected bytes. The daemon reads/writes only its own state/object storage and never opens arbitrary client-supplied host paths. It validates scoped upload authority, manifests and object identities, but a byte upload cannot prove the original source path: export-root enforcement requires the trusted adapter or local sandbox, not a daemon claim about unseen files.

Materialize into an explicitly chosen separate destination. Initially support regular files, directories and executable bits; reject symlinks, hardlinks, special files and Git submodule entries with an explicit unsupported-content error. Do not silently fetch LFS/submodule content. Validate traversal, collisions and platform differences before writes. Use raw Git object reads with isolated configuration; do not run checkout filters, text conversions, hooks or received executable configuration during transfer/materialization. Authenticated repository instructions remain untrusted input when the participant later opens a coding session there. [Git object reads and filter options](https://git-scm.com/docs/git-cat-file).

Integration uses an expected accepted-head/base check and produces an inspectable diff. Report `submitted`, `accepted` and `integrated` separately. A combined accept-and-apply UX may reuse one explicit authorization, but its protocol decision and local Git update are separate recoverable operations, not an atomic SQLite/Git transaction. Conflicts preserve both contributions and dirty work; a failed apply must never report merged.

Expose artifact states separately: available locally, verified/retained, requested from peers, and acknowledged by a named retention peer. A successful fetch must remain available after its source disconnects according to the local retention policy. If every holder is offline, report unavailability. A receipt represents a peer's retention assertion and policy, not permanent guaranteed storage.

Keep detachable user content behind payload hashes; minimize content in signed structural headers. Define `leave` as stopping local participation/sync and requesting membership removal, without claiming peers learned it while disconnected or erased their copies. Local data purge is separate. An incident can stop sharing and withdraw detachable locally served user payloads while retaining the typed signed fields required for membership/assignment validation and deterministic projection replay. A payload hash alone cannot replace those structural records; missing user content is reported as unavailable. A coordinated redaction protocol is deferred and cannot promise erasure from other participants or backups.

## 6. Local API, client adapters and operating skill

Use an authenticated local IPC endpoint, with a Unix socket as the initial platform default. Keep it independent of the P2P transport. Do not expose an unauthenticated TCP administration endpoint for convenience. CLI, stdio MCP and future SDKs call the same authorization/state-transition implementation. The MCP bridge holds scoped agent authority and exposes typed collaboration operations, not generic host execution. Do not install a broad unsandboxed `locust` command exemption to work around a client's IPC restrictions.

Command status below is checked against the [CLI command tree](../crates/locust/src/cli/args.rs) and [typed request dispatch](../crates/locust/src/cli/mod.rs). Named commands and generic `locust call` access do not imply a complete end-user workflow:

| Area | Current surface and remaining work |
|---|---|
| Lifecycle | Implemented: `locust daemon run`, `locust daemon stop`, `locust status`, `locust doctor`; managed service installation remains open |
| Client bridge | Planned: `locust mcp` over stdio and any qualified client hooks |
| Managed client sessions | Protected session-file creation and local session API exist; client launch/readiness/resume integration remains open |
| Identity and enrollment | `agent enroll` plus typed local API operations through `call`; full lifecycle qualification remains open |
| Goals | Implemented named create, invite, join, status and leave commands |
| Board | Implemented `board`, `pending`, `events`, `event show`, `wait` and `task show` |
| Tasks | Named propose, assign, authorize, claim, takeover, progress, submit, cancel, decline and fail; result accept/reject; broader request surface through `call` |
| Scratchpad | Named `doc read`, `doc revise`, `doc accept` and note operations; complete workflow qualification remains later work |
| Artifacts/workspace | Blob API and snapshot libraries exist; no complete named snapshot/materialize/patch/apply CLI workflow |

Provide human-readable output plus versioned `--json` responses, stable error codes, request IDs and semantic result states. Keep a short documented happy path; JSON is an output format, not where uncommon operations are hidden. Never print credentials, invitation secrets or private keys in routine logs. Support an immediate pending-work query and wait-until-event with a caller-selected timeout, with distinct no-event, disconnected, denied, unsupported-version and corrupted-state outcomes. Adapter defaults must fit the tested client version's limits; there is no universal shortest-client timeout or hidden task-runtime cap. Persist/recover delivery state before checkpointing, independently of the model session.

Explicit resume is the supported baseline. Hooks may surface pending-work/cancellation IDs at qualified lifecycle points; their elevated output contains daemon-authored identifiers/status, not peer-written instructions. Hook polling is nonblocking and uses ordinary authorization. Idle wake, mid-tool interruption and closed-session activation are separate capabilities, never inferred from a successful wait or MCP handshake. The daemon enforces collaboration authority; the participant's adapter/runner enforces local execution policy. Automatic wake is scoped only to Merak for now; Codex, Claude Code, Factory Droid and Pi use active sessions and explicit resume. Deeper execution/sandbox extensions remain separate; baseline Pi participation is required. A supervised external client is another option. [Agent-agnostic integration](../research/agent-agnostic-integration.md).

The skill teaches enrollment and goal joining, context inspection, typed coordination operations, attempt recovery, progress reporting, patch/result submission and the distinction between completion and acceptance. Ship coordinator and worker playbooks with goal/task templates: the coordinator decomposes work and names review criteria; the worker claims the exact assignment, executes against its input and submits evidence; the coordinator reviews that evidence before acceptance. Exercise these playbooks in the first real-agent run. The skill can include small invocation helpers and protocol examples. It must treat peer content as untrusted task data and use the host's existing permission/sandbox controls. Skill prose does not enforce security. The daemon enforces collaboration permissions; the selected client or runner enforces local execution restrictions.

The [Agent Skills specification](https://agentskills.io/specification) supports instructions and optional scripts. The CLI and stdio MCP bridge supply executable operations; the skill teaches their use. Qualify the bridge early on isolated default configurations for all four baseline clients, testing actual daemon reachability and each client's permission behavior. [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli), [Claude Code MCP](https://code.claude.com/docs/en/mcp).

### Locust-owned client lifecycle adapters

**Accepted direction; partially implemented:** [client configuration](../crates/locust-adapter/src/config.rs) exists for the four baseline clients, and the core serves local session records. Locust-managed client launch, readiness, delivery and recovery remain unfinished. Implement those in Rust inside Locust, using the [hcom assessment](../research/hcom-dissection.md) as a behavioral reference. Do not depend on an hcom executable, maintain an hcom fork, or transplant its modules. Keep the existing daemon, authenticated API and task state as the only collaboration authority.

The required baseline is Codex, Claude Code, Factory Droid and Pi. Keep common lifecycle logic separate from each client's argument, configuration and event formats. Introduce only the small modules and types those implementations actually share; do not prebuild a generic plugin framework. A client-native shim may be added where an observed integration requirement needs it, while Rust retains the common lifecycle and daemon logic. Automatic wake is outside these four adapters' current scope and is reserved for Merak. Optional execution/sandbox extensions qualify separately; their absence does not remove a client from baseline task-flow and explicit-resume testing. The [client matrix](release-evidence.md) tracks implementation and evidence per client.

| Component | Locust responsibility and boundary |
|---|---|
| Per-run configuration | Build explicit argv/environment and a reviewable configuration overlay for the selected workspace, Locust skill/MCP setup and participant-selected permissions/authentication. Preserve unrelated settings and refuse an invalid required policy; never silently add trust, writable roots, network access or auto-approval. |
| Launch and readiness | Persist a local launch intent before spawning. Distinguish process started, client initialized, tools/hooks ready, blocked on user action and exited. Do not infer readiness from a process existing or a prose startup line. |
| Session binding | Persist worker/attempt/generation to local launch, process and client session/actor mapping in Locust's local operational state. Handle new/resume/fork explicitly. A session identifier or actor label is not a scoped credential or claim-recovery proof. |
| Notifications | Use qualified hooks or normal tools for active-session delivery. Codex, Claude Code, Factory Droid and Pi retain explicit resume; do not implement automatic wake or PTY input injection for them in this scope. Merak is the only automatic-wake target for now, through a separately qualified native adapter. Deliver daemon-authored pending-work/cancellation IDs and status; retrieve attributed peer content through ordinary scoped tools. |
| Recovery and cancellation | Reconcile launch/session state after interruption, recover pending work from durable task state and deduplicate notifications. Keep cancellation requested until the executor acknowledges its outcome; messaging shutdown, idle status and process-exit observation have different meanings. |
| Diagnostics and capabilities | Report exact client/version, effective profile and support for tool access, active-session delivery, idle wake, manual resume, session recovery and confinement independently. Retain redacted task/event/session correlations without credentials or private transcripts by default. |

Only local participant authorization may initiate or resume a managed client. Remote assignment remains a task offer; it cannot launch or kill a local process. Existing explicit local grants persist. A manually launched managed session does not establish unattended activation when it exits. Do not claim native wake can attach to an arbitrary existing terminal or Codex desktop chat; unmanaged sessions keep the CLI/MCP and explicit-resume path.

Treat launch as a recoverable side effect. A unique launch identifier alone does not prevent duplication: if Locust crashes after spawning but before recording the returned session, reconcile the persisted intent against owned process/session evidence before retrying. If the outcome cannot be established, report it as unknown and require local resolution rather than automatically creating a second worker. Validate attempt generation and scoped authority again when a recovered session claims or submits work.

Keep wake issued, notification emitted/enqueued, attempt durably claimed, result submitted and result accepted as separate states. A hook flush or plugin enqueue does not prove the model saw the work or took ownership. Client exit after delivery, lost/duplicate/late wake, expired credentials and stale bindings must leave unclaimed work recoverable through the daemon. Hook callbacks stay nonblocking; a missed wake cannot be the only record of pending work.

The [client qualification harness](client-qualification.md) already exercises actual client binaries in isolated profiles with scripted providers and an independently written fixture MCP server. Extend it to the production daemon and bridge as those paths become available. Test launch/delivery/resume/cancellation through actual client binaries, then run separate real-model task flows and packaged installation checks. hcom's passing lifecycle cases and intermittent Claude approval-resume failure identify scenarios to exercise; they do not qualify Locust. Default-permission tests must be distinct from deliberately permissive lifecycle tests. Test automatic wake only in the Merak-specific path for now, and claim it only for the exact version/profile where it passes. The four baseline clients require active-session and explicit-resume evidence.

### First-session contract

Installation reports what is usable now and whether a new session/reload is needed. Goal creation records export root, selected manifest, source commit and sharing authority. The inviter passes a one-use invitation through a channel they choose. Joining shows the inviter's key, destination directory, incoming content, local client/account use and the execution policy; it does not reuse an arbitrary install-session directory or authorize broader sharing. An existing standing grant covers matching tasks; otherwise work remains pending authorization. The worker reports the exact base, patch and test evidence; the coordinator reviews it. The final status names the accepted snapshot and, separately, the local branch/worktree actually updated. Exercise this full transcript with both client roles before treating onboarding as complete.

**A2A assessment (2026-10-03):** keep A2A as an optional integration for a concrete external agent or client, with no dependency or compatibility claim in the first release. It supplies a useful standard service interface and Rust SDK, but Locust still needs its durable peer history, coordinator authority and artifact-retention contract. A future adapter must use the existing task engine and distinguish external completion from coordinator acceptance. [Research and primary sources](../research/a2a-assessment.md).

## 7. One-prompt installation

Make the pasteable prompt the primary onboarding entry point. Name the official download origin, binary/skill/MCP configuration destinations and selected user-service mechanism in the prompt so the user can authorize the actual changes. Direct the coding agent to a versioned manifest and deterministic installer, with a supported-environment check. Pin the installation transaction to one release; do not let the model invent build/download/configuration steps or disable its permission policy.

The initial prompt/release page is obtained from the official configured HTTPS origin and supplies the versioned bootstrap location and expected digest. Verify the downloaded installer before executing it. The initial distribution key is trusted through that publisher/origin, then pinned for artifact checks and subsequent updates; document key rotation separately. This does not claim independence from a compromised initial publisher. Never bootstrap from a URL or replacement verification key supplied by an untrusted peer.

The installer must:

1. Detect OS/architecture, whether the environment is local or a remote/ephemeral workspace, and the invoking supported client. Explain where the daemon will actually live.
2. Resolve a compatible release and verify signed release metadata and artifact digests using an explicit distribution trust root. Fetch installation content from the configured trusted release source, not peer-supplied board instructions.
3. Install the user-owned binary, create restrictive state directories, and establish or reuse identity without silently replacing it.
4. Install the Locust skill and scoped stdio MCP configuration for the invoking client while preserving unrelated skills and configuration. Client-specific directories, approvals and reload behavior belong in adapters, not the P2P protocol.
5. Start the daemon using a supported user-session service mechanism, or explicitly report a session-only mode. Avoid duplicate processes and unnecessary administrator privileges.
6. Run `doctor` and a harmless real API roundtrip through the invoking client's intended CLI/MCP path. Check binary/API/skill compatibility and actual tool usability; report denied setup or pending reload accurately.
7. Report separately: binary installed, daemon running, skill discoverable in this/new session, and participation ready. Installation alone neither enrolls in a remote goal nor exports local files.

The prompt should be short and stable; the installer implements the actual work. Publish a manual installation path using the same artifacts. Support repeat installation, interrupted-install recovery, upgrade and uninstall. Preserve identity and goal data across normal upgrades; uninstall distinguishes removing software from an explicit data purge. Schema migrations require a recoverable pre-migration state; rollback must not reuse an older binary against an incompatible migrated database or resume signing from an old frontier after newer events were issued. Each participant authenticates their own coding client; the Locust daemon does not receive or distribute provider/account credentials. Record authentication mode, never secrets, in qualification evidence.

Test client reload requirements instead of claiming they are uniform. Current references include [Claude Code skill loading](https://code.claude.com/docs/en/skills#edit-a-skill-during-a-session) and [Cursor skills](https://cursor.com/docs/skills). Verify each supported client's exact version and paths during implementation. A newly installed CLI can be usable immediately even if native skill/tool discovery requires a reload.

### Installation does not establish an unattended worker

The daemon can stay online to retain, synchronize and receive work while no model session runs. The first release requires an active coding agent to join a goal and query/wait for work, with explicit resume when the session ends. The participant may start that session directly or through Locust's locally authorized client adapter. Some clients support pushing events into open sessions; that is an adapter-specific capability, not a protocol guarantee.

A later opt-in unattended runner may activate a supported headless client when no session is running, under an explicit local standing grant and operator-selected credentials, workspace, sandbox and spending policy. Reuse the local adapter's launch/session/recovery contract rather than add another execution authority. Closed-session activation and cross-client session resume require separate qualification from delivery into an already active client. [Claude Code headless interface](https://code.claude.com/docs/en/headless), [Cursor headless interface](https://cursor.com/docs/cli/headless).

## 8. Execution order and release gates

The [earlier review](implementation-plan-review.md) correctly identifies the risk of leaving the first real-agent workflow until the end. Its multi-quarter estimate is unsupported and is not a planning input. The original release target was October 4 at night. Publication is now deferred by the owner; the next physical test uses the two available Macs. Keep the core daemon, P2P board and context, workspace contributions, CLI/stdio bridge and one-prompt skill installation in scope. The deferred capabilities in section 1 remain deferred.

M0–M6 below identify work packages and their acceptance evidence, not seven sequential phases. Begin independent work once the relevant interface fixtures exist; integrate running slices throughout. Tests, skill writing and packaging proceed alongside implementation. The criteria remain acceptance requirements: some have component or local-process evidence, but no complete release gate has passed. Use section 2 and the ledger for current status. Land small reviewed commits and link retained evidence and resulting decisions from `docs/`.

The repository owner decides release go/no-go and any change to required scope or support claims. Agents fix and report failed gates; they cannot waive them or substitute the short checklist for the full M0–M6 criteria. If a required gate remains red, report the exact failure and candidate limitation for that decision; do not label an unverified candidate as the planned release. No ordered automatic scope-cut list is adopted.

Use the single [release evidence ledger](release-evidence.md) for gate status, candidate commit/artifact hash, evidence level, client/environment configuration, exact command, raw artifact, observer and unresolved failure. Evidence levels are source review, deterministic/component runtime, multiprocess local, real-client/network, packaged install and public-artifact verification. Passing one level does not imply the next. A reviewed implementation change has a scoped diff/evidence review by another agent or person and the required checks on the integrated tree.

### Parallel workstreams

| Stream | Work and initial handoff | Integration responsibility |
|---|---|---|
| Daemon and coordination | M1/M3: local IPC, SQLite event/pending-intent transactions, authority validation, tasks/claims and scratchpad projections; publish command/event fixtures first | One state-transition implementation used by local commands and incoming peer events |
| P2P and artifacts | M0/M2 plus blob handling in M4: stack selection, identity/invites, authorized replication, encryption and retained transfers | Consume the shared event/authority contract; exercise two-machine connectivity immediately |
| Agent workflow and workspace | M1/M3/M4/M6: CLI/stdio MCP flow, coordinator/worker playbooks, wait/resume behavior, snapshots, patches and review | Run real Codex, Claude Code, Factory Droid and Pi sessions under clean default profiles as soon as the executable task path exists; feed failures directly back to other streams |
| Client lifecycle integration | M1/M3/M5/M6: Locust-owned Rust launch/configuration, session binding, active-session hooks, recovery and diagnostics; Merak-only wake qualification; publish capability and lifecycle fixtures | Use the existing daemon API and local operational store; test actual binaries for all four baseline clients with independently written scripted-provider fixtures where their provider interfaces permit them, recording unavailable test paths explicitly; preserve the unmanaged CLI/MCP path |
| Installation and release | M5: build artifacts, installer, skill adapters, service lifecycle and clean-machine checks | Start from development artifacts; replace them with the exact release candidate and verify every claimed platform/client |

Module/file ownership should be explicit before concurrent edits. Shared event types, command responses and storage interfaces have one integration owner; other streams propose changes through that owner. Streams share one checkout and commit only their own paths, following the [workstream rules](workstreams.md); verify the integrated tree, preserving unrelated work. Select maintained transport, storage and cryptographic components against the required behavior. Evaluate whether a collaboration library also supplies useful replication/persistence pieces; reuse where its demonstrated semantics fit, without assuming either adoption or a custom implementation is inherently faster.

### Current checkpoints (supersedes the original calendar sequence)

| Order | Required integrated result | Evidence to retain |
|---|---|---|
| Completed local foundation | Integrated daemon, CLI, core, store and peer transport; exact candidate passes the three-process workflow | Existing A-C1/A-C2 records in the release ledger |
| Now: T1 on two Macs | Copy the same identified candidate, create independent identities, join, complete an assigned task, exchange notes, stop/restart each daemon, recover offline writes and test OS sleep/wake | Hash/version and OS per Mac, event IDs, observed routes, failure and recovery records; no physical run recorded yet |
| Three-peer extension | Add a third daemon with an independent home, on either available Mac or a later third Mac; verify passive history retention and surviving-peer exchange with the coordinator offline | Exact process/host topology; three daemons on two hosts is not three physical Macs |
| Next: T2 | Production MCP bridge and skill, workspace snapshot and patch, a real code task with a mixed-client pair, wait/interruption/explicit resume; then all four baseline clients | Actual client/profile/account modes, artifact/event IDs and recovery results |
| Later release qualification | Complete outstanding runtime, lifecycle, signed packaging, installation and independent-collaborator gates | Exact artifact/platform/client matrix and open failures |
| Publication: deferred | Revisit only when requested by the owner and after go/no-go | Public-artifact verification against the exact tested candidate |

These are execution priorities, not passing records. The two-Mac first pass replaces waiting for a third physical laptop; it does not waive three-instance fault tests or the four-client baseline. Deterministic tests continue alongside integrated work. A failed required check is recorded and fixed; an unverified behavior is never presented as supported.

### M0 — Protocol contract and transport proof

**Deliverables:** a short versioned protocol specification, transition/permission matrix naming approval owners, exact-byte event/invitation fixtures, and a disposable two-peer Rust connectivity experiment. Decide encoding/crypto composition, membership cutoff, claim recovery, goal scoping and CLI error semantics. Publish each settled interface immediately; completing every experiment is not a prerequisite for starting the daemon, skill or packaging. Evaluate Iroh transport and its blob/storage options separately, at pinned compatible versions, including direct unauthorized fetch/push and crash-durability tests. A local file store alone does not provide authorized resumable transfer. Compare rust-libp2p or a different blob layer when a candidate fails its gate. Include collaboration-layer reuse against offline reconciliation, authorization and restart requirements; do not select unrelated latest crate versions independently.

**Exit evidence:** two machines on separate home/mobile networks connect using invitations; direct and forced relay paths are visible; an alternate independently operated relay works; known peers reconnect when a discovery/bootstrap service is unavailable where configured routes permit it. Record the default relay/discovery operator, addressing metadata exposure and replacement configuration; do not promise every pair connects directly. Record binary size, idle memory/CPU/network use, startup latency and transfer memory as initial measurements. Select dependency versions and record the rationale and unresolved upstream warnings.

**Also required:** review the task/coordinator state tables with examples of duplicate, delayed, reordered, revoked and conflicting input. Publish test vectors before implementing multiple independent serializers. No formal BFT claim is needed or intended.

### M1 — Durable local daemon, CLI and stdio bridge

**Interface dependency:** M0 event/API contracts. **Deliverables:** daemon lifecycle, local authenticated IPC, identity/client enrollment, SQLite migrations, durable event append, deterministic projections, recoverable outbound intent, CLI query/doctor operations and thin stdio MCP bridge. Expose the path early for the agent-workflow stream. Start installer/skill/MCP configuration against development artifacts without representing them as a public release.

**Exit evidence:** local CLI and MCP roundtrips work; unauthorized/missing credentials and wrong-goal operations are rejected; request-digest idempotency works; state and recoverable outbound intent commit together; restart rebuilds the same projection without duplicate application; cursors cannot lose work; process shutdown/concurrent startup and restored-state signing restrictions behave correctly. Crash-inject before/after durable acknowledgments. Add matching macOS CI alongside Linux and a release-build fresh-database smoke as the runtime lands; do not treat CI as packaged-install or WAN proof.

In parallel, implement the first Locust-owned client launch/configuration and session-binding path, then cover the remaining three baseline clients and extract demonstrated common behavior. An early two-client slice is useful integration evidence but does not complete the four-client baseline. Define lifecycle observations and capability reporting separately from task transitions. Extend the existing [real-client qualification harness](client-qualification.md) from its fixture MCP server to the production daemon, retaining isolated profiles and a localhost scripted provider, informed by [hcom's test design](../research/hcom-dissection.md). Verify actual CLI/MCP access and readiness, unchanged permission policy/configuration, blocked authentication/approval states, and crash recovery around launch. These are Locust implementations and tests, with no hcom runtime or copied fixtures. Separate default-permission tests from permissive lifecycle tests and real-model collaboration; none substitutes for the others.

### M2 — Private peer replication and retained artifacts

**Interface dependencies:** M0 transport/event selection and M1 append/projection interface; develop concurrently with M1. **Deliverables:** signed genesis/invites, the generic coordinator decision chain with membership/revocation validation, accepted membership, encrypted content/key distribution, peer session authorization, missing-event reconciliation, blob verification/retention and route/sync diagnostics. Persist enough peer/contact and immutable-root information to recover without an ephemeral DHT history index. Mainline DHT endpoint lookup is implemented; it locates known peer keys, not public goals or arbitrary agents. Global agent discovery remains deferred.

**Exit evidence:** disconnect/reconnect recovers missed events; all-peer restart preserves state and restores reachability; duplicate/reordered events and missing ancestors converge; bogus author/genesis and invalid signatures are rejected. Direct event-range and blob-by-hash requests from non-members/revoked peers are refused; private-goal plaintext/reserved-event bypasses and unauthorized pushes are rejected. Exercise a member missing a key change and three-identity revoked-ancestor cases. Fetch a blob after its original source disconnects from the retained replica. Resume interrupted transfer; detect corruption, configured admission-limit violations and failed disk writes before durability acknowledgment.

### M3 — Board, coordinator and active workers

**Interface dependencies:** M0 transition contract and M1 local API/projection interface; private remote operation integrates with M2. **Deliverables:** task and accepted-head decisions, typed task lifecycle, durable claims/generations/attempts, result acceptance, cancellation, board/event queries and scratchpad contributions/revisions. Build a deterministic worker harness alongside the first real-agent workflow so protocol failures can be isolated without postponing product testing.

**Exit evidence:** one coordinator and two worker identities on three local daemon instances complete a synthetic goal; the agent credential alone cannot recover another session's claim; long tool calls and restart do not lose work; authenticated recovery is idempotent and superseded tokens cannot finalize. Authored deadlines/retry policy survive transfer; cancellation remains visible until acknowledged; result delivery survives acknowledgment loss. With the coordinator disconnected, the two workers exchange contributions while authoritative changes remain pending. Conflicting shared-document revisions stay inspectable; authority conflicts halt only the affected goal. The two-machine public scenario does not replace these three-instance tests.

Connect the client adapters to this same task flow: persist attempt/session mappings, deliver pending IDs through qualified hooks, and verify restart/rebind, duplicate or lost notifications, launch uncertainty and cancellation races. Test that peer assignment never invokes launch and that native session identifiers cannot replace claim proofs. Exercise idle wake separately only for Merak, including approval prompts and active user input; the four baseline clients use explicit resume.

### M4 — Coding workspace contributions

**Interface dependencies:** M3 task/result types and M2 blob interface; local snapshot/patch operations can develop concurrently. **Deliverables:** explicit manifest export, base-bound task input, separate participant workspace materialization, patch/artifact submission, reviewable integration and accepted workspace-head updates. Define retention/garbage collection, local payload withdrawal and leave behavior with pinned metadata and active-transfer protection.

**Exit evidence:** two workers modify the same base without overwriting each other; stale patches cannot advance the accepted head; conflicts preserve both outputs and dirty work. Test out-of-scope export, unsafe/unsupported paths and file types, executable content, case collisions, Git filter/hook execution, interrupted materialization and missing/withdrawn payloads. Recovery distinguishes accepted from integrated. Neither peer messages nor received repository instructions cause automatic execution by the daemon.

### M5 — Release packaging and one-prompt onboarding

**Begins:** with the initial command/install contract and development artifacts. **Public qualification depends on:** integrated M1–M4. **Deliverables:** signed artifacts/manifest, installer, service/skill/MCP adapters, prompt, upgrade/uninstall behavior and concise troubleshooting. Assign release location/signing-key custody and the owner's license choice as release prerequisites. Specify log locations, a redacted diagnostic bundle joined by task/event IDs, and signed withdrawn-version metadata; do not install a withdrawn release by default. Website release information must refer to real artifacts.

**Exit evidence:** clean installs on every claimed OS/architecture/client using isolated default profiles, preserving the owner's settings; record exact permission/authentication modes and necessary opt-ins. Repeat install preserves identity/configuration/services; the agent performs real operations through its configured CLI or MCP path and a new session discovers the skill. Exercise task flow, host timeout/interruption, cancellation, explicit resume and persisted delivery recovery. Qualify Locust-managed launch/session behavior separately from existing-session CLI/MCP use, and record active-session hook results per client/profile and automatic-wake results only for Merak. Test denied permissions, reload, interrupted installation, failed service start, restore-safe migration and uninstall preservation, including Locust-owned hook/configuration artifacts without deleting unrelated settings. Each packaged binary must write/read/restart on a fresh database; `--version` is insufficient.

Do not advertise “any coding agent” from one successful install. Publish OS/architecture, client/version, permission/authentication mode, transport, active-session/wake behavior and reload requirements. Generic shell agents may use the CLI where permitted but are not automatically native-skill-compatible or exempt from their sandbox.

Ship Locust's own adapter code in the Locust artifact. Coding clients remain participant-installed and authenticated; record the exact tested client versions and reject or clearly report unsupported capability/version combinations. No hcom companion executable, dependency or update channel is part of installation.

### M6 — End-to-end collaboration beta

**Begins:** at the first runnable M1/M3/M4 slice. **Final release evidence depends on:** integrated M2–M5. **Deliverables:** a repeatable end-to-end scenario, operator-facing evidence, failure diagnostics and a measured resource baseline for the supported release artifacts. Use the same concrete task from the early real-agent trial through the packaged release run so integration failures remain comparable.

**Exit evidence:** two people on separate machines, using mixed clients and independent accounts without a shared forge dependency, paste the prompt, join a goal, delegate a concrete code task, share notes, submit/accept a patch and recover after disconnect/restart. Exercise direct/relayed transport and honest no-overlap availability reporting. Confirm the exact declared base, accepted artifact and separately observed local integration. Preserve event/artifact IDs, versions, route information and results without credentials/private source content.

Cover all four baseline clients in the task flow, with each acting as coordinator and worker in mixed-client runs. Repeat the task flow through Locust-managed client sessions for each claimed lifecycle capability, while retaining successful unmanaged CLI/MCP and explicit-resume evidence per client. Real-model task completion, deterministic hook/wake tests and confinement checks establish different claims; record them separately.

Publish the exact verified boundary: platform/client versions, real networks used, active-session requirement, coordinator outage behavior, holder availability and measured resource use. Set defensible “small daemon” targets from M0/M6 measurements, then guard regressions. Do not substitute an unmeasured size slogan or arbitrary fixed resource caps.

### Later milestones

1. **Optional retention peers:** encrypted stores/mailboxes for participants with non-overlapping uptime; explicit retention policies, signed receipts, authorization and holder-loss tests. A local sender outbox alone does not provide this service.
2. **Opt-in unattended runners:** one supported headless client at a time, with operator-controlled execution/sandbox and real closed-session startup/cancellation/restart tests.
3. **Additional transports/clients and A2A:** deeper execution/sandbox adapters, automatic wake beyond Merak, further active-session hook coverage and additional MCP transports over the same contracts; identical authorization and task tests, plus separately qualified local sandbox capabilities.
4. **Scale and authority evolution:** improve selective sync and storage after profiling; design explicit coordinator handoff/recovery if demanded by usage. Public discovery and replicated authority require their own threat/failure models and decisions.

## 9. Failure-focused conformance suite

Build tests around invariants and boundary failures, not copies of implementation branches. Promote the following matrix into executable tests as each subsystem arrives:

| Invariant | Required failure scenario | Planned gate |
|---|---|---|
| One accepted genesis/authority history | Different creator; conflicting coordinator successors; signed decision with conflicting preconditions; affected-goal-only halt | M2/M3 |
| Restore does not silently reuse stale signing state | Managed rollback after signing; copied-state equivocation; unavailable peers during recovery; single-store lock | M1/M2 |
| Authorization is independent of active session count | Missing credentials, session expiry, revocation during a stream | M1/M2 |
| Receipt/acknowledgment matches durability | Crash around event, blob, task and pending-delivery commits; disk-full/write error | M1–M3 |
| Replay has one semantic effect | Duplicate event/result; crash after application but before checkpoint | M1/M3 |
| Claims separate identity from execution instance | Same agent with two instances; lost claim response; long tool call, restart and explicit takeover | M3 |
| Checkpoints cannot lose unclaimed work | Crash after delivery before claim; transient claim failure | M1/M3 |
| Ingress checkpoint cannot outrun persistence | Event insert failure, unchanged cursor, storage recovery and identical-event retry; atomic projection update | M1/M2 |
| Notification handoff is not task ownership | Client exits after flush/enqueue before claim; duplicate/late wake; stale session binding and failed delivery acknowledgment | M1/M3/M5 |
| Client launch preserves local policy and ownership | Invalid config; existing root/network/approval settings; argv separators; authentication denied; remote assignment cannot spawn a process | M1/M5 |
| Launch and session recovery do not duplicate execution | Crash after spawn before binding; lost readiness response; stale/reused process identity; resume/fork/rebind with an old attempt generation | M1/M3 |
| Wake and cancellation respect client state | Merak-only automatic wake; cancellation for all baseline clients; idle versus busy client; approval prompt or active user input; request/exit/result race; surviving child processes and unknown effects | M3/M5 |
| Idempotency covers the entire request | Same key with another assignee/input/policy | M1/M3 |
| Remote policy matches authored intent | Deadline/attempt policy through request, restart and reassignment | M3 |
| Cancellation and completion have defined ordering | Executor offline; cancellation races result; effect outcome unknown | M3 |
| Revocation has an honest offline contract | Partitioned member continues producing old-epoch events/results | M2/M3 |
| Excluded history cannot grant authority | Honest causal descendant of revoked event; excluded membership/assignment/base used as semantic dependency | M2/M3 |
| Private data access is authorized per request | Non-member/revoked event-range or blob-by-hash fetch; unauthorized push; missing key update; plaintext/reserved-event bypass | M2 |
| Member input cannot bypass admission limits | Oversized/ambiguous frames, object/count quota exhaustion and repeated invalid references; explicit rejection without partial acceptance | M1/M2 |
| Gossip and DHT are replaceable hints | Missed gossip, expired discovery records, bootstrap unavailable | M0/M2 |
| Replication retains usable objects | Original source offline; all peers restart; interrupted transfer | M2 |
| Workspace integration preserves local ownership | Out-of-scope export, unsafe types/path escapes, Git filters, dirty checkout and accepted-but-not-integrated recovery | M4 |
| Packaged runtime actually works | Fresh install/database, repeat install, migration and service restart | M5 |
| Agent integration actually works | Default-profile roundtrip through the configured CLI or MCP path, skill discovery, interrupted wait, explicit resume, machine sleep/recovery and durable work recovery | M1/M5/M6 |

Keep unit/state-machine tests deterministic and exercise delayed, reordered, partitioned and clock-skewed inputs through a small controllable harness. Add real database crash tests as storage lands. Automate two-peer tests and use three local daemon instances where coordinator outage/revoked-author ancestry requires them; a third physical laptop is unnecessary. Run separate-network and actual coding-agent tests as soon as their paths exist. A new simulation framework is not a prerequisite. Link commands/raw evidence in the ledger; intermittent failures remain failures to investigate and are not erased by a rerun.

## 10. Implementation workflow and immediate next steps

1. Run the [two-Mac T1 guide](t1-run.md) with the exact [identified local candidate](t1-build.md). Copy only the executable bundle, create independent state on each Mac, and verify matching hashes. Record task flow, notes, offline catch-up, restart and sleep/wake in the ledger. Fix failures before adding unrelated features.
2. Retain three-peer fault coverage. Add a third daemon on either Mac when exercising surviving-peer exchange without the coordinator; a later third physical Mac is a separate topology check, not a prerequisite for starting now.
3. Complete T2: implement `locust mcp` and the operating skill, connect snapshot export/materialization and patches to the task workflow, then use two real coding clients on the available Macs. Exercise default-profile approvals, wait, interruption and explicit resume; extend the runnable pair to Codex, Claude Code, Factory Droid and Pi.
4. Complete remaining workflow and qualification work for key rotation, cancellation delivery, document revisions, larger content and Locust-managed client sessions. Some core behavior already exists; consult section 2 before creating replacement implementations. Keep independent-collaborator evidence separate from the owner's two-machine test.
5. Prepare the remaining release/install support matrix against the exact candidate. Public distribution remains deferred until the owner requests it; do not create a public repository or publish artifacts as part of local testing.

For Rust changes, use the pinned toolchain and crate-scoped `cargo fmt -p <crate> --check`, `cargo clippy --locked -p <crate> --all-targets -- -D warnings`, and `cargo test --locked -p <crate>`. Integration commits and changes to `locust-proto`, the root manifest or lockfile require the full-workspace equivalents. Documentation changes require content/link review and `python3 scripts/check_docs.py`; website changes have their own checks. [AGENTS.md](../AGENTS.md) and [workstreams](workstreams.md) define these rules; the [CI workflow](../.github/workflows/ci.yml) runs workspace checks. Requirements above count as verified only at the evidence level their enforcing code, tests and retained runs establish.

Commit completed scoped work, preserve unrelated changes, and keep useful experimental evidence in `research/`. Keep design status accurate: a published plan, a passing local test, a working packaged daemon and a successful real-peer collaboration are four separate outcomes.
