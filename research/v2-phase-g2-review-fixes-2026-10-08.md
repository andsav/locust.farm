# Locust v2 phase G2 review fixes

Status: implementation record, 8 October 2026. All five findings of the
[review](v2-phase-g2-review-2026-10-08.md) of phase G2 as built are fixed on
the branch `fix/g2`. The branch is linear on top of the integration branch at
`ab8520d`, after A2 and S1b. Each code fix has a test that fails without it.
The tests were run both ways: the fix stashed or its old code put back, then
restored. The [G2 build notes](v2-phase-g2-build-notes-2026-10-08.md) describe
the phase as first built. Where this record and those notes differ, this
record is current.

## Disposition

| # | Finding | Disposition | Commit | Test or text | Note |
| --- | --- | --- | --- | --- | --- |
| 1 | An agent with its own missing records reads "catches up by itself" while the goal waits for the person | Fixed | `a7d7f50` | `only_a_hold_no_other_computer_can_end_waits_for_the_person`, extended ([guard tests](../crates/locust-core/src/node/tests/guard.rs)); `an_agent_held_with_the_goals_key_reads_that_the_person_ends_the_wait` ([cli tests](../crates/locust/tests/cli.rs)) | `Node::hold_view` in [guard.rs](../crates/locust-core/src/node/guard.rs) and `holding` in [only_you.rs](../crates/locust/src/cli/only_you.rs) give an agent the goal's own key's view whenever this computer hosts the goal and that key is held. The agent's own `Behind` view is kept only while the goal's key still waits for another computer, that is while its view does not `waits_for_you()`. The core scenario is the review's: an admission and the host's agent's finding are both lost, and the member answers with neither. Without the fix the refusal names the agent's key and says it catches up by itself. |
| 2 | `goal add` of an agent already in a held goal is refused, though it would sign nothing | Fixed, with `farm off` | `a7d7f50` | `goal_add_and_farm_off_that_sign_nothing_go_on_while_the_host_is_catching_up` ([cli tests](../crates/locust/tests/cli.rs)) | `add_plan` asks whether the agent is already a member first, and refuses only an admission it would sign. [farm.rs](../crates/locust/src/cli/farm.rs) reads `farm show` first. `farm off` of a page whose `desired` is none signs nothing, as the daemon does, so it goes on to its plan. A page that is on is still refused while held. The test checks each half failing without its own part of the fix. |
| 3 | Some words of the `goal continue` plan point at nothing | Fixed | `a7d7f50` | `the_continue_plan_puts_the_hosts_records_first_and_names_what_each_hold_risks` ([only_you.rs](../crates/locust/src/cli/only_you.rs)) | `continue_block` prints the host's line first and "They may include ..." right after it, then the agents' lines. Where only agents' keys are held, the closing line names them. Their missing records give "and NAME then signs nothing more in this goal". A copy of unknown age on a member's computer gives "If another computer holds a record that NAME signed after this copy was made, ...". A goal whose only hold is a key just admitted reads "Continue: sign in this goal before this computer has heard from the host's computer." It adds one line, "NAME was just admitted, and this computer has not heard from the host's computer since. Continuing skips that check.", and no line about copies, computers heard or conflicts. The words for the host's key are the mockup's, unchanged. |
| 4 | Doctor reads a symlinked marks directory as "not a directory"; the daemon uses it | Fixed | `a7d7f50` | `doctor_names_the_marks_directory`, extended ([doctor tests](../crates/locust/src/cli/doctor/tests.rs)) | The `marks` check in [doctor.rs](../crates/locust/src/cli/doctor.rs) reads through a link with `fs::metadata`, as the store's `create_directories` and `marks::open` do. A link that points at nothing fails with "is a link to something that does not exist", because the store cannot create the directory over it. No link is refused, in doctor or in the store. |
| 5 | The guide's row for a refused action does not name the new side | Fixed | `4e98423` | [help.md](../docs/guide/help.md), "An action is refused" | The row names the goal's state too, which it had left out before G2, and this computer, "which is catching up after a [restore](../docs/guide/operations.md#backups)". |

## What changed for E1

E1 is built on `a2cde40`, before these fixes. The names it uses are unchanged:
`next_place(entry, author, body)`, `Node::hold_view`, `guard_view`,
`GuardView::waits_for_you`, `refuse_if_host_held`, `refuse_if_agent_held`,
`continue_block`, `continue_plan`, `goal_continue` and `waiting_for` keep their
signatures. Three behaviours changed:

- **The view an agent's refusal carries.** On the host's computer, while the
  goal's own key is held and its view waits for the person, an agent's refusal
  carries that view even if the agent has a `Behind` hold of its own.
  Elsewhere nothing changed. The same rule holds in the CLI's `holding`.
- **What `farm.rs` asks first.** For `farm off` it now calls `farm show`
  before `goal status`. It checks the hold only when the page is on, that is
  when the status's `desired` is some. E1's delete-only path for `farm off`
  in a held goal replaces this check. The `refuse_if_host_held` call in
  `farm.rs` now sits inside an `if signs { .. }` block. Where E1 skips the
  check for `farm off`, `signs` can simply be false for that operation.
- **The `continue_block` text.** Its line order and closing line now depend
  on which keys are held. A test that asserts the plan's text for a hold on
  the goal's key sees the same words as before. The order is the host's line,
  "They may include ...", then the agents' lines.

`views.rs` is unchanged by these fixes.

## Verification

On `4e98423`:

- `cargo fmt --all --check`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 39 suites, 1,389 passed, 0 failed,
  12 ignored.
- `cargo build --locked -p locust`, then `python3 scripts/check_formations.py`:
  verifies. The contract is unchanged: no API or command-line shape changed.
- `python3 scripts/check_documentation.py --binary target/debug/locust
  --timeout 60` (Python 3.13): all four recipes pass.
- `python3 -m unittest discover -s scripts/tests`: 335 run, 10 skipped, OK.
- `python3 scripts/check_docs.py`: only the five lines that fail on the base
  commit `665fbe7`, with this record and its index entry staged.
- `sites/locust.farm`: `npm ci`, then `npm run lint`, `npm run check`, `npm
  test` and `npm run build` pass. The site reads help.md.
