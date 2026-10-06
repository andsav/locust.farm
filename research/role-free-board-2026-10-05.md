# Real agents on a board without work roles — October 5, 2026

Status: observed on today's code, not an accepted design decision. No product
code changed. Three real clients chose tasks and completed an eight-task board
under each built-in formation, without a human assigning work after launch.
**Unrequested review remains unproven:** `open` refused optional reviews;
`peer-review` automatically requested them from the other members.

This tests part of the proposed [intended behavior](../docs/roles-and-permissions-plan.md),
starting from the [earlier live demo](../docs/live-farm-demo.md) and its
[controller](../scripts/live_farm_demo.py). Unlike that demo, this experiment
started no attempts for the agents and supplied no task assignments or offers.

## Method and identity

The two trials used fresh local state, fresh native sessions and three distinct
principals on one production daemon per goal. Clients were Codex CLI 0.153.4
(`gpt-6.1-sol`), Claude Code 2.1.280 (`claude-opus-5-5`) and Pi 1.0.1
(`gpt-6-luna`). These were genuine provider-backed client processes, not scripted
answers or this chat's subagents. All six native processes exited successfully
without hitting the 1,200-second deadline. The goals were not publicly published.

The pinned executable was built from `8a6b0dbd13c139f83c8299441b41d20bc0e8f94f`,
reported `locust 0.1.0 (8a6b0dbd13c1-dirty) api 6 protocol 6`, and had SHA-256
`3d758d9f5e83128fddaa53262e44d0862c259e9838782232e8351f18f6f2a7b0`.
The initial working tree had unrelated edits; source status and diff are retained
privately and fingerprinted in the [manifest](evidence/role-free-board-2026-10-05/manifest.json).
Concurrent repository work later committed additional tests and research. Both
trials continued to use the same copied executable.

The eight task texts were identical between trials: ASCII slugging, interval
merging, chunking, ASCII word counts, duration parsing, stable deduplication,
rotation and nested-list flattening. Each required one Python implementation and
one unittest file. Every client received the same seed in its own throwaway Git
repository. Immutable JSON bundles traveled through Locust blobs and signed
task contributions. No shared-tree integration, commits or dependency ordering
were required. Independent directories prevented incidental shared-file edits
from acting as a coordination channel.

The built-in [open](evidence/role-free-board-2026-10-05/open/formation.json) and
[peer-review](evidence/role-free-board-2026-10-05/peer-review/formation.json)
definitions both had empty `roles`, `task_types` and `flow`, and allowed members
to start independent attempts. Completion was an author declaration in the first,
and one non-author approval in the second. No role bindings, work offers,
selection or closure were introduced. Codex's principal created the goal and
seeded the board during setup; its native client was not given a coordinator role.

All three received the same prompt apart from their workspace/CLI paths. It
explained the board, completion rules, available commands and deadline. It told
them to keep working or checking the board until the shared objective was met;
it assigned no task or particular review. This is **prompted participation with
self-selected work**, not discovery of a goal by otherwise idle agents. The
repository [Locust skill](../skills/locust/SKILL.md) was also supplied. Exact
submitted prompts are retained for [open Codex](evidence/role-free-board-2026-10-05/open/prompts/codex.txt),
[open Claude](evidence/role-free-board-2026-10-05/open/prompts/claude.txt),
[open Pi](evidence/role-free-board-2026-10-05/open/prompts/pi.txt),
[peer Codex](evidence/role-free-board-2026-10-05/peer-review/prompts/codex.txt),
[peer Claude](evidence/role-free-board-2026-10-05/peer-review/prompts/claude.txt) and
[peer Pi](evidence/role-free-board-2026-10-05/peer-review/prompts/pi.txt).
The [template](evidence/role-free-board-2026-10-05/prompt-template.txt) and
[runner](evidence/role-free-board-2026-10-05/runner.py) retain the exact task texts
and preparation procedure. No follow-up instructions were sent during either trial.

## Outcomes measured from signed events

Times below are seconds from the controller's launch timestamp, immediately
before starting the three clients; launch skew was under 0.2 seconds. The open
trial began at **20:22:09.705 PDT**, and peer-review at **20:25:10.188 PDT** on
October 5 (October 6 UTC). Event `at_ms` is a signed diagnostic wall clock, not
protocol authority or proof of CPU effort.

| Measure | `open` | `peer-review` |
| --- | ---: | ---: |
| Seeded tasks | 8 | 8 |
| Tasks with a result meeting the rule | 8 | 8 |
| Signed attempts / task results | 9 / 9 | 8 / 8 |
| Tasks with multiple attempts | 1 | 0 |
| Tasks never taken | 0 | 0 |
| Time by which every task had been taken | 93.433 s | 134.727 s |
| Time by which every task had a counted result | 107.379 s | 199.510 s |
| Controller observed all native processes finished | 176.507 s | 238.571 s |
| Recorded approvals / distinct results approved | 0 / 0 | 11 / 8 |
| Automatic review requests | 0 | 16 |
| Recorded reviews with no prior request to that reviewer | **0** | **0** |
| Attempts to record optional reviews, refused | 4 | 0 |
| Results still awaiting their required completion evidence at finish | 0 | 0 |
| Local permission/authorization refusals during the trial | 0 | 0 |
| Formation-rule refusals | 4 | 0 |
| Other Locust CLI errors | 4 | 2 |

Every attempt's `offer` was null. The counts exclude setup events, daemon-derived
review requests and a goal-wide open-trial finding from the task-result count.
Neither trial created further tasks. Full IDs, principal mappings, signed times,
reviews and error receipts are in the [open metrics](evidence/role-free-board-2026-10-05/open/metrics.json)
and [peer metrics](evidence/role-free-board-2026-10-05/peer-review/metrics.json).

### Who took what, and when

“Start” means the signed `attempt_started`, not an inferred start of model
reasoning. “Result” is publication of the signed contribution containing its
artifact reference. “Counts” is the first applicable declaration or approval for
that exact result. Times retain three decimals for traceability, not accuracy
beyond this one host's clock.

| Open task | Agent | Start | Result | Counts |
| --- | --- | ---: | ---: | ---: |
| slug | Pi | 15.199 | 40.834 | 45.446 |
| intervals | Claude | 17.434 | 34.547 | 40.016 |
| duration | Codex | 27.912 | 56.337 | 62.984 |
| flatten | Claude | 54.757 | 60.708 | 60.804 |
| rotate | Pi | 57.724 | 75.762 | 79.215 |
| counts | Codex | 78.469 | 87.800 | 87.955 |
| chunks | Claude | 79.405 | 86.382 | 86.477 |
| unique | Pi | 93.433 | 118.929 | 123.080 |
| unique, second attempt | Claude | 100.229 | 107.283 | 107.379 |

| Peer-review task | Agent | Start | Result | First approval | Wait |
| --- | --- | ---: | ---: | ---: | ---: |
| chunks | Pi | 16.067 | 43.605 | 98.389 | 54.784 |
| duration | Claude | 26.268 | 54.115 | 82.882 | 28.767 |
| slug | Codex | 28.240 | 60.743 | 102.311 | 41.568 |
| counts | Claude | 64.808 | 97.998 | 178.926 | 80.928 |
| flatten | Claude | 110.063 | 130.289 | 198.222 | 67.933 |
| rotate | Pi | 112.282 | 138.854 | 181.215 | 42.361 |
| intervals | Codex | 116.195 | 126.596 | 158.568 | 31.972 |
| unique | Codex | 134.727 | 164.469 | 199.510 | 35.041 |

### Duplicate work

The open trial's `unique` task received two independent implementations and two
test suites: **one extra result bundle / two extra files**, not merely two claims
on the same bytes. Pi's bundle contains 14 implementation lines and 33 test lines;
Claude's contains 39 and 62, including comments and blank lines. Both pass their
own tests and the independent fixture checks. Neither cites the other's result,
and Claude published before Pi did.

Pi's claim-to-result interval was 25.496 seconds; the later-starting Claude
attempt's was 7.054 seconds. Both claims were active for those 7.054 seconds up to
Claude's publication. The later starter delivered the first counted result,
11.701 seconds before Pi's declaration. These are elapsed signed-event intervals,
**not attributable compute time, tokens, dollars or a precise wasted-effort
estimate**. Model reasoning and code generation can precede the tool call that
records a claim. The extra implementation and tests are the directly observable
cost; net benefit or cost is not established.

A retained [CLI read receipt](evidence/role-free-board-2026-10-05/open/selected-cli-receipts.json)
shows Claude saw `unique` with no attempts at +86.518 seconds. Pi claimed it at
+93.433; Claude started at +100.229 without another board read in between. The
last task acquired attention after that snapshot. Reordering that earlier list
would not resolve this stale-read race. There were no duplicate attempts in the
peer-review trial, but three extra approvals: intervals, counts and flatten each
received two. Review work can overlap too.

### Reviews: no human dispatch, but automatic requests

All three peer-review members approved other members' results: Pi signed five
approvals, Codex three and Claude three. No human assigned those reviews and no
agent issued a targeted review command. However, **each of the eleven approvals
has a preceding signed `request_review` effect addressed to its reviewer for
that exact contribution**. The daemon produced two requests per result. The
[review template implementation](../crates/locust-core/src/goal/flow.rs) derives
these from completion eligibility even when the formation's explicit `flow` is
empty. Thus “nobody was asked” is false if daemon requests count, as they should
for this measurement. Codex also wrote “awaiting independent review” in three
attempt reports; those status messages are not evidence of spontaneous review.

The first-review wait across eight results was **28.767–80.928 seconds, median
41.965 seconds**. These waits run from signed contribution publication to signed
approval, not from request timestamps: the derived request headers use `at_ms: 0`.
There were no rejection reviews, so this trial does not show review finding and
repairing a defect.

In `open`, Claude attempted four approvals without requests, at about +157.1
seconds, after the board had already completed. All four returned
`conflict: reviewer is not eligible for this exact candidate`. Local review
permission was already true. The [review eligibility rule](../crates/locust-core/src/goal/rules.rs)
permits review evidence for a reviews completion rule, not an author declaration.
Claude then published a goal-wide finding about its checks. That signed finding
is retained, but **is not counted as four signed reviews**. There is no valid
open-trial review-wait statistic; the results did not require reviews.

### Stalls and friction

Neither board stalled through the deadline or left a task untouched. The longest
gap between substantive signed activity before all tasks counted was 15.199
seconds in open and 18.074 seconds in peer-review, excluding derived zero-clock
request events. The peer-review board waited on approvals after results existed;
its last result arrived at +164.469 and all tasks counted at +199.510. Reviewing
continued while others coded. The open review refusals happened after task
completion and did not block the board.

The **four formation permission refusals** above are separate from **zero missing
local grants**. Four other open CLI failures were one omitted boolean value for
`--unread-only` and three mistyped goal IDs. Peer-review had two context CLI syntax
errors: an omitted boolean and `--unread_only` instead of `--unread-only`. These
were not permissions. Refused operations cannot appear as signed successful
writes; their evidence is the independent CLI response log, retained separately
from event-derived outcomes. Native tool records show these clients used the
instrumented CLI, not unlogged Locust MCP operations.

Up front, each trial enrolled three principals with `manage_goals`, created three
execution sessions, admitted the other two members, and made three equal per-goal
grants: contribute/review/execute/flow true; administer/select/takeover false.
The runner also authorized all 24 agent/task pairs, copying the earlier demo's
explicit preparation. **Those 24 authorizations were redundant:** today's
[local start check](../crates/locust-core/src/node/views.rs) accepts either the
execute grant or task authorization. Count this as runner setup overhead, not
24 permission prompts inherently required by Locust. No grants were repaired
while agents worked. Native profiles allowed the necessary shell, file and
Locust tools; ordinary interactive default-permission friction was not tested.

Before the counted trials, one preparation failed because the runner expected
the wrong task-open response shape. A subsequent launch using installed Kimi
failed authentication: its credential store was empty. The controller stopped
Codex and Claude after 46.802 seconds, retaining their two signed attempts and
zero signed task results. The already-prepared peer goal was never launched.
Fresh goals then used the Pi installation from the earlier demo. This is one
provider-authentication failure, not a Locust permission refusal or a failed
three-agent coordination trial. [Setup failures](evidence/role-free-board-2026-10-05/setup-failures.json)
also record collection fixes and a non-fatal Codex native-rollout flush warning.
No failed launch was relabeled as a completed trial.

## What this implies for the three decisions

1. **Default formation.** Peer approval is viable for a shared goal with three
   active, preauthorized agents: the current automatic requests got every result
   approved without human dispatch. This supports trying `peer-review` as the
   shared-goal default, with an explicit solo/open choice. It does not establish
   that agents would review without those requests, that peer review improves
   quality, or that a newly created one-member goal should wait for a peer. If
   the proposed design removes review requests, this experiment does not qualify
   that replacement behavior. Open also completed, but its ban on optional
   review evidence is a concrete limitation to account for.
2. **Least-attended task first.** No starvation appeared: every task was taken.
   The only duplicate came from a stale observation of the last unclaimed task,
   so this is not evidence that changing sort order would have prevented it.
   Showing current attention and refreshing before a start are plausible next
   experiments; a least-attended ordering should remain an unproven heuristic,
   not a conclusion from these runs. Compare it on larger, uneven boards before
   making a performance claim. Approval backlogs also deserve separate attention.
3. **Exclusive claims.** There is now direct evidence of extra work: two complete
   implementations and test suites for one task. “No evidence duplication costs
   anything” is too strong. But one small duplicate, no abandoned tasks and an
   earlier finish from the later starter do not justify mandatory exclusivity.
   Keep it deferred on this evidence; measure costly tasks, failure/release
   behavior and repeated collisions before accepting its coordination cost.
   Exclusivity's effect on this run's speed is a counterfactual, not a measured
   comparison.

## Evidence, checks and limits

The committed [evidence index](evidence/role-free-board-2026-10-05/README.md) includes
114 canonical signed headers and signatures, their independently decoded headers,
exact prompts, event details, task snapshots, 17 submitted bundles, metrics,
selected refusal/read receipts, and runnable collection/measurement helpers.
All **114 signatures, event IDs and canonical encodings verified** with
`Event::decode`; decoded bodies, authors and times matched the API details.
Changed signature, header and identifier controls were rejected. API replay
positions and SQLite insertion positions can differ; metrics join by immutable
IDs, not by assuming those positions are interchangeable.

The private runtime is `~/.locust-demos/role-free-board-20261005/`, with `open/`
and `peer-review/` directories, plus `setup-failed-before-agents/`,
`aborted-kimi-open/` and `aborted-kimi-peer-review/`. It retains the copied binary,
SQLite databases, signing/content keys, profiles, sessions, throwaway repositories,
full native transcripts and CLI logs. Those credentials and full transcripts are
not committed. Daemons were stopped after collection; no goal-close event was
invented. The manifest fingerprints the private originals, and
[SHA256SUMS.json](evidence/role-free-board-2026-10-05/SHA256SUMS.json) fingerprints
the committed evidence files. Decoded prose accompanies signed payload references;
the private daemon state retains the keys needed to independently decrypt it.

Independent post-run checks materialized every exact result bundle: all **17**
passed their bundled unittest suites (**81 cases in open, 74 in peer-review**)
and the additional [fixture oracle](evidence/role-free-board-2026-10-05/oracle.py).
These are independent checks of the small task contracts, not proof of general
correctness. The repository's `cargo fmt --all --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo test --locked --workspace` passed with their existing ignored tests.
The research Python helpers compile, and `python3 scripts/check_docs.py` passed.
No site files were changed by this task.

This is one machine, one person's agents, one small human-seeded board per rule,
and one trial per condition. It does not test independent owners, distrust,
network delay or partitions, remote permissions, filesystem confinement,
problem decomposition, hard dependencies, shared-file integration, idle wakeup,
long-running recovery, task abandonment, adversarial or poor-quality reviews,
or one-member onboarding. All clients used permissive local tooling and provider
access already available to the owner. The trials had fresh sessions, but
formation order, generated task-ID sort order, stochastic model behavior and
shared-host load were not controlled; repository checks overlapped the latter
peer-review run. The timing difference is descriptive, not a causal estimate of
peer-review overhead or a benchmark. It establishes task choice without human
assignments under current rules; it leaves truly unrequested review unanswered.
