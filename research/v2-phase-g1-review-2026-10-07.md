# Review of phase G1 as built, of S1a, A1 and the E1/E2 models, and of the hooks branch

Status: research finding, 7 October 2026. A review by reading of work landed
on main at `a19a395`, and of the unmerged hooks branch at `2ba4917`. No
checks were re-run: no build, no tests, no daemons, no model runs and no check
scripts. The building sessions ran those.

Eleven readers each took one part: for G1 the marks file, the start and settle
rules, every path that signs, and the tests with the build notes; then S1a,
the models, A1, and four parts of the hooks branch (structure, the stop and
notice rules, setup, and scope against the plan and main). A second reader
tried to refute each report. Every finding below survived that. Where the
second reader changed a severity, the finding carries the second reader's
grade. Several readers found the same defect on the hooks branch; each such
defect appears once here. No finding remains marked uncertain, so no area has
an uncertain subsection.

Severities: **blocking** stops the work from counting as done; **major** is
wrong behaviour a person or an agent would meet, or a check that passes
without checking; **minor** is real but rare, cheap, or confined to text and
tests.

## What landed and what did not

Landed on main:

- **The R6 mark** in the master plan's build order, `30815e7`.
- **S1a**, the removal half of S1: `Commit::drop_blobs` and its paths are
  gone, `28c6425`.
- **G1**: the marks file beside the data directory, `1847b24`; holding a
  daemon's keys after a restore until it has caught up, `b135883`; and the
  [build notes](v2-phase-g1-build-notes-2026-10-07.md), `a19a395`.
- **The E1 and E2 models**, written first: `adcacc4` and `dcedcbe`.
- **A1**, the tool summaries and the context page cap, `60c22d1`.
- **A4's sentence**, `c5623d7`.

Not landed:

- **The hooks branch**: six commits on `8f27419` (`7332c76`, `53ef034`,
  `e14ad24`, `06f3b05`, `da387ad`, `2ba4917`). It is six commits behind main
  and does not yet include S1a or G1.
- **The network sync fix**, which has no commits.
- **A2 and A3.**

In short, G1 does what the plan asks in the ordinary cases, and its signing
gate covers every path that signs a record. Two G1 defects need fixing: a
start on an empty home throws away the marks a later restore needs (1), and a
restart while one goal is still behind revokes invitations in goals made
since (5). The E2 models pass without checking two of the things E2 asks of
them (16, 19). On the hooks branch, a subagent's tool calls get the hook's
failure line in every chat (26), a worker that cannot advance its attempt is
blocked at every stop (27), and the branch does not compile once put on main
(37). G1 also overwrote A1's text in the checked-in runtime contract, so the
formation check fails on main (23).

## G1: the marks file

### 1. A start on a new store clears the only marks that protect a later restore

Major. [guard.rs](../crates/locust-core/src/node/guard.rs) lines 438-448.

**What is wrong.** `guard_start` treats a store with no file record and no
goals as new, and writes a `MarkWrite::Clear` for every mark it finds. The plan
says only that a new store gets its `file` written and holds nothing
([plan](../docs/host-safety-and-ending-plan.md) lines 2062-2063). The clearing
was the builder's choice, in build note 4. Its reason is that no key the new
store holds can sign under the old marks. That is true, and it is also why the
old marks are harmless: `local_keys` already filters by held keys (guard.rs
258-271). The plan wants the marks to outlive the data directory (lines
2629-2631).

**How it fails.** The data directory is lost or moved aside, and
`~/.locust.marks` survives. The daemon starts once on the empty home: the
installed service has RunAtLoad and KeepAlive, and the daemon creates a
missing home. Every mark is cleared in place, and the file keeps its identity,
so it still reads as kept. The person then restores the old home from a
backup. The database is another file and the marks are kept, so the start is
`Replaced`. No mark is ahead, so nothing is held, and every key signs at once
at positions peers already hold. That is the fork G1 exists to prevent.

**Fix.** Do not clear marks on a new store; write `file` only. Restrict
`ahead()` (guard.rs 454-460) to marks whose key this daemon holds, so stale
marks cannot make a later start read as overwritten. Add a durable test:
start, sign, move the home aside, start fresh, put the old home back, and
check that the key is held as behind.

**Skeptic.** Confirmed: each step can happen, and no test covers clearing
marks on a new store.

### 2. A creation time that appears or disappears makes an untouched home look copied

Minor. [marks.rs](../crates/locust-store/src/marks.rs) lines 213-226; the
same comparison in [guard.rs](../crates/locust-core/src/node/guard.rs) lines
435 and 450.

**What is wrong.** `decode` requires the header's `FileId` to equal the
current one exactly. `FileId` derives `PartialEq`, so `Some(t)` never equals
`None`. `FileId::of` returns `None` whenever `created()` fails
([store.rs](../crates/locust-proto/src/store.rs) lines 157-169). On Linux that
depends on the environment: a seccomp or sandbox profile that forces the stat
fallback, or a kernel or file system that adds or drops btime. The plan says
that with no creation time "the inode number alone decides" (lines
1897-1900). The code does not do that when only one side has one.

**How it fails.** The same untouched home is started once where a creation
time is reported and once where it is not. The marks read as lost and the
database reads as another file, so the start is `Unknown`. Every goal waits to
be heard, hosted goals wait for `goal.continue`, and pending invitations are
revoked, with no copy or restore having happened.

**Fix.** Compare `created_ms` only when both sides have one; otherwise decide
on the inode alone. Use the same comparison for the database `file` record.

**Skeptic.** Confirmed as minor: rare, mostly Linux, and it falls into the
conservative restore path rather than losing anything.

### 3. A home path ending in `..`, or `/`, puts the marks inside the home

Minor. [local.rs](../crates/locust-proto/src/local.rs) lines 203-216.

**What is wrong.** `marks_dir` appends `.marks` to the raw path when
`file_name()` is `None`. For `/srv/locust/..` (the home is `/srv`) that gives
`/srv/locust/...marks`, inside the home. For `/` it gives `/.marks`. The doc
comment says the directory is never inside the home. `home_dir` checks only
that the path is absolute.

**How it fails.** With `LOCUST_HOME=/srv/locust/..`, a backup of `/srv`
carries the marks. Restoring it in place rolls the marks back with the
database, both keep their identity, and the start is ordinary. The in-place
overwrite the marks exist to catch goes unnoticed.

**Fix.** Refuse a home whose last component is not a normal name, or
normalise it lexically in `home_dir`.

**Skeptic.** Confirmed as minor: the setup is contrived, and the default home
is unaffected.

### 4. An existing marks directory or file is never checked for owner-only modes

Minor. [marks.rs](../crates/locust-store/src/marks.rs) lines 73-88, with
`create_directories` in [files.rs](../crates/locust-store/src/files.rs) lines
323-366.

**What is wrong.** Mode 0700 is applied only when the directory is created,
and 0600 only when the file is. An existing directory or file is used as it
is. The home gets `check_private`; the marks directory sits beside it, so that
check does not cover it. The plan says the marks reveal goal ids, public keys
and counts, and that the directory is private.

**How it fails.** A person or a script makes the directory 0755 and the file
0644. Every local user who can reach the person's home directory (often 0750
group staff on macOS, 0755 on many Linux systems) can read which goals this
person is in, their keys and their signature counts.

**Fix.** Check the directory as `check_private` checks the home, and check the
file's mode at open.

**Skeptic.** Confirmed as minor: nothing Locust does makes the modes
permissive, and only metadata leaks.

## G1: the start and settle rules, and the paths that sign

### 5. A restart while one goal is still behind revokes invitations in goals made since

Major. [guard.rs](../crates/locust-core/src/node/guard.rs) lines 454-467, with
lines 497-501 and 400-404.

**What is wrong.** `ahead()` counts every kept mark whose record the store
lacks, including marks in goals that already hold a `RESTORED` record and are
only catching up. After a `Replaced` start the file record is rewritten, so
every later start while some key is still behind reads as `Overwritten`. Only
goals that already have a `RESTORED` record are skipped (line 498). A goal
created or joined after the restore has none, so `restore_found` runs for it:
it revokes that goal's pending invitations and writes a `RESTORED` record. The
plan says the exception exists so that "a restart while still behind does not
revoke an invitation issued since" (lines 1996-1998). That holds only for
goals that existed at the restore.

**How it fails.** The host's data directory is replaced by an older copy with
the marks kept. Goal X stays behind because its only other member is offline
for days. The person creates goal C and sends a ticket. The host reboots
before the ticket is redeemed. C's ticket is revoked, the friend's join is
refused for good, and C shows `restored` until X catches up. It happens again
at each reboot for each goal made since the last.

**Fix.** Leave out of `ahead()` any mark whose goal already holds a
`RESTORED` record. Add a test: restore with the marks kept, stay behind, create
a hosted goal with a pending invitation, restart, and check the invitation is
still pending and the new goal has no `restored` value.

**Skeptic.** Confirmed by two readers. One graded it minor, one major; it is
major here because a person loses an invitation silently and no test covers
it.

### 6. A halt-proof delivery counts as hearing a computer

Minor. [driver.rs](../crates/locust-core/src/sync/driver.rs) lines 586-596;
[responder.rs](../crates/locust-core/src/sync/responder.rs) lines 103-114 and
212-214.

**What is wrong.** On the accepted side, `Host::reconciled` runs for any
exchange that ended `Completed` from an admitted remote with no pushed events.
The responder never checks that a frontier was served. So `Hello`, `HaltProof`,
`Done`, the evidence-only path, counts as hearing. The records the proof lands
do not set `received`. The plan defines hearing as an exchange that ran its
record stage to the end and brought nothing (lines 1731-1736). The dialed side
gets this right (driver.rs 186-191).

**How it fails.** Member computer M is restored from a copy of unknown age and
waits for the host. The host has since removed M and holds a fork of some key
other than governance that M's copy also knows. It sends M a proof-only
exchange. M records the host as heard and clears its unheard hold with no
record stage. The harm is small, since M's later records are ignored once its
removal arrives, but it is a path where hearing is granted without
reconciling.

**Fix.** Set a flag in `serve` when the remote's `Frontier` is handled, and
call `Host::reconciled` only when it is set and `received` is false. Set
`received` when a halt proof lands a record.

**Skeptic.** Confirmed as minor: a governance fork is already turned into a
refusal, which leaves only a narrow case.

## G1: tests and build notes

### 7. The simulator's residual-6 exclusion accepts a host restore taken before the admission existed

Minor. [check.rs](../crates/locust-core/src/node/sim/check.rs) lines 181-200.

**What is wrong.** `host_missed_the_same_admission` drops a member's reuse
from the claims when an admission is missing from the member's copy and from
any host restore's copy. It never checks that the host held the admission
before its restore (`host.before`). The guide offers a restore point just
before the third machine joins, so a host copy without that admission is
common.

**How it fails.** m1 is restored before m3 joins, then m3 joins. Later m2 is
restored with its marks kept, to a copy from before m3 joined. m2 learns of m3
through m1, so the guard must wait for m3. If a guard bug let m2 sign at a
position only m3 holds, the reuse would count as outside the claims and the
sweep would report no failure. The notes' "no failure inside the claims" over
seeds 0 to 10,000 proves less than it reads.

**Fix.** Require `host.before.contains(admission) && !host.copy.contains(admission)`.
Re-run the sweep and compare the residual count.

**Skeptic.** Confirmed, regraded from major to minor: a gap in the test
oracle, not in what Locust does.

### 8. Any failing run with an unclaimed reuse counts as residual, whatever made it fail

Minor. [seed.rs](../crates/locust-core/src/node/sim/seed.rs) line 258.

**What is wrong.** `residual = result.is_err() && !found.is_empty() && claimed.is_empty()`
never looks at why the run failed. A timeout, a host hold that never ends, a
lost acknowledged record on another key, or a panic all become residual when
the run also has an unclaimed reuse. Shrinking skips residual runs, and
`sim_guide_under_faults` asserts only that `failures` is empty. The notes say
a run that fails "only on unclaimed reuses" counts as residual (lines
224-225); the code does not check "only".

**How it fails.** A guard bug that leaves a hold up forever, in a seed that
also has an unclaimed reuse, is filed under residuals and never reported.
About 0.14% of seeds are residual, so the exposure is small.

**Fix.** Mark a run residual only when every violation follows from the
reused key: fork reports, halts or pending standings on it. Correct the notes'
sentence.

**Skeptic.** Confirmed, regraded from major to minor: such a bug would almost
always also fail in other seeds.

### 9. No Rust test pins that an agent's mark is not given up while the governance key is held

Minor. [guard.rs](../crates/locust-core/src/node/guard.rs) lines 326-345.

**What is wrong.** `guard_settle` gives up an agent's mark only when
`heard_all && !governance_held`. No test has the host's agent and the
governance key both behind with every other computer heard. Removing
`!governance_held` would fail no named test. The model's
`restore-give-agent-held` trace has no Rust twin. No Rust test has a removed
member as the only holder of the governance record with the marks kept.

**How it fails.** A refactor drops the condition. The host is restored with
the marks kept and both records missing; the only listed computer answers
without them. The agent's mark is lowered while governance is still behind.
When the governance record returns, the agent's hold ends at once and it can
sign at a position a newly admitted caller holds.

**Fix.** Add a network test: a copy with the marks kept, a governance record
and a host-agent record that reach only daemon 2, daemon 2 down while daemon 1
answers. Check the agent's mark is unchanged and the agent held, then bring
daemon 2 back and check both records return with no fork.

**Skeptic.** Confirmed as minor: the code is correct today, and the plan
accepts model coverage for this rule.

### 10. Parts of the build notes' simulator account do not match the code or the plan

Minor. [build notes](v2-phase-g1-build-notes-2026-10-07.md) lines 180-187,
251-256 and 264-266.

**What is wrong.**

- The notes say the sweep's 14 residual runs include "the two kept runs of the
  plan". The kept runs are hand-built tests in
  [restore_runs.rs](../crates/locust-core/src/node/sim/restore_runs.rs) with
  fixed seeds and `faults = false`. `sweep` never runs them.
- The notes say of residual 6 that "the plan accepts the fork". Risk (5) of
  the plan lists three agent-key forks (lines 2461-2470), and residual 6 is
  none of them. The plan's claim at lines 2340-2343 still covers it. The
  exclusion in `host_missed_the_same_admission` narrows a plan claim and an
  exit criterion, and `a19a395` did not change the plan.
- The notes count "the plan's 23" guard tests and "eight" store tests. There
  are 25 and 9. The three left out are
  `a_second_restore_before_the_first_is_caught_up_is_still_unheard`,
  `records_that_return_raise_the_mark` and
  `a_marks_unheard_bit_survives_reopen_and_a_nonzero_reserved_byte_is_lost`.

**How it fails.** The owner reads residual 6 as already accepted and is never
asked about it. A reader takes the plan's residual-5 runs as reproduced inside
the seeded sweep.

**Fix.** Say the kept runs are separate deterministic tests. Say residual 6 is
a new residual for the owner to accept or reject, and either record it in the
plan or remove the exclusion. Correct the counts and name the three tests.

**Skeptic.** Confirmed in part: the claim that the governance key's account
was wrong is refuted; the rest holds.

### 11. The "no mark, so no unheard bit" residual is still inside the simulator's claims

Minor. [guard.rs](../crates/locust-core/src/node/guard.rs) lines 470-490 and
519-528; [check.rs](../crates/locust-core/src/node/sim/check.rs) lines
142-175.

**What is wrong.** A goal where no local key has a record in the copy carries
no mark, so the unheard bit cannot be kept there. The code comment and the
notes (lines 247-250) name this residual. But `claimed()` still claims it: the
marks-lost exclusion ends at the machine's next restore, and that restore is
kept, with no removal and no missed admission. A seed that hits it would show
up as a failure inside the claims.

**How it fails.** m3 is backed up before its agent signs anything. The agent
signs record 0, which reaches only m2. m3 is restored whole: marks lost, goal
unheard, a `Clear` for the agent. Before m3 hears anyone, its data directory
alone is restored again to the same backup, with the marks kept. The start is
`Replaced`, no mark carries the unheard bit, and the agent signs at position 0
at once.

**Fix.** Keep the goal's unheard memory outside per-key marks, for example in
a goal-level slot in the marks file. Or narrow the claim in `claimed()` and the
plan text explicitly, and add a kept run for it.

**Skeptic.** Confirmed as minor: the residual is accepted in the plan's prose;
the defect is that the oracle and the notes disagree.

### 12. No test admits a joiner while only the host's agent is behind

Minor. [tests/guard.rs](../crates/locust-core/src/node/tests/guard.rs) lines
383-450.

**What is wrong.** The exit-criterion drill says `goal invite` and admission
still work while only the host's agent is behind. No Rust test admits a joiner
during that hold. `only_the_goals_that_are_behind_are_held` never invites, and
the delivery test admits only after the hold ends. Its check uses
`Node::hold`, not `admission_hold`.

**How it fails.** A change makes `admission_hold` fall back to the agent's
hold. Admission returns `CatchingUp` during an agent-only hold, and only the
manual drill would catch it.

**Fix.** Add an invite and a join while the agent is behind to
`only_the_goals_that_are_behind_are_held`.

**Skeptic.** Confirmed for this item only; the report's two other items (an
agent could release holds, and exchanges opening before the guard commit) are
refuted by `continuing_is_the_owners_alone` and by the sans-IO structure of
`Node::open`.

## S1a

### 13. crates.md cites the wrong master-plan lines for the version rule

Minor. [crates.md](../docs/crates.md) line 50.

**What is wrong.** The new sentence cites master-plan lines 353-363 for the
rule that signed bytes may change inside version 7 before the first release.
Those lines are build-order rows. The rule is the "Versions." paragraph at
[master-plan.md](../docs/master-plan.md) lines 406-417, where it already was
when S1a landed, and where the S1a spec said to cite it.

**How it fails.** A reader who follows the citation lands on the build-order
table. The doc checker does not check line numbers.

**Fix.** Change "353-363" to "406-417".

**Skeptic.** Confirmed: the number was stale before S1a landed.

### 14. The master plan does not record S1a as built

Minor. [master-plan.md](../docs/master-plan.md) lines 333 and 375.

**What is wrong.** The pieces table still says "A1 is built; the rest are
not", and the S1 row has no built note. The only record of S1a is in the G1
build notes. The store plan names the build-order table as the only place
phase state is kept.

**How it fails.** A session picking the next phase reads S1's removal as still
to do and plans it again.

**Fix.** Mark the removal half of S1 built (`28c6425`) with the sentences half
still open, and say "A1 and S1a are built".

**Skeptic.** Confirmed: docs only.

### 15. The rebuilt cache test's "authorizes nothing" assertion cannot fail

Minor. [content_graph_tests.rs](../crates/locust-core/src/node/content_graph_tests.rs)
lines 673-686.

**What is wrong.** The old test asserted `NotFound` for an object peer 1 held
once its manifest was gone. The rebuilt one asserts `NotFound` for
`later_data`, which peer 1 has never received. The answer is `NotFound` however
reachability behaves.

**How it fails.** A regression that serves any held object without a
reachable manifest still passes this test. It is caught only by the test at
lines 164-170, which covers the same property.

**Fix.** Land the sealed `later_data` on peer 1 directly before the
assertion, as `tests/context.rs` does, or drop the assertion and its comment.

**Skeptic.** Confirmed as minor: the property is covered elsewhere.

## The E1 and E2 models

### 16. `restore-leave` stops at `settle`, so `RemovalMatchesBefore` passes without checking

Major. [RestoreGuard.tla](tla/RestoreGuard.tla) line 200.

**What is wrong.** The program is `copy, leave, removeP, store, start, PL,
hear, settle, continue, removeP`. In the state reached before `settle`, every
disjunct of Settle's guard (line 161) is false: the goal was shared, the
agent's mark is 0, nothing is unheard and nothing was admitted. The run stops
at phase 7. `continue` and the second removal never run, so `hist` holds one
removal and `RemovalMatchesBefore` (lines 268-274) compares it with itself.
`CHECK_DEADLOCK FALSE` and a plain `expect: pass` hide the stall. `Leave` only
sets `didLeave`, which nothing reads. [restore-guard.md](tla/restore-guard.md)
lines 94-97 and the plan's E2 text (lines 3790-3794) say the model shows the
re-signed removal equals the first.

**How it fails.** A defect that made the re-signed removal differ would still
pass.

**Fix.** Drop `settle` from the program, or add a twin that expects a
violation of "the program completes with two removals in `hist`". Give `Leave`
an effect the model observes if E2 relies on how the leaver behaves.

**Skeptic.** Confirmed, regraded from blocking to major: the design is
probably right if the trace ran, but a required model-first check passes
without exercising anything.

### 17. The model's leave rule is stronger than Rust and E2's plan

Minor. [Organization.tla](tla/Organization.tla) line 210.

**What is wrong.** `Eligible` excludes every record above any record of kind
`leave`. It does not check that the leave is valid, which admission it names,
or whether a removal is held. Rust excludes work above a leave only through
the removal's cutoff, and E2's plan says the member stays, marked as left,
until the removal (lines 3714-3720).

**How it fails.** In leave-removal, records 16 and 17 are excluded even when
the removal is not held, so the cutoff path is never what decides. An invalid
leave by the host's agent still excludes its later work. A member admitted
again with the same key can never have new work count. Someone building E2's
Rust from the model would build that last case wrongly.

**Fix.** Remove the leave clause and let the removal's cutoff do the
exclusion, as Rust does. Or restrict it to a valid leave of the author's
admission at the record's anchor, and add the counter-cases.

**Skeptic.** Confirmed, regraded from major to minor: it lives only in the
model, and E2's Rust is not built.

### 18. `organization-leave-fork` models a governance fork, not E2's case

Minor. [Organization.tla](tla/Organization.tla) lines 130-134 and 306.

**What is wrong.** E2 asks for a second record below the leave, delivered
after the removal, with the leave and its records kept (plan lines
3784-3786). The model has two leaves at one position and two removals at one
governance position, which forks governance and retracts both removals. Its
unused witness expects everything retracted.
[Plan details](../docs/host-safety-and-ending-plan-details.md) line 288 cites
the case as evidence the removal keeps work "also against a later fork".

**How it fails.** A model bug that dropped records 10 or 14 once a fork below
the leave arrives would go unnoticed. Nothing is asserted in the reachable
state closest to E2's case.

**Fix.** Rewrite the case as one leave, one removal with cutoff at the leave,
and one record forking below the leave. Add an invariant that 10 and 14 stay
in `view.ordinary` and the fork record does not.

**Skeptic.** Confirmed, regraded from major to minor: no behaviour is wrong
today, but fix it before E2 cites the case.

### 19. `organization-leave-readmission` admits a new identity, so it tests no readmission

Major. [Organization.tla](tla/Organization.tla) lines 135-139.

**What is wrong.** E2 asks for "an admission of the same identity and one
record under each admission" (plan lines 3786-3788). The model admits member
6, who was never a member, and record 17 is member 6's first record. No record
by identity 2 sits under a second admission.
[organization.md](tla/organization.md) line 39 rewords the case as "a new
agent key". Under the model's leave rule (finding 17), a readmitted identity
2's new work could never count, and this case hides that.

**How it fails.** If the model, or Rust later, kept a readmitted member's new
work excluded because of its old leave, the case still passes.

**Fix.** Readmit identity 2 and add a record by identity 2 anchored at the
readmission. Assert it is effective and that record 10 stays retained by the
cutoff.

**Skeptic.** Confirmed as major: the model step does not deliver the case the
plan names, and the gap hides a real difference from intended behaviour.

### 20. Most new organization cases assert nothing about their named outcome

Minor. [Organization.tla](tla/Organization.tla) lines 305-309 and 362-368;
[cases.json](tla/cases.json).

**What is wrong.** Apart from `end-late-record`'s witness, every new case is
`expect: pass` with only the shared invariants and `EndIsTerminal`. The
outcomes the plan names for fork-at-or-before, fork-above, after-readmission,
leave-removal and leave-host-agent are not checked. The witness arms for the
four leave scenarios are computed but never read, since no case runs
`NeverWitness` for them. In fork-at-or-before both forks have seq at or below
the end's, so `EndIsTerminal` still holds if the end wrongly stays in force.
For E1 this matches what the plan prescribed; for E2 the plan states
checkable outcomes that are not checked.

**How it fails.** A model that kept the end in force after a fork at its
position, or dropped it on a fork above, still passes. organization.md lines
38-39 and the plan details say these cases check those outcomes.

**Fix.** Add a scenario-scoped invariant per claim, or register a `-witness`
case for each existing witness arm.

**Skeptic.** Confirmed, regraded from major to minor: overstated evidence in
a model that runs before the code exists.

### 21. No recorded run of the organization suite with the eleven new cases

Minor. [organization.md](tla/organization.md) lines 6-7.

**What is wrong.** The only recorded run touching these commits is G1's
`--suite restore`, which holds `restore-leave` and none of the organization
cases. Neither `adcacc4` nor `dcedcbe` records a `--suite organization` or
`--suite fast` run. organization.md still points to the
[retained record](evidence/tla/organization/README.md) as the verification
record, which predates both commits.

**How it fails.** A reader takes the E1 and E2 cases as checked. Nothing
records that the suite reached its declared outcomes after `end` and `leave`
changed `Governance` and `Eligible` for every scenario.

**Fix.** Record a `--suite organization` and `--suite fast` run in a dated
note, or say in organization.md that the retained record predates E1 and E2.

**Skeptic.** Confirmed as minor: the evidence README says it is historical,
and E1 and E2 each run the suite at their own exit.

### 22. `Restore` now resets `didRemove`, which lifts the stated one-removal bound

Minor. [RestoreGuard.tla](tla/RestoreGuard.tla) lines 10, 87, 103 and 204.

**What is wrong.** `Remove` already allows a second removal once `continued`
is set, so the reset in `Restore` is not needed for `restore-leave`. It lets
the general store cases sign a second removal after a restore with no
`continue`. The header, [restore-guard.md](tla/restore-guard.md) line 26 and
the plan (line 4399) still state at most one removal. The retained results
file predates the change. The `leaveP` alias is unused.

**How it fails.** A reader trusts the stated bound when reading the p1 results,
but `restore-p1-store` and three other store cases now explore runs with two
removals. The `p2` cases cannot, since the governance key stays held there.

**Fix.** Drop the reset and keep the `continued` guard, or update the bound in
all three places. Remove `leaveP`.

**Skeptic.** Confirmed as minor: a larger state space checks more, not less;
only the description is wrong.

## A1

### 23. G1's commit put back the old summaries in the checked-in runtime contract

Minor. [runtime.contract.json](../docs/reference/generated/runtime.contract.json)
lines 2599 and 7305, and about seventy other summary lines.

**What is wrong.** A1 regenerated the contract with 37 new summaries and the
context limit's description. `b135883`, the next commit to touch the file,
regenerated it from a tree that had G1's API changes but not A1's. It put
back every bare-name summary ("board", "contributions", "attempt cancel" and
the rest) and removed "Most items on this page, at least 1. A value above 32
is treated as 32". [api.rs](../crates/locust-proto/src/api.rs) on main still
holds A1's text (lines 756, 908, 920). The build notes say the regenerated
contract verifies (line 274); that was true only of the tree G1 built from.

**How it fails.** `python3 scripts/check_formations.py` on main reports
"Formation export drift" ([check_formations.py](../scripts/check_formations.py)
lines 104-116), which breaks one of A1's exit criteria. Readers of the
published reference see the bare names and no page cap. The daemon and CLI
serve the contract from the binary, so behaviour is unaffected.

**Fix.** Regenerate from main with `check_formations.py --write` and commit
only the generated file. Note in the G1 build notes that the contract was
regenerated from a tree without A1.

**Skeptic.** Confirmed, regraded from major to minor: only the reference
document and the drift check are affected.

### 24. The `contributions` summary tells agents to read others' results before publishing

Minor. [api.rs](../crates/locust-proto/src/api.rs) line 920.

**What is wrong.** The new summary ends "Read it to reuse earlier work before
publishing." [SKILL.md](../skills/locust/SKILL.md) lines 108-111 tell agents to
post their result before reading other members' results on the same task. The
tool summary is what a model sees when it picks the tool.

**How it fails.** Two agents attempt the same task under peer review. Each
calls `locust_contributions` for the task before publishing, sees the other's
result and converges on it. The skill's independence rule is defeated by the
tool's own help text.

**Fix.** Scope the sentence: read it for goal-wide findings and earlier
rounds; on a task you are attempting, publish before reading others' results
on it.

**Skeptic.** Confirmed as minor: independence is a convention, not a
counting rule.

### 25. The skill says the list of tasks to start shows titles; `locust_pending` returns none

Minor. [SKILL.md](../skills/locust/SKILL.md) lines 141-142.

**What is wrong.** The new ordering paragraph says a waiting task's first line
is "the title the board and the list of tasks to start show". Only the CLI's
rendering adds titles to that list. The MCP tool returns `WorkItem { task,
offer, attempting, results }` ([api.rs](../crates/locust-proto/src/api.rs)
lines 1651-1656), with no title. The plan made the same assumption (store plan
lines 250-252), and A2's wording repeats it (lines 329-331).

**How it fails.** An MCP agent picks a task from pending, never sees its
"After task:" line, and starts it before its prerequisite is done. The skill's
check never fires, because nothing the agent read marked the task.

**Fix.** Say the title is on the board, and that `locust_pending` lists only
ids, so read the board or the task before starting one from pending. Fix A2's
wording before A2 is built, or add titles to pending.

**Skeptic.** Confirmed as minor: the ordering is advisory, and the plan
accepts that nothing enforces it.

## The hooks branch: what the hook says and when

Locations on the branch are given as `hooks:<path>:<line>`; these files do not
exist on main, so they are not linked.

### 26. A subagent's tool calls get the failure line, in every chat, Locust or not

Major. `hooks:crates/locust-adapter/src/hooks.rs:238-243`;
`hooks:crates/locust/src/cli/hook.rs:73-74, 95-99`.

**What is wrong.** `parse_input` refuses any callback that carries `agent_id`
(or `agent_type` on Codex). Refusing to attribute a subagent's call is right.
But the CLI turns that refusal, like any error, into `core::failure()` and
prints "Locust context was NOT injected" as PostToolUse `additionalContext`.
This happens before the check that keeps unassociated chats silent
(`hooks:crates/locust/src/hook.rs:246-253`). The qualification note says an
unassociated native chat stays silent (line 23); for subagents it does not.

**How it fails.** Setup installs hooks into the person's own profile by
default. In any chat, including one that never used Locust, a subagent makes
thirty tool calls. Codex 0.153.4 sends `agent_id` on each, as the adapter's
own comment says, and Claude Code documents it too. Each result gets the
failure line. SKILL.md (line 266) tells the model the line means the hook
failed and to read state with the Locust tools, so an unrelated subagent may
start calling them.

**Fix.** Treat an identified subagent callback as not ours: no output, no
daemon contact, no marks. Keep the failure line for real faults. Add a binary
test per harness: a subagent PostToolUse prints nothing and writes no marks.

**Skeptic.** Confirmed as major by four readers independently: no data is at
risk, but every subagent tool call in the default profile gets a false
failure notice.

### 27. A worker that cannot advance its attempt is blocked again at every stop

Major. `hooks:crates/locust-adapter/src/hooks/core.rs:254-257`.

**What is wrong.** Any own call that is not read-only clears `shown` and
`has_blocked`. Two such calls are what a blocked model is led to make:
`context.acknowledge`, which SKILL.md says to send after every context read,
and a progress report. A context read returns already-acknowledged items
unless `unread_only` is set, so it always yields a receipt. Codex and Claude
Code give each call a fresh `tool_use_id`, so the dedupe does not catch
repeated acknowledgments. `stop_hook_active` is parsed and never read, so
nothing on the harness side caps the loop. The plan names Beads #3451, agents
pushed to keep going at every stop, as the risk this rule guards against
(store plan lines 681-683). It does not consider that its own bookkeeping
write re-arms the block.

**How it fails.** A worker holds attempt X but needs its owner's input. Stop
blocks with "1 held attempts ... keep going unless your owner asked you to
stop". The model reads its context, acknowledges as the skill says, and ends
its turn. Stop blocks again on X. The same happens with a progress note. This
repeats until a person intervenes, spending tokens each round, and the
worker's question to its owner never ends the turn.

**Fix.** Do not count `context.acknowledge` as a write for the stop rule. Do
not let a progress report re-arm the block for its own claim: keep claim ids
in `shown` until their generation changes. Record the rule in the plan.

**Skeptic.** Confirmed as major: each round needs a write, but a model that
follows the skill makes one.

### 28. Chats sharing a session get "claim lost" when a sibling chat finishes its attempt

Minor. `hooks:crates/locust-adapter/src/hooks/core.rs:412-418, 442-458`;
`hooks:crates/locust/src/installation/setup/launcher.rs:29`.

**What is wrong.** The launcher binds one session per profile, so every chat
in it shares one session. `reconcile` puts every claim of the session into
each associated chat's baseline. Only the chat that made the terminal write
releases the claim in its own marks. To every other associated chat, a normal
completion looks like a loss no write of that chat explains. Reminding sibling
chats of the session's attempts, and telling them of its cancellations, is the
documented design and not a defect. The same false line reaches the writing
chat itself when it ends the attempt through the CLI in a shell, since only
MCP calls are recognized.

**How it fails.** Chat B once called `locust_status`. Chat A starts and
completes attempt X. On B's next tool call, B is told "attempt X, generation
1: claim lost. Use locust_pending and locust_context_read." about work it
never held.

**Fix.** Treat a release by any chat of the session as explained, using a
session-wide release record beside the per-chat marks. Or report loss only for
claims this chat established. Say in agents.md that a CLI write is not seen.

**Skeptic.** Confirmed as minor by two readers: one non-blocking line, once
per claim generation.

### 29. A cancellation read that keeps failing turns off every hook for the chat

Minor. `hooks:crates/locust/src/hook.rs:161-166, 277, 301, 336-337`.

**What is wrong.** `resolve_releases` runs on every start, stop and tool
callback, before the snapshot. An error from `cancellation_target` aborts the
whole callback with `?`, including a lasting answer: not effective, not found,
or not a cancellation. The unresolved entry is never dropped, so the error
repeats. The qualification note says a read failure defers only that goal's
reconciliation (lines 162-166); the core does that, the runtime does not.

**How it fails.** A chat acknowledges a cancellation whose target it never
saw, so it is stored as unresolved. A later removal or dispute makes the
cancel event excluded without halting the goal. From then on every callback
in that chat prints only the failure line: no stop blocks, no idle waits and
no notices in any goal.

**Fix.** On a lasting refusal, drop the pending release and let ordinary
reconciliation report the claim. On a transient error, skip only that goal.

**Skeptic.** Confirmed as minor: the sequence is rare and fails open.

### 30. A stale baseline revision makes the whole tool poll fail

Minor. `hooks:crates/locust/src/hook.rs:111-127`.

**What is wrong.** The tool path calls `wait` with `seen` set to the chat's
stored revision. The daemon refuses `seen` ahead of the goal's revision as
invalid (main's `requests/reading.rs` lines 206-215). The `?` aborts the
snapshot for every goal. Hook marks live in a separate file from `locust.db`,
and the G1 plan names restoring the database file alone as an expected case.

**How it fails.** After a database-only restore, the hook marks are newer
than the restored revision. Every tool call fails for every goal until the
next stop rewrites the baseline, so cancellation and claim-loss notices are
delayed.

**Fix.** When `wait` answers invalid for a goal, fall back to `Pending` for
that goal. Never let one goal's baseline fail the others.

**Skeptic.** Confirmed as minor: notices are delayed, not lost.

### 31. Start and tool hooks inherit the stop hook's 299-second deadline

Minor. `hooks:crates/locust/src/cli/hook.rs:87-90`;
`hooks:crates/locust/src/hook.rs:238`.

**What is wrong.** `hook_timeout_ms` comes from `stop_timeout_seconds` for
every event. Every socket read and the chat-lock wait run up to that
deadline. The native limits for tool and start hooks are 600 s for Codex and
Claude, so they never cut it short. The tool path never parks, so it needs no
such budget.

**How it fails.** The daemon is wedged but its socket still accepts. Every
tool call, Bash and Read included, in every associated chat stalls for about
five minutes before the failure line appears. Hooks started together time out
together, so they do not add up.

**Fix.** Give start and tool a deadline of a few seconds, and keep the long
one for the stop wait.

**Skeptic.** Confirmed as minor by two readers: it needs a wedged daemon.

### 32. A worker chat in the person's own client waits at every turn end

Minor. `hooks:crates/locust-adapter/src/hooks/core.rs:249-253, 375-379`;
`hooks:crates/locust/src/hook.rs:289-291`.

**What is wrong.** One `locust_wait` call makes a chat a worker for good, and
every idle stop then waits up to 270 s, up to about 299 s in all. Nothing
ends the wait when the person types. The plan chooses this, and lists as an
open risk whether a waiting stop holds a typed prompt, to be recorded by each
adapter's check (store plan lines 682-684). The qualification note leaves it
unanswered for every harness.

**How it fails.** If a harness does hold typed input, a person who asks their
chat a question after it once ran `locust_wait` waits up to five minutes after
every answer, unless they press Esc.

**Fix.** Run the interactive typed-prompt check for each harness before
installing into the default profile. Consider waiting only while the chat
holds a claim.

**Skeptic.** Confirmed as minor: the code half is certain; whether prompts are
held is the plan's own open question.

### 33. Every tool callback in an associated chat rewrites and syncs its marks, and they grow

Minor. `hooks:crates/locust/src/hook.rs:271, 281`;
`hooks:crates/locust/src/hook/marks.rs:128-138`;
`hooks:crates/locust-adapter/src/hooks/core.rs:112, 243`.

**What is wrong.** `chat.save()` runs after every decision, whether or not
the marks changed. Each save syncs the file and the directory, which is
`F_FULLFSYNC` on macOS. `invocations` gains one id per Locust call and is never
pruned; `delivered` is never pruned either. The plan's "free when unchanged"
covers the daemon side only.

**How it fails.** A worker chat on macOS makes hundreds of Bash and Read
calls. Each pays two full syncs, a few to tens of milliseconds, and the file
grows slowly over a long run.

**Fix.** Save only when the marks changed. Keep the last N invocation ids and
prune delivered notices whose claim generation is gone.

**Skeptic.** Confirmed as minor: one save per ordinary tool call, not two, and
the growth is per Locust call.

## The hooks branch: setup

### 34. Missing secrets or a removed install make every chat report a Locust failure

Minor. `hooks:crates/locust/src/hook.rs:232-235`; the Pi shim's close handler
in `hooks:crates/locust-adapter/src/hooks/pi-shim.ts`.

**What is wrong.** `hook::run` reads the credential and session secrets before
it checks whether the chat is associated. A failure there becomes the failure
line, in chats that never used Locust. The Pi shim prints the failure line on
any non-zero exit, including when the launcher's binary is gone. Software
uninstall before `setup remove` is a supported order.

**How it fails.** The person deletes the daemon home or a secret file, or
uninstalls the software, without running `setup remove`. Every tool call in
every Claude Code, Codex or Droid chat of the profile gets the failure line
(with the binary still installed), and every Pi tool result gets it (with the
binary gone).

**Fix.** Before association, stay silent for start, stop and plain tool
events when secrets or paths fail. In the Pi shim, report a launcher failure
only for an own Locust call.

**Skeptic.** Confirmed, regraded from major to minor: it needs an
out-of-order removal; a daemon that is down stays silent as intended.

### 35. Applying setup again never updates installed hooks or the Pi shim

Minor. `hooks:crates/locust/src/installation/setup.rs:497-504, 1093-1104`.

**What is wrong.** When ownership exists, `prepare_hooks` keeps the recorded
registration and plans no change. A newer release's registration is never
computed. The event list, timeouts, command and the whole Pi extension stay as
the first install wrote them, and status still reports the hooks ready. The
doc comment on `hooks::install` says reapply should replace the retained
groups.

**How it fails.** A later release adds an event, as this branch adds Claude's
`PostToolUseFailure`. The person upgrades and applies setup. The plan shows no
hook change, status says configured, and the new event is never delivered.
Only `setup remove` then apply fixes it.

**Fix.** On reapply, compute the current registration and replace exactly the
retained owned entries, without bringing back hand-removed ones. Or have plan
and status say the hooks are from an older release.

**Skeptic.** Confirmed as minor: no released version has hooks yet.

### 36. A dotfile-managed hook settings file makes all of setup refuse

Minor. `hooks:crates/locust/src/installation/setup.rs:943-975`, with
`package::regular`.

**What is wrong.** Hooks are installed by default for Codex, Claude Code,
Droid and Pi, with no way to skip them. The hook file is opened with
`O_NOFOLLOW` and must have one link. Before this branch, Claude setup touched
only `~/.claude.json` and the skill.

**How it fails.** The person keeps `~/.claude/settings.json` as a stow
symlink. Plan and apply now fail with "cannot open regular file", and Claude
Code cannot be set up at all, MCP entry included.

**Fix.** Keep refusing to write through a link, but let setup continue
without hooks in that case, or with an explicit `--no-hooks`, and report the
hooks as not ready with the reason.

**Skeptic.** Confirmed as minor: the refusal is fail-safe and the workaround
is a regular copy.

## The hooks branch: structure and landing

### 37. The branch does not compile once put on main

Major. `hooks:crates/locust/tests/hooks.rs:248-273`.

**What is wrong.** The branch sits on `8f27419`, before S1a and G1.
`git merge-tree main hooks` merges with no textual conflict, but G1 added
`guard` and `restored` to `GoalSummary`, which has no `Default`. The new hooks
test file builds a full `GoalSummary` without them. Both sides also edited the
generated runtime contract.

**How it fails.** Rebase onto main and run the required checks: E0063, missing
fields `guard` and `restored`. The checks the building session ran were
against the old base.

**Fix.** Rebase, add `guard: vec![], restored: None` to the fixture,
regenerate the runtime contract from the merged code, and re-run the hook
suite. G1's per-agent `halted` now makes the hook skip goals where an agent's
own key is held, which the suite should see.

**Skeptic.** Confirmed by two readers, graded major by one and minor by the
other; it is major here because it blocks landing as the branch stands, and
blocking was refused because nothing has landed.

### 38. Adapters are a shared dispatcher with per-harness branches, not data

Minor. `hooks:crates/locust-adapter/src/hooks.rs:128-505`;
`hooks:crates/locust/src/installation/setup.rs:930, 1058, 1116`.

**What is wrong.** The core holds every decision with no harness name, as the
plan asks. The adapter layer branches on `Client` in ten places: config
shape, the Codex-only `agent_type`, Pi's own call, the string `tool_response`
for Claude and Droid, the envelope, the registration, and the success-event
names `"PostToolUse" | "tool_result"`. Setup hard-codes Codex's trust review.
The plan says a new harness adds only an adapter and its tests (store plan
lines 600-604).

**How it fails.** Adding Kimi Code means editing six shared functions. If Kimi
sends a JSON-string `tool_response` and line 429 is not updated, `own_call`
returns `None` with no error, and the chat never becomes a Locust chat.

**Fix.** Move each fact into `HookAdapter` or `NativeEvent` data: success
flag, response encoding, config shape, envelope kind, subagent fields, trust
review. Do it before H1b adds Kimi.

**Skeptic.** Confirmed, regraded from major to minor: nothing behaves wrongly
today, and a new adapter's own tests would likely catch a miss.

### 39. The failure line is written out twice more, outside the core

Minor. `hooks:crates/locust/src/cli/hook.rs:102`;
`hooks:crates/locust/src/cli/mod.rs:91`.

**What is wrong.** The plan says the core holds the failure line once. The CLI
repeats the literal in two places and prints it as bare text. The branch at
`hook.rs:101-103` cannot run, since `envelope` never refuses `failure()`. The
one at `mod.rs:89-93` runs when an older config names a harness this build does
not know.

**How it fails.** A rewording in `core::failure()` leaves two stale copies,
and no test compares them.

**Fix.** Take the text from `core::failure()` in both places, and retry
`envelope` with `failure()` at `hook.rs:101`.

**Skeptic.** Confirmed as minor: the proposed "print nothing" for an unknown
harness would be no better, so only the duplication is kept.

### 40. `compacted` and `stop_hook_active` are parsed and never used

Minor. `hooks:crates/locust-adapter/src/hooks.rs:196-197, 244-248, 275-276`.

**What is wrong.** Both fields are parsed and validated, and nothing outside
tests reads them. H3, which would use `compacted`, is not built. AGENTS.md
forbids dormant code. A non-boolean `stop_hook_active` becomes invalid input,
and so the failure line.

**How it fails.** A harness version that sends `null` there gets the failure
line at every stop in every chat, for a field Locust ignores. No known harness
does this today.

**Fix.** Remove both fields and the validation; add `compacted` with H3.

**Skeptic.** Confirmed as minor: the trigger is hypothetical.

### 41. Phase state on the branch is stale against main and incomplete for H

Minor. `hooks:docs/master-plan.md:379, 382`;
`hooks:research/agent-hooks-qualification-2026-10-07.md:4, 93`.

**What is wrong.** The note says phase state belongs to the A4 and H rows. The
H row is unchanged and records nothing built. The A4 row and note line 93 say
A4's claim lines and H3 wait for G1, which is built on main.

**How it fails.** After a straight merge, the master plan shows H as unbuilt
and A4 and H3 blocked on G1.

**Fix.** On rebase, mark H1a, H1b (Codex, Claude Code, Droid; Pi without
native qualification) and H2 built with their commits, and reword the A4 and
H3 blockers to name only G2 and E2.

**Skeptic.** Confirmed as minor: docs only.

## Checked and sound

**G1, the marks file.** Every log record is signed in `sign_at`, which pushes
its mark into the same transaction. The store writes and syncs the marks after
the SQLite commit and before `commit` returns; a commit carrying only marks is
not dropped, and a failed marks write fences the store. `guard_start` runs
before the first `drive_flow` and before the transport, and raises marks that
are behind, which covers a crash between the database commit and the marks
sync. In-place writes are synced once; replacement goes through a synced
temporary file, a rename and a directory sync. A torn record, a bad length, a
bad flag, a nonzero reserved byte or a key in two slots each make the marks
lost, never kept and short. The header holds the file's own identity, so
copies, clones and new-file restores read as lost. The replacement waits for
the first write, so a crash cannot turn lost marks into kept, empty ones. The
SQLite lock is taken before the marks open. Missing, unwritable and lost cases
fail closed. Putting both folders back in place is the plan's accepted
residual (2).

**G1, the rules.** The signing gate checks in the plan's order: a fork, then
behind (stricter than the model past a gap), then the host's agent through the
governance key, then unheard, then admitted. It is enforced in `next_place`,
`drive_flow` and `plan_join`. The start table's five outcomes match the plan's
four rows and the overwrite caveat. Settle preserves `empty-shared`,
`lacking-peer`, `give-agent-held`, `NoGiveWhileHeld` and `all-listed`.
`received` gives `hear-with-data` within one exchange, and a refused exchange
never counts. The shared bit reaches disk before the admission that first
shares the goal leaves. `goal.continue` matches the model's Continue and
overrides no fork. The two planned residuals are reproduced as designed. The
`unheard` bit added beyond the plan is set and cleared consistently.

**G1, the paths that sign.** There is one call to `Event::sign`, and every
caller of `sign_at` comes after a hold check on the same key. That covers
agent requests, governance and host operations, materialized effects and
admissions. Invitations, join requests, context receipts and farm requests are
signatures without a log position, and are intentionally not held; a restored
copy's farm sequence numbers are J3's, a known limit. The node is
single-writer, so no plan signed before a hold is landed after it. Replay
returns a response stored in the same commit as its event. Callers are kept
only where the daemon hosts and the governance key is held, at most eight. The
hooks branch adds no signing call.

**G1, tests and notes.** Each row of both tables has a test that exercises the
behaviour, and so do the holds only the person ends, hearing only on an empty
exchange, callers, joins, fork reporting, `goal.continue` and marks
durability. The model registry has 32 restore cases. The latency table carries
both numbers. The six drills match the plan. The kept simulator runs exist and
assert unclaimed reuses. Departures 1, 4-10 and 12-14 match the code.

**S1a.** Nothing dead remains: no `drop_blobs`, `objects::delete`,
`Files::remove` or the old DELETE statement anywhere under `crates/`. Every
removal the spec lists is done, and the remaining existence checks serve
content not yet arrived. The replacement tests check what the old ones meant,
except finding 15; `recovery_flush.rs` still checks the synced WAL and the
refused open. The crates.md rule itself matches the master plan.

**The models.** `end` is a governance kind, governance drops anything above an
end, and work anchored at an end is refused. `EndIsTerminal` matches the plan
and neither clause is vacuous. All eleven cases have an arm, a config and a
registry entry. The end-late-record witness is a real reachability witness.
The counts in organization.md match the registry.

**A1.** All 37 summaries match their handlers, apart from findings 24 and 25.
The context page cap is applied after the zero refusal and before the cursor
check, has no off-by-one, and its receipt covers only the page; the const
assertion holds with about a twofold margin. No client assumes a single page.
The extended model-strings test would have caught every old bare summary. The
paging tests are adapted correctly.

**The hooks branch.** The core holds every decision with no harness name.
Every line is fixed ASCII under 512 bytes with only counts, typed ids and tool
names; titles, names and transcripts never reach it, and the fixtures check
this with injected text. Exit codes are always 0, and a block comes only from
the stop envelope. A well-formed callback from an unassociated root chat
never connects and prints nothing. `LOCUST_HOOKS=off` is honoured everywhere,
and `client run` sets it. The stop rule blocks once per new id; only workers
wait, one per session, bounded by 270 s and the deadline. Marks are private,
written through a synced rename, and serialized per chat; replayed callbacks
are ignored. Halted goals are skipped for stop and tool, which fits G1's holds.
Free tasks come from the daemon's `unattended`. Setup adds and removes only
its own entries, restores the original bytes, keeps hand-removed entries out,
refuses edited ones, journals each change, refuses v2 records without a legacy
reader, writes 0600 files and 0700 directories, and quotes every path. Tests
stay out of the real harness home. H2 matches the plan's zero-timeout wait.
Real-model runs cover only the stop path on Codex, Claude Code and Droid,
from test builds; H2 and Pi were exercised with scripted payloads only, which
the note states.

**Refuted reports, by reader.** G1 marks: 1 of 5. G1 rules: 1 of 2. G1 paths:
1 of 3. G1 tests and notes: 0 of 6. S1a: 0 of 3. Models: 1 of 8. A1: 0 of 3.
Hooks structure: 1 of 5. Hooks stop and notices: 2 of 9. Hooks setup: 1 of 5.
Hooks scope: 0 of 7.

## What to do next

No finding is blocking. These come first:

1. **Keep the marks on a new store** (1). Write `file` only, filter `ahead()`
   by held keys, and add the move-aside-and-put-back test.
2. **Stop revoking invitations in goals made since a restore** (5). Leave
   goals that already hold `RESTORED` out of `ahead()`, with the test.
3. **Regenerate the runtime contract from main** (23). It is mechanical, and
   it is the one known failing check on main.
4. **Make the E2 models check what E2 asks** (16, 19). Let `restore-leave`
   reach its second removal and prove it, and readmit the same identity in
   `leave-readmission`. Do 17, 18 and 20 in the same pass, and record a run
   of the organization suite (21).
5. **Put residual 6 to the owner** (10, 7). Either accept it in the plan or
   remove the exclusion; narrow the exclusion to a host that held the
   admission before its restore either way, and re-run the sweep.

Before the hooks branch lands:

1. **Rebase onto main** and fix the fixture (37); regenerate the contract and
   re-run the required checks and the hook suite on the merged tree.
2. **Silence subagent callbacks** (26) and **missing-secret failures in
   unassociated chats** (34), each with a binary test.
3. **Stop re-arming the block** on an acknowledgment or a progress note for
   the same claim (27).
4. **Fail one goal, not the callback** on a cancellation read (29) or a stale
   baseline (30).
5. **Give start and tool a short deadline** (31).
6. **Answer the plan's typed-prompt question** for each harness before
   installing into the person's own profile (32).
7. **Mark H's phase state** and correct the A4 and H3 blockers (41).

Minor, when convenient: 2, 3, 4, 6, 8, 9, 11, 12, 13, 14, 15, 22, 24, 25, 28,
33, 35, 36, 38 (before Kimi), 39 and 40.
