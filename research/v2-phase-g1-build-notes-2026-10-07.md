# Locust v2 G1: a daemon knows what it signed

Status: implemented; the required checks and the drills passed. This is
build-order phase 8, phase G1 of the
[host safety and ending plan](../docs/host-safety-and-ending-plan.md) (section
"G1: A daemon knows what it signed"), with its
[companion](../docs/host-safety-and-ending-plan-details.md). The model it
implements is [the restore guard model](restore-guard-model-2026-10-06.md); the
file-identity measurements it relies on are
[restore-file-identity-2026-10-06.md](restore-file-identity-2026-10-06.md). The
preceding record is [R6](v2-phase-r6-build-notes-2026-10-06.md). The plans were
read, not edited. G2 owns the words a person reads; the words here are interim.

## What changed

- **Marks file.** Each data directory has a marks directory beside it, never
  inside it: `<home>.marks`, named by `marks_dir` and `MARKS_SUFFIX` in
  [local.rs](../crates/locust-proto/src/local.rs). It holds, per goal and key,
  the last record that key signed and whether the goal was ever shared
  (`Mark`, `MarkWrite`, `Marks` and `FileId` in
  [store.rs](../crates/locust-proto/src/store.rs)). `SqliteStore::open(dir,
  marks)` writes a commit's marks after its database transaction and syncs
  them before `commit` returns, so no record leaves before its mark is on
  disk ([marks.rs](../crates/locust-store/src/marks.rs)). A marks file copied
  from elsewhere, torn or put back from an older copy reads as lost. The
  memory store carries its marks in a separate cell, so tests can keep or lose
  them.
- **Guard.** [guard.rs](../crates/locust-core/src/node/guard.rs) holds the two
  tables of the plan. `guard_start` (called by `Node::open` before its last
  loop) decides what the start is from two facts, marks kept or lost and the
  database file last used or another, and writes, in one commit synced before
  any exchange: a `RESTORED` record per restored goal (revoking pending
  invitations where this daemon hosts), marks from the store's tips where they
  are lost or short, the `RESTORED` record deleted where an ordinary start
  holds nothing, and the file identity. `Node::hold` answers for one key in one
  goal: `Behind` (the marked record is not in the key's usable log), the
  governance key's hold for an agent on the host's computer, `Unheard` (copy of
  unknown age) or `Admitted` (just admitted on a member's computer).
  `guard_settle` ends holds by the second table and lowers a mark where the
  table gives its record up.
- **Gates.** Every gate reads `Node::hold` and adds no condition of its own:
  `next_place` in [authoring.rs](../crates/locust-core/src/node/authoring.rs)
  refuses with `read_only` (`Behind`, `Unheard`) or `unavailable`
  (`Admitted`), and then answers `halted` for an agent whose own log is
  forked; `drive_flow` in [flow.rs](../crates/locust-core/src/node/flow.rs)
  passes over a held step; `plan_join` in
  [peers.rs](../crates/locust-core/src/node/peers.rs) answers
  `Refusal::CatchingUp` while the governance key is held, so a join is asked
  again rather than refused for good.
- **Sync.** An exchange counts as hearing a computer only when it brought
  nothing new: `Initiator::reconciled` and the accepted side's check before
  `Done` call `Host::reconciled`, which the node maps to `guard_heard`
  ([initiator.rs](../crates/locust-core/src/sync/initiator.rs),
  [driver.rs](../crates/locust-core/src/sync/driver.rs)). A caller that is not a
  member of a goal whose governance key is held here is remembered
  (`Host::note_caller`, at most eight per goal) and dialed back, since it may
  hold the host's later records; callers are forgotten when the hold ends.
- **Requests and views.** `goal.continue` (owner only) deletes the goal's
  `UNHEARD` records, clears `unheard` in its `RESTORED` record and lowers every
  mark that is ahead; it answers the number of keys released.
  `GoalStatus` and `GoalSummary` gain `guard` (key, `by_host`, reason, heard,
  waiting) and `restored` (invitations revoked); `halted` gains
  `signer_recovery` and `signer_conflict`
  ([api/guard.rs](../crates/locust-proto/src/api/guard.rs),
  [goals.rs](../crates/locust-core/src/node/requests/goals.rs)). The generated
  [runtime contract](../docs/reference/generated/runtime.contract.json) is
  regenerated; the generic `locust --owner goal continue --goal <id>` and
  `locust --owner call goal.continue` reach it.
- **Removed.** S1a's dead `Commit::drop_blobs` path and its tests.

## Departures and precise readings of the plan

1. The marks file is replaced lazily. A marks file that reads as lost is
   replaced at the first commit that carries marks (`marks.tmp`, rename, and a
   sync of the directory), not at open: a crash between a replace at open and
   the first commit would make lost marks read as kept and empty.
   `creating_the_marks_file_syncs_its_directory` checks the first commit. A
   commit that only clears marks, on lost marks, creates an empty valid file.
   A crash in the middle of the positional writes leaves marks behind the
   store, never ahead.
2. `marks_dir` treats a trailing `/` or `/.` as the directory itself; for `/`
   or a path ending in `..` it appends the suffix to the raw path.
3. The plan counts 25 callers of `SqliteStore::open`; there are fewer. Every
   one passes a marks directory.
4. A store that holds no goal and has no file record is new. Marks found beside
   it are kept: they hold the old directory's keys if it is put back from an
   older copy ([review](v2-phase-g1-review-2026-10-07.md) finding 1;
   `a_start_on_an_empty_home_keeps_the_marks_a_later_restore_needs`). The
   guard reads only the marks of keys this daemon signs with, so they hold no
   key of the new store and are never lowered. A goal the store does not hold
   at all, or that already holds its `RESTORED` record, is left out of the
   overwritten-in-place test, so its marks do not make every later start look
   overwritten (finding 5).
5. With the marks lost and a goal still holding its `RESTORED` record, the
   goal's keys become unheard as the plan says, and its marks are also
   rewritten from the store's tips; the unheard hold covers the goal until it
   ends.
6. `goal.continue` lowers each mark that is ahead to the end of its key's
   usable log, not to "the tip the store holds". With a gap the tip is not in
   the usable log, so lowering to it would release nothing.
7. `guard_heard` settles on every hearing, not only the first from a computer:
   a rule can be met by records that arrived since. An admission that lands
   forgets whom the daemon heard from in that goal (`Guard::unhear`), so the
   admitted key waits for a hearing after it.
8. Callers are forgotten in `Node::land` for every goal a commit touches once
   its hold has ended, not only on a hearing. Found by
   `callers_are_bounded_and_forgotten_when_the_hold_ends`: `goal.continue` and
   records that end the hold left up to eight callers remembered.
9. A forked log is never `Behind`: its usable prefix ends at the fork, and the
   key signs nothing there anyway. `local_keys` also counts keys a local
   principal holds that have a mark in the goal.
10. The dial-back to an unknown caller sends `Hello`, an empty `Frontier` and
    `Refused(NotAMember)`, not `Hello` alone; none carries a record
    (`an_unknown_caller_is_dialed_back_with_an_empty_frontier` asserts this).
11. The hold check in `drive_flow` is a backstop. `drive_flow` already turned
    a failed signature into a failed effect, and `next_place` refuses a held
    key; with the check removed the delivery tests still passed. It stays
    because the plan names it a gate and it keeps a held step out of the
    failure list.
12. A rejoined key with its marks kept is first `Behind`, then `Admitted`
    (`finish_joins` writes `UNHEARD`), so its hold lasts until the host's
    computer is heard.
13. After a copy of unknown age on the host's computer the host's agent has its
    own `unheard` view beside the governance key's, and `goal.continue` counts
    both. This follows the first table ("every key this daemon holds is
    unheard"); the sentence that an agent held through the governance key has
    no view of its own applies to the `Behind` case, and holds there.
14. `goal.continue` sent by the owner on behalf of an agent answers `invalid`
    by the general owner-only rule; an agent's own credential gets `denied`.
15. A member removed after the copy usually never receives its own removal. It
    still counts itself a member and sends everything it holds, so it "brings
    nothing" only when it holds nothing newer than the copy. The second half
    of `a_copy_of_unknown_age_on_the_hosts_computer_waits_for_the_person`
    keeps that computer offline from the copy on.
16. K1's private-goal setup of the two `sqlite_older_directory_*` tests cannot
    produce a hold: in a goal never shared the missing record is given up at
    the start. Their replacement,
    `an_older_directory_is_held_until_its_later_events_return_for_an_agent_record`
    and `..._for_a_governance_record`, uses a second real daemon that is
    stopped. The copy is started pointed at the original's marks directory,
    since a copied marks file reads as lost. The events return by replay into
    the stopped store, as K1 did.
17. The admission hold survives a restart (`UNHEARD` is durable), and an
    exchange that brought records does not count, so in a busy goal it lasts
    longer than the plan's "about a second". Tests that act as a member right
    after a join wait for its `guard` to be empty;
    `two_real_daemons_join_claim_sync_large_payload_and_accept` failed once
    under load for this reason and now waits.
18. `Response` carries `#[allow(clippy::large_enum_variant)]`: `GoalStatus`
    grew 8 bytes past the lint's limit.
19. The plan's API test `operations_that_are_only_the_persons_are_never_tools`
    does not exist under that name. `goal.continue` is `Audience::Owner` with
    `tool: false` in `OPERATIONS`, and the MCP schema filters every owner and
    host operation out of agent tools
    ([schema.rs](../crates/locust/src/mcp/schema.rs)).
20. What a daemon heard is memory only, so the simulator's restart comparison
    clears `heard` and `waiting` from the views it compares.
21. A restore revokes pending invitations, so
    `an_admission_signed_twice_for_one_request_is_one_record` now continues
    after its restore before admitting.
22. The store crate's latency table names 4,380-byte headers while the example
    prints 4,349; the drift predates this phase.

## Findings outside this phase

- On the host's computer, an agent enrolled before the copy and admitted after
  it has no `Local.part` record in the copy. Once its admission and records
  return the guard is empty, but every request it makes answers `not_found`,
  because `Entry::membership` needs that record. This is a gap in restoring
  local records, not in the guard; `a_key_admitted_while_catching_up_is_held_with_the_rest`
  runs on a member's computer for this reason. Left for the owner.
- A member opening a task right after a restart can get `conflict` ("the
  candidate cannot be applied yet") until the content of records it already
  holds arrives. Refused candidates sign nothing; the real-daemon helper
  retries. Since `5e71dc7` a candidate that waits on the rules definition is
  refused as `unavailable` instead, saying the rules have not arrived.
- On a joined member `events` lists the first `rules_bound` twice. This is the
  feed's intended behaviour (it numbers standing changes,
  [feed.rs](../crates/locust-core/src/node/feed.rs)): the record is pending
  until its definition arrives. Reproduced with the binary from before the
  guard.
- The user guides do not mention the marks directory yet; G2 owns those words.

## Tests

- Store: nine tests in [tests.rs](../crates/locust-store/src/tests.rs), the
  ninth `a_marks_unheard_bit_survives_reopen_and_a_nonzero_reserved_byte_is_lost`
  for the bit the sweep's fix added, and
  `marks_follow_their_commit_and_survive_reopen` in the conformance module.
- Proto: `refusals_render_in_snake_case` gains `catching_up`,
  `the_published_names_and_modes_are_stable` the suffix, and
  `the_marks_directory_is_beside_the_home_and_never_inside_it` is new.
- [node/tests/guard.rs](../crates/locust-core/src/node/tests/guard.rs): 25
  tests on `Network`, the plan's 23 and two for the sweep's fix:
  `a_second_restore_before_the_first_is_caught_up_is_still_unheard` and
  `records_that_return_raise_the_mark`.
- [delivery.rs](../crates/locust-core/src/node/tests/delivery.rs): the four
  restored-host tests rewritten under the plan's names, and
  `a_stage_step_signed_twice_from_one_input_is_one_record`. Each caller of
  `snapshot` or `snapshot_all` says whether the marks were kept.
- [replica_tests.rs](../crates/locust-core/src/node/replica_tests.rs):
  `a_rejoined_key_signs_nothing_until_its_own_log_returns` and
  `a_newly_admitted_key_waits_for_the_hosts_computer_once`.
- [sync/tests/driver.rs](../crates/locust-core/src/sync/tests/driver.rs): the
  three hearing and caller tests.
- [durable_tests.rs](../crates/locust/src/daemon/durable_tests.rs): the two
  older-directory tests, `a_lost_marks_directory_is_an_ordinary_start_and_a_copy_of_both_is_not`,
  `a_database_overwritten_in_place_is_found_by_its_marks` and
  `lost_marks_while_a_restore_is_caught_up_make_the_goal_unheard`;
  [reconcile_tests.rs](../crates/locust/src/daemon/reconcile_tests.rs)
  continues before the restored member signs.

## Simulator

`Kind::Restored { m, marks }` in
[chaos.rs](../crates/locust-core/src/node/sim/chaos.rs) stops a machine and
puts one of its earlier backups back, with the marks kept (a replaced data
directory) or lost (a whole-computer restore);
[restore.rs](../crates/locust-core/src/node/sim/restore.rs) and
[restore_runs.rs](../crates/locust-core/src/node/sim/restore_runs.rs) hold
the fault and the kept runs. Backups and restores draw
from their own seeded stream, so runs without restores are unchanged;
`LOCUST_SIM_NO_RESTORES=1` turns restores off. A simulated owner sends
`goal.continue` on the host once its hold has heard from every machine that
holds the goal.

The invariant, in [check.rs](../crates/locust-core/src/node/sim/check.rs):
across all stores, one key at one position never holds two different records
(the model's `StoreNoFork`). It is claimed wherever the plan claims it and
nowhere else: a reuse signed after the owner continued, after a member's
machine was restored with its marks lost, or after a marks-kept restore in a
run whose member removal the copy does not hold is left out; the governance
key, and the host's agent with the marks lost, are always claimed. As built
here, a run that failed and had unclaimed reuses and no claimed one counted as
residual whatever made it fail, and shrinking ignored it; the review
(finding 8) narrowed that, below.

The first sweeps found two gaps inside the claims, and one guard bug:

- **Guard bug (fixed).** After a start with the marks lost, a key that had
  signed nothing got no marks write, so the marks stayed lost and every later
  start read the restore as still being caught up — on the host, a wait for
  the owner on every restart. `guard_start` now writes `MarkWrite::Clear` for
  such keys. Found by seed 431; the kept run
  `a_restart_after_a_copy_of_unknown_age_was_caught_up_holds_nothing` covers
  it.
- **A second restore before the first is caught up (fixed).** A start with
  the marks lost rewrites them from the copy's tips, so the fact that the
  copy's age was unknown lived only in the store's `RESTORED` record. A
  second restore with the marks kept, to a copy no newer than the first, then
  matched its store and signed at used positions — 19 of 10,000 seeds,
  including one where the goal's own key ended unheld beside a divergent
  copy. The marks now carry an `unheard` bit (byte 106 of the record), set
  while the goal is unheard and rewritten when the hold ends, and a start
  that reads it without a matching `RESTORED` record treats the goal as a
  copy of unknown age. A commit that lands a local key's own records also
  raises that key's mark to the key's tip, so a daemon that caught up and was
  restored again is `Behind`, not signing blind. One residual stands: a goal
  in which no local key has any record in the copy carries no mark, so the
  bit cannot be kept for it (on the host the governance key always has one;
  this is a member's computer whose agents signed nothing in the copy).
- **Two restores older than one admission (called residual 6 here, claim
  narrowed).** A member restored with its marks kept to a copy older than an
  admission gives a record up once it has heard from every computer its copy
  lists — but when the host's computer was itself restored from a copy that
  missed the same admission, no computer that answers knows the admitted
  member, which holds the given-up records. The plan as written at this
  phase did not accept this fork: its risk (5) listed three agent-key forks,
  none of them this one, and its simulator claim still covered it, so the
  exclusion in `host_missed_the_same_admission` narrowed a claim the plan
  made. After the [review](v2-phase-g1-review-2026-10-07.md) (findings 7
  and 10) the plan author accepted it as a stated limit: the plan now lists
  it as the fourth way of risk (5), and the master plan says what a person
  sees. The kept run
  `a_member_restored_with_its_marks_kept_gives_up_what_only_a_member_admitted_since_holds`
  ends in that reused position and stays outside the claims.

The plan's two kept runs of residual 5 stand beside these:
`a_member_restored_with_its_marks_lost_signs_again_what_only_another_member_holds`
and `a_member_restored_with_its_marks_kept_gives_up_what_only_a_removed_member_holds`.
All the kept runs are separate deterministic tests in
[restore_runs.rs](../crates/locust-core/src/node/sim/restore_runs.rs), built
step by step with fixed seeds and no injected faults; the seeded sweep never
runs them.

Seeded sweeps in a release build: seeds 0 to 10,000 in about a minute, no
failure inside the claims; 30 positions signed again outside them, across 14
residual runs of the sweep. Before the fixes: 21 failures and 11 residuals.

## After the review

The [review](v2-phase-g1-review-2026-10-07.md) of 7 October found two gaps in
the simulator's oracle (findings 7 and 8), corrected here:

- **The fourth case is narrowed.** `host_missed_the_same_admission` now needs
  a host restore whose copy lacks an admission the host held before that
  restore. A host copy taken before the admission existed no longer counts:
  the host learns of the admission again and answers for the member.
  `a_host_restore_from_before_an_admission_existed_leaves_the_claim_standing`
  in [restore_runs.rs](../crates/locust-core/src/node/sim/restore_runs.rs)
  covers it.
- **A residual is a run the fork explains.** A failing run counts as residual
  only when every invariant it breaks follows from the positions signed
  again outside the claims: the reused key's own halt, or records that stand
  on its reused records (and the deliveries of effects they materialize) not
  being effective ([check.rs](../crates/locust-core/src/node/sim/check.rs),
  `Violation` and `Forked`). A step that fails, a panic, a missing record or
  a hold that never ends beside such a reuse is a failure.
  `a_failure_beside_an_unclaimed_reuse_is_residual_only_while_the_fork_explains_it`
  covers it, and a replayed seed's report says when it is residual.

The sweep over seeds 0 to 10,000 in a release build, rerun with both: no
failure; 30 positions signed again outside the claims and none inside, across
the same 14 residual runs (241, 765, 1936, 2110, 3138, 3147, 3552, 5420,
5933, 6481, 7125, 7842, 8333, 8340). Each fails only at quiet, on the reused
agent's halt and on records standing on its reused ones. A first version of
the narrower rule left out acknowledgements of deliveries whose effects stood
on the fork, and reported four of these as failures (765, 7125, 8333, 8340).

Three defects found after the review fixes were fixed on 7 October
([follow-up defects](v2-phase-g1-review-fixes-2026-10-07.md#follow-up-defects)):

- **A fork after a second restore is a limit, not a guard bug (`3ae5cae`).**
  A member restored with its marks lost writes them again from the copy. If
  its hold then ends by hearing the host's computer alone, and that computer
  never had the agent's last record while another member's did, the agent
  signs over it: the first way of risk (5). The same happens when, before
  the agent signs, the data directory alone is put back again with the
  marks kept. The marks never named the record, so that start finds nothing
  missing. Nothing on the computer remembers the record, so the guard
  could hold it only by waiting for every member's computer after every copy
  of unknown age, which the second table does not ask. The guard is
  unchanged. The plan states the case under risk (5). The simulator's
  marks-lost exclusion now carries past a later marks-kept restore for a
  position the marks have not reached since, through `lost_with_the_marks`
  in [check.rs](../crates/locust-core/src/node/sim/check.rs). That one
  rule replaces the clause added for review finding 11, which it covers.
  `Restore::marked` now keeps each mark's position. The kept run
  `a_member_restored_with_its_marks_lost_and_then_kept_signs_again_what_only_another_member_holds`
  ends in the reused position. Before the change it failed with that
  position inside the claims.
  `a_record_that_came_back_after_the_marks_were_lost_leaves_the_claim_standing`
  checks that a record which came back and raised the mark stays inside the
  claims. The guard test
  `a_record_lost_with_the_marks_is_signed_over_after_a_later_restore_with_the_marks_kept`
  pins the guard's behaviour.
- **A new member's admission hold ended before it could read the rules
  (`a0dff5e`).** The guard's `admitted` hold ended on hearing the host's
  computer. That hearing can come from an exchange the host opened, which
  brings no content. Meanwhile the member's own exchange was still fetching,
  one object at a time, the content its records name, the rules definition
  among them. With the guard empty, every candidate came out pending and was
  refused as `conflict`: "the candidate cannot be applied yet … Nothing to
  change". The hold now also waits until the goal's current rules are
  readable and its current content key is held (`ready` in
  [guard.rs](../crates/locust-core/src/node/guard.rs)). `guard_admissions`,
  which `Node::land` runs for each goal it touches, ends the hold once that
  content lands after the hearing. The restore-guard model carries no content, and this condition
  only lengthens a hold. The plan's `Admitted` row says so.
  `a_key_just_admitted_waits_for_the_goals_rules_as_well_as_the_hosts_computer`
  steps the test network one input at a time (`Network::step`). Without the
  change it fails with the guard empty.
- **One owner-only mode rule (`79ce9cb`).** `locust_proto::local::owner_only`
  holds the rule and its message. The marks directory and file
  ([marks.rs](../crates/locust-store/src/marks.rs)) and the state directory
  ([home.rs](../crates/locust/src/daemon/home.rs)) call it.

The 10,000-seed release sweep, rerun after each of the first two and again
once they were rebased on main `ed75324`: no failure; 30 positions signed
again outside the claims and none inside, across the same 14 residual runs.
On the rebased tree `run.py --quick` passed all three scenarios and
`check_t1.py --network local` passed its 21 checkpoints. Before the second
fix the new member's first post was refused in 4 of 7 `crash` runs with the
kill and 2 of 6 without it. With the simulation's new wait and the old
daemon, five of five runs failed with `conflict`. With the fix, nine of nine
passed, one of them after the rebase.

A second review of those fixes found the cause under the second defect
([follow-up defects](v2-phase-g1-review-fixes-2026-10-07.md#follow-up-defects)).
A candidate whose rules definition had not arrived was refused as a
permanent `conflict`, so the admission hold was the only defence, and an
owner's `goal continue` or a restart before the content arrived still
reached it. Since `5e71dc7` such a candidate is refused as `unavailable` and
says the rules have not arrived. Only a missing definition is treated so: a
first version also mapped every other pending kind, and two existing tests
showed why not. A pick whose evidence was cancelled waits on evidence for
good, and a delivery acknowledgement whose effect stands on a fork waits on
a reference for good. Both stay `conflict`. `17591de` pins the content-key
half of the admission hold in a goal whose rules predate a removal, and
`e94a83a` makes the hold's refusal and the API's `admitted` description name
the rules and key it also waits for. The plan's mockup, function list,
`finish_joins` note and E2 risks follow. The 10,000-seed release sweep on
that tree: no failure, 30 positions signed again outside the claims and none
inside, the same 14 residual runs.

## Verification

- `cargo fmt --all --check`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 38 suites, 1,205 passed, 0 failed.
- `python3 scripts/check_formations.py`: the regenerated contract verifies. That contract was regenerated from a tree without A1's operation summaries, so main failed this check until `0c62207` regenerated it again (G1 review 23).
- `python3 scripts/check_docs.py`: passes with these notes and their index
  entry staged.
- `python3 scripts/check_documentation.py --binary target/debug/locust
  --timeout 60` (Python 3.13; the default 3.10 is too old for the extractor):
  all four recipes pass.
- `python3 -m unittest discover -s scripts/tests`: 301 run, 3 skipped, OK.
- `python3 scripts/check_tla.py --suite restore` (Python 3.13), rerun after
  the rebase onto `main`: all 32 registered cases, the new `restore-leave`
  among them, give their declared outcomes.
- `cargo run --release -p locust-store --example commit_latency`: one event
  4.0 ms (p90 4.1), one event and one mark 7.9 ms (p90 8.1), on the M5 Pro
  with APFS of the crate's table, which now carries both numbers.
- `git grep "signing watermarks" -- crates` finds nothing.
- The drills above, all passed.

## Drills

Two daemons on one Mac, each on a throwaway home under `/tmp`, with
`LOCUST_RELAY=none LOCUST_LOOKUP=none` and fixed loopback ports (a restarted
daemon on port 0 binds a new port its peer does not know). Author positions
were read with `sqlite3` from a copy of the database. All passed; none needed
a fix.

| Drill | Observed |
| --- | --- |
| A lost post | One `guard` entry for A's agent, `behind` (held 0, signed 1), `by_host` false, waiting on B; `"halted": null`; `restored` 0. The post exits 9 `read_only`; `goal invite` works and a third daemon is admitted. B started: the entry is gone on the first exchange (about 0.8 s), the post lands at seq 1, no halt on either daemon |
| A lost rule change | `"halted": "signer_recovery"`, one entry with `by_host` true, `behind` (held 4, signed 5). `rules bind --confirm` and a post by A's agent exit 9 `read_only` (`--plan` still answers). B started: the entry is gone within about 0.2 s and `rules bind` lands at governance seq 5 |
| Marks copied and put back too | On A both keys `unheard`, still after more than two exchanges with B; `call goal.continue` answers `{"continued":{"keys":2}}` and the post lands at seq 1. On B the agent is `unheard`; the first exchange (it brought records) leaves it, the second clears it with no command |
| `rsync -a` of both folders | The marks directory keeps its inode; the restored marks file is a new inode and reads as lost, and is replaced at start. Both keys `unheard` through two exchanges; `goal.continue` releases 2 |
| Marks directory deleted | After an ordinary run: no `guard` entry, `restored` null, the post lands. While a restore is caught up (agent `behind`): both keys `unheard`, still after B is heard; `goal.continue` releases 2 |
| A host alone | A daemon killed between the `rules bind` commit and its step's commit, restarted with no peer: the step is signed by the first status answer, no `guard` entry, no halt. The same crash state put back with the marks kept: released at once and the same step id signed |

Observed and not investigated: after A restarted, B sometimes took 15 to 31
seconds to receive A's next record (1 to 10 seconds otherwise); the guard's
results did not depend on it.

## Evidence boundary

The drills ran two daemons on one computer over loopback; no second machine,
Time Machine restore, Migration Assistant transfer or reboot was run (the file
identity note lists those as still missing). The commit latency was measured
on one M5 Pro with APFS. The simulator and the model are bounded runs, not
proofs.

## Commits

- `30815e7`: row 7 (R6) marked Built.
- `28c6425`: S1a's dead `Commit::drop_blobs` path removed.
- `1847b24`: the marks file beside the data directory, with its tests and
  the latency numbers.
- `b135883`: the guard, its views and `goal.continue`, the sync hooks, the
  tests, the simulator's restores, and the regenerated runtime contract.
- The commit containing this note adds the notes, their index entry and row
  8 marked Built; its hash is reported with the handoff.
