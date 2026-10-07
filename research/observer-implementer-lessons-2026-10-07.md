# Observer and implementer: useful lessons from Merak4 and Merak8

Status: source and saved-run assessment, 2026-10-07. This records engineering
lessons and experimental-design proposals. It does not establish that an
observer improves a model's solution quality. No new API calls were made.

## What was inspected

This extends the [tag-team comparison](merak4-tag-team-comparison-2026-10-07.md)
with a distinct arrangement: one model implements while other models observe
and advise it. The [evidence record](evidence/observer-implementer-2026-10-07.json)
contains selected metadata from the retained Merak8 campaign, original file
hashes and runtime status/step counts. It excludes prompts, reasoning and tool
payloads. The [audit script](experiments/observer_implementer_audit.py) reproduces
that record from the local sibling checkout.

Merak8 was inspected at HEAD `52cbf37c` with unrelated working-tree changes.
Source paths below are relative to that repository unless marked Merak4. The
record hashes the inspected files; it does not assign that HEAD or today's
blueprint configuration to a historical run.

Both the native built-in
`polaris/src-tauri/builtin_blueprints/swe-pipeline-gold-v2.json` and the archived
`merak/fixtures/legacy/builtin_blueprints/swe-pipeline-gold-v2.json` exist. The
earlier comparison located the archived copy. They are different snapshots:
the archived observer uses Opus 4.7, the inspected native built-in uses Opus 4.8.
Both configure a GPT-5.5 executor, an observer reading `execution_log`, and a
Gemini strategic observer named `superego` reading `execution_log_thinking`.
Both advisers publish `directives`, which the executor subscribes to. Their
termination feed is `execution_done`.

The ordinary observer can inspect files, images and code and search the web.
The strategic observer has no tools. Their prompts request brief interventions
for concrete problems and silence when the work is on track. This makes advice
possible during implementation, before the executor changes models or submits
a completed solution. Model rotation and completion verification are separate
parts of the same workflow and must not be credited to the observer by default.

Merak4 also implements a collaborative mode (`observer/prompt.go:130`,
`observer/decision.go:58`): a peer can speak, insist or redirect, with its message
presented as a suggestion. Its progress-monitoring modes are more restrictive
about technical advice. "Observer" therefore names several different treatments;
one cannot infer the actual prompt, tools or authority from the role name.

## What the saved Merak8 runs show

The local campaign ledger has 28 records, including four `swe-polaris-default`
OpenSSL task attempts and two `smoke-normal` OpenSSL attempts. The job archive
also contains two default-pipeline GPT-2 code-golf results absent from that
ledger subset. The audit retains all six default-pipeline per-trial results,
the two available runtime traces, and the six relevant ledger rows. It does
not confuse job-level aggregates with per-trial outcomes.

| Default-pipeline attempt, May 30 | Task | External reward | Exception | Ledger duration |
| --- | --- | --- | --- | --- |
| 13:37:53 | OpenSSL certificate | 0 | None recorded in result; executor failure in trace | 647 s |
| 14:13:14 | OpenSSL certificate | 0 | `AgentTimeoutError` | 917 s |
| 14:29:20 | OpenSSL certificate | 1 | `AgentTimeoutError` | 917 s |
| 14:51:28 | OpenSSL certificate | 1 | None | 86 s |
| 15:26:24 | GPT-2 code golf | 0 | None | Not in selected ledger rows |
| 15:40:29 | GPT-2 code golf | 0 | None | Not in selected ledger rows |

These are debugging attempts on two tasks, not independent replications of a
frozen observer experiment. Every default-pipeline result has a null cost field.
The `smoke-normal` label is not an established pure-solo control, and the records
do not pin all historical prompts, models and harness versions. No causal
comparison or matched-cost improvement estimate follows from these rows.

### A failed executor left observers running

In the 13:37:53 OpenSSL trace, the executor fails at sequence 29 with:

> node "executor" requested suspension, which is not supported by the in-memory engine

The two adviser nodes reach terminal events about **594.8 seconds later**. Each
node is marked completed at the graph level, but its outputs say `timed_out`,
`task_completed=false`, and two steps. The campaign records 647 seconds overall
and the external reward is zero.

This is direct evidence of lifecycle friction: advisers outlived a failed
executor by almost ten minutes. It is not evidence that their advice made the
executor fail, nor a measurement of API spend during that interval.

### A passing run does not establish that advice helped

In the 14:51:28 OpenSSL trace, both adviser nodes report `steps=0`,
`tool_calls_count=0` and `stopped_by_feed=true`. The executor reports five steps,
`task_completed=true` and `auto_completed_from_verification=true`; the external
verifier gives reward 1.

This establishes successful execution and adviser shutdown. It records no
completed adviser steps, so it cannot demonstrate a helpful adviser
intervention. Zero completed steps also does not prove that no provider request
was ever started; the retained metadata is not a complete API billing log.

### Internal status, runtime success and external correctness differ

The 14:29:20 attempt has external reward 1 and an `AgentTimeoutError`; its ledger
labels it passed. A usable artifact can exist even when the orchestration does
not terminate successfully. Conversely, the earlier advisers have graph-level
completion events while their own outputs say timed out. A single "passed" or
"completed" label hides these different outcomes.

The available automated checks have their own limits. The parity fixtures
explicitly contain `stub executor`. The real-provider SWE Gold smoke test's
`patch_for_smoke` function changes both `observer` and `superego` into scripts
that run `true` (`merak/crates/merak-engine/tests/swe_gold_real.rs:560`). That test
can exercise other parts of the pipeline, but it does not exercise real adviser
behavior. This inspection did not rerun that test.

## Lessons worth carrying into Locust

1. **Study advice during work as its own mechanism.** An observer can catch a
   wrong assumption before a complete independent solution exists. This is a
   plausible lower-duplication use of another model, distinct from tag-team
   takeover, portfolio search and final negotiation. Its quality and cost
   advantage still need measurement.
2. **Make advice evidence-bearing.** Preserve the useful separation between an
   implementer that changes artifacts and an observer that can independently
   inspect requirements, files and tests. Ask for the specific violated claim,
   evidence or counterexample, and suggested check. Commentary on another
   model's reasoning alone is a weaker basis for correction and can inherit its
   framing. The executor should be able to reject advice with evidence.
3. **Keep observation separate from authority and verification.** A peer's
   recommendation must not substitute for permission to act or for an external
   correctness check. In the experiment, adviser messages are proposals; the
   grader remains independent. Separate observer advice from model rotation,
   completion gates and automatic verification in both controls and reporting.
4. **Verify useful participation, not just node presence.** Record the evidence
   an observer received, whether it completed a response, the advice delivered,
   when the executor could use it, and the resulting action or rejection. A
   started/completed observer node is not evidence of a delivered treatment.
   Test real advice delivery and shutdown; a no-op observer smoke is insufficient.
5. **Stop dependent work on every terminal path.** The saved failure supports
   checking success, failure and cancellation shutdown, not only the happy
   path. Preserve the final record, then stop advisers that no longer have an
   active implementer to help. Count all tail latency and actual charges.
6. **Intervene selectively.** The prompts' default-silence policy is worth
   testing. Trigger review from new evidence or a concrete anomaly, and measure
   whether it arrives before the relevant decision. Do not extend an already
   solved task just to make an observer speak. Silence can be correct behavior;
   it is not evidence of a quality gain.
7. **Separate outcome dimensions.** Report external quality, agent completion,
   orchestration errors, adviser delivery, latency and total cost individually.
   The retained reward-1 timeout demonstrates why this matters. Track harmful
   advice and right-to-wrong revisions as well as helpful corrections.

## A sharper next comparison

Add a continuously advised implementer to the
[proposed follow-up controls](merak4-tag-team-comparison-2026-10-07.md). Use the
same task information, tool access and total allowance for all conditions.
Compare a strong solo model's self-review with same-family and different-family
observers, holding model rotation and the completion checker constant. Use a
separate calibration set so scored tasks leave room for a useful intervention.

For a more direct test of advice, freeze selected failing or uncertain workspace
states before observer feedback. Replay matched continuations with no message,
self-review, or a delivered peer critique, including the critique's cost. Repeat
across tasks and model samples. This tests local correction from a given state;
it does not replace an end-to-end trial of the whole collaboration policy.

The desired evidence is a traceable sequence: a peer identifies a checkable
problem, the executor changes a decision, an independent test improves, and the
gain survives comparison with equally resourced controls. Different model names,
more messages or agreement alone are not that evidence.

## Reproduction

With the saved Merak8 checkout and ignored job records available:

```sh
python3 research/experiments/observer_implementer_audit.py \
  ../merak8 output/observer-implementer-audit.json
cmp research/evidence/observer-implementer-2026-10-07.json \
  output/observer-implementer-audit.json
```

The source files and saved runs were inspected read-only. This assessment does
not qualify a current Merak8 build or imply that all historical runs used the
same implementation.
