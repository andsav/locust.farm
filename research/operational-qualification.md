# Operational workflow qualification

Date: 2026-10-03. **Status: enforcing component regressions and all six integrated
three-daemon workflow cases passed on the identified source-pinned candidate.** This record covers step 4 of the
[remaining sequence](../docs/implementation-plan.md). All files and principals
are synthetic. Same-host daemon evidence does not qualify physical machines,
OS sleep/wake, independent people, installed clients or public artifacts.

## Source-backed gap map

| Workflow | Implemented enforcement | Qualification |
|---|---|---|
| Offline removal and rotation | [Goal requests](../crates/locust-core/src/node/requests/goals.rs) sign the observed member cutoff and seal the removal under a fresh epoch; [access](../crates/locust-core/src/node/access.rs) checks each principal's rights | Existing [lifecycle](../crates/locust-core/src/node/tests/lifecycle.rs) and [authorization](../crates/locust-core/src/node/tests/authorization.rs) regressions; new [offline endpoint refusal regression](../crates/locust-core/src/node/replica_tests.rs); joined production workflow in the campaign |
| Conflicting document revisions | Acceptance checks the accepted base; signed competing evidence remains retained | New [content regression](../crates/locust-core/src/node/tests/content.rs) combines stale acceptance, withdrawal, leave and restart |
| Cancellation acknowledgment | Claimed session and generation govern acknowledgment and finalization | Existing [lifecycle regressions](../crates/locust-core/src/node/tests/lifecycle.rs) and [managed recovery campaign](t2-production-qualification.md); no replacement cancellation engine |
| Multi-chunk snapshot and patch transfer | [SQLite object staging](../crates/locust-store/src/files.rs), [typed content graph](../crates/locust-core/src/node/replica.rs) and production peer transfer | Campaign requires an observed durable partial file of at least one 1 MiB protocol chunk before killing the receiver; a restart without that observation fails |
| Retained third replica | Durable content is independent of the originating daemon | Campaign restarts the retained replica, stops the original source and materializes byte-identical content on a third replica |
| Withdrawal and leave | [Content requests](../crates/locust-core/src/node/requests/content.rs) persist withdrawal; [goal requests](../crates/locust-core/src/node/requests/goals.rs) persist local departure | New core regression verifies both persist across restart without deleting evidence |
| Competing workers | [Result acceptance](../crates/locust-core/src/node/requests/tasks.rs) checks the accepted head; [apply](../crates/locust-workspace/src/apply.rs) preflights conflicts | Campaign submits two patches from one base, refuses stale acceptance and conflicting application, then explicitly applies while preserving unrelated dirty files and the other worker's output |
| Retention and garbage collection | [Startup collection](../crates/locust-store/src/objects.rs) removes orphan object/temporary files and keeps all staged files | New [store regression](../crates/locust-store/src/tests.rs) removes local metadata, reopens beside an orphan, and verifies durable content plus resumable partial bytes survive |

## Current retention contract

Durable blob rows pin content indefinitely. The store has no automatic
age-based, size-based or reference-count eviction policy. This conservative
policy preserves accepted history, pending submissions and locally uploaded
objects alike; it can consume disk indefinitely. There is no signed remote
retention receipt or promise of permanent availability from another peer.

Withdrawal is a durable per-goal availability decision. It refuses subsequent
local reads and peer serving of that object on this daemon, while retaining
the sealed bytes and the signed event that names them. It does not erase the
object, delete another replica's copy, revoke old keys, or retract prior reads.
An explicit local put of the same content can restore local availability.

Leaving records a signed leave request and durable local departure immediately.
It stops local participation; the coordinator must still make the membership
decision. Neither leaving nor withdrawal automatically purges event metadata,
stored objects or in-progress transfer prefixes. A coordinator cannot leave or
remove itself without authority handoff, which is not implemented.

An offline removed endpoint is refused ordinary synchronization before peers
send inventory, headers, epoch keys or content. It can therefore retain a stale
local membership view and continue authoring locally under its old epoch; that
does not restore admission or acceptance by current replicas. Protocol 1 does
not promise a signed revocation notification to a former offline member. The
only historical-contact proof exception is a coordinator halt proof. The
[network findings](network-hardening-final.md) and
[production-node regression](../crates/locust-core/src/node/replica_tests.rs)
enforce this distinction. A still-current member that missed the removal
recovers the new key and new content through ordinary authorized exchange.

Startup garbage collection removes only recognized temporary files and final
object files without a corresponding file-backed durable database row. It
preserves every recognized staging file, including a redundant stage for an
already held object. Thus an active/interrupted transfer is protected without
a timeout or lease. Acknowledged metadata and staged prefixes are protected by
the store's durability/recovery barriers; collection does not run past failed
recovery. Unknown file names are not treated as owned garbage. Abandoned staged
objects also remain until an explicit transfer action discards or promotes
them; no bounded-storage claim follows from this policy.

## Reproduction and evidence

Run the [operational harness](../scripts/check_operations.py) with one explicit
production binary:

```sh
python3 scripts/check_operations.py --binary /absolute/path/to/locust --timeout-seconds 90 --network default
python3 -m unittest discover -s scripts/tests -p test_operations.py
CARGO_TARGET_DIR=target/lastmile-operations CARGO_BUILD_JOBS=2 cargo test --locked -p locust-core node::tests::content
CARGO_TARGET_DIR=target/lastmile-operations CARGO_BUILD_JOBS=2 cargo test --locked -p locust-store startup_collection_keeps_unreferenced_durable_content_and_interrupted_transfers
CARGO_TARGET_DIR=target/lastmile-operations CARGO_BUILD_JOBS=2 cargo test --locked -p locust-workspace --test contribution
```

The harness copies the supplied binary into its private disposable directory
and verifies its SHA-256 before launching any daemon. It reuses the redacted
CLI transcript and identity checks from [the T1 harness](../scripts/check_t1.py).
Tickets are redacted; credential files are never read by the harness. Its
deadline is a test-run boundary, not an added daemon execution limit. Relays
are disabled and local multicast discovery is enabled in the optional `local`
harness mode. The default `--network default` exercises the daemon's ordinary
n0 relay and Mainline/local discovery configuration. Daemon port mapping is not
independently blocked, so neither mode proves zero external traffic.

The initial immutable candidate `040da1187719-dirty` (API/protocol 1, SHA-256
`205169864dc78dca8d7c51c484b7d44bafab1cb69b619816bc577344b5c38c51`)
predates subsequent transport hardening. Its local-discovery profile failed to
establish an independent worker-to-worker link within the authored 60-second
test deadline. A loopback-bind trial consequently observed no partial transfer
with the original source offline. These trials remain failed; the default
relay/discovery profile is qualified separately. No local-discovery-only result
is inferred from a successful default-profile campaign.

The harness requires the actual text after an event header arrives, and the
holder's session when reading cancellation acknowledgment work. A canceled
holder's late submission is `conflict` with an instruction to acknowledge; this
differs from a generation fenced by takeover. It also distinguishes transport
path establishment from successful authorized goal synchronization. These
assertions prevent a header-only read, an unrelated session or a mere network
connection from being counted as the requested workflow evidence.

The final native candidate is built from source commit
`5bb254d97504209c1ee4277e74c1365c2d8620e0`, with its 12-character commit
embedded in the version. Its SHA-256 is
`abe1c0271de5c8fdbd8145d35b6b0932233d02eee7b5957fc99fc3211eac8580`.
This matches the exact artifact used by the separate
[installation and native upgrade campaign](installation-qualification.md).
The prior `c74e4511f120c57a6440ffc6cfa99ff749faed04` campaign also passed all
six cases, but this new run is the evidence for the final changed executable.

Campaign `20261004T065745Z-cd79d5a1` passed all six cases with no cleanup failure
in 45.9 seconds:

| Case | Retained observation |
|---|---|
| Conflicting documents | Both proposal texts arrived; one accepted; stale acceptance refused; competing text remained inspectable |
| Interrupted snapshot and retained replica | A 12,582,912-byte file transferred from the restarted retained peer while its original source was stopped; the receiving daemon was killed with a 1,048,576-byte staging prefix, restarted and materialized byte-identical content |
| Competing patches | Two workers submitted distinct changes against one base; patch reception was interrupted at a 1,048,576-byte prefix and completed after restart; stale acceptance and conflicting application returned `conflict`; successful explicit apply preserved unrelated dirty work and the other worker's output |
| Offline cancellation | Claimed worker restarted into a durable cancellation; late submission was refused; explicit acknowledgment reached the coordinator and remained acknowledged after another restart |
| Withdrawal and leave | Withdrawal persisted after restart and removed local text visibility while another replica retained its copy; leaving refused new local writes before and after restart |
| Offline removal and key rotation | Removal and future payload used epoch 1; a still-current offline member recovered the new key and text; the removed endpoint attempted a fresh transport path but could not read future content, and its offline contribution did not appear on the coordinator |

The [reviewed compact evidence](operational-qualification-evidence.json) retains
the binary and harness hashes, public event/object identifiers, prefix hashes,
whole-file hash, platform, transport selection and raw-artifact hashes. Both
relay and direct selected paths were observed; this does not mean every
operation used both. Raw redacted artifacts remain under
`output/operations/20261004T065745Z-cd79d5a1/`; the tracked evidence preserves
the useful results independently of that disposable directory.

Earlier focused verification passed: four Python harness assertion tests, nine core
content tests, five workspace contribution tests, 32 store unit tests and two
store process-crash tests. Three existing store tests were ignored: two helper
entrypoints launched only by their parent tests and the separate macOS flush
interposer qualification. Clippy passed for core/store/workspace with all
targets and warnings denied. Full workspace gates and staged documentation
index checking belong to the final integration pass.

Earlier integration verification passed workspace formatting, strict all-target
Clippy and 533 Rust tests (11 existing ignored tests). The final service/setup
source gate passed 571 Rust tests (11 ignored) with formatting and strict Clippy;
the completed harness commit passed 171 Python tests and the staged documentation
checker. The operational harness now has five focused assertion tests. These
source gates complement the exact final native artifact campaign recorded here.

## Resource observation scope

The final-candidate campaign samples receiver RSS after pausing the owned daemon
at an observed partial-transfer boundary and again after completed snapshot
resume or patch integration. These are individual resident-memory samples, not
peak memory and not a performance pass/fail threshold. Pausing before sampling
keeps the partial receiver state from advancing while `ps` reads RSS; the owned
process is then killed for the existing interruption scenario. Network bytes
and operation-campaign CPU consumption are explicitly unmeasured. Native install
startup and idle measurements are recorded separately in the
[installation campaign](installation-qualification.md). The final run observed:

| Receiver checkpoint | Sampled RSS |
|---|---:|
| Snapshot receiver paused at 1 MiB durable prefix | 25,083,904 bytes |
| Snapshot receiver after complete resume | 54,280,192 bytes |
| Patch receiver paused at 1 MiB durable prefix | 24,788,992 bytes |
| Patch receiver after resumed patches and integration | 58,130,432 bytes |

The campaign ran concurrently with local installation qualification. The
checkpoint values do not establish peak memory or a performance regression
threshold; network byte consumption and operational CPU remain unmeasured.
