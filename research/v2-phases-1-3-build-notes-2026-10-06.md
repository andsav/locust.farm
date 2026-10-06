# Locust v2 phases 1–3 build notes

Status: implementation in progress, 6 October 2026. This note records the
implementation of only phases 1, 2 and 3 of the
[roles and permissions plan](../docs/roles-and-permissions-plan.md) and its
[removals and checks](../docs/roles-and-permissions-plan-details.md), against
the current checkout. The [master plan](../docs/master-plan.md) supplies the
owner's principles. No phase after phase 3 is in this work.

## Starting point and owner corrections

The checkout started clean at `986c18d`, after the plan's source snapshot
`cfb5b45`. Source is re-read for each phase. The plan files remain owned by
another session and are not edited here.

- Every phase updates executable recipes and script harnesses that use an
  interface it removes. The plan's temporary breakage until phase 6 is not
  accepted. Each phase runs the CI executable manual recipes and Python
  helper tests as well as the Rust and documentation checks.
- `checkout.register` stays an agent request for its own checkout. The
  person's folder command is `workspace connect` (spelled `workspace
  checkout` before the phase 2 rename). An agent may register its own new
  folder and make its first file proposal without waiting for a person.
  Capturing local files still requires the acting agent's own registered
  checkout; a composition reads no local folder and the person's own
  capture is exempt.
- Confirmation binds the arguments and state shown by the plan and changed
  by the act. It does not bind every command to `governance_head` or
  `current_rules`; `rules bind` retains `current_rules`.
- Joining defaults to `auto`, like adding a local agent. Join plans list
  all three levels and mark the selected one. Only the local-collaboration
  recipe intentionally walks through `ask`. Missing local level records
  must not turn members into `read`.
- A task allowance lasts until the host revises the task or the person
  revokes it. A task becoming finished only hides the allowance while the
  task cannot be taken; reopening on the same round preserves it. The
  printed sentence is `AGENT may take "TASK" in "T" until the host revises it.`
- No second rules table (`Goal::rules_allow`) is built. Phase 3 must use the
  existing trial application of candidate records and structured replay
  refusal reasons, before the level check. Level, content and private-path
  checks remain outside the trial. Refused records are neither stored nor
  sent.

## Phase 1

The host/owner split is implemented: host requests require the owner on the
hosting daemon and name no acting agent; create, join and leave name a local
agent. Daemon-wide goal-management grants, viewer credentials, the separate
invitation-join API and the administer grant are removed. Invitations require
an expiry and pending invitations can be revoked together. The API and store
marker are 7; the signed protocol stays 6.

Scripts and all four executable recipes have been rewritten in this phase
for the interfaces it removes, rather than being left broken until phase 6.
The two formation-explanation sentences that the plan explicitly leaves for
later remain identical to the site's TypeScript explanation. No site source
file needed a phase 1 edit.

### Checkout correction against today's code

The original `checkout.register` accepted a complete `Checkout`, including
an arbitrary folder path and its device/inode identity. The CLI copied the
files; the daemon only recorded that metadata. Leaving that request intact
would not implement the owner's correction that the daemon makes the
agent's new folder.

The implementation therefore separates the two requests:

- Agent `checkout.register` takes a checkout identifier, an optional
  accepted revision and optional task/attempt bindings. It accepts no
  folder path. A filesystem capability supplied by the daemon shell
  materializes the files in its private checkout area, after the core
  validates membership and the requested shared tree.
- Owner `workspace.connect` registers the explicit folder that the
  person's CLI command materializes for a named local agent. The CLI
  spelling remains `workspace checkout --destination` in phase 1 and
  changes to `workspace connect --folder` in phase 2.

This adds an owner-only operation beyond the plan's operation count and
exposes the agent's checkout request as an MCP tool. The regenerated contract
has 95 operations and 58 tools, rather than the plan's 94 and 57.
Filesystem I/O stays in the daemon shell; the core receives the capability
as it receives storage and entropy. The agent's first proposal can use its
own new folder without an owner connecting one. This phase retains the
existing requirement for an initialized shared workspace with an accepted
revision; automatic first-tree acceptance and default policy changes belong
to later phases and are not introduced here.

### Checks and evidence

Passed: `cargo fmt --all --check`, `cargo clippy --locked --workspace
--all-targets -- -D warnings`, `cargo test --locked --workspace` (955 passed,
14 ignored), `python3 -m unittest discover -s scripts/tests` (291 run,
3 skipped), `python3 scripts/check_docs.py`, `python3 scripts/check_formations.py`,
and `python3 scripts/check_documentation.py --binary target/debug/locust
--timeout 60` (shared-workspace-loop, local-collaboration, private-authoring,
separate-goal-export). The phase 1 site parity gate `npm test` passed all
208 tests. No site source was edited, so its lint/check/build gates were not
required for this phase.

The checks use fresh test homes; there is no migration from store marker 6. The frozen signed-protocol vectors
and unsupported-store-marker test remain unchanged.

The real-daemon integration test
[managed_checkout.rs](../crates/locust/tests/managed_checkout.rs) registers an
agent's own daemon-created folder and proposes files from it without an
owner connecting it. A final review found that an overlapping owner-connected
ancestor could be changed before registration refused it. A side-effect-free
canonical destination lookup now allows the core to reject that overlap before
file creation; regression tests cover this and symlinked homes. Core tests cover owner-only create/join/leave, all nine
host operations on a remote copy, grant-free host acts and admission,
invitation expiry and revoke-all, capture ownership and replacement bases.

The 14 Rust ignores are the two long one-way Iroh idle-interval tests, MCP
schema-cost measurement, installed Claude/Codex overlay checks, the long and
single-seed simulation runners, context and organization performance
measurements, public Mainline DHT and multicast discovery qualification, the
crash-test child entry point, the recovery child entry point, and the macOS
flush-interposer recovery test. Child entry points are exercised where their
parent tests invoke them. The three Python skips are the farm-seed signing
checks that require the separately documented `uv` signing-test environment.
No ignored qualification or performance run was claimed. Earlier Clippy,
fixture-transition and generated-contract/site-parity failures were fixed;
there are no remaining failing phase 1 gates.

## Phase 2

The command grammar uses global `--agent` for one of the person's agents
and `--member` for a goal member. Commands that share information or cannot
be undone in one command use the shared plan/confirm mechanism. The raw
`call` command remains an exact request. Scripts and executable recipes are
updated alongside each removed spelling. The onboarding prompt and
first-contact text change together, with their byte-for-byte parity test.

The global agent option enforces its owner requirement during dispatch,
rather than through Clap's `requires` on a global argument. The latter
incorrectly rejected a leading `--owner` with `--agent` after a subcommand;
both flag positions now work. This changes the implementation detail, not
the required owner authority.

Confirmation re-reads the displayed state before acting. Rules binding and
task revision send the reviewed expected rule/round, so a concurrent change
cannot be silently selected by another read after confirmation. Invitation
expiry parsing also rejects non-ASCII invalid input without panicking.

The person's `workspace connect --folder` continues to send the owner-only
`workspace.connect` API introduced in phase 1. It does not call the agent's
`checkout.register`, despite that spelling in the phase 2 plan. That is the
same owner correction recorded above, carried through to the renamed CLI.

A real-daemon regression checks that an unrelated rules binding does not
invalidate an invitation plan when its displayed state has not changed, and
that issuing the invitation makes that same plan identifier stale. The plan's
pending-invitation count alone returned to its old value after redemption or
revocation, making a used plan valid again. The invitation review therefore
also shows and binds the total issued count. Revoking the ticket cannot make
its old confirmation valid again; the regression covers that sequence. This
adds visible changed state rather than a hidden governance dependency.

Deleting `cli/local_members.rs` exposed source links to that module in the
protected plans. Keeping dead source or editing those plans would violate the
owner's constraints. The [documentation checker](../scripts/check_docs.py)
therefore validates missing `crates/` targets only for four explicitly pinned
historical plan documents against the source commits named in their introductions.
It requires the exact historical blob; an unavailable commit, invented path,
missing documentation page, or a broken current-guide link still fails. The
[checker tests](../scripts/tests/test_check_docs.py) cover these boundaries.
The CI and release-build checkout steps fetch history for that verification.
The historical links remain historical; this does not claim that the removed
files still exist in the current checkout.

### Checks and evidence

Passed: `cargo fmt --all --check`, `cargo clippy --locked --workspace
--all-targets -- -D warnings`, `cargo test --locked --workspace` (976 passed,
14 ignored), `python3 -m unittest discover -s scripts/tests` (297 run,
3 skipped), `python3 scripts/check_docs.py`, `python3 scripts/check_formations.py`,
and `python3 scripts/check_documentation.py --binary target/debug/locust
--timeout 60` (all four executable recipes). In `sites/locust.farm`,
`npm run lint`, `npm run check`, `npm test` (208 tests) and `npm run build`
passed. The regenerated command contract has the required confirmation
flags and no removed command or flag; the source cleanup search is empty.

The Rust ignores and Python skips are the same qualification, performance,
child-entry and signing-environment cases listed for phase 1. Earlier
Clap flag-ordering failures, fixture changes, a Clippy argument-count failure,
and a recipe run invalidated by a concurrent binary rebuild were repaired;
the final gates have no failures. The frozen-binary recipe rerun passed.
The new tests exercise owner agent inference, daemon denial for an agent's
person-only command, immediate revoke and raw requests, stale plan refusal,
member selectors, subtask wording, stdin joins without prompting, onboarding
name stability, and workspace plans that perform no pre-confirmation file
I/O. Workspace race regressions keep the reviewed revision/epoch pinned.

## Phase 3

Not started.

## Verification boundary

Checks above describe only their phase and local automated environment.
Local automated checks do not establish real-model or two-computer behavior.
