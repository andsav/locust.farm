# Agent memory and store follow-ups: implementation plan

Status: approved by the owner on 6 October 2026 as phases in the master
plan's build order (answer 30); not built. Derived from
[the beads assessment](../research/beads-assessment-2026-10-06.md) at
baseline `0a448ad`. The owner approved the
[rows below](#rows-in-the-master-plans-build-order) for the
[master plan](master-plan.md)'s build order.

Code lines were verified at `0e902e3`; lines in files changed since were
re-checked at `0a448ad`, and H's at `3dfb1fd`. Each phase re-checks its
lines when it starts.

## What this does, today against after

| | Today | After |
| --- | --- | --- |
| Choosing a tool | 31 tools describe themselves by name only, such as "board". | Each says what it returns and when to use it. (A1) |
| A large context read | A page can be any size and can in principle outgrow a message. | A page holds at most 32 items; asking for more quietly gives 32 and paging continues. (A1) |
| Ordering tasks | Nothing tells an agent how to say "do this after that". | The skill says: a follow-up names its parent; a task that should wait starts its title "After task:" and the full ID. Nothing enforces it. (A1) |
| Starting work | An agent picks a task by ID; another session of the same agent may pick the same one. | When the list shows work, one call with no task starts a task nobody is attempting, as far as this computer has heard. Asking twice returns the same claim. With nothing free, nothing is signed. (A2) |
| What the goal knows | Findings arrive as items to page through; after J5 a newcomer's unread view leaves older ones out. | The first compact page lists the 20 newest goal-wide findings by first line, with the total. (A3) |
| Back after a break | `locust_status` returns the attempts an agent holds, but nothing says what they mean, and the CLI `status` does not print them. | The instructions say each is an attempt to resume and how; the CLI `status` lists each with its title and generation and the two commands that resume it. (A4) |
| Work waiting, cancellation, compaction | Seen only through the agent's calls. | Hooks, in every harness Locust sets up, keep it working, show a cancellation at the next tool call, and reset the read record. (H) |
| Retiring a finding | An agent cannot say its own earlier finding is out of date. | A new finding can replace the same author's old one, which leaves the current list. (A6) |
| IDs a person sees | Some lines cut IDs at a fixed 8 characters even on a clash. | One cutter everywhere: 8 characters, longer only on a real clash. (A5a) |
| IDs an agent types | MCP tools need full IDs. | After R7, if its counts justify it: 8 characters of an ID a tool showed in the same connection; anything else is refused with a fix. (A5b) |
| The daemon will not start | The error says what happened, sometimes with a hint. | Each case ends with what to do and what starting fresh loses; a guide section repeats it. (S1b) |
| Stored content | Code that deletes stored content exists, though only tests call it. | That code is gone. (S1a) |
| A long goal | Replay cost is measured up to 512 tasks. | Replay and one new record are measured at about 10,000 and 41,000 records. (S2) |
| Releasing | Nothing stops a format change under a released number. | Such a build is refused; every build opens the last release's data. (S3) |
| A sync keeps being refused | `goal status` does not say why. | Each peer line shows its last refusal until a sync completes. (J1+) |
| This repository's phase state | Three places disagree on what is built. | Only the build-order table says, with the commit. (P) |

## Where it sits in v2

**One need.** R7's early run waits for H1a, so it sees the stop hook that
ships. Every other phase lands before its target if ready; the v2 phase
never waits. The critical path stays R4/R5 fixes, G1, G2, E1, E2, R7, R8,
R9, R10, then J2 to J6.

**Before R7's early run, if ready:** A1, A2, A3, A4, A5a, P, H1b, H2 and
H3. R7 rewords skill and tool words on a miss
([roles plan](roles-and-permissions-plan.md) 4157-4160), so it should see
the text that ships; P rewrites R7's run text. A slipped phase is measured
in R10's run.

**After R7's early run:** A5b. MCP results still show full IDs, so prefix
input is judged against measured ID errors, together with output shortening
([agent-ergonomics](../research/agent-ergonomics-2026-10-05.md) 467-470).

**S1 splits.** S1a (removal) lands now, before G1 opens the store files if
ready, else right after G1, never interleaved. G1 does not build on
`drop_blobs`; it only shares the `Commit` literals. S1b (open sentences)
lands after G1, so one pass covers G1's new `open(dir, marks)` failures.

**The one signed change.** A6 lands right after J2's change set, before any
build with J2 is published. J2 changes signed bytes anyway
([master plan](master-plan.md) 400, 411), so A6 shares its step, and the five
sentences saying E1's end record is the last event change in the fifteen
phases (master plan 408; host-safety plan 231, 883, 3620, 3741) stay true.
J2 alone owns any version raise. A6 never lands alone after a release.

**Format rule.** Every API change here is "inside 7" only if it lands before
the first published build; after that it follows Versions.

**Lanes on one checkout.** Sessions stage whole files, so one session writes
a file at a time. Each phase runs `git status` first, waits for any session
with uncommitted edits in its files, and stages by file. Regenerate
`runtime.contract.json` only when no other session has an uncommitted API
change, since the binary carries that code.

- **Lane A, agent surface:** A1, A2, A3, A4, A5a, later A5b. Owns the
  operation table and constants in `api.rs`, `api/context.rs`, `context.rs`,
  `context_views.rs`, `claims.rs`, `to_start` in `views.rs`, status and ID
  cuts in `presentation.rs`, `mcp/*`, `context_receipts.rs`, `selectors.rs`
  and SKILL.md; never the store, sync or event code. When G1 starts wiring into `api.rs`,
  `views.rs`, `context_views.rs` or `presentation.rs`, Lane A finishes its
  commit and pauses. A4 or A5a that miss that window wait until E2 has
  landed and rebase onto its status and `Member:` lines.
- **Lane B, store and process:** S1a, P, S2, S1b, S3. Owns
  `locust-store`, proto `store.rs`, `commit.rs`, `content_graph*`,
  `definitions.rs`, `daemon/mod.rs` open errors,
  `organization_performance.rs`, `build_release.py`
  and P's plan edits; never Lane A's files.
- **Lane D, hooks:** H. Owns `hook.rs`, `hooks.rs`, setup's hook entries and
  qualification scripts; edits Lane A's files between its commits, after
  A4, and `node/mod.rs` never during G1.
- **Lane C, J1 with J1+,** in R7's window, when v2 writes almost no Rust.
  Never touches SKILL.md, the `wait` and `pending` rows or collaboration.md.

Shared files: S1a precedes A1 in `node/tests/context.rs`; in
`tests/cli.rs` the first of A2 and S1b commits before the other starts;
`INSTRUCTIONS` sentences land A3, A2, A4, G2, then A5b.

| Phase | Lands | Needs | Why then |
| --- | --- | --- | --- |
| S1a | Now; before G1's store work, else right after | nothing | G1 would otherwise carry code about to be deleted |
| A1 | Now, alongside G1 | nothing | Later agent rows are written under its test; R7 sees the words |
| P | Now | nothing | Build sessions then edit only their row; R7's text is right before R7 |
| A3 | After S1a, before R7 | S1a's test helper | R7's seeded findings reach agents through it; J5 builds on it |
| A2 | Before R7 | nothing | R7 starts agents with no task named |
| A4, A5a | Before G1 touches `presentation.rs`, else after E2 | nothing | Small; G1 to E2 then build on them |
| S2 | Any time, before R8 for a baseline | nothing | R8, R9 and J2 add fold work; rerun at R10 and J6 |
| S1b | After G1 | G1 | Covers G1's open failures in one pass |
| J1+ | Inside J1 | J1, E2 | J1 rewrites the same function; it folds in E2's set |
| A5b | After R7's early run | A5a, R7's counts | Judged against measured ID errors |
| H1a | Before R7's early run (a need) | A4 (order) | R7 sees the hook that ships |
| H1b, H2, H3 | After H1a, before R7 if ready | H1a | Harnesses as confirmed |
| A6 | Right after J2's change set | A3, R7's run | Shares J2's signed step |
| S3 | With the first published build | G1, owner's answer on versions | The ledger must exist at the first release |

### Rows in the master plan's build order

Under Build order, after the table of fifteen:

> **Alongside the fifteen.** From the
> [agent memory and store plan](agent-memory-and-store-plan.md), approved by
> the owner (answer 30). Only R7's early run waits for one, H1a. Only A6
> changes signed bytes, and it rides in J2's step.
>
> | Phase | What works afterwards | Lands |
> | --- | --- | --- |
> | S1 | Nothing removes stored content; a failed start says what to do | before G1, else right after (removal); after G1 (sentences) |
> | A1 | Agent tools say what they return; a context page holds at most 32 items | before R7, if ready |
> | A2 | An agent starts a free task in one call | before R7, if ready |
> | A3 | The first compact page lists current findings | before R7 and J5, if ready |
> | A4 | `status` lists held tasks and how to resume | before R7, if ready |
> | A5 | One ID cutter; later, short IDs an agent was shown | before R7 (cutter); after R7 (bridge) |
> | S2 | A long goal's cost is measured | before R8 |
> | H | An agent with work waiting keeps working, in every harness Locust sets up, with any model | before R7 (H1a needed) |
> | A6 | An agent can retire its own finding | right after J2, in J2's step |
> | S3 | No format change ships under a released number | the first published build |

Amendments: **R7**'s Needs ends "…and H1a of the agent memory and store
plan." **J1** ends "…and `goal status` shows the last refusal each computer
sent." J1's Needs becomes "nothing; E2 for the last refusal", and the Public
goals bullet adds "its last-refusal line waits for E2". **J2** adds "A
finding can replace the same author's earlier one (A6), in J2's step." In
Versions, after "The first public door changes signed bytes once more, in
J2." comes "A6 lands right after it in the same step.", and "that change
stays inside 7" becomes "J2's change, with A6's, stays inside 7". Under
"Assumed until the owner objects", five bullets name what a person sees of
this plan's own choices.

## Decided by the plan author

- **A2 keeps the wait:** agents call `locust_wait` or `locust_pending`, on their own or after a hook line, then start with no task; R7's criterion widens (P).
- **A no-task start ignores "After task:" lines:** the skill sends the agent to name a task when one is still waiting.
- **No pick tiebreak:** R7's run shows herding if any; a local fix would change no format.
- **The context cap is 32, silent, in the daemon:** half a frame, every client bounded alike.
- **No listed tool becomes `tool: false`:** each returns something no other read does.
- **Tool sentences stay near 30 words:** every description is sent at every session start.
- **A3 lists on the compact first page only, and not again unchanged:** the full view already delivers every finding.
- **Splits:** A5b and S1b each wait for what they need; A4, A5a and S1a land now.
- **A5b keeps blockers, narrowed:** they still stop a prefix copied from text.
- **A6 ships without a kind field:** "finding" keeps A3's meaning.
- **J1+ replaces E2's refused set:** one structure, not two.
- **R7's seeds are findings, not tasks:** R7's setup stays true and A3 shows them.
- **Stop rule:** block only for work IDs not shown since the chat's last Locust write; after an ignored block, stops go through until a new ID. A context acknowledgment is not a write for this rule, and a held claim stays shown until its generation changes. One worker chat per session waits, at most 270 s, or less where an adapter's harness ends a hook sooner, and only where no person types; in the person's own chat an idle worker is told once and the next stop goes through.
- **Hook output:** one ASCII line under 512 bytes: counts, IDs, tool names, fixed words; on failure, "Locust context was NOT injected", once per failure episode and only in a chat that has used Locust. A chat that never used Locust, and a subagent's callback, get nothing.
- **State stays local;** a reset forgets rather than flags. No `--owner`, prompt-submit or reject hook.

## Decided by the owner

Answers of 6 October 2026 to this plan's two questions and its rows.

1. **Hooks.** First: "We should initially integrate locust as mcp tools, we
   can explore hooks if it helps the system. Hooks are supported by all
   harnesses not just claude code." Then: "we already know the numbers
   are going to be better with hooks and we can use mcp tools as
   fallback"; each harness's hooks "we can figure out in time". So hooks
   are in, with no measurement gate, for every harness Locust sets up (H).
   MCP tools stay the integration, the fallback and the only way an agent
   acts.
2. **An agent marking one of its own earlier findings as replaced.** "yes".
   A6 is accepted and lands in J2's format change.
3. **The rows and the hooks prompt.** "yes, add the rows and write the hooks
   prompt. Remember that we want to keep the hooks work model and harness
   agnostic." H's shape follows it.

S3 still waits on the master plan's open versions question, not asked again.

## Ownership

Each item is defined in one phase. Later phases describe their own rows and
edit only their own skill sentences.

| Item | Phase |
| --- | --- |
| The 37 summaries; the description checks in the model-strings test; `MAX_CONTEXT_PAGE` and the clamp; SKILL.md 92, 103, the cap and ordering paragraphs; the paging test adaptations | A1 |
| `AttemptStart.task`, the offer guard, the `is_answered_by` split, the `attempt.start` row, the no-task pick, A2's skill and `INSTRUCTIONS` sentences | A2 |
| `BRIEF_FINDINGS`, `FINDING_LINE_CHARS`, `CurrentFindings`, `FindingHeadline`, `ContextBrief.findings`, the goal-finding predicate, `current_findings` (what "current finding" means), A3's sentences | A3 |
| The claims sentence in `INSTRUCTIONS` and SKILL.md 35; status claim lines, `agent_lines` taking the `Reader`, the `Context:` and `Pending:` lines | A4 |
| `short` as the only cutter; the cut scan test | A5a |
| `Shown`, `MAX_SHOWN_IDS`, `response_schema`, `refusal_schema`, MCP ID widening, prefix refusals, SKILL.md 87-88 | A5b |
| `supersedes`, its fold rule, the `contribution.publish` rewrite, "not superseded" | A6 |
| Removing `drop_blobs`, `objects::delete`, `Files::remove`; the missing-payload test helper; crates.md 47-50 | S1a |
| Store-open sentences, `OpenError` Display, the lock sentence, the operations.md section | S1b |
| `one_event_apply`, the new sizes, the long-goal note | S2 |
| `format_digests`, the ledger, `--record-published`, the fixture and its test | S3 |
| `Report.received`, the last-refusal map, `PeerView.last_refusal`, `PeerRefusal`, `refusal_words` | J1+ |
| `locust hook`, the hook events, output contract and adapter interface, marks, the hook rules, `hooks.rs`; later adapters; `session.chat`, presence, the hooks each chat has reported | H1a; H1b; H3 |
| Phase state, findings numbering, R7's run text, hand-off sentences in v2 plans | P |

## Phases

### A1: Words and page caps

**Goal.** Every operation an agent or author can call says in one sentence
what it returns or records and when to use it. The skill names only real
tools. A `context.read` page holds at most 32 items. The skill says how to
order tasks.

**Depends on.** R6 (landed). S1a first in `node/tests/context.rs`.

**Changes.**
- [api.rs](../crates/locust-proto/src/api.rs) `operations!` (846-938): 37
  `Agent` or `Author` rows have a summary equal to their name. Each gets one
  third-person sentence from its handler (roles plan 3374-3376), near 30
  words: the 31 listed tools at 891-933 and the CLI-only
  `session.report`, `session.show`, `sessions`, `session.drop` (857-860),
  `blob.put` and `blob.get` (919-920). Facts that must appear: `board` is
  the only read listing every task (`TaskView`, 1575-1587); a task with a
  parent follows the parent's rules
  ([tasks.rs](../crates/locust-core/src/node/requests/tasks.rs) 66-78);
  `scope.select` refuses workspace changes (283-313); `blob.withdraw` also
  stops local reads and keeps the bytes
  ([content.rs](../crates/locust-core/src/node/requests/content.rs)
  171-192); `events` pages at most 256 entries. The 14 owner and host rows
  (852-856, 861-864, 869-872, 895) stay. The "Not tools" list in
  runtime-reference.md (80-84) stays.
- `pub const MAX_CONTEXT_PAGE: u32 = 32;` beside `MAX_FEED_PAGE` (122-124),
  with `const _: () = assert!(MAX_CONTEXT_PAGE as usize *
  (crate::limits::MAX_PAYLOAD_BYTES + crate::limits::MAX_HEADER_BYTES) <
  crate::limits::MAX_LOCAL_FRAME_BYTES);`. Items take about half a frame
  ([limits.rs](../crates/locust-proto/src/limits.rs) 11, 21, 51); the first
  page's summary is not bounded. `Context.limit` (742) gets a plain doc
  comment, with no brackets or dotted names, since it reaches the schema.
- [context.rs](../crates/locust-core/src/node/context.rs): after the zero
  refusal (173-178), `let limit = limit.min(MAX_CONTEXT_PAGE);`, before the
  cursor check (192). The cursor stores the capped value (282) and the
  receipt lists only delivered items (285-301).
- [SKILL.md](../skills/locust/SKILL.md): line 92 writes out the three
  `locust_formation_*` names. Line 103: "`locust_pending` lists each task to
  start with `attempting` and each result to review with `verdicts`"
  (api.rs 1606-1611, 1620-1626). The context paragraph (43-51) gains the
  cap. A new paragraph between 130 and 132: a follow-up names its source as
  `parent` and follows that task's rules; a task that should wait begins
  "After task:" and the full task ID, its title in the board and ready list
  ([entry.rs](../crates/locust-core/src/node/entry.rs) 140-149;
  presentation.rs 405-413, 468); check the named task is complete before
  starting and prefer other work if not; no code reads the line. Always the
  full ID, because other computers never showed a prefix.
- [runtime-reference.md](guide/runtime-reference.md) 92-95 gains the cap.
  Regenerate [runtime.contract.json](reference/generated/runtime.contract.json)
  with `python3 scripts/check_formations.py --write` after a build.

**Tests.**
- `strings_written_for_a_model_name_listed_tools_and_no_operation`
  ([mcp/tests.rs](../crates/locust/src/mcp/tests.rs) 287) adds SKILL.md to
  `written` (path as at args.rs 408), so every `locust_*` token must be a
  listed tool, and asserts no `Agent` or `Author` summary is its bare name.
  Both hold at HEAD after the change. The forbidden-word loop (334-352)
  already covers the new sentences.
- New `a_context_limit_above_the_page_maximum_is_capped_and_continues`
  (context_views.rs): 33 findings, `u32::MAX`; 32 items and `next.limit`
  32; following with `u32::MAX` succeeds; every finding appears once; each
  receipt lists only its page.
- Adapted: the `context()` helper (tests/context.rs 38-49) asserts
  `next.is_none()`; `compact_seen_context_size_does_not_repeat_review_obligations`
  (context_views.rs 431) follows `next` and acknowledges every page before
  its unread read; `context_read_performance` (context.rs 707, call at 767)
  reads and acknowledges every page.
- Must pass: context_views.rs 126; args.rs 406. Run the ignored
  `tool_schema_cost_report` (schema.rs 120) before and after and put both
  catalog sizes in the commit message.

**Exit criteria.** No bare agent or author summary in `locust contract`; a
read with limit 1000 returns at most 32 items and pages to the end;
`check_formations.py`, `check_docs.py` and the three Rust checks pass.

**Format impact.** Text and paging only.

**Overlap with v2.** Later agent rows are written under the test; E1
replaces the `scope.close` and `scope.reopen` sentences
(host-safety plan 3111-3112); R9 deletes the integrate row, which A1 leaves.

**Risks and notes.** Each sentence is checked against its handler; an
earlier draft had three false ones (agent-ergonomics 474-477).

### A2: Start the next task

**Goal.** `locust_attempt_start` with a goal and no task returns this
session's live claim if it has one, else starts the first `to_start` task
nobody is attempting, as far as this computer has heard, else signs nothing and returns the pending
answer. Naming a task works as today.

**Depends on.** Nothing.

**Changes.**
- [api.rs](../crates/locust-proto/src/api.rs): `AttemptStart.task` becomes
  `Option<TaskId>` (574-578). `Request::check` (1043) refuses an offer with
  no task as `Invalid`: "an offer belongs to a task; name the task too".
  `is_answered_by` (1170) lets a no-task start take `Claimed` or `Pending`
  (the client enforces it, client.rs 228). The `Response::Pending` doc
  (1324) and the `attempt.start` row (897) say so.
- [claims.rs](../crates/locust-core/src/node/requests/claims.rs)
  `attempt_start` (87-148), with no task: validate the session binding and
  drop the write, as context.rs 180-183 does; read `pending_work` (views.rs
  355); if `work.claimed` holds this session's live claim (465-490), answer
  it; else take the first `to_start` item whose current round has no
  running attempt by anyone, via a new `unattended` helper, because
  `attempting` leaves out the caller's own principal (428-443); run the
  existing signing path; with no item, return `Pending(work)` unsigned. One
  `respond` plans and lands both (requests/mod.rs 54, 116, 138), so two
  sessions on one daemon never pick the same task. `to_start` already
  excludes forbidden, closed and above-level tasks (views.rs 419-424,
  446-459). `requests/mod.rs` 321-322 passes the `Option`.
- CLI: no code change; `--task` turns optional from the schema (args.rs
  41-46, 63-70) and `--session` stays required (cli/mod.rs 455-468).
- `mcp.rs` `INSTRUCTIONS` (41) becomes "Independent work begins with an
  attempt: when locust_wait or locust_pending lists tasks to start,
  locust_attempt_start without a task takes one nobody attempts.
  Contributions do not select or apply files."
- SKILL.md after 116-117: "When `locust_wait` or `locust_pending` lists
  tasks to start, call `locust_attempt_start` with the goal and no task. If
  this session holds a claim, you get it back: finish and report it first.
  Otherwise it starts the first task nobody attempts. It does not read
  'After task:' lines, and `locust_pending` lists tasks to start by ID
  only: if `locust_board` shows a task to start whose title begins 'After
  task:' and the named task is not completed, start another task by name.
  `pending` means every task you may start is
  attempted; if the rules allow several attempts, start one by name. The
  pick sees only this computer, so a member elsewhere may start the same
  task before the next sync." [concepts.md](guide/concepts.md) 53-54 gains
  one sentence. Regenerate the contract.
- Every `Request::AttemptStart` literal becomes `task: Some(..)`:
  `sim/scenario.rs` 203, twelve `node/tests` files,
  `locust-core/tests/organizations.rs` (7), `client.rs` 514,
  `durable_tests.rs` 226, 264, 583, and `tests/cli.rs` 1996.

**Tests.** In context_views.rs: the first unattended task is taken, and a
failed attempt frees it; a second session gets another task; repeating
returns the same claim; all attempted answers `pending` and signs nothing;
`ask` leaves `ask_first` tasks alone; another principal's session is denied. In api.rs tests (2088): pending answers only
a no-task start; an offer without a task is invalid. In args.rs:
`attempt_start_parses_without_a_task`. `t2_flow.rs` 324-327 starts with
`{"goal": goal}`. Must pass: context_views.rs 550, presentation.rs 2067.

**Exit criteria.** Checks as A1. In a throwaway daemon under /tmp, a no-task
start prints a claim, repeated prints the same attempt, and with all tasks
attempted prints the pending view and exits 0.

**Format impact.** One optional field, one widened answer check.

**Overlap with v2.** E1 decides whether a no-task start in an ended goal
answers `pending` with `ended` or refuses `ENDED` (host-safety plan
3076-3079, 3191-3193). J2 narrows `to_start` (joinable plan 2171) and the
pick follows.

**Risks and notes.** Two computers can still start the same task, but
the stale window shrinks from think time (13.7 s, role-free-board 146-149)
to sync delay; the order (views.rs 638-639) is the same everywhere. A `pending`
under an idempotency key replays on retry (requests/mod.rs 102-110).
Unverified: that schemars leaves `Option<TaskId>` out of `required`.

### A3: Current findings

**Goal.** A compact read's first page lists the 20 newest goal-wide findings
(event ID, author name, first line up to 120 characters) and the total.
`locust_event_show` reads one in full. Listing marks nothing read.

**Depends on.** S1a's test helper. A1 keeps `event.show` a listed tool.

**Changes.**
- [api/context.rs](../crates/locust-proto/src/api/context.rs):
  `BRIEF_FINDINGS = 20`, `FINDING_LINE_CHARS = 120`, `CurrentFindings {
  total, newest_event, newest: Vec<FindingHeadline> }`, `FindingHeadline {
  event, author, name: Option<String>, line: Option<String> }`;
  `ContextBrief` (120-135) gains `findings`.
- A fold-level predicate in `goal/`: scope `Goal` and no attempt (fold.rs
  420-422, 530-539). A6 reuses it.
- [context_views.rs](../crates/locust-core/src/node/context_views.rs):
  `Node::current_findings(entry, reader)` alone defines a current finding:
  an Effective contribution (projection.rs 45-51, 203-223) that meets the
  predicate and whose payload is not withdrawn here (`blob_record`,
  content.rs 18-37, checked per candidate since `total` counts unshown
  ones). Newest first by feed position, then ID. `name` via
  `members.get`, never an index. `line` is the first line of `entry.text`
  (entry.rs 114-136), cut by `char_indices` as context.rs 257-262 does.
- `context_summary` calls it in the compact branch only (44-67). An
  `unread_only` read fills `newest` only when the session has acknowledged
  no context yet or a current finding is unread to it; otherwise only
  `total` and `newest_event`. A read without `unread_only` always fills it.
  Nothing touches `delivered`, `ContextNews` or `ContextSeen` (context.rs
  120-158, 240-301).
- `INSTRUCTIONS` gains "The first compact context page lists the newest
  goal-wide findings by first line; read one in full with locust_event_show
  before relying on it." SKILL.md 54-71 says the same, plus "a first line is
  not the finding; it is not acknowledged." runtime-reference.md 92-95 gains
  one sentence. Regenerate the contract.

**Tests.** New: the first page lists 20 of 25 newest first, first lines cut
at 120 characters, the right names, no attempt result, the same list with a
task, none on continuations; listing marks nothing read (`limit: 1`,
`context_news` unchanged across reads); an unchanged list is not repeated on
a checkpoint; a finding withdrawn here is not current and one without its
payload has `line: None`, using S1a's helper. `"locust_event_show"` joins
the names `INSTRUCTIONS` must contain (mcp/tests.rs 293-297). Must pass:
context_views.rs 67, 381, 431 (as A1 adapts it), tests/context.rs and
`scripts/tests/test_skill_words.py`. Add a 2,048-finding compact row to
`context_read_performance` and record medians before and after.

**Exit criteria.** 25 findings show 20 and `total: 25`; two reads report the
same `context_news`; checks as A1. **Format impact.** One field, two types.

**Overlap with v2.** G1 sets `ContextBrief.halted` in the same literal
(host-safety plan 2139-2149); not both at once. J2 adds compact counts
(joinable plan 2697-2699). J5 hides pre-admission records from unread reads
(4027-4049); this list keeps current knowledge in a newcomer's view.

**Risks and notes.** Order follows arrival; withdrawal is local, so totals
differ; progress notes count. Confirm at build time that session state shows
whether a session has acknowledged anything.

### A4: Back after a break

**Goal.** An agent that lost its context, in a new chat, after a restart or
after a compaction, finds the attempts it holds and how to resume them
through the tools it already calls. Over MCP, `locust_status` already returns
each held claim with its goal, task, attempt and generation
([api.rs](../crates/locust-proto/src/api.rs) `Claim`, 1598-1604;
`Abilities.claims`, [level.rs](../crates/locust-proto/src/api/level.rs)
143), and the instructions already say to start with it
([mcp.rs](../crates/locust/src/mcp.rs) 41). A4 says what those claims mean,
and makes `locust status` print them for an agent on the CLI.

**Depends on.** R5.

**Changes.**
- `INSTRUCTIONS` (mcp.rs 41) gains two sentences after "Start with
  locust_status.": "Each entry in its claims is an attempt you hold; after a
  new chat, a restart or a compaction, read that task with
  locust_context_read view=full, then continue the work or report it. One
  locust_pending lists under held_elsewhere needs locust_attempt_takeover
  first." (views.rs 483-488.) SKILL.md 35 says the same.
- [cli/mod.rs](../crates/locust/src/cli/mod.rs) 489-504: for each goal with
  claims, read `board()` (628-637) into `Reader.tasks`.
  [presentation.rs](../crates/locust/src/cli/presentation.rs) `agent_lines`
  (706-748) takes the `Reader` instead of `voice` (caller at 786). Under the
  standing line (737): `Holds "TITLE" (TASK) · attempt ATTEMPT · generation
  N`, with the title through `quoted` (level.rs 321-330), the task through
  `Reader::task_id` (69-78), the attempt in full as `pending` prints it
  (479-486), since `--attempt` prefixes resolve against the whole feed
  (selectors.rs 227-279). In `Voice::Agent`, per claim `Context: locust
  context read --goal G --task T --view full --limit 20` (SKILL.md 43-45)
  and per goal `Pending: locust pending --goal G`. agents.md gains one fact.
- No API change. H1's start line tells a compacted chat the same.

**Tests.** Presentation tests for claim lines, quoted titles and a missing
board; `every_printed_command_parses_as_printed` (presentation.rs 2131) adds
an agent-voice status; `t2_flow.rs` runs a printed `Context:` line;
`strings_written_for_a_model_name_listed_tools_and_no_operation`
(mcp/tests.rs 287) passes with the new sentence.

**Exit criteria.** The printed `Context:` line runs; the contract is
unchanged; checks as A1.

**Format impact.** None.

**Overlap with v2.** G1 rewords halt (host-safety plan 2156-2160), G2 adds
status blocks (2793-2810), E1 replaces standing lines in an ended goal
(3229-3236), E2 changes `membership_action` (3914-3919). Each says what
claim lines do in its state.

**Risks and notes.** Titles other members wrote are printed; escaping stops
terminal tricks, not instructions. Other sessions' attempts of the same
agent print too (levels.rs 64-77). Whether agents call `locust_status` after
a compaction without being told is not yet observed; R7's early run records
it (see P).

### A5: Short IDs that cannot misfire

**Goal.** A5a: every printed ID is cut by `short`. A5b: an agent may type 8
or more characters of an ID a tool showed it in the same MCP connection;
anything else is refused with a fix.

**Depends on.** A5a: R5. A5b: A5a and R7's counts of ID errors.

**Changes, A5a.** `short` ([level.rs](../crates/locust-proto/src/api/level.rs)
549-577) is the only cutter. Fixed cuts move to it: level.rs 229, 237;
`member_key_prefix` (selectors.rs 118-131) and 111; presentation.rs 212,
830, 866-871; roles.rs 139;
[cli/workspace.rs](../crates/locust/src/cli/workspace.rs) 1027, 1148; the
fallback name at access.rs 182, callers.rs 84, requests/levels.rs 25.

**Changes, A5b.** A bridge-only `Shown` lives for one `locust mcp` process
(mcp.rs 280-431), never on disk.
- Targets: full IDs of each `byte_id!` kind (id.rs 174-252) except
  `IdempotencyKey` and `Signature`, from a typed walk of each result against
  new `response_schema()` and of refusal details against `refusal_schema()`,
  cached in `api/schema.rs`; `contract()` uses them unchanged. A value that
  does not fit, such as a `ctx:` receipt reference (context_receipts.rs
  197-205), is text.
- Blockers: every run of 8 or more hex in any shown string; a run blocks a
  prefix only if neither is a prefix of the other.
- `record` runs on completed tool calls (385-390). `expand` runs before
  `parse_call` (373), finds identifier fields from `operation_schema`,
  nullable `anyOf` included, and expands only on exactly one target of that
  kind and no conflicting blocker. `MAX_SHOWN_IDS = 65_536`; past it, only
  full IDs; nothing is evicted.
- Refusals use the CLI codes (selectors.rs 40-47): `not_found` ("No Locust
  tool showed a task ID starting with task:7f3a9c1e in this session. Pass
  the full ID, or read it again and use the ID shown.") and `invalid` for
  ambiguity with candidates.
- MCP ID patterns widen for `Surface::Mcp` only; the rule is stated once in
  `INSTRUCTIONS`. SKILL.md 87-88: "An ID may be shortened to its first 8 or
  more characters only if a Locust tool showed it in this session; give an
  ID from anywhere else in full. A task ID keeps its `task:` or `effect:`
  tag either way." runtime-reference.md 71-88 matches.

**Tests.** A5a: new `crates/locust/tests/identifier_cuts.rs` scans
`crates/*/src`, whitespace removed, for `to_string()[..` and
`.chars().take(8)`. A5b: expansion only to shown IDs; both refusals; a
ground title blocks and a target's own prefix in a title does not; kinds do
not cross; the walker reaches every identifier; receipts are text; the
ceiling; a no-task start passes through; a board-then-prefix call through a
two-connection fake built from `welcome` (mcp/tests.rs 100).

**Exit criteria.** The scan passes; over MCP an 8-hex shown prefix succeeds
and unshown, ambiguous or ground ones are refused; checks pass.

**Format impact.** None beyond MCP input schemas.

**Overlap with v2.** G2 edits level.rs, presentation.rs, access.rs and
`mcp.rs` text; E1 edits `render` and cli/workspace.rs; E2 edits the
`Member:` line (host-safety plan 3914-3915).

**Risks and notes.** An outside ID whose prefix matches another shown ID is
covered only by the skill rule. This answers ergonomics question 1: "8,
grown". A test pins the unverified schemars shape of `Response`.

### A6: Retiring a finding

**Goal.** An agent can publish a goal-wide finding that
replaces one of its own earlier ones, which leaves the current list. Nothing
is deleted.

**Depends on.** A3, R7's early run, J2's change set. Accepted by the owner
on 6 October 2026.

**Changes.** `ContributionPublish` and its body
([event.rs](../crates/locust-proto/src/event.rs) 465-471) gain `supersedes:
Option<EventId>`. The fold accepts it only when both records meet A3's
predicate and share an author. Concurrent supersessions all stay current,
with no tie-break ([master plan](master-plan.md) 87-94).
`current_findings` adds "not superseded by an Effective record". A6 rewrites
the `contribution.publish` summary (api.rs 902) and adds one skill sentence.
It touches `vectors.rs`, `every_body`, `fold.rs`, `projection.rs`, `claims.rs`,
the CLI argument, the simulator and the contract. No kind field.

**Tests.** A new vector; fold tests for same author accepted, another author
or an attempt result refused, two concurrent supersessions both current; a
context test that the replaced finding leaves the list and `total`.

**Exit criteria.** Checks pass with new vectors; A3 drops a superseded
finding.

**Format impact.** Signed bytes, in J2's step: inside 7 if nothing is
released before the door, else under J2's 8.

**Overlap with v2.** J2's set lands together (joinable plan 2105-2107), so
A6 is its own commit right after it.

**Risks and notes.** Progress notes can be superseded too.

### H: Hooks

**Goal.** Hooks bring Locust facts to an agent at moments it would not
think to ask, in every harness Locust sets up, with any model. MCP tools
stay the fallback and the only way an agent acts.
- H1: when a worker chat tries to end its turn while work waits for it, the
  stop hook keeps it going with one line naming the work, or waits for work
  without spending model tokens. A worker chat is one that started or took
  over an attempt, or called `locust_wait`. Any other chat that used Locust
  is held only for attempts it holds. After a compaction, the chat is told
  which attempts it holds.
- H2: after any tool call, a chat learns at once that its attempt was
  cancelled or taken over.
- H3: a chat's first Locust call and each compaction reset that session's
  read record, and `status` says whether an attempt's holder is active,
  waiting or closed, and which hooks its chat has reported
  ([agent ergonomics](../research/agent-ergonomics-2026-10-05.md) 525-546).

**Shape: one core, thin adapters,** as the owner requires.
- Locust's hook events are `start` (a chat opens or is compacted), `stop`
  and `tool` (after a tool call). Each returns nothing or one line; `stop`
  may return "keep going" with one line. That is the one output contract.
- The core holds every decision once (the stop rule, marks, which facts
  to print, the failure line), with no harness branch.
- An adapter is data and a thin shim: native events to Locust events, the
  envelope its harness reads a line or a block in, where its config entry
  lives and its shape, how a tool result is encoded, which payload fields
  mark a subagent, whether the payload can say no person types, its
  harness's hook time limits (set in the config entry where the harness
  allows), and whether the harness asks its person to trust new hooks. The
  dispatcher and setup serve every adapter through those facts, with no
  harness branch. A new harness adds only a table entry, a shim if it needs
  one, and its goldens.
- Output suits any model: plain facts and tool names in fixed wording, no
  vendor markup or model-tuned phrasing, no assumption about how a model
  treats system text. Adapters wrap the same line in any JSON needed.
- Where a harness lacks an event, Locust falls back: for `stop`, the agent
  calls `locust_wait`; for `tool`, `locust_pending`; for a compaction, A4's
  instruction to start with `locust_status`. The read record is then not
  reset (H3).

**Depends on.** H1a: A4, for order. H1b, H2: H1a. H3: H1a, A4.

**H1a harness choice, 7 October 2026.** R7's run text does not yet name
harnesses. Its first adapters are Codex and Claude Code, from different
vendors. Both wrap the same core events and output. The runtime receives the
daemon's `WorkItem.unattended` fact from `Goal::unattended`, which checks every
running attempt in the current round, including the caller's own principal.
A2's future no-task selection must reuse that predicate; the other-members-only
`attempting` list cannot establish it. No signed or synced record changes.

**Changes, H1a** (the core, its adapter interface, and adapters for the
harnesses R7's early run uses, named in R7's run text by P).
- `locust hook start|stop|tool --harness NAME`, a new `hook.rs` dispatched
  like `mcp` (args.rs 221-225), run through `locust-cli` (launcher.rs
  14-35). It refuses an owner (mcp.rs 253-255) and exits 0.
- Per-chat marks under `HOME/hook-marks/`, private like receipts
  (context_receipts.rs 55-98), only for Locust chats, locked while used.
- `stop` reads `status` (member, not halted: api.rs 1487, 1495) and
  `pending`: the session's claims and cancellations (views.rs 481-506), and
  for a worker chat reviews, deliveries and free tasks, by a predicate
  shared with A2. It waits on one connection per goal (callers.rs 27-32).
  `start` keeps this session's claims (`Claim.instance`, api.rs 1602).
- A pure `locust-adapter/src/hooks.rs` holds the adapter table, keyed by
  `Client` (config.rs 28-34), and registers and removes only its own
  entries. Each native event name and config path is confirmed when its
  adapter is built.
- Setup installs them by default under the MCP entry's rules (setup.rs
  600-749), in a v3 record (220, 476); a v2 one is refused with its
  recovery, and a hand-removed entry stays out. `LOCUST_HOOKS=off`
  silences them; `client run` and all but one qualification scenario set
  it (cli/client.rs 336-337; check_installed_clients.py 433);
  `real_models.py` merges each adapted harness's settings.
- agents.md gains a Hooks section (the default profile is the person's own
  client); SKILL.md after 245-246 explains `Locust:` lines.

**Changes, H1b.** An adapter, in its own commit with its tests, for each
other harness Locust sets up (config.rs 28-34 lists Codex, Claude Code,
Droid, Kimi Code and Pi). Kimi Code waits until setup has it (setup.rs
10-18).

**Changes, H2.** `tool` calls `wait` with `timeout_ms` 0 per goal, free when
unchanged (reading.rs 216-219), and names a cancellation, or a lost claim no
write of this chat explains.

**Changes, H3.** The daemon keeps in memory each session's last
request (requests/mod.rs 54-73), open chats and closed time. A local `session.chat`:
the bridge sends `Opened` at its first call and `Closed` at stdin EOF, the
start hook `Compacted`. `Opened` and `Compacted` delete the session's acknowledgments
and receipt cache, keeping the revision (context.rs 31-90, 348-350).
`DaemonStatus` (api.rs 1394-1408) gains presence and the hooks each chat
has reported, since each `locust hook` call names its event; A4's claim line ends
"active 10 s ago", "waiting" or "chat closed, not reported".

**Tests and exit.** Each rule as a pure function, alternating sets and a
declining chat included; titles never print; concurrent tool events print
once; the predicate matches A2's. The core runs against a fake harness
replaying events, and `status` then shows the hooks each chat reported.
Each adapter has goldens of its config entry and envelope. One conformance scenario (work waiting, a block, an ignored
block, a cancellation, a compaction) runs through every adapter and gives
the same Locust-level outcome; an adapter without an event shows its
fallback instead. For each adapted harness, in a profile under
/tmp, a waiting task blocks once, then goes through; `setup remove` restores
bytes; client checks pass or are reported missing; checks as A1.

**Format impact.** Nothing signed or synced; one local operation, inside 7.
**Overlap with v2.** G1, G2 and E2 edit `status`, not at once; E1 counts no
work in an ended goal.

**Risks and notes.** Beads #3451 pushed stops; guards: the stop rule,
facts only, "unless your owner asked you to stop", the harness's own
interrupt, and a rule that a bookkeeping write (a context acknowledgment) or
a note on a claim already shown never re-arms a block. Whether a waiting stop
holds a typed prompt is unverified per harness, so a stop waits only where no
person types: `LOCUST_HOOKS=unattended`, or the adapter's own signal (pi's
print and JSON modes); elsewhere an idle worker is told once and the next stop
goes through. Start and tool hooks give up after 5 s; only a stop waits.
Unverified: one bridge per chat.

**Silence rule** (owner, 7 October 2026). A chat that has not used Locust
never sees anything from Locust's hooks, whatever fails, and a subagent's
callbacks are silent. The failure line appears only in a chat already
associated with Locust, once per failure episode, never on every tool call.
One goal's failing read (a cancellation target, a stale baseline) skips that
goal, never the callback. Chats of one session share a record of the claims
their own terminal writes ended, so a sibling's completion is not a loss.

### S1: Store hygiene

**Goal.** Nothing removes stored content. Each store-open failure ends with
what to do and what starting fresh loses. A damaged or wrong-version folder
is never deleted.

**Depends on.** S1a: nothing. S1b: G1.

**Changes, S1a.**
- [proto store.rs](../crates/locust-proto/src/store.rs): remove
  `Commit::drop_blobs` (131-133), the loop at 343-345 and the case at
  1052-1109 with its call (564); literals at 588 and 681 lose the field; the
  doc (120-121) becomes "local writes take effect in order".
- [locust-store store.rs](../crates/locust-store/src/store.rs) `commit`
  (145-163) loses `objects::delete` (157) and the file removal (159-161);
  [objects.rs](../crates/locust-store/src/objects.rs) loses `DELETE` and
  `delete` (37, 105-122); files.rs loses `Files::remove` (128-132); lib.rs
  loses 23-25.
- locust-core: commit.rs 86, definitions.rs 74 and 137-139,
  content_graph.rs 577-583.
- A node-test helper that delivers another member's finding without its
  payload, modelled on content_graph_tests.rs 64-74.
- [crates.md](crates.md) 47-50: "Before the first release of version 7,
  signed bytes may change inside 7 if the vectors change in the same commit
  ([master plan](master-plan.md) 406-417). After a release, a change raises
  `PROTOCOL_VERSION`. Until the release gate exists, this is a convention."

**Changes, S1b.** `OpenError` Display
([error.rs](../crates/locust-store/src/error.rs) 26-45) keeps only what
happened; advice (31, 36, 40) moves to `store_open_failure(home, error)`
([daemon/mod.rs](../crates/locust/src/daemon/mod.rs) 267-278), which also
takes `Node::open`'s error (116-123) and G1's new failures. Exit codes stay
(failure.rs 18-34).
- In use: the lock refusal (home.rs 143-156) gains "Stop it, then start
  again."; `OpenError::InUse` says "Another program has HOME's database
  open. Close it, then start again."
- Other version: "This data folder was made by another Locust version, and
  no version converts it. Start the version that made it, or move the folder
  aside, do not delete it, and start with a new one."
- Damaged (naming the record when it can, events.rs 107): "Move HOME aside
  and do not delete it: it holds your keys and every record. A new data
  folder starts with no goals. Goals you host cannot continue from it, and
  goals you joined need a new invitation." Checked against G1's lost-goals
  wording.
- Failed: "Check that the disk has space and that you can read and write
  HOME, then start again."
- [operations.md](guide/operations.md) gains "If the daemon will not start"
  after Backups, linking to it; it describes no restore (G2 owns Backups,
  host-safety plan 2839-2846).

**Tests.** S1a rebuilds on content that never arrived: content_graph_tests.rs
638 (keeping its replaced-key half), 733 (now a definition arriving after its
binding), 826, and tests/context.rs 135. locust-store tests.rs 391 loses `transient` and
the drop of `large`; tests.rs 241 and crash.rs 106 lose the field;
`recovery_flush.rs` commits only genesis first (43-49) and adds the large
object in the armed commit (63-67). S1b: new
`each_store_open_failure_says_one_next_step` over the five `OpenError`
cases; cli.rs 2575 checks the sentence and a byte-identical `locust.db`;
cli.rs 2584 checks the lock sentence.

**Exit criteria.** `drop_blobs`, `objects::delete` and `Files::remove`
appear nowhere; checks and `check_docs.py` pass; the ignored macOS
`recovery_flush` test runs, or the report says it could not.

**Format impact.** None; `Commit` is in-process.

**Overlap with v2.** G1 adds `Commit.marks` and changes `open` at 25 sites
(host-safety plan 1853-1911); S1a goes before or right after, never during.

**Risks and notes.** Unverified: that opening a non-SQLite `locust.db`
leaves it byte-identical.

### S2: Long-goal measurement

**Goal.** The repository records replay and one-record apply times at about
10,000 and 41,000 records.

**Depends on.** Nothing.

**Changes.** [organization_performance.rs](../crates/locust-core/tests/organization_performance.rs):
sizes (140) become 16, 128, 512, 2,048 and 8,192 tasks (10,244 and 40,964
events at the new sizes). Batch modes (187-210) stop at 2,048, since each
batch refolds everything (goal/mod.rs 97-139). New `one_event_apply` clones
a goal built from all but the last event outside the timer (`Goal` is
`Clone`, mod.rs 54) and times `apply(&[last], ..)`; `sample` (124-135) gains
a setup argument. A new research note records machine, commit, profile and
CSV, and says whether one apply at 40,964 events nears J6's 250 ms `status`
bound ([joinable plan](joinable-farms-plan.md) 4825-4827); raw output goes
under `research/evidence/`, indexed.

**Tests.** The harness stays ignored; its checks (152-168) hold at every
size.

**Exit criteria.** All five sizes, healthy and forked, with
`one_event_apply`; `check_docs.py` passes. **Format impact.** None.

**Overlap with v2.** E1, R8 and J2 add fold work; rerun at R10 and J6.

**Risks and notes.** Measures `MemStore` and `Goal` only; SQLite's roughly
27 ms at 41k events is arithmetic (lib.rs 77-78). One parent for all tasks.

### S3: Release gates

**Goal.** From the first published build at 7, a build is refused if its
signed vectors differ from a released build with the same protocol version,
or its store layout differs from one with the same store version. Every
build opens the last release's data folder.

**Trigger.** The first published build at 7: a private release after R10 or
the door in J6 ([master plan](master-plan.md) 406-417). The session making
it lands S3 in the commit that records the publish. The 0.1.0 preview gets
no row.

**Depends on.** G1 and the owner's answer on versions.

**Changes.** Lines are re-checked at the trigger.
[build_release.py](../scripts/build_release.py) gains `format_digests`:
`protocol_sha256` over the vector constants (vectors.rs 127-151), keyed by
`PROTOCOL_VERSION`, and `store_sha256` over the normalized DDL,
`INLINE_MAX_BYTES` and G1's marks layout, keyed by schema `VERSION`; the API
version is left out. `verify_released_formats` refuses
`released_format_changed` against a tracked `scripts/released-formats.json`,
comparing each digest only with rows of the same number. The ledger joins
`RELEASE_INPUTS`; `--record-published` appends a row. A new
`write_release_fixture.py` has the released binary make a small goal (two
agents, a task, an attempt, a finding, a review, a shared file) and writes
`home.tar` (home and marks, modes kept), `answers.json` (recorded on an
extracted copy, times masked) and `versions.json`.
locust-store exports its schema version (schema.rs 6). packaging.md gains
the refusal and a paragraph on recording the row and replacing the fixture.

**Tests.** Script tests: a changed digest refused; each number raised alone
builds; `--record-published` appends once. New
`last_release_data_reads_the_same_or_is_refused_untouched`: same numbers,
every recorded field present; other numbers, exit 10 and every tarred file
unchanged (the lock file is still written, home.rs 159-162).

**Exit criteria.** Changing one vector without raising `PROTOCOL_VERSION`
is refused; the fixture and script tests pass.

**Format impact.** None of its own. **Overlap with v2.** One of J6's gates
(master plan 404).

**Risks and notes.** Formation documents, invitations, manifests and sync
frames are not gated.

### J1+: Last sync refusal (addition to J1)

**Goal.** Each peer line in `goal status`, and its JSON, shows the last
refusal that peer sent on an exchange this daemon opened, and when. A
completed sync clears it; a restart forgets it.

**Depends on.** J1 ([joinable plan](joinable-farms-plan.md) 1983-1995) and
E2.

**Changes.** Lines are re-checked against J1's rewrite.
[driver.rs](../crates/locust-core/src/sync/driver.rs) `Report` (51-66)
gains `received: Option<Refusal>`, filled in `end_dialed` (464-506), since
`Ended::Refused` covers both directions. In
[peers.rs](../crates/locust-core/src/node/peers.rs) `exchange_ended`
(269-297), one in-memory map `(GoalId, EndpointId) -> PeerRefusal { reason,
at_ms, presented_join }` replaces E2's `Node.refused` (host-safety plan
3895-3905) in the same commit; E2's join rule and J1's backoff read it.
`PeerView` (api.rs 1564-1572) gains `last_refusal`. The peer line
(presentation.rs 1028-1040) gains ` · last refused: WORDS, at TIME` from
`refusal_words`, covering every variant, `CatchingUp` included.

**Tests.** Only refusals the peer sent show, until the next completed sync
or a restart; refused exchanges commit nothing; E2's tests pass against the
map; the CLI shows text and JSON.

**Exit criteria.** Checks pass; in the simulator a computer removed while
away shows `not_a_member` on every peer line.

**Format impact.** API only.

**Overlap with v2.** G2 rewrites the status blocks (host-safety plan
2793-2800); do not build both at once.

**Risks and notes.** `NotAMember` also answers an unknown goal (sync.rs
266-268), so no text says "removed".

## Changes to how this repository tracks work

P is one docs commit in Lane B, now.

- **One place for phase state.** The master plan's status paragraph (3-6)
  becomes "Phase state is in the build-order table." The pieces table drops
  counts and states (330-333). Built rows read "Built (`hash`)": R1
  `65aecf1`, R2 `48120a4`, R3 `3196be8`, K1 `cb1acaa`, R4 `8c086c1`, R5
  `9c337df`, R6 `e46450f` and `9499cde` (unmarked today, 358). The status
  lines of the [roles plan](roles-and-permissions-plan.md) (3-5),
  [host-safety plan](host-safety-and-ending-plan.md) (3-5) and
  [joinable plan](joinable-farms-plan.md) (3-18) point to the table. Each
  build session edits only its own row.
- **Numbered findings, a convention,** as in the
  [R5 review](../research/v2-phase-r5-review-2026-10-06.md).
- **R7's early run** (roles plan 4064-4083): one task is open before the
  second admission, as now. Before the run the host's agent publishes three
  to five findings still open in this repository when R7 starts, as
  goal-wide findings, so A3 shows them and the non-host agent's second task
  can come from one. Each agent has its own profile home, worktree and build
  directory; two agents share one daemon home, the third uses another, and
  the note says which. The run text names each agent's harness; H1a builds
  those adapters. Results land by hand outside the run. The note also
  records tasks with more than one attempt and the work wasted, starts
  despite an "After task:" line, ID errors and prefix refusals, and each
  compaction or restart an agent went through, with whether it found its
  held attempt again, and each hook line and its effect. One agent runs
  with `LOCUST_HOOKS=off` and still calls `locust_wait` first; the others
  may answer a stop-hook line instead (roles plan 4072-4073, 4135; R10
  4918).
- **Hand-offs,** one sentence each in the v2 phase: E1 decides the
  no-task start in an ended goal, prints no claim lines there and replaces
  A1's two scope sentences; G2 lists claims while catching up; E2 notes that
  J1+ folds `Node.refused` into its map; J1 lists J1+; J2 and J5 name A3's
  list; J2 names A6's slot; the Versions line as above.

**Exit.** The table is the only phase state, each built row has its commit,
and `check_docs.py` passes.

## Not in this plan

From [the research note](../research/beads-assessment-2026-10-06.md)'s
rejected and deferred items:

- Goal bundle export and import: removes no person step.
- A Summary naming the events it covers: no agent writes one.
- Importing a beads export; Locust as a beads backend.
- `locust store verify` and a storage report (idea 6).
- Reference-aware reclaim (idea 7): no growth is measured.
- Board paging, surface budgets, an oversized-page refusal (idea 1): only if an oversized page is recorded.
- A hashed start tiebreak and a new empty-answer type (idea 2).
- A schema-based field checker, plain-status refusal lines, a finding-ID checker.
- Waking a closed chat (agent-ergonomics 564); hooks for `client run`.
- A task ordering edge (idea 10): asked only if R7 records early starts.
- A local fold checkpoint (idea 9): raised only if S2 nears J6's bound.
