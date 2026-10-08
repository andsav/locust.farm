# Review of side phases A2 and S1b as built

Status: research finding, 8 October 2026. An independent review of two
unmerged side phases of the
[agent memory and store plan](../docs/agent-memory-and-store-plan.md), each
against its own section of that plan.

- **A2, "Start the next task"**: branch `side/a2`, commits `665fbe7..4f6acd9`
  (`47379f9` code, `4f6acd9` notes), read with its
  [build notes](v2-phase-a2-build-notes-2026-10-08.md). On `4f6acd9` the
  reviewer ran every check in the phase brief. `cargo fmt --all --check` was
  clean. `cargo clippy --locked --workspace --all-targets -- -D warnings` was
  clean. `cargo test --locked --workspace` ran 39 suites: 1,356 passed, 0
  failed, 12 ignored. `cargo build --locked -p locust` built, then
  `check_formations.py` passed and `check_documentation.py` passed all four
  recipes. The Python tests ran 335, skipped 10 and passed. `check_docs.py`
  printed exactly the five known baseline lines. The reviewer also ran the
  exit drill in a throwaway daemon under `/tmp`, with more cases than the
  build notes give, and a scratch hook-core test that was not committed.
- **S1b, the open-sentences half of S1**: branch `side/s1b`, commits
  `665fbe7..e6742bd` (`bcaa62f` code, `e6742bd` notes), read with its build
  notes, `research/v2-phase-s1b-build-notes-2026-10-08.md` on that branch.
  This review is by reading only. Nothing was built from `side/s1b`. Its
  store-opening code (`connection.rs`, `schema.rs`) is the same as on
  `side/a2`, so finding 4 was demonstrated with A2's debug binary.

Severities are as in the [G1 review](v2-phase-g1-review-2026-10-07.md):
**blocking** means the work does not count as done; **major** is wrong
behaviour a person or an agent would meet, or a check that passes without
checking; **minor** is real but rare, cheap, or confined to text and tests.

## Summary

**A2** does what its section asks. With no task, `attempt.start` returns the
session's live claim first. Otherwise it takes the first `to_start` item whose
`unattended` is true, using the same predicate the hooks read. When nothing is
free it answers `pending` and signs and binds nothing. One request plans and
lands the pick, so two sessions on one daemon cannot take the same task.

- At `ask`, `to_start` holds only allowed tasks, so `ask_first` tasks are left
  alone.
- A session bound to another member is refused `denied` before the pending work
  is read.
- A `pending` answer under an idempotency key is replayed, as the plan's risk
  note accepts.
- The `INSTRUCTIONS` text matches the plan word for word. The SKILL.md
  paragraph matches it too, apart from backticks in place of quotation marks.

The reviewer's drill agreed with the build notes:

- A start without a task printed a claim and exited 0.
- Repeating it returned the same attempt.
- Another session of the same member got the other task.
- With every task attempted, the start printed the pending view and exited 0.
- An offer without a task was refused `invalid`, exit 6.
- Another member using a borrowed session was refused `denied`, exit 3.
- An agent at `ask` got the pending view with every task under "Waits for
  carol's owner".

The build departs from the plan in two places, and both are justified:

- It reuses H1a's `Goal::unattended` rather than adding a new helper.
- A chat whose start without a task was answered `pending` is a worker chat
  in the hooks. The plan's H1 definition is "started or took over an
  attempt, or called `locust_wait`", and the [agents guide](../docs/guide/agents.md)
  says what the build does.

The hooks change asked about is right as far as it goes. The plan's stop rule
blocks "only for work IDs not shown since the chat's last Locust write". A
`pending` answer writes nothing, so treating it as a write would re-arm a block
the chat had already been shown. The change misses the same case with the
other answer. A start without a task that returns a claim the chat already
holds also writes nothing, but the core still counts it as a write. Together
with the new skill paragraph and the free-task stop line, that re-blocks a
claim-holding worker at every stop (1). The skill also tells an agent to
"finish" a returned claim whose cancellation is waiting (2). An older skill
sentence now contradicts the new paragraph (3).

**S1b** does what its section asks:

- Each `OpenError` case and the new `MarksNotPrivate` print what happened and
  then exactly one next step. The version and damaged cases also print what a
  new data folder loses.
- Every exit code is unchanged: `unavailable` 8, `unsupported_version` 10,
  `corrupted` 11, and `internal` 1 for both `Failed` and `MarksNotPrivate`.
- Every G1 marks failure at open is covered. A directory or file that others
  may enter names its `chmod`. One that cannot be created, opened, inspected
  or read gets the step that names both directories. Unreadable marks are
  "lost", not a failure.
- The plan's tests exist and can fail: they assert whole messages and
  byte-identical files. The guide section sits after Backups and links to it.

The build's departures are justified in its notes: the folder path in the
version sentence, its loss sentences, and the marks directory in the `Failed`
sentence.

Two S1b defects remain. Neither is new code; each sends a person to the wrong
step:

- A real "another program has the database open" start is not classified as
  `InUse`. The guide's own `sqlite3` example gets the disk-space step (4).
- `Node::open` still turns every guard, settle and flow error into `Failed`,
  which the notes leave for after G2 (5).

No finding is blocking. Neither branch carries attribution trailers. The two
branches merge cleanly except for one row of the
[master plan](../docs/master-plan.md)'s plans table. Each branch rewrote the
row "Agent memory and store" ("A1, A2, A3 and S1a" against "A1, A3 and S1"),
so whoever merges second writes "A1, A2, A3 and S1".

## A2

### 1. A start without a task that returns the held claim re-arms the stop block

Major. [core.rs](../crates/locust-adapter/src/hooks/core.rs) lines 309-326
(`observe`); the stop line at 679-695; [SKILL.md](../skills/locust/SKILL.md)
lines 156-165.

**What is wrong.** `observe` now treats an `attempt.start` answered with
pending work (`OwnAction::Other`) as no write (`started_nothing`, line 317).
An `attempt.start` answered with `Claimed` still counts as a write. That is
so even when the claim is the one this chat already holds, at the same
generation, and nothing was signed or stored. The write clears every shown
fact except claims and sets `has_blocked` to false (lines 318-326). Before A2
that answer came only from a retry of a named start. A2 makes it the ordinary
answer: the skill says "When `locust_wait` or `locust_pending` lists tasks to
start, call `locust_attempt_start` with the goal and no task. If this session
holds a claim, you get it back".

**How it fails.** A worker holds claim C, and another task T is free. At a
stop, the hook blocks once with "… 1 free tasks; goal G, task:T. Use
locust_context_read and locust_attempt_start". The agent obeys the line as the
skill says, with no task. It gets C back, which signs nothing, and the hook
core counts that as progress. T is no longer shown, so the next stop blocks on
T again, and so on for every round in which the agent obeys. The plan's rule
is "block only for work IDs not shown since the chat's last Locust write". Its
H risks section adds that a bookkeeping write "never re-arms a block". The
[agents guide](../docs/guide/agents.md) promises that "a worker can always end
its turn to ask its owner". A worker that stops mid-task to ask its owner is
the case that breaks.

**Demonstration.** A scratch integration test (not committed) used the
public `hooks::core` API. It had one held claim and one unattended task in
`to_start`, in a person's chat (`Stop { unattended: false }`). Each round
called `decide(Stop)` and then `observe` of an `attempt.start` whose answer is
`Claimed(held)`, with a new native tool-use ID each time:

```text
stop 0: Locust: 1 held attempts, … 1 free tasks; goal 0101…, attempt 0303…, generation 1. Use locust_c…
stop 1: Locust: 1 held attempts, … 1 free tasks; goal 0101…, task:0404…. Use locust_context_read and l…
stop 2: Locust: 1 held attempts, … 1 free tasks; goal 0101…, task:0404…. Use locust_context_read and l…
blocks in 5 stops: 5
```

The answer was `Pending` in place of the claim the build's own test uses.
With that answer, the stop after the first block goes through. Codex and
Claude Code send native tool-use IDs, so every such call counts. Droid is
spared only by accident: its logical-effect ID for the recovered claim equals
the one its original start recorded, so the callback is dropped as a replay.

**Fix.** In `observe`, also treat as no write an `attempt.start` or
`attempt.takeover` whose claim is already in
`marks.goals[goal].claims` at the same generation. The plan says "a held
claim stays shown until its generation changes". Add a core test beside
`a_start_that_found_nothing_to_take_does_not_block_its_turn_end_again`, with a
held claim and a free task, that obeys the line twice. Also consider making
the free-task stop line name the task to start ("locust_attempt_start with
this task") while the chat holds a claim. A start without a task cannot take
the task the line names then.

### 2. The skill tells an agent to "finish" a returned claim whose cancellation is waiting

Minor. [SKILL.md](../skills/locust/SKILL.md) line 158;
[claims.rs](../crates/locust-core/src/node/requests/claims.rs) lines 41-58
and 109-111.

**What is wrong.** `work.claimed` keeps a claim whose cancellation has not
been answered, so a start without a task returns it. The build notes'
departure 4 says so. The skill then says "you get it back: finish and report
it first". For such a claim, finishing is what the cancellation forbids: the
`attempt.cancel` summary says the attempt "takes no more results or progress".
Reporting is refused until the cancellation is acknowledged
(`check_cancellation`: "answer the cancellation before ending the attempt;
cancelled work cannot continue"). The answer is a bare `Claimed`, so it does
not show the `to_acknowledge` item a `pending` read would. The H2 notice is
printed once and may already have been spent.

**Fix.** Change the skill sentence to "If this session holds a claim, you get
it back: finish and report it first, or, when `locust_pending` lists a
cancellation for it, acknowledge that first."

### 3. An older skill sentence now says the opposite of A2's paragraph

Minor. [SKILL.md](../skills/locust/SKILL.md) lines 277-278.

**What is wrong.** "The daemon drives allowed flow transitions; an agent need
not request the next task." The sentence was meant for formation flows
opening their next task. A2's paragraph, under the phase name "Start the next
task", now tells the agent to request a task with
`locust_attempt_start` and no task. A model reading both is told to do it and
told it need not.

**Fix.** "The daemon drives allowed flow transitions; an agent need not open a
flow's next task itself."

## S1b

Line numbers in this section are at `side/s1b`.

### 4. A program holding the database is reported as a storage failure, not as in use

Minor. [connection.rs](../crates/locust-store/src/connection.rs) lines
16-33 and 71-90; the guide row in `docs/guide/operations.md` line 105 at
`side/s1b`; [daemon/mod.rs](../crates/locust/src/daemon/mod.rs) lines
290-328 at `side/s1b`.

**What is wrong.** S1b rewords `InUse` to "another program has the database in
HOME open", and gives it "Close it, then start again." The build notes reason
that "whatever holds SQLite's lock at a start is another program". The guide
row says "Close that program (an `sqlite3` shell, for example)". But an
ordinary SQLite connection does not reach `InUse`. In `connection::open`, the
first access under `locking_mode=EXCLUSIVE` is `schema::check` (line 25),
whose error goes through `sql()` and becomes `StoreError::Failed`. Only the
later `journal_mode` pragma maps busy or locked to `InUse`. The read-only
`preflight` read succeeds beside a WAL reader. So the person is told "Check
that the disk has space and that you can read and write HOME and MARKS, then
start again", and the exit is 1, not 8.

**Demonstration.** This used A2's debug binary, whose `connection.rs` and
`schema.rs` are those of `side/s1b`. A home was created by one daemon run and
the daemon was stopped. A second start with nothing holding the database
succeeded. Then a Python `sqlite3` connection opened `locust.db`, read
`PRAGMA user_version` and stayed open. The next `daemon run` printed
`locust: internal: storage failed: sqlite: database is locked` and exited 1. A
plain-Python reproduction of the open sequence (SQLite 3.45.1) found the read-only read
succeeding, then "database is locked" on the first `PRAGMA user_version` after
`locking_mode=EXCLUSIVE`. With S1b, the same start prints the disk-space step.

**Fix.** In `connection::open`, map `DatabaseBusy` and `DatabaseLocked` to
`OpenError::InUse` for the first access under the exclusive lock, as
`preflight` does. The simplest way is one lock-aware read before
`schema::check`. Add a store test that keeps a second rusqlite connection
open on `locust.db` and expects `InUse`. Add a daemon test that expects
"Close it, then start again."

### 5. `Node::open` still turns guard, settle and flow errors into `Failed`

Minor. [node/mod.rs](../crates/locust-core/src/node/mod.rs) lines 211-221 at
`side/s1b`.

**What is wrong.** Errors from `guard_start`, `land_once` and `drive_flow` are
`ApiError`s, and each becomes `StoreError::Failed(error.to_string())`. A
`corrupted` one met there therefore gets the disk-space step and exits 1,
not 11, which is not the next step the plan gives "Damaged". The text also
reads `storage failed: corrupted: …` or `storage failed: internal: storage
failed: …`. The build notes list this under "Not done": the mapping predates
G1, and `node/mod.rs` may be changing under G2. That reason holds for now.
The defect is still S1b's to close, since the plan routes "`Node::open`'s
error" through `store_open_failure`.

**Fix.** Once G2 has landed, keep the code. Map an `ApiError` with
`ErrorCode::Corrupted` to `StoreError::Corrupted`, and use the inner message
so the prefix is not doubled. Alternatively, let these paths return the
`StoreError` they wrap. Add a case to `each_store_open_failure_says_one_next_step`, or a
start test, that reaches the guard's first commit with a damaged record.
