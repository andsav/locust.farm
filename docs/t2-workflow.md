# T2 coding-agent workflow

> **Historical pre-organization document.** Its protocol-1 commands, source
> snapshots and qualification claims do not apply to API 2 / protocol 2. Read
> [the current manual](guide/overview.md) and
> [implementation status](formations-status.md) for current behavior.
> No legacy reader, migration or old-runtime support is provided.

Status: implemented and locally verified, October 3, 2026. This document records
the initial T2 checkpoint at `8a7d170`: the real stdio MCP bridge, operating skill
and snapshot/contribution workflow. Subsequent [production client campaigns](../research/t2-production-qualification.md)
and [managed-session implementation](managed-clients.md) have separate evidence.
Installation, signed packaging and automatic wake remain later work. The owner
handles physical-machine qualification; component and CLI fixture checks here
do not replace it.

## Local MCP server

Run the foreground daemon with its existing private home. Enroll an agent and
create a protected session using the existing CLI. Register `locust mcp` with
absolute `LOCUST_HOME`, `LOCUST_CREDENTIAL` and `LOCUST_SESSION` environment
values, or pass the corresponding command flags. Keep session and credential
files private. The [adapter configuration library](../crates/locust-adapter/src/config.rs)
emits references to these paths. This initial T2 path does not install or launch
a client; the later managed CLI supplies explicit foreground launch.

The [bridge](../crates/locust/src/mcp.rs) implements newline-delimited JSON-RPC
with MCP negotiation for 2025-11-25, 2025-06-18 and 2025-03-26. It checks the
actual daemon caller and refuses owner credentials. Its
[tool schemas](../crates/locust/src/mcp/schema.rs) cover exactly the local API
operations marked `tool`; no shell, file transfer, enrollment, session management
or daemon shutdown is exposed. Tool failures carry the stable API error code and
`isError`; recent protocol versions also receive structured content. Read-only
annotations describe operations, not whether received content is trustworthy.

Each tool call has its own authenticated socket and the same explicit session.
Cancellation shuts down that call's socket; EOF, stdout closure or write
failure cancels and joins active calls. The OS-pipe observer detects reader
closure even when only daemon waits remain and stdin is still open. A mutation
can commit before cancellation, so inspect state
or retry with the same idempotency key and identical arguments. The bridge
backpressures output but is a local trusted-client interface, not a resource
isolation boundary for arbitrarily many concurrent requests.

Use the [packaged operating skill](../skills/locust/SKILL.md) for coordinator and
worker behavior. T2 adds the source skill to the repository; it does not alter
any user's installed skills or client profiles.

## Snapshots and contributions

All filesystem actions run in the trusted CLI through
[workspace commands](../crates/locust/src/cli/workspace.rs). The daemon stores
binding strings and content but never opens the supplied paths.

| Command | Effect |
|---|---|
| `workspace preview --root ROOT --commit COMMIT` | Reads the committed export scope locally and reports paths, exclusions and refusals; stores no blobs |
| `workspace export --goal GOAL --root ROOT --commit COMMIT` | Stores a snapshot and records its exact manifest, provenance commit and chosen root |
| `workspace materialize --goal GOAL --manifest INPUT --destination NEW` | Fetches a manifest and its files into a new directory; records the destination after success |
| `patch create --goal GOAL --base INPUT --root ROOT --commit COMMIT` | Captures a named committed tree against the exact base |
| `patch create ... --path FILE --path OTHER` | Captures only explicitly named regular files, including deletion of selected missing base files |
| `patch review --goal GOAL --patch PATCH` | Validates the exact delta and renders text diffs or binary/mode summaries |
| `patch submit --goal GOAL --patch PATCH --assignment ASSIGNMENT --generation N SUMMARY` | Validates and submits the contribution with its base and head under the claimed session |
| `patch accept --goal GOAL --result RESULT --patch PATCH` | Checks the result/patch/base relationship and accepts the exact head |
| `patch apply --goal GOAL --patch PATCH --root ROOT --expected-base BASE` | Applies a currently accepted contribution, preserves originals and records local integration after success |

Paths are absolute local selections. An exported Git root also requires
`--expected-git-head FULL_COMMIT` for application. Capture and application must
use a root already recorded by export or materialization. CLI `--json` produces
one response envelope. Composite workspace commands do not accept a single
`--idempotency-key`; retry them using the same immutable identifiers.

The [versioned contribution format](../crates/locust-proto/src/contribution.rs)
is inert content: a tagged canonical encoding of its exact base/head manifest
identifiers and sorted before/after file entries. It adds no command execution
or new signed-event variants. [Validation](../crates/locust-workspace/src/contribution.rs)
requires the delta to equal the full difference between those two manifests.
The workspace handles regular files and an executable bit, including binary
content, additions, edits, deletions and file/directory transitions. It refuses
symlinks, hardlinks, submodules, special files and private or reserved paths.

[Application](../crates/locust-workspace/src/apply.rs) preflights all changed paths
against the exact before or already-applied after state. It uses descriptor-relative
no-follow operations and retains originals, an inert contribution plan and path
mapping in a private recovery directory. Unrelated local work is preserved.
Materialization uses a private unique staging directory and no-replace publication,
so concurrent attempts cannot mix files or overwrite a destination that appeared
during the operation. Failed/interrupted stages remain for explicit recovery;
their names are excluded from subsequent exports.

A conflict records no integration. Retrying the same contribution recognizes
already-applied files. Recovery is explicit; Locust does not run Git reset,
commit, hooks, arbitrary patch commands or received code. The integration marker
records successful application of that contribution; it is not an assertion
that the checkout has no unrelated modifications.

Acceptance and local integration remain separate. Materializing a task input
records integration only if that exact input is the locally accepted head.
Exporting a commit clears a previous integration marker because it does not
inspect the current working copy. If files change successfully but recording the
binding fails, the CLI reports that boundary instead of claiming full success.

## Replicating referenced content

A snapshot or contribution is more than the event's directly named object.
Typed roots and containers determine which descendants may be fetched; ordinary
file or artifact bytes are never guessed to be manifests. A task input, result
base or accepted head is a manifest; a result patch is a contribution, which
names base and head manifests; manifests name regular file blobs with exact
plaintext lengths. Container traversal and plaintext reads verify goal-bound
authenticated sealing;
transfer verifies content hashes and permitted length/epoch associations.

Descendant access inherits the root and container key-epoch constraints. A
removed reader must not gain new access to an old file only because a newer
container mentions it. Another independently valid older path may still grant
access. Withdrawal removes that path's reachability; received descendants do
not become independent permanent local roots. Missing content is requested and
reported unavailable; no partial success is invented.

The [integration findings](../research/t2-integration-review.md) preserve the
reviewed defects, fixes and source evidence.

## Verification boundary

The integrated workspace passed `cargo fmt --all --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo test --locked --workspace`: **483 passed, zero failed, nine explicit
ignores**. The ignores retain the existing installed-client, opt-in network and
storage fault-harness boundaries; they were not silently counted as passes.
Documentation/link checks and the skill structure validator passed.

The suite includes nine graph regressions, 32 workspace tests, four MCP binary
pipe tests, three workspace CLI fixtures and the real-core MCP/CLI workflow.
Independent Sol review covered the workspace implementation, CLI failure/retry
behavior, content graph and operating-skill steps. These checks exercise
production code with local deterministic fixtures. Subsequent actual-client and
interruption/resume campaigns are recorded in the [production findings](../research/t2-production-qualification.md).
Default interactive approval, physical machines and separate accounts remain
qualification work. No T2 release gate is closed by a component pass.
