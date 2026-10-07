# Locust v2 phase R5 review fixes

Status: implementation and verification record, 6 October 2026.

This follows the [R5 review](v2-phase-r5-review-2026-10-06.md) of `9c337df`
and the plan corrections the review itself made in `fa142b2`. The review gave
40 of its 47 findings to a fix session; findings 24 to 27 went to the plan and
were corrected with the review, 44 and 46 went to the guides phase, and 45
waits for qualification. The 40 are accounted for below. No signed encoding
changed. No plan, guide, script or skill was edited; the plan corrections the
fixes call for are listed at the end for the plan's owner.

Other sessions committed guides and research in the same checkout while this
work ran (`e46450f`, `9499cde`, `acf9d6e`). Shared files were reread before
editing, and each commit here stages only this session's paths.

## Findings and disposition

The paths and line numbers identify the review headings, not current lines.
Commit labels in the last column are expanded below.

| # | Review heading | Disposition and evidence | Commit |
| --- | --- | --- | --- |
| 1 | `cli/selectors.rs:88`, a name that passes for a key | **Fixed.** A name of 8 to 64 hex digits is not a member name (`is_member_name`, with a signed vector), so admissions and join requests refuse it and the terminal says why; an enrolled local name that looks like a key counts as a second candidate beside the key prefix and is refused as ambiguous. The removal plan and confirmation print the removed member's label, not the text the host typed. | A |
| 2 | `cli/presentation.rs:951`, task text posing as a command | **Fixed.** Wanted and refused titles come from the task's first line, cut and escaped by `Entry::task_title`; quotation marks and backslashes inside are escaped by `quoted`. The copyable command sits on its own line under the title in status, goal status, pending and watch. | B |
| 3 | `requests/goals.rs:239`, a bind that removes a review | **Fixed, option (b).** `rules bind` gives the counting role to no one when any earlier binding, which open tasks still follow, lets one holder of that role act alone (`organization::acts_alone`). The plan says so in a sentence before the `role give` lines, and `GoalStatus.acting_alone` names such roles. A Node test shows the single reviewer's self-approval does not count after such a bind and a lead's approval does; a real daemon test reads the sentence and the unchanged holder list. The owner may prefer option (a), re-reviewing under the new rules; see the plan corrections. | F |
| 4 | `node/views.rs:599`, findings dropped from review lists | **Fixed.** Goal-wide findings and document revisions stay in every reviewer's to-review list after a new binding; the list no longer compares their rules revision with the current one. | D |
| 5 | `node/views.rs:173`, allow at level read | **Fixed.** The waiting list distinguishes a task waiting on an allowance from an agent waiting on its level (`WaitingKind::SetAsk`); at read, status offers the level command, and the allow command prints its own "takes the task once it is set to ask" line. | D |
| 6 | `cli/workspace.rs:548`, a departed accepting member | **Fixed, option (b).** `workspace init` replaces a kept participant integrator who is no longer a member with the host's agent, marks the policy for rebinding, and shows the choice in the plan. Four plan cases (explicit or not, present or not) assert the integrator and whether rules are rebound. | G |
| 7 | `cli/workspace.rs:596`, files pinned to replaced rules | **Fixed.** The daemon refuses `WorkspaceEpochSet` with `conflict` when its rules are not the goal's current revision; a Node test and a CLI test with rules shifted between the read and the write cover it. | G |
| 8 | `node/views.rs:197`, whole text as a title | **Fixed** with finding 2: titles are the first line, cut to a width. | B |
| 9 | `api/level.rs:450`, the empty rule | **Fixed.** An empty rule reads "nobody takes a task here without being handed it" or "these rules let nobody hand a task to that member", and a refused start under hand-out rules ends "A task can be taken once it is handed out." instead of "pick other work". | C |
| 10 | `api/level.rs:417`, the author who holds the role | **Fixed.** A role holder barred as the author reads that it wrote the result and another holder must review; no give-the-role advice. `RuleRefusal.author` carries it to both voices. | C |
| 11 | `cli/only_you.rs:739`, a give line off the host | **Fixed.** A repeated add or join prints the `role give` line only on the hosting computer for the owner; elsewhere it says the host, named as the owner of the host's agent, gives roles. A real two-daemon test reads the member-side sentence. | E |
| 12 | `cli/only_you.rs:592`, eight-character goal cuts | **Fixed.** Printed commands cut the goal identifier among the daemon's goals (`cut_goal`, `cut_goal_among`), growing the prefix past eight when another goal shares them, and fall back to the full identifier when the daemon cannot be asked. | E |
| 13 | `cli/roles.rs:112`, unguarded role names | **Fixed.** `role_command_line` quotes role names that need it, guards a leading dash with `--`, and prints hidden characters escaped. | E |
| 14 | `cli/presentation.rs:796`, a revoked agent's lines | **Fixed.** A revoked agent shows no level or wanted task; a disconnected agent shows its reconnect line by name, with the key in a note. | D |
| 15 | `node/views.rs:170`, allow lines that settle nothing | **Fixed.** The waiting list checks the agent is connected, has not taken the task and may still start it (`Node::may_start`) before offering an allow line. | D |
| 16 | `cli/only_you.rs:1468`, any-of reviewer counts | **Fixed.** `reviewer_requirement` and `missing_reviewers` read a role inside an any-of choice; bind and create plans and goal status print the right count and one `role give` line per missing holder. A CLI test binds such rules and reads "2 more reviewers are needed." | E |
| 17 | `requests/goals.rs:624`, oversized bind advice | **Fixed.** A bind that cannot fit every member in the role says so and names `--no-role`; give and take name removing a member or using another role. Both sentences are asserted. | F |
| 18 | `cli/only_you.rs:1073`, re-running a waiting join | **Fixed.** A repeated join whose admission already landed under the earlier name says that name stays; while still waiting it says the new name is asked for. A CLI test runs both states through a plan and its confirmation. | E |
| 19 | `cli/presentation.rs:434`, "Nobody is attempting it" | **Fixed.** The line reads "No other member is attempting it", which is what the list behind it covers. | E |
| 20 | `requests/levels.rs:203`, "can't resume" | **Fixed.** Allowing a closed, finished or picked task is refused as `Act::TakeTask` on that task, through a shared `Node::refusal`; the regression asserts the act and task. | F |
| 21 | `node/access.rs:60`, "this task" and "approve" | **Fixed.** Refused close, reopen and review name the scope that was asked (task, goal, plan document, shared files) and the verdict that was given. | C |
| 22 | `api/level.rs:262`, the host as the person's owner | **Fixed.** `Why::Rules` carries `hosted_here`; on the hosting computer the person's voice says "you" where other views do. | C |
| 23 | `checks.ts:130`, a role as the accepting member | **Fixed.** The editor's note names a specific agent by key, the bad-key correction asks for the member's 64-character public key, a role integrator draws one diagnostic, the problems copy drops "a role is usually better", and the `Authority` comment states the exception. Generated schema, contract and vectors regenerated. | H |
| 28 | `cli/presentation.rs:847`, same names told apart | **Fixed.** Members list in admission order (`MemberView.admitted`), and a shared-name refusal marks the later one ", joined later". | E |
| 29 | `cli/presentation.rs:261`, eight-character member labels | **Fixed.** `member_label` grows the key prefix past eight characters when another member's key shares them. | E |
| 30 | `node/views.rs:145`, the asked name while joining | **Fixed.** Status shows the name the person asked for while the agent is joining or was refused. The build notes' "byte for byte" claim is for their owner to correct. | D |
| 31 | `cli/presentation.rs:909`, "become reviewers" | **Fixed.** Goal status prints a `role give` line for each existing member without the counting role, and the "members you add become" sentence only when every member holds it. | E |
| 32 | `requests/invitations.rs:182`, roles missing from the refusal | **Fixed.** `goal invite` with an unknown role carries `roles` in its details like `role give`; the admission test asserts the list. | F |
| 33 | `cli/watch.rs:106`, stale names after a wait | **Fixed.** After waking for work, watch reads members and tasks again before printing. A CLI test shows a task and a member that appeared during the wait printing with title and name; it failed without the change. | G |
| 34 | `failure.rs:102`, the dropped operation identifier | **Fixed.** `for_person` replaces the agent-voice sentence inside the message and keeps the words a command put around it, such as the publication prefix. | G |
| 35 | `api/level.rs:567`, four invisible ranges | **Fixed.** `safe` escapes the four missed ranges; its comment matches. | C |
| 36 | `cli/only_you.rs:365`, reads after the change | **Fixed.** `level` and `allow` read the daemon to cut an identifier only when the line prints (not under `--json`), and a failure of that read falls back to the full identifier instead of reporting the saved change as failed. | E |
| 37 | `cli/only_you.rs:1578`, the role named twice | **Fixed.** A bind refused for a role changing kind names the role once in the person's line, and the JSON message is the daemon's. A CLI test checks both forms. | E |
| 38 | `organization/roles.rs:96`, the integrator duty | **Fixed.** The line is gone; `names` is a module function. | F |
| 39 | `failure.rs:165`, the owner's refusal end to end | **Fixed.** `an_owner_reads_a_refusal_in_the_persons_voice_and_json_keeps_the_daemons` runs the binary with `--owner` against a rules refusal of `contribution publish`: exit 13, the person's sentence with the quoted goal title on stderr, and the daemon's message unchanged under `--json`. | I |
| 40 | `cli/mod.rs:491`, pending and watch in text mode | **Fixed.** `pending_in_text_mode_names_the_attempting_member_and_the_tasks_title` runs two owners through a real daemon: a taken task prints its title and attempting member, a posted result prints its approval count, and `watch --timeout-ms 0` names the task. | I |
| 41 | `api/level.rs:850`, escaping unasserted | **Fixed.** `titles_are_quoted_escaped_and_cut` and `terminal_controls_and_bidi_never_reach_the_terminal` put terminal controls and a bidi override in a task title, goal title, member name, role name and host name and assert the escaped forms in both voices; the status view's runs-as-printed test carries a hostile title that tries to close its quotation and add a command; the CLI test for a role named like a flag asserts hidden characters print escaped. | B, C, E |
| 42 | `cli/presentation.rs:1936`, two unrendered commands | **Fixed.** The runs-as-printed test renders the reconnect line for a disconnected agent and the level line for an agent at read. | D |
| 43 | `node/tests/daemon.rs:145`, two local agents | **Fixed.** The daemon test joins a second local agent to the same goal and checks that each agent's status lists only its own entry. | D |
| 47 | `tla/organization.md:33`, the anchor bound in the map | **Fixed.** The property map has a row for evidence anchored before its subject: not modeled, with the one Rust test named. | I |

Findings 24 to 27 (plan text), 44 and 46 (guides), and 45 (qualification) are
not this session's.

## Implementation references

- Refusal wording: [level](../crates/locust-proto/src/api/level.rs), [access](../crates/locust-core/src/node/access.rs), [failure](../crates/locust/src/failure.rs).
- The status view: [views](../crates/locust-core/src/node/views.rs), [entry](../crates/locust-core/src/node/entry.rs), [presentation](../crates/locust/src/cli/presentation.rs), [watch](../crates/locust/src/cli/watch.rs).
- Host requests: [goal requests](../crates/locust-core/src/node/requests/goals.rs), [invitation requests](../crates/locust-core/src/node/requests/invitations.rs), [level requests](../crates/locust-core/src/node/requests/levels.rs), [workspace requests](../crates/locust-core/src/node/requests/workspace.rs), [roles](../crates/locust-core/src/organization/roles.rs).
- Terminal: [owner commands](../crates/locust/src/cli/only_you.rs), [selectors](../crates/locust/src/cli/selectors.rs), [role commands](../crates/locust/src/cli/roles.rs), [workspace commands](../crates/locust/src/cli/workspace.rs).
- Tests: [CLI assertions](../crates/locust/tests/cli.rs), [workspace assertions](../crates/locust/tests/workspace.rs), [real daemon tests](../crates/locust/tests/t2_flow.rs), [daemon tests](../crates/locust-core/src/node/tests/daemon.rs), [role tests](../crates/locust-core/src/node/tests/roles.rs), [workspace lifecycle](../crates/locust-core/src/node/tests/workspace_lifecycle.rs).
- Site and exports: [checks](../sites/locust.farm/src/lib/formation-editor/model/checks.ts), [site validation](../sites/locust.farm/src/lib/formation-editor/contract/rules.ts), [Rust validation](../crates/locust-core/src/organization/validation.rs), [runtime contract](../docs/reference/generated/runtime.contract.json).
- Formal boundaries: [property map](tla/organization.md).

## Commits

| Label | Commit | Scope |
| --- | --- | --- |
| A | `1b21c3f` | Key-like names refused; the removed member labeled. |
| B | `3d978f7` | Titles as escaped, cut first lines; commands on their own line. |
| C | `756c896` | Rules refusals for an empty rule, an author, the hosting person, every scope; four invisible ranges. |
| D | `f5b82e2` | Waiting list settles only what a command settles; set ask at read; findings kept after a bind; disconnected agents; the asked name. |
| E | `7c833c8` | Runnable role commands; any-of reviewer counts; who gives roles off the host; admitted order and longer prefixes; repeated joins. |
| F | `0e902e3` | No role from a bind where one reviewer acts alone; oversized-role advice; take, not resume; roles in the invite refusal; the integrator duty removed. |
| G | `88304d1` | Files not pinned to replaced rules; a departed integrator's part to the host; watch rereads; the kept prefix. |
| H | `39dc890` | The site no longer suggests a role as the accepting member; exports regenerated. |
| I | `0a448ad` | The owner's refusal and text-mode pending through the binary; the anchor bound in the property map. |

## Verification

Each Rust commit was checked with `cargo fmt --all --check`, workspace Clippy
with `-D warnings`, and the tests of the crates it touched; API changes
regenerated the contract through `scripts/check_formations.py --write`. New
tests for findings 33 and 39 were run against the unfixed code and failed
there. The final checks ran at `0a448ad` with `/opt/homebrew/bin/python3`:

| Required check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed. |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed. |
| `cargo test --locked --workspace --no-fail-fast` | Passed: 1,156 tests, 0 failures, 15 ignored. |
| `python3 scripts/check_formations.py` | Passed; six examples and every diagnostic conformance vector. |
| `python3 scripts/check_documentation.py --binary target/debug/locust --timeout 60` | Passed: shared-workspace-loop, local-collaboration, private-authoring, separate-goal-export. |
| `python3 -m unittest discover -s scripts/tests` | Passed: 301 tests, 3 skipped. |
| `python3 scripts/check_docs.py` | Passed with this record and its index entry staged. |
| Site `npm run lint` | Passed. |
| Site `npm run check` | Passed: 0 errors, 0 warnings. |
| Site `npm test` | Passed: 222 tests. |
| Site `npm run build` | Passed: 89 prerendered routes, 14 raw articles, 10 exact assets. |

During an earlier per-crate run under load,
`daemon::reconcile_tests::a_diverged_author_log_reconciles_between_two_real_daemons`
failed on timing and passed alone; the final full suite passed without a
retry. `scripts/check_tla.py` was not run: the model did not change, only the
property map's text. Nothing was run on two physical machines; the
member-side sentences come from two daemons on one computer.

## Remaining plan corrections

No plan was edited. The owner of the
[roles plan](../docs/roles-and-permissions-plan.md), its
[companion](../docs/roles-and-permissions-plan-details.md) and the
[R5 build notes](v2-phase-r5-build-notes-2026-10-06.md) should make these
corrections, cited by the review's line numbers:

- Finding 1 (plan 2729–2736, details row 312): a member name that is a key or
  a hex prefix of one is refused at add, join and invite; selection does not
  need a key-before-name order.
- Finding 2 (plan 2028): titles are the task's first line, cut and escaped,
  with the command on the next line.
- Finding 3 (plan 2721–2727): the role is given to no one when an earlier
  binding, still followed by open tasks, lets one holder of the counting role
  act alone; `GoalStatus.acting_alone` lists such roles. If the owner prefers
  re-reviewing under the new rules instead, that is a replay change not made
  here.
- Finding 5 (plan 3363–3367, details 371): at read, status offers the level
  command, and allow prints its "once it is set to ask" line.
- Finding 6 (plan 2749–2753, details 242): init writes the host's agent only
  when the rules have no workspace part or name a participant who left, and
  shows a replaced integrator in the plan; `verify_pinned_initial_policy`
  compares the pinned part.
- Finding 10 (Phase 5 bullet, P5-2): an author who holds the role is told
  another holder must review.
- Finding 12 (Phase 5 cut note): goal cuts grow past eight characters among
  the daemon's goals.
- Finding 13 (plan 2701–2703): role names are quoted or guarded in printed
  commands.
- Finding 14 (plan 3333–3336, 3369; details 355): a revoked agent shows no
  level or wanted task; a disconnected agent's reconnect line is by name.
- Finding 15 (plan 3362–3369): the waiting list requires a connected agent
  that may still start the task.
- Finding 17 (Risks 3213): a bind is refused likewise when the role would
  not fit, and `--no-role` lets it through.
- Finding 18 (plan 2605–2607): a repeated join after admission says the
  admitted name stays.
- Finding 21 (plan 561–564, 1850): refusals name the scope asked and the
  verdict given.
- Finding 22 (plan 3302–3305): on the hosting computer the person's voice
  says "you".
- Finding 28 (plan 2729–2740; build notes departure 8): members list in
  admission order and a shared-name refusal marks the later one.
- Finding 29 (plan 2738, details 469): member key prefixes grow when shared.
- Finding 30 (build notes "byte for byte"): the asked name now shows; the
  claim is true at `f5b82e2`, not at `9c337df`.
- Finding 31 (plan 2743–2746): goal status gives a `role give` line per
  unheld member.
- Finding 34 (plan 3414–3417): the person's wording keeps the words a
  command put around the refusal.
- Finding 39 (details table row): the end-to-end owner refusal test is
  `an_owner_reads_a_refusal_in_the_persons_voice_and_json_keeps_the_daemons`
  in [CLI assertions](../crates/locust/tests/cli.rs).
- Finding 40 (details check table 366–384, pending-view row): the text-mode
  pending and watch test is
  `pending_in_text_mode_names_the_attempting_member_and_the_tasks_title` in
  [real daemon tests](../crates/locust/tests/t2_flow.rs).
- Finding 47 (plan 2812–2816): the anchor bound on reviews, checks and
  declarations is Rust-only evidence, as the property map now says.
