# Organization formal verification plan

**Status: current protocol/API-2 bounded models implemented; verification results
and exact limits are recorded in the [organization model map](../research/tla/organization.md).**
The modeled source baseline is `c88e3bc960de79eb990b3185a653bd982e587ed6`.
Historical protocol-0/1 executable models were replaced; their
[prose and evidence](../research/evidence/tla/README.md) retain pinned source links.
There is no migration model or parallel legacy execution path.

The goal is to make authority, proof retention and recovery failures reproducible
before relying on larger integration tests. A completed finite TLC check examines
all behaviors in its declared configuration. It does not prove arbitrary swarm
sizes, all event bodies, or correspondence between Rust and TLA+. The Rust
regressions linked in the model map are separate implementation evidence.

## Implemented model boundaries

| Model | Current subset | Important limits |
| --- | --- | --- |
| [Organization](../research/tla/Organization.tla) | Narrow administrator governance; membership tenure/cutoff; immutable definition and rule pins; taskless contributions; distinct positive reviewers; scope-local exact proof closure and named selection; late forks, missing dependencies, removal/readmission and incompatible branch combinations. | Finite authenticated transcripts from a verified founding prefix, abstract immutable definitions and two fixed thresholds. No arbitrary signed-body generation, hash/signature implementation, encryption epochs, all selectors, document payloads, or closure/reopen actions. |
| [Attempt sessions](../research/tla/AttemptSessions.tla) | Attempts have local claims; session/principal binding; local permission distinct from shared eligibility; per-attempt generation fencing; independent sessions/attempts; caller-keyed retries; all-or-none uncertain commit and reopen. | One or two finite attempts, two sessions, finite generations/events. One worker identity is shared-eligible. Verified round/member/cancellation changes are supplied as external inputs; permission grants are aggregated in this subset. No distributed lease or guarantee that only one physical executor is running. |
| [Flow effects](../research/tla/FlowEffects.tla) | Witness-independent logical effect id; two pinned rounds; multiple signatures for one logical task; daemon materialization without agent polling; durable outbox reconstruction; distinct delivery, recipient acknowledgment and locally permitted start; evidence retraction. | Two configured logical effects and three authenticated signatures; configured signer/recipient validity is assumed. Atomic commit is abstract; no queue scheduling fairness, transport delivery guarantee or physical process/power-loss model. |

The [case registry](../research/tla/cases.json) distinguishes safety cases,
negated reachability witnesses and deliberately broken mutations. An expected
counterexample is evidence of that specified witness/mutation, not a product bug
and not passing verification of the violated invariant.

## Verification and maintenance

Run the commands in the [model guide](../research/tla/README.md). The pinned runner
requires normal completion and an empty queue for safety success. It freezes
models/configurations, records source hashes, and checks the exact property and
state requirements for counterexamples. Unknown outcomes, changed source during
a run, parser errors, resource exhaustion and explicit deadline expiry are failures.
No implicit wall-time cap is added. The finite model bounds and configurable Java
heap allocation are stated separately from product behavior.

For every protocol change, update the model action/property mapping and its Rust
regression links together. Keep a current executable model only. When a contract
is replaced, preserve useful historical results with their original source
identity; remove superseded executable models/configurations from current suites.
Do not claim conformance merely because both implementations use similarly named
operations or a projection is defined twice by the same mathematical function.
The organization's `ReplayMatchesHeld` checks fresh projection maintenance across
delivery, not an independently proved incremental replay algorithm.

## Remaining formal work

1. Generate a richer bounded family of signed event bodies, roles, task/document
   rounds, closure streams and malformed dependencies; relate it to byte-level
   Rust transcripts rather than only matching scenario descriptions.
2. Model durable flow dispatch scheduling, retry and acknowledgment under explicit
   fairness assumptions. The current effects model proves safety/reachability in
   its finite subset, not eventual delivery.
3. Model replication frontiers, missing-content acquisition and retention across
   partitions. Deterministic equal-set replay and eventual set acquisition are
   different claims.
4. Refine atomic storage into process crash, flush/WAL and power-loss states, then
   compare against storage fault-injection tests. Atomic models cannot prove
   SQLite or filesystem durability.
5. Add observed CI qualification for model suites and investigate deductive proofs
   only after interfaces and invariants stabilize. Existing bootstrap workflow
   configuration alone does not establish remote execution.
