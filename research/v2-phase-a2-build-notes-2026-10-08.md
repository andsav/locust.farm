# Locust v2 A2: start the next task

Status: implemented in `47379f9`; the required checks and the exit drill
passed, except five `check_docs.py` lines that fail on the base commit too
(see Verification). This is
phase A2 of the [agent memory and store plan](../docs/agent-memory-and-store-plan.md)
(section "A2: Start the next task"), one of the phases alongside the fifteen
in the [master plan](../docs/master-plan.md)'s build order. The plan was read,
not edited. The predicate it reuses came from H1a and is described in
[the hooks qualification](agent-hooks-qualification-2026-10-07.md).

## What changed

- **Request.** `AttemptStart.task` is `Option<TaskId>`
  ([api.rs](../crates/locust-proto/src/api.rs)). `Request::check` refuses an
  offer with no task as `invalid`: "an offer belongs to a task; name the task
  too". `is_answered_by` lets only a no-task start take `Claimed` or
  `Pending`, where a named start still takes only `Claimed`; the client
  enforces it. The field's doc, the `Response::Pending` doc and the `attempt.start`
  summary say what a no-task start answers. The JSON schema leaves `task` out
  of `required`, so the CLI's `--task` and the MCP tool's `task` turned
  optional with no code change; `--session` stays required.
- **The pick.** `attempt_start` in
  [claims.rs](../crates/locust-core/src/node/requests/claims.rs), with no
  task: checks membership and the session, validates the session's binding
  without writing it, reads `pending_work`, answers the first of
  `work.claimed` if this session holds a live claim, else takes the first
  `to_start` item whose `unattended` is true and runs the existing signing
  path with that item's task and offer, else answers `Pending(work)` and
  signs nothing. One request plans and lands the pick, so two sessions on one
  daemon never take the same task. `to_start` already leaves out forbidden,
  closed and above-level tasks, so `ask` never touches `ask_first`.
- **Agent words.** `INSTRUCTIONS` in [mcp.rs](../crates/locust/src/mcp.rs)
  now says: "Independent work begins with an attempt: when locust_wait or
  locust_pending lists tasks to start, locust_attempt_start without a task
  takes one nobody attempts. Contributions do not select or apply files."
  [SKILL.md](../skills/locust/SKILL.md) gains the plan's paragraph, and
  [concepts.md](../docs/guide/concepts.md) one sentence.
- **Hooks.** The own-call check in
  [hooks.rs](../crates/locust-adapter/src/hooks.rs) matches a claim to the
  requested task only when a task was named; a no-task start takes any claim
  in its goal. The core's stop rule
  ([core.rs](../crates/locust-adapter/src/hooks/core.rs)) does not count a
  start answered with pending work as a write, and the
  [agents guide](../docs/guide/agents.md)'s Hooks section says so.
- **Literals.** Every `Request::AttemptStart` literal names `task: Some(..)`:
  `sim/scenario.rs`, thirteen `node/tests` files, `organizations.rs` (7),
  `client.rs`, `durable_tests.rs` (3) and
  [cli.rs](../crates/locust/tests/cli.rs) (1).
- The generated [runtime contract](../docs/reference/generated/runtime.contract.json)
  is regenerated: the optional field and its description, the `--task` flag
  no longer required, the summary and the `Pending` doc.

## Departures and precise readings of the plan

1. **No new `unattended` helper.** H1a already added `Goal::unattended`
   ([goal/mod.rs](../crates/locust-core/src/goal/mod.rs)) and
   `WorkItem.unattended`, with the note that A2 must reuse it. The pick reads
   `item.unattended` from the same `pending_work` the hooks read, so the
   predicates match by construction. The comment on `Goal::unattended` now
   names both users.
2. **One function.** The no-task branch resolves a task and offer and falls
   into the existing path, rather than a separate function; the recovery loop
   then finds nothing, since `work.claimed` was empty.
3. **Offers.** `to_start` lists an offer made to the caller as its own item,
   so the pick starts an offered task with that offer
   (`at_ask_a_start_without_a_task_leaves_ask_first_tasks_alone_and_takes_an_allowed_offer`).
   Where a task has both an independent item and an offered one, the
   independent item comes first: the sort is stable and lists it first. The
   plan does not say which.
4. **A cancelled claim** whose cancellation is not answered is still in
   `work.claimed`, so a no-task start returns it, as the plan's "live claim"
   (views.rs) reads. The skill says to finish and report it first; pending,
   the hooks and the refusals on report point at the cancellation.
5. **The pending answer and the stop rule.** The plan names no hook change.
   A start answered with pending work signs nothing, but the hook core counted
   every `attempt.start` as a write, so it re-armed a block already ignored.
   It is now neutral, like a context acknowledgment. As built, a start that
   answered a claim still counted; after the review, a claim the chat
   already knows at the same generation is neutral too (below). The chat
   still becomes a worker chat. Tested by
   `a_start_that_found_nothing_to_take_does_not_block_its_turn_end_again`.
   The stop line's text was not changed: it names `locust_attempt_start` and
   a task ID, which works with either shape.
6. **SKILL.md placement.** The plan puts the paragraph "after 116-117", the
   sentence "Start with `locust_attempt_start` ..." at its baseline. It is its
   own paragraph after A1's ordering paragraph instead, so `After task:` is
   explained before this paragraph says the pick ignores it, and no existing
   line changes. `After task:` is in backticks, as A1's paragraph writes it;
   the words are the plan's.
7. **concepts.md.** The plan gives no words; the sentence is "An agent can
   start without naming a task: it then takes the first task nobody is
   attempting, as far as its computer has heard." The agents guide also says
   a chat that asked to start an attempt is a worker chat.
8. **Summary length.** The `attempt.start` summary grew to 39 words; A1 keeps
   summaries near 30.
9. **Tests and lines.** The plan's six context-view cases are five tests; "a
   second session gets another task" is inside the first, with another
   session of the same member, since that is the case `attempting` misses.
   `tests/cli.rs` is now `crates/locust/tests/cli.rs` and `t2_flow.rs` is
   `crates/locust/tests/t2_flow.rs`. There are thirteen `node/tests` files to
   change, not twelve: A3 added `current_findings.rs`.
10. **The plan's unverified point** holds: schemars leaves `Option<TaskId>`
    out of `required`; `only_a_start_without_a_task_is_answered_with_pending_work`
    asserts it.

## Not done

- E1 decides what a no-task start in an ended goal answers; E1 is not built.
- J2 narrows `to_start`; the pick will follow it with no change.
- The pending view's text still prints shortened IDs and "Participant
  action"; those belong to A5a and the status phases.

## Tests

- [context_views.rs](../crates/locust-core/src/node/tests/context_views.rs):
  `a_start_without_a_task_takes_the_first_unattended_task_and_a_failed_attempt_frees_it`
  (another session of the same member gets the other task, a third member
  gets pending, and a failed attempt frees its task),
  `repeating_a_start_without_a_task_returns_the_same_claim` (also a claim on
  a task named earlier; nothing is written),
  `with_every_task_attempted_a_start_without_a_task_answers_pending_and_signs_nothing`
  (no task at all, then all attempted: the answer equals a `pending` read, the
  revision and attempts are unchanged, a fresh session is not bound, and a
  named start still works),
  `at_ask_a_start_without_a_task_leaves_ask_first_tasks_alone_and_takes_an_allowed_offer`,
  `a_start_without_a_task_is_denied_on_another_members_session` and
  `a_pending_answer_under_an_idempotency_key_replays_on_retry` (the plan's
  risk note, pinned). Replacing `item.unattended` with an empty `attempting`
  list fails the first test.
- [api.rs](../crates/locust-proto/src/api.rs):
  `only_a_start_without_a_task_is_answered_with_pending_work`.
- [args.rs](../crates/locust/src/cli/args.rs):
  `attempt_start_parses_without_a_task`.
- [t2_flow.rs](../crates/locust/tests/t2_flow.rs): the MCP flow starts with
  `{"goal": goal}` and checks the claim names the only task.
- [hooks.rs](../crates/locust-adapter/src/hooks.rs):
  `a_start_without_a_task_is_typed_with_any_claim_in_its_goal_or_pending_work`;
  [core.rs](../crates/locust-adapter/src/hooks/core.rs):
  `a_start_that_found_nothing_to_take_does_not_block_its_turn_end_again`.
- The plan's must-pass tests, the least-attended ordering test in
  context_views.rs and `pending_names_who_is_attempting_and_counts_approvals`
  in presentation.rs, pass unchanged.

## Exit drill

A throwaway daemon under `/tmp` (`mktemp -d /tmp/locust-a2.XXXXXX`,
`LOCUST_RELAY=none LOCUST_LOOKUP=none LOCUST_BIND=127.0.0.1:0`), the debug
binary, an `open` goal with agents alice and bob at the default level, and one
task. Passed:

| Step | Observed |
| --- | --- |
| `alice attempt start --goal G` | Prints `attempt ...`, `generation 1`, `instance ...`; exit 0 |
| The same, twice, with `--json` | The same claim both times, naming the only task |
| `bob attempt start --goal G` | Prints the pending view: the task under "Ready to start" with alice attempting; exit 0 |
| The same with `--json` | `pending`, the task with `unattended: false`, no claim |
| `bob attempt start --goal G --offer 00...0` | `invalid: an offer belongs to a task; name the task too`; exit 6 |

## Verification

On `47379f9`, Python 3.13:

- `cargo fmt --all --check`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 39 suites, 1,356 passed, 0 failed, 12
  ignored. A first run stopped at
  `transaction::tests::external_change_or_unknown_journal_never_authorizes_mutation`
  in `locust-workspace` ("another process is updating this checkout") while
  the documentation recipes ran beside it; this phase does not touch that
  crate, the test passed three times alone, and the full rerun passed.
- `cargo build --locked -p locust`, then `python3 scripts/check_formations.py`:
  the regenerated contract verifies.
- `python3 scripts/check_documentation.py --binary target/debug/locust
  --timeout 60`: all four recipes pass.
- `python3 -m unittest discover -s scripts/tests`: 335 run, 10 skipped, OK.
- `python3 scripts/check_docs.py`, with these notes and their index entry
  staged: only the five missing link targets that fail on the base commit
  `665fbe7` too (pinned plan source commits are not in the remote's
  history), in `host-safety-and-ending-plan.md` 1198 and
  `roles-and-permissions-plan.md` 828, 961, 988 and 1092. Nothing else.
- The exit drill above passed.

## After the review

The [review of A2 and S1b](v2-side-a2-s1b-review-2026-10-08.md) found three
A2 defects, none blocking. All three are fixed on `side/a2`, after S1b's two
commits were cherry-picked onto it (`f015564`, `e82cd8c`; the master plan's
pieces table now reads "A1, A2, A3 and S1"). Review numbers are its headings.

| # | Finding | Disposition | Commit | Test or text | Note |
| --- | --- | --- | --- | --- | --- |
| 1 | A start without a task that returns the held claim re-arms the stop block | Fixed | `01bbf6e` | `a_start_that_returns_the_held_claim_does_not_block_its_turn_end_again` ([core.rs](../crates/locust-adapter/src/hooks/core.rs)); the Hooks section of the [agents guide](../docs/guide/agents.md) | `observe` treats an `attempt.start` or `attempt.takeover` answered with a claim already in the chat's baseline at the same generation as no write, beside the pending answer. The test obeys the free-task line twice with no task and once by a takeover at the same generation, in a person's chat; it fails without the fix. A new generation still counts. The review's optional change to the free-task line was not made: the skill says to finish a held claim first, and a line naming the task would start a second attempt beside it. |
| 2 | The skill tells an agent to "finish" a returned claim whose cancellation is waiting | Fixed | `a079e67` | `a_claim_returned_with_its_cancellation_waiting_is_acknowledged_first` ([mcp tests](../crates/locust/src/mcp/tests.rs)); [SKILL.md](../skills/locust/SKILL.md) | The review's sentence: "finish and report it first, or, when `locust_pending` lists a cancellation for it, acknowledge that first." |
| 3 | An older skill sentence says an agent need not request the next task | Fixed | `a079e67` | `the_skill_never_says_an_agent_need_not_ask_for_its_next_task` | Now "an agent need not open a flow's next task itself", the review's words. |

Finding 4 is S1b's, recorded in its
[build notes](v2-phase-s1b-build-notes-2026-10-08.md); finding 5 waits for G2.

Verification, on the tree of `32ec5a5` (all three fix commits and S1b's
finding 4): `cargo fmt --all --check` clean; workspace clippy with
`-D warnings` clean; `cargo test --locked --workspace` 39 suites, 1,365
passed, 0 failed, 12 ignored; `cargo build --locked -p locust`, then
`check_formations.py` verifies with the contract unchanged;
`check_documentation.py` passes all four recipes; the Python helper tests ran
335 with 10 skipped, OK; `check_docs.py` prints only the five baseline lines.
The full set ran once on that final tree; each fix commit ran its own new
tests, and each new test was seen to fail without its fix.
