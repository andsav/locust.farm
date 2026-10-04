# T2 integration findings

Date: 2026-10-03 (local). Status: fixes implemented; final workspace integration
checks are recorded in [the T2 workflow](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/t2-workflow.md). This is source
and local deterministic evidence, not a real-client or physical-network result.
The owner explicitly keeps live qualification in a separate testing effort.

## Missing nested-content replication

The T1 replica indexed objects directly named by event headers and local blob
records. An event could carry a snapshot manifest while the files it named
remained unknown on another participant. The contribution flow therefore needed
an explicit graph, not only CLI commands that upload a manifest.

The [typed graph](../crates/locust-core/src/node/content_graph.rs) follows only
specified container positions: task inputs/result bases/accepted heads are
manifests; result patches are canonical contributions. It authenticates a
container for its goal before creating descendants. File entries and ordinary
artifacts stay opaque. Per-path epoch ceilings and reader requirements preserve
removed-reader restrictions, including an old file referenced by a new-epoch
container. Received children never gain independent local publication records.
The [nine regressions](../crates/locust-core/src/node/content_graph_tests.rs)
exercise real Node/Driver exchanges, a child ordered before its parent hash,
late keys, reopen, withdrawals with alternate references, conflicting lengths,
malformed/cross-goal containers, and a removed principal plus its viewer.

## Concurrent materialization could mix snapshots

Independent review found that the previous materializer used one predictable
stage per destination, deleted it on each attempt, and used a replace-capable
rename after a separate destination-existence check. If two callers interleaved
fetches, one could write into the other's stage and publish mixed files. A new
empty destination created between the check and rename could also be replaced.

The [fix](../crates/locust-workspace/src/materialize.rs) gives every invocation
its own private stage, writes relative to open directory descriptors without
following received-path symlinks, and publishes with `NOREPLACE`. Failures keep
that invocation's stage and report its path. No attempt deletes another's
leftovers. [Deterministic tests](../crates/locust-workspace/tests/materialize.rs)
interleave two stages after their first files, create a destination during
fetch, and replace a staging ancestor with a symlink. The published winner is
one exact snapshot; the independent destination and outside directory survive.
This replaces the earlier cleanup-on-retry convention with explicit recovery.

## Failure and session boundaries

The CLI initially lost the daemon's `Unavailable` code through an `io::Error`
wrapper. Traversing only `Error::source()` skipped the contained `Failure`.
The [mapping](../crates/locust/src/cli/workspace.rs) now also inspects `get_ref()`;
[CLI fixtures](../crates/locust/tests/workspace.rs) enforce stable failure codes,
exact base/patch/head submission, binding and claim requirements, accepted vs
integrated state, and preservation of unrelated local work.

The [real-core workflow](../crates/locust/tests/t2_flow.rs) uses the production
Node with a memory store behind a local Unix API fixture, a real MCP subprocess
and CLI subprocesses. It exercises authenticated enrollment, goal creation,
sealed snapshot export, MCP task proposal/assignment/claim, materialization,
selected-file contribution, review, submission, acceptance, application and
reported integration. The transport endpoint is deterministic test input; no
Iroh connection, installed coding client or external model is involved.

The [MCP tests](../crates/locust/src/mcp/tests.rs) cover the actual credential
class, protocol negotiation, exact tool exposure, cancellation, EOF, output
failure and queued-worker cancellation. The [binary tests](../crates/locust/tests/mcp.rs)
exercise operating-system pipes and keep transport stdout free of CLI errors.
Cancellation means disconnecting the call, not rolling back an already recorded
mutation. Locust does not infer that a cancelled reply proves non-execution.

## Scope retained

The operating skill was independently reviewed against actual commands and
schemas. The pass corrected missing invitation admission/task-claim steps and
removed an implication that preview returns stored content IDs. Its structure
validator passes; behavioral validation here is a source-based forward review,
not a claim of real-model skill use.

The accepted [T2 workflow](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/t2-workflow.md) documents these choices and
remaining proof boundaries. Managed client launch, installer behavior, signed
release artifacts and publication are not part of this checkpoint.
