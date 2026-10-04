# Final network hardening — 2026-10-03

Status: implemented; verification results below distinguish deterministic engine
checks, shell checks, and real transport evidence. This follows the remaining
findings in [the multi-machine simulation review](multi-machine-simulation.md).
The earlier report and its withdrawn findings remain unchanged.

## Clock corrections (SIM-8)

The [peer engine seam](../crates/locust-proto/src/engine.rs) now receives two
samples. `unix_ms` remains the authority for invitation expiry, signed event
wall timestamps, and last-sync diagnostics. `elapsed_ms` is monotonic within one
engine lifetime and drives only retries and periodic synchronization in the
[driver](../crates/locust-core/src/sync/driver.rs). Its origin is arbitrary and it
is never persisted. The [worker](../crates/locust/src/daemon/worker.rs) samples
`Instant`; the [simulator](../crates/locust-core/src/node/sim/world.rs) supplies
simulated elapsed time separately from each machine's stepped wall clock.

The focused driver regression steps wall time backward and forward while
checking both retry and anti-entropy boundaries. Reports retain the changed
wall timestamp. This is an internal Rust seam change, not a wire format change.
Elapsed time during actual operating-system sleep follows the platform's
`Instant` behavior; physical sleep is still a final-machine check.

## Early invitations with explicit bind (SIM-5)

Previously, an invitation copied the current endpoint snapshot. An explicitly
bound direct address could appear before the relay became ready, producing a
ticket without a relay hint. The [worker](../crates/locust/src/daemon/worker.rs)
now defers invitation creation through the asynchronous
[network shell](../crates/locust/src/daemon/network.rs). When relays are enabled,
that operation waits for relay readiness under the existing 30-second network
I/O deadline, refreshes the endpoint snapshot, then submits the original
request to the engine. Relays disabled means no readiness wait.

Daemon startup, other local connections, and peer processing continue while an
invitation waits. If relay readiness fails, the request still uses the current
snapshot, preserving direct-hint invitations while offline. Such a ticket can
still lack a relay hint after the deadline; this is a deliberate fallback, not
a promise of relay reachability. Authentication, expiry checks, and idempotency
remain engine decisions at the eventual request execution time. A deferred request whose local connection has already detached from the
worker is not executed.

## Discovery disabled (SIM-4)

No address-gossip protocol was added. With `LOCUST_LOOKUP=none`, keys identify
peers but cannot discover their addresses. The ticket supplies hints to its
inviter; other members' keys do not supply their hints. Therefore coordinator-
offline operation between members that have never learned each other's route
is not supported under this override. A moved peer can also become unreachable
when its retained hints are stale. Default discovery and a healthy relay are
separate mechanisms; enabling a relay does not itself distribute every member's
address. This is an explicit operator boundary, not a simulated NAT fix.

## Founding title before bulk content

A production-node regression with forty earlier notes reproduced the title
ordering issue: the first requested content object was not the founding text.
The [replica adapter](../crates/locust-core/src/node/replica.rs) now exposes its
missing effective genesis payload. The
[initiator](../crates/locust-core/src/sync/initiator.rs) fetches it before ordinary
hash-ordered content, then requests missing keys. Key acceptance still requires
the authenticated founding or epoch-proof payload. A final key pass remains
for epochs whose proof arrived during bulk content transfer. No batching,
parallel-transfer abstraction, or content cap was introduced.

The regression checks the real nodes' wire-frame order: founding blob, epoch-0
key, then bulk history content, and a readable final title. It verifies
ordering, not a production throughput or latency claim.

## Simulator reconciliation

The [simulated shell](../crates/locust-core/src/node/sim/conn.rs) now chooses the
newest live connection, replaces connections at least two seconds old when a
new one arrives, and uses the production 15-second connection idle timeout.
Previously it still described and modeled first-connection selection and a
30-second default, despite the production fixes already being applied.
These constants remain mirrored test conventions, not a shared shell state
machine. QUIC, relay routing, discovery, NAT, and actual laptop sleep still need
real transport and machine evidence.

## Verification

- The forty-note title-order regression failed before its repair and passes
  afterward.
- Clock-step retry/anti-entropy regression passes.
- Core library suite: 128 passed, two long simulator tests ignored at the first
  combined check. Additional verification is recorded by the final integrator.

Further checks on the combined working source:

- `LOCUST_SIM_SEEDS=1000 LOCUST_SIM_CLOCK_STEPS=1 cargo test --locked -p
  locust-core --lib node::sim::tests::sim_many -- --ignored --nocapture` passed:
  1,000 seeds, 9,674 faults, 403,955 frames. Quiet after the last fault was
  median 1 second, 90th percentile 6 seconds, longest 15 seconds. No physical
  transport or operating-system behavior is inferred from these figures.
- Temporarily restoring wall-time scheduling makes the new clock regression
  fail at the retry after rollback; restoring elapsed time makes it pass.
- Worker tests: three passed, including processing status and peer frames while
  invitation readiness is held. Network shell tests: five passed, two existing
  long one-way idle tests intentionally ignored by that focused invocation.
- Core and network crates passed `cargo clippy --locked -p locust-core -p
  locust-net --all-targets -- -D warnings`. Formatting and whitespace checks
  passed. Full-workspace verification remains the integrator's gate.
- Three real daemon probes used disposable homes, explicit
  `LOCUST_BIND=0.0.0.0:0`, and `LOCUST_LOOKUP=none`. Each issued an invitation
  about 0.28 seconds after starting the daemon and, while that request was in
  flight, issued an independent owner `status` request. With n0 relays, the
  invite completed in 2.885 seconds with one relay hint and two IP hints. With
  the deliberately unreachable custom relay `https://127.0.0.1:1`, it completed
  in 30.017 seconds with a direct hint, demonstrating the bounded fallback.
  With relays disabled, it had completed by 0.111 seconds (the probe includes
  an intentional 0.1-second status-check delay). Independent status calls took
  0.006–0.007 seconds in all three cases. These observations establish snapshot
  contents and responsiveness, not separate-machine relay reachability.

## Offline removal is access revocation, not guaranteed remote notification

Operational qualification exposed a harness assumption: it expected an offline
removed member to restart and learn its removal. The current
[sync authorization contract](../crates/locust-proto/src/sync.rs) explicitly
refuses ordinary frontier, inventory, header, key, and content exchange with
endpoints that no longer speak for a current member. Its historical-contact
exception carries coordinator fork evidence only. A transport `not_a_member`
refusal is not a signed removal decision and must not rewrite local goal state.

Consequently the removed daemon may continue showing stale locally held
membership and may author old-epoch local work. Current replicas enforce the
signed removal cutoff and refuse that endpoint future access. Its previously
held data and keys remain available locally. Automatic revocation notification
would require a separately specified evidence protocol and is not implemented
by this hardening work.

A new [production-node regression](../crates/locust-core/src/node/replica_tests.rs)
stops the third replica, removes it and rotates keys on the two remaining
replicas, restarts it, and requires a `NotAMember` response, unchanged old
history, absent new epoch key and absent future note text. It explicitly checks
that the offline replica's local membership remains stale, preventing a test
from claiming remote convergence that the protocol does not provide.

Final integration verification of the staged source snapshot passed: workspace formatting, strict all-target Clippy, and 533 Rust tests (11 existing ignored tests). Four operational harness tests and the staged documentation checker also passed. This source gate does not requalify a different native artifact.
