# Transport probe

Date: 2026-10-03. **Status: qualification tool; not the daemon or a release-qualified network.** The probe exercises [Locust's framed peer link](../crates/locust-net/src/lib.rs) with ephemeral identities and a small, validated exchange. It does not join a goal, authorize a member, exchange artifacts or persist anything.

The [source and infrastructure findings](../research/iroh-transport-probe.md) distinguish pinned-library behavior from observations. Record completed checks in the [release evidence ledger](release-evidence.md); a same-machine result cannot close the separate-network gate.

## Build and local checks

```sh
CARGO_TARGET_DIR=target/lane-b-probe cargo build --locked -p locust-net --example transport_probe
target/lane-b-probe/debug/examples/transport_probe --help
python3 scripts/check_transport_probe.py --binary "$PWD/target/lane-b-probe/debug/examples/transport_probe" --timeout-ms 10000
```

The harness uses two local processes with direct loopback transports. It checks the exchange and authenticated peer IDs, unexpected-peer rejection, an idle listener's timeout, interruption, and timeout with a full, undrained stdout pipe. The last check uses a separate 100 ms regression-fixture deadline. It starts no public relay or discovery client. The example also has Rust tests:

```sh
CARGO_TARGET_DIR=target/lane-b-probe cargo test --locked -p locust-net --example transport_probe
```

## Modes and evidence

| Mode | Configuration | What a successful result establishes |
|---|---|---|
| `direct` | IP transport; no relay or endpoint address lookup | The exchange completed with an observed direct path; no relay transport was available |
| `relay` | Explicit relay choice; IP transports removed | The exchange completed with an observed relay path; a direct path could not mask a relay failure |
| `auto` | IP transport and explicit relay choice | The exchange completed; inspect the path observations for the selected route |

Relays are selected with either `--n0-relays` or one or more `--relay-url` arguments. Endpoint address lookup and router port mapping are disabled in every mode. Relay and automatic modes enable HTTPS latency probes to the configured relays so Iroh can select a home relay when direct QUIC probes are unavailable. Captive-portal checks remain disabled. Peers use the contact hints supplied on the command line. A hostname in a relay URL still needs ordinary DNS resolution.

`--timeout-ms` is mandatory and positive. It bounds the attempt, including relay readiness and the exchange; choose it to include the time spent copying contact hints between machines. Endpoint shutdown follows cancellation or completion and can add time before process exit. Final diagnostic output has at most one further interval of the same caller-selected duration; a blocked output pipe cannot indefinitely hold the process open. Ctrl-C cancels the attempt and closes the endpoint. Keys are generated for each process and are never written or printed. Restarting changes the endpoint ID.

Output is line-oriented `key=value` records. `record=contact` includes public endpoint IDs, IP addresses and relay URLs needed by the other participant. Exchange those records through the intended handoff channel; omit contact addresses from shared evidence. Path records contain route kind, selection state and RTT without addresses. A path snapshot records a moment in time, not the route of every packet. `record=result status=success` is the success assertion; a ready listener or configured relay alone is not one.

Exit codes: `0` success, `1` runtime/protocol failure, `2` invalid arguments, `124` deadline expired, `130` interrupted. Failure records identify the phase and a stable error category. A timeout establishes failure to complete within the chosen deadline, not its root cause. If output stays blocked, the final record may be unavailable; the process still exits. Parsing errors and `--help` use ordinary CLI output before accepting an attempt deadline.

## Two machines on separate networks

Build the same commit with the pinned toolchain on both machines. Record the commit, executable SHA-256, operating system and architecture. Choose two genuinely separate networks; record their descriptions without public IP addresses. The example values below give the listener five minutes for the manual handoff and the connector one minute; they are operator choices, not production policies.

On machine A, start a listener:

```sh
target/lane-b-probe/debug/examples/transport_probe listen --mode direct --timeout-ms 300000
```

Copy A's endpoint ID and a reachable `direct_addr`. A private or loopback address is not reachable from an unrelated network. Direct-only mode does not supply a relay-assisted rendezvous, router mapping or universal NAT traversal. If neither side has a directly reachable route, retain that failed result and continue with the relay run; do not relabel a LAN test as a successful WAN test.

On machine B, substitute A's actual values:

```sh
target/lane-b-probe/debug/examples/transport_probe connect --mode direct --timeout-ms 60000 --peer A_ENDPOINT_ID --peer-addr A_REACHABLE_IP:PORT
```

Both processes must exit successfully and report the other endpoint's ID. Compare IDs through the same trusted channel used for the contact hints. The connector authenticates the supplied `--peer` identity. A listener can additionally require `--expect-peer B_ENDPOINT_ID` when the connecting identity is known before it starts; otherwise it reports, but does not pre-authorize, the first authenticated endpoint. This probe does not grant that endpoint any Locust membership.

## Forced relay and an alternate relay

On A:

```sh
target/lane-b-probe/debug/examples/transport_probe listen --mode relay --n0-relays --timeout-ms 300000
```

On B, supply A's emitted ID and relay URL:

```sh
target/lane-b-probe/debug/examples/transport_probe connect --mode relay --n0-relays --timeout-ms 60000 --peer A_ENDPOINT_ID --peer-relay A_RELAY_URL
```

Both sides must report selected relay paths and successful completion. For an alternate relay, run fresh processes on both machines with `--relay-url https://YOUR_RELAY_HOST/` in place of `--n0-relays`, and use A's new contact records. Only that configured relay set is eligible; default relays are not silently added. Record the relay operator and URL, server version when known, and both results. An alternate region run operated by n0 does not establish independence from n0.

Custom relays must be compatible with the pinned Iroh release and accept the ephemeral endpoint identities. This probe does not provision a relay, install certificates or obtain managed-service credentials. HTTPS certificate verification stays enabled. HTTP is restricted to loopback for local fixtures; URL credentials, query strings and fragments are rejected.

Use `--mode auto` with the same relay selection to observe ordinary path choice. Give B both `--peer-addr` and `--peer-relay` when available. A short exchange may finish before a direct path is established; a relay snapshot in that run does not prove direct connectivity is impossible.

To test a direct path after relay-assisted rendezvous, add `--wait-for-route direct` to both automatic-mode commands. The existing attempt deadline covers waiting for the selected route. This lets the relay help the peers find each other before the fixture exchange; unlike direct-only mode, it is not evidence that no relay participated in connection setup. `--wait-for-route relay` is also available for automatic mode, but use relay-only mode when the requirement is to eliminate all direct transport.

## Evidence to retain

- Commit, binary hashes, toolchain, OS/architecture, date and mode.
- The caller-selected timeout, relay selection and redacted network descriptions.
- Both endpoint IDs, path observations, result records and exit codes. Remove contact IP addresses and any unrelated diagnostic data.
- Whether the machines were separate and whether their networks were separate; whether the alternate relay used an independent operator.
- Failed attempts and their phases, without upgrading configuration or readiness to successful exchange.

The probe's fixture proves transport exchange only. Membership, reconciliation, reconnect recovery, durable acknowledgment, blob authorization and crash-resumable transfer need their own daemon-level tests.
