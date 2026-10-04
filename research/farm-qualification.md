# Farm development qualification

**Status: first-slice local implementation, 2026-10-04.** This report distinguishes
production-process integration from synthetic browser fixtures and from the
unrun two-physical-machine, real-harness rehearsal.

The [farm guide](../docs/guide/farm-publication.md) describes the implemented behavior;
the [design](../docs/swarm-visualization-plan.md) records the broader demo target.

## Local daemon and HTTP pipeline

The [qualification script](../scripts/check_farm.py) uses two private daemon homes,
scripted CLI clients and the actual SQLite HTTP service. It checks owner-only
controls, consent on both daemons, private text/identity canaries, replicated
attempts and reviewed/selected results, stable public identifiers across restart,
closure/reopen, remote revocation/resume, and acknowledged deletion while offline.
The [retained run](evidence/farm/local-pipeline.json) passed all 25 checks.
Post-restart peer recovery took 27.7 seconds, within the 30-second anti-entropy
interval. It uses explicit stable loopback UDP binds and disables relays. That configuration
qualifies farm recovery, not address-discovery recovery after changing ports.

A core regression test also instantiates the staged team-chat formation and
checks its branch prerequisites, approved public labels, evaluated task state
and stable mapping after a new round. This verifies the projection from actual
formation state; the browser fixture alone does not establish that mapping.

Two earlier runs used ephemeral sockets and local multicast discovery. Both
established the goal and remote work before restart. After restarting the creator,
the peer did not receive new events within the 45-second test deadline. The first
run therefore could not deliver remote revocation; the second added a direct
post-restart event check and isolated the transport boundary. The creator still
published its local post-restart work and closure. No successful revocation claim
is based on those runs. Retained [failure summaries](evidence/farm/ephemeral-discovery-failures.json)
include binary identities and completed checks. Fixing dynamic discovery remains
separate work; stable binds must not be represented as evidence for it.

## Public browser and proxy

The shared [synthetic fixture](../crates/locust-proto/fixtures/farm-snapshot.json)
contains branches, multiple attempts, unstaged work and unknown observation times.
Browser tests exercise desktop and phone layouts, keyboard task selection, reduced
motion, literal hostile text, disconnected/quiet/ended states, and invalidation
while updates are paused.

The actual Nginx configuration was also run in a local `nginx:1.28-alpine` container
with the built site and native HTTP service. The only upstream change was
`127.0.0.1` to `host.docker.internal` to cross the container boundary. A local test
certificate was used. Public farm/gallery/API paths returned 200; `/`, `/start`
and `/docs` remained 401 without preview credentials. The hydrated page received
a second signed snapshot over the real SSE connection without reloading. Script
CSP remained enabled; Martian Mono and Major Mono Display loaded. This is local
proxy evidence, not production TLS, deployment or Linux-service qualification.

Retained evidence: [route and header checks](evidence/farm/nginx-routes.json),
[farm desktop](evidence/farm/live-nginx-1440.png),
[farm phone](evidence/farm/live-nginx-390.png),
[gallery desktop](evidence/farm/live-nginx-gallery-1440.png) and
[gallery phone](evidence/farm/live-nginx-gallery-390.png). These screenshots use
the explicitly synthetic signed fixture, not real agent activity.

## Verification

`cargo fmt --all --check`, locked workspace Clippy with warnings denied, and
`cargo test --locked --workspace` passed. The run passed 743 tests and reported 14 ignored entries (including installed-client,
network and performance probes, and helper subprocess entries). The documentation index/link checker passed.

The required website checks passed: `npm run lint`, `npm run check`, `npm test`
(186 tests) and `npm run build` (91 prerendered routes, 19 raw articles and 10
exact assets). All six farm browser tests passed. The full browser suite passed
23 of 24: the existing formations matrix test expects no `.line.main svg`
elements, while the shared site design now renders four. That unrelated test
was left unchanged. The generated farm types match the exported Rust schema;
all six built-in formation examples and their conformance exports validate.

## Proof boundaries

No real provider, model activity, two-host network run, production deployment,
Linux service execution, load benchmark or complete team-chat application was
part of this campaign. Group labels are reports, not proof of physical machines.

The existing TLA+ organization models were not extended for publication consent,
the HTTP outbox, SQLite receipts or SSE. Existing governance invariants still
inform implementation, but their previous results are not formal verification of
this new state machine. Authority conflicts and unavailable projection proof currently suspend the entire
farm. New tests cover concrete consent and recovery behavior;
a future formal model would need explicit publication eligibility, pending
mutation identity, service sequence/tombstone state and interrupted delivery.
