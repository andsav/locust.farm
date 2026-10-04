# Real-model shared-context pilot

On October 4, 2026, Merak and Codex completed a useful collaboration through the
API-3 Locust daemon, using `gpt-6-luna` and separate principals, execution sessions
and workspaces. The resulting patch passed nine independently authored cases.
**Direct contribution citation remains incomplete:** Codex cited the finding in
its durable task description, but omitted the event identifier from the published
contribution summary, where the skill asks for it.
This was one real-model attempt; it was not repeated until green.

The [machine-readable record](evidence/shared-context-models-2026-10-04.json)
retains candidate identity, assertions, event/patch identifiers, usage and
recoverable failures. The [runner](../scripts/check_shared_context_models.py),
[scenario and oracle](../scripts/client_qualification/collaboration_case.py), and
[evidence predicates](../scripts/client_qualification/collaboration_evidence.py)
make the experiment repeatable.

## Experiment

A harness performed the local human setup: enrolled two principals, inspected
and accepted signed invitations, granted contribution/review/execution, created
separate protected session files, and exported a committed synthetic workspace.
The daemon used its real persistent SQLite store and Unix transport, with peer
networking restricted to loopback. Both actual clients used the same authorized
OpenAI API account. No scripted provider answered their turns.

Merak received a private operational note: archive members produced on Windows
can contain backslash separators, even though extraction happens on Linux.
Codex received a generic archive-path helper brief and a vulnerable starter
based on `PurePosixPath`. The private note, examples and solution were absent
from Codex's prompt and initial workspace. Both agents were directed to use the
current Locust skill and given a scoped CLI wrapper; Codex also had real MCP
registration. Their prompts supplied goals and boundaries, not numbered tool
recipes or a preselected finding event.

Merak completed, then the harness started Codex with the finding already
available. This tests sequential handoff; concurrent collaboration and automatic
wake were not exercised. Filesystem separation was by instruction and distinct
workspaces, under permissive native-tool policies on one OS account. It was not
an OS sandbox or proof that a determined client could not access another profile.

## Observed results

- Merak read the full skill and private note, read and acknowledged shared
  context, and published an attributed, goal-wide finding. Its durable transcript
  records the actual tool calls and results, not just its final narrative.
- Codex read the complete finding through MCP and acknowledged the exact signed
  receipt under its own principal and session. It opened a task, started an
  authorized attempt, edited `safe_member.py`, created/reviewed/submitted a patch,
  reported execution and declared completion through the installed interfaces.
- Independent checks rejected the starter and passed the completed artifact on
  nine cases: normal POSIX and Windows names, Windows and POSIX traversal,
  drive-rooted and network-rooted paths, absolute paths, empty names and NULs.
  Codex additionally reported twelve own checks; those are separate evidence.
- An independently fetched authenticated patch review named the exported base
  and only `safe_member.py`. Reconstructing its diff from the original file
  matched the independently tested artifact byte for byte. Git HEAD, README and
  an unrelated local file were preserved. No patch application to another
  workspace or shared selection was part of this experiment.
- The builder session ended with zero unread context items. A fresh session for
  the same principal still saw eleven unread items, including the finding.
  Context and acknowledgments survived daemon restart. Both clients and daemon
  exited cleanly, and temporary runtime profiles were removed.

Merak took 62.357 seconds and Codex 150.273 seconds. Merak reported 286,560 input
and 3,832 output tokens, with 258,056 input tokens cached and a reported cost of
$0.008065. Codex reported 1,025,357 input and 12,931 output tokens, with 960,758
input tokens cached; its stream did not report cost. These are client/provider
usage fields, not an independently reconciled bill or a performance benchmark.

## Friction retained

1. **The contribution did not cite the finding.** Its persisted summary was
   “Implement portable safe archive member path normalization.” The exact finding
   was delivered and acknowledged, the implementation handled its cases,
   and the task description durably cites the exact finding ID. The published
   contribution links to that task, so attribution is recoverable through it.
   The operating skill alone did not ensure a direct contribution citation.
2. **Copying a receipt caused a recoverable error.** Codex changed several hex
   characters in one event-version hash while copying a later signed receipt.
   Locust correctly rejected it with `denied: context receipt was not issued by
   this daemon`. Codex reread and successfully acknowledged the unchanged receipt
   without intervention. The long structured receipt is a demonstrated source
   of model transcription friction; the signature check behaved correctly.
3. **Merak tried unavailable MCP discovery.** It recovered through its configured
   CLI wrapper, but this was unnecessary work. It also read its private note
   before its first shared-context check despite the skill's checkpoint guidance.

The first draft of the harness incorrectly counted only MCP
`contribution.publish` calls. Codex used the native `patch submit` CLI, so that
predicate falsely marked publication absent. Evaluation was corrected to inspect
the durable signed contribution and authenticated patch. The original result is
retained locally; the missing-citation failure remains in the corrected result.
No second provider run was used to repair the outcome.

## Scope and reproduction

This adds real-model evidence for shared findings, current skill use, CLI/MCP
interoperation, session acknowledgments and task-backed publication. The checkout
was `510fcdb`; the tested debug binary reports `3ca78265119c-dirty`, API 3,
protocol 3, and is pinned by SHA-256 in the evidence. It was built during the
preceding implementation, so this is not a clean committed-build qualification.
It does not qualify physical machines, independent accounts,
peer synchronization, installed packages, default interactive approvals, all
client combinations, or the release ledger's complete real-collaboration gate.
It also does not show that collaboration improved success or cost over a single
agent: no paired no-finding model control was run.

Build the API-3 Locust binary and its configuration probe, then run with an
explicit Merak binary and an empty evidence directory. Load the authorized
`OPENAI_API_KEY` into the calling environment without logging it:

```sh
cargo build --locked -p locust-adapter --example config_probe
cargo build --locked -p locust --bin locust
python3.12 scripts/check_shared_context_models.py \
  --merak /absolute/path/to/merak \
  --output output/shared-context-models-new-run
```

The Python suite passed all 204 tests, including nine new false-pass checks for
private-input separation, receipts, attribution and patch reconstruction.
Documentation checks passed. No Rust runtime source changed.

The runner imposes no agent token, tool-call or working-time ceiling. Its exposed
`--rpc-timeout` controls individual setup calls and cleanup. It returns nonzero
when any acceptance predicate fails and retains the report, including recoverable
errors. It never modifies personal client profiles or publishes a release.
