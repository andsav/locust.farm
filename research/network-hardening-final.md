# Network recovery constraints and experiments

These source-backed constraints explain what the
[multi-machine simulator](multi-machine-simulation.md) can exercise and where
native transport qualification is still required.

## Separate clocks and authority

The [engine seam](../crates/locust-proto/src/engine.rs) supplies wall time and
elapsed time separately. Invitation expiry and signed event timestamps use
wall time; [sync retry scheduling](../crates/locust-core/src/sync/driver.rs) uses
monotonic elapsed time. Clock steps must not silently redefine collaboration
authority or suspend local retries for the duration of a backward wall-clock step.
[Driver tests](../crates/locust-core/src/sync/tests/driver.rs) exercise retry,
antientropy and failed/partial exchange behavior.

## Connections and discovery

The [daemon network layer](../crates/locust/src/daemon/network.rs) manages
connections while the engine validates membership and frame effects. A known
endpoint key, stored address hint, relay route and independently discovered
route are separate inputs. Disabling discovery does not promise that moved peers
can be found. A transport success alone does not establish goal admission or
artifact reconciliation.

Use the [transport probe](../docs/transport-probe.md) to record direct, relay and
automatic route observations. Use the production-daemon scenarios for retained
state and offline reconciliation. Report same-host, same-LAN, separate-network
and physical-machine results as different topologies.

## Reproduce and interpret

The [seeded simulator](../crates/locust-core/src/node/sim/mod.rs) separates elapsed
and wall clocks, packet schedules and node restarts. The
[multi-process runner](../scripts/simulate_machines/run.py) uses actual local
daemons. Neither alone proves native sleep/wake, power loss, arbitrary network
liveness or current-platform multicast discovery. Failures and unknown routes
remain part of the result.
