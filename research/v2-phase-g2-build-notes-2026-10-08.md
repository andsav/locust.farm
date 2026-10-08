# Locust v2 G2: catching up in the person's words

Status: implemented; the required checks pass and the drills run as automated
tests. This is build-order phase 9, phase G2 of the
[host safety and ending plan](../docs/host-safety-and-ending-plan.md) (section
"G2: Catching up in the person's words", with the terminal texts G-1 to G-6),
with its [companion](../docs/host-safety-and-ending-plan-details.md). It gives
the words to the guard [G1](v2-phase-g1-build-notes-2026-10-07.md) built,
after G1's [review](v2-phase-g1-review-2026-10-07.md) and
[review fixes](v2-phase-g1-review-fixes-2026-10-07.md). The plans were read,
not edited.

## What changed

- **The refusal's fifth side.** `Why::ThisComputer { hold: GuardView }`
  (`"side": "this_computer"`) in
  [level.rs](../crates/locust-proto/src/api/level.rs), worded by `render`:
  the reason ("the Locust data here is older than what this computer signed in
  the goal", "may be an old copy", or "admission has just arrived; Locust is
  checking with the host's computer"), the side "(this computer)", and a fix
  that says what the hold waits for. The person's voice prints the continue
  line from the new `continue_command(goal)`; the agent's names the owner and
  prints no command; an `admitted` hold "ends by itself" in both. A refusal of
  the goal's own key reads "You can't ..." to the person, "The host can't ..."
  to an agent, and names no key. `GuardView::waits_for_you()` in
  [api/guard.rs](../crates/locust-proto/src/api/guard.rs) is the one
  predicate for "only the person ends this hold".
- **The refusal's act.** `next_place(entry, author, body)` in
  [authoring.rs](../crates/locust-core/src/node/authoring.rs) takes the body
  it signs, at its three callers (`author`, `member_remove` in
  [goals.rs](../crates/locust-core/src/node/requests/goals.rs) and
  `cancel_acknowledge` in
  [claims.rs](../crates/locust-core/src/node/requests/claims.rs)). A held key
  is refused through `Node::refusal` with the act `access::subject(body)`
  names ([access.rs](../crates/locust-core/src/node/access.rs)), which now
  maps an admission to `Invite`, a removal to `RemoveMember`, a role record to
  `GiveRole` and a consent to `Publish`. `Node::hold_view` in
  [guard.rs](../crates/locust-core/src/node/guard.rs) gives an agent held
  through the goal's own key that key's view. The local `goal.join` handler
  ([invitations.rs](../crates/locust-core/src/node/requests/invitations.rs))
  builds the same refusal with `Act::Join`. G1's fixed hold sentences and
  `Hold::refusal` are gone.
- **What waits for the person.** `WaitingKind::CatchingUp { holds }` and
  `DaemonStatus.lost_goals` in [api.rs](../crates/locust-proto/src/api.rs).
  `Node::waiting_for` in [views.rs](../crates/locust-core/src/node/views.rs)
  lists, first, each goal this daemon hosts whose own key's hold only the
  person can end, with the continue line. `Guard.lost` counts, at a start
  that put the data back with the marks kept, the goals the marks name and the
  store lacks. `Node::stalled` in
  [levels.rs](../crates/locust-core/src/node/requests/levels.rs) reports a
  step whose runner is held as `Stall::CatchingUp`, after "is a member" and
  before "can sign next".
- **The views.** [presentation.rs](../crates/locust/src/cli/presentation.rs)
  prints under a goal in `status` and in `goal status` one block per goal that
  is catching up (mockups G-1, G-2, G-5), the restored line, `Just admitted:
  checking with the host's computer.` under such an agent, an agent's own
  conflict (G-6) and, after the goals, the line for goals a copy lost. The
  heading says "halted" only for an authority conflict. In `status` the CLI
  reads `goal.status` of each goal that is catching up, for the names of its
  computers and when each was last seen ([cli/mod.rs](../crates/locust/src/cli/mod.rs)).
- **The override.** `locust --owner goal continue (--goal G | --all)` in
  [only_you.rs](../crates/locust/src/cli/only_you.rs): the plan of mockup G-3
  for each goal that is catching up, one yes, `Continued "T". This computer
  signs here again.`, no undo line; with nothing held, `"T" is not catching
  up. Nothing changed.` A command that would sign with a held key shows no
  plan and prints the daemon's refusal (`refuse_if_held` and its two
  wrappers): `goal add`, `goal leave`, `member remove`, `rules bind`, `task
  revise`, `workspace init` ([workspace.rs](../crates/locust/src/cli/workspace.rs)),
  `farm on`, `farm off`, `farm consent` ([farm.rs](../crates/locust/src/cli/farm.rs)),
  and `role give` and `role take` ([roles.rs](../crates/locust/src/cli/roles.rs)),
  which print no undo line. The plan of `goal invite` warns "This computer is
  catching up; nobody is admitted until it has."
- **Doctor.** A `marks` check after `state_directory`
  ([doctor.rs](../crates/locust/src/cli/doctor.rs)): its detail is the marks
  directory's path; it fails, with a recovery sentence, for a file in its
  place, a directory others may read or one that cannot be written, or one
  whose parent cannot be written when it does not exist yet.
- **Words for agents and people.** The MCP instructions
  ([mcp.rs](../crates/locust/src/mcp.rs)), the
  [skill](../skills/locust/SKILL.md) and [sharing.md](../docs/guide/sharing.md)
  say what `read_only` means now and not to retry it in a loop.
  [concepts.md](../docs/guide/concepts.md) lists `goal continue` among the
  commands that ask for a yes. [operations.md](../docs/guide/operations.md)
  names the marks directory and rewrites Backups as the plan lists; "Restoring
  a copy is untested." is gone.
- The [runtime contract](../docs/reference/generated/runtime.contract.json)
  is regenerated: the refusal schema gains the fifth side, the CLI tree the
  new `goal continue`.

## Tests

- Proto: `this_computer_reads_the_same_facts_in_both_voices` in level.rs.
- Core, in [tests/guard.rs](../crates/locust-core/src/node/tests/guard.rs):
  `only_a_hold_no_other_computer_can_end_waits_for_the_person` (the four
  cases: the goal's own record unsent, a hosted copy of unknown age, a
  member's copy, a copy that lists nobody),
  `a_member_restored_from_an_older_copy_reads_that_it_catches_up_by_itself`,
  `a_copy_older_than_a_goal_counts_it_lost_until_the_next_ordinary_start`, and
  the G1 tests that read sentences now check the side and the words through a
  shared `this_computer` helper. `a_newly_admitted_key_waits_for_the_hosts_computer_once`
  checks the admitted refusal's words. `goal_status_reports_each_stalled_runner_condition`
  (Phase 3's `goal_status_reports_stalled_effects`) gains a step held by the
  guard.
- CLI views, in presentation.rs:
  `status_shows_who_a_goal_catching_up_waits_for_and_the_continue_line` (G-1,
  after the answer, and both goals of G-2),
  `a_hosted_goal_of_unknown_age_is_listed_under_waiting_for_you`,
  `a_goal_waiting_for_a_call_is_not_listed_under_waiting_for_you` (G-5),
  `the_hosts_hold_is_printed_once_per_goal_from_the_summaries`,
  `a_just_admitted_agent_gets_one_line_and_no_block`,
  `an_agent_with_conflicting_records_reads_its_own_conflict`,
  `status_says_when_this_copy_is_older_than_goals_it_took_part_in`;
  `every_printed_command_parses_as_printed` now parses the continue line in
  status, goal status and every this-computer refusal.
- CLI commands, in [tests/cli.rs](../crates/locust/tests/cli.rs) against the
  typed stub daemon: `goal_continue_shows_its_plan_and_needs_a_goal_or_all`,
  `the_invite_plan_warns_while_the_host_is_catching_up`,
  `a_signing_command_in_a_goal_that_is_catching_up_shows_no_plan` (`rules
  bind`, `role give`, `member remove`). The two doctor tests there name the new
  check. `doctor_names_the_marks_directory` in
  [cli/doctor/tests.rs](../crates/locust/src/cli/doctor/tests.rs).
- MCP: `a_goal_catching_up_reaches_the_tool_as_this_computer` in
  [mcp/tests.rs](../crates/locust/src/mcp/tests.rs).
- The drills, in [catching_up_tests.rs](../crates/locust/src/daemon/catching_up_tests.rs)
  (below).

## How each exit criterion was checked

- **The checks.** All pass; see Verification.
- **The second form of the drill, the lost rule change.**
  `after_the_lost_rule_change_status_names_the_computers_and_the_hosts_command_is_refused`:
  two real daemons (SQLite, Unix sockets, local Iroh); A hosts on a fixed
  loopback port, B is stopped while A starts from the copy beside the marks it
  kept. The person's `status`, run through the real command line in-process
  (`cli::run_for_test`), starts "Nothing is waiting for you." and prints the
  G-1 block: "Missing: 1 record this computer signed as host", "Heard from
  since this start: nobody yet.", "Not yet: Member's computer (...)", the
  continue line, then the restored line; it never names the goal's key.
  `rules bind --plan` exits 9 with `locust: read_only: You can't change the
  rules of "Durable lifecycle": ... It catches up by itself. To go on without
  waiting: locust --owner goal continue --goal ...` and no plan. B starts and
  calls A: the hold ends, the block is gone and the restored line stays.
- **The first form, the lost post.**
  `after_the_lost_post_status_names_the_hosts_agent_and_its_one_missing_record`:
  the block reads "Missing: 1 record Host signed", the host's agent's name;
  `rules bind` shows its plan, since the goal's own key is not held.
- **Data and marks both copied back.**
  `a_host_restored_with_its_marks_waits_for_the_person_and_continues_on_a_yes`:
  status lists the goal under "Waiting for you" with the continue line from
  the start, and still after B answered (B is then under "Heard from"). `goal
  continue --goal T --plan` shows the G-3 plan; `--confirm` with a stale id
  exits 7 with `locust: conflict: the plan changed; run --plan again`; with the
  shown id it prints `Continued "Durable lifecycle". This computer signs here
  again.`, and status then lists nothing. "Proceeds on yes" is checked with
  `--confirm`; the terminal prompt is `confirm::decide`'s, unchanged.
  `a_member_restored_with_its_marks_waits_to_hear_from_the_hosts_computer`:
  on B the block reads "Waiting to hear from the host's computer (Host), last
  seen ... Nothing is needed from you." with "To continue without it:", and
  nothing waits for the person; once B calls A the hold ends with no command.
- **An agent's post.** In both forms of the drill, the agent's `locust
  --credential ... --json contribution publish` exits 9 with `read_only`,
  `details.why.side` `this_computer`, and the agent-voice sentence "agent-1
  can't post to this goal: ... It catches up by itself, or agent-1's owner can
  continue without waiting." The MCP bridge passes the same details into the
  tool result (`a_goal_catching_up_reaches_the_tool_as_this_computer`).
- **`git grep -n "Restoring a copy is untested" -- docs`** finds the sentence
  only where the plan and its companion quote it; no guide has it. The plans
  are not edited.

## Departures and precise readings of the plan

1. `crates/locust/tests/cli.rs` exists; the plan's three CLI tests are there.
   The plan's `Node::refuse` and `Attempted::Sign(body)` are plan text: the
   tree's `Node::refusal(entry, agent, act, task, why)` is used, with the act
   from the new `access::subject(body)`. `Attempted` is unchanged, since
   `next_place` runs before any event is signed.
2. A `Behind` hold on the goal's own key waits for the person only when no
   other computer is left to hear from **and** at least one was heard. Read
   literally ("whose `waiting` is empty"), a copy that lists no other computer
   would also wait for the person, which G-5 and `waiting_for` say it must
   not: continuing there forks the goal. The refusal, the status block and
   "Waiting for you" all use `GuardView::waits_for_you()`.
3. The plan gives no agent voice for a refused host command. It reads "The
   host can't ..." and "the host can continue", since `agent_name` is `host`.
   The agent voice of the status block reads "Waiting for NAME's owner: only
   NAME's owner can say ...", "Nothing is needed from NAME's owner" and
   "NAME's owner can continue[ without them]: LINE", as the pending view
   words a command for an agent.
4. No `Halted` waiting entry exists in the tree (Phase 5 built only the task
   entries), so "a `Halted` entry is listed for `AuthorityConflict` only" has
   nothing to change.
5. `GoalSummary` carries no member names or last-seen times. The holds of the
   block come from the summaries, once per goal; the names come from one
   `goal.status` read per goal that is catching up. If that read fails the
   computers are named by their endpoint's first eight characters.
   Computers under "Heard from" print no time (G-2); those under "Not yet" do.
   Several holds in one goal print one block, their missing records joined by
   "and". The restored line with no invitation revoked reads "Restored from a
   copy. Levels, ...", and it prints on a member's computer and to an agent
   too.
6. Words the plan does not give: the `goal continue` plan line for a just
   admitted key, and for a hold on an agent's key alone "If another computer
   holds one of those records, the next record signed here conflicts with it,
   and that agent signs nothing more in this goal." Where the goal's own key
   is held the mockup's words are used. `goal continue` answers JSON
   `{"continued":[{"goal","keys"}],"changed":true}`, and with nothing held
   `{"continued":[],"changed":false}` (`--all` then reads "No goal is
   catching up. Nothing changed."). Without `--owner` the request goes to the
   daemon, which refuses it as only the owner's.
7. The no-plan rule: `goal add` refuses for the added agent with `Act::Join`,
   as the daemon's local join does; `workspace init` checks the goal's key and
   the host's agent (`Act::ProposeFiles`); `farm consent` checks the consenting
   agent. `farm off` signs a publication record until E1 gives it a
   delete-only path, so it is refused while held.
8. `lost_goals` is counted only at a start that kept the marks and found the
   data replaced or overwritten. A start that lost the marks has none to name
   goals with, and a new store is not a copy. It counts every goal the kept
   marks name and the store lacks, so it also counts goals of a data directory
   this one replaced; the sentence is true of those too. A goal where no key
   of this computer signed has no mark and is not counted.
9. Doctor creates nothing (its contract), so "cannot be created" is read as
   "does not exist and its parent cannot be written". It also fails a
   directory others may read, which the store refuses at open. Write access is
   tested with `access(2)`; a test as root cannot exercise those two cases,
   so the test covers the path, a mode others may read and a file in the
   directory's place.
10. The drills' host listens on a fixed loopback port (`Running::start_at` in
    [durable_tests.rs](../crates/locust/src/daemon/durable_tests.rs)). A host
    learns no member's address with lookup off, and a member reaches a host
    only where its ticket said, so a restored host on a new port would never be
    called. As in G1's manual drill, B is stopped while A starts from the copy
    and started afterwards. The member-side drill keeps G1's `Gated` node.
11. The Halted case of the stall test is made by a fork instead of a gap: a
    received record of a local key past a gap raises that key's mark (G1), so
    the guard now holds it and the step reads `catching_up`, as the plan's
    order says it should.
12. The simulator's restart comparison leaves out the catching-up waiting
    entries and `lost_goals`, which follow from whom the guard heard and from
    the start, both memory only.
13. The MCP sentence goes after A2's attempt sentence, per the orchestrator's
    order. `INSTRUCTIONS` is one line, so the merge with A2 conflicts on that
    line whatever the place; SKILL.md's paragraph sits after the paragraph on
    ordering work, away from A2's insertion.

## Not done, and why

- The joiner's sentence of G-4 ("The host's computer is catching up and
  admits nobody yet ...") needs the field the joinable plan stores; that plan
  owns it.
- `goal continue --all` also continues holds that would end by themselves, as
  the plan's notes leave it.
- E1's parts: no catching-up entry and no guard views for an ended goal,
  `Act::End`, and `goal end` and `farm off` under the no-plan rule.
- The roles plan's reading test with people, which should include "catching
  up".

## Verification

On the commit `daa1dac`:

- `cargo fmt --all --check`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 39 test suites, 1,366 passed, 0 failed,
  12 ignored.
- `cargo build --locked -p locust`, then `python3 scripts/check_formations.py`:
  the regenerated contract verifies.
- `python3 scripts/check_documentation.py --binary target/debug/locust
  --timeout 60` (Python 3.13): all four recipes pass.
- `python3 -m unittest discover -s scripts/tests`: 335 run, 10 skipped, OK.
- `python3 scripts/check_docs.py`: only the five lines that fail on the base
  commit `665fbe7` too (plan links to files that pinned source commits name,
  missing from the remote's history), with these notes and their index entry
  staged.
- `sites/locust.farm`: `npm ci`, then `npm run lint`, `npm run check`, `npm
  test` and `npm run build` pass; the site reads the guides this phase
  changed.
- The drills above, run five times in a row: all passed, in under four
  seconds each run.

## Notes for E1

- `GoalStatus.guard` is `Vec<GuardView>`: for the owner every local key's own
  hold, the goal's own key's included where this computer hosts the goal; for
  an agent its own and the goal's key's. `GuardView::waits_for_you()` says
  whether only the person can end it. E1's "an ended goal has no guard views
  and no catching-up entry" goes in `guard_views`, `Node::halt` and the
  catching-up loop of `waiting_for`.
- `next_place(entry, author, body: &Body)` checks, in order: the goal's own
  key's authority halt; member and `Local.part` (skipped for the goal's key);
  the hold, refused as `Why::ThisComputer` with the act of
  `access::subject(body)`; an agent's own fork (`halted`); `Goal::next`. E1
  puts `conflict(ENDED)` first and maps `Body::GoalEnded` to `Act::End` in
  `access::act`; `act_phrase` and the tests that list every act need it too.
- `goal continue` lives in only_you.rs (`goal_continue`, `continue_plan`,
  `continue_block`); its plan already names "the end of the goal".
  `refuse_if_host_held(observed, act, task)` is what a host command's plan
  calls first; `goal end` should call it with `Act::End`, and E1's
  delete-only `farm off` must skip the call that farm.rs now makes.
- The drill harness (`cli::run_for_test`, `Running::start_at`, the helpers of
  catching_up_tests.rs) runs a command line against real daemons in-process.
