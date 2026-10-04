# Separate-goal subgroup acceptance

Status: implemented Engine acceptance test; focused local verification passed on
2026-10-04. This is one daemon with two authenticated agent credentials and
separate goal memberships, using a durable `MemStore::reopen` boundary. It is not
network, remote-daemon, published-build or real-model qualification.

The executable [subgroup test](../crates/locust-core/tests/subgroups.rs) creates a
parent goal with coordinator review and selection policy, and an Open child
goal. A bridge belongs to both goals; the child-only principal does not belong
to the parent. Parent blob and contribution reads by that principal return
`NotFound`, preserving the API's non-disclosure boundary even though the daemon
holds both goals locally.

The bridge reads exactly the selected parent contribution's artifact and calls
`BlobPut` in the child goal. The same plaintext obtains a distinct goal-bound
sealed hash. The child sees the explicit exported contribution and its bytes;
it cannot read the original parent hashes, and receives neither the private
parent draft nor parent event history. No subgroup entity, inherited membership,
implicit history export or additional API is introduced.

The child publishes an independent taskless finding. The bridge deliberately
reads and re-seals its artifact into the parent goal, then publishes a new
parent taskless contribution. It remains unapproved and unselected through
restart. Selection with the correct current decision predecessor conflicts until
the parent records its own review; subsequent explicit parent selection succeeds.
Child evidence does not act as parent approval.

Focused command: `cargo test --locked -p locust-core --test subgroups`.
Observed result: one test passed, zero failed, zero ignored, test execution
0.02 seconds. The existing public Engine seam handles all requests; the test does
not edit Store records directly. Full workspace gate results are reported with
the implementation commit, separately from this focused observation.

The [accepted implementation plan](../docs/formations-implementation-plan.md)
tracks the O6.6 separate-goal subgroup boundary. PeerEngine transfer and physical
machine discovery need separate evidence. The
[local discovery qualification](organization-local-discovery.md) records why the
key-only worker mesh was unavailable on this host; this credential acceptance
does not supersede that finding.
