# Reproducible multi-machine simulation

Locust has two simulation levels for exercising distributed failures on one
computer. Both use the current runtime; neither establishes physical-network,
OS sleep/wake or power-loss behavior.

## In-process state-machine simulation

[The simulator](../crates/locust-core/src/node/sim/mod.rs) runs real nodes over
`MemStore` and the sync engine with a seeded network and clock. The seed controls
fault ordering, so a failed run can be replayed. It exercises partitions,
restarts, delayed/duplicate delivery and independent wall/elapsed clock changes.
The invariants concern retained events, acknowledged writes, convergence and
readable content after faults stop.

```sh
cargo test --locked -p locust-core --lib node::sim
LOCUST_SIM_SEEDS=10000 cargo test --locked -p locust-core --lib node::sim::tests::sim_many -- --ignored --nocapture
```

## Multi-process scenarios

[The scenario runner](../scripts/simulate_machines/run.py) launches separate
identified local daemons and checks their CLI-visible state. It records commands,
results and cleanup under ignored `output/sim/`. Select an immutable binary; a
concurrent rebuild must not replace the executable under test.

```sh
python3 scripts/simulate_machines/run.py --binary target/release/locust --quick
python3 scripts/simulate_machines/run.py --binary target/release/locust
```

The quick suite and full suite cover different cases. A successful run requires
its actual scenario assertions, not process startup alone. Inspect the selected
network configuration and report it with the candidate identity.

## Evidence boundary

One-host processes share a kernel and local network. The in-process model does
not include native Iroh routing, SQLite flushes or client execution. The process
runner does not reproduce two independent owners or accounts. Retain exact
source/binary identities and failed seeds when reporting a new campaign.
The [network constraints](network-hardening-final.md) describe the clock,
connection and discovery behavior behind these experiments.
