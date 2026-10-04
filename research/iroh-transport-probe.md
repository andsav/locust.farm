# Iroh transport probe findings

Date: 2026-10-03. **Status: pinned-source findings and qualification method; measured results are listed separately below.** Accepted usage is in the [probe runbook](../docs/transport-probe.md). The [workstream plan](../docs/workstreams.md) calls for direct, forced-relay and alternate-relay checks across separate networks.

## Source scope

Inspected `iroh` **1.3.0**, fixed by [Cargo.lock](../Cargo.lock), registry checksum `885787b892b5e2507c701f132ecbd45d2bad4bbb75157a19427087c16dadb833`. Relevant files in that published crate are `src/endpoint.rs`, `src/endpoint/presets.rs`, `src/endpoint/connection.rs`, `src/defaults.rs` and `src/address_lookup/pkarr.rs`. The published [crate source](https://docs.rs/crate/iroh/1.3.0/source/) identifies the same version. Current web documentation was consulted for the operator's infrastructure description, not used to substitute newer API examples for the pinned source.

## Findings that affect the probe

1. `presets::Minimal` configures the cryptographic provider. Other networking is configured independently. The probe explicitly disables endpoint address lookup and port mapping; it does not assume the name "Minimal" provides those guarantees.
2. `Builder::clear_ip_transports` actually removes the direct transports. Omitting direct addresses from a contact hint alone would not establish forced relay use, because peers can learn addresses after connecting. The relay-only probe disables the transports themselves.
3. `Endpoint::online` waits for a completed relay handshake. It does not prove peer connectivity, and with no relay configured it waits indefinitely. Therefore direct mode does not call it; the operator's deadline covers it in the relay modes.
4. `Connection::paths` exposes active paths, their selection state and RTT. These are source-backed observations, not per-packet traces. The probe checks that a selected path matches a forced mode but does not infer that every frame used a particular snapshot. A short automatic-mode exchange may finish on a relay before the direct path is established; an optional route wait keeps that experiment within the same operator-selected deadline.
5. A newly opened QUIC bidirectional stream is not visible to its peer until data is sent. The fixture writes its first frame before expecting the other process to accept the stream. Shutdown also needs peer completion evidence: local write/finish success alone does not prove the other process consumed the final message.
6. Public infrastructure defaults and code are separate choices. `RelayMode::Default` uses the production relay map; the helper `default_relay_mode()` also considers the staging environment override. The probe uses the explicit production selection for `--n0-relays` and no address-lookup preset.
7. **Disabling every optional probe breaks relay-only readiness.** `NetReportConfig::minimal()` disables HTTPS probes. Removing IP transports also disables QUIC address discovery probes in `socket.rs`; `net_report/options.rs` then builds an empty probe set. Without measured relay latency, no home relay is selected. The initial probe reproduced a 30-second `online` timeout with both the default relay set and one custom n0 URL. Relay/automatic modes therefore enable HTTPS probes, while keeping captive-portal checks disabled. This is required connectivity behavior, not permission to enable unrelated address-lookup services.
8. **Synchronous diagnostic writes can defeat an async deadline.** A reviewer filled a stdout pipe before launching a direct probe with a 100 ms deadline; the initial implementation remained alive after 800 ms because the current-thread runtime was blocked writing metadata. Diagnostic output must not block the runtime that handles cancellation. A process regression check covers this boundary.

## Operators and observable metadata

The pinned production relay map names four n0/Number 0 endpoints: `use1-1.relay.n0.iroh.link.`, `usw1-1.relay.n0.iroh.link.`, `euc1-1.relay.n0.iroh.link.` and `aps1-1.relay.n0.iroh.link.`. A selected home relay is not proof that every connection goes through it.

The operator describes the public relays as shared development/testing infrastructure. Relays cannot read the end-to-end encrypted application stream, but can observe connection metadata including IP addresses, timing and relayed volume. This is an operator-documented property, not a privacy audit performed for Locust. See [Iroh security and privacy](https://docs.iroh.computer/deployment/security-privacy) and [infrastructure overview](https://docs.iroh.computer/deployment/dedicated-infrastructure).

The pinned `N0` preset would also publish and resolve endpoint contact data using n0's `iroh.link` infrastructure; its PKARR HTTPS service is `https://dns.iroh.link/pkarr`. That service receives endpoint-associated signed contact records and lookup requests. DNS resolution also exposes queries to the selected recursive resolver. **The probe does not enable these endpoint-lookup services.** Relay-hostname DNS resolution remains necessary for relay URLs. Peer IPs are exchanged in direct/automatic modes; removing addresses from the retained report does not prevent their use on the network.

For an operator-provided custom relay, record that operator explicitly. Selecting another n0 region exercises another endpoint, not an independent infrastructure operator. Self-hosting and managed relay provisioning are outside this probe; the official [dedicated infrastructure documentation](https://docs.iroh.computer/deployment/dedicated-infrastructure) describes those options.

## Verification record

Code commits: `b4daf3f` (path observations and acknowledged stream shutdown), `905f31a` (probe and process harness). The [redacted measurement summary](evidence/transport-probe-2026-10-03.json) retains the selected paths, both result records, both exit codes and identity-comparison outcomes. The executable was a local debug example on macOS arm64, Rust 1.96.1, Iroh 1.3.0; SHA-256 `df3cf7ae4b771c0e055a0acc286c654faa1e95f9526ed65319f249534b4df3b0`. This is not a packaged release artifact.

| Check | Actual topology and configuration | Observed result |
|---|---|---|
| Process harness | Two local processes; direct loopback; 10,000 ms caller timeout | All five checks passed: authenticated exchange with matching IDs, unexpected-peer rejection, no-peer timeout, Ctrl-C, blocked-stdout timeout |
| Forced public relay | Two processes on the same machine/network; `--mode relay --n0-relays`; 30,000 ms | Both exited 0; IDs matched; both selected relay paths before/after exchange; listener used `usw1-1.relay.n0.iroh.link.` |
| Explicit alternate endpoint | Same machine/network; `--mode relay --relay-url https://use1-1.relay.n0.iroh.link./`; 30,000 ms | Both exited 0; IDs matched; both selected relay paths. This checks custom selection of another n0 endpoint, not operator independence |
| Automatic selection with direct wait | Same machine/network; `--mode auto --n0-relays --wait-for-route direct --bind 127.0.0.1:0`; 30,000 ms; supplied loopback and relay hints | Both exited 0; IDs matched; both selected direct paths. No WAN direct-path conclusion follows |

For the three network checks, the listener's contact records supplied the connector's `--peer`, `--peer-relay` and, for automatic mode, `--peer-addr`. All waits and child cleanup used the process helpers in [the harness](../scripts/check_transport_probe.py); the extra network orchestration was a one-off local runner. The [runbook](../docs/transport-probe.md) gives the equivalent commands for repetition on separate machines. No existing credentials were used, and no endpoint secret keys were retained.

After the fixes, focused formatting and all-target Clippy passed; `cargo test --locked -p locust-net --all-targets` passed **25 tests** (13 library, 12 example), and Python discovery passed **21 tests**. The initial whole-workspace run passed before unrelated workspace-crate implementation began. The final whole-workspace attempt was blocked by that concurrent work: formatting differences in `crates/locust-workspace`, and a `FnOnce` lifetime compile error at `materialize.rs:165` in Clippy/tests. Those files were not changed by lane B. This record establishes the focused component checks, not a green final integration tree.

Separate-machine, separate-network, independent-operator, blob-transfer and production authorization evidence remain unverified. The failed pre-fix relay readiness attempts and blocked-output reproduction are retained above because they exposed implementation bugs; their later success does not erase that history.

## Revision-2 transport review follow-up

The [October 3 follow-up measurements](evidence/transport-review-2026-10-03.json) repeat the same-host relay and direct scenarios against `28dcfdd`, which resolves lane A's admission, buffering and lifecycle findings. Both sides authenticated the expected peer and completed the exchange with the expected selected route before and after transfer. The default and east-region relays still share n0 as their operator. These results neither qualify a second machine nor exercise automatic address lookup, membership, replicated state or reconnect recovery. The [implementation log](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-b-implementation-log.md) records the accompanying tests and EOF/drop observation.
