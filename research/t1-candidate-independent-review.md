# Independent review of the T1 release candidate — 2026-10-03

**Status: review findings. Nothing here was fixed, and no source under `crates/` or `scripts/` was changed by this review.** Requested by the owner, who asked the session that previously ran lane A to stay on as an independent reviewer after lane A's source moved to the shared orchestrator (see [workstreams](../docs/workstreams.md)). The subject is the runtime integrated in `885b372` and the release candidate built from `3422c7b`, which prints `locust 0.1.0 (3422c7b51948) api 0 protocol 0` and has SHA-256 `299aafb3c473d7d1051317a636640dfd8c68a52f0d2fe3317cf685b6d56ffbcf`. The crates are byte-identical between `3422c7b` and the head at review time; later commits changed documentation only.

## Summary

- **Nothing found blocks the run in the [T1 guide](../docs/t1-run.md).** Following that guide literally with the candidate, as three daemons in three private homes on one Mac, worked end to end, including the coordinator-offline step and the restarts.
- **The claims in the [integration findings](t1-integration-2026-10-03.md) reproduce.** Run independently at the head: formatting and Clippy clean, 391 Rust tests passed with none failed and five ignored, and [the harness](../scripts/check_t1.py) passed all 21 checks against the candidate.
- **36 defects were found and each was confirmed by a second reviewer who tried to refute it**: 10 at P2 and 26 at P3, none at P1. They concern situations outside the guide: two principals on one daemon, a member who signs conflicting events, revocation, a daemon killed during a flush, and a first-time user who makes a mistake.
- The findings that matter most before anyone other than the owner uses this: a local principal can read a goal it was never admitted to (IR-1), a join can push a goal's history to an endpoint nobody admitted (IR-2), and invitations keep working after their issuer is revoked (IR-3).

Severity as used here: P1 loses or corrupts data, breaks an authority or confidentiality rule in the guide's own flow, or stops the three-Mac run; P2 is a real defect with limited reach; P3 is a gap, an inefficiency or a rough edge.

## Method and limits

Seven reviewers each took one lens: goal state, node authority, reconciliation, the daemon process, the first-time user, the store, and secrets. Each worked read-only on a scratch copy made with `git archive`, wrote its own probes, and ran the candidate binary where behaviour mattered. A second reviewer per lens then tried to refute every finding and reproduced each one independently. Two reported P2 findings were lowered to P3 by their verifier (IR-19 and IR-25); none was refuted. Two findings were reported by two lenses and are merged here (IR-1 and IR-14).

The probes are preserved as patches that apply to `3422c7b` under [evidence](evidence/README.md), in `evidence/t1-candidate-review/`; see [Reproduction](#reproduction).

Not covered: separate machines and networks, sleep and wake, a published download, coding clients and the MCP bridge, `locust-workspace`, member removal at scale, and large content beyond a few megabytes. No physical power-loss test was run; the durability findings use a library interposer that fails or exits at a chosen flush.

## Worth knowing while running the guide

None of these stops the run. Each is something a person following the guide can trip over.

- **The two tickets look the same.** Two invitations from one goal share all but their last 66 characters. Joining with a ticket that another machine already redeemed answers `joining` with exit status 0, and the right ticket is then refused with `conflict` until the first refusal comes back, which took between 4 and 72 seconds in five trials (IR-10).
- **Enroll with `--manage-goals` the first time.** Enrolling a name without the flag and repeating the command with it succeeds and grants nothing. The way out is `locust --owner call agent.grant '{"agent":"<key>","grants":{"manage_goals":true}}'`; there is no `agent grant` command and the `denied` message does not say so (IR-25).
- **A second principal on the same Mac cannot join a goal that Mac coordinates.** The join stays `joining` forever (IR-4). The guide uses one principal per machine, so it does not arise there.
- **A fresh terminal needs the credential again.** Without `LOCUST_CREDENTIAL` or `--owner` the error is `LOCUST_CREDENTIAL is not set` and does not mention the options (IR-28).
- **`--help` describes nothing** and `--version` works only as the sole argument (IR-29).
- **Do not start the daemon twice.** The download command in the [build guide](../docs/t1-build.md) already starts the daemon that the run guide's first block starts again; the second one exits with status 8 (IR-31).
- **The ticket is on the command line**, where other accounts on the same Mac can read it with `ps`, and the guide's tickets never expire (IR-36).
- **`scripts/build_t1.py` needs Python 3.11 or later.** On the owner's machine `python3` is 3.10, so the AGENTS.md command `python3 -m unittest discover -s scripts/tests` reports one error; under `python3.13` all 63 tests pass. `scripts/check_t1.py` runs on 3.10.
- **A hypothesis, not verified:** the multicast-only failure recorded in the integration findings (`No route to host`, error 65, on multicast send) is the usual symptom of macOS refusing local-network access to the application that launched the process. The runs here and there were launched from inside the Claude desktop application. A daemon started from Terminal may prompt for local-network access the first time and then work. This was not tested, because every process this review can start has the same parent.

## P2 findings

| ID | Area | Finding |
|---|---|---|
| IR-1 | Node, confidentiality | A local principal that was never admitted reads a goal the daemon holds for another local principal |
| IR-2 | Sync, confidentiality | A join for a goal already held pushes the goal's signed history to the ticket's endpoint without admission |
| IR-3 | Node, authority | Outstanding invitations keep admitting members after the coordinator is revoked, loses its grants or leaves |
| IR-4 | Node | A second principal on the coordinator's own daemon can never join |
| IR-5 | Goal | One conflicting event from a member voids coordinator decisions, including other members' accepted results |
| IR-6 | Goal | A screening bound can drop the event a held decision names, so a late replica stalls for good |
| IR-7 | Sync | A chunk with an inflated total wedges an object so it is never fetched again |
| IR-8 | Sync | The responder builds every inventory answer at once, so a member can exhaust a peer's memory |
| IR-9 | Store | Opening trusts log frames that were never flushed, so a restarted daemon can publish an event that power loss then erases |
| IR-10 | CLI | A wrong or reused ticket answers `joining`, and the right ticket then conflicts for up to about 72 seconds |

### IR-1: a principal that was never admitted reads a co-hosted goal

On one daemon with principals `alice` and `bob`, `alice` creates a goal, adds a note and proposes a task. `bob` gets `not_found` for `notes` and `board`, as the contract promises. `bob` then runs `goal join` with any ticket naming that goal: an expired one, one already redeemed, or one built by hand from the goal identifier and the coordinator's key, both of which are public to anyone who has seen the goal named. The answer is `joining`, and from then on `notes`, `board`, `status` and `event show` return the decrypted text to `bob`, including text written after the inviter refused the join, and after a restart. The cause is that a join record, refused or not, counts as taking part ([access](../crates/locust-core/src/node/access.rs), [views](../crates/locust-core/src/node/views.rs), [invitations](../crates/locust-core/src/node/requests/invitations.rs)), and the daemon opens payloads with whichever key it holds. Reproduced on the candidate binary. *Fix:* reads of content need an admitted, removed or left principal; a join in progress shows only the ticket's own data; a refused join shows nothing.

### IR-2: a join pushes history to an endpoint nobody admitted

When the daemon already holds a goal and a local principal joins it with a ticket naming an arbitrary endpoint, the daemon dials that endpoint, sends `Hello` and `Join`, and treats any `Frontier` in reply as admission: it then sends its own frontier and every held event, and repeats this on every change and every anti-entropy round because the admission never arrives ([driver](../crates/locust-core/src/sync/driver.rs), [initiator](../crates/locust-core/src/sync/initiator.rs)). The endpoint receives every signed header: member keys and endpoints, the task graph, timestamps, payload hashes and sizes. It receives no keys and no plaintext. This contradicts the contract's rule that an endpoint that is not a member learns nothing. *Fix:* for a held goal, refuse a ticket whose endpoint is not the one the held state binds to the coordinator; and keep a joining exchange receive-only until the joiner's admission has been applied.

### IR-3: invitations outlive revocation

A coordinator issues two tickets. The owner removes its `decide` and `manage_goals` grants: its own `goal.invite` is now denied, yet a peer redeeming the first ticket is admitted. The owner then revokes the coordinator: a third daemon redeems the second ticket, is admitted, receives the epoch key and reads the goal. The same holds after the coordinator's own `goal.leave`. Nothing lists or cancels invitations, and the owner cannot act for a revoked principal to remove the new member. The admission path signs with the invitation's coordinator without checking that the principal is active, granted and taking part ([peers](../crates/locust-core/src/node/peers.rs), [authoring](../crates/locust-core/src/node/authoring.rs)). *Fix:* check all three when a join arrives, void a principal's invitations in the commit that revokes it, and add a request to cancel an invitation.

### IR-4: a second principal on the coordinator's daemon never joins

`carol`, enrolled on the same daemon as the coordinator, joins with a valid ticket and stays `joining`: through eight polls, minutes later, and after a restart. The driver dials the daemon's own endpoint, the transport refuses to connect to itself, and the join is retried for ever. The daemon logs nothing. The API models several local principals per goal. Reproduced on the candidate. *Fix:* redeem a ticket whose endpoint is this daemon locally, in the same commit.

### IR-5: a member's fork voids decisions about other members' work

Worker N completes three tasks and each is accepted with a workspace head. Member M's only contribution is proposing the second task. M then signs a second event at its position 0. On every replica, permanently: M's proposal is excluded, the assignment and acceptance that name it fail their precondition, N's result for it is excluded, and the third task's acceptance fails because its base is no longer the accepted head. Output before: `tasks [Accepted, Accepted, Accepted] heads [100, 101, 102]`; after: `tasks [Accepted, Submitted] heads [100]`. Removing M afterwards does not help.

This follows the rule the design fixed (a forked author's events from its fork point on grant nothing), which this reviewer wrote before the integration, so it is a consequence of the design rather than a mistake in the code ([fold](../crates/locust-core/src/goal/fold.rs)). The reach was not thought through: any member can undo accepted work that depends on something it authored. *Fix:* bound a fork the way removal is bounded. Events of the forked author that an applied decision names, and the events before them on that branch, stay judged; only events no decision depends on are excluded. This is a rule change and belongs in the [protocol contract](../docs/protocol-v0.md).

### IR-6: a screening bound drops an event that a held decision names

Screening keeps at most 16 different events per author position and at most 1,024 events past an author's usable prefix ([screen](../crates/locust-core/src/goal/screen.rs)). A member proposes a task, the coordinator assigns it and then admits a joiner, and the member signs further events at the same position until 16 of them sort below the proposal. A replica that held the proposal beforehand is fine. A fresh replica is offered the member's events in ascending order, keeps the first 16 and drops the proposal; the assignment then waits for an event the replica will never keep, and nothing after it is applied. If that replica is the joiner's, the join never completes. The same happens through the waiting bound with a fork followed by more than 1,024 events. Both need one member that signs conflicting events. *Fix:* exempt from both bounds any event that a held decision names.

### IR-7: an inflated total wedges an object

A member answers a request for a 94-byte payload with a chunk that claims a total of 4 MiB and carries 1 MiB. The receiver stages it, because the chunk is consistent with its own total, and the link drops. From then on every request for that object resumes at offset 1,048,576, which honest peers answer with `BlobUnavailable` because it is past the object's end, across reconciliation rounds and a restart. The note's text is never readable on that replica ([replica](../crates/locust-core/src/node/replica.rs)). *Fix:* when the goal names the object as a payload its length is known, so refuse a chunk whose total differs; and discard a staged copy when a resume is answered `BlobUnavailable`.

### IR-8: inventory answers are built at once

A member sends `Hello` and then 200 `InventoryRequest` frames of 34 bytes each for an author with 4,106 points, and grants no flow-control credit. The responder holds 200 fully built inventory frames, 27.8 MB for 6.8 KB received, about 4,000 times what was sent; 50 `Frontier` frames listing diverging authors leave 40.8 MB queued. The shell keeps reading while its writer is blocked, and the writer's deadline is per send. Event and object answers are already lazy; inventories are not ([responder](../crates/locust-core/src/sync/responder.rs), [outbox](../crates/locust-core/src/sync/outbox.rs)). *Fix:* make inventory answers cursors built when the transport has capacity, and bound the number of answers an exchange may owe.

### IR-9: open trusts frames that were never flushed

A process commits A, which is acknowledged, starts commit B (its own event at the next position) and dies at the flush of the write-ahead log, so B's frames are written but not flushed. A new process opens the store: a trace of the flush calls shows nothing flushes the log before the store reads it, so B is served from the page cache ([store](../crates/locust-store/src/store.rs)). The daemon then treats B as held and can release it to peers; a power loss before the next flush erases B, and after the restart the daemon signs a different event at that position, which every peer sees as a fork.

The window is narrow, and the verifier measured it on the candidate. The daemon must be killed during the roughly 4 ms flush of its own event. With the default bind the next start commits changed contact hints before any peer is served, and that flush makes the dead process's frames durable; the window stays open only when the hints are unchanged, for example with `LOCUST_BIND` fixed. A peer must then fetch the event, and power must be lost before the daemon's next commit. The guide's restarts use `daemon stop`, so it does not arise there. In the same probe, garbage collection on open deleted an object file that the durable state still names ([objects](../crates/locust-store/src/objects.rs)); the daemon cannot reach that today, because nothing outside tests asks the store to drop an object. *Fix:* in `open`, flush the log file if it exists before the first read, and refuse to open if that fails.

### IR-10: a wrong ticket is not reported

Described above under the guide. `goal status` shows no members and no title, which is exactly what the guide tells the reader to wait out, and `status` keeps listing the goal as `joining` after the refusal has arrived ([invitations](../crates/locust-core/src/node/requests/invitations.rs)). *Fix:* show a refused join as refused, let a new ticket replace a pending one or say in the `conflict` message what to do, find out why the refusal often arrives only after several retries, and note in the guide that the two tickets differ only at the end.

## P3 findings

| ID | Area | Finding and where | Suggested fix |
|---|---|---|---|
| IR-11 | Goal | `member.remove` accepts the coordinator's own key. Afterwards no decision can be signed, including readmission, and no halt is reported ([goals](../crates/locust-core/src/node/requests/goals.rs)) | Refuse it, or let a coordinator that is not a member admit itself |
| IR-12 | Goal | A result signed after acceptance becomes the task's displayed result, on a task still shown as accepted ([transition](https://github.com/andsav/locust.farm/blob/b758b12/crates/locust-core/src/goal/transition.rs)) | Do not move the task's result once one is accepted |
| IR-13 | Goal, sync | After a coordinator fork, members admitted at or after the fork position are dropped from the endpoint map, so nobody dials them and they never receive the evidence of the halt ([peers](../crates/locust-core/src/node/peers.rs)) | For a halted goal, reconcile with every key a held admission names |
| IR-14 | Node, confidentiality | A removed principal keeps reading text sealed after its removal when its daemon still speaks for another member, because payloads are opened with any key the daemon holds ([entry](../crates/locust-core/src/node/entry.rs)) | Record the principal's last epoch and report later text as absent |
| IR-15 | Node | `goal.leave` by the goal's own coordinator is accepted and disables every decision for good | Refuse it until coordination can be handed over |
| IR-16 | Node, daemon | After a failed commit the daemon answers every request with an error, including `daemon.stop`, and keeps its socket and lock until killed. The storage contract says it stops ([requests](../crates/locust-core/src/node/requests/mod.rs)) | Report a stop request once the node has failed |
| IR-17 | Node | `agent enroll` for a revoked principal reports success while the credential stays revoked | Answer `conflict` |
| IR-18 | Node | `daemon stop` under an idempotency key used before a restart answers `done` and leaves the daemon running | Do not record `daemon.stop` under a key |
| IR-19 | Sync | Pushing one new event to a peer that is one behind makes it return an inventory of the author's whole log: 9,999 points for one event at 10,000 events ([responder](../crates/locust-core/src/sync/responder.rs)) | Send an inventory only on a real divergence, not when merely behind |
| IR-20 | Sync | Catching up on N objects costs N squared: one lookup rescans the goal, 3.5 ms at 4,000 notes, about 14 s in total and about 90 s extrapolated to 10,000 ([replica](../crates/locust-core/src/node/replica.rs)) | Keep an ordered set of wanted objects in memory |
| IR-21 | Sync | An `EventRequest` with no identifiers is answered with one frame where the grammar implies none | Answer with no frame, or document it |
| IR-22 | Daemon | An exchange whose traffic flows one way for more than 30 seconds is aborted while making progress; the code comment says there is no such cutoff ([network](../crates/locust/src/daemon/network.rs)) | One idle deadline per exchange, reset by progress in either direction |
| IR-23 | Daemon | Once standard error cannot be written, the next log line aborts the daemon (`eprintln!` with `panic = "abort"`) and leaves the socket behind | Log through a helper that ignores write errors |
| IR-24 | Daemon | Connections from endpoints that are not members are kept for ever without a cap, about 130 KB each: 300 strangers took the daemon from 14 MB to 53 MB | Close a connection that presents no member exchange; cap them |
| IR-25 | CLI | Repeating `agent enroll <name> --manage-goals` after enrolling without the flag succeeds and grants nothing; there is no `agent grant` command | Apply the grants or answer `conflict`; add the command |
| IR-26 | CLI | `goal invite --expires-ms` takes an absolute Unix time, has no help text, and accepts a time in the past | Refuse a past expiry; describe the flag or take a duration |
| IR-27 | CLI | `daemon run` on a store that fails its integrity check exits 1 `internal` instead of 11 `corrupted` | Carry the error code through engine start |
| IR-28 | CLI | Errors about `--home`, `--credential` and session paths name environment variables the user did not use; the missing-credential error omits `--credential` and `--owner` | Name the option; list all three ways |
| IR-29 | CLI | `--version` works only as the sole argument and is absent from `--help`; `--help` describes no command or option | Register the version with the parser; add one-line descriptions |
| IR-30 | CLI | Human output prints enum values in Rust debug form (`Member`, `Joining`) and human `status` omits `halted` | Use the stable snake_case names; show `halted` |
| IR-31 | Docs | The build guide's download command starts the daemon that the run guide starts again; its "T1 CLI sequence" link points at workstreams; the protocol contract still says the commands are not implemented | Fix the three places |
| IR-32 | Store | `finish_blob` renames a large staged copy into place and commits its row without flushing the file, after a staging call whose flush failed still appended the bytes | Flush before the rename; truncate on a failed stage |
| IR-33 | Store | A large `finish_blob` uses up the verified staged copy before its row commits; on failure the object is neither held nor staged and a retry reports a hash mismatch for good bytes | Keep the staged name until the row commits |
| IR-34 | Store | `stage_blob` and discard report success without flushing the directory entry when an earlier call's directory flush failed or the process was killed | Flush `blobs/` once on open; remember a failed directory flush |
| IR-35 | Store | Reopening deletes a staged copy of an object already held, where the in-memory reference store keeps it. The daemon never stages a held object | Keep it, or state the difference in the contract |
| IR-36 | Secrets | The guide puts the ticket on the command line and its tickets never expire; `goal join` has no standard-input form | Accept `--ticket -`; recommend an expiry |

## The eight store claims from the interrupted review

An earlier store review by this session was cut off before its claims were verified. At the candidate: unflushed log frames trusted on open is real (IR-9); unflushed staged bytes promoted is real (IR-32); a staged copy used up before its row commits, and the retry that then reports a mismatch, are one real defect (IR-33); the unflushed directory entry is real (IR-34); the staged copy of a held object deleted on reopen is real and harmless to the daemon (IR-35). Two are not defects. A stale or high `last_position` could be produced only by editing the database from outside: no code path writes an inconsistent value, because the goal row and the event rows commit in one transaction. And a zeroed signature column, or same-length damage to stored object bytes, does come back without an error, but that is the documented scope of the store's integrity checks (identifier, index and length), since `Event::from_stored` states that it does not verify the signature again.

## What was tested and held

Recorded so that later work knows what was exercised, by lens:

- **Goal.** 3,300 generated scenarios (member and coordinator forks, broken chains, removals with arbitrary cutoffs, readmission, backdated anchors, wrong epochs, stalled decisions, duplicates), each fed in 10 to 12 orders with random batch splits: state, halt and every standing were equal in every order, incremental application equalled a from-scratch fold after every batch, and replay from a store gave the same view. Every arrival order (1,129,704 in total) of six hand-built sets agreed. Four faults injected into the incremental path were each detected. Halts are permanent for the held set.
- **Node.** Isolation of a principal that never took part, and of its viewer; credential resolution and revocation on open connections; `on_behalf`; `denied` versus `authorization_required`; grants per goal; sessions bound to one principal; claim recovery, takeover and fencing (A, then B, then A gives generations 1, 2, 3 with stale writes superseded, also after restart); idempotency across restart; memory unchanged after a failed commit; nothing signed for a halted goal or by a principal that is not a member; keys refused unless they open the epoch's first payload; waits never missing a change.
- **Sync.** Frame counts and limits for event requests and inventory pages; an endpoint that is not a member receiving identical answers for a held and an unknown goal; three real nodes converging with every stream cut after 0 to 59 frames; a 9,000-event log with interleaved gaps converging; an object fetched from two peers at once; resume from every cut; completion only after acknowledged delivery; an unreachable member not delaying the others.
- **Daemon.** File and directory modes; a second daemon refused; recovery after `kill -9`; stop by request and by signal within 80 ms with acknowledged writes all present afterwards; a client that never reads blocking only itself; oversized and malformed frames closing only their connection; every network wait bounded at 30 seconds; reconnect after a peer restarts on a new port in about a second; start with no network at all; invalid overrides refused without echoing them; no ticket, title, text, credential or address on standard error; no growth in descriptors or threads over hundreds of connections.
- **CLI.** The guide followed literally; exit statuses and the JSON envelope for each error family; credential resolution never falling back to the owner; goal prefix resolution; `status` and `doctor` in each readiness state; the download command simulated with a truncated download and a re-run.
- **Secrets.** Secret files created exclusively at mode 0600 with no partial-content window; redaction in debug, display and JSON forms; idempotency keys scoped per caller; discovery publishing only what the guide says; the tracked evidence files containing no ticket, credential or private path.

## Earlier cross-review of lane B's commits

Run before the takeover and not recorded then. Reviewed at `31ca555` (build helper) and `0e7b150` (client configuration and qualification harness). Only the first two rows were checked again at the candidate.

| Subject | Finding | State at the candidate |
|---|---|---|
| Build helper | `verify_version` accepted a `-dirty` commit | Fixed: refused as `version_dirty` |
| Build helper | Needs Python 3.11 or later while the guides call `python3`, which is 3.10 on the owner's machine | Still open; see above |
| Build helper | The bundle is a bare binary; a command-line fetch loses the executable bit | The drafted download command runs `chmod` |
| Build helper | The build environment is neither cleared nor recorded: with `RUSTC` set, another compiler builds the binary and the metadata names the pinned one | Not re-checked |
| Build helper | Index flags (`skip-worktree`, `assume-unchanged`) hide an edited source from the dirty check; a `.DS_Store` under `crates/` blocks the build | Not re-checked |
| Build helper | The same commit built at another checkout path gives a different hash (48 differing bytes) | Not re-checked |
| Client configuration | The session file is mandatory, against the local conventions, and the persistent Droid and Pi files pin one session secret for every session on a profile | Not re-checked |
| Client configuration | Codex merges the Locust entry into an existing `mcp_servers.locust`; nothing lists the names in use, so a collision is neither refused nor replaced | Not re-checked |
| Qualification harness | The interruption pass comes from signalling the whole process group; Pi's bridge outlived Pi and still passed | Not re-checked |
| Qualification harness | The Droid fixture refuses about nine backend calls without recording them | Not re-checked |
| Client configuration | Bridge paths are visible in the client's arguments, and the secret files are readable by the same user the model's shell runs as | Not re-checked |
| Qualification notes | The research note says a 30-second budget; the corrected record used 20 seconds | Not re-checked |

Building from a `git archive` copy at a fixed path, with the commit passed in explicitly and a cleaned environment, would settle the environment, index-flag and path findings together.

## Addendum: the two-Mac run recorded on October 3

Reviewed afterwards, at the owner's request: the [M1 observation](evidence/two-mac-t1-2026-10-03.json), the M2 [join record](evidence/t1-m2-smoke-2026-10-03.json) and [task-readiness record](evidence/t1-m2-task-2026-10-03.json), their [note](t1-m2-smoke-2026-10-03.md) and the rows they add to the [release ledger](../docs/release-evidence.md).

**The evidence is consistent with this review for the steps that were run, and it claims no more than it shows.** On two physical Macs: enrollment, `status` and `doctor`; a join that completed in about a second (join sent at 02:57:36.6 UTC, first synchronization at 02:57:37.5); both members and the decrypted title on both sides; M1's note readable on M2, which exercises sealed content and key delivery between machines; the assignment replicated and shown as `to_authorize`, then `to_claim` after the local `task authorize`; a session created. Every recorded command exited 0. None of the 36 findings was touched, which is expected, because they lie outside the guide's path.

Checked independently: the build-input comparison between `3422c7b` and `8ef9dbc` is empty, as recorded; the timestamps in the three records agree with each other; no invitation, credential or session secret appears in the tracked files.

What the run does not yet show: claim, submission, the result travelling back to M1 and its acceptance (the run stopped before the claim at the owner's request); restarts; sleep and wake. The coordinator-offline step needs a third member and cannot be run with two machines. The two Macs ran different bytes (M2 built its own binary), so the same-artifact requirement stays open, as the ledger says.

Observations from the evidence:

- **Both sides selected a relay path at about 57 ms.** If the two Macs are on one local network, a direct path of a few milliseconds would be expected. The daemon prints the path once, when a connection is established, so the records cannot say whether the path became direct later; nothing in `status` or `goal status` shows the current path. If it stayed relayed, that fits the unverified local-network-access hypothesis above: M2's daemon was started as a detached process by an agent session, not from Terminal, and macOS applies that restriction to direct traffic to local addresses as well as to multicast. Everything works over the relay, but all traffic then passes through the relay operator's servers, encrypted. A way to settle it: start one daemon from Terminal, allow local-network access if macOS asks, and read the `paths` line of the next connection.
- **The M1 record joins two moments under one timestamp.** Its `goal_status.decision_head` is still M2's admission while its board already shows the assignment. On the candidate the head does advance when a task is assigned (checked locally), so the status was captured before the assignment and the board after it. This is a precision point about the record, not a runtime defect.
- **The M2 records contain absolute local paths** (the home directory, the checkout and the state directory), where the earlier evidence files use placeholders and the integration findings say tracked summaries omit private paths.
- **The ticket was passed on the command line**, as IR-36 describes; the record redacts it.

## Reproduction

The probes are patches under `evidence/t1-candidate-review/`, one per reviewer (`goal`, `node`, `sync`, `store`, `secrets`, `daemon`) and one per verifier (`verify-goal` and so on). Each applies on its own to a clean copy of `3422c7b`:

```sh
mkdir /tmp/locust-probe && git archive 3422c7b | tar -x -C /tmp/locust-probe
cd /tmp/locust-probe && git apply /path/to/research/evidence/t1-candidate-review/goal.patch
CARGO_TARGET_DIR=/tmp/locust-probe-target cargo test --locked -p locust-core --lib goal::probe -- --nocapture
```

Several probes fail by design: the failing assertion is the finding. The goal patch also contains the seeded permutation harness (`goal/perm.rs`), which passes and is worth adopting as a regression. The store probes need the flush interposer `store-flush-interposer.c` from the same directory, built as a dynamic library and loaded with `DYLD_INSERT_LIBRARIES`; its header comment lists the three environment variables that trace, fail or exit at a flush. The daemon and CLI findings were reproduced with the candidate binary and the commands quoted in each finding.
