# Locust protocol contract, version 0

Date: 2026-10-03. **Status: opening-pass contract for the [implementation plan](implementation-plan.md). Implemented: the types, encodings, structural checks and golden vectors in [`crates/locust-proto`](../crates/locust-proto/src/lib.rs). Not implemented: the authorization and state-transition rules below, the daemon, storage and transport.** Version 0 carries no compatibility promise until the first release; after that, a change to a golden vector needs a new version. Crate ownership is in [workstreams](workstreams.md).

## Decisions recorded here

| Topic | Decision | Reason |
|---|---|---|
| Encoding | postcard 1.x: positional fields, variable-length integers, enum variants by declaration index | Compact and fast; no field names means no duplicate-field ambiguity; already in Iroh's dependency tree |
| Exact bytes | A header is encoded once; those bytes are hashed, signed, stored and relayed verbatim. Decoding rejects any bytes that are not the one encoding of the header they decode to | Verification never depends on re-serialization; one header has one identifier |
| Hash | BLAKE3. Identifiers use its key-derivation mode with a fixed context string per purpose; stored content uses plain BLAKE3 | Domain separation without ad hoc prefixes; content hashes match standard tools |
| Signature | Ed25519 (`ed25519-dalek` 3, strict verification) over a domain-separated digest of the event identifier | Same key type as the transport; one small fixed-size message to sign |
| Principal | An Ed25519 key held by the daemon for an enrolled agent. A membership decision binds the principal to the transport endpoint that may speak for it | The daemon is the signing delegate the plan describes; no certificate travels in every event |
| Goal identifier | Digest of the genesis record (owner key, coordinator key, random salt) | The initial authority cannot be replaced under the same identifier |
| Local API framing | Little-endian `u32` length, then a postcard value; JSON is only a rendering of the same types | One set of types; the CLI needs no async runtime |
| State directory | `$LOCUST_HOME`, default `~/.locust`, mode 0700; the socket is `daemon.sock` inside it | Unix socket paths are limited to about 104 bytes on macOS, and Codex does not pass `XDG_RUNTIME_DIR` to MCP servers |
| Execution session | One `locust mcp` bridge process is one session and holds one instance identifier; CLI calls name theirs explicitly | A claim is bound to a session without asking the model to remember a token |

## Signed events

[`event.rs`](../crates/locust-proto/src/event.rs) defines one header for every event.

| Field | Meaning |
|---|---|
| `version` | Protocol version; the first byte on the wire, so an unknown version is reported before anything else is decoded |
| `goal` | The goal this event belongs to. Every event binds its goal |
| `author`, `seq`, `prev` | The signing principal, its position in its own log for this goal, and its previous event (absent exactly at position zero) |
| `anchor` | For a coordinator decision, the decision it succeeds: decisions form one hash-linked chain. For a contribution, the latest decision the author had applied. Absent only on genesis |
| `parents` | Further causal references, strictly ascending |
| `at_ms` | The author's clock. Diagnostic only |
| `payload` | Hash, length and optional key epoch of detachable user content |
| `body` | The typed, structural content |

User text (task text, notes, summaries, document revisions) is always payload. Everything membership, assignment and acceptance depend on is in the signed body, so withholding a payload never breaks replay.

Three separate questions are asked of an event:

1. **Structurally valid** (`Event::decode`, implemented): canonical bytes within limits, internally consistent, signed by its author.
2. **Authorized** (core, not implemented): the author was a member at the decision the event anchors to and is inside any removal frontier; a decision is signed by the goal's coordinator and anchors to the current head.
3. **Effective** (core, not implemented): the transition in the table below is allowed in the current state.

A structurally valid event that fails the second or third question is retained as attributed evidence and satisfies ancestry links; it grants nothing. A missing dependency leaves an event pending. Two decisions with the same anchor, or two events by one author at one position, are conflicting histories: both are kept, and for decisions the goal's authoritative state stops advancing.

### Event kinds

Tasks, assignments, results, cancellations and revisions are identified by the event that created them.

| Event | Author | Requires | Effect |
|---|---|---|---|
| `Genesis` | Owner key it names | First event of the goal | Founds the goal and pins the coordinator |
| `MemberAdmitted` | Coordinator | Principal is not a member | Principal may contribute; its endpoint may synchronize |
| `MemberRemoved` | Coordinator | Principal is a member | Later contributions grant nothing; its open assignments can no longer finalize |
| `TaskProposed` | Any member | — | Task exists, proposed |
| `TaskAssigned` | Coordinator | Task exists and has no accepted result; assignee is a member; attempt is one more than the last, within the authored budget | Task assigned; any earlier assignment is superseded |
| `AssignmentAccepted` | Assignee | Assignment is current; a local claim was granted | Task taken |
| `AssignmentDeclined` | Assignee | Assignment is current and not taken | Task declined |
| `Progress` | Assignee | Assignment is current and taken | None |
| `ResultSubmitted` | Assignee | Assignment is current and taken | Task submitted |
| `AttemptFailed` | Assignee | Assignment is current and taken | Task failed |
| `CancelRequested` | Coordinator | Assignment has no accepted result | Cancellation requested; a later result of that assignment cannot be accepted |
| `CancelAcknowledged` | Assignee | Names a cancellation of its assignment | Cancelled, with the executor's reported outcome |
| `ResultAccepted` | Coordinator | Result belongs to the current assignment, which is not cancelled or superseded | Result accepted; the named manifest, if any, becomes the accepted workspace head |
| `ResultRejected` | Coordinator | Result is submitted and undecided | Result rejected |
| `Note` | Any member | — | Appended; may name the note it corrects |
| `Revision` | Any member | — | Proposed revision of the plan or summary |
| `RevisionAccepted` | Coordinator | The revision's base is the currently accepted revision | Accepted revision advances |

Local consent is not a protocol event. Whether the assignee's participant authorized the work, and which execution session holds the claim, is local daemon state: a claim is bound to a session and carries a generation, and a takeover raises the generation so the earlier session can no longer report or submit.

## Invitations and joining

[`invite.rs`](../crates/locust-proto/src/invite.rs). A ticket is the text `locust-invite-` followed by the hex of the encoded invitation: goal, coordinator key, the endpoint that redeems it, contact hints, a 32-byte secret and an optional expiry. The issuer stores only a digest of the secret. The joiner sends a join request signed by the key it wants admitted; the first key to redeem a ticket is the only one that ever can, and repeating the request returns the same result. Debug output of an invitation or a join request never contains the secret.

## Peer synchronization

[`sync.rs`](../crates/locust-proto/src/sync.rs). Two daemons exchange length-prefixed frames over one authenticated link. Each side states, per author, how many consecutive events it holds from position zero; the other sends what is missing, then the content objects that were asked for. A refusal is always sent as a frame with a reason. Membership is checked for every request, not once per connection. The frame set is provisional until the transport experiment confirms it.

## Local API

[`api.rs`](../crates/locust-proto/src/api.rs). A connection opens with a hello that presents a credential; the daemon answers with the caller it resolved (the owner, or one enrolled principal) or a refusal. Each request frame carries a client-chosen number, an optional idempotency key and one request. Every goal-scoped request names its goal.

- **Owner** enrolls and revokes principals, sets their grants and authorizes single assignments that no grant covers. The owner authors no events.
- **Grants** are what the participant has authorized a principal to do without asking again: manage goals, execute assigned work, sign coordinator decisions.
- **Operation names** (`Request::name`) are the stable names for commands, logs and errors; an MCP tool is `locust_` plus the name with dots replaced by underscores. `Request::is_read_only` marks the operations a bridge may advertise as read-only.
- **Pending work** is computed from task state and returned as identifiers only. `wait` holds the same question open until something changes or a caller-chosen timeout passes, and distinguishes work, no event, and no reachable peer. Denied, unsupported-version and corrupted-state are error codes.
- **Credentials** cross the socket in binary frames and render as `<redacted>` in any text form.

## Storage seam

[`store.rs`](../crates/locust-proto/src/store.rs). The state machine computes a commit: the events, content objects and local records one operation produces. A store applies a commit entirely or not at all, and content is durable before any record that names it is visible. Local, non-replicated records (enrollments, grants, claims, cursors, idempotency results, invitations, pending obligations, client sessions) are opaque key and value pairs in named spaces. Task and membership state is rebuilt in memory from the event log, so it cannot disagree with the log after a crash. `MemStore` is the reference implementation, and `store::conformance::run` is the behavior every store must match.

## Limits

[`limits.rs`](../crates/locust-proto/src/limits.rs) publishes the sizes checked before input is allocated or retained: 16 KiB per header, 64 parents, 1 MiB per payload, 64 MiB per content object by default, 256 events per batch.

## Golden vectors

[`vectors.rs`](../crates/locust-proto/src/vectors.rs) fixes the exact header bytes, identifier and signature of a genesis event and a task proposal, the plain content hash of a payload, and an invitation ticket, all from published test keys.

## Not decided here

- The payload encryption scheme and key distribution. `PayloadRef::key_epoch` reserves the field.
- Transfer of a content object larger than one frame.
- The default relay and address-lookup operator.
- Coordinator handoff, and any exit from a halted goal other than inspection, export and a new goal.
