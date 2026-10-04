# Organization local discovery qualification

Status: measured failure on one macOS arm64 host, 2026-10-04. This is a
transport availability boundary, not published release qualification.

The [sanitized evidence](evidence/organization-local-discovery/results.json)
retains exact observed binary identities, commands, timeouts and raw-log hashes.
The development binaries reported dirty source identity markers. Neither run is
an immutable committed package qualification.

The three-daemon T1 run admitted all members, replicated submitted content,
reviewed and selected the task, and recovered the third daemon. It then failed to
exchange standalone contributions between workers while the administrator was
offline. Routes observed before restart connected each worker to the
administrator; no worker-to-worker route was observed. The operations campaign
also stopped at the initial independent worker link. Both explicitly used local
lookup and disabled relay. The T1 final CLI timeout occurred when the campaign's
remaining wait budget expired; it does not establish an independently stalled
healthy daemon read.

The isolated `mdns_finds_a_peer_by_key_without_contact_hints` transport test
failed with `transport: Connect` after 10.04 seconds, without any goal or
organization runtime. Explicit invitation ticket contact hints established
direct routes in the campaign. This evidence localizes the blocked key-only mesh
to host discovery availability; it does not establish the reason multicast was
unavailable. No system permissions or network settings were changed.

The relevant transport implementation and its isolated test are
[the network lookup](../crates/locust-net/src/lib.rs) and
[network tests](../crates/locust-net/src/tests.rs). Protocol fixture tests with
explicit known contact hints are separate evidence and cannot qualify multicast
discovery, published builds, three physical machines or sleep/wake behavior.

## Current committed-source rerun

After the final runtime and greenfield cleanup, the isolated test was repeated
at `0a295cdabe6a878cc733c791ca73863933cfa45a`. It again failed with `transport:
Connect`, after 10.03 seconds (one failed test; process exit 101). The linked
record identifies this source-test executable's SHA-256; it is distinct from
the native release candidate built from the same commit.

A read-only source/configuration audit found no actionable Locust configuration
omission: local lookup is added after clearing default providers; relay-disabled
endpoints still publish IP addresses; mDNS advertising, address admission and
multicast loopback are enabled. The host's default multicast route selected
`en0`. The pinned `iroh-mdns-address-lookup` 0.6.0 resolver sets
`LOOKUP_DURATION` to ten seconds in its `src/lib.rs` (lines 95 and 427); the test's
30-second outer watchdog did not expire. This explains the observed duration,
not why discovery failed. The underlying host/discovery failure remains
unproven, and no network setting or permission was changed.
