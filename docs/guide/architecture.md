# Architecture and local boundaries

**Status: accepted design; organization runtime replacement is in progress.**
The daemon, local harness and distributed participants have separate duties.
This page describes those boundaries rather than claiming runtime qualification.

## The daemon is the authority

Locust stores signed collaboration events and retained artifacts. Its pure goal
engine derives a view from verified evidence; network reception order and clocks
are not a decision rule. The daemon checks authenticated membership, scoped rules
and local grants before it materializes configured actions. A visual display or
agent transcript does not replace this state.

A local API and stdio MCP bridge let an agent inspect and contribute through
explicit operations. The local API is not a public HTTP API. A replica can lack
an artifact or proof even when it has the referring event: that action stays
pending verification instead of pretending there is no evidence.

## Harnesses execute local work

A harness supplies the model, tools, account and native approval policy. Locust
attributes its contribution and offers work, but does not acquire unlimited
filesystem or spending authority. Receiving an offer is distinct from accepting
it, obtaining a local grant and observing a process start. An existing shell
available to a harness remains governed by the person's native tool policy.

Merak can be a local executor. Remote participants are Locust identities, not
synthetic local Merak runs. Read [agent integration](agents.md) before interpreting
configured tools as model-visible capability.

## Transport and replicated evidence

Transport exchanges authenticated event and content data. Disconnected replicas
can continue independently authorized work when required proofs are available.
They cannot invent a missing membership proof or take over unavailable authority.
Reconnect reevaluates evidence; later forks or cutoffs can change an observed
outcome. [Recovery](recovery.md) explains those states.

## Optional Polaris

Polaris is a planned visual interface to the same authoritative contract. Its
canvas layout is presentation. Native connector access, authoring operations and
browser screenshots are separately qualified; none follows from this site build.
Locust remains independently usable. Read [Polaris](polaris.md) for its intended
round-trip and unsupported-input behavior.

The exact [protocol obligations](../organization-blueprints-semantics.md) describe
the signed context and durable delivery required by this architecture.
