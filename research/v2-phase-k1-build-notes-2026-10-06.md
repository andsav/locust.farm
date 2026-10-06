# Locust v2 phase K1 build notes: a goal's governance has its own key

Status: phase K1 implemented and verified, 6 October 2026. This note records
the implementation of only phase K1 of the
[host safety and ending plan](../docs/host-safety-and-ending-plan.md),
against the current checkout, with the owner's principles from the
[master plan](../docs/master-plan.md) and the phase 1 to 3 state recorded in
the [phases 1–3 build notes](v2-phases-1-3-build-notes-2026-10-06.md). No
phase after K1 is in this work, and nothing of the roles plan's phase 4.

## Starting point

The checkout started clean at `e1e3e0d`. The plan files are owned by another
session and are not edited here; every departure from them is listed below.
The order the plan asks for was kept: the model under `research/tla` was
changed and run first, then the protocol crate, then the core, then the
command line, then the tests, recipes, scripts and site.

## The model, written first

[Organization.tla](tla/Organization.tla) now treats identity 0 as the goal's
governance key and identity 5 as the host's agent: record 2 of the founding
transcript admits identity 5 instead of identity 0, and no other id, position
or anchor moved, so no existing scenario was renumbered. Two scenarios were
added with configurations under [tla/configs](tla/configs) and rows in
[cases.json](tla/cases.json):

- `governance-key-work`: a contribution and a review signed by identity 0.
  The invariant `GovernanceKeyIsNoMember` says no record in `view.ordinary`
  or `view.selected` is identity 0's, in every reachable state (256 distinct
  states).
- `host-agent-fork`: two records at position 0 of identity 5's log, then an
  admission by identity 0. The invariant `HostAgentForkCostsGovernanceNothing`
  says every held governance record stays in `view.governance` (512 distinct
  states).

The plan names the scenarios and their claims but no invariants; the two
names above are this note's. [organization.md](tla/organization.md) gained a
row for the claim and says which identity governs and which is the host's
agent. `python3 scripts/check_tla.py --suite organization` was run before
chain.rs was touched (35 cases matched their expectation, run
`output/tla/runs/20261006T181440Z-c1f2f20e`) and again at the end (below).

## What was built

**Protocol** (`crates/locust-proto`). `PROTOCOL_VERSION` is 7; a store or a
peer from before this phase is refused as unsupported, which the unchanged
tests `incompatible_event_protocol_refuses_open_before_collecting_or_rewriting_state`
and `another_protocol_version_is_refused` check. `Genesis` is
`{ governance, host, definition, salt }`; `Header::check` refuses a first
record whose author is not `governance` or whose `host` equals it, and the
goal identifier commits to both keys. `Body::host_may_sign` is the one list
of kinds the key signs: governance and `EffectMaterialized`. `GoalStatus`
gained `governance`, `hosted_here` and an optional `host`; `ContextBrief.host`
and `Abilities.host` are options; `EventView` and `TaskView` gained
`by_host`. One request was added, `agent.reconnect` (owner, not a tool,
answered `Done`). The testkit's `Author::genesis` and `found_goal` take the
host's agent's key, and the frozen vectors were regenerated.

**Core** (`crates/locust-core`). The goal's key is store record `K` in
`Space::Goal`, built into `Local.governance`; nothing shows or exports it.
`Node::key_for` picks it for the key's public key on the hosting daemon,
`next_place` skips the member and `Local.part` tests for it, and
`Node::author_alone` (no text, clock field 0) is the one way a record is
signed with nobody present: `plan_join` and `drive_flow` call it and nothing
else does. `Chain::build` takes the key and the host's agent from the first
record; an admission of the key and a removal of the host's agent are
excluded with a position and a snapshot; `authorize_base` excludes anything
else the key signs as `NotAMember`. The stage runner is the key. `hosts` and
`host` in access.rs read `Local.governance`. `plan_join` keeps the retry
answer for an admission already committed ahead of every test and of any
signature, then tests that this computer hosts the goal, expiry, the ticket's
governance against `state().governance`, that the request does not name the
key, and membership. The farm's `eligible` leaves the key out of the authors
whose consent is required and `project` keeps its records in the page's list
of changes. `agent_reconnect` clears the flag `agent_revoke` set and touches
the goals the agent is in. Levels report `Abilities.host` as the host's
agent; the key's one stall is `Halted`. The stage check in
[validation.rs](../crates/locust-core/src/organization/validation.rs)
reports `selector_scope` for `task_creator` in the rules a stage's task gets,
in the `by` of an independent start, the `to` of an offered start and any
completion criterion, alone or inside an `any`; replay runs it on every
binding, so such a binding is `InvalidDefinition` on every computer.

**Command line** (`crates/locust`). `goal add` refuses before any plan on a
computer that does not host the goal; `member remove` naming the host's
agent refuses before any plan with the daemon's own sentence; `agent revoke`
has no plan and no confirmation and prints its `Undo:` line last; `agent
reconnect` is new; `workspace init` refuses before any plan when the goal is
hosted elsewhere or the host's agent is disconnected, and names `agent
reconnect`. Records the key signed print `host`; the `Host:` line prints
`you`, the agent's label, or `on another computer`; the ticket review prints
no key, and no rendered text prints the key or the words governance key.

**Sync**, not in the plan's text for this phase. The responder and the
initiator now send the governance key's log before any other author's
(`Replica::first_author` and `governance_first` in
[outbox.rs](../crates/locust-core/src/sync/outbox.rs)); the frontier frame
itself keeps its ascending order. The reason: `screen` drops every record of
an author whose admission is not held yet, and before this phase the host's
one key carried the admissions and the host's work in one log. With two keys
the host's agent's work sorted before the governance log about half the time
and was dropped on a join's first exchange, to arrive at the next one. The
node test `joining_fetches_founding_text_and_key_before_bulk_history_content`
found it. The [joinable farms plan](../docs/joinable-farms-plan.md) asks for
the same change under the same names in a later phase ("The governance
key's log first"); its test `a_frontier_answer_starts_with_the_governance_keys_log`
is written here, in the sync machine tests, with member keys that sort before
and after the governance key. Its second test,
`a_joiner_holds_the_members_and_rules_after_the_first_frame_of_records`, is
left to that phase.

**Documentation, scripts and site.** `versions.protocol` in
[site.json](../docs/site.json) is 7, as are the three script harnesses that
pin API 7 and the protocol, and the runtime reference. The stage check is
mirrored in the site's [rules.ts](../sites/locust.farm/src/lib/formation-editor/contract/rules.ts),
with the case `rules/selector_scope-stage-task-creator` in
[organization.cases.json](../docs/reference/conformance/organization.cases.json)
and one clause in the selector table of [formations.md](../docs/formations.md).
`python3 scripts/check_formations.py --write` regenerated the runtime contract
and the conformance vectors. No recipe under `docs/guide` and no script calls
`agent revoke`, reads a host fingerprint or binds a stage whose task names
`task_creator`, so none needed rewriting; the guide's sentences that said the
host signs stage steps or that a ticket holds the host's key now say the
goal's key.

## Departures from the plan, and corrections of it

- `goal add` on a computer that does not host the goal refuses with
  `denied` (exit 3): `this goal is hosted on another computer; request an
  invitation from its host`. The plan says only that it refuses before its
  plan; `denied` is the code the daemon's `host()` and the plan's own E1
  example use for a goal hosted elsewhere.
- `goal leave` naming the host's agent refuses before any plan with the
  daemon's sentence, `the host's agent cannot leave its own goal`
  (`conflict`, exit 7). The plan's text names only `member remove` for a
  refusal before the plan, but its exit criterion says `goal leave --agent
  maple` exits 7, and with the plan shown first it exited 0; the daemon's
  own refusal stays.
- `workspace init` refuses before any plan on a computer that does not host
  the goal (`denied`, the daemon's sentence), also when the goal's first
  record is held and `host` is known. The plan asks only for the refusal
  while the host's agent is disconnected; before this phase the command
  showed a plan the daemon then refused.
- `rules bind` with a formation whose stage's task names `task_creator` is
  refused by the daemon with the generic sentence every invalid formation
  gets, `the formation is invalid; validate it for diagnostics` (`invalid`,
  exit 6), and `formation validate` prints the `selector_scope` diagnostic.
  The command shows its plan first, as it does for every invalid formation;
  validating before the plan would be a change for every formation command
  and is not made here.
- `agent revoke` always sends `agent.revoke` and always prints its `Undo:`
  line, also for an agent that is already disconnected (the daemon answers
  `Done` and writes nothing). Its JSON keeps `hosted_goals`, the goals in
  which the agent is the host's agent, because the plan says the command still
  reads them and E2 decides what to print for them.
- The ticket review's signature line reads `verified against the goal's key,
  which the identifier commits to.` in place of the removed host fingerprint.
- `plan_join` signs through `author_alone`, so an admission carries clock
  field 0 rather than the inviter's clock; the plan asks for exactly that, and
  it is noted because it changes the bytes of every admission.
- The goal fixture is split as the plan asks (`Fixture.governance` signs
  governance, `Fixture.admin` is the host's agent, `Fixture::host` routes by
  `host_may_sign`), with one addition: `Fixture::founded` builds a goal
  without the validity run that `Fixture::new` makes, for tests that forge
  records the chain refuses.
- In the two `acceptances_by_the_hosts_agent_*` tests, both acceptances at
  one log position are `Disputed` at once (the fork is in the agent's own
  log, which is cut at the forked position), not pending; the admission that
  follows stays effective and the governance key can sign next, as the plan
  says.
- The sync ordering change above was not in K1's text.
- `organizationRuntime` in [availability.json](../docs/reference/availability.json)
  said `api-6-protocol-6` since phase 1 raised the API to 7; it now says
  `api-7-protocol-7`. Nothing else in that file was touched.
- The node test `encoded_two_daemon_workspace_edit_retains_files_and_converges_after_both_restart`
  relied on a child object's hash sorting before its manifest's by accident;
  the extra keypair moved the deterministic draws and the order flipped. It
  now varies the file's last byte until the order holds, and its assertion of
  the order is gone because the construction guarantees it.

The tests under `node/tests`, `replica_tests.rs`, `tests/organizations.rs`
and the `locust` crate were adapted along the plan's list of renames, with
these differences from its text:

- `Keypair` is not `Clone`, so the key is rebuilt from its seed where a test
  needs it; the helpers `governance_key(&Daemon, GoalId)` and
  `author_of(&Daemon, &EventId)` in
  [authorization.rs](../crates/locust-core/src/node/tests/authorization.rs)
  are shared by the delivery and farm tests.
- `admission_stops_when_the_host_agent_is_revoked` is deleted; its one
  replacement is `admission_continues_when_the_hosts_agent_is_revoked` in
  invitations.rs, not a second copy in authorization.rs.
- `goal.create` refuses a disconnected agent, so no stage can open "at
  creation" with the host's agent disconnected. The integration test
  `stage_steps_are_signed_while_the_hosts_agent_is_disconnected` creates the
  goal, revokes the agent, then binds `pipeline`; the stage step is signed
  by the key with the agent disconnected, which is the claim.
- Under a governance halt `goal.invite` answers `Halted` (its own check) and
  `member.remove`, which signs through `next_place`, answers `Unavailable`.
  The plan gives no code; the tests record both.
- The host's agent's key cannot produce a ticket at all: `Invitation::sign`
  refuses it with `InvalidSignature` because it does not match the ticket's
  `governance`. The test asserts that error rather than a refused join.
- An agent operation sent `on_behalf` of the key is `not_found`; an owner
  operation that carries an `agent` field (`goal.leave`, `level.set`,
  `agent.revoke`, `agent.reconnect`) gives `not_found` for the key sent
  directly, and any owner or host operation sent `on_behalf` of anything is
  `invalid`, a rule from before this phase. `no_credential_and_no_on_behalf_reaches_the_governance_key`
  checks every `Audience::Host` row of `OPERATIONS`, so a new host operation
  fails it until covered.
- `goal_status_reports_stalled_effects` does not exist; the closest test,
  `goal_status_reports_each_stalled_runner_condition` in context_views.rs,
  passes unchanged, and `stalled()` already stalls a governance-run effect
  only by a halt.
- A stage step's `FarmChange.task` is `None` (its body's context is the
  goal), so the page lists the step as a change by the goal without a task
  link; `the_page_keeps_changes_signed_by_the_governance_key` records that.
- A member's post under the coordinator formation of `setup()` adds a
  governance-signed stage step beside the contribution, so the tests count
  "no record added" around the revoke and the reconnect only.
- `restored_host_signing_before_peer_recovery_forks_but_recovery_first_extends`
  became `a_restored_hosts_agent_forks_only_its_own_log`; the durable test
  `sqlite_older_directory_signs_at_used_host_position_unless_later_events_are_recovered_first`
  became two, one where the agent's own log forks and governance goes on,
  one where a lost governance record halts governance.
- `agent reconnect` for an agent that is not disconnected reads `status`,
  sends no `agent.reconnect` and prints `NAME is not disconnected. Nothing
  changed.`; the plan's sentence is the same and its "sends one
  `agent.reconnect`" describes the disconnected case.

## Checks and evidence

All of the plan's exit-criteria checks were run on the final tree, with
`/opt/homebrew/bin/python3` for the scripts:

- `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets
  -- -D warnings` and `cargo test --locked --workspace` pass: 0 failures.
  Per crate: `locust` 219 unit tests (3 ignored, all from before this
  phase) and 48 + 8 + 2 + 12 + 2 + 5 + 3 + 15 integration tests;
  `locust-core` 350 lib tests (3 ignored, the `sim` characterizations from
  before this phase), 8 organizations, 1 subgroups; `locust-proto` 137;
  `locust-store` 34 + 2 (3 ignored, as before); `locust-net` 26 (2 ignored,
  as before); `locust-farm` 39; `locust-workspace` 33 + 38; `locust-adapter`
  18 + 7 + 17 (2 ignored, as before). No `#[ignore]` was added or removed. Neither
  `repeated_explicit_signals_reach_retained_child` nor
  `a_diverged_author_log_reconciles_between_two_real_daemons` failed in the
  full run, so neither needed a rerun alone.
- `python3 scripts/check_tla.py --suite organization`: 35 cases matched
  their expectation, run `output/tla/runs/20261006T193853Z-beb66289`;
  `organization-governance-key-work` 256 distinct states in 5.1 s,
  `organization-host-agent-fork` 512 distinct states in 12.5 s.
- `python3 scripts/check_formations.py` (verify mode): exports verified, 6
  examples validate, the conformance vectors cover every diagnostic code.
- `python3 scripts/check_docs.py`: passed, with the new note and its index
  entry staged.
- `python3 scripts/check_documentation.py --binary target/debug/locust
  --timeout 60`: `shared-workspace-loop`, `local-collaboration`,
  `private-authoring` and `separate-goal-export` pass.
- `python3 -m unittest discover -s scripts/tests`: 300 tests, OK, 3 skipped
  (the same three as before this phase).
- In `sites/locust.farm`: `npm run lint`, `npm run check` (0 errors, 0
  warnings), `npm test` (209 pass, 0 fail) and `npm run build` (89
  prerendered routes checked) pass.
- `locust --json contract` reports `protocol_version` 7 and `api_version` 7.
- `git grep -n 'host agent is disconnected' -- crates` and `git grep -n
  'freeze for everyone' -- crates` find nothing; `git grep -n 'author_alone('
  -- crates/locust-core/src/node` finds the definition in authoring.rs and
  the calls in flow.rs and peers.rs only.

The fresh-home walkthrough of the exit criteria was run by hand against a
real daemon (`LOCUST_RELAY=none LOCUST_LOOKUP=none LOCUST_BIND=127.0.0.1:0`,
home under `output/`), with `maple` and `juniper` enrolled:

- `goal create --title "Parser cleanup" --agent maple`: `goal status --json`
  gives a `governance` key that is none of `members[].member` and none of
  the agents of `status --json`, `hosted_here` true and `host` maple's key;
  `events` lists `genesis`, `member_admitted` and `rules_bound` by `host`.
  `goal leave --agent maple` exits 7 `conflict` before any plan.
- `agent revoke --agent maple` shows no plan, prints `maple is disconnected.
  The name stays taken.` and then `Undo: locust --owner agent reconnect
  --agent maple`. `goal invite`, `goal add --agent juniper` and `rules bind
  --formation peer-review` succeed and their records are by `host`; `member
  remove --member maple` and `workspace init --empty` exit 7 `conflict`
  before any plan, with and without `--plan`, and `events` is unchanged.
- The `Undo:` line as printed connects maple again; `workspace init --empty`
  shows its plan; maple's `contribution publish` succeeds under the key it
  had, and `events` shows no record for the revoke or the reconnect.
- `rules bind --formation-json` with the pipeline example whose `draft`
  task's completion is a declaration by `task_creator` is refused at confirm
  (`invalid`, exit 6); `formation validate` prints the one `selector_scope`
  diagnostic at `/flow/draft`; `events` shows no new record.
- `goal create --title "Docs pass" --agent juniper`, `agent revoke --agent
  juniper`, then `rules bind --formation pipeline`: the first stage's task
  opens and `events` shows its `effect_materialized` by `host`; `board`
  lists it `By host`.
- `coordinator` cannot be bound without a role binding (`conflict: declared
  role is not bound`), which is from before this phase; `peer-review` was
  used for the step above.

## Left open

- GUARD (G1) is needed before anything is released: a daemon started on a
  copy of the data directory is a second host and two hosts that both sign
  halt governance for good. Nothing in this phase prevents that.
- The restore tests in delivery.rs and durable_tests.rs state what the code
  does between this phase and G1; G1 rewrites what they expect.
- `a_joiner_holds_the_members_and_rules_after_the_first_frame_of_records`
  belongs to the joinable farms plan's phase with the other sync changes.
