# Live shared workspace with Luna and Haiku

Status: passing live-model shared-workspace sequence on 2026-10-05, completed by
an explicit continuation of the same native coordinator session. The first
harness invocation stopped before local update because an evidence comparison
rejected a redacted byte-count field. Its failed report is retained separately;
the successful continuation does not relabel it as a complete passing invocation.

This experiment used one physical Mac, one production SQLite daemon and exactly
two goal principals. Claude Code with `claude-haiku-4-5-20251001` performed the
worker turn; Codex with `gpt-6-luna` performed the coordinator turn. Both clients
made genuine provider requests, used native tools and the registered Locust MCP
bridge, and operated in separate temporary profiles. The coordinator reused the
fixture's enrolled principal; enrolling the worker did not add a third goal
member. This is live-model evidence for a synthetic five-test arithmetic fixture,
not two-machine or remote-peer qualification.

The person authorized provider execution and public farm publication. The
[listed farm](https://locust.farm/farm/b15c8c7a9c2c9ad7f6b511a826d4ba08)
shows the consented public projection. Its labels describe the actual local
topology. The public farm is separate from the private native transcripts and
filesystem evidence retained for this experiment.

## Method and observed work

The [live driver](../scripts/check_shared_workspace_models.py) prepares an
explicitly selected three-file seed: buggy `calculator.py`, unchanged
`test_calculator.py` and managed `notes.md`. The seed receives the coordinator's
exact review and integration before either model starts. Its five tests exercise
positive, negative, mixed, zero and floating-point addition. Four baseline tests
fail; the zero case passes. The coordinator's ordinary checkout additionally
contains unpublished notes and an unrelated file.

The harness authors seed setup, task assignment and local permissions. The
models author the substantive worker claim, progress, publication, review and
acceptance actions. The worker has contribution permission; it does not have
review or selection permission. Neither task text nor the worker's report can
authorize integration.

In the first invocation:

1. Haiku inspected shared task/status state through MCP, started the exact
   offered attempt and reported progress. It created and bound an ordinary
   checkout, read the fixture and repaired only `calculator.py`.
2. Haiku froze the requested file, published the frozen workspace proposal and
   published a separate signed task report citing that exact proposal. The
   independently materialized candidate changes `return a - b` to
   `return a + b`; tests and managed notes remain unchanged. The harness ran all
   five supplied tests against its own candidate copy successfully.
3. Luna independently materialized the exact candidate, read its diff and files,
   and ran all five tests successfully. It signed separate approvals for the
   workspace proposal and task report, integrated the proposal against the
   expected seed revision, and selected the task report.
4. Before any local update, the accepted shared head names the new proposal while
   the coordinator checkout remains pinned to the seed. Its buggy calculator,
   unpublished notes and unrelated file are preserved. This checks that shared
   acceptance does not automatically replace local files.
5. The harness stopped before the explicit update turn. Its exact-review
   comparison saw `"<redacted>"` in the independently retained `before.bytes`
   and `after.bytes` fields, whereas the native review contained their actual
   numeric byte counts. All other recorded assertions were true. This was an
   evidence-redaction mismatch, not a failing candidate test or refused runtime
   integration.

After correcting the comparison, re-evaluating the retained native evidence
confirmed the exact worker publication, complete coordinator review, coordinator
test command and integration receipt. The corrected command parser requires an
actual scoped executable/subcommand and Python test invocation in the verified
directory; echoed JSON or test prose does not qualify. Re-evaluation supplies
the original wrapper paths as evidence metadata and does not rerun either model
turn or change the original failed report.

The [farm helper](../scripts/client_qualification/live_farm.py) publishes only the
consented projection and waits for public ended-state evidence before normal
successful shutdown. Provider profiles are deliberately permissive, with no OS
filesystem isolation. Prompts prohibit credential inspection and access to other
profiles, but those instructions do not establish confinement. Provider keys are
passed through isolated environments; retained process stdout/stderr redact the
named provider key. This redaction does not establish that arbitrary model-visible
tool output or client internal state cannot expose secrets.

## Receipts and retained identity

The initial report is
`output/live-workspace-models-2026-10-05-a/report.json`, started at
17:07:07 UTC and finished at 17:10:59 UTC on 2026-10-05. Its complete file SHA-256
is `0999b35a6153060fdf62669a7874cf7d2d78a1516c1f8c3250fc7be6fd761235`.
It records `passed: false` and
`RuntimeError: Coordinator review evidence failed; local update withheld`.
Private runtime profiles were retained for the authorized continuation rather
than erased or replaced with fresh identities.

The following immutable IDs bind the observed candidate and shared acceptance;
they are public identifiers, not credential or session-secret contents:

| Item | Recorded identifier |
| --- | --- |
| Seed revision | `4f85520cffea04b410aea65368a9b5696af0c8ee184b80cc35d7a515f20825f3` |
| Worker workspace proposal | `ef6b53043fe5d75952cc3fbd003c992262933bc8370a8fc630ce57a4d9dbcb39` |
| Worker task report | `b377ced602b8151e16e421794781674098e1fa315c2191be565c8055a58b7973` |
| Integrated revision | `c5b5bee01b7d1676b1a334370f3c0b21988572cdda128c9bb84be90fd9be98f3` |
| Candidate manifest | `8c12aa14c7421d97adc70840ed93c6b64236c2306a0e01e59304a77c4ec40633` |

The proposal's parent is the seed revision. The signed workspace selection names
that proposal, includes its separate coordinator review as evidence, and names
the seed revision as its previous decision. The task report cites the same
proposal in advisory `sources` and carries the exact worker attempt. Before local
update, checkout status still reports the seed revision and seed manifest.

The initial report identifies API 6 / protocol 6, source commit
`6535b668b7fc68d7e87f2c749b9d97cf763f4a63` with a dirty working tree, and these
SHA-256 fingerprints. They identify the actual development artifacts and driver,
not every later source revision or a packaged release:

| Artifact | SHA-256 |
| --- | --- |
| Pinned Locust executable | `41ad1915b8439d5f2871f8bac20eed02b9442d7f54f69fa3ab977a7c984570ab` |
| Initial live driver | `ce9b5886821eafba55ae4b74c07ade5e367086ba28991117301fe8d42c9cfa93` |
| Codex executable | `b973d440acac501fd2594a43e7ca9ce41e0a65b9dfb28d0d7a7837c99e1261e3` |
| Claude Code executable | `387a5c5dcdbb815085edf0baf79591f9d8894efe922bceaf3d75b1b08055229d` |
| Client configuration probe | `fca17b8e0cc73e2557c90e0dc195db2667b6715bcad2027cbe74e98278d22680` |
| Worker MCP receipts | `fc59a75c4d785bf84b805f38869535eb5b72c0fd52b6727ff1244e7326ecef0f` |
| Coordinator review MCP receipts | `75acb24340b7395fb5f659e72d7deda361d88b67758681af2b366c4a76bbdd1e` |
| Corrected continuation live driver | `960e319944f9740d0256beaa3644205e64462d30edc9787aa28d1b2a9545a5bb` |
| Continuation farm helper | `9b5b5406f6f258438400ab8298ed976d2af29d7a6622e8f8b63ac475e3a012e0` |
| Continuation launcher | `bb6572cf50e169bb1db6349702fc3f13da91c7092b42e16753c9f2c2b4238ba4` |
| Coordinator update MCP receipts | `b0046373f11bbafc3bbb18761c7311eb5eb9cc9e675830730c9967344d65048c` |

Both first-phase client processes exited with code zero and recorded natural,
verified cleanup without forced cleanup. Their observed model identifiers match
the requested Haiku and Luna identifiers. The coordinator's native session ID is
`01a10d0b-29ad-7400-918e-3a599b2c686d`; the continuation must retain this identity
to qualify the explicit same-session update.

## Passing continuation and public receipt

The authorized continuation retained its own report at
`output/live-workspace-models-2026-10-05-a-continuation/report.json`, with
`passed: true` and all 14 assertions true. It re-evaluated retained exact review
evidence under the corrected predicates, restarted the same private daemon and
native profile, and performed only the remaining coordinator update turn. The
worker and completed coordinator-review turns were not repeated. The executable
and API/protocol identities remained those listed above; the corrected driver,
farm helper and continuation launcher have distinct source hashes.

The completed continuation report SHA-256 is
`71655029dade82823aadf2e6911c7abf600e5244ef7efd39f533753aec911277`.
Its top-level finish time is 17:14:08.408 UTC. The initial invocation's earlier
finish time is retained separately as `previous_attempt_finished`; correcting
this copied metadata did not change the native receipts or repeat provider work.

The continuation timeline records the update turn from 17:13:44.975 UTC to
17:14:07.799 UTC. Codex resumed native session
`01a10d0b-29ad-7400-918e-3a599b2c686d`, reconciled shared goal status through MCP,
and explicitly updated the existing checkout to the accepted revision. Its own
test command and the harness's final test command passed all five supplied tests.
The calculator equals the exact independently reviewed candidate; the original
test file, dirty managed notes and unrelated file survive byte for byte.

Final checkout status reports the accepted revision and candidate manifest, no
active operation, `notes.md` still dirty, and `unrelated.txt` still untracked. The
wrapper is also an unrelated untracked file. Update operation
`b60f6a8e793190bc8a0f2ce3d06bebec` has a durable `completed` receipt with
`target_in_lineage_at_completion: true`. Its recovery plan digest is
`676c28c8d9a4467e845de00e4490c49922049014433abee9288e5153d2d8eee3`.
The separate generic task is completed and selects the exact worker task report.

All three observed native-client processes exited naturally with code zero and
verified cleanup, without timeout or forced cleanup. Private qualification
profiles were deliberately retained for the owner after continuation. Client
process cleanup therefore does not imply deletion of all fixture/runtime files.

After closing the goal, the farm helper retained local receipt sequence 15 and
stream version 15 with request digest
`6cd6f644b52c0dda5fa31de59a94571e1dbaf7a0fc681ee2a54c1be91c923f67`.
The service acknowledged it at 17:14:08.038 UTC. A separate public API readback in
the report confirms the same farm ID, `status: available`, `visibility: listed`,
stream version 15 and `goal_state: ended`. It shows two agents in one local-Mac
group with the consented Codex/Claude Code harness labels, plus the completed
selected task report. This verifies the final public projection beyond a local
preview or receipt for an earlier still-open upload. The projection does not
itself establish the private checkout's bytes; those are checked separately above.

An independent final public API request, gallery request and SSE snapshot were
also retained in the continuation directory as `public-final.json`,
`gallery-final.json` and `public-final.sse`. They show stream version 15, an ended
goal, two correctly labeled harnesses, one completed task and 27 public changes.
A separate browser observation showed `GOAL ENDED` and `Completed 1 of 1` on the
listed farm. These observations establish publication of this projection; they
do not establish remote-peer transport or publish the private native transcripts.

| Independent public artifact | SHA-256 |
| --- | --- |
| Final API readback | `b0e61dca0e80833f6bc8c7e34b9ed822a10e06271081791c9d72a51baae51be6` |
| Final SSE snapshot | `30175f8287393aee93c76122a1258ef2d6a5efa1114d23e645cb3403babbc0f1` |
| Final gallery readback | `634511c55144b47e259bbb846eb65ef5779622abdd4513223b8f5b2bc28d5503` |

The final Python helper suite reported 288 tests with three skips. The
targeted [native-evidence cases](../scripts/tests/test_shared_workspace_models.py)
cover actual command parsing, echoed-envelope and
test-summary refusals, exact immutable review identity, the earlier byte-count
redaction mismatch and ordinary checkout preservation. The
[farm helper cases](../scripts/tests/test_live_farm.py) cover public ended-state
acknowledgement. Source tests and the live sequence are distinct evidence.

## Usage, cost and limits

Haiku's worker turn lasted 70.474 seconds in the client process observation. Its
client reported 6,892 output tokens and USD 0.1336538 for that invocation. This is
client-reported cost; no provider invoice or billing reconciliation was collected.
The client separately reported input/cache counters, including 177 input tokens,
33,848 cache-creation input tokens and 567,068 cache-read input tokens. Those
counters are retained in the report with their native meanings.

Luna's review turn lasted 61.791 seconds. Codex reported cumulative native-thread
counters of 256,661 input tokens, including 228,106 cached input tokens, and
4,254 output tokens. OpenAI cost is unknown. No price estimate is substituted
for missing billing evidence. Luna's resumed update took 22.781 seconds and
reported phase deltas of 194,594 input tokens, including 187,126 cached input
tokens, and 1,272 output tokens. The final cumulative native-thread counters are
451,255 input tokens, including 415,232 cached input tokens, and 5,526 output
tokens. These cumulative values are not added again to the review-phase counters.
OpenAI cost remains unknown for both turns.

The harness imposes no model execution deadline, token ceiling or tool-call
ceiling. It uses an explicit 30-second timeout for RPC/startup/cleanup operations;
that timeout does not bound the model's work. Timings and costs are observations
for this one fixture, with no comparison against a single-agent baseline and no
performance or collective-intelligence claim.

This run does not establish OS sandbox enforcement, two-host/WAN convergence,
unattended scheduling, default interactive permissions, cross-architecture
support, power-loss recovery or packaged/public-release readiness. It does not
exercise interrupted checkout mutation. Ordinary directories were used without
creating Git repositories; this live run did not remove Git from the process PATH.
The separate [model-free daemon/transport evidence](shared-file-tree-local-loop-2026-10-05.md)
and [workspace implementation contract](../docs/workspace.md) cover different
proof boundaries. The earlier model-free note's statement that it did not execute
providers remains true for those identified campaigns.
