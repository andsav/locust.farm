# Locust protocol contract, version 1

Date: 2026-10-03. **Status: implemented protocol/API version 1, with source and local regression verification tracked in the [remediation record](../research/t1-remediation.md).** The types and encoding live in [locust-proto](../crates/locust-proto/src/lib.rs), authority and state projection in [locust-core](../crates/locust-core/src/lib.rs), persistence in [locust-store](../crates/locust-store/src/lib.rs), and CLI/transport assembly in [locust](../crates/locust/src/main.rs). The production MCP bridge and client lifecycle remain separate implementation work. Component checks do not close packaged, physical-machine or release gates.

This revision accepts the changes motivated by the [independent T1 review](../research/t1-candidate-independent-review.md) and [assessment](../research/t1-candidate-review-response.md). `PROTOCOL_VERSION` (signed events, invitations and peer frames) and `API_VERSION` (the local socket) are both **1**. Version 0 is [archived](protocol-v0.md). Version-0 peers, tickets and local clients are explicitly refused. Homes containing version-0 events are refused with `unsupported_version` before schema migration, stored node-identity changes or garbage collection; exclusive WAL recovery may checkpoint physical pages. Daemon home preparation may create a missing owner credential before store preflight. No event migration is implemented. Preserve old binaries/homes and start fresh version-1 goals for new qualification.

The cryptographic context strings containing `locust v0` below remain fixed domain identifiers. Version 1 changes the signed header version and resulting event identifiers/signatures, not those primitive domains or the sealed-object layout. [Golden vectors](../crates/locust-proto/src/vectors.rs) pin the new bytes and explicit version-0 rejection. Existing enum indices are preserved; new variants are appended. Crate ownership is in [workstreams](workstreams.md).

## Decisions recorded here

| Topic | Decision | Reason |
|---|---|---|
| Encoding | postcard 1.x: positional fields, variable-length integers, enum variants by declaration index. Variants are only appended, never reordered | Compact and fast; no field names, so no duplicate-field ambiguity |
| Exact bytes | A header is encoded once; those bytes are hashed, signed, stored and relayed verbatim. Anything whose identity is the hash of its bytes (an event header, a manifest) is decoded only from its one canonical encoding | Verification never depends on re-encoding; one value has one identifier |
| Hash | BLAKE3. Identifiers and digests use its key-derivation mode with a fixed context string per purpose; content identity is plain BLAKE3 of the stored bytes | Domain separation without ad hoc prefixes; content hashes match standard tools |
| Signature | Ed25519 (`ed25519-dalek` 3, strict verification) over a domain-separated digest, never over raw caller bytes | Same key type as the transport; one small fixed-size message to sign |
| Principal | An Ed25519 key that the daemon holds for an enrolled agent and signs with as its delegate. A membership decision binds the principal to the one transport endpoint that may speak for it | No certificate travels in every event |
| Goal identifier | Digest of the genesis record: goal owner key, coordinator key, random salt | The initial authority cannot be replaced under the same identifier |
| Founding | Genesis admits nobody; the coordinator's first decision admits itself and binds its endpoint. In version 1 the goal owner is the coordinator | One rule for every member's endpoint binding, the coordinator's included |
| State | A goal's state is a function of the set of held events, never of their arrival order | Two daemons holding the same events show the same board |
| Content | Every content object a goal names is sealed; its identity is the hash of the sealed bytes | Storage, relays and transfer verify objects without keys; plaintext is not expressible |
| Key epochs | Count member removals through an event's anchor, plus the event itself when it is a removal | A removal carries proof of its new key with no further decision, and a stale epoch is visible in the header |
| Reconciliation | Per author, a count of consecutive positions and a running digest of exactly which events are held | Equal-length divergent histories are found; a simply-behind peer is answered with a suffix |
| Discovery | Works like BitTorrent: peers are found by endpoint key, and any member synchronizes with any member | Which network a peer is on is not something a participant should care about |
| Local API framing | Little-endian `u32` length, then a postcard value; JSON is only a rendering of the same types | One set of types; the client needs no async runtime |
| State directory | `$LOCUST_HOME`, default `~/.locust`, mode 0700; every name in it is defined once in [`local.rs`](../crates/locust-proto/src/local.rs) | One definition shared by daemon, CLI, bridge, hooks and installer |
| Execution session | A launcher or bridge generates a 32-byte session secret, keeps it in a file the model never reads, and presents it in the hello. No request names a session | A claim is bound to a session without asking the model to hold a token |
| Claims | A claim binds an assignment to one session and carries a generation; a takeover raises it and fences the earlier holder | A delayed write from a replaced session fails instead of landing |
| Commands | A command is an operation name with dots written as spaces; `--json` prints one envelope; exit statuses are fixed per error code | The installer, skill, guide and tests share the implemented CLI contract |
| Storage commit | A store applies a commit entirely or not at all, and what it reports as durable survives power loss | An event, the state derived from it and the idempotency record never disagree after a crash |

## Identifiers, hashing and signatures

[`id.rs`](../crates/locust-proto/src/id.rs) defines the fixed-size identifiers. In binary encodings they are their raw bytes with no length prefix; in JSON they are lowercase hex strings, and parsing also accepts uppercase.

| Identifier | Bytes | What it is |
|---|---|---|
| `EventId` | 32 | Domain-separated digest of an event's exact header bytes. Tasks, assignments, results, cancellations, notes and revisions are identified by the event that created them |
| `GoalId` | 32 | Domain-separated digest of the genesis record |
| `BlobHash` | 32 | Plain BLAKE3 of a stored content object, which is always its sealed bytes |
| `PublicKey` | 32 | Ed25519 verifying key of an enrolled principal |
| `EndpointId` | 32 | Transport identity of a daemon |
| `Signature` | 64 | Ed25519 signature |
| `InstanceId` | 16 | Public handle of one execution session |
| `IdempotencyKey` | 16 | Client-chosen key that makes a retried request return its first result |

[`crypto.rs`](../crates/locust-proto/src/crypto.rs) holds the primitives. A domain-separated digest is BLAKE3 in key-derivation mode, `derive_key(context, input)`, with one context string per purpose, so a digest made for one purpose can never be presented as another:

| Context string | Input | Result |
|---|---|---|
| `locust v0 event id` | The exact header bytes | `EventId` |
| `locust v0 event signature` | The event identifier | The message an event signature signs |
| `locust v0 goal id` | Goal owner key, coordinator key and salt, concatenated | `GoalId` |
| `locust v0 invitation secret` | An invitation secret | What the issuer stores instead of the secret |
| `locust v0 join signature` | Goal, member key, endpoint and the secret's digest, concatenated | The digest a join request signs (signed under the same context) |
| `locust v0 local credential` | A local credential | What the daemon stores instead of the credential |
| `locust v0 session instance` | A session secret | Its first 16 bytes are the `InstanceId` |
| `locust v0 author log digest` | Previous state, position (`u64` little-endian) and event identifier | One step of an author's running log digest |
| `locust v0 seal key` | A content key | The cipher key that seals content |
| `locust v0 seal nonce` | A content key | The key of the keyed hash that derives a nonce |

A signature for a purpose signs `derive_key(context, digest)` of a 32-byte digest. Verification is strict: a malformed key or a non-canonical signature fails, and because every peer must reach the same verdict on every event, any batched fast path must fall back to this check for the final answer. The contract crate never generates randomness; callers supply it.

## Signed events

[`event.rs`](../crates/locust-proto/src/event.rs) defines one header for every event. A goal's history is the set of its events: each one is signed by its author, and each names the goal it belongs to.

| Field | Meaning |
|---|---|
| `version` | `PROTOCOL_VERSION`. The first byte on the wire, so an unknown version is reported before anything else is decoded |
| `goal` | The goal this event belongs to. Every event binds its goal |
| `author` | The principal whose key signed the header |
| `seq` | The event's position in its author's own log for this goal, from zero, at most `i64::MAX` so every store can index it as a signed 64-bit integer |
| `prev` | The author's previous event; absent exactly when `seq` is zero |
| `anchor` | For a coordinator decision, the decision it succeeds, which makes a goal's decisions one hash-linked chain. For a contribution, the latest decision the author had applied when writing. Absent only on genesis |
| `parents` | Further events this one causally follows, strictly ascending, at most 64 |
| `at_ms` | The author's wall clock in Unix milliseconds. Diagnostic only: it never orders events, grants authority or resolves a conflict |
| `payload` | The sealed user content of the event, if any; see [Payloads and content](#payloads-and-content) |
| `body` | The typed, structural content |

User-written text (task text, notes, result summaries, document revisions, reasons) is always payload. Everything membership, assignment and acceptance depend on is in the signed body, so withholding a payload never breaks replay.

An event travels and is stored as a `WireEvent`: the exact header bytes and the author's signature. `WireEvent::id` computes the identifier from the bytes alone, which is far cheaper than validation, so a receiver drops events it already holds before decoding them. `Event::decode` validates bytes from a peer or a client in this order: size (at most 16 KiB), version byte, the one canonical encoding, the structural check, then the signature. `Event::from_stored` rebuilds an event this daemon already validated: it repeats the structural check and refuses bytes that no longer hash to the identifier they were stored under, which the local API reports as `corrupted`, but it skips the canonical re-encoding and the signature check. It is never used on input from outside the local store.

The structural check (`Header::check`) needs no history. It requires the version to be 1; `seq` at most `i64::MAX` and `prev` present exactly when `seq` is not zero; at most 64 parents in strictly ascending order; a payload length between 45 bytes (an empty sealed object) and 1 MiB; at most 64 task dependencies and 64 result artifacts; a genesis event that founds the goal it names; and an anchor on every other event.

### What is asked of an event

Three separate questions are asked of every event:

1. **Structurally valid** (`Event::decode`, implemented): canonical bytes within limits, internally consistent, signed by the author it names.
2. **Authorized** (core): the author was a member at the decision the event anchors to and, if it has since been removed, the event lies at or before the last point of its log that the removal still counts; a decision is signed by the goal's coordinator and anchors to the decision it succeeds; the payload is sealed under the event's key epoch.
3. **Effective** (core): the transition the event describes is allowed in the state at the point where the event applies.

A structurally valid event that fails the second or third question is kept as attributed evidence, called *excluded*: it satisfies ancestry links but grants nothing. An event whose dependency has not arrived is *pending* and is judged when the dependency arrives. Two decisions with the same anchor, or two events by one author at one position (two genesis events of one goal included), are conflicting histories: both are kept and served, neither replaces the other, and when decisions conflict the goal's authoritative state stops advancing (the goal is *halted*; see [Local API](#local-api)).

**State is a function of the held events** (core). A goal's state never depends on the order in which its events arrived. Decisions apply in chain order. A contribution applies directly after the decision it anchors to, and contributions that share an anchor apply in order of author key, then sequence number. Two daemons holding the same events therefore show the same board, membership and accepted heads. The feed of the local API is the one place where local order shows; it numbers events in the order this daemon applied them.

**Member forks and coordinator commitments.** An unselected member fork excludes that author's suffix. A semantically valid decision on the canonical coordinator chain can select the exact contribution it names and the transitive semantic and author-predecessor dependencies needed for it. Selections are recomputed from the held set in coordinator-chain order, never from what this replica happened to apply earlier. Earlier valid selections win conflicts; later selections must preserve them. Candidate branches are checked by the ordinary fold, including membership, key epoch, removal cutoffs, reference kind and transition preconditions. A known-invalid decision selects no branch. A pending decision retains required evidence while withholding branch selection and keeps later canonical decisions stalled until that evidence resolves. A direct referenced header can be retained to establish invalidity, but a known-invalid target does not exempt arbitrary ancestry from screening. Coordinator forks still halt authority; this mechanism never selects one coordinator branch. These rules are enforced in [commitments](../crates/locust-core/src/goal/commitments.rs), [fold](../crates/locust-core/src/goal/fold.rs), [screening](../crates/locust-core/src/goal/screen.rs) and [goal regressions](../crates/locust-core/src/goal/tests.rs).

The selected canonical dependencies can exceed the ordinary waiting/variant screening quotas when needed to judge a valid or still-pending canonical decision. Same-batch decisions and references are considered together. Conflicting evidence remains retained, and later ordinary reconciliation can offer screened-out events again. A late signed result is evidence but cannot replace an already accepted task's displayed result.

### Founding a goal

The genesis record names the *goal owner* (the root authority, which signs genesis), the *coordinator* (which signs every later decision) and 16 random bytes of salt; the goal identifier is the digest of the three. The genesis event is the owner's position 0, with no anchor and no parents. In version 1 the goal owner is the coordinator: the structural check refuses a genesis record that names two different keys.

Genesis founds authority and admits nobody. The coordinator's first decision, at its position 1 and anchored to genesis, is `member_admitted` for itself and its own endpoint; that is what binds the coordinator's endpoint like any other member's. Goal creation commits both events together. The goal identifier pins the genesis record, not the genesis event, so an owner can sign two genesis events for one goal; they are conflicting histories like any other.

### Event kinds

Body variants 0 to 7 are *decisions*, which only the goal's coordinator may author (`Body::is_decision`); variants 8 to 17 are *contributions*, which any current member may author. Each variant's JSON tag is its kind (`Body::kind`). The requirements and effects are core rules.

| Index | Kind | Author | Requires | Effect |
|---|---|---|---|---|
| 0 | `genesis` | Goal owner named in the record | First event of the goal | Founds the goal and pins the coordinator; admits nobody |
| 1 | `member_admitted` | Coordinator | The principal is not a member | The principal may contribute; the named endpoint is the only one that may synchronize on its behalf |
| 2 | `member_removed` | Coordinator | The principal is a member | Its contributions after the named `last_accepted` point of its log are kept as evidence but grant nothing; its open assignments can no longer finalize; events anchored at or after this decision use the next key epoch |
| 3 | `task_assigned` | Coordinator | The task exists and has no accepted result; the assignee is a member; `attempt` is one more than the previous assignment's, within the task's authored attempt budget | Task assigned; any earlier assignment of the task is superseded |
| 4 | `cancel_requested` | Coordinator | The assignment has no accepted result | The assignment can no longer finalize, so none of its results can be accepted; the executor answers with `cancel_acknowledged` |
| 5 | `result_accepted` | Coordinator | The result belongs to the current assignment, which is neither cancelled nor superseded. With `head`, the result's `base` must equal the currently accepted head, or no head is accepted yet | Result accepted; the named manifest becomes the accepted workspace head |
| 6 | `result_rejected` | Coordinator | The result is the latest undecided submission of the current, non-cancelled, non-revoked assignment; the task has no accepted result | Result rejected; the payload may say why |
| 7 | `revision_accepted` | Coordinator | The revision's `base` is the currently accepted revision of its document | The accepted revision advances |
| 8 | `task_proposed` | Any member | At most 64 dependencies | The task exists, proposed. The payload is the task text; `input`, `depends_on`, `deadline_ms` and `max_attempts` are the authored policy every peer sees unchanged |
| 9 | `assignment_accepted` | Assignee | The assignment is current, and a session of the assignee holds the local claim on it | Task taken |
| 10 | `assignment_declined` | Assignee | The assignment is current and not taken | Task declined |
| 11 | `progress` | Assignee | The assignment is current and taken | None; text in the payload |
| 12 | `result_submitted` | Assignee | The assignment is current and taken; at most 64 artifacts | Task submitted, naming the manifest the work was done against, a patch and further output objects. Completion is not acceptance |
| 13 | `attempt_failed` | Assignee | The assignment is current and taken | Task failed; reason in the payload |
| 14 | `cancel_acknowledged` | Assignee | Names a cancellation of its assignment | Cancelled, with the executor's reported outcome: `stopped`, `completed` or `uncertain` |
| 15 | `note` | Any member | — | An append-only note or finding, optionally about one event and optionally correcting an earlier note |
| 16 | `revision` | Any member | — | A proposed revision of the `plan` or the `summary`, naming the accepted revision it was written against |
| 17 | `leave_requested` | Any member | — | The author asks the coordinator to remove it; membership ends only with `member_removed` |

Adding a variant requires a new protocol version: an older peer cannot decode the new index, and it cannot skip the event either, because the author's later events chain to it through `prev`.

Local consent is not a protocol event. Whether the assignee's local participant authorized the work, and which execution session holds the claim, is local daemon state (see [Sessions and claims](#sessions-and-claims)).

## Payloads and content

Every version 1 goal is private. [`seal.rs`](../crates/locust-proto/src/seal.rs) defines the only form in which a goal's content is stored and transferred: every content object an event names (a payload, a manifest, a file, a patch, an artifact) is the output of sealing. Its identity is the plain BLAKE3 hash of the sealed bytes, so a store, a relay or a peer verifies, deduplicates and resumes a transfer of an object without holding any key. Plaintext exists only inside a member's daemon and on its local API; keys never reach the CLI, the bridge or an adapter.

An event names its payload with a `PayloadRef`: `hash` and `len` describe the stored, sealed bytes, and `key_epoch` is the epoch they are sealed under. Before a daemon keeps bytes as an event's payload it checks `PayloadRef::admits`: the same hash, the same length, and the epoch written in the sealed object equal to the one the signed header claims, so an author cannot claim the current epoch while sealing under a key a removed member still holds. A payload is sealed under exactly its event's epoch. Every other object an event names may be sealed under any epoch up to the event's own (core), since a manifest exported earlier keeps its identity when a later event names it. `Header::blobs` lists every object a header names: the payload first, then whichever of these the body carries, an accepted result's head, a proposed task's input, or a submitted result's base, patch and artifacts.

A workspace manifest ([`manifest.rs`](../crates/locust-proto/src/manifest.rs)) is the reviewed list of files that makes up a shared snapshot: entries of path, executable bit, plaintext size and the hash of the file's sealed object, strictly ascending by path, at most 100,000. Only regular files and an executable bit are expressible; symlinks, hard links, special files and submodules are not. `is_safe_path` refuses a path longer than 1,024 bytes, and a path with any component that is empty (so no absolute path), `.` or `..`, `.git` or its Windows short name `git~1` in any ASCII case, or longer than 255 bytes, or that contains a control character (the C1 range included), `\` or `:`, ends with a dot or a space, or contains an invisible formatting character. No path may be both a file and a directory. Case and Unicode folding depend on the destination filesystem, so the materializer creates every file with create-new semantics in a fresh destination and reports a collision instead of overwriting. Events name a manifest by the hash of its sealed object, like any content. `Manifest::plain_digest` is the BLAKE3 of the canonical plaintext encoding, the same in every goal and epoch, for comparing two snapshots; it is never what an event names, and comparing it with an event's reference never matches. A source Git commit is provenance only.

### Sealed object layout

| Offset | Bytes | Field |
|---|---|---|
| 0 | 1 | Format byte, `1` |
| 1 | 4 | Key epoch, `u32` little-endian |
| 5 | 24 | Nonce |
| 29 | n | Ciphertext, exactly as long as the plaintext |
| 29 + n | 16 | Poly1305 tag |

A sealed object is 45 bytes longer than its plaintext, so the smallest is 45 bytes (the empty plaintext) and the largest plaintext is 64 MiB minus 45 bytes. Every limit on content counts sealed bytes. The format byte and epoch are in the clear so that a holder learns which epoch key opens an object from the object alone: a manifest entry or a patch reference is a bare hash with nowhere else to say it. An object carrying another format byte is refused, never guessed at; a different cipher, derivation or layout takes a different byte.

### Key and nonce derivation

`key` is the goal's 32-byte content key for the epoch.

```text
enc_key   = BLAKE3 derive_key(context = "locust v0 seal key",   material = key)
nonce_key = BLAKE3 derive_key(context = "locust v0 seal nonce", material = key)
aad       = format (1 byte) || epoch (4 bytes, little-endian) || goal (32 bytes)
nonce     = first 24 bytes of BLAKE3 keyed_hash(nonce_key, aad || plaintext)
ciphertext || tag = XChaCha20-Poly1305(enc_key, nonce, aad, plaintext)
```

The nonce is a keyed hash of everything the cipher is given, so sealing is deterministic and needs no randomness or counter: the same content in the same goal and epoch always gives the same bytes and therefore one identity. Covering the associated data as well as the plaintext means two different inputs never share a nonce, even if one key were wrongly used for two epochs or two goals. Opening recomputes the nonce from what it decrypted and refuses a mismatch (`NotCanonical`), so exactly one byte string opens to a given content under a given goal, epoch and key, no key holder can mint a second identity for it, and making one object open to different content under two keys needs a collision on 192 bits of keyed BLAKE3. The epoch read from an object without a key (`epoch_of`) is unauthenticated until opening succeeds and is used only to choose a key.

### What sealing protects and what it does not

Sealing protects the confidentiality and integrity of content against anyone without the epoch key, relays and non-member holders included. An object opens only for the goal and epoch it was sealed for, and a change to any byte is detected.

It does not hide equality: two objects with the same content in one goal and epoch are the same bytes, so an observer who can also get a member to seal content of its choosing can confirm a guess of another object's whole content. It does not hide length or epoch. It does not establish authorship, because every holder of the epoch key can seal; who introduced an object is established by the signed event that names its hash. And it does not withdraw earlier epochs: a removed member keeps the keys it was given and can still open what was sealed under them.

### Key epochs and how keys travel

The epoch of an event is the number of `member_removed` decisions in the chain up to and including its anchor, plus one when the event itself is a removal, so genesis is epoch 0. A removal therefore rotates the key with no further decision: its own payload and events anchored at or after it use the next epoch. The core checks every payload's epoch against the chain and refuses a stale one. This resolves the earlier contradiction between removal's epoch and the requirement to verify a newly served key against its founding decision.

The coordinator's daemon generates a random content key for each epoch. Keys are local records (`Space::Key`), never rendered in text and never read from it. Between daemons a key travels only in a `Key` frame over a link whose remote endpoint speaks for a current member of the goal: any member holding a key may serve it, a member admitted later is given every earlier epoch so it can read history, a removed member is refused with `not_a_member`, and a responder without that epoch's key answers `key_unavailable` so the asker tries another member. A member that missed a rotation obtains the new key on its next exchange with any current member.

A daemon that hosts several principals checks each local reader's entitlement separately. Pending/refused join intent confers no read authority. Canonical admission grants historical epochs; removal caps access at the epoch before that removal, and a subsequently applied readmission restores history through its new tenure. Titles, event/task/note text, document views, blob availability and raw blob reads use the caller's epoch rights, including viewer credentials. Possessing an epoch key for another co-hosted principal does not grant it to every local reader. These checks are enforced by [access](../crates/locust-core/src/node/access.rs), [entry](../crates/locust-core/src/node/entry.rs), [views](../crates/locust-core/src/node/views.rs), [content requests](../crates/locust-core/src/node/requests/content.rs) and [authorization regressions](../crates/locust-core/src/node/tests/authorization.rs).

Nothing signed commits to an epoch key, so a receiver checks a served key before keeping it: a key for an epoch is accepted only if it opens the payload of the decision that starts that epoch, an object the coordinator sealed under it (core). Genesis proves epoch 0; each removal proves the epoch it starts. The coordinator's daemon always gives those decisions a sealed payload, using empty text when there is no explanatory text. If the proof payload is absent or has not arrived, a receiver does not retain an unverified key. Because opening also checks the nonce, a wrong key can never make an object open to different content; it can only fail to open, and the receiver then asks another member.

## Invitations and joining

[`invite.rs`](../crates/locust-proto/src/invite.rs). An invitation is a single-use capability the inviter hands over through a channel of their choosing. It names the goal, the coordinator's key, the endpoint of the daemon that redeems it, contact hints, a 32-byte secret and an optional expiry in Unix milliseconds judged by the issuer's clock. Holding one shares nothing and enrolls nothing. In version 1 only a goal's coordinator issues invitations, so the daemon that redeems one is the daemon that signs the admission.

A *ticket* is the invitation's pasteable text: `locust-invite-` followed by the lowercase hex of the encoded invitation, whose first byte is the protocol version. Reading a ticket ignores surrounding whitespace and any ASCII whitespace inside the hex, because chat and mail clients wrap long lines; refuses text longer than 8,206 bytes (the prefix plus 8,192 hex characters) before decoding anything; and reports another protocol version as unsupported before reading further.

An invitation carries at most 8 contact hints, each non-empty, at most 256 bytes and free of control characters (bytes below 0x20 and 0x7f), so it can be shown to the joiner as is. The hint grammar (`Hint::parse`) has two forms: a relay URL, which is `https://` followed by at least one character and is taken whole; and a socket address `ip:port`, with an IPv6 address in brackets. A consumer skips any other hint without error, so a later version can add forms without breaking tickets for this one. Hints are optional; see [Discovery](#discovery).

The secret is the capability. The issuer indexes the invitation by its digest and stores no secret in that lookup record. When `goal.invite` carries an idempotency key, the protected local response cache retains the returned ticket so a retry returns the same capability; this is the exception to digest-only storage. The secret's type renders as `InviteSecret(..)` in debug output and `<redacted>` in JSON, and is never read from text. The ticket text carries the secret, so its debug form is `Ticket(..)`; JSON shows a ticket as its bare text only where it is being handed over, in the answer to `goal.invite` and in the `goal.join` request.

To redeem, the joiner sends a `JoinRequest` naming the goal, the key it wants admitted, the endpoint that will speak for that key and the secret, signed by that key. The signature proves possession of the key only: whoever admits must also require the request's endpoint to equal the authenticated remote endpoint of the link it arrived on, or a relayed copy could bind the member to an endpoint that never asked. Whether the secret names a live invitation is the issuer's lookup by digest. The first key to redeem an invitation is the only one that ever can; repeating the request with that key returns the same result, and any other key is refused. On the peer link a `Join` is answered with the responder's frontier once the key is admitted, also on a repeat, and otherwise with `Refused(invitation_refused)`. The admission is a `member_admitted` decision binding the new member to the endpoint in the request. The joiner is shown the coordinator's key as a fingerprint and checks it against the genesis record once that arrives.

Redemption rechecks that the issuing principal is active, has not locally left, and still has `manage_goals` and the goal's `decide` grant. Redemption is refused while issuer activity, grant or departure conditions fail. Grant changes do not delete invitation records: restoring grants can re-enable a still-unexpired, unredeemed ticket. Retrying an already admitted recipient remains idempotent only while its endpoint binding is current. Owner-initiated issuance also remains subject to this revocation policy. A past or present `expires_ms` is invalid. The same checks redeem an invitation addressed to this daemon locally, atomically with participation, instead of dialing its own endpoint.

When the goal is already held, the ticket's coordinator endpoint must match the signed binding. On an outbound join, no held frontier, inventory or event history is disclosed until the authenticated remote endpoint is established as a current member; an empty frontier is permitted while receiving admission evidence. A refusal becomes public membership `refused`. A fresh invitation may replace a refused attempt. Repeating the same pending ticket is idempotent; a different ticket while pending returns conflict and directs the caller to check status and retry after refusal. A refused attempt never erases genuine historical read rights. The local coordinator cannot leave or remove itself until an explicit handover policy exists.

## Peer synchronization

[`sync.rs`](../crates/locust-proto/src/sync.rs) defines the frames two daemons exchange and the rule that reconciles their copies of a goal.

**What the transport provides.** The transport is constructed from a 32-byte endpoint secret. It gives the daemon the local endpoint identifier, the local contact hints, the authenticated remote endpoint identifier of each link, and notifications when a link comes up or goes down. Each exchange is one bidirectional stream of length-prefixed frames, each an encoded `SyncMessage`. The transport decides nothing about membership: whether a remote endpoint speaks for a member (whether a `member_admitted` decision of the goal binds a current member to it) is decided by the daemon for every request, not once per connection.

### Frontiers and the reconciliation rule

A *point* is a position in one author's log and the event there. A *frontier* states, for each author a daemon holds events of, an `AuthorFrontier`: `next_seq`, the number of consecutive positions held from zero (a position holding several conflicting events counts once), and `digest`, a running digest of every retained event of that author below `next_seq` in ascending (position, identifier) order, conflicting events included. The digest starts at 32 zero bytes, and each point extends it to `derive_key("locust v0 author log digest", state || seq as u64 little-endian || id)`; a holder that keeps the state after every point answers the digest of any prefix by lookup. Equal `next_seq` and `digest` mean equal retained prefixes below `next_seq`; gapped points beyond that prefix are not represented by the digest. Frontier entries are strictly ascending by author, at most 4,096; an author missing from a frontier is held from nothing, which is `(0, 32 zero bytes)`.

The rule for answering a peer's entry `(n, d)` for one author (`AuthorFrontier::is_prefix_of`): if I hold at least `n` positions and my digest below `n` equals `d`, I send the suffix. If I have a shorter contiguous history, I return my frontier so the longer holder can verify that prefix and send its suffix. A longer count alone never proves agreement: a failed prefix comparison requests inventory, including for unequal-length forks. Gapped or divergent histories use `Inventory` points and requests by exact identifier. Both directions use this rule. Conflicting events make digests differ even at equal counts; the [convergence tests](../crates/locust-core/src/sync/tests/convergence.rs) cover suffix-only exchange and unequal-length forks. Live pushes remain hints: later frontiers recover missed events.

### Frames

Frames are identified by declaration index, so the order below is the wire tag. `Hello` stays index 0 and `version` its first field in every protocol version, so a peer always reads the version first.

| Index | Frame | Meaning |
|---|---|---|
| 0 | `Hello { version, goal }` | Opens an exchange about one goal; the initiator's first frame |
| 1 | `Join(JoinRequest)` | Redeems an invitation; eligible historical nonmembers may also send halt proof |
| 2 | `Refused(Refusal)` | The responder will not serve the request this answers |
| 3 | `Frontier(Frontier)` | What the sender holds of the goal |
| 4 | `Events(Vec<WireEvent>)` | At most 256 events, each header at most 16 KiB, each author's log ascending |
| 5 | `InventoryRequest { author, after }` | Asks for the responder's points of an author after a point, or from the first |
| 6 | `Inventory { author, points, more }` | At most 4,096 points, strictly ascending by (position, identifier); `more` says further points follow |
| 7 | `EventRequest(Vec<EventId>)` | Asks for at most 4,096 events by identifier |
| 8 | `KeyRequest { epoch }` | Asks for the goal's content key of one epoch |
| 9 | `Key { epoch, key }` | The content key of one epoch; never shown in debug or text forms |
| 10 | `BlobRequest { hash, offset }` | Asks for a stored object from a byte offset, which resumes an interrupted transfer |
| 11 | `BlobChunk { hash, offset, total, bytes }` | At most 1 MiB of the object's stored bytes from `offset`; `total` is its whole stored length, at most 64 MiB |
| 12 | `BlobUnavailable(hash)` | The responder does not hold, has withdrawn, or cannot serve the asked range of this object |
| 13 | `Done` | Ends the exchange |
| 14 | `HaltProof([WireEvent; 2])` | Exactly two conflicting signed events at one coordinator position for this known goal; proof-only delivery |

A refusal is always sent as a frame, never expressed by silence. Its reasons render as `unsupported_version`, `not_a_member` (also the answer for a goal the responder does not know, so the two cannot be told apart), `invitation_refused`, `limit_exceeded`, `protocol_error` and `key_unavailable`.

Frames are decoded with `SyncMessage::decode`, which returns the refusal to send back. It refuses a `Hello` of another version with `unsupported_version` before decoding the rest; a count or size above a limit (events per batch, header size, frontier authors, inventory points, event request size, chunk size, object size) with `limit_exceeded`; and bytes that do not decode, a frontier not strictly ascending by author, an inventory not strictly ascending by point, or a chunk that ends past its object with `protocol_error`.

### Exchanges

An exchange is one bidirectional stream opened by the initiator. A daemon that wants something opens its own exchange, so two exchanges never share a stream. The initiator sends `Hello`, then requests; the responder answers each request in order and sends nothing unasked:

- `Hello` is not answered when accepted. A `Hello` of another version is answered with `Refused(unsupported_version)` and ends the exchange.
- `Join` is answered with the responder's `Frontier` once the key is admitted, also when the same key repeats it, and otherwise with `Refused(invitation_refused)`.
- `Frontier(mine)` is answered with zero or more `Events` and `Inventory` frames, chosen by the reconciliation rule for every author either side holds, then the responder's own `Frontier`. The initiator applies the same rule to that frontier and pushes the `Events` the responder lacks. Pushed `Events` are not answered.
- `InventoryRequest` is answered with one `Inventory`.
- `EventRequest` is answered with one `Events` frame for each run of 256 requested identifiers, in order, carrying the events of that run the responder holds (possibly none). An empty request has zero runs and produces no frame.
- `KeyRequest` is answered with `Key`, with `Refused(not_a_member)` when the remote endpoint does not speak for a current member, or with `Refused(key_unavailable)` when the responder holds no key for that epoch.
- `BlobRequest` is answered with `BlobChunk` frames that run contiguously from `offset` to `total` (one empty chunk when `offset` equals `total`), or with `BlobUnavailable` when the responder does not serve the object or `offset` is past its end.
- The driver emits `Hello`, `HaltProof`, `Done` for evidence delivery. A receiver authorizes each proof independently after Hello, including in an already admitted exchange. Both signatures, the known goal/coordinator and the conflicting position are checked. Eligible historical contacts include endpoints named by held signed coordinator admissions, including excluded history, and a pending inviter. Proof permission grants no membership, key or ordinary content access.
- `Done` is not answered; it ends the exchange.

**Receive limits and backpressure.** Unadmitted incoming peers start with the 4 KiB hello limit. Current-member admission raises the limit to the ordinary peer frame limit; a historical proof-only exchange instead receives the 33,024-byte two-header proof allowance. Dialed exchanges start at the peer read limit, independently of outbound disclosure authorization. Unknown goals and ineligible endpoints get the same `not_a_member` refusal. Neither proof allowance nor a connection established by dialing is authorization to disclose ordinary content or keys. All outbound reconciliation checks authenticated endpoint membership.

Responses are lazy cursors, including inventory and frontier expansion. The shell holds the next input acknowledgement while that exchange owes output, and rechecks `peer_readable` as writes complete. At most one frame awaits transport capacity; an input flood cannot queue eager inventory answers. Exchange idleness is measured across completed frames in either direction, so active one-way transfers survive while genuinely idle streams time out. Unadmitted transport connections have a fixed admission deadline and a shared receive-credit budget; admitted duplicate links are bounded per endpoint. See the [outbox](../crates/locust-core/src/sync/outbox.rs), [driver](../crates/locust-core/src/sync/driver.rs), [worker](../crates/locust/src/daemon/worker.rs) and [network shell](../crates/locust/src/daemon/network.rs).

**Ending.** A stream that ends without `Done` is an aborted exchange, not a finished one; what arrived before stays valid, since every event stands alone. The sender of an exchange's last frame, a `Refused` in particular, waits under a deadline for the transport to acknowledge it before dropping the link, because a frame queued behind a dropped link is not delivered. Stream reset codes and connection close codes carry no meaning in version 1 and are zero; reasons travel as `Refused` frames.

### Chunked content

An object travels as `BlobChunk` frames of at most 1 MiB. The receiver stages the chunks durably (`Store::stage_blob`) and, after a reconnect or a restart, resumes with a `BlobRequest` at the staged length. The receiver reads the staged prefix with `Store::staged_range` to check the sealed epoch and signed reference before promotion, including when the prefix spans chunks or a restart. Invalid staged content is durably discarded with `Store::discard_staged_blob`, without removing a separately held object. When the object is complete it is verified against its hash before it is promoted to a held object (`Store::finish_blob`); a mismatch discards the staged bytes. Because identity is plain BLAKE3, verified streaming can be added later without changing identifiers. The local socket still carries a whole object in one frame.

For payload-only associations, an incoming total must match at least one permissible signed PayloadRef length. Bare body hashes and local wants do not carry a signed length; their totals remain protocol-bounded and their epoch metadata is checked once a complete sealed prefix is available. An unavailable nonzero resume receives one zero-offset retry. A poisoned legacy prefix is replaced only when that retry supplies admissible metadata; unavailability itself preserves shared staging. The ordered wanted-object index follows committed event/blob changes and is rebuilt on reopen, avoiding a full-goal scan when selecting the next wanted object. Metadata admission and serving can still scan goal events. [Replica regressions](../crates/locust-core/src/node/replica_tests.rs) cover resume, peer failure, completion and reopening.

## Discovery

**Implemented lookup; separate-machine qualification pending.** Peer discovery follows the owner's BitTorrent direction in the [implementation plan](implementation-plan.md), section 3. The [transport](../crates/locust-net/src/lib.rs) registers both lookup providers and the [daemon](../crates/locust/src/daemon/network.rs) enables them by default. A participant does not need to know or care which network a peer is on, and the [first three-machine test](workstreams.md) does not treat the network as a variable.

A goal behaves like a private swarm. An invitation plays the part of a magnet link: it names the goal and the inviter by key. Members learn each other's endpoint keys from the signed `member_admitted` decisions. Each daemon publishes its relay address under its endpoint key and looks peers up by key on the BitTorrent Mainline DHT, and advertises direct IP addresses plus at most one relay on the local network through multicast discovery, so it can dial a member from its endpoint key alone, including a member it has never met. Any member synchronizes with any other member it can reach, not only with the coordinator: the reconciliation rule works between any two holders, and any member holding a key may serve it. Contact hints in an invitation are an accelerator, never a requirement.

Three things remain unlike a public torrent and are stated to users rather than hidden. The DHT is reached through bootstrap nodes, as BitTorrent's is. Two peers that are both behind address translation still need a third party to help them connect, which here is a replaceable relay. And a goal is private: only invited members are admitted and content is sealed. Mainline publication reveals the endpoint key and relay address; DHT nodes also observe the network source of requests. Multicast publication exposes endpoint keys and contact addresses on the local network. Goal identifiers, membership and content are not in those discovery records.

The contract does not depend on how an address is found: it carries only `EndpointId` and hint strings, and the [engine seam](#the-engine-seam) asks the transport to open an exchange by endpoint key with hints that may be empty, because finding an endpoint by its key is the transport's job. The pinned Iroh lookup crates `iroh-mainline-address-lookup` and `iroh-mdns-address-lookup` 0.6 are used by `locust-net`. Operator settings and the current runtime evidence boundary are in the [T1 run guide](t1-run.md).

## Local API

[`api.rs`](../crates/locust-proto/src/api.rs) defines what the CLI, the stdio MCP bridge and client adapters ask of the daemon over its Unix socket. [`client.rs`](../crates/locust-proto/src/client.rs) is the client side.

### Connection

A connection opens with one `ClientHello`: the API version first (and first in every future layout), a credential, and optionally a session secret. The daemon answers with one `ServerHello`. `Welcome` carries the API version, the daemon's own version, the caller the credential resolved to and `max_blob_bytes`, the largest content object this daemon stores, counted as sealed bytes (operators may set it below 64 MiB). `Refused` carries an error and the daemon's API and own versions, and its layout never changes, so a client of any version can read why it was refused. A hello that opens with another API version is refused as `unsupported_version` whatever follows, because its layout is not known; a hello of this version that does not decode is `invalid`; an unknown or revoked credential is `denied`.

After the hello the client sends `RequestFrame`s: a client-chosen number, an optional idempotency key, an optional principal the owner acts for, and one request. Exactly one `ResponseFrame` answers each, in order, matched by number. Hello frames are read with a 4 KiB limit and every later frame with the content limit plus 64 KiB. A frame that does not decode has no number to answer under, so the daemon closes the connection. A request that blocks, such as `wait`, occupies its connection until it is answered; a client that wants to keep working opens a second connection with the same credential and session.

With an idempotency key, repeating a request that succeeded returns its first result instead of running it again; a request that failed left nothing behind and runs again. Keys are scoped to the caller, and reusing one for a different request fails with `idempotency_mismatch`. `daemon.stop` is a lifecycle exception: it is executed for the current process and is not persisted as an ordinary cached response, so reusing a stop key after restart still stops the new daemon.

The blocking client opens over any byte stream, sends the hello, then makes one call at a time. It accepts only a response with the request's number and of the kind that request gets (`Request::is_answered_by`). A request too large for one frame is refused before anything is sent and leaves the connection usable, as does an error from the daemon; after any other error the client must be dropped.

### Callers

The credential in the hello fixes the *caller* for the whole connection. A *credential* is a local 32-byte bearer secret; it scopes and attributes requests, and is not protection against another process that can read the same user's files. Whoever generates a credential writes it to its file first, and the daemon stores only its digest.

- The **owner** is the local participant operating this daemon, not the goal owner of a genesis record. It enrolls and revokes principals, enrolls viewers, sets grants, authorizes single assignments no grant covers, and stops the daemon. It authors no events. Directly it may make the owner-only requests, every read-only request and `session.drop`. For any other request it names an enrolled principal in the frame's `on_behalf`; that direct act is the authorization, and the principal's grants are not consulted. `on_behalf` from any other caller is `denied`, naming an unknown or revoked principal is `not_found`, and naming one on an owner-only request is `invalid`.
- An **agent** is one enrolled principal acting within its grants. It sees only the goals it is part of; to it, any other goal does not exist (`not_found`).
- A **viewer** is a read-only credential for one enrolled principal, enrolled by the owner with `viewer.enroll`. It sees exactly what that principal sees and changes nothing. It may make every read-only operation and is answered as the principal would be; every other request is `denied`, the session operations that write (`session.report`, `session.drop`) and `workspace.set` included, and so is naming anyone in `on_behalf`. It is never an execution session: a hello that presents a viewer's credential with a session secret is refused as `invalid`, so a viewer holds no claim and its pending work lists none. Its reads store nothing: it has no feed cursor (`events` reads after the position it passes, or from the start, and moves no cursor, the principal's included), its `blob.get` of an object not held is `unavailable` without noting a want, and its idempotency keys are not recorded, so a repeated read runs again. Wherever this document speaks of the calling principal, a viewer's is the principal it was enrolled for.

Every goal-scoped request names its goal. There is no current or default goal, and a missing credential never selects a wider scope. Enrollment (`agent.enroll`) carries the digest of a credential the client generated and already stored, so no secret is ever in a request or a response; the answer is the new principal's key, whose signing half the daemon generates and holds. Names are unique on a daemon and are 1 to 32 characters from `a-z`, `0-9` and `-`, because a name is also a file name. `viewer.enroll` likewise carries only the digest of a viewer credential the client already stored. A principal may have several viewers; repeating the request with the same principal and digest changes nothing; an unknown or revoked principal is `not_found`, and a digest that is already the credential of anything else is `conflict`. Revoking a principal stops its credential and those of its viewers at once; no request revokes one viewer alone. A revoked principal's name stays taken and its key stays attributed in history.

### Grants

*Grants* are what the local participant lets a principal do without asking again. `Grants { manage_goals }` is daemon-wide and covers creating goals, issuing invitations, joining and leaving. `GoalGrants { execute, decide, takeover }` is one goal's standing policy for one local principal, set by the owner with `goal.grant`: `execute` takes work assigned to the principal, `decide` signs coordinator decisions in a goal it coordinates, and `takeover` replaces another session's claim (`execute` does not imply it). Creating a goal gives its creator `decide` and nothing else; joining gives nothing, so new work waits for the owner. The owner authorizes one assignment with `task.authorize`, optionally including takeover, for the life of that assignment.

A request that is the caller's to make but that no grant covers fails with `authorization_required` and waits for the owner, who answers with `task.authorize`, with `goal.grant`, or by making the request on the principal's behalf. A request that is not the caller's to make at all is `denied`.

### Sessions and claims

A *session* is one execution session of a coding client. Its launcher or bridge generates a 32-byte session secret, keeps it in a file the model never reads, and presents it in the hello; the connection is that session for its whole life. The session's public handle, its `InstanceId`, is the first 16 bytes of the secret's domain-separated digest; it proves nothing, and holding the secret is what recovers a claim after a lost response or a restart. No request carries a session or instance field, so a second session of the same principal cannot act as the first by naming its handle. A session belongs to one principal: the first request that uses it (a session record or a claim) binds it, and using it as another principal is `denied`. Without a session the connection can read and coordinate but cannot hold a claim.

A *claim* binds an assignment to the session that took it and carries a *generation*: 1 for the first claim, raised by one at each takeover. Claims are local daemon state, not protocol events.

- `task.claim` takes an assignment for the connection's session, or returns the claim that session already holds with the same generation, so a retry after a lost response is safe. It needs a session (`invalid` without) and the `execute` grant or an authorization for the assignment (`authorization_required` without). Another session holding the claim is `claim_held`; an assignment that is no longer current is `superseded`.
- `task.takeover` replaces another session's claim, raises the generation and fences the earlier holder. It needs the `takeover` grant or an authorization that includes takeover; `execute` is not enough. If the calling session already holds the claim it is returned unchanged, so a retry does not raise the generation twice. Nobody holding a claim is `conflict`: use `task.claim`.
- `task.progress`, `task.submit` and `task.fail` are claim-bound: they name the generation, and the daemon requires that the connection's session holds the claim and that the generation is current, otherwise `superseded`. So a delayed write from a replaced session fails instead of landing.
- `cancel.acknowledge` is answered by the session holding the claim, naming its current generation, when a claim exists; when none exists, any connection of the assignee answers with no generation.
- `task.decline` is possible only before the assignment is claimed.

### Session records

A client adapter keeps one typed record per session through the daemon, stored under the session's handle. A `SessionRecord` holds the client and its version as the adapter reports them, the session's state (`launching`, `started`, `ready`, `blocked`, `exited` or `unknown`), the client's own session identifier used to resume it (which proves nothing to the daemon), six capability flags reported independently (`tools`, `active_delivery`, `idle_wake`, `manual_resume`, `recovery`, `confinement`), and up to 64 KiB of `detail` that is opaque to the daemon and owned by `locust-adapter`. The two strings are at most 4,096 bytes each.

`session.report` writes the connection's own record, replacing any earlier one; it needs a session in the hello, and it is how a launcher makes its launch intent durable before spawning the client. Only the session itself writes its record. `session.show` reads the connection's own record when no handle is given, or a named session's record, which must belong to the caller's principal unless the caller is the owner; any connection of the same principal may read it, because a session deciding on a takeover has to see whether the holder exited. `sessions` lists the records of the caller's principal, or all of them for the owner. `session.drop` deletes the connection's own session's record, or any for the owner, and is `conflict` while the session still holds a claim on an unfinished assignment, so a claim is never left without a visible holder. A view adds the session's handle, its principal, when the record was last written, whether a connection that presented its secret is open, and the claims it holds. None of the session operations is offered to the model as a tool; they belong to the launcher and the adapter.

### Reading before deciding

A coordinator reads what it decides on through the same operations everyone uses. `event.show` returns one event in full: its feed entry (identifier, author, kind, author's clock and *standing*, which is `effective`, `pending` or `excluded`), its anchor, its typed body, its payload reference, the payload as text when held and within the reader's epoch entitlement, the task it concerns, and for each content object it names, in `Header::blobs` order, its reader-visible availability. `task.show` returns a task's board entry, text and authored policy, and any unanswered cancellation. `doc.read` returns a document's accepted revision and text and the revisions proposed against it. `blob.stat` reports for up to 256 hashes whether each object is `held`, `requested`, `unavailable` or `unknown`, without transferring anything.

A task's state on the board is one of `proposed`, `assigned`, `taken`, `declined`, `failed`, `submitted`, `accepted`, `rejected`, `cancel_requested` and `cancelled` with its outcome. States reported by the worker are shown as reported; only `accepted` and `rejected` reflect a coordinator decision on a result. Each board entry also says whether the task's result is *applied*: true when its accepted result carried a workspace head and the calling principal's workspace binding on this machine records that head, or a head accepted after it, as integrated. Submitted, accepted and applied are the three outcomes a result goes through. Applied is derived each time it is read and is local to this machine and principal; it is false when the accepted result named no head, when the binding records a manifest that is not an accepted head of the goal, and for the owner asking directly, who has no binding.

### Content and workspace

The local API carries plaintext. `blob.put` stores bytes the client selected as content of a goal: the daemon seals them and answers with the hash of the stored, sealed object, which is the name events and manifests use, or `limit_exceeded` when the sealed object would exceed `max_blob_bytes`. The daemon never opens a path a client names; the trusted CLI or adapter reads the file and sends its bytes. `blob.get` returns plaintext; when the object is not held it fails at once with `unavailable`, notes the want and asks peers (`blob.stat` shows when it has arrived), and when nothing in the goal names the hash it is `not_found`. `blob.withdraw` stops serving a locally held object to peers and to local callers; events that name it stay valid.

A `WorkspaceBinding` records where one local principal's files for one goal live: the directory it shares from, the Git commit of the last export (provenance only), the manifest of the last export, the separate directory received snapshots are materialized into, and the accepted head last integrated into the participant's own files. It is local and never replicated; the daemon stores the strings verbatim, each at most 4,096 bytes, and never opens the paths. `workspace.set` replaces it. The trusted CLI records a head as integrated after it has applied that head to the participant's files; the daemon stores what it is given and never checks it against the files.

`goal.status` shows the goal as this daemon holds it: title (once the founding text is held), coordinator key, latest decision applied locally, current members with the endpoint each is bound to and whether it is local, whether the goal is halted, the *accepted head* (the manifest named by the latest accepted result that carried one), the caller's workspace binding and grants, and each peer daemon with whether a link is up and when it last synchronized. `status` shows the daemon's version and endpoint (absent until the transport runs), its principals and, per goal and local principal, how the principal stands: `joining`, `member`, `removed`, `left` or `refused`. A joining/refused principal without prior canonical admission sees its own join state but no title or goal contents. The owner sees every principal and goal; an agent sees itself and its own goals.

A goal is *halted* on this daemon for one of two reasons. `authority_conflict`: the coordinator's history conflicts with itself, so the goal stays readable and exportable and requests that sign fail with `halted`. `signer_recovery`: this daemon's signer for the goal is in restore recovery and cannot yet show what it already signed, so requests that sign fail with `read_only`; reads still work.

### Waiting and the feed

*Pending work* is what needs the caller in one goal, computed from task state and local records each time it is asked and returned as identifiers only, each item naming its task: assignments to authorize, assignments the caller may claim now, claims this connection's session holds (with the generation its claim-bound requests must name), assignments of the caller's principal whose claim another session holds (which this connection can only take over), cancellations this connection may answer, and results waiting for the caller's decision as coordinator. The claims list is about the connection's own session, so it is empty for a connection without one and for a viewer. For the owner asking directly only the assignments to authorize are filled, across every local principal. It never depends on a notification having been delivered.

Every goal has a change counter (`revision` in the answer to `pending`; unrelated to document revisions). The daemon raises it in the same commit as any change to the goal's events or to a local record that feeds pending work, and it starts at 1. `wait` holds the question open: it answers `work` with the pending work as soon as the counter differs from the caller's `seen`, which may be at once (a `seen` of 0 asks for the current state), and answers `no_event`, or `disconnected` when no peer of the goal is reachable, only when its timeout has passed. A change between two calls is therefore never missed.

The *feed* (`events`) lists a goal's events oldest first, at most the requested limit and at most 256. Feed positions number events from 1 in the order they took effect on this daemon, as `effective` or as `excluded`; a `pending` event has no position yet. Feed positions are local and are not the storage log's positions, which follow arrival. Passing a position acknowledges everything up to it and stores it as the caller's cursor for the goal; passing none resumes from the stored cursor, or from the start, without moving it. The cursor belongs to the caller, so sessions of one principal share it; a reader that needs its own position passes one. A viewer has no cursor.

### Names and text forms

Operation names are stable: they are the command names, the request's JSON tag, and the names in logs and errors. JSON output renders the public types: identifiers as hex, enum values by stable snake_case names, and a request as `{"<operation name>": {fields}}` (the bare name when it has no fields). Requests and request frames refuse unknown fields. An MCP tool is `locust_` plus the operation name with dots replaced by underscores. Credentials, session secrets, invitation secrets and content keys render as `<redacted>` and are never read from text; the one capability JSON shows is an invitation ticket, where it is being handed over. An error message is written by the daemon and never contains peer-written text; clients branch on the code, never on the message.

### Operations

`OPERATIONS` lists, one per request variant in wire order, each operation's name, whether it is read-only, whether it names a goal, its audience, whether the stdio bridge offers it to the model as a tool, and a one-line summary; a test keeps the table and the request type in agreement. *Read-only* means the operation authors no event and changes nothing another caller can observe; `events` moves only the caller's own cursor and counts as read-only. The *audience* is who may make the request: **owner** is the owner credential only, never on behalf of a principal; **agent** is any enrolled principal within its grants, or the owner (on behalf of a principal unless the operation is read-only); **coordinator** is the principal that coordinates the goal, holding the `decide` grant, or the owner on its behalf. A viewer is outside these audiences: it may make exactly the read-only operations, every one of which has the agent audience. An operation that is not a tool is owner-only, carries raw bytes, or manages session records. The first eleven operations and the last are daemon-wide; the rest name a goal. The answer is the response variant's JSON tag: `recorded` carries the identifier of the signed event, which identifies what the request created.

| Operation | Audience | Read-only | Tool | Answer |
|---|---|---|---|---|
| `status` | agent | yes | yes | `status` |
| `daemon.stop` | owner | no | no | `done`, then the daemon stops |
| `agent.enroll` | owner | no | no | `agent_enrolled` |
| `agent.grant` | owner | no | no | `done` |
| `agent.revoke` | owner | no | no | `done` |
| `session.report` | agent | no | no | `done` |
| `session.show` | agent | yes | no | `session` |
| `sessions` | agent | yes | no | `sessions` |
| `session.drop` | agent | no | no | `done` |
| `goal.create` | agent | no | yes | `goal_created` |
| `goal.join` | agent | no | yes | `joined` |
| `goal.invite` | coordinator | no | yes | `invited` |
| `goal.leave` | agent | no | yes | `recorded` |
| `goal.grant` | owner | no | no | `done` |
| `goal.status` | agent | yes | yes | `goal_status` |
| `member.remove` | coordinator | no | yes | `recorded` |
| `workspace.set` | agent | no | yes | `done` |
| `board` | agent | yes | yes | `board` |
| `task.show` | agent | yes | yes | `task` |
| `event.show` | agent | yes | yes | `event` |
| `task.propose` | agent | no | yes | `recorded` |
| `task.assign` | coordinator | no | yes | `recorded` |
| `task.cancel` | coordinator | no | yes | `recorded` |
| `task.authorize` | owner | no | no | `done` |
| `task.claim` | agent | no | yes | `claimed` |
| `task.takeover` | agent | no | yes | `claimed` |
| `task.decline` | agent | no | yes | `recorded` |
| `task.progress` | agent | no | yes | `recorded` |
| `task.submit` | agent | no | yes | `recorded` |
| `task.fail` | agent | no | yes | `recorded` |
| `cancel.acknowledge` | agent | no | yes | `recorded` |
| `result.accept` | coordinator | no | yes | `recorded` |
| `result.reject` | coordinator | no | yes | `recorded` |
| `pending` | agent | yes | yes | `pending` |
| `wait` | agent | yes | yes | `waited` |
| `events` | agent | yes | yes | `events` |
| `note.add` | agent | no | yes | `recorded` |
| `notes` | agent | yes | yes | `notes` |
| `doc.read` | agent | yes | yes | `doc` |
| `doc.revise` | agent | no | yes | `recorded` |
| `doc.accept` | coordinator | no | yes | `recorded` |
| `blob.put` | agent | no | no | `blob_stored` |
| `blob.get` | agent | yes | no | `blob` |
| `blob.stat` | agent | yes | yes | `blob_states` |
| `blob.withdraw` | agent | no | yes | `done` |
| `viewer.enroll` | owner | no | no | `done` |

`goal.create` founds a goal that the calling principal owns and coordinates, committing genesis and the coordinator's self-admission, and needs `manage_goals`, as do `goal.invite`, `goal.join` and `goal.leave`. For a remote endpoint, `goal.join` records intent and asks the inviter; its answer names the goal and coordinator and says `joining` until admission arrives. Own-endpoint redemption is checked and committed immediately and can return `member`. Repeating the same pending ticket returns its current state; repeating the same refused ticket is `denied`. A non-coordinator's `goal.leave` signs a `leave_requested` contribution and stops that principal taking part at once; coordinator leave is refused. `member.remove` signs `member_removed` and refuses coordinator self-removal. `result.accept` with a head whose result's base is not the accepted head is `conflict`, as is `doc.accept` of a revision whose base is not the accepted revision. Every request that signs an event fails with `halted` or `read_only` while the goal is halted. `Request::check` applies the name rule and the count and string limits before anything else, and a client may run it to fail before a round trip.

### Error codes

| Code | Meaning |
|---|---|
| `denied` | Not the caller's to make: unknown or revoked credential, not the owner, coordinator, assignee or member the request is for, a missing daemon-wide grant, a viewer making a request that is not read-only, or an invitation the inviter refused |
| `not_found` | The named thing does not exist, or is in a goal the caller is not part of |
| `invalid` | Malformed, or needs something the connection lacks: a session for a claim-bound request, a principal for the owner |
| `conflict` | A precondition no longer holds: a stale base, a task already reassigned, a name already taken |
| `claim_held` | Another session holds the claim |
| `superseded` | The caller's claim, generation, attempt or assignment is not current |
| `idempotency_mismatch` | The idempotency key was already used for a different request |
| `limit_exceeded` | A published or configured limit is exceeded |
| `unavailable` | Needed content or a needed peer is not reachable now |
| `halted` | The goal's authority history conflicts, so nothing more is signed for it |
| `unsupported_version` | The client, a ticket or an event uses a version this daemon does not speak |
| `corrupted` | Local state failed an integrity check |
| `internal` | The daemon failed (storage, or a fault of its own); retrying may succeed |
| `authorization_required` | Allowed once the local participant authorizes it: a claim, takeover or decision that no goal grant covers |
| `read_only` | This daemon's signer for the goal is in restore recovery and signs nothing; reads still work |

Contract errors convert to these codes: a storage failure is `internal` and stored data that fails an integrity check is `corrupted`; another protocol version in an event or a ticket is `unsupported_version`; a stored event that no longer matches its identifier is `corrupted`; any other structural fault in an event, a manifest, a ticket or a frame's bytes is `invalid`; a value that cannot be encoded is `internal`.

### The engine seam

[`engine.rs`](../crates/locust-proto/src/engine.rs) is the seam between the daemon's I/O shell and the state machine. The shell owns sockets, timers and the clock; the engine owns every decision and all state. Its synchronous store operations may block the dedicated engine thread; network I/O stays in the shell. One dedicated thread owns the engine and calls it in the order inputs arrive, and the shell's I/O tasks exchange messages with that thread over channels, which keeps the daemon single-writer without a lock. The engine never reads a clock or a random source: the shell passes the time into every call and supplies random bytes through `Entropy`. Both sides of the seam are plain values in and out, so a test drives several engines against each other with no sockets.

The local side is `Engine`. It answers a hello (`connect`) and each request (`request`) with either a reply or a *parked* wait naming the request number, the goal and the timeout. The shell calls `resume` when that goal changes, which it learns from `take_changed` after every call into the engine, or when the timeout passes; with the timeout passed the engine always replies. A connection has at most one parked request, because its requests are answered in order. `disconnect` forgets a connection; claims held by its session stay with the session.

The transport's side is `PeerEngine`. The engine creates the 32-byte endpoint secret on first start and keeps it in its store, and the transport derives its identity from it. Each exchange is numbered by the side that creates it: `Dialed` by the engine, `Accepted` by the shell. The shell passes in what happened on the network: the transport's endpoint and contact hints (the hints are what an invitation carries), an exchange the engine asked for opened or failed to open, a peer opened an exchange from an authenticated endpoint that is not yet a member of anything, the next frame of an exchange, an exchange closed, and a poll that the shell sends after local requests changed a goal and at least once a second, so the engine can start exchanges and act on its own deadlines. The engine answers with outputs the shell carries out in order: open an exchange to an endpoint (hints may be empty, because finding an endpoint by its key is the transport's job), send a frame, *admit* an accepted exchange whose remote endpoint speaks for a member so its later frames are read at the peer limit (exchanges this daemon opened are read at the peer limit from the start), and finish an exchange once everything sent on it was acknowledged. The shell sends `Writable` only after a frame has been written, allowing the engine to produce the next frame under transport backpressure. `Finished` confirms acknowledged stream completion; `Closed` reports an aborted or failed exchange and cannot advance successful synchronization status. `Connection` reports actual connection up/down state separately from an exchange result. An incoming frame is processed through the engine and its admission output applied before the receiver reads the next prefix. These ordering rules are enforced by the [network shell](../crates/locust/src/daemon/network.rs), [worker](../crates/locust/src/daemon/worker.rs), and [sync driver tests](../crates/locust-core/src/sync/tests/driver.rs). A frame from a peer can answer a parked wait, so the shell calls `take_changed` after each peer input too.

`PeerOutput::Evidence` grants only the bounded proof read limit. `PeerEngine::peer_readable` provides input backpressure until the complete response drains. `Engine::stop_requested` includes fatal storage failures; `Engine::failure` carries the error to the shell, which removes its socket, releases its lock and reports failure. Diagnostic writes are fallible and cannot abort the daemon merely because stderr is closed. Startup keeps typed errors: storage corruption is exit 11, incompatible schema/protocol is exit 10, and a held daemon lock is exit 8.

## Local conventions

[`local.rs`](../crates/locust-proto/src/local.rs) is the one definition of the names shared by the daemon, CLI, bridge, hooks and installer. Everything there is a constant or a pure function of values the caller read from the environment and the file system.

The *state directory* is `$LOCUST_HOME`, or `~/.locust` when that is unset or empty, with mode 0700; `LOCUST_HOME` and `HOME` must be absolute paths, because processes with different working directories would otherwise resolve them differently.

| Path in the state directory | Holds |
|---|---|
| `daemon.sock` | The daemon's Unix socket |
| `daemon.lock` | Lock held by the one daemon that runs on this directory |
| `locust.db` | The store's database |
| `blobs/` | Content objects kept as files |
| `logs/` | Daemon and adapter logs |
| `owner.credential` | The owner's credential |
| `agents/<name>.credential` | One enrolled principal's credential |
| `sessions/<instance>.secret` | One execution session's secret, named by its public handle in hex |

The socket path may be at most 103 bytes on every platform: macOS limits a Unix socket path to 104 bytes including its terminator, the tightest of the supported platforms, and applying it everywhere keeps one state directory valid on all of them. A too-long path is refused with a message to choose a shorter `LOCUST_HOME`.

A credential or secret file holds exactly the 32 secret bytes and nothing else, has mode 0600, and is written before the daemon is told about the secret, so a secret the daemon knows is never lost. A file of any other length is refused, not trimmed or padded. `LOCUST_CREDENTIAL` names the credential file a client presents; it has no default, so a client without it has no credential and never falls back to the owner's. `LOCUST_SESSION` names the session secret file; unset or empty means the client is not an execution session. Both hold absolute paths, never secret values. A principal's name becomes a file name, so a name that fails the name rule is refused.

## Command contract

These are the names and outputs of the `locust` binary that the installer, the operating skill, the first-contact guide and the three-machine test depend on. The T1 CLI commands are implemented; generic `call` exposes other typed operations. This does not imply the production MCP bridge or client launcher is implemented.

### Version

`locust --version` prints one line: `locust <semver> (<commit>) api <n> protocol <n>`. `<commit>` is the 12-character Git commit the binary was built from, with `-dirty` appended when the tree had local changes, or `unknown`. It is embedded at build time by a build script that asks Git, and can be overridden with the environment variable `LOCUST_BUILD_COMMIT` for builds from an archive.

### Global options

- `--home <dir>`: state directory; overrides `LOCUST_HOME`.
- `--credential <file>`: credential file; overrides `LOCUST_CREDENTIAL`.
- `--owner`: act as the local participant, using `owner.credential` in the state directory.
- `--as <principal>`: with `--owner`, perform the request on behalf of an enrolled principal, given by name or by key.
- `--session <file>`: session secret file; overrides `LOCUST_SESSION`.
- `--json`: machine-readable output.
- `--idempotency-key <hex>`: 16 bytes in hex; a repeated request returns its first result.

Resolution follows the local conventions: no credential option and no `LOCUST_CREDENTIAL` is an error, never a silent fall back to the owner.

### Commands

A command is the operation name from `OPERATIONS` with dots written as spaces, so `task.claim` is `locust task claim`. Field values are flags named after the fields; a goal is always `--goal <id>`. A goal may be given by a unique prefix of at least 8 hex characters, resolved by the CLI from `status`; every other identifier is given in full. Where a request carries text (`task propose`, `note add`, `task submit`, `doc revise`), the text is an argument, or is read from standard input when the argument is `-`. `goal join --ticket -` reads the ticket from standard input; `goal invite --expires-ms` takes a future absolute Unix timestamp in milliseconds. `--help` describes commands/options, and `--version` composes with `--json`. Human membership and halt output use stable snake_case tags.

Commands that are not single API operations, or that do more than send one:

- `locust daemon run` runs the daemon in the foreground; this is what a service unit executes. It creates the state directory with mode 0700 if missing, takes `daemon.lock` (a second daemon on the same directory exits with status 8 and says which directory is in use), creates `owner.credential` on first start, listens on `daemon.sock`, and stops cleanly on SIGINT, SIGTERM or `daemon stop`.
- `locust daemon stop` asks the running daemon to stop (operation `daemon.stop`, owner only).
- `locust status` is the operation `status`; see readiness below.
- `locust doctor` runs readiness checks; see below.
- `locust agent enroll <name> [--manage-goals]` is owner only. It generates a credential, writes it to `agents/<name>.credential` with mode 0600, then enrolls its digest. It prints the principal key and the credential path, never the secret. Repeating it with the same credential and grants is safe; a revoked principal or different grant request returns `conflict`. Change grants explicitly with `locust --owner agent grant --agent <key> --manage-goals true|false`.
- `locust call <operation> [json]` sends any operation with its fields given as one JSON object, or `-` for standard input. It is the escape hatch for operations without a dedicated command.

### Output

Without `--json`: short human text on standard output and diagnostics on standard error. Identifiers are printed in full, because they are meant to be copied.

With `--json`: exactly one JSON object on standard output and nothing else.

- Success: `{"ok": true, "result": <the response rendered as JSON>}`.
- Failure: `{"ok": false, "error": {"code": "<stable code>", "message": "<text>"}}`.

The codes are the error codes above. A failure before any frame exists uses the same shape: a daemon that does not answer is `unavailable`, and a hello refused for its version is `unsupported_version`.

### Exit status

| Status | Meaning |
|---|---|
| 0 | Success. For `wait`: there is work |
| 1 | `internal`, or an unexpected failure |
| 2 | Usage error (bad arguments) |
| 3 | `denied` |
| 4 | `authorization_required` |
| 5 | `not_found` |
| 6 | `invalid` |
| 7 | `conflict`, `claim_held`, `superseded`, `idempotency_mismatch` |
| 8 | `unavailable`, including a daemon that is not running or not answering |
| 9 | `halted`, `read_only` |
| 10 | `unsupported_version` |
| 11 | `corrupted` |
| 12 | `limit_exceeded` |
| 20 | `wait` only: nothing changed before the timeout |
| 21 | `wait` only: nothing changed and no peer of the goal is reachable |

### Readiness

`locust status` separates three states: the daemon is not answering (status 8, code `unavailable`, and the message names the socket path); the daemon speaks another API version (status 10, code `unsupported_version`, and the message names both versions); a harmless call succeeded (status 0, and the daemon's status).

`locust doctor` runs these checks in order and reports each as `{"name", "ok", "detail"}`: the state directory exists with mode 0700; the socket path fits the 103-byte limit; the daemon lock is held by a running process; the socket accepts a connection; the hello succeeds and the versions match; a harmless call (`status`) succeeds; the credential file named by the environment or the options is readable and has mode 0600; and the session file, when one is named, likewise. The exit status is 0 when every check passes and 1 otherwise. With `--json` the result is `{"ok": <all passed>, "result": {"checks": [...]}}`.

## Storage seam

[`store.rs`](../crates/locust-proto/src/store.rs) is the seam between the state machine and durable storage. The state machine computes a `Commit`: the structurally valid events it has decided to retain, the content objects to hold, writes to local records, and content objects to stop holding. One dedicated thread owns the store, so the trait is synchronous and needs no lock.

**Commit semantics.** A store applies a commit entirely or not at all, as one durable step. Within it, events and content objects are applied first, then local writes in order, then removals last. Events and objects already held are left untouched, so replaying a commit changes nothing. An event may name content that is not held. Every content object is durable before any record that names it becomes visible.

**Positions.** Events not already held are appended to their goal's storage log in the order they appear in the commit: the n-th event ever stored for a goal has position n, counted per goal from 1 with no gaps, so the caller knows every new position without reading back. Positions follow local arrival order, strictly increase and never change, so a position is a durable cursor. They are neither an author's sequence numbers nor feed positions. `author_log` lists one author's events in ascending (position in the author's log, identifier) order after a cursor of that pair; conflicting events at one position are all kept and returned, and paging visits every event exactly once however many share a position. The state machine rebuilds task, membership and frontier state in memory by replaying the log, so that state cannot disagree with the log after a crash.

**Durability.** What a store reports as durable survives power loss, not only a process crash (SQLite: WAL, `synchronous=FULL`, and `fullfsync` on macOS). The daemon releases its own signed events to peers once a commit returns; a commit lost after that would make it sign a second event at the same position, which peers see as a forked history.

On reopen, the SQLite store holds exclusive ownership and completes a FULL WAL checkpoint/durability barrier before exposing recovered state or collecting objects. It validates checkpoint completion and refuses a failed barrier. Protocol compatibility is checked before logical migration/GC. Staged files and their directory are synchronized during recovery. The [store tests](../crates/locust-store/src/tests.rs) and [optional macOS syscall regression](../crates/locust-store/tests/recovery_flush.rs) enforce these ordering/failure rules; fault injection is not a physical power-loss qualification.

A failure before applying a mutation leaves nothing applied. A failure while a commit, staged write, promotion or discard is being made durable can leave an unknown persisted outcome: the handle refuses further use until reopened and repaired. The daemon therefore shuts down before signing again. Promotion syncs bytes before publishing them and preserves the original staged name until the database row is durable; a pre-commit failure cannot consume its only recoverable copy. Redundant staging of an already held object survives reopen, matching `MemStore`. These rules are enforced by [store](../crates/locust-store/src/store.rs), [file operations](../crates/locust-store/src/files.rs), [recovery](../crates/locust-store/src/connection.rs) and [conformance](../crates/locust-proto/src/store.rs).

**Staging and removal.** `stage_blob` appends a chunk of an object being received only when its offset equals the staged length, and returns the staged length either way, so a repeated or misplaced chunk changes nothing and tells the caller where to resume. `finish_blob` promotes the staged bytes to a held object if they hash to the expected hash and discards them otherwise; either way nothing remains staged. Both are durable like a commit when they return. A commit's `drop_blobs` stops holding objects, absent ones ignored; events that name them are kept. `blob_len` and `blob_range` serve one transfer chunk without reading the whole object.

**Local records.** Local, non-replicated records are opaque key and value byte strings in named spaces; the crate that owns a space defines their encoding, and an empty value is a value. Content objects themselves are held apart from these records; the `Blob` space holds only what the daemon knows about them.

| Space | Holds |
|---|---|
| 0 `Identity` | Daemon identity, endpoint key and signing watermarks |
| 1 `Agent` | Enrolled principals: keys, names, grants and credential digests |
| 2 `Goal` | Per-goal local settings: workspace binding, standing grants, halt state |
| 3 `Peer` | Known peers: contact hints and last successful synchronization |
| 4 `Invite` | Issued invitations and their redemption state |
| 5 `Claim` | Session-bound claims and their generations |
| 6 `Cursor` | Consumer positions in a goal's feed |
| 7 `Idempotency` | Request keys with the digest and result of their first execution |
| 8 `Pending` | Obligations that cannot be rediscovered from the event log |
| 9 `Session` | Session records of client adapters, keyed by instance |
| 10 `Blob` | Which goals a held content object belongs to, withdrawn marks and retention |
| 11 `Key` | Content keys per goal and key epoch |

**Conformance.** `MemStore` is the reference implementation, held in memory and not durable. `store::conformance` (feature `testkit`) states the behavior every store shares: `run` checks it against fresh stores, and `run_reopen` checks the behavior that spans a restart against stores reopened over one persistent state. The SQLite store in `locust-store` runs both.

## Limits

[`limits.rs`](../crates/locust-proto/src/limits.rs) publishes the sizes checked before input is allocated or retained. Byte limits apply to a frame, a header or a ticket before it is buffered or decoded; count limits apply to the decoded value before it is acted on or retained, and the enclosing frame's byte limit bounds what decoding can allocate. Violations are rejected and reported, including for input from admitted members; nothing is silently truncated.

| Limit | Value | Where |
|---|---|---|
| Historical halt-proof frame | 33,024 bytes; no ordinary admission implied | `daemon/network.rs` |
| Signed header | 16 KiB (16,384 bytes) | `limits.rs` |
| Causal parents per event | 64 | `limits.rs` |
| Task dependencies per proposal | 64 | `limits.rs` |
| Output objects per result | 64 | `limits.rs` |
| Event payload, sealed bytes | 1 MiB (1,048,576 bytes); at least 45 | `limits.rs`, `seal.rs` |
| Content object, sealed bytes | 64 MiB (67,108,864 bytes) ceiling; operators may lower it, and the hello reports the effective value | `limits.rs` |
| Plaintext of one content object | 64 MiB minus 45 bytes (67,108,819) | `seal.rs` |
| Sealing overhead | 45 bytes | `seal.rs` |
| Content bytes per peer chunk | 1 MiB | `limits.rs` |
| Events per sync batch | 256 | `limits.rs` |
| Authors per frontier | 4,096 | `limits.rs` |
| Points per inventory, identifiers per event request | 4,096 | `limits.rs` |
| Frame before the sender is identified (local hello, or a peer not yet known to speak for a member) | 4 KiB (4,096 bytes) | `limits.rs` |
| Peer frame once the remote endpoint speaks for a member | 256 × (16 KiB + 128) = 4,227,072 bytes | `limits.rs` |
| Local frame after the hello | 64 MiB + 64 KiB = 67,174,400 bytes | `limits.rs` |
| Encoded invitation | 4 KiB (4,096 bytes) | `limits.rs` |
| Ticket text | 8,206 bytes (prefix plus 8,192 hex characters) | `invite.rs` |
| Contact hints per invitation | 8, each at most 256 bytes | `invite.rs` |
| Manifest entries | 100,000 | `limits.rs` |
| Manifest path | 1,024 bytes, each component at most 255 | `limits.rs`, `manifest.rs` |
| Sequence number | `i64::MAX` | `event.rs` |
| Principal name | 1 to 32 characters from `a-z`, `0-9`, `-` | `api.rs` |
| Verbatim local string (client name, client session identifier, workspace path, commit name) | 4,096 bytes | `api.rs` |
| Session record detail | 64 KiB (65,536 bytes) | `limits.rs` |
| Hashes per `blob.stat` | 256 | `api.rs` |
| Events per feed page | 256; a larger requested limit is treated as 256 | `api.rs` |
| Socket path | 103 bytes | `local.rs` |
| Credential or secret file | exactly 32 bytes | `local.rs` |

## Golden vectors

A golden vector fixes exact bytes from published test keys (`testkit::keypair(n)` is the Ed25519 key with seed `[n; 32]`; `testkit::content_key(n)` is the content key `[n; 32]`). A change that makes one fail changes what existing peers and stored goals understand, so once a release exists it needs a new version, not an updated constant.

- [`vectors.rs`](../crates/locust-proto/src/vectors.rs): the goal owner's and a member's public keys and the goal identifier; the genesis event's header bytes, identifier and signature; the coordinator's self-admission (header bytes, identifier and signature) with endpoint `[7; 32]`; a task proposal whose payload is the text `Add a login page` sealed at epoch 0 under `content_key(1)`, with the sealed bytes, their hash, the header bytes, identifier and signature; the running log digest of the coordinator after founding; the frozen declaration index and name of every event kind, cancellation outcome and document; and an invitation ticket.
- [`seal.rs`](../crates/locust-proto/src/seal.rs), in its tests: the derived cipher and nonce keys for the key `00 01 .. 1f`, and the sealed bytes and hash of the empty plaintext and of the 150 bytes `0, 1, .. 149`, for the test goal at epoch `0x01020304` (four distinct bytes, so byte order is pinned). The same tests check the implementation against the construction written out independently with literal values.
- [`sync.rs`](../crates/locust-proto/src/sync.rs), in its tests: the log digest step against its documented input, and the wire index of every frame.

## Not decided here

- Coordinator handoff, and any exit from a halted goal other than inspection, export and a new goal.
- Verified streaming of content objects. Identity is plain BLAKE3, so it can be added without changing identifiers.
- The default relay and address-lookup operators, and what each observes; lane B's transport work records them.
- Signing and notarization of release builds. Until builds are signed and notarized, a binary is fetched with a command-line tool: one saved through a browser is quarantined by macOS, and an unsigned binary is then blocked.
