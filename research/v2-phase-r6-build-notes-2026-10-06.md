# Locust v2 R6: documents, site and scripts

Status: implemented; required checks passed. The supplemental LAN harnesses
remain failing, and two corrected-plan behaviors await the concurrent Rust fixes
listed below. This is build-order phase 7, the roles plan's Phase 6. No Rust
behavior is changed by this work.

The scope is [Phase 6 of the roles plan](../docs/roles-and-permissions-plan.md),
its [companion](../docs/roles-and-permissions-plan-details.md), and the owner's
answers in the [master plan](../docs/master-plan.md). The preceding records are
[R5](v2-phase-r5-build-notes-2026-10-06.md) and
[the R4 fixes](v2-phase-r4-fixes-2026-10-06.md). Those plans were read, not edited.
The checkout started at `fa142b2`; another session owns the concurrent Rust fixes.

## What changed

- [Concepts](../docs/guide/concepts.md) now opens with **Who may do what**: the
  shared board, independent work, exact-result approval and the only-member
  exception, followed by the host, roles and levels. It defines the host as a
  person and members as agents. **Levels on your computer** and **Only you**
  explain auto by default, ask and read, task allowances, confirmation and undo.
- The collaboration, sharing and shared-tree guides show the person's commands,
  seven-day invitations, a join at auto, first files without approval, later
  changes under the tree's rule, fresh agent folders, and explicit local updates.
  The executable recipes retain their already-migrated interfaces.
- Formation and authoring guides list the six presets in contract order with
  **What waits on one member**. They explain automatic reviewer admission,
  binding-time reviewer assignment and its exceptions, roles retained for older
  work, a role's fixed kind, one member's named-check claim, latest reviews and
  decisions that retain earlier evidence.
- The glossary, overview, installation, agents, operations, farm and runtime
  pages use the same terms. First contact retains its exact quoted setup prompt;
  its surrounding text links to **Only you**. Status and the engineering pages
  remove superseded descriptions of levels and shared files. Current user text
  describes the host without exposing the goal's signing key.
- The [manual inventory](../docs/manual.md) and [site manifest](../docs/site.json)
  use `concepts/levels` and `concepts/members-roles` with real anchors. The start
  and how-it-works pages lead with agents organizing work and explain levels.
  `llms.txt` and generated command-reference prose follow. The formation prompt
  and its fixed-text contract change together.
- The [installed skill](../skills/locust/SKILL.md) uses these terms, describes
  current shared-file defaults, and teaches board habits: prefer unheld work,
  post before reading competing results, read standing rejects and approve only
  checked work. It explains when there is no review to wait for and what a late
  reject cannot undo.
- The [demo](../scripts/live_farm_demo.py) publishes a contribution summary as
  positional text and asks another member to approve the exact file proposal.
  It also restores Codex's actual `--ask-for-approval` flag. Prompt and launch
  tests cover both failures.

## Requested review findings

| Review finding | Result | Evidence |
| --- | --- | --- |
| [R5](v2-phase-r5-review-2026-10-06.md), 44: nonexistent summary flag and review/declaration ambiguity in the demo prompt | Fixed | `Demo.launch` uses positional summary text and the exact proposal's `review record`; [demo tests](../scripts/tests/test_live_farm_demo.py) reject the old flag and ambiguous instruction |
| R5, 46: formation design omits counting-role assignment on `rules bind` | Fixed in the documentation, including the later plan correction | [Formation design](../docs/formations.md) and [guide](../docs/guide/formations.md) describe the same-commit assignment, `--no-role`, undo and the earlier-rule exception, with code/test links |
| [R4](v2-phase-r4-review-2026-10-06.md): skill and apply guide describe the earlier shared-file default | Fixed | The skill and [apply guide](../docs/guide/apply.md) describe the goal's rule by default, explicit workspace overrides, binding changes and approval-free first files |
| R4: authoring guide asks for a redundant reviewer assignment after joining | Fixed | [Authoring](../docs/guide/formation-authoring.md) explains automatic reviewer admission and uses `role give` only for deliberate holder changes |
| [K1](v2-phase-k1-review-2026-10-06.md): glossary and linked pages call the host a member or the starting agent | Fixed | Help, concepts and collaboration consistently distinguish the person from the host's agent |

## Departures and precise readings of the plan

1. The user forbids changes under `crates/`. The requested new Rust vocabulary
   test is therefore a Python helper test,
   [test_skill_words.py](../scripts/tests/test_skill_words.py). The existing Rust
   test that parses the skill's commands and flags remains unchanged and runs
   in the required full suite.
2. The literal search pattern `viewer` also matches the required word
   `reviewer`. The vocabulary tests match whole words, allowing reviewer while
   rejecting viewer and the other retired terms. The old `--yes` spelling in
   the unchanged first-contact test is a negative assertion, not a command;
   it is intentionally retained. The first-contact quote remains byte-for-byte
   equal to the setup prompt, including its generic reference to tool permissions.
3. Shared trees follow the goal's completion rule by default. An explicit
   formation workspace policy can name another member to accept changes and
   a separate completion rule; initialization preserves that policy, and
   `--completion` can override the completion part. Binding a formation with no
   explicit workspace policy moves an active tree to the new goal rule and the
   host's agent. The guide keeps these implemented exceptions rather than saying
   all trees always use the goal's rule or the host's agent. See
   [workspace CLI](../crates/locust/src/cli/workspace.rs) and
   [goal requests](../crates/locust-core/src/node/requests/goals.rs).
4. Revoking an invitation applies at once but prints **Invite again:**, not
   **Undo:**. It makes a new ticket; it cannot restore the old one. The guide
   spells out that exception to the two-tier description. See
   [invitation_revoke](../crates/locust/src/cli/only_you.rs).
5. Leaving currently records a leave request, stops new local work and removes
   the local level and allowances. The host must still remove shared membership;
   leaving does not terminate an already-running process. The guide does not
   claim the planned automatic removal or whole-goal ending is built. See
   [goal_leave](../crates/locust-core/src/node/requests/goals.rs).
6. The demo's misspelled `--agentk-for-approval` was an additional executable
   drift found while fixing its prompt. Local `codex --help` confirms
   `--ask-for-approval`; its launch test now checks that actual argument.
7. The read-level refusal paragraph in the skill needed a small follow-up to R5:
   only ask yields a task waiting for an allowance; read needs a level change.
   Other refusal guidance and the two parser-pinned joining/folder instructions
   retain their contract. The setup strings and `guide.test.ts` are untouched.
8. The more recent correction to automatic reviewer assignment is described as
   requested by the user: if any earlier rules let one holder of the counting
   role act alone, `rules bind` keeps its holders and explains the earlier use.
   This preserves review on older tasks. Final source readback is recorded below.

## Verification

The implementation commit is `e46450f417bc657224c707c5dc4dac649fcb7d9e`. It was
HEAD when exported with `git archive` for the final build. The immutable source
copy is `output/r6-source-e46450f417bc`; its `target` links to an existing build
cache. The binary was freshly built there with `cargo build --locked -p locust
-p locust-farm` and `LOCUST_BUILD_COMMIT` set to that exact commit. The live
checkout's unfinished Rust changes were excluded. All commands below use
`/opt/homebrew/bin/python3` and put `/opt/homebrew/bin` first in `PATH`.

- Locust SHA-256: `3a2bf28d29f93d2ec54b02afc35e0fe6d1b4d6352cd57ec463304f24a56e2a21`.
- Farm service SHA-256: `6db330befcd25d65a150f17c79d28c7eef208181ff6bb34ed955201ff15b8a29`.
- `locust --version`: `locust 0.1.0 (e46450f417bc657224c707c5dc4dac649fcb7d9e) api 7 protocol 7`.

| Required check | Result |
| --- | --- |
| `scripts/check_docs.py`, with new documents and their index staged | Passed, including this note and its index entry |
| `scripts/check_documentation.py --binary target/debug/locust --timeout 60` in the exact source copy | Passed all four recipes: shared-workspace-loop, local-collaboration, private-authoring and separate-goal-export |
| `scripts/check_formations.py` in the exact source copy | Passed; six examples and generated conformance/runtime artifacts match |
| `python3 -m unittest discover -s scripts/tests` in the exact source copy | 301 tests, three existing skips, no failures |
| Site `npm run lint`, `npm run check`, `npm test`, `npm run build` | Passed; zero type-check errors/warnings and 222 passing tests |
| `cargo fmt --all --check` in the exact source copy | Passed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` in the exact source copy | Passed |
| `cargo test --locked --workspace --no-fail-fast` | Working-checkout rerun: 1,145 passed, 14 existing ignores. Exact-commit parallel run: one intermittent reconciliation failure. Exact-commit full rerun with `RUST_TEST_THREADS=1`: 1,145 passed, 14 existing ignores, no failures |

The failing Rust test is
`daemon::reconcile_tests::a_diverged_author_log_reconciles_between_two_real_daemons`.
It failed once in the live checkout and once in the exact source copy at the
first publication after a daemon restart (`reconcile_tests.rs:409`), with
`conflict: the candidate cannot be applied yet`. Its focused rerun passed, as
did a full working-checkout rerun. The failing fixture has only waited for the
member and title before the restart; the cause has not been established. No
Rust test or runtime was changed here. The exact-commit full suite passed with `RUST_TEST_THREADS=1`, which runs every
enabled test. Serial success does not erase the intermittent parallel failure.

The retired-command search covers the plan's directories and both command and
quoted-argument spellings. Its sole match is the intentional `--yes` negative
assertion in the unchanged setup-prompt test. The skill vocabulary test and
the new guide-prose test pass. The public homepage and formation page each
returned HTTP 200 without credentials on 2026-10-07 UTC, supporting removal of
the obsolete password-protection descriptions; no site was deployed here.

## Supplemental harnesses

These ran on the fresh working-checkout build copied before further edits, with
version `1b21c3f7cfad-dirty` and SHA-256
`00fdb99af96c0b3082d8858e39ff28c8c557699ee021126a425efe8bb4e5169f`.
They are separate from the exact-commit recipe and Rust results above. The
working tree included this phase's skill and the other session's unfinished
Rust work; this is not qualification of an immutable source commit.

| Check | Result and retained finding |
| --- | --- |
| `check_t1.py --network local` | Failed after ten successful checkpoints. Membership, task result, review and initial replication succeeded. After the host stopped and the third daemon restarted, the two remaining members did not exchange their notes within 60 seconds. |
| `check_operations.py` with its default network | Passed 35 checkpoints. It exercised accepted file transfer from the retained replica with the original source offline, interrupted large transfer, competing exact proposals under an explicit declaration rule, integration, cancellation, restart, epoch rotation and removal. |
| `check_farm.py` with the freshly built farm service | Passed 25 checkpoints: consent, publication, evaluated result, daemon/service restarts, suspension/resumption and durable deletion. |
| `simulate_machines/run.py --quick` with its default LAN profile | Failed all three scenarios. `two-machines-complete` and `three-machines` timed out exchanging notes with the host offline. `crash` retained and shared the acknowledged note but timed out waiting for the restarted member's peer connections. |

The failures are not hidden by changing network profiles, reducing the checks
or editing Rust. They leave the plan's complete supplemental harness exit
criterion unmet. The successful default-network operations run is not a pass
for the local-network T1 or simulation cases. None of these runs represents
separate physical computers.

Local logs and transcripts are disposable reproducing detail; the results,
source boundaries and failure reasons above are the tracked record. They are
under `output/r6-*.log`, `output/t1-local/20261007T024753Z-de10d248/`,
`output/operations/20261007T025013Z-a8ded071/`, `output/r6-farm.json/` and
`output/r6-sim/20261007T025014Z-7917/`.

## Concurrent behavior readback

Read again immediately before the final documentation commit, with `3d978f7` at
HEAD. These are source observations of the current checkout, separate from the
exact `e46450f` binary qualification above.

| Behavior | Readback |
| --- | --- |
| Task titles | `3d978f7` adds `Entry::task_title` in [entry.rs](../crates/locust-core/src/node/entry.rs), taking the opening text's first line. [quoted](../crates/locust-proto/src/api/level.rs) cuts at 60 characters, escapes quotes/backslashes and makes hidden controls visible. The current presentation uses it. |
| Valid names | `1b21c3f` makes [is_member_name](../crates/locust-proto/src/event.rs) refuse an 8–64 character ASCII hexadecimal name that can be confused with a key or prefix, in addition to the existing length, outer-space and control-character checks. The guide does not invent an alternative name rule. |
| Status at read | Still pending in the other session: [waiting_for](../crates/locust-core/src/node/views.rs) excludes auto, so read can still receive the ineffective `allow` suggestion. The guide and skill correctly say that read needs a level change, and describe the allowance route for ask. No Rust fix was made here. |
| Historical use of a counting role | Still pending in the other session: [rules_bind](../crates/locust-core/src/node/requests/goals.rs) assigns all active members whenever `!no_role` and a counting role exists. It does not yet check earlier rules that let one holder act alone. At the user's explicit direction, the formation pages describe the corrected `fa142b2` contract. That earlier-rule exception must not be treated as implemented or tested by this phase. The existing assignment and `--no-role` behavior are implemented and covered by the passing tests. |

The task-title and name commits belong to the other session. The remaining two
rows are recorded rather than changed because the user expressly excluded
`crates/` and assigned those fixes to that session. No protected plan was edited.

## Evidence boundary

The demo prompt is unit-tested; no paid model, installed client, signed package
or public deployment is run for this phase. Local daemon checks are one-computer
checks and do not prove two-computer discovery or real-agent self-organization.
No Rust behavior, protected plan, release, push or history rewrite is part of
this work. The pre-existing research-index reorder is preserved separately.

## Commits

- `e46450f`: the 38-file implementation and regression checks.
- `1b21c3f` and `3d978f7`: concurrent name and title fixes, read back but not
  authored by this phase. Only `1b21c3f` is an ancestor of the tested implementation.
- The commit containing this note adds the verification record and its index
  entry, and removes an overbroad claim that every documented role-binding
  behavior already has code enforcement. Its hash is reported with the handoff.
