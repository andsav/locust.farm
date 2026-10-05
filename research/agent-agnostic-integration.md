# Agent integration and local execution boundaries

Locust separates durable collaboration from model invocation and local execution.
The daemon's CLI and stdio MCP bridge expose the same authenticated operations.
An agent does not need to implement the peer transport. The
[architecture guide](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/guide/architecture.md) describes the implemented split;
this note identifies the additional claims an execution adapter must establish.

## Three interfaces

| Interface | Responsibility | Current source |
| --- | --- | --- |
| Peer transport | Exchange authenticated collaboration events and retained artifacts | [locust-net](../crates/locust-net/src/lib.rs) |
| Local agent API | Validate credentials, membership, rules, local grants and request identity | [typed contract](../crates/locust-proto/src/api.rs), [MCP bridge](../crates/locust/src/mcp.rs) |
| Execution adapter | Configure a client, bind its native session and observe managed lifecycle | [locust-adapter](../crates/locust-adapter/src/lib.rs), [managed sessions](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/managed-clients.md) |

Successful MCP registration does not prove idle wake, native resume, cancellation
or execution confinement. The [client harnesses](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/client-qualification.md)
measure these separately with identified client versions, profiles and providers.
The [hcom source assessment](hcom-dissection.md) is public prior art for lifecycle
patterns, not a runtime dependency.

## A remote task is an offer

A signed task does not grant permission to run commands on another machine.
Each host controls its filesystem, network, credentials, model accounts and
spending. Locust checks local execution grants, but an existing client's own
shell and tools remain governed by that client's native policy.

A stronger worker sandbox would need one host-owned runner covering every
execution path: file tools, subprocesses, descendants, hooks and extensions.
Task text, repository instructions and child content must not widen it. A
materialized workspace or Git worktree is not an OS isolation boundary.

## Qualify a sandbox claim explicitly

An adapter proposing confinement should test a useful edit/test/patch workflow
alongside denied outside-root reads and writes, credential access, unapproved
networking and descendant privilege widening. Keep signing and administrative
keys in the trusted host. Export only the selected patch/artifacts through a
checked operation and reconcile interrupted launch/result receipts before retry.

These are requirements for a future execution runner. This repository's local
API authorization, scripted-client network guard and managed-process cleanup
do not establish universal worker confinement or automatic closed-session wake.
