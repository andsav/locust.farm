# Public farm architecture

Public farm pages show a creator's explicitly published projection of a Locust
goal. The daemon remains the authority for private collaboration; the optional
farm service stores signed public snapshots and serves the website.
Read [the publication guide](guide/farm-publication.md) for commands and consent.

## Data and consent

The [public types](../crates/locust-proto/src/farm.rs) define structured task,
attempt, contribution and stage state. Public profiles and text require explicit
publication choices. The projection excludes private prompts, source, messages,
credentials, local paths and endpoint addresses. Losing required consent makes
the projection unavailable; it cannot recall copies already held elsewhere.

A listed farm appears in the gallery. An unlisted farm is accessible by its
link. Showing a farm and joining its private goal are separate operations.
Synthetic examples use the same signed API and disclose their synthetic origin.

## Service and browser

[locust-farm](../crates/locust-farm/src/lib.rs) verifies uploads, persists receipts,
projections and tombstones in SQLite, and exposes public snapshots and SSE.
Monotonic sequence and request identity make interrupted uploads reconcilable.
A service receipt, creator observation and private peer sync are different facts.

The static SvelteKit website displays the full snapshot and ordered full-state
updates. Its stage map and task table show observed work, with a keyboard-readable
view of each task. Quiet means no recent receipt; it does not mean execution
stopped. An ended goal requires explicit closure.

## Reproduce checks

Use [check_farm.py](../scripts/check_farm.py) for daemon/public-service checks and
[the site tests](../sites/locust.farm/README.md) for rendered behavior.
[Farm qualification](../research/farm-qualification.md) distinguishes local process
checks from synthetic browser fixtures. [The live rehearsal](../research/live-farm-demo.md)
records four real clients, a public projection and one local daemon, with retained
failures. These experiments do not qualify two-machine operation or unattended
scheduling. [Operator instructions](../sites/locust.farm/ops/README.md) describe
service installation, backups and takedown.
