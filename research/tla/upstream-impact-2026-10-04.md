# Upstream remediation impact on TLA+ verification

Date: 2026-10-04. **Status: source assessment for integration; no protocol-1
model checks or physical qualification are claimed.** This reviews
`3cb9c5da86d6351ccbeb620674e10ad5c9ea0d28` through fetched upstream
`908c95321402f5ec1818bec26926a03f6442127c`. The runtime change is
`d253a07bf26cef2b59172df16297383fd286e369`; build isolation is
`9c0198650d68478a67c30e501924b61d11ab1a08`, and client-harness cleanup is
`0391da8675b7ab7e5273a6f7b5eda796dd28b075`. Read the
[implemented remediation](../t1-remediation.md) alongside this assessment.

The delivered [Stage 1 models](README.md), [property map](property-map.md) and
[results](../evidence/tla/README.md) describe protocol-0 rules. Their initial
baseline is `86980594acee708b2f9bf303afc9ee636dc55337`; runtime Rust/build inputs
did not change between that baseline and `3cb9c5d`. Local pre-merge commit
`4fc3079df29da4d2c4b503b2478c5ed75250c037` added models/evidence and Rust trace
fixtures, not runtime semantic fixes. Recorded executed-input hashes remain
the authority for those historical runs.

## Changed verification seams

| Area | Implemented upstream change and formal impact |
|---|---|
| Goal replay and IR-5 | [Commitments](../../crates/locust-core/src/goal/commitments.rs) derives exact member branches from valid canonical coordinator decisions and semantic dependency closure. Selection never depends on a replica's earlier applied state; earlier valid commitments win incompatible later selections. Missing plausible evidence remains pending. Coordinator forks still halt authority. The old `GoalLog` excludes the forked member suffix and therefore cannot represent this policy. |
| Accepted display and IR-12 | [Result recording](https://github.com/andsav/locust.farm/blob/b758b12/crates/locust-core/src/goal/transition.rs) changes the task's displayed result only while it has no acceptance. Later signed submissions remain evidence. The old model's expected display-binding counterexample now describes the superseded rule. |
| Retention and IR-6 | [Screening](../../crates/locust-core/src/goal/screen.rs) evaluates canonical dependencies over held history plus the incoming batch before applying variant/waiting quotas. Required references and ancestry receive exemptions, while known-invalid targets do not exempt arbitrary ancestry. `depends_on` now participates in semantic retention; this does not implement scheduling dependencies. Stage 1's fixed delivery universes do not model screening or storage retention. |
| Local authority | [Read access](../../crates/locust-core/src/node/access.rs) requires canonical admission, and [reader entitlement](../../crates/locust-core/src/node/entry.rs) caps removed members' content epochs. Invitation redemption rechecks issuer eligibility; self-removal/leave by the coordinator is refused. [Signing](../../crates/locust-core/src/node/authoring.rs) requires an active principal without local departure. Sessions omits invitations, content epochs and local departure. |
| Claims and failure boundaries | `sessions.rs` and `requests/claims.rs` are unchanged in this upstream range. Generation fencing and ordinary keyed replay remain useful abstract contracts, but their old TLC runs remain tied to their recorded source. [Shutdown](../../crates/locust-core/src/node/requests/mod.rs) is now excluded from persisted request replay. Fatal storage failure requests daemon shutdown and is reported after socket/lock cleanup; uncertainty also covers staged writes, promotion and discard. Sessions checks progress writes with atomic storage, not these additional operations or shell cleanup. |
| Replication and IR-13 | [Protocol 1](../../docs/protocol-v1.md) adds a bounded two-event `HaltProof` channel for eligible historical contacts, without ordinary membership/content/key authority. Outbound disclosure waits for canonical recipient membership. Reconciliation defers inventory for a shorter contiguous prefix; lazy replies and intake backpressure bound queued responses. Blob resumption validates lengths/metadata and preserves shared staging on unavailability. None of these exchanges is modeled by Stage 1. |
| Storage recovery and IR-9 | [Opening](../../crates/locust-store/src/connection.rs) completes a checked FULL WAL checkpoint before logical state exposure or garbage collection. [Staging](../../crates/locust-store/src/files.rs) synchronizes recovered files/directories; large promotion installs an independent copy and retains staging until the row commits. [Recovery collection](../../crates/locust-store/src/objects.rs) preserves redundant staged copies. Atomic Sessions storage cannot establish these flush guarantees or power-loss safety. |

## Evidence and integration consequences

Preserve every old TLC finding, witness, mutation and passing result. All old
`GoalLog` configurations check the old replay algebra; a green rerun after this
merge would still check that algebra. In particular, `goal-ir5`,
`goal-ir5-current`, `goal-ir12` and `goal-ir12-current` must retain their
historical classification rather than become protocol-1 qualification. The
Sessions results also remain historical, even where unchanged claim code
supports a reviewed explanation that its abstract contract still applies.

The hand-mapped IR-5/IR-12 Rust fixtures must now assert the corrected runtime
outcomes. Retain the same signed transcripts, arrival-order/replay checks and
historical model mapping; do not remove or ignore failing characterization
assertions. Recheck the A-B-A/retry/reopen fixture against integrated source.
These are concrete regression executions, not revised-model evidence. The
adapted IR-5 fixture confirms that the member fork preserves all three heads,
while its final explicit empty removal cutoff still withdraws dependent work.
Fork protection therefore does not establish permanence across arbitrary
coordinator removals. The generation/idempotency fixture needs no request-shape
change. The runner now records an explicit modeled baseline separately from the
current checkout identity and labels historical protocol checks.

Both [API and protocol versions](../../crates/locust-proto/src/lib.rs) change
from 0 to 1, including signed headers, invitations and golden vectors. The store
refuses incompatible stored events before migrations or garbage collection;
opening may first checkpoint physical WAL pages. There is no in-place event
migration. Preserve the running M2 protocol-0 executable, home and credential.
Any later protocol-1 two-Mac test needs aligned artifacts and fresh independent
homes, following the [current run guide](../../docs/t1-run.md).

The integration changes protocol, core, store, daemon, Python helpers and the
website; run their required checks and inspect merged test-module inclusions.
Upstream test counts are upstream evidence, not local merge results. The
[bootstrap workflow](../../.github/workflows/tla-bootstrap.yml) checks runner
fixtures only, with tool/model path filters; it neither runs the full model
suite nor guards runtime-only changes. Its remote execution remains unobserved.
The revised [build helper](../../scripts/build_t1.py) verifies committed archive
bytes and isolates recorded compiler settings; this improves provenance without
claiming hermetic or byte-reproducible builds.

## Follow-up scope

1. Create a separately identified protocol-1 GoalLog revision. Model canonical
   reference selection, transitive dependency closure, pending versus invalid
   decisions and incompatible commitments. Check acceptance permanence/display
   binding and replay equality under varied arrival and public-key orders;
   distinguish fork protection from removal/cutoff effects explicitly.
2. Revalidate Sessions against its current source map. Extend authority/failure
   coverage only where its abstraction needs it; do not infer invitation,
   content-read or daemon-cleanup correctness from local claim safety.
3. Stage 2 must cover quota retention, historical proof delivery and recipient
   authorization, unequal/divergent prefixes, pagination, interrupted exchanges,
   lazy response credit and fair reconnect. State transport/peer-selection
   assumptions explicitly before claiming catch-up or halt dissemination.
4. Stage 3 must distinguish volatile, OS-visible and durable state, process death
   and power loss. Model recovery before exposure/GC/signing, staging acknowledgments,
   independent-copy promotion, durable row publication and cleanup failure.

This integration assessment implements no new model stage. Physical power loss,
protocol-1 two-Mac completion/restart/sleep-wake and production coding-client
integration remain unqualified by the existing formal evidence.
