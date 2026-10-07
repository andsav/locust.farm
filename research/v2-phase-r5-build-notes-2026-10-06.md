# Locust v2 phase R5 build notes: the one view and refusals

Status: implemented, 6 October 2026; source, contract, recipe, script and
site checks pass. This note covers **Phase 5: The one view and refusals** of
the [roles plan](../docs/roles-and-permissions-plan.md) and its
[companion](../docs/roles-and-permissions-plan-details.md), and the ten
findings of the [R4 review](v2-phase-r4-review-2026-10-06.md) marked
"phase 6 (R5)". Phase 6 (documents, site and scripts) is not part of this
work: the guide pages it owns still carry their Phase 4 prose. The starting
point was `0ed4dc5`, after the [R4 build](v2-phase-r4-build-notes-2026-10-06.md)
and the review's plan corrections. The protected plan files are owned by
another session and were not edited. That session committed its
[R4 review fixes](v2-phase-r4-fixes-2026-10-06.md) (`726de21` to `ec1e078`)
while this work was under way; this build's files were kept apart from
its files and rebased on them at commit time by re-running every check.

## Implemented behavior and its checks

### One wording for every refusal

- [level.rs](../crates/locust-proto/src/api/level.rs) now holds the whole
  wording of a refusal. `Voice::{Person, Agent}` chooses the reader.
  `render(&Refused, Voice)` gives one sentence per side: the level side names
  the agent, says "this task" and gives the `allow` or `level` line; the
  rules side says what the rule asks for and, for the person, which role or
  which members; the state side names the state; only-you says whose command
  it is and names the command's words, never the dotted operation. The
  agent's voice quotes nothing another member wrote: no title, member name,
  host name or role name reaches a message; they ride in `details`. The
  person's voice quotes titles through `safe`, escaped and cut.
  `short(id, others)` cuts an identifier to its shortest unique prefix of at
  least eight characters, keeping a `task:` or `effect:` tag.
  `allow_command` and `level_command` print the one line that settles a
  wait; `shell_word` quotes a local name only when it is not plain, so the
  line runs as printed and the mockups' names stay bare. `safe` moved here
  from the CLI and also escapes the default-ignorable code points the review
  listed (U+00AD, U+034F, U+061C, U+115F–1160, U+180B–180F, U+3164,
  U+FE00–FE0F, U+FFA0, U+E0000–E0FFF). The ten tests the plan names pin the
  P5-2 lines byte for byte in both voices.
- The daemon's `ApiError.message` for a `Refused` is `render(Voice::Agent)`
  of its details: [access.rs](../crates/locust-core/src/node/access.rs) and
  [callers.rs](../crates/locust-core/src/node/callers.rs). The node test
  harness's `send` asserts this for every refused answer in the suite
  ([tests/mod.rs](../crates/locust-core/src/node/tests/mod.rs)). An agent
  that asks for `role give` or `role take` is refused with `GiveRole`, the
  act the plan names (review finding callers.rs:95).
- [failure.rs](../crates/locust/src/failure.rs): `Failure::for_person`
  decodes a refusal's details and rewrites the message in the person's
  voice. [cli/mod.rs](../crates/locust/src/cli/mod.rs) applies it only with
  `--owner` and without `--json`; a `--json` answer leaves as it came.
  `print_failure` appends `Role:` and `Roles here:` lines from a role
  refusal's details, so the person sees which role and which roles the goal
  has (review finding goals.rs:170);
  `a_role_refusal_names_the_role_and_the_goals_roles_for_the_person` in
  [cli.rs](../crates/locust/tests/cli.rs) pins the output with a
  bidi-override in the role name escaped.

### The one view

- [api.rs](../crates/locust-proto/src/api.rs): `DaemonStatus.waiting` lists
  `WaitingForYou` entries, each with its goal, agent, `WaitingKind` and the
  complete command. `GoalSummary` gains the agent's name in the goal, the
  host's name, and the open invitation count and latest expiry. The
  summaries of `status`, `goal.status`, `attempt.start`, `attempt.takeover`,
  `review.record`, `pending` and `wait` are sentences, and the contract
  exports `refusal_schema`. [runtime.contract.json](../docs/reference/generated/runtime.contract.json)
  was regenerated with `check_formations.py --write`.
- [views.rs](../crates/locust-core/src/node/views.rs): `goal_summaries`
  counts open invitations in one scan of the invite space, only for the
  owner and only on goals this daemon hosts; `waiting_for` gives one
  `AllowTask` per task an agent wanted while its level is below auto,
  oldest first, with goal and task cut among what the daemon holds. A
  joining agent and a halted goal give no entry. `wants_review` lists a
  result only while its task's round is open (review finding views.rs:419),
  and pending counts come from one `latest_reviews` call for all subjects
  (review finding mod.rs:349). `Goal::role_holders` is gone (review finding
  mod.rs:337). [daemon.rs](../crates/locust-core/src/node/tests/daemon.rs)
  tests the waiting list with its ready command, the halted and joining
  exclusions, and the owner-only invitation counts;
  [lifecycle.rs](../crates/locust-core/src/node/tests/lifecycle.rs) tests
  that a closed, picked or revised task asks for no review.
- [presentation.rs](../crates/locust/src/cli/presentation.rs) was rewritten
  around a `Reader`: the voice, the principal, the local names, the goals,
  members and tasks the daemon holds, and the clock. Every printed command
  cuts identifiers through `short`; every member is named through
  `member_label`, which prints a chosen name bare when plain and quoted and
  escaped otherwise (review finding event.rs:1011, first part). `status`
  prints mockup P5-1 byte for byte: "Waiting for you" first, then each goal
  with each agent's name, roles, level and standing, the invitation count,
  expiry and revoke line on a hosted goal, the joining sentence under its
  goal, the agents in no goal and the daemon line with eight characters of
  the endpoint. An agent reads the same view about its owner. `pending`
  names who is attempting each task and counts approvals on each result;
  the heading of a task that waits for its agent's owner is "Waits for
  {owner}: TASK" with the allow line. `counts_clause` brackets an `Any`
  inside an `All` (review finding presentation.rs:81). `goal status` says
  members become reviewers only to the person who hosts the goal (review
  finding presentation.rs:618) and marks a host name that comes from the
  ticket as unconfirmed while the agent is still joining (review finding
  presentation.rs:188). `every_printed_command_parses_as_printed` now
  covers status, the person's rendering of each `Why` and the help line of
  every `Owner` and `Host` operation.
- [cli/mod.rs](../crates/locust/src/cli/mod.rs) and
  [watch.rs](../crates/locust/src/cli/watch.rs) pre-read `status`, and the
  goal's members and board where a view names them, only outside `--json`.
  [only_you.rs](../crates/locust/src/cli/only_you.rs) prints the `Undo:`
  lines of `level` and `allow` through `level_command` and `allow_command`
  with cut identifiers, and the standing line in the person's voice.
  [roles.rs](../crates/locust/src/cli/roles.rs) names members through the
  same label and no longer trims the label's last character to annotate the
  host's agent.
- [mcp.rs](../crates/locust/src/mcp.rs): `INSTRUCTIONS` says to start with
  `locust_status`, names the three sides of a refusal, and says never to run
  a command with `--owner` unless the owner asks in the chat.
  [mcp/tests.rs](../crates/locust/src/mcp/tests.rs) answers a `Refused` from
  the fake daemon and checks code, message, `details.why.side` and the
  rendered content; the strings test refuses the old words in `INSTRUCTIONS`
  and every description and expects the pending, wait and review sentences.
  [SKILL.md](../skills/locust/SKILL.md) tells an agent what to do on each
  code, names `locust --owner goal join` and `locust --owner level` beside
  `--plan` and `--confirm`, and says `details` are material.
- [t2_flow.rs](../crates/locust/tests/t2_flow.rs):
  `a_refused_task_reaches_the_person_as_a_waiting_line_they_can_run` starts
  a task at ask through the agent, reads the refusal, finds the line under
  "Waiting for you", runs it as printed, sees the `Undo:` line, starts
  again and reads "Nothing is waiting for you." in both voices.

### Verification

`cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets
-- -D warnings` and `cargo test --locked --workspace` (1145 tests) pass.
`python3 scripts/check_formations.py` verifies the regenerated exports.
`scripts/check_documentation.py --binary target/debug/locust --timeout 60`
passes the four recipes, and `python3 -m unittest discover -s scripts/tests`
passes 297 tests (3 skipped) under Python 3.12; the shell's default Python
3.10 lacks `hashlib.file_digest`, which 25 of those tests use, so the run
was repeated with `/opt/homebrew/bin/python3.12`. In `sites/locust.farm`,
`npm run lint`, `npm run check`, `npm test` and `npm run build` pass. The
exit searches `rg -i 'grant|authoriz|administrator|participant|principal|viewer'
crates/locust/src/mcp.rs` and `rg authorization_required skills` find
nothing.

## Departures and readings of the plan

1. `Node::waiting_for` takes the summaries `status` already built instead of
   recomputing memberships and levels, so the two parts of the view cannot
   disagree about who is a member or at what level.
2. The plan's sentences for a rule's side ("the rule asks for ...") are
   inferred from mockup P5-2; the companion has no separate specification of
   each `Rule` variant's words. The tests pin the mockup lines and one
   sentence per variant.
3. Invitation open-ness is computed inline in `goal_summaries` from the
   invite record's redeemed, revoked and expiry fields rather than through a
   shared predicate, since the invitation list already reads those fields
   the same way.
4. `safe` lives in `locust_proto::api::level` and is re-exported from the
   CLI's presentation module, because the daemon's agent-voice rendering of
   a person's title and the CLI's person-voice rendering must escape the
   same code points.
5. `WaitingKind` is externally tagged (`{"allow_task": {..}}`) and not
   internally tagged with `kind`: postcard, the binary frame encoding, cannot
   decode an internally tagged enum. The JSON shape in the contract follows.
6. Views take a `Reader` rather than the plan's `render(.., Voice)`
   signature. The voice alone cannot cut identifiers or name members; the
   reader carries the names, goals, members and tasks the CLI pre-reads,
   and the clock for the invitation expiry.
7. `allow_command` puts `--revoke` last, after `--agent`, so the allow line
   and its undo differ only by the trailing flag. The local name is quoted
   as one shell word when it is not plain, which the plan does not say but
   its "runs as printed" criterion needs; the mockups' names are plain and
   print bare. The former `undo_agent` helper in only_you.rs is replaced by
   that one function.
8. Review finding event.rs:1011, second part (join order): `safe` escapes
   the default-ignorable code points, and `member_label` quotes names that
   imitate the view's separators, but the admission position on
   `MemberView`, the join-ordered member list and the "later" mark in the
   shared-name refusal are not built here. They touch
   [goals.rs](../crates/locust-core/src/node/requests/goals.rs) and
   [selectors.rs](../crates/locust/src/cli/selectors.rs), which the
   concurrent R4-fixes session was rewriting, and the review says the plan
   must change with the code. The `is_member_name` comment the review asks
   to correct is in that session's file too and is left as it is.
9. In the agent's voice the host line reads "Host: NAME's owner" with the
   host agent's name in the goal, not "your owner", because an agent in a
   goal hosted elsewhere reads the same line as one hosted here. While the
   host's record is not yet held, `host_line` prints the ticket's name
   through `chosen_name` and says the record has not arrived.
10. Fixture names differ from the companion where the K1 and R4 builds had
    already placed the helpers: the fake daemon in cli.rs answers
    `GoalStatus` and `Board` so the views can cut identifiers, the one in
    mcp/tests.rs answers a `level_required` refusal, and the formations
    test asserts `refusal_schema.title == "Refused"` and the
    `Why` definition rather than a separate schema file.
11. The person's `Role:` and `Roles here:` lines are printed by the CLI
    from the refusal's details; the daemon's message stays generic, as the
    R4 build decided for untrusted text in error sentences.
12. The clippy `nonminimal_bool` and `too_many_arguments` lints that the
    pinned toolchain raises were answered by `is_none_or` and by folding a
    test helper's goal/title and host/hosted-here arguments into pairs.
