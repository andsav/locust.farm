# A2A and Locust

Research date: 2026-10-03. **Recommendation: keep A2A out of the October 4 release; use it only when a concrete external interoperability requirement appears.** A2A is relevant and could make a useful adapter. It does not eliminate enough of the currently planned collaboration work to justify making it Locust's core protocol.

Method: current official specification and documentation, the released [v1.0.1 specification](https://github.com/a2aproject/A2A/blob/v1.0.1/docs/specification.md), and a focused source inspection of the official Rust SDK at `2f722e393bbdd3c27749d3c8488686c69dc0e3fb`. GitHub reports [v1.0.1](https://github.com/a2aproject/A2A/releases/tag/v1.0.1) as the latest non-prerelease specification release, published May 28, 2026. Protocol negotiation uses `1.0`. No SDK was installed, compiled or exercised; runtime compatibility, footprint and implementation effort are unmeasured. The comparison targets the [Locust plan](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/implementation-plan.md), whose implementation has not started.

## What it actually provides

A2A standardizes interaction with an independently implemented agent service. A caller discovers capabilities through an Agent Card, sends messages, tracks tasks and receives output artifacts. It accommodates long-running work, progress updates, clarification and cancellation. The serving agent can keep its internal implementation private. [Core concepts](https://a2a-protocol.org/latest/topics/key-concepts/), [asynchronous operations](https://a2a-protocol.org/latest/topics/streaming-and-async/).

This is useful interoperability: a Locust participant could ask an external A2A research or coding agent to perform a task without requiring that service to install Locust. Conversely, an A2A client could request work from a Locust-backed service. The value is compatibility with another implementation, rather than merely serializing our own task records.

A2A does not require a vendor-operated central service. Discovery can use direct/private configuration. Standard bindings include JSON-RPC, gRPC and HTTP+JSON; custom bindings are supported. An Iroh-based binding is therefore possible in principle, but it needs its own transport mapping and interoperability checks. Existing HTTP clients would still need a supported endpoint or bridge. [Discovery](https://a2a-protocol.org/latest/topics/agent-discovery/), [custom bindings](https://a2a-protocol.org/latest/topics/custom-protocol-bindings/).

## What Locust would still implement

Our assessment of the overlap:

| Requirement in Locust | A2A contribution | Remaining Locust responsibility |
|---|---|---|
| Agent capability description and remote invocation | Agent Cards and standard operations | Enroll the principal and bind it to permitted goal operations |
| Progress and outputs | Tasks, messages, streaming and artifacts | Persist accepted records and map external execution to a local attempt |
| Shared board and scratchpad across disconnected peers | Can carry application data | Replication, causal dependencies, revision conflicts and missed-state reconciliation |
| Coordinator decisions and result review | Can expose related tasks and data | Signed authority history, assignment supersession and explicit result acceptance |
| Snapshots and retained patches | Can describe or reference outputs | Immutable object verification, encryption, retention and safe materialization |
| Existing coding-agent session uses the daemon | An external API could be added | CLI/skill onboarding and the actual wait/work loop |

Three contract details matter:

- **Durability:** the specification allows streaming updates to be missed after reconnection and does not guarantee complete task message history. Send-message deduplication is optional. Locust requires durable replay and request-digest idempotency. These are additional implementation guarantees, not supplied by base A2A.
- **Acceptance:** A2A treats completed tasks as terminal. Its task guide explicitly leaves artifact version linkage and client acceptance decisions to the client. That is compatible with Locust, but an external agent finishing a patch cannot advance Locust's accepted workspace head.
- **Cancellation:** `CancelTask` attempts cancellation; success is not guaranteed. An adapter must distinguish requesting cancellation from receiving the executor's reported outcome, and cannot infer reversal of external effects.

Sources: [specification sections 3.1.5, 3.3.1 and 3.7](https://github.com/a2aproject/A2A/blob/v1.0.1/docs/specification.md), [task lifecycle and artifact acceptance](https://a2a-protocol.org/latest/topics/life-of-a-task/).

These are not reasons A2A could never work. A Locust profile could add goal/attempt identifiers, signed decisions, durable reconciliation and retention semantics through extensions. A2A explicitly permits additional behavior and metadata. However, requiring all peers to implement that profile would still require a Locust-specific collaboration implementation. A generic A2A peer would not gain these behaviors automatically. [Extensions](https://a2a-protocol.org/latest/topics/extensions/).

## Rust reuse is real, but solves only part of the problem

The official Rust SDK has separate crates for protocol types, client and server operations, and several transport bindings. Its core types could be reused without adopting its entire server framework. For an actual A2A endpoint, the SDK would also supply standard routing, transport negotiation and streaming/error serialization. This is the strongest implementation argument for adoption: avoid writing a separate standard agent-service API and its serializers. [SDK at the inspected commit](https://github.com/a2aproject/a2a-rs/tree/2f722e393bbdd3c27749d3c8488686c69dc0e3fb).

The source also exposes an application-supplied `AgentExecutor` and `TaskStore`. The inspected included task store is explicitly in-memory and loses data on restart. Locust would still provide durable task transitions, authorization and event/outbox transactions; implementing the store trait alone does not supply those invariants. This is a scoped source finding, not a claim that A2A servers cannot be durable. [Executor interface](https://github.com/a2aproject/a2a-rs/blob/2f722e393bbdd3c27749d3c8488686c69dc0e3fb/a2a-server/src/executor.rs), [storage interface](https://github.com/a2aproject/a2a-rs/blob/2f722e393bbdd3c27749d3c8488686c69dc0e3fb/a2a-server/src/task_store/store.rs), [in-memory implementation](https://github.com/a2aproject/a2a-rs/blob/2f722e393bbdd3c27749d3c8488686c69dc0e3fb/a2a-server/src/task_store/inmemory.rs).

In the planned first release, every collaborating participant runs Locust and operates it through the CLI and skill. No required external A2A endpoint or consumer has been identified. Adopting the types or defining a custom binding now would introduce another contract to maintain while leaving the core shared-state work in place. There is no measured implementation saving to justify that addition.

## If a real integration is needed later

Use an adapter over Locust's existing authorization and task engine, with one negotiated A2A version and one standard binding initially. Do not build a parallel scheduler or make an A2A service a prerequisite for ordinary Locust peers.

Proposed mapping, requiring implementation and interoperability tests:

1. A permitted Locust assignment can call a configured external A2A service. Store the association between goal/task/attempt, endpoint, A2A context and A2A task; external IDs alone are not authorization.
2. Persist and validate returned artifacts before recording a result submission. External `completed` reports that the provider's work finished; coordinator acceptance remains a separate Locust decision. A new execution or refinement after a terminal task uses a new Locust attempt and A2A task. A delivery retry after a lost response must reconcile the existing request instead of automatically creating another execution.
3. For inbound requests, authenticate and scope them before creating a Locust offer. The remote request cannot directly launch shell commands or claim coordinator authority. Define whether the exposed service promises a worker output or an accepted contribution, and report completion at that promised boundary.
4. Test lost responses, repeat sends, disconnection, cancellation races and unavailable artifacts against an independent implementation. Do not retry uncertain non-idempotent work merely because a message has an ID.

**Release action:** keep the current daemon/P2P/CLI/skill design. Add no A2A dependency, compatibility claim or mandatory implementation milestone now. Revisit when a named integration would let a user do something the native Locust workflow cannot.
