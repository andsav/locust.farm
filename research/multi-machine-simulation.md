# Simulating multi-machine tests on one computer — 2026-10-03

**Status: both prototypes are built, run and saved as evidence, and every finding has been checked by a second reviewer; the verdicts are in the table under "Verdicts". Two of this note's earlier statements were wrong and are corrected there (SIM-3 and SIM-5).** Written by the independent reviewer (see the [candidate review](t1-candidate-independent-review.md)); no source was changed. The prototypes are in `evidence/multi-machine-simulation/`, listed in the [evidence index](evidence/README.md), for the owner of the source to adopt.

The owner asked for the multi-machine tests to be simulated on one computer for everyday use, keeping real machines as a final gate. Two levels were built.

## Level 1: an in-process machine simulator

Three real nodes (`Node` over `MemStore`, with the real sync driver) in one test process, joined by a simulated network and driven only through the two engine seams. A seed chooses the faults and the delivery order, so a seed reproduces a run exactly and a failing seed prints its own replay line. It runs the whole [guide](../docs/t1-run.md) sequence and, between and during steps, stops and restarts machines, takes the coordinator offline while the others write, puts a machine to sleep for longer than every timeout, partitions and heals the network, cuts exchanges after any frame, and offsets clocks by minutes. After the faults stop it requires that every member holds the same events and state, that every acknowledged write is everywhere, that every text is readable, and that nothing is halted.

It is a test-only module, `crates/locust-core/src/node/sim/` (15 files, 3,629 lines), delivered as `simulator-head.patch` (applies to `1481d7f`; formatting and Clippy clean) and `simulator-candidate-3422c7b.patch`.

```sh
git apply research/evidence/multi-machine-simulation/simulator-head.patch
cargo test --locked -p locust-core --lib node::sim                      # about 2 seconds: 32 seeds under faults and four smaller checks
LOCUST_SIM_SEEDS=10000 cargo test --locked -p locust-core --lib node::sim::tests::sim_many -- --ignored --nocapture   # about 3.5 minutes
LOCUST_SIM_SEED=5252 LOCUST_SIM_TRUE_CLOCKS=1 LOCUST_SIM_TRACE=1 cargo test --locked -p locust-core --lib node::sim::tests::sim_one -- --ignored --nocapture   # replay one seed
```

Results: the candidate passes 10,000 seeds under about 97,000 faults. The current source fails one seed in 10,000 (SIM-6 below). After the last fault heals the network goes quiet in a median of 1 second and at most 39. To check that the simulator can fail, seven faults were injected into a copy of the product: five were caught; removing retry-after-failure alone, or periodic re-synchronization alone, was not, because each hides the other's absence.

It does not simulate the transport, discovery, address translation, the store on disk, the CLI or operating-system sleep. The network shell's rules (which connection is reused, when one is closed) are re-implemented in the simulator by reading the shell, so a finding that depends on them needs confirming on real daemons.

## Level 2: a multi-process scenario runner

Several daemons of the real binary on one Mac, each with a private home, driven only through the CLI. It reuses [the existing harness](../scripts/check_t1.py) unchanged and adds scenarios; it runs under Python 3.10. Saved in `evidence/multi-machine-simulation/runner/` (`run.py --list` shows the scenarios).

| Scenario | What it does | Time | Result on the candidate |
|---|---|---|---|
| two-machines-complete | The flow run on the two real Macs, carried through claim, submit and accept | 9 s | Pass |
| three-machines | The existing sequence, 21 checks | 11 s | Pass |
| mixed-build | Two binaries with different version lines, as the two Macs ran | 10 s | Pass |
| crash | `kill -9` when idle, during a burst of writes and during a join | 134 s | Pass: every acknowledged write present everywhere, nothing halted |
| restart-latency | Time for peers to hear from a restarted daemon | 95 s | Pass, with SIM-2 and SIM-3 |
| network-modes | The flow under each relay and lookup setting | 322 s | Pass |
| new-address | Restart on a new port under each lookup setting | 474 s | Pass, with SIM-3 and SIM-4 |
| sleep | A daemon suspended for 45 and 120 seconds while the others write | 607 s | Functional checks pass; SIM-1 |

Suggested use: the first four scenarios on every change (under three minutes), the whole set nightly (28 minutes), and real machines as the final gate.

It cannot simulate separate network stacks and addresses, address translation, a path forced through the relay, real local-network discovery, operating-system sleep or a first-time download. Suspending a process is not a sleeping laptop: the network interface stays up and the clock does not jump.

## What would let the simulation go further

Small seams in the product, in order of value: the network shell's decisions as a state machine without I/O, shared by the daemon and the simulator, so level 1 stops re-implementing them; elapsed time at the engine seam, separate from the wall clock (this also fixes SIM-8); an override that disables direct paths, so the relay path is exercised on one host; a store for tests that can fail a chosen commit or lose its unflushed tail; and a read-only view of the peer driver's state for diagnosis.

## Verdicts

Each finding was reproduced or refuted by a second reviewer, with instrumented builds where needed.

| ID | Verdict | Severity | Could real machines hit it | What verification added |
|---|---|---|---|---|
| SIM-6 | Product defect, a regression from `d253a07` | P2 | Yes | Confirmed on the real network shell over loopback connections, not only in the simulator. Over 30,000 fault-free join seeds the slowest join is 54 seconds on the current source against 23 on the candidate |
| SIM-9 (new) | Product defect, also from `d253a07` | P2 | Not in the guide | Found while verifying SIM-6: outgoing dials and incoming connections that are not yet admitted share one pool of 7 permits, and a dial holds its permit for up to 30 seconds. One member that goes offline while sharing 8 goals made the daemon dial it 8 times at once; every incoming connection was then refused and a new join took 31 seconds instead of 0.15 |
| SIM-4 | Product defect in the sense that nothing documents it | P2 | Not with the defaults | With lookup off, members other than the coordinator have no address for each other, so the guide's coordinator-offline step cannot pass whatever the ports. The guide's sentence that member keys are enough to attempt discovery does not hold under that override |
| SIM-1 | Product defect | P3 | Not shown | The hang is in `endpoint.close().await`, not in the task teardown. After the process is frozen, acknowledgements wait in the socket buffer while the clock runs, so one round-trip sample of about 120 seconds drives the connection's smoothed round-trip estimate to 15 seconds; closing such a connection took 405 seconds. The same inflated estimate explains the 31-second catch-up after resuming. Whether a laptop's real sleep does the same is not known |
| SIM-2 | Product defect | P3 | Yes, after a crash, a kill, a power loss or a force-quit | Confirmed with an instrumented build: the peer held two connections to the restarted daemon and opened its next exchange on the dead one, which ended exactly 30.0 seconds later. A clean `daemon stop` sends a close, so the guide's own restarts are not affected |
| SIM-7 | Product defect | P3 | Yes, by seconds | Confirmed over 3,000 seeds. A completed exchange that the peer opened does not clear the backoff, although the driver's own comment says a completed exchange clears it |
| SIM-8 | Product defect | P3 | Not in the guide | Confirmed; operating systems can step the clock, so it is a robustness gap and not only something a simulator can do |
| SIM-5 | Product defect, narrower than first stated | P3 | Not with the defaults | **Correction:** it happens only when `LOCUST_BIND` is set. With the default bind every ticket carried a relay address from 0.3 seconds after start |
| SIM-3 | **Harness artifact; withdrawn** | none | No | **Correction:** the 30 seconds the runner measured was not synchronization latency. After a coordinator restarted on a new port, notes were readable in both directions in 0.05 to 1.27 seconds in seven trials, with no stall from the stale ticket address. The runner timed `last_sync_ms`, which a member updates only on its own periodic exchange. One small real issue is underneath: an exchange that a peer opened ends on the accepting side as aborted, so `goal status` shows a stale last-synchronization time |

Because SIM-3 is withdrawn, the row "Dialing with a remembered address" in the deadline table below is worth doing only for daemons run with lookup off; with lookup on, the transport consults discovery about 200 ms into a dial that has addresses.

## Findings to start on

Found by the scenario runner against the candidate, before the [remediation](t1-remediation.md). None has been re-run against the current source, but the lines each one points at are unchanged at `ea72523`. They are new: none is among IR-1 to IR-36.

| ID | Finding | Where, at `ea72523` | Start here |
|---|---|---|---|
| SIM-1 | A daemon that was suspended with SIGSTOP and resumed acknowledges `daemon stop` and then never exits. SIGTERM does not end it either; only SIGKILL does. The local socket is gone while both UDP sockets, the lock and the database stay open. Seen in 3 of 4 runs on the suspended daemon, never on the others. This is the nearest thing the runner has to a laptop that slept, so the guide's sleep step followed by a stop is likely to hit it | The teardown at the end of `serve` in [network.rs](../crates/locust/src/daemon/network.rs), lines 255 to 260: `abort_all`, the `join_next` loop and `endpoint.close().await` have no deadline, and the process waits for them | Put the whole teardown under a deadline so the process exits whatever the transport does; then find which await hangs after a suspension |
| SIM-2 | After a peer is killed and restarted, the others receive its new event at once but its text becomes readable only after about 30 seconds (31.0 to 35.6 s in 4 of 6 trials, about a second in the other 2 and after a clean stop) | The dial path in the same file, line 146: the first connection that is not yet closed is reused, so an exchange is opened on the connection to the dead process and stalls until the 30-second idle deadline, while the driver allows one exchange per goal and peer | Detect a dead connection in a few seconds (a shorter transport idle timeout, or drop a connection whose stream fails to open) and retry on a fresh one |
| SIM-3 | After the coordinator restarts on a new port, members often need about 30 seconds to synchronize with it again (28.7 to 29.9 s under every lookup setting; 0 to 6 s when a worker moves instead) | Same file, line 158: `endpoint.connect(peer, &hints)` runs under the 30-second deadline with the only hints a member keeps, the ticket's, which are now stale; discovery is consulted only after that attempt gives up | Give a dial with stored hints a short deadline and fall back to the key alone, or race the two; refresh stored hints from connections that succeed |
| SIM-5 | A ticket issued within about a second of daemon start carries one IP hint and no relay address; one issued 8 seconds later carries both. With lookup off such a ticket cannot be joined after the issuer moves; with the defaults discovery covers the gap, in about 1 second by local lookup and 35 to 38 seconds by the DHT | [invitations.rs](../crates/locust-core/src/node/requests/invitations.rs), line 55: the ticket copies the endpoint's hint snapshot, which the shell records at start before the relay is online | Have the shell wait briefly for the relay before reporting the endpoint, or have `goal invite` wait for a relay hint when relays are enabled |

Found by the in-process simulator:

| ID | Finding | Where | Start here |
|---|---|---|---|
| SIM-6 | **Current source only, and likely introduced by the remediation:** a join can be cut again and again by the coordinator's own dial to the joiner. The coordinator admits the joiner and dials it at once; the joiner does not hold the goal yet and answers `NotAMember`; the shell then closes that connection because it is not admitted, which also kills the joiner's own join exchange on it. Both sides retry on the same 1, 2, 4 … 60 second schedule, so the order repeats. One seed in 10,000 never joins within 600 simulated seconds, and without any fault the slowest join is 69 seconds against 24 on the candidate. The shell rule was modelled by reading the code and is being confirmed on real daemons now | [network.rs](../crates/locust/src/daemon/network.rs), where a connection that is not admitted is closed when an exchange ends (the fix for IR-24), together with the unjittered backoff in [driver.rs](../crates/locust-core/src/sync/driver.rs) | Do not close a connection that carries an exchange this daemon opened; and add jitter to the backoff, drawn through `Entropy` so runs stay reproducible |
| SIM-7 | The text of a note lags its event by up to a minute after a peer returns: median 1.5 seconds, 90th percentile 30.5, longest 61. The event arrives by push; its text does not | Content and keys are fetched only on exchanges the receiver opens ([initiator.rs](../crates/locust-core/src/sync/initiator.rs)), and the receiver's backoff toward the sender, grown to 60 seconds while it was away, is cleared only by the receiver's own completed dial ([driver.rs](../crates/locust-core/src/sync/driver.rs)) | Clear the backoff toward a peer when an exchange it opened completes, and start a fetch then |
| SIM-8 | Setting a machine's clock back suspends its retries and periodic synchronization for as long as the step: time to quiet rises from under 40 seconds to several minutes | The driver compares wall-clock milliseconds for backoff and the re-synchronization interval | Pass elapsed time at the seam and use it for both |

SIM-4, for completeness and not a head start: with `LOCUST_LOOKUP=none` a peer that moves is never found again and two members that are not the coordinator never connect, because only a joiner stores hints and only the ticket's. That is the expected cost of turning discovery off; carrying addresses between members would be a design change.

## How quickly to give up: a recommendation for SIM-2 and SIM-3

The principle: giving up wrongly is cheap here and waiting is not. A reconnect costs one handshake and an exchange that was cut resumes where it stopped, while a stall holds the only exchange slot for that goal and peer and the person waits. So the deadlines should sit a small multiple above the slowest healthy case, not at a round 30 seconds. Healthy cases measured so far: a direct path answers in a few milliseconds, the relay path between the two Macs in about 57 ms, local lookup in about a second, and a DHT lookup in 6 to 7 seconds.

| Situation | Now | Recommended | Why |
|---|---|---|---|
| A new connection arrives from an endpoint that already has one | The older connection keeps being used until it times out | The newest connection replaces the older ones at once | No timeout is needed: a peer that reconnects has proved the old connection dead. This alone removes the 30-second lag of SIM-2, because the restarted daemon dials its peers as soon as it has something to send |
| The peer is silent on an open connection | 30 seconds | 15 seconds, with the existing 5-second heartbeat | Three missed heartbeats. The transport library uses the same figure for direct paths. A Wi-Fi blip of a few seconds survives; a dead process or a sleeping laptop is noticed in half the time |
| Dialing with a remembered address | 30 seconds | 5 seconds, then dial by key alone | A remembered address either answers within a second or is stale; a stale one currently blocks discovery, which is why a moved coordinator takes 30 seconds to reach and a moved worker (dialed by key) takes at most 6 |
| Dialing by key alone, through discovery | 30 seconds | Keep 30 seconds | The DHT needs 6 to 7 seconds when healthy and more when not |
| An exchange whose peer is alive but makes no progress | 30 seconds | Keep 30 seconds | This is the backstop for a stalled application, and a 1 MiB frame on a slow link legitimately takes many seconds |

After a dial succeeds, store the address that worked, so the remembered address stops being the ticket's.

SIM-1 and SIM-5 are small and local. SIM-2 and SIM-3 share a cause, a 30-second wait on a dead path while holding the only exchange slot, and need one decision about how quickly to give up on a connection or an address.
