# Locust v2 S1b: a failed start says what to do

Status: implemented; the required checks pass, apart from the five known
`check_docs.py` lines that fail on the base commit too (below). This is S1b,
the open-sentences half of S1 ("Store hygiene") in the
[agent memory and store plan](../docs/agent-memory-and-store-plan.md), a
phase alongside the fifteen of the [master plan](../docs/master-plan.md). The
removal half, S1a, was built earlier (`28c6425`). S1b lands after
[G1](v2-phase-g1-build-notes-2026-10-07.md), so one pass covers the failures
G1 added when it gave the store a marks directory. The plans were read, not
edited, apart from the master plan's S1 row and its pieces table.

## What changed

- **What happened, and only that.** `OpenError`'s Display in
  [error.rs](../crates/locust-store/src/error.rs) keeps what happened and
  drops the advice it carried ("initialize a fresh state directory", "use the
  matching Locust release or a separate state directory", "is another daemon
  running on it?"). `InUse` now reads "another program has the database in
  HOME open": the daemon's own lock refuses a second daemon before the store
  is opened, so whatever holds SQLite's lock at a start is another program.
- **G1's mode refusal is a case of its own.** A marks directory or marks file
  that others may enter was a `StoreError::Failed` whose text ended in the
  `chmod` that fixes it. It is now `OpenError::MarksNotPrivate { path, mode,
  wanted }`, raised by `private` in
  [marks.rs](../crates/locust-store/src/marks.rs), so its next step is said
  where the others are. `marks::open` returns `OpenError`.
- **One next step per case.** `store_open_failure(home, marks, error)` in
  [daemon/mod.rs](../crates/locust/src/daemon/mod.rs) takes every failure of
  `SqliteStore::open` and, as `OpenError::Store`, every failure of
  `Node::open`, which before went straight to an `ApiError`. It prints what
  happened, then:

  | Case | Code (exit) | Next step |
  | --- | --- | --- |
  | `InUse` | `unavailable` (8) | Close it, then start again. |
  | `UnsupportedSchema`, `UnsupportedProtocolVersion` | `unsupported_version` (10) | The data folder HOME was made by another Locust version, and no version converts it. Start the version that made it, or move the folder aside, do not delete it, and start with a new one. A new data folder starts with no goals. Goals you host cannot continue from it, and goals you joined need a new invitation. |
  | `Store(Corrupted)` | `corrupted` (11) | Move HOME aside and do not delete it: it holds your keys and every record. A new data folder starts with no goals. Goals you host cannot continue from it, and goals you joined need a new invitation. |
  | `Store(Failed)` | `internal` (1) | Check that the disk has space and that you can read and write HOME and MARKS, then start again. |
  | `MarksNotPrivate` | `internal` (1) | Run chmod 700 PATH (600 for the file), then start again. |

  Exit codes are unchanged for every case, `MarksNotPrivate` included.
  A damaged record is named when the store names it: `stored event ID: ...`
  from [events.rs](../crates/locust-store/src/events.rs) stays in front of the
  sentence.
- **The lock.** The refusal of a second daemon in
  [home.rs](../crates/locust/src/daemon/home.rs) gains "Stop it, then start
  again."
- **The guide.** [operations.md](../docs/guide/operations.md) gains "If the
  daemon will not start" after Backups, linking to it. It lists each message
  with what to do, names the marks directory beside the data directory, says
  that the daemon never deletes a damaged or other-version data directory and
  never starts over on its own, and says what a new data directory loses. It
  describes no restore and leaves Backups to G2.

A start that fails this way moves and deletes no stored content. A
`locust.db` that is not SQLite, or is of another version, is refused by the
read-only check that runs before any directory is created, and a non-SQLite
one stays byte-identical, which the plan listed as unverified.

## Tests

- [open_tests.rs](../crates/locust/src/daemon/open_tests.rs), new:
  `each_store_open_failure_says_one_next_step` over the five `OpenError`
  cases and `MarksNotPrivate`, with the full message, the code and the exit
  status of each, and that the part before the next step holds no advice.
  Three starts of the production assembly that fail before listening, each
  leaving no socket and no lock:
  `a_marks_directory_others_may_enter_names_the_chmod_that_fixes_it` (the
  mode is left as found), `a_marks_directory_that_cannot_be_made_names_both_directories`
  (a file where the marks directory goes, left as found) and
  `a_record_the_node_cannot_read_says_to_move_the_folder_aside` (a local
  record `Node::open` refuses; `locust.db` stays byte-identical).
- [cli.rs](../crates/locust/tests/cli.rs):
  `corrupt_database_startup_preserves_corrupted_code_and_cleans_socket` checks
  the whole sentence and that `locust.db` is byte-identical;
  `held_daemon_lock_is_unavailable` checks the lock sentence.
- [home.rs](../crates/locust/src/daemon/home.rs):
  `a_second_daemon_on_the_same_directory_is_refused_until_the_first_is_gone`
  checks the whole lock sentence with the process number.
- [locust-store tests.rs](../crates/locust-store/src/tests.rs):
  `a_marks_directory_or_file_others_may_read_is_refused` expects
  `MarksNotPrivate` with the path and both modes.

## Departures and precise readings of the plan

1. The plan's lines were written against older trees. `store_open_failure`
   was at daemon/mod.rs 277-289 and `Node::open`'s mapping at 126-133;
   home.rs's refusal at 152-155. `tests/cli.rs` 2575 and 2584 are
   `corrupt_database_startup_preserves_corrupted_code_and_cleans_socket` and
   `held_daemon_lock_is_unavailable`, now at 2610 and 2633 of
   `crates/locust/tests/cli.rs`; the file has not moved.
2. "This data folder was made by another Locust version" reads "The data
   folder HOME was made by ...", with the path. Neither version error names
   the folder, and the next step is to move it.
3. The version case also says what a new data folder loses. The plan gives
   the loss sentences only to the damaged case, but its goal is that each
   failure "ends with what to do and what starting fresh loses", and the
   version case is the other one whose next step starts with a new folder.
   The cases whose next step is to start again say no loss.
4. `Failed` names the marks directory beside HOME. G1 put the marks there, a
   failure to create, read or write them is a `Failed` too, and one in the
   guard's first commit at start comes through `Node::open` as a plain
   `StoreError::Failed` that cannot be told apart from a database failure.
5. `MarksNotPrivate` is new (above), so the test covers six cases, not five.
   Its exit status stays 1, as it was under `Failed`, since the plan keeps
   exit codes; the state directory's own mode refusal exits 6.
6. `InUse` says "another program has the database in HOME open" rather than
   "HOME's database", which reads poorly with a path. As everywhere in the
   CLI, what happened starts in lower case after `locust: CODE: `, and each
   next step is one or more whole sentences after it.
7. The guide section uses the guide's term, "data directory"; the messages
   use the plan's "data folder".
8. Checked against G1's lost-goals wording: the host safety plan's mockup
   G-6, which G2 prints, says "A goal you hosted cannot be brought back from
   it. A goal you joined needs its ticket again." The S1b sentences say the
   same of a new data folder: hosted goals cannot continue, joined goals need
   a new invitation (a ticket is a single-use invitation, help.md). G2 may
   want one of "ticket" and "invitation" in both.

## Not done, and why

- `Node::open` turns an `ApiError` from the guard's first commit, and from
  the settle, project and flow loop after it, into
  `StoreError::Failed(error.to_string())`, so such a failure reads
  `storage failed: internal: storage failed: ...` before its next step. The
  pattern predates G1 (`82cabad`, `1f23a0e`); G1 added the guard's call to
  it. Fixing it means changing `Node::open`'s error mapping in
  `node/mod.rs`, which G2 may be editing for `lost_goals`, so it is left; the
  next step printed after it is right.
- The state directory's other refusals in home.rs (cannot be created, open
  to others, not a directory, a damaged owner credential, a socket path too
  long) keep their texts: the plan names only the lock sentence there.
- `docs/README.md`'s one-line description of operations.md and help.md's
  troubleshooting table are unchanged, to keep this phase to its own files
  while G2 edits nearby ones.
- Nothing ran on macOS; every check below ran on Linux.

## Verification

Linux 6.18 on ext4, rustc 1.96.1, Python 3.13.

- `cargo fmt --all --check`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 39 test binaries, 1,350 passed, 0
  failed, 12 ignored.
- `cargo build --locked -p locust`: built.
- `python3 scripts/check_formations.py`: passes. No API change, so nothing
  was regenerated.
- `python3 scripts/check_documentation.py --binary target/debug/locust
  --timeout 60`: all four recipes pass.
- `python3 -m unittest discover -s scripts/tests`: 335 run, 10 skipped, OK.
- `python3 scripts/check_docs.py`, with these notes and their index entry
  staged: fails with exactly the five known lines and no other. They fail on
  the untouched base commit `665fbe7` too: the pinned plan source commits in
  `SOURCE_BASELINES` are not in the remote's history.

  ```text
  docs/host-safety-and-ending-plan.md:1198: missing link target: ../crates/locust/src/cli/local_members.rs
  docs/roles-and-permissions-plan.md:828: missing link target: ../crates/locust-proto/src/api/permissions.rs
  docs/roles-and-permissions-plan.md:961: missing link target: ../crates/locust/src/cli/local_members.rs
  docs/roles-and-permissions-plan.md:988: missing link target: ../crates/locust/src/cli/permissions.rs
  docs/roles-and-permissions-plan.md:1092: missing link target: ../crates/locust-core/src/node/tests/permissions.rs
  ```
- By hand, `locust --home H daemon run` over a non-SQLite `locust.db` printed
  the damaged sentence, exited 11 and left `locust.db`, and created no
  `blobs/` and no marks directory; over a marks directory of mode 0755 it
  printed the `chmod 700` sentence and exited 1.

## Commits

- `f015564` (`bcaa62f` on `side/s1b`, cherry-picked onto `side/a2`): the
  sentences, `MarksNotPrivate`, the guide section and the tests.
- The commit containing these notes adds their index entry and marks S1
  built in the master plan; its hash is reported with the handoff.

## After the review

The [review of A2 and S1b](v2-side-a2-s1b-review-2026-10-08.md) found two S1b
defects, none blocking. S1b's commits were cherry-picked onto `side/a2`
(`f015564`, `e82cd8c`), where finding 4 is fixed. Review numbers are its
headings.

| # | Finding | Disposition | Commit | Test or text | Note |
| --- | --- | --- | --- | --- | --- |
| 4 | A program holding the database is reported as a storage failure, not as in use | Fixed | `32ec5a5` | `a_database_another_program_has_open_is_in_use` ([store tests](../crates/locust-store/src/tests.rs)), `a_database_another_program_has_open_says_to_close_it` ([open tests](../crates/locust/src/daemon/open_tests.rs)) | Under the exclusive lock the first access was `schema::check`, whose error became `Failed`. [connection.rs](../crates/locust-store/src/connection.rs) now reads once with the preflight's lock-aware mapping (`first_read`, `in_use`) before it. Both tests hold `locust.db` with an ordinary rusqlite connection that read `user_version`, as an `sqlite3` shell does, and fail without the fix; the daemon test checks the whole message and exit 8. `rusqlite` is a dev-dependency of `locust` for it. By hand, a daemon started while Python's `sqlite3` held the database printed "… open. Close it, then start again." and exited 8. |
| 5 | `Node::open` still turns guard, settle and flow errors into `Failed` | Deferred | | | Left until G2 has landed, as under "Not done, and why"; the orchestrator will ask for it then. |

Verification is that of the A2 fixes, on the same tree, in the A2
[build notes](v2-phase-a2-build-notes-2026-10-08.md#after-the-review).
