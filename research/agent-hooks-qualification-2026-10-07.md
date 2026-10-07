# Agent hooks: implementation and isolated qualification

This note separates pure rules, process checks, native harness behavior and
model behavior. Phase state and landed commits belong to the A4 and H rows in
the [master plan](../docs/master-plan.md). The accepted requirements are in the
[agent memory and store plan](../docs/agent-memory-and-store-plan.md).

## Shape and evidence

R7's run text names no harnesses at this baseline, so H1a uses Codex and
Claude Code, from different vendors. The
[pure core](../crates/locust-adapter/src/hooks/core.rs) owns the three events,
fixed ASCII lines shorter than 512 bytes, worker classification and cumulative
stop marks. It contains no harness names or model decisions. Tests replay a
fake harness. The [adapters](../crates/locust-adapter/src/hooks.rs) own native
events, identity and successful tool-envelope parsing, output envelopes,
timeouts, config paths and owned entry installation/removal.

The [runtime](../crates/locust/src/hook.rs) authenticates an agent and only
reads status, pending, wait and cancellation events. [Private marks](../crates/locust/src/hook/marks.rs)
are scoped by credential, execution session and a hash of native chat identity.
They use private owned directories and files, reject symlinks, serialize chat
updates and atomically replace their JSON. An unassociated native chat stays silent even when the shared profile session
holds a claim. Successful tool evidence establishes that chat first; A4
instructions cover a new chat. One absolute invocation deadline also covers
lock contention and slow fragmented socket reads. A separate nonblocking session lock
allows one waiting chat. Each goal has its own parked connection; an answer
closes the others. No hook signs or syncs a record.

An ignored stop reminder does not start another wait. The shown-ID set grows
across alternating pending sets, and a successful Locust write from that chat
clears it. Replayed native tool invocation IDs cannot clear it twice. Tools
from another server, failed calls and untyped result text do not establish
Locust activity. Every model-visible line uses counts, typed IDs and fixed tool
names; task titles, transcript content and native tool text do not enter it.

`WorkItem.unattended` comes from
[`Goal::unattended`](../crates/locust-core/src/goal/mod.rs). It includes any
running attempt in the current round, including the caller's own principal
and attempts without a local claim. This is the predicate for A2 to reuse.
The `attempting` list intentionally excludes the caller and is insufficient.
[Regression tests](../crates/locust-core/src/node/tests/context_views.rs)
exercise another session of the same principal, a missing claim, a failed
attempt and ordinary other-member attempts. Only the local API gains a field;
signed event bytes and sync records do not change.

## Native contracts

- Codex uses `.codex/hooks.json`: `SessionStart` (including compact), `Stop`
  and `PostToolUse`. The stop envelope is `decision: block` with `reason`;
  start/tool lines use `hookSpecificOutput.additionalContext`.
  Sources: [official hooks guide](https://learn.chatgpt.com/docs/hooks),
  [0.153.4 post-tool implementation](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/hooks/src/events/post_tool_use.rs).
  The pinned source includes `agent_id` for subagent tool calls; these adapters
  cover root chats and do not attribute those calls to the parent. Native hook
  trust review is separate from file installation.
- Claude Code uses `.claude/settings.json`: the same events, plus
  `PostToolUseFailure` for post-tool polling without successful activity.
  Sources: [official hook reference](https://code.claude.com/docs/en/hooks),
  [CLI reference](https://code.claude.com/docs/en/cli-reference).
  Real hook checks load the private settings explicitly; `--bare` skips hooks
  and remains the default for other real-model checks.

Both stop commands allow 300 seconds, with at most 270 seconds of waiting in
the core runtime. The line contract is identical; native JSON is an adapter
detail. No model is selected or detected by the implementation.

## Verification results

The initial A4 instruction commit `c5623d7` passed formatting, strict workspace
Clippy, workspace tests and the documentation checker before landing on main.
Its CLI claim lines remain separate because G1 already edits presentation.

H1a focused checks passed: the adapter/core tests, 14 binary integration
tests, 31 setup tests, 4 private-mark tests, the absolute-deadline test, the
authoritative unattended-task regression and 13 Python real-profile tests. The binary fixture permits only
read-only daemon requests, checks both adapters' equal outcomes, races eight
stops, covers halted/joining goals, and verifies owner/failure/off behavior.
Setup checks cover unchanged-byte restoration, unrelated edits, hand-removed
entries, modified-entry refusal and interrupted journals. The v2 ownership
record is refused with recovery instructions, without a migration reader.

The full workspace passed formatting, strict Clippy and tests (1,210 passed,
14 ignored). The initial full Python suite passed (309 tests, 3 skipped); final hook-check
helper coverage adds three tests, and all 9 hook-check tests pass. The docs
checker and formation contract verification passed. A full run intermittently failed the existing two-daemon reconciliation test
`a_diverged_author_log_reconciles_between_two_real_daemons` with a rejected
post; the focused rerun passed. The final required full workspace run passed after all changes.
No reconciliation code was changed in this lane.

## Limits and follow-ups

H3 needs G1 on main and a window without unmerged G2/E2 status changes.
Until then, start hooks can remind a chat of held attempts but do not reset
acknowledgments or receipt caches. Status does not report chat presence or
observed hooks. A4's instruction to start with `locust_status` remains the
fallback after context loss. Missing stop/tool events use `locust_wait` and
`locust_pending` respectively. `client run` disables these hooks.

The interactive question of whether a waiting hook holds a newly typed prompt
requires a native interactive check for each harness; process-level replay
does not establish it. This work does not qualify two-computer collaboration,
closed-chat wakeups or release readiness.

## H1a isolated native checks

Both adapters passed the same process-level scenario through their generated
setup command: waiting work blocks once, a second stop passes, a cancellation
reaches the stop callback, and compaction restores the held-attempt line.
Setup removal restored the exact original hook and MCP configuration bytes.

The final native run used [`scripts/check_hooks.py`](../scripts/check_hooks.py)
with a test-signed package of the uncommitted source candidate, embedded build
label `c5623d7`. This is source/process qualification, not release provenance or
a trusted production-package check. Every daemon and client ran with a fresh
`mktemp -d /tmp/lh.XXXXXX` HOME. API keys came only from the environment; no
login was copied. The disposable profile directories were removed afterward.

| Harness | Model | Native result |
| --- | --- | --- |
| Codex 0.153.4 | `gpt-5.4-mini-2026-03-17` | One successful `locust_wait`; native Stop blocked, the client continued, the next Stop returned no output, and the client exited 0 without timeout. |
| Claude Code 2.1.280 | `claude-haiku-4-5-20251001` | Same sequence and result. Its native successful MCP callback supplies a JSON string containing the exact typed Locust envelope. |

Qualification wrapped only the generated hooks, forwarding their original
stdin and stdout unchanged. It retained event/tool names, JSON key/type shapes
and booleans; no transcript, request values or credentials are retained in the
report. The final report is disposable `output/hooks-qualification-final.json`.
Codex used its documented one-off hook-trust bypass for the reviewed hooks in
that disposable profile. Claude explicitly loaded the setup-created private
MCP config, enabled strict MCP selection, and allowed only `locust_wait` and
`ToolSearch`. These overrides qualify native transport and continuation; normal
setup still requires the harness's own permission/trust review.

The first native attempts exposed qualification errors (tool discovery without
execution, a consumed offer and an unsupported Codex test-config key) and the
Claude string-envelope parser gap. Those were corrected before the final run.
The final proof requires observed hook callbacks and natural client exit, not
assistant-written JSON or the presence of marks alone.

## H2: notices after tools

Every tool callback from an associated chat reads status, then uses
`wait(timeout_ms=0)` for each known eligible goal. An unchanged answer does not
read pending again. A changed wait already returns the complete pending view;
only a newly discovered goal needs a separate pending read. Omitted or halted
goals preserve their previous baseline instead of inventing a claim loss.

The pure core records cancellation IDs and unexplained lost claim generations.
Start and stop snapshots reconcile the same baseline, so observing a change
there cannot consume a later tool notice. The per-chat lock covers observation,
polling, reconciliation and saving; concurrent callbacks deliver a notice once.
An informational cancellation or held-attempt line also records its displayed
work IDs for the stop rule. The core separately remembers an actual stop block,
so an informational line does not disable an idle worker's wait.

Native adapters only copy typed action fields. The core interprets terminal
reports, including `uncertain`, and terminal cancellation acknowledgments;
`uncertain` acknowledgments do not end a claim. It matches the exact goal,
attempt and generation. An acknowledgment without a generation proves that no
claim was held at that call, so it cannot explain an earlier claim loss.

If a terminal acknowledgment's cancellation target was not observed, the
runtime reads that exact effective cancellation event. It persists the pending
release before resolving the target, retries after a read failure, and defers
only that goal's reconciliation. A removed goal's unresolved release cannot
disable other goals. Previously associated chats retain a validated successful
own action before reconnecting, so a failed handshake does not erase that fact.
First association still requires live agent authentication.

Post-tool delivery can arrive out of order: another callback may observe a
disappearance before the callback for this chat's successful terminal action
has arrived. The hook cannot attribute an unseen callback. Filesystem failures
that prevent saving observations likewise retain the fixed failure line and
MCP fallback. These checks do not prove native callback ordering or durability
when local storage fails.

H2 verification passed all 27 binary hook tests, the pure core and adapter
checks, 22 Python hook/profile checks, formatting and strict workspace Clippy.
The isolated real-daemon replay passed the same scenario for both adapters,
including a cancellation first observed at stop, one later tool notice and no
false lost-claim notice after this chat's successful cancellation acknowledgment.
The test-signed candidate carries the prior commit label `965426f`; it is not
release provenance. No additional real-model run was selected for H2.

The required full workspace command failed twice, and a serial full run failed
at the same two-daemon reconciliation test named above: `agent-2`'s post was
rejected as not applicable. Its focused rerun passed. These failures are retained
as failures, not treated as a passing gate; this lane changes no reconciliation
code. H2 remains unqualified by the full workspace gate until that failure is
resolved or a complete required run passes.

A separate full workspace run excluding only that named reconciliation test
passed: 1236 tests passed, 14 ignored, one filtered out. This completes the
remaining coverage but does not replace the failing required command.

The subsequent complete required workspace run, after the generic adapter
renderer gained an explicit absent-file result, passed (1,237 passed, 14 ignored).
Formatting and strict workspace Clippy also passed. That run includes H2 and the
previously failing reconciliation test; the earlier failures above remain part
of the qualification record. The renderer change lets an adapter remove an
owned source file without leaving an invalid empty extension on disk.

## H1b: Droid

The Droid adapter uses `.factory/hooks.json`, whose document is the event map
without a `hooks` wrapper. `SessionStart`, `Stop` and `PostToolUse` map to the
same core events. The adapter normalizes only the native document and envelope;
setup and the runtime do not branch on Droid. Stop has a 300-second native
limit, leaving the shared 270-second wait. The other callbacks also explicitly
set 300 seconds so the native timeout outlasts the generic runtime deadline. See the [Factory hook contract](https://docs.factory.com/harness/hooks).

An isolated native probe of Droid 0.218.1 observed the tool name
`locust___locust_*` (the preliminary fixture used the same server/tool separator)
and a JSON-string Locust result envelope. Its tool hooks supplied no invocation
ID, unlike the stream records. The adapter validates the typed request and
response, then identifies the logical successful effect for replay suppression.
Repeated identical effects can coalesce; this is not a fabricated unique native
invocation ID. Each callback still polls for H2 notices.

The native root probe ran with an ambient OpenAI key and BYOK `gpt-5.4-mini`,
without a Factory key or copied login. Its root tool call succeeded. A separate
child probe reached a healthy private Droid daemon but failed before any child
callback because inherited-user authentication had no access token. Child hook
routing remains unverified. The adapter uses the supplied nonempty session ID;
it does not infer parentage or merge chat IDs. Automatic updates are disabled in
qualification profiles. [Factory BYOK](https://docs.factory.com/model-independence/byok)
and the [CLI reference](https://docs.factory.com/droid-cli/cli-reference) describe
the native configuration and command controls.

The Locust-native qualification then passed on Droid 0.218.1 with OpenAI
`gpt-5.4-mini-2026-03-17`. The actual harness completed one registered
`locust_wait`, received a native Stop block, continued, received empty output
from the next Stop and exited 0 without a timeout. Shape-only instrumentation
observed those callbacks while forwarding the generated command's input/output
unchanged. No hook failure line appeared. The private home was removed.

The same installed-command scenario also passed work waiting, one block,
an ignored block, cancellation at stop and then once at a tool callback,
compaction recovery, and no false claim loss after the chat's terminal
acknowledgment. Setup removal restored the original hook and MCP bytes exactly.
The synthetic signed candidate contains the uncommitted Droid adapter and
carries the prior commit label `7a0a0cd`; this is not release provenance.
The disposable report is `output/hooks-droid-qualification.json`.

The native run used high autonomy with an exact wait/tool-search selector and
a private qualification guard that denied all other tools. The tool catalog
still advertised other registered Locust tools; the guard supplied an additional
read restriction for this synthetic run. These are recorded qualification overrides;
normal setup does not change native approval settings or install a guard.

Droid's final required checks passed: formatting, strict workspace Clippy,
the full workspace suite (1,239 passed, 14 ignored) and the docs checker.
Focused coverage includes 42 adapter/core, 27 hook subprocess, 31 setup and
27 Python hook/provider-profile tests. A first workspace run exposed a test
fixture that sent a Codex payload directly to the Droid adapter on the missing
daemon path; the fixture now normalizes native input consistently. Production
behavior did not change after the successful native qualification.

## H1b: Pi

The [Pi adapter](../crates/locust-adapter/src/hooks/pi.rs) installs one owned `.pi/agent/extensions/locust.ts` extension.
Its API contract is pinned to `@earendil-works/pi-coding-agent` 1.0.4, commit
`2db5e359bf84c1c0be51d2c5c5c5c7cf27072c2b`. The [event types](https://github.com/earendil-works/pi/blob/2db5e359bf84c1c0be51d2c5c5c5c7cf27072c2b/packages/coding-agent/src/core/extensions/types.ts)
and [extension runner](https://github.com/earendil-works/pi/blob/2db5e359bf84c1c0be51d2c5c5c5c7cf27072c2b/packages/coding-agent/src/core/extensions/runner.ts)
define the event mapping and replacement semantics. `session_start` and
`session_compact` map to start; `tool_result` maps to tool;
`agent_before_settle` maps to stop. Chat identity is the native session manager's
session ID. A `parentToolCallId` identifies a nested tool call in that same
session, not a separate child agent.

The extension spawns only the setup-bound launcher with literal arguments and
JSON stdin. It appends the core's one line to the native tool content while
preserving the original structured result, details, error status and usage.
Pi's [MCP projection](https://github.com/earendil-works/pi/blob/2db5e359bf84c1c0be51d2c5c5c5c7cf27072c2b/packages/coding-agent/src/extensions/mcp/tools.ts)
nests the entire MCP result inside native structured content; the adapter checks
the exact server/tool metadata and both error layers before accepting an own call.
Settlement preserves prior extension entries and their continuation request.
Abort and error outcomes do not launch a stop hook. Abort, timeout and shutdown
reap the owned process before returning; changed sessions discard late output.
The shim uses the adapter's 300-second timeout and the shared 270-second wait.

Setup treats an existing empty source file as an occupied path. Modified owned
source conflicts; a hand-deleted extension stays absent on reapply. Removal
produces no file, including after a mode-only edit, because an empty `.ts` file
is not a valid Pi extension factory. See the [native loader](https://github.com/earendil-works/pi/blob/2db5e359bf84c1c0be51d2c5c5c5c7cf27072c2b/packages/coding-agent/src/core/extensions/loader.ts).
The generated shim receives the failure line from the core. It contains no
Locust work decisions or model selection.

Pi was not installed on this machine, so native harness and real-model checks
are not run. Node 22.22.0 can strip the type-only imports and exercise the
extension against a fake API without installing Pi. That evidence does not
establish native extension discovery, interactive prompt handling or third-party
subagent-extension routing.

The installed-extension replay passed the same real-daemon scenario used by
the other adapters: waiting work, a block, an ignored block, cancellation at
stop and then once at a tool callback, compaction recovery, and no false claim
loss after this chat's terminal acknowledgment. The fake Pi API loaded the
actual setup-generated source, which spawned the bound installed launcher.
It verified that the original nested MCP result and native tool data survived
line injection. Setup removal restored the MCP bytes and the extension's
original absence. The temporary profile was removed.

The synthetic signed candidate carries the prior commit label `42c348d` and
includes the uncommitted Pi adapter. This is source/process qualification, not
release provenance. The disposable report is `output/hooks-pi-qualification.json`.
The driver is [check_pi_hook_driver.mjs](../scripts/check_pi_hook_driver.mjs),
called by [check_hooks.py](../scripts/check_hooks.py). Both native Pi execution
and real-model execution remain explicitly `not_run` because Pi is missing.

Before the Pi commit, formatting, strict workspace clippy and the full workspace
test suite passed: 1,250 tests passed and 14 were ignored. Focused coverage
includes 49 adapter/core, 28 hook subprocess, 35 setup and 12 Node fake API tests.
The 35 Python hook/profile helper tests and staged documentation checks also
passed. Native Pi remains unverified for the reason above.

## Combined verification after A1

The hook commits were rebased onto main at `8f27419`, including A1's operation
descriptions and context-page limit. The resulting source at `da387ad` passed
formation export verification, all six bundled examples and diagnostic
conformance vectors without regenerating artifacts.

A fresh candidate carrying `da387ad` replayed the shared real-daemon scenario
through all four adapters. Each passed waiting work, one block and an ignored
block, cancellation at stop and once after a tool, compaction recovery, and
own-terminal acknowledgment without false claim loss. All setup removals
restored original MCP/hook bytes or the Pi extension's original absence, and
all temporary profiles were removed. The report is
`output/hooks-rebased-qualification.json`. Pi used the actual installed extension
with a fake API; the other three used scripted native payloads and their
installed commands. This combined run did not repeat model calls. Earlier
native/model results and their candidate labels remain recorded above.

The final rebased formatting and strict workspace clippy checks passed. The
full workspace suite passed with 1,251 tests passed and 14 ignored, including
the reconciliation case that had failed in the earlier H2 runs. The 35 Python
hook/profile tests, 12 Node fake API tests and documentation check passed.
