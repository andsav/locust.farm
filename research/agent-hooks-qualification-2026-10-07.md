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
reads status, pending or wait. [Private marks](../crates/locust/src/hook/marks.rs)
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
