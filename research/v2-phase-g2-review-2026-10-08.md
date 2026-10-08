# Review of phase G2 as built

Status: research finding, 8 October 2026. An independent review of phase G2
of the [host safety and ending plan](../docs/host-safety-and-ending-plan.md)
("Catching up in the person's words", build-order phase 9) at `a2cde40`: the
code commit `daa1dac` and the notes commit `a2cde40`, read against the plan's
G2 section, its terminal texts G-1 to G-6, the
[companion](../docs/host-safety-and-ending-plan-details.md), the
[G1 build notes](v2-phase-g1-build-notes-2026-10-07.md) and the
[G2 build notes](v2-phase-g2-build-notes-2026-10-08.md).

Every check was re-run on `a2cde40` in a clean worktree:

- `cargo fmt --all --check`: clean.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 39 suites, 1,366 passed, 0 failed,
  12 ignored.
- `cargo build --locked -p locust`, then `python3 scripts/check_formations.py`:
  the contract verifies.
- `python3 scripts/check_documentation.py --binary target/debug/locust
  --timeout 60` (Python 3.13.16): all four recipes pass.
- `python3 -m unittest discover -s scripts/tests`: 335 run, 10 skipped, OK.
- `python3 scripts/check_docs.py`: only the five lines that fail on the base
  commit `665fbe7`.
- `sites/locust.farm`: `npm ci`, `npm run lint`, `npm run check`, `npm test`
  and `npm run build` pass.
- The four drills in `catching_up_tests.rs`, run three more times: all pass,
  each run in under two seconds.

Findings 1 and 2 were demonstrated with scratch tests, and finding 4 with a
command. None of these was committed. Severities are the brief's:
**blocking** stops the work from counting as done; **major** is wrong
behaviour a person or an agent would meet, or a check that passes without
checking; **minor** is real but rare, cheap, or confined to text and tests.

## Summary

G2 does what the plan asks. The fifth side of the refusal is worded as
mockup G-4 gives it, in both voices. The status block reads as G-1, G-2 and
G-5, and the three short lines as G-6. `goal continue` shows the plan of
G-3, asks for a yes, binds a plan id with no clock reading in it, and prints
no Undo line. Commands that would sign with a held key show no plan, and the
`goal invite` plan warns. `next_place` takes the body, and every path that
signs reaches it with the act the plan's table gives. The drills run two real
daemons through the real command line and check each exit criterion with
exact text. No text names the goal's own key. The guides, the skill and the
MCP instructions say what the plan lists. Nothing blocks, and nothing is
major.

Five minor findings remain. The most useful is 1: an agent whose own records
are missing too is told its hold "catches up by itself" while `status` lists
the goal under "Waiting for you". The other four are a `goal add` refused
although it would sign nothing (2), some words of the `goal continue` plan
(3), a doctor check that disagrees with the store about a symlink (4), and a
guide that does not name the new side (5).

## Findings

### 1. An agent with its own missing records reads "catches up by itself" while the goal waits for the person

Minor. [guard.rs](../crates/locust-core/src/node/guard.rs) lines 165-177
(`hold_view`), and the same choice in `holding` in
[only_you.rs](../crates/locust/src/cli/only_you.rs) lines 531-541.

**What is wrong.** A refusal carries one `GuardView`, and its fix ("It
catches up by itself" or "It waits for you") follows that view's
`waits_for_you()`. `hold_view` gives an agent the governance key's view only
when the agent has no `Behind` hold of its own. On the host's computer, an
agent can have its own `Behind` hold while the governance key's `Behind` hold
waits for the person: every other computer has answered, and none sent the
host's records. The agent's own view has `by_host: false`, so the refusal
says the hold ends by itself. It does not. `guard_settle` (guard.rs lines
351-355) gives up an agent's record only when the governance key is not held,
so the agent stays held until the person runs `goal continue`. The plan says
that where only the person ends a hold, the person's voice reads "It waits
for you" ([plan](../docs/host-safety-and-ending-plan.md) lines 2744-2749). An
agent held because the governance key is held carries that key's view (line
2758).

**How it shows.** This is the main case the "Waiting for you" rule exists
for. The host's computer signs records with both keys while every member is
offline. Its disk then fails, and it is restored from a copy. A scratch test
in `tests/guard.rs` did that: a shared goal, a copy, the member down, an
admission signed by the governance key and a finding by the host's agent, a
start from the copy, then the member back and `settle`. Its output, with the
keys cut (the fixture names the host's agent `host`):

```text
LISTED: Some([GuardView { key: 7a562355.., by_host: false, reason: Behind { held: 0, signed: 1 }, heard: [48484848..], waiting: [] },
              GuardView { key: f24823b6.., by_host: true,  reason: Behind { held: 4, signed: 5 }, heard: [48484848..], waiting: [] }])
AGENT: host can't post to this goal: the Locust data here is older than what this computer signed in the goal (this computer). It catches up by itself, or host's owner can continue without waiting.
PERSON: host can't post to "Guarded": ... (this computer). It catches up by itself. To go on without waiting: locust --owner goal continue --goal e223f041
```

The goal is listed under "Waiting for you", and its status block says "No
computer that answered sent them. Waiting for you." The agent's refusal
contradicts both. The agent is told it will be released and its owner need
not act. `goal leave` and `farm consent` on the CLI make the same choice
through `holding`.

**Fix.** Where this daemon hosts the goal and the governance key is held,
give every agent's refusal the governance key's view whenever that view
`waits_for_you()`. Better still, word the fix from the goal: when any hold
there waits for the person, say so. Make the same change in `holding`. Add
this scenario to `only_a_hold_no_other_computer_can_end_waits_for_the_person`.

### 2. `goal add` of an agent already in a held goal is refused, though it would sign nothing

Minor. [only_you.rs](../crates/locust/src/cli/only_you.rs) lines 972-987
(`add_plan`) and 1070-1080 (`goal_add`).

**What is wrong.** `add_plan` calls `refuse_if_held` before it checks
whether the agent is already a member. `goal_add` only prints "X is already
in "T" as ..." (`already_member_output`, which signs nothing) after
`add_plan` returns. In a goal that is catching up, `goal add` for an agent
already in it now exits 9. The refusal also reads as if the agent were
joining a goal it is already in. The plan's rule covers only commands "that
would sign with a key that `GoalStatus.guard` lists" (line 2821).

**How it shows.** A scratch drill: the lost rule change of the first drill,
then `locust --owner goal add --goal G --agent agent-1` on the restored host,
where agent-1 is the host's agent:

```text
HELD goal add -> 9: locust: read_only: Host can't join "Durable lifecycle": the Locust data here is older than what this computer signed in the goal (this computer). It catches up by itself. To go on without waiting: locust --owner goal continue --goal a82c8a15
```

Without the hold, the same command prints "agent-1 is already in ..." and
exits 0, as `t2_flow.rs` tests. `farm off` has a smaller form of the same
problem: it is refused while held even when the farm is already off and the
daemon would sign nothing. E1's delete-only path changes `farm off` anyway.

**Fix.** In `add_plan`, check membership first, and call `refuse_if_held`
only when the agent is not yet a member.

### 3. Some words of the `goal continue` plan point at nothing

Minor. [only_you.rs](../crates/locust/src/cli/only_you.rs) lines 1384-1474
(`continue_block`), and build note 6.

**What is wrong.**

- One line is printed per hold, in `observed.guard`'s order, which is key
  order. `They may include a removal, a rule change or the end of the goal.`
  then follows them all. When an agent's key sorts after the governance key,
  "They" follows the agent's line, not the host's. The plan puts it "after
  the line on the missing records" of the governance key (lines 2807-2809).
- Where no host key is held, the closing line is the builder's: "If another
  computer holds one of those records, the next record signed here
  conflicts with it, and that agent signs nothing more in this goal." The
  plan gives no words here (build note 6). On a member's computer of unknown
  age, the block names no records and no agent, so "those records" and "that
  agent" refer to nothing.
- For a key that was only just admitted, the `goal continue` plan still ends
  with this line and with "Safe when this is the newest copy ...". Yet no
  record is missing and no copy is involved.

**Fix.** Print the host's missing-records line first, then "They may
include". Where only agents' keys are held, name the agents in the closing
line, for example "... and NAME signs nothing more in this goal". For an
`Admitted`-only goal, print one line saying that continuing skips the check
with the host's computer, and leave out the copy lines.

### 4. Doctor reads a symlinked marks directory as "not a directory"; the daemon uses it

Minor. [doctor.rs](../crates/locust/src/cli/doctor.rs) lines 80-115.

**What is wrong.** The new `marks` check reads `fs::symlink_metadata`. The
store opens the marks directory through `fs::metadata` (`create_directories`
and `open` in [marks.rs](../crates/locust-store/src/marks.rs)), so it follows
a link. Doctor's own `state_directory` check follows links too. A marks
directory that is a link to a private directory therefore starts the daemon,
but fails doctor with a false reason:

```text
$ locust --home .../doc/h --json doctor     # h.marks -> real/, mode 0700
{'name': 'state_directory', 'ok': True, ...}
{'name': 'marks', 'ok': False, 'detail': 'invalid: .../doc/h.marks is not a directory', ...}
```

**Fix.** Read the marks directory the way the store does. If a link should be
refused, because it could point into the home, refuse it in the store as
well, and say "is a symbolic link" in both places.

### 5. The guide's row for a refused action does not name the new side

Minor. [help.md](../docs/guide/help.md) line 12.

**What is wrong.** The row reads "Read which side refused: your level, the
goal's rule, or an act only you or the host can do." After a restore, the
refusal a person meets most is "(this computer)", with `read_only` and exit
9, and the row does not mention it. The plan makes a phase after Phase 6
responsible for the documents it makes stale (line 2726). The row already
left out "the goal's state" before G2.

**Fix.** Add "or this computer, which is catching up after a restore", with
a link to the Backups section of
[operations.md](../docs/guide/operations.md), and add the goal's state.

## The focus areas

**(a) When a goal waits for the person, and deviation 2.** `Node::waiting_for`
([views.rs](../crates/locust-core/src/node/views.rs) lines 181-211) lists a
goal only from the governance key's view, and only where this daemon hosts the
goal. `GuardView::waits_for_you()`
([api/guard.rs](../crates/locust-proto/src/api/guard.rs) line 30) is the one
predicate behind the listing, the block and the refusal. Deviation 2 adds
"and at least one computer was heard". It is the right reading. Read
literally, "`waiting` is empty" would list G-5's goal, whose copy lists
nobody. G-5 and the plan's own rule say that goal must not be listed: it
waits for a member's computer to call, and continuing would fork it. The
Risks section words the case as "no computer that answered sent [them]
back", which needs one that answered. `heard` also counts callers that are
not members of the copy. So a member admitted after the copy can still end
a G-5 hold, and if it holds nothing the goal then waits for the person, as
the plan expects. The one gap is finding 1, where a refusal uses a view
other than the one the listing uses.

**(b) `next_place(entry, author, body)`.**
[authoring.rs](../crates/locust-core/src/node/authoring.rs) lines 218-257
check in this order: the governance key's authority halt (K1), membership
and `Local.part` (skipped for the governance key), the hold, the agent's own
fork, then `Goal::next`. That is the plan's order, with K1's halt in front
and the fork test after it; E1 puts `conflict(ENDED)` first. Every signature
goes through `author` (and so `next_place`), `member_remove` or
`cancel_acknowledge`. The other direct `sign_at` calls are the genesis of a
new goal and records chained in the same transaction after a checked first
record. `access::subject` maps the bodies as the plan's table does:
admission to Invite, removal to RemoveMember, role record to GiveRole,
publication and consent to Publish. The local `goal.join` uses `Act::Join`.
Flow steps are gated by `hold` before signing and report
`Stall::CatchingUp` in the order the plan gives (`Node::stalled`). The
refusal's code is `read_only`, or `unavailable` for `Admitted`.

**(c) No plan while held.** `rules bind`, `member remove`, `task revise`,
`goal add`, `goal leave`, `workspace init`, `farm on`, `farm off` and
`farm consent` call `refuse_if_held` before any plan is computed. A held
goal's `role give` and `role take` print the refusal and nothing else, with no
Undo line. The test's stub daemon panics on any other request, so "computes
no plan" is really checked. `level`, `allow` and `invitation revoke` are
untouched. The exception is finding 2.

**(d) The plan id of `goal continue`.** The review is
`{"goals":[{"goal","title","guard"}]}`. A `GuardView` holds keys, a reason,
counts and endpoints, and no time. Last-seen times appear only in the human
text. `goal_continue_shows_its_plan_and_needs_a_goal_or_all` takes the id
from an `--all --plan` run and finds it in a later `--goal --plan` run, so a
clock reading in the review would fail the test. Once nothing is held,
`continue_plan` returns `None`, and `--confirm` answers `conflict: the plan
changed; run --plan again`.

**(e) Words.** Every line of G-4 is asserted exactly
(`this_computer_reads_the_same_facts_in_both_voices`). So are G-1, G-2 and
G-5 (presentation tests and drills), the G-3 lines the CLI test checks, and
G-6. The governance key is never named: `refusal` calls it `host`, a host
refusal reads "You" to the person, records read "signed as host", and
computers are named by their members' names or by endpoint. The tests assert
the key's prefix is absent from status text and refusals.

**(f) The drills.** The four tests in
[catching_up_tests.rs](../crates/locust/src/daemon/catching_up_tests.rs) run
two production daemons through `cli::run_for_test`. That helper mirrors
`run`'s handling of `--owner`, `--json` and `for_person`. Each exit criterion
is asserted with exact text: G-1's block with the member under "Not yet",
nothing waiting, `rules bind` refused with exit 9 and no plan, the block gone
and the restored line kept after B answers, the first form's "1 record Host
signed", the host of unknown age listed before and after B answers, the
G-3 plan, a wrong id refused with exit 7, the continue and its second run,
B's "Waiting to hear from the host's computer", and the agent's exit 9 with
`details.why.side`. "Proceeds on yes" is checked through `--confirm`, not
the terminal prompt. The MCP half of the agent criterion is checked only by
passing a made-up refusal through the bridge
(`a_goal_catching_up_reaches_the_tool_as_this_computer`). The daemon half is
covered by the drill's JSON. `run_for_test` is `#[cfg(test)]`; only
`failure_text`, which `print_failure` uses, was moved into shared code.
`Running::start_at` and `local_endpoint_at` live in the `#[cfg(test)]`
module `durable_tests`. A non-test `cargo build -p locust` compiles without
them, so no test-only code reaches the binary.

**(g) Guides, skill and MCP.** The operations.md Backups section says each
thing the plan lists, in order, and "Restoring a copy is untested." is gone
from the guides. `git grep` finds it only where the plan and its companion
quote it. The section also names the marks directory and doctor's check.
sharing.md, SKILL.md and `INSTRUCTIONS` in mcp.rs carry the plan's short
text, in the person's voice in sharing.md. concepts.md lists `goal
continue` among the commands that ask for a yes. help.md is finding 5.

## The build notes' departures

| # | Departure | Verdict |
| --- | --- | --- |
| 1 | `Node::refusal` and `access::subject` in place of the plan's `Node::refuse`/`Attempted::Sign` | Accepted: the plan says its Phase 3 names are plan text. |
| 2 | Behind on the governance key waits for the person only if at least one computer was heard | Accepted; see (a). Finding 1 is a separate gap in the refusal. |
| 3 | Agent voice of a host refusal and of the status block | Accepted: "the host" is the existing agent-voice word (`Why::OnlyYou`), and "NAME's owner can continue: LINE" follows the pending view. |
| 4 | No `Halted` waiting entry to restrict | Accepted: `WaitingKind` has none. |
| 5 | Names from one `goal.status` read per held goal; joined missing lines; restored line with nothing revoked | Accepted. |
| 6 | Words the plan does not give; `goal continue` JSON | Accepted for the JSON; the words are finding 3. |
| 7 | How each no-plan command is checked | Accepted, except finding 2. |
| 8 | `lost_goals` counted only at a replaced or overwritten start | Accepted: a start without kept marks has nothing to count from. |
| 9 | Doctor creates nothing and also fails a directory others may read | Accepted; the symlink is finding 4. |
| 10 | The drills' host on a fixed loopback port | Accepted: test code only. The port is taken from a socket that was bound and then dropped, so another bind could take it in between; no failure was seen in four runs. |
| 11 | The stall test's `Halted` case made by a fork | Accepted: a gap now raises the mark and reads `catching_up`, as the plan's order says. |
| 12 | The simulator's restart comparison leaves out catching-up entries and `lost_goals` | Accepted. It also leaves out the unknown-age entry, which does not depend on whom the guard heard; the holds are still compared. |
| 13 | Where the MCP and skill sentences sit | Accepted. |

The commits carry no attribution trailers. No dead code was left: G1's fixed
hold sentences and `Hold::refusal` are gone, and every new helper has a
caller.
