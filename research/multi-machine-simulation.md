# Simulating multi-machine tests on one computer — 2026-10-03

**Status: preliminary. The prototypes exist in scratch and their results are still being verified; this note is published early so that fixing can start. It will be replaced by the full write-up, with the prototypes as patches.** Written by the independent reviewer (see the [candidate review](t1-candidate-independent-review.md)); no source was changed.

The owner asked for the multi-machine tests to be simulated on one computer for everyday use, keeping real machines as a final gate. Two levels were chosen:

1. **An in-process simulator**: several real nodes in one test, joined by a simulated network with a controllable clock and seed-chosen faults. On the release candidate it ran the three-machine sequence over 4,000 seeds with about 39,000 injected faults in 31 seconds with no failure. It is being ported to the current source.
2. **A multi-process scenario runner**: several daemons of the real binary on one Mac, each with its own home, driven through the CLI. Its final run against the candidate `3422c7b` passed 7 of 8 scenarios: the two-machine flow through claim, submit and accept; the three-machine sequence; crash; new address; network modes; mixed build; restart latency. The sleep scenario's functional checks passed and it exposed SIM-1.

## Findings to start on

Found by the scenario runner against the candidate, before the [remediation](t1-remediation.md). A second reviewer is still verifying them, and none has been re-run against the current source; but the lines each one points at are unchanged at `ea72523`, so they are worth starting on. They are new: none is among IR-1 to IR-36.

| ID | Finding | Where, at `ea72523` | Start here |
|---|---|---|---|
| SIM-1 | A daemon that was suspended with SIGSTOP and resumed acknowledges `daemon stop` and then never exits. SIGTERM does not end it either; only SIGKILL does. The local socket is gone while both UDP sockets, the lock and the database stay open. Seen in 3 of 4 runs on the suspended daemon, never on the others. This is the nearest thing the runner has to a laptop that slept, so the guide's sleep step followed by a stop is likely to hit it | The teardown at the end of `serve` in [network.rs](../crates/locust/src/daemon/network.rs), lines 255 to 260: `abort_all`, the `join_next` loop and `endpoint.close().await` have no deadline, and the process waits for them | Put the whole teardown under a deadline so the process exits whatever the transport does; then find which await hangs after a suspension |
| SIM-2 | After a peer is killed and restarted, the others receive its new event at once but its text becomes readable only after about 30 seconds (31.0 to 35.6 s in 4 of 6 trials, about a second in the other 2 and after a clean stop) | The dial path in the same file, line 146: the first connection that is not yet closed is reused, so an exchange is opened on the connection to the dead process and stalls until the 30-second idle deadline, while the driver allows one exchange per goal and peer | Detect a dead connection in a few seconds (a shorter transport idle timeout, or drop a connection whose stream fails to open) and retry on a fresh one |
| SIM-3 | After the coordinator restarts on a new port, members often need about 30 seconds to synchronize with it again (28.7 to 29.9 s under every lookup setting; 0 to 6 s when a worker moves instead) | Same file, line 158: `endpoint.connect(peer, &hints)` runs under the 30-second deadline with the only hints a member keeps, the ticket's, which are now stale; discovery is consulted only after that attempt gives up | Give a dial with stored hints a short deadline and fall back to the key alone, or race the two; refresh stored hints from connections that succeed |
| SIM-5 | A ticket issued within about a second of daemon start carries one IP hint and no relay address; one issued 8 seconds later carries both. With lookup off such a ticket cannot be joined after the issuer moves; with the defaults discovery covers the gap, in about 1 second by local lookup and 35 to 38 seconds by the DHT | [invitations.rs](../crates/locust-core/src/node/requests/invitations.rs), line 55: the ticket copies the endpoint's hint snapshot, which the shell records at start before the relay is online | Have the shell wait briefly for the relay before reporting the endpoint, or have `goal invite` wait for a relay hint when relays are enabled |

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
