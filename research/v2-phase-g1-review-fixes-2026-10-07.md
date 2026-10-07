# Locust v2 phase G1 review fixes

Status: implementation record, checked by reading, 7 October 2026. All 41
findings are fixed on main. Findings 1 to 25 landed at `8319432`; 23 and 25
were finished in the commit that adds this record. Findings 26 to 41 were fixed
on the branch `fix-hooks` and landed on main by fast-forward at `e20453b`, so
the "Locust context was NOT injected" line on subagent tool calls (finding 26)
is fixed on main.

This follows the [review](v2-phase-g1-review-2026-10-07.md) of G1 as built,
S1a, A1, the E1/E2 models and the hooks branch. Five lanes did the fixes: guard
(1, 5, 6, 9, 11, 12), marks (2-4, 7, 8, 10, 13-15 and residual 6), models
(16-22), A1 text (24, 25) and hooks (26-41). Finding 23 was fixed before the
lanes started, in `0c62207`. A final reader then checked each finding against
the code, the tests and the commits. That reader re-ran nothing; the lanes ran
the checks listed under [Verification](#verification).

## Disposition

Review numbers identify the review's headings, not current lines. Hooks paths
are written `fix-hooks:<path>`; since `e20453b` they are the same paths on main.

| # | Finding | Disposition | Commit | Test or text | Note |
| --- | --- | --- | --- | --- | --- |
| 1 | A start on a new store clears the marks a later restore needs | Fixed | `8656e2e` | `a_start_on_a_new_store_keeps_the_marks_a_later_restore_needs`, `marks_a_replaced_store_left_hold_no_key_of_the_new_one` ([guard tests](../crates/locust-core/src/node/tests/guard.rs)); `a_start_on_an_empty_home_keeps_the_marks_a_later_restore_needs` ([durable tests](../crates/locust/src/daemon/durable_tests.rs)) | No `Clear` on a new store. `own_marks` in [guard.rs](../crates/locust-core/src/node/guard.rs) reads only marks of a local principal or of the hosted governance key, for settle, `ahead()` and the unheard test. The durable test is the review's move-aside, start-fresh, put-back drill. Plan table and build note 4 updated. |
| 2 | A creation time on one side only makes an untouched home look copied | Fixed | `e6c9052` | `a_creation_time_only_one_side_reports_leaves_the_inode_to_decide` ([store.rs](../crates/locust-proto/src/store.rs)), `marks_whose_header_lacks_the_creation_time_the_file_reports_are_kept` ([store tests](../crates/locust-store/src/tests.rs)), `a_creation_time_that_appears_or_disappears_leaves_an_ordinary_start` | `FileId::same` compares the creation time only when both sides have one. It is used for the marks header in [marks.rs](../crates/locust-store/src/marks.rs) and for the database file in `guard_start`. |
| 3 | A home ending in `..` or `/` puts the marks inside it | Fixed | `e6c9052`, `b8bcea2` | `the_marks_directory_is_beside_the_home_and_never_inside_it`, extended ([local.rs](../crates/locust-proto/src/local.rs)) | `marks_dir` returns a `Result`; `home_dir` refuses such a `LOCUST_HOME` with `LocalError::Unnamed`. `b8bcea2` adds the `.unwrap()` the guard lane's durable test needed. |
| 4 | An existing marks directory or file is never checked for owner-only modes | Fixed | `e6c9052` | `a_marks_directory_or_file_others_may_read_is_refused` ([store tests](../crates/locust-store/src/tests.rs)) | Opening refuses a directory or file with any group or other bit and names the `chmod`. A leftover temporary file is reset to 0600. |
| 5 | A restart while one goal is behind revokes invitations in goals made since | Fixed | `8656e2e` | `a_restart_while_behind_keeps_the_invitations_of_a_goal_made_since` | `ahead()` skips goals that hold a `RESTORED` record. The test checks the new goal's invitation is pending, the goal is not marked restored, and the ticket admits. |
| 6 | A halt-proof delivery counts as hearing a computer | Fixed | `b83c972` | `an_accepted_halt_proof_does_not_count_as_hearing` ([sync driver tests](../crates/locust-core/src/sync/tests/driver.rs)) | [responder.rs](../crates/locust-core/src/sync/responder.rs) sets `reconciling` on the remote's `Frontier`, and `received` when a halt proof lands a record it lacked. The driver calls `reconciled` only when both say so. |
| 7 | The residual-6 exclusion accepts a host restore taken before the admission existed | Fixed | `74e1d81` | `a_host_restore_from_before_an_admission_existed_leaves_the_claim_standing` ([kept runs](../crates/locust-core/src/node/sim/restore_runs.rs)) | Now `host.before.contains(admission) && !host.copy.contains(admission)` in [check.rs](../crates/locust-core/src/node/sim/check.rs). The sweep's counts did not change. |
| 8 | Any failing run with an unclaimed reuse counts as residual | Fixed | `74e1d81` | `a_failure_beside_an_unclaimed_reuse_is_residual_only_while_the_fork_explains_it` | Each `Violation` is marked as forked or not (`Forked` follows records standing on the reused ones, their effects and acknowledgements). [seed.rs](../crates/locust-core/src/node/sim/seed.rs) marks a run residual only if every violation is forked; a step failure or panic never is. |
| 9 | No test pins that an agent's mark is kept while the governance key is held | Fixed (test only) | `6239344` | `an_agents_mark_is_kept_while_the_goals_own_key_is_held` | The code was already right. The lane reports the test fails with `!governance_held` removed. |
| 10 | The build notes misdescribe the kept runs, the counts and residual 6 | Fixed | `74e1d81`, `46df19f` | [build notes](v2-phase-g1-build-notes-2026-10-07.md), "Tests" and "After the review" | Kept runs said to be separate deterministic tests; 25 guard and 9 store tests with the left-out three named; residual 6 said not to be accepted by the plan as written. See residual 6 below. |
| 11 | The "no mark, so no unheard bit" residual is still inside the simulator's claims | Fixed (claim narrowed, the review's second option) | `7ee4b52` | `a_member_whose_agent_had_no_record_in_the_copy_signs_again_after_a_second_restore` | `Restore` records the keys with a mark; `claimed()` leaves out a marks-kept restore on a member's computer that finds no mark for the agent. The [plan](../docs/host-safety-and-ending-plan.md) states the limit under the start table. The guard's gap itself remains, as a stated limit. |
| 12 | No test admits a joiner while only the host's agent is behind | Fixed (test only) | `6239344` | `only_the_goals_that_are_behind_are_held`, extended | Checks `admission_hold` is `None`, the agent is held, a joiner is admitted and can post, and only the agent stays in `guard`. |
| 13 | crates.md cites the wrong master-plan lines | Fixed | `4db8acc`, `1712724` | [crates.md](../docs/crates.md) line 50 | Now 413-424, where the Versions paragraph sits after the new limit bullet. It is the same on `fix-hooks`. |
| 14 | The master plan does not record S1a | Fixed | `4db8acc` | [master plan](../docs/master-plan.md) pieces table and S1 row | "A1, A3 and S1a … are built"; S1: "Removal built as S1a (`28c6425`); sentences after G1". |
| 15 | The cache test's "authorizes nothing" assertion cannot fail | Fixed | `0cf9b95` | `cache_does_not_authorize_a_missing_manifest_or_replaced_key` ([content graph tests](../crates/locust-core/src/node/content_graph_tests.rs)) | The sealed `later_data` is landed on peer 1 before the `NotFound` assertion, so only the missing manifest can refuse it. |
| 16 | `restore-leave` stops at `settle`, so `RemovalMatchesBefore` checks nothing | Fixed | `16922c5` | `restore-leave-witness` (`NeverTwoRemovals`) in [cases.json](tla/cases.json) | The trace is copy, leave, exchange, removal, store, start, exchange, continue, removal. `Leave` now gates the leaver's removal on an exchange since the last start. See [RestoreGuard.tla](tla/RestoreGuard.tla) and [restore-guard.md](tla/restore-guard.md). |
| 17 | The model's leave rule is stronger than Rust and E2's plan | Fixed | `52245eb` | `ScenarioOutcome` in leave-removal, leave-readmission, leave-host-agent | The leave clause is gone from `Eligible`; a leave is valid only if it names its author's admission and is not the host's agent's ([Organization.tla](tla/Organization.tla)). |
| 18 | `organization-leave-fork` models a governance fork | Fixed | `52245eb` | `ScenarioOutcome`, `organization-leave-fork-witness` | One leave, one removal with its cutoff at the leave, and record 16 forking identity 2 below the leave. With the removal held, 10 and 14 count and 16 does not. |
| 19 | `organization-leave-readmission` admits a new identity | Fixed | `52245eb` | `ScenarioOutcome`, `organization-leave-readmission-witness` | Identity 2 is admitted again and its record 17 under the new admission counts; 10 stays by the cutoff. |
| 20 | Most new organization cases assert nothing about their named outcome | Fixed | `52245eb` | `ScenarioOutcome` for all six E1 and four E2 cases; four leave witnesses | [organization.md](tla/organization.md) describes each outcome and counts seventeen witnesses. |
| 21 | No recorded run of the organization suite with the new cases | Fixed | `93e187e` | [evidence README](evidence/tla/organization/README.md), "G1 review fixes to the E1 and E2 models" | Organization 57/57, restore 33/33, fast 163/163. The runs were on `e995af5` before the rebase; the README says its Rust hashes are of `e350a85`. |
| 22 | `Restore` resets `didRemove`, lifting the one-removal bound | Fixed | `16922c5` | Model header, [restore-guard.md](tla/restore-guard.md), the plan's model paragraph | The reset is gone; a second removal needs `Continue`. `leaveP` is removed. `restore-guard.md` says its retained results predate the change. |
| 23 | G1 put back the old summaries in the runtime contract | Fixed | `0c62207`, this record's commit | [runtime.contract.json](../docs/reference/generated/runtime.contract.json); the G1 build notes' Verification line | Regenerated before the lanes started, and again in `ecc3926`. The build notes now say the G1 contract was regenerated from a tree without A1 and that `0c62207` repaired it. |
| 24 | The `contributions` summary tells agents to read others' results first | Fixed | `ecc3926` | `the_contributions_summary_keeps_attempts_independent` ([api.rs](../crates/locust-proto/src/api.rs)) | "on a task you are attempting, publish your result before reading other members' results on it". The contract was regenerated. |
| 25 | The skill says the list of tasks to start shows titles | Fixed | `7f91d35`, this record's commit | [SKILL.md](../skills/locust/SKILL.md) lines 139-147; A2's planned wording in the [store plan](../docs/agent-memory-and-store-plan.md) | The skill says `locust_pending` lists IDs only. A2's planned skill text now sends the agent to `locust_board` for the "After task:" title before it starts another task by name. |
| 26 | A subagent's tool calls get the failure line, in every chat | Fixed | `589b534` | `a_subagents_callbacks_are_silent_and_untouched_in_every_adapter` (`fix-hooks:crates/locust/tests/hooks.rs`), `subagent_callbacks_are_ignored_and_transcripts_titles_never_enter_core` | `parse_input` returns `Parsed::Ignored` for any adapter's `subagent_fields`; `hook::run` returns nothing for it before reading secrets or contacting the daemon. The test runs thirty subagent calls per harness and checks no output, no connection and no marks, also in an associated chat with a failing daemon. |
| 27 | A worker that cannot advance its attempt is blocked again at every stop | Fixed | `589b534` | `a_held_claim_blocks_once_and_acknowledgments_and_notes_do_not_block_it_again`, core `acknowledgments_and_notes_on_a_held_claim_do_not_block_its_turn_end_again` | `context.acknowledge` is neutral for the stop rule, and a write keeps held claims in `shown`. |
| 28 | Sibling chats of a session get "claim lost" when one finishes | Fixed | `589b534` | `sibling_chats_of_a_session_hear_no_loss_when_one_of_them_ends_the_attempt`, core `a_sibling_chats_own_release_is_not_reported_as_a_loss` | A session-wide record of released claims, at most 256. agents.md says a CLI write in a shell is still not seen. |
| 29 | A failing cancellation read turns off every hook for the chat | Fixed | `589b534` | `a_failing_cancellation_read_defers_only_its_goal_and_is_retried`, `cancellation_release_requires_effective_exact_cancellation_detail_and_drops_one_that_never_will` | `cancellation_target` answers `Attempt`, `Gone` or `Later`; no `?` aborts the callback. |
| 30 | A stale baseline revision fails the whole tool poll | Fixed | `589b534` | `a_baseline_ahead_of_a_restored_goal_is_read_afresh_without_failing` | An API refusal of `wait` falls back to `Pending` for that goal; a refused `Pending` skips the goal. |
| 31 | Start and tool inherit the stop deadline | Fixed | `589b534` | `start_and_tool_give_up_on_a_wedged_daemon_within_seconds` | 5 s for start, tool and the reads before a stop; native start and tool limit 30 s; only a parked stop waits, at most 270 s. |
| 32 | A worker chat in the person's own client waits at every turn end | Fixed differently | `589b534`, `e20453b` | `an_idle_worker_in_a_persons_chat_is_told_once_and_never_held`, `an_unattended_idle_worker_waits_on_each_member_goal_using_only_reads` | The interactive typed-prompt check was not run. Instead a stop parks only with `LOCUST_HOOKS=unattended` or in Pi's print and JSON modes; in a person's chat an idle worker gets one line naming `locust_wait`. The qualification note records what each harness's payload can tell. |
| 33 | Every tool callback rewrites and syncs its marks, and they grow | Fixed | `589b534` | `ordinary_tool_calls_do_not_rewrite_unchanged_marks`, core `invocation_history_is_bounded_and_still_ignores_recent_replays` | Saved only when changed; 256 invocation IDs; delivered notices pruned. |
| 34 | Missing secrets or a removed install make every chat report a failure | Fixed | `589b534` | `a_chat_that_never_used_locust_hears_nothing_whatever_fails`; Node test "invalid output and a missing launcher leave every native event unchanged" | A config or secret failure is silent. The Pi shim only relays and prints no line of its own. |
| 35 | Applying setup again never updates installed hooks or the Pi shim | Fixed | `a6b8166` | `applying_again_brings_installed_hooks_up_to_this_release`, `applying_again_replaces_an_older_pi_shim_it_owns`, adapter `reapply_updates_in_place_keeps_declined_events_out_and_adds_new_ones` | Status reports hooks from an older release as not ready. |
| 36 | A dotfile-managed hook settings file makes all of setup refuse | Fixed | `a6b8166` | `a_linked_hook_file_turns_hooks_off_and_setup_goes_on` | Hooks turn off for that client with one line in plan, apply and status; the MCP entry and skill are still set up. |
| 37 | The branch does not compile once put on main | Fixed | `cdefab6` | The filtered-goals hook test, extended to see G1's signer-recovery halt | The branch is rebased on `8319432`; the contract regenerated with no change. |
| 38 | Adapters are a dispatcher with per-harness branches | Fixed | `589b534`, `a6b8166` | Adapter goldens and the conformance scenario | Each adapter is a row of `ADAPTERS` (`fix-hooks:crates/locust-adapter/src/hooks.rs`); no `Client` branch remains outside tests; setup reads `trust_review` from the row. |
| 39 | The failure line is written out twice more, outside the core | Fixed | `589b534` | `a_locust_chat_hears_of_a_failure_once_per_episode_in_every_adapter`, `an_unknown_harness_name_from_another_build_stays_silent`, core `failure_is_said_once_per_episode_and_only_to_a_chat_that_used_locust` | The only non-test copy is `core::FAILURE_LINE`. An unparsable hook command line, such as an unknown harness, now prints nothing. |
| 40 | `compacted` and `stop_hook_active` are parsed and never used | Fixed | `589b534` | none needed | Both fields and their validation are deleted. |
| 41 | Phase state on the branch is stale against main | Fixed | `e20453b` | Master plan H and A4 rows on `fix-hooks` | H1a, H1b and H2 marked built with the branch's hashes; A4 and H3 wait only on G2 and E2. The hashes hold only if the branch lands by fast-forward. |

## Guard lane

Commits `8656e2e`, `6239344`, `b83c972` and `7ee4b52`. For finding 11 the lane
chose the review's second option, narrowing the simulator's claim, over a
goal-level slot in the marks file, because the marks lane was changing that
file at the time. The guard still cannot keep "copy of unknown age" for a goal
where this computer's keys have no record in the copy; the
[plan](../docs/host-safety-and-ending-plan.md) now states that as a limit of the
same kind as risk (5), and the kept run must end in the reused position.

The lane found one more gap it did not fix, and it was not recorded anywhere
else: a member restored with its marks lost, whose unheard hold then ends by
hearing the host, can be restored again with the marks kept, and its agent can
then sign at a position another member holds. The simulator still claims that
case, because the marks-lost exclusion ends at the machine's next restore; the
10,000-seed sweep did not hit it. It is an oracle gap of the same kind as
finding 11 and needs either the same narrowing with a kept run or a guard
change. This reader did not reproduce it.

## Marks lane

Commits `e6c9052`, `74e1d81`, `0cf9b95`, `4db8acc`, `46df19f`, `1712724` and
`b8bcea2`. `e6c9052` on its own does not compile, because the guard lane's
durable test, already on main, called `marks_dir` without `.unwrap()`;
`b8bcea2` repairs it.

**Residual 6.** The fork after two restores from before the same admission is
now recorded as an accepted limit: as the fourth way of risk (5) in the
[plan](../docs/host-safety-and-ending-plan.md), and in plain words in the
[master plan](../docs/master-plan.md) under "Assumed until the owner objects".
The owner has not answered it; it stands until the owner objects. The build
notes' "After the review" section says "the plan author accepted it", which
reads stronger than that. The simulator exclusion stays, narrowed as in 7.

The re-run sweep over seeds 0 to 10,000 kept its counts: no failure, 30 reused
positions outside the claims, the same 14 residual runs. A first version of the
rule for 8 called four of them failures, because delivery acknowledgements of
effects built on the fork were not yet counted as following from it.

The owner-only mode check in [marks.rs](../crates/locust-store/src/marks.rs)
repeats the rule and the message shape of `check_private` in
[home.rs](../crates/locust/src/daemon/home.rs), in another crate. It is small,
but it is a second copy of one rule.

## Models lane

Commits `52245eb`, `16922c5` and `93e187e`. No Rust changed. The lane checked
by hand, without keeping those runs, that putting back the old leave clause,
dropping `RetainedByCutoff`, putting `settle` back, or moving the first removal
before the exchange each makes the named case fail. The
[evidence README](evidence/tla/organization/README.md) records this.

## A1 text lane

Commits `ecc3926` and `7f91d35`. A2's wording in the store plan, which
repeated finding 25's assumption, was corrected in the commit that adds this
record.

## Hooks lane and landing

Commits on `fix-hooks`: the six original hooks commits rebased as `36f1120`,
`d664008`, `fa96a3e`, `c198ee3`, `59d380a` and `4882584`, then the fixes
`cdefab6` (37), `589b534` (26-34, 38-40), `a6b8166` (35, 36, 38) and `e20453b`
(32, 41). The branch sits directly on main's `8319432`, so it lands by
fast-forward: `git -C /Users/andrei/Projects26/locust merge --ff-only fix-hooks`.

The first fast-forward was refused because the branch adds an index line to
`research/README.md` and the main checkout held another session's uncommitted
edit of that file. That edit was stale: besides moving the R5 build-notes line,
it dropped the index entry for
[the local-profile sync note](lan-sync-host-offline-2026-10-07.md), committed in
`765cb0f`. The orchestrating session reset that file to its committed text,
landed `fix-hooks` by fast-forward at `e20453b` (main had not moved, so the
landed tree is the one the lane checked), and put the one real change back as
an uncommitted edit: the R5 build-notes line moved below the R4 fixes line. The
`locust-fix-hooks` worktree and `fix-hooks` branch are removed. The original
`hooks` branch and its worktree `/Users/andrei/Projects26/locust-hooks`
(`2ba4917`) are superseded by the rebased copies. A person who installed hooks
from a build of the old branch keeps getting the old behaviour until they
install a build from main and apply setup again (35).

The lane could not answer the plan's typed-prompt question for any harness: that
needs an interactive terminal run. Finding 32's fix makes the answer matter
only where `LOCUST_HOOKS=unattended` is set. Pi was run against a fake Pi only.
A terminal report made with the CLI in a shell is still not seen by the hooks;
[agents.md](../docs/guide/agents.md) says so.

## Checks on main

- Worktrees: the `fix-*` worktrees and branches are gone; the superseded
  `locust-hooks` remains.
- `git status` on main shows only the other session's `research/README.md`
  reorder, as before. The two older stashes on main are unchanged.
- No lane left a dead path found by this reading. `compacted` and
  `stop_hook_active` are gone, the `leaveP` alias is gone, and the failure line
  has one non-test copy. The one second copy found is the mode check noted
  under the marks lane.

## Verification

The lanes ran these. This reader re-ran none of them.

| Lane | Checks, on the landed tree |
| --- | --- |
| Guard | fmt, clippy `-D warnings`, `cargo test --locked --workspace` (1,213 passed) on `7ee4b52`; `check_docs.py`; a 10,000-seed release sweep. |
| Marks | fmt, clippy, workspace tests (1,219 passed) after rebasing; `check_docs.py`; `check_formations.py`; the 10,000-seed sweep. |
| Models | `check_tla.py` organization 57/57, restore 33/33, fast 163/163 (on `e995af5`, before the rebase); the changed cases again after it; `check_docs.py`; 25 `test_check_tla.py` tests. |
| A1 text | fmt, clippy, workspace tests; `check_formations.py --write` then the check; 301 script tests, 3 skipped; `check_docs.py`. |
| Hooks | On the branch: fmt, clippy, workspace tests (1,346 passed, 14 ignored); `check_formations.py`; 335 script tests, 3 skipped; `check_docs.py`; 13 Pi shim Node tests (one timing test failed once under load, then passed three times); `check_hooks.py` through all four adapters against a real daemon. Native Pi and real-model runs were not run. |

What the final reading did not re-run: no build, no Rust test, no simulator
sweep, no TLA run, no check script, no hook conformance run and no daemon. It
did not run any test with a fix removed. Where this record says a test fails
without its fix, that is the lane's report. It also did not check
`check_docs.py` on this record.

## Follow-up defects

Fixed on the branch `fix-g1-more` after this record, 7 October 2026. The
[G1 build notes](v2-phase-g1-build-notes-2026-10-07.md), "After the review",
give the detail.

| Defect | Disposition | Commit | Test |
| --- | --- | --- | --- |
| The second-restore gap under [Guard lane](#guard-lane): a member restored with its marks lost, caught up by the host's computer alone, then restored again with its marks kept, signs at a position another member holds, and the simulator claimed it | Reproduced. Not held by the guard: nothing on the computer remembers the record, which was lost with the marks. Holding it would mean waiting for every member's computer after every copy of unknown age. The plan states it under the first way of risk (5), and the simulator no longer claims it | `3ae5cae` | `a_member_restored_with_its_marks_lost_and_then_kept_signs_again_what_only_another_member_holds` and `a_record_that_came_back_after_the_marks_were_lost_leaves_the_claim_standing` ([kept runs](../crates/locust-core/src/node/sim/restore_runs.rs)); `a_record_lost_with_the_marks_is_signed_over_after_a_later_restore_with_the_marks_kept` ([guard tests](../crates/locust-core/src/node/tests/guard.rs)) |
| A new member's first post refused as `conflict` ([local-profile sync note](lan-sync-host-offline-2026-10-07.md), "Also found") | Fixed in the daemon. The admission hold ended on hearing the host's computer while the member could not yet read the goal's rules. It now also waits for the rules and the current content key. The restart was incidental: without the kill, 2 of 6 runs were refused too. The simulation now waits for the new member's `guard` to clear before acting, as the Rust tests already did | `a0dff5e` | `a_key_just_admitted_waits_for_the_goals_rules_as_well_as_the_hosts_computer`; `test_a_member_signs_only_once_the_restore_guard_holds_nothing` |
| The second copy of the owner-only mode check under [Marks lane](#marks-lane) | Fixed: one rule, `locust_proto::local::owner_only`, called by the marks file and the state directory | `79ce9cb` | `only_modes_without_group_or_other_bits_are_owner_only` ([local.rs](../crates/locust-proto/src/local.rs)); the existing marks and state-directory mode tests |
