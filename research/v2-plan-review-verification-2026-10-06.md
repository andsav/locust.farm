# Checking the v2 plan review: every finding, tried for refutation

Status: research finding, 6 October 2026. Nothing was built or run. Six
readers each took one group of findings from the
[independent review of the v2 plan](v2-plan-review-2026-10-06.md) and tried
to refute them against the plans and the code. A seventh scoped one way to
recover after a restore. What they returned is kept whole in the
[evidence folder](evidence/v2-plan-review-verification-2026-10-06/README.md).

## Result

Of 55 findings and sub-findings, 38 were confirmed, 17 hold in their core
with a detail that differs, and none was refuted. The review's conclusion
stands: the sixteen phases should not be built as written.

What matters most:

- **The restore guard's two blockers are real.** The first, that the note of
  what a computer last signed is not forced to disk, is closed by a change of
  a few sentences and one measurement. The second cannot be closed by proof:
  for a copy of unknown age, the records the computer is missing are the ones
  that say whom to ask. The choices are to wait for a person, to guess, or to
  go on under a new key. The last is scoped under
  [Recovering under a new key](#recovering-under-a-new-key).
- **Most other findings are edits, not redesigns.** The readers named about
  75 changes of a sentence and 35 edits to a phase. They are listed with each
  finding below and are not yet applied.
- **Where the review was imprecise** is recorded with each finding. Two cases
  change the remedy: a check that never stores, marks or sends a rejected
  record is met by today's order of work, so the one rules check needs no new
  boundary; and a failed step in CI stops the steps after it, so from R1 to
  R6 the helper tests and the documentation check would not run at all.

The plans cited are the [master plan](../docs/master-plan.md), the
[roles plan](../docs/roles-and-permissions-plan.md) and the
[host safety and ending plan](../docs/host-safety-and-ending-plan.md).

## The restore guard (S1, S2)

### S1 (confirmed)

G1 writes a changed mark in place with no sync and drops a mark whose checksum
fails. After a power failure the mark on disk can be older than what the key
signed, so a data folder put back from a copy passes the guard and the key
signs at a position it already used.

**Where the review is imprecise.** Nothing false. Two things are left out. It
does not say that an ordinary start on the original database repairs a short
mark (HP:1478-1480), so the dangerous order is power failure, then restore,
with no clean start between. It says a torn mark has the same problem as a
short one; it is worse, because the old value is lost with the new one.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | G1 Changes, the marks.rs bullet at lines 1330-1333, and the commit contract at 1308-1313 | Replace 'A changed mark is one positional write in place, not synced' with: every commit that carries a mark writes its marks and syncs the marks file once before commit returns. Creating or replacing the file also syncs its directory, as locust-store/src/files.rs:44-49 already does for the object directory. The start's own mark commit (1478-1480) falls under the same rule, so it is durable before the transport starts. | sentence |
| host-safety-and-ending-plan.md | G1 Changes, same bullet at lines 1332-1333, and the test name at line 1600 | Replace 'A record whose checksum fails is left out' with: a file that holds a record whose checksum fails reads as lost (kept is None). A torn record then leads to the rows for lost marks in the first table, never to a key with no mark that signs at once. The plan author may instead keep two slots per mark written in turn, so the older value survives. | sentence |
| host-safety-and-ending-plan.md | G1 Tests at lines 1599-1604 and Exit criteria at 1723-1754 | Add store tests at the crash points. A failed mark sync, injected with the existing Point::FileSync (locust-store/src/faults.rs:9-14), breaks the store and nothing is released. A commit whose mark was never written is raised at the next open before any exchange. A torn record reads as lost. Add a marked commit to locust-store/examples/commit_latency.rs and record the number beside the table at locust-store/src/lib.rs:71-75. | phase edit |
| host-safety-and-ending-plan.md | G1 Risks and notes, residual 7 at lines 1784-1786 | Delete residual 7. What remains is a crash between the database commit and the mark's sync, and the records it leaves unmarked never left this computer. | sentence |
| host-safety-and-ending-plan-details.md | The write rule at line 104 and the unsettled item at line 201 | State the synced write rule. Replace 'Not settled: the unsynced mark' with the measured cost of the second flush. | sentence |

### S2a: the host's hold ends after hearing from the computers the old copy lists (confirmed)

After a whole-computer restore the host's hold ends by itself once every
computer the old copy lists has answered. Computers admitted after the copy
are not asked, and a listed computer that is stale or was removed answers with
nothing, so the hold can end while later host records exist.

**Where the review is imprecise.** Nothing false. The review presents as a
discovery what the plan already lists as residual 4 and labels 'not proof'. It
does not say what the fork looks like. In this trace no computer holds both
records, so nothing halts and nothing is shown: B is back in the goal, C is
shut out, and the halt comes only if the two branches ever meet.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | G1, second table row at line 1259, third table rows at 1281-1286, Intended behavior at 67-70 | Delete the row 'Unheard after a copy of unknown age, on the host's computer'. Such a hold then falls under 'Everything else waits for the person' (1263). The host still dials every listed computer and calls back unknown callers while held (1536-1555), so records keep returning. | phase edit |
| host-safety-and-ending-plan.md | G1 test at lines 1626-1627, drill at 1740-1743, residual 4 at 1770-1776 | The test and the drill expect the hold to stay until goal.continue. Residual 4 becomes a case of residual 8, the person continuing too early. | phase edit |
| host-safety-and-ending-plan.md | G2, waiting_for at lines 2003-2009 and the plan of goal continue at 2024-2030 | List an Unheard hold on a goal this computer hosts under 'Waiting for you' from the start. The plan of goal continue adds one sentence: wait until the computers of the members you added most recently have been on. | phase edit |
| roles-and-permissions-plan.md | Lines 246-251 and the Phase 10 recipe at 4096-4099 | 'Nobody runs a command' becomes one command for the recipe that copies back both the data folder and the marks folder. | sentence |
| master-plan.md | Answer 13 at lines 90-91, item 4 at 285-288, the public-goal gap at 310 | Only after the owner answers the question below. If the answer is to wait: answer 13 covers every goal the person hosts, and the public-goal gap closes because no door member is waited for. If the answer is to carry on: add the question beside question 17 in the host plan (after line 3390) and one sentence on the rare break in Intended behavior. | sentence |

### S2b: the farm service's answer Current ends a hold (confirmed)

The service hook lets the answer Current end an Unheard hold. Current shows
only that the service saw no newer page request, so a host record signed after
the copy and before any accepted request is missed and the host signs at its
position again.

**Where the review is imprecise.** The trace needs a stretch with no accepted
page request between the copy and the loss, and the review does not say how
that happens. For an open, published, reachable page a request is accepted
within about 30 seconds (farm.rs:851-859, 953), so the stretch needs an
unreachable service or a suspended page (farm.rs:822-833, 842). The review
also does not note that the contract limits this path to a copy that shows no
member on another computer (contract:275-278) and that G1 drops the limit.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | G1, guard_attest at lines 1448-1462 and the public-goal row at 1286 | guard_attest keeps Behind only. Remove Current and the sentence that the check can end an unheard hold. The row reads: the farm-service check can add a hold and never ends one. | sentence |
| host-safety-and-ending-plan-details.md | Line 116 (the hook) and line 179 (its two tests) | Drop Current, its two limits and the test attest_current_clears_an_unknown_copy_and_never_a_known_gap. | sentence |
| research/joinable-farms-rewrite-contract-2026-10-05.md | Way (2) of the catch-up gate at lines 275-278 and its J3 reading at 557-560 | When the contract is revised, remove way (2). The service's 409 stays as the signal that this copy is behind. | sentence |

### S2c: the planned model leaves the failing case out (partly)

The planned checks inject a restore that loses the marks only where nobody is
admitted after the snapshot, so the claimed invariant never meets the trace of
S2a.

**Where the review is imprecise.** The review says the model omits the failing
trace and asks to include it. The TLA model in the plan already keeps it as a
named counterexample (HP:3267-3268). The omission is in the simulator
invariant and in the wording of HD:175, which is the line the review cites.
The review also misses that the stated condition is too weak for a member's
machine.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | G1 Tests, the simulator bullet at lines 1707-1717, and the exit criterion at 1747 | State where the invariant holds. With marks kept: for the governance key always, once S1 is fixed. With marks lost: only on the host's machine and only with no admission after the snapshot, or on the host's machine always if the row at 1259 is deleted. Never claimed for a member's machine with marks lost. Add two seeded runs that must end in a reused position: residual 4 and residual 6. | phase edit |
| host-safety-and-ending-plan-details.md | The behavior row at line 175 | Put the conditions in the claim itself, not only in the 'shown by' column. | sentence |
| host-safety-and-ending-plan.md | Models written first, the restore guard paragraph at lines 3255-3268 | Add residual 6 to the named counterexamples. Check property 1 against the give-up rule with a removal (see also_found). | sentence |

Also found in this area:

- A torn in-place write loses the whole mark, not one record. The record is
  left out (HP:1332-1333), Behind needs a mark to exist (HP:1403), so after
  any data-folder restore that key signs at once, however old the copy.
- The shared bit is carried by the same unsynced write (HP:1302,
  HP:1507-1509). The first admission of a member on another computer is the
  write that sets it. If that write is lost, the rule at HP:1256 ends a
  governance hold at once on a goal that was shared.
- Residual 7 says the mark is 'one record short' (HP:1785). Nothing bounds it
  to one. Every in-place write since the last write-back is lost together, and
  after R8 and R9 the governance key signs on every plan change and landed
  file change (RP:238-240).
- Model property 1, 'NoFork after RestoreStore in every run without Continue'
  (HP:3255-3256), looks false inside the model's own actions. An agent's
  record reaches one peer. That peer is removed. The store is restored with
  marks kept. The remaining peer answers. The rule at HP:1257 lowers the mark
  and the agent signs at the used position. The removed computer still holds
  the first record and brings it back if it is ever admitted again. The cost
  is one agent's key (HP:1783-1784), and for the host's agent that also stops
  first files (HP:54-56). The property needs an exemption or the rule needs a
  condition.
- The service's Behind sets Unheard without condition (HP:1453-1454). After a
  restore of the data folder alone, the page counter is behind, so a public
  goal becomes Unheard even though kept marks already give proof under HP:1255
  or HP:1256. A lone public host then waits for the person, and a shared
  public goal waits for every door member (HP:39-42). The third table says a
  public goal gets 'the same answers' (HP:1286). It does not. Behind should
  set Unheard only at a start the first table called ordinary.
- No owner question covers the host-side guess of residual 4. Question 17
  (HP:3387-3390) asks only about the member-side guess, where the cost is one
  agent's key. The larger risk was never put to the owner.
- When the host's hold ends on the guess, the result is two branches and no
  message anywhere. The restored host refuses the computers it does not know
  (responder.rs:116, 156), never dials them (peers.rs:179-198), and
  note_caller works only while held (HP:1537-1540). HP:1292-1293 adds that no
  view records how a hold ended, so a later halt cannot be traced to it.
- The tests named for the marks file (HP:1599-1604) cannot exercise a lost
  unsynced write. The store's crash tests state they cannot simulate a power
  loss (locust-store/tests/crash.rs:6-8). The fault points that exist are for
  syncs (locust-store/src/faults.rs:9-14), and the planned write has none.

## The key and the founding agent (S4, S5)

### S4a (partly)

The key for members and rules is given more to sign than the words of answer 9
allow: a stage's steps in K1, the plan's text in R8 and landed file changes in
R9. The review says this is an exception that still has to be written down as
one exact list with what happens when a signing goes wrong.

**Where the review is imprecise.** It says an exact list of permitted bodies
is still needed. K1 already has one by record kind and has replay enforce it
(host plan 638-641, 681-682). The gap is the list of situations and the
failure handling for each, and the host plan's Known gap 2 already names it
(host plan 34-37). Minor: answer 9 is at master 69-71 in 33002ea, not 70-72.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | K1, the table 'Who signs what after this phase' (590-600) and the host_may_sign bullet (638-641) | Make the table the one list. Give one row to each of the seven situations in which the key signs with nobody present (count 157-167), including the removal after a leave under answer 14. For each row say what a second attempt signs and what happens while the computer is catching up, while the goal is halted and after it is ended. Or point each row at the unattended-signing contract once that is written. | phase edit |
| host-safety-and-ending-plan.md | Questions for the owner, question 1 (3313-3318), and the companion's 'Not settled' entry at docs/host-safety-and-ending-plan-details.md:96 | Mark the question withdrawn. Say that the master (290-293) leaves it to engineering and that one key and one log are kept. | sentence |
| master-plan.md | Answer 9 (76-78), as line 293 already promises | Once the contract is settled, restate the answer: the key signs members, rules and what the host's computer records by itself, and never a member's work. | sentence |

### S4b (confirmed)

After K1 the creator of a stage's task is the governance key, which is no
member. A hand-written rule that names task_creator as the one who must post,
declare, review or attest in a stage's task can then be met by nobody. K1 says
so but adds no validation error.

**Where the review is imprecise.** 'Publish' is ambiguous. A formation cannot
name task_creator in work.publish at all: validation refuses it as
selector_scope (organization/validation.rs:54-59, 92-93,
docs/reference/conformance/organization.cases.json:310). The case that holds
is the completion rule 'a result posted by the task's creator counts', which
K1 calls 'post' (host plan 1072-1073). The review, like K1, leaves out start
rules and subtasks (see also_found). The line that fixes the creator for the
task's own rules is goal/rules.rs:64-74. The cited goal/flow.rs:39-56 only
feeds the choice of recipients.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | K1, Changes: a new entry for crates/locust-core/src/organization/validation.rs, placed after the goal/flow.rs entry (696-699) | In the stage loop of Validator::run, report selector_scope when the rules a stage's task gets name task_creator where a member must act: an independent start, the 'to' of an offer, or any completion criterion. The rules are the stage's task type's, or the formation's defaults where the type sets none. Keep 'offered by: task_creator', which still works (host plan 1069-1071). Mirror it in sites/locust.farm/src/lib/formation-editor/contract/rules.ts, add one case to docs/reference/conformance/organization.cases.json and one clause to docs/formations.md:39. It lands with K1's protocol step because replay runs validation (goal/mod.rs:42-50, goal/chain.rs:476, goal/rules.rs:118). | phase edit |
| host-safety-and-ending-plan.md | K1, Risks and notes (1067-1075) and the test a_stage_task_names_the_governance_key_as_its_creator (879-882) | Replace 'can be met by nobody' with 'is refused when the rules are checked', and have the test assert the diagnostic. | sentence |

### S5 (confirmed)

K1 keeps the agent a goal was started with a member for good, yet lets the
person disconnect it for good. After that the goal's first files cannot be
shared and that agent's consent to a page cannot be given, and the only way
back the plan offers is a later release.

**Where the review is imprecise.** Three imprecisions, none of which breaks
it. First, it repeats 'waiting for a future backup host' as the way back
without noting that the release with backup hosts ends every v2 goal (master
235-236). Second, the consent loss is certain only for a page with no public
door. The public-goals contract makes the host's publication record the
consent on a joinable farm (contract 295-302), that contract is marked for
revision, and the companion leaves the case undecided (companion 91). Third,
'cannot share first files' is true of the commands, not of the signed rules: a
member's change with no parent in an empty epoch counts once another member
approves it (roles plan 2520-2523, 3727-3731, 3958-3959), but only the raw
door can post one (roles plan 2245-2248, 4188). Its two remedies (separate
identities, or a signed replacement of the working agent) are redesigns and
are not needed to close what it names.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 2, the command table row for agent revoke (1109) and the list of commands that ask first (1156-1158). Follow-on lines: master 164-166, host plan Intended behavior 53-56, K1 Risks 1085-1102, K1 test at 920-927 and exit criteria 1005-1008, E2's agent revoke lines 2867-2878, host plan question 9 at 3349-3356 | Add a way to connect a disconnected agent again, kept on this computer only: the stored key gets a working credential again and no goal record is signed. agent revoke then applies at once and prints its undo line (answer 7, master 67-70). The agent stays a member throughout, so Phase 4's reasoning that only a host's agent is ever a goal's only member is untouched (roles plan 2048-2051). | phase edit |
| host-safety-and-ending-plan.md | K1 Risks 1054-1055, questions 2 and 9 (3319-3322, 3355-3356), and E2's agent revoke plan lines (2871-2873) | Needed whether or not the undo is adopted. Stop saying a way back comes with the backup host, and say that a goal made under v2 gets none, because that release ends these goals (master 235-236). Add the page line to the agent revoke plan, which today names roles and first files only, as question 9 already proposes (3353-3354). | sentence |

### S5b (confirmed)

If the plans ever let a host remove the agent a goal was started with, the
rule that a lone member needs no approval must change too. Otherwise a member
who came through the public door could become the only member and have results
count with no approval.

### S5c (confirmed)

Losing the governance key itself (putting back a copy of the data older than
the goal) loses membership, rules, ending and automatic recording for good,
because v2 has no takeover. The review accepts this as a stated limit of the
first release and not a reason to make people set up a backup host.

**Where the review is imprecise.** Its master line numbers (77-80, 111-114)
are those of 33002ea. In the current file answers 11 and 22 are at 83-86 and
120-123. The substance is unchanged.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | K1 Risks 1022-1024 and question 8 (3345-3347) | Drop 'until a backup host exists'. For a goal made under v2 the loss is for good, because the release that brings backup hosts ends these goals (master 235-236). | sentence |

Also found in this area:

- Start rules are hit too, and neither K1's note (host plan 1067-1075) nor the
  review lists them. Validation allows task_creator in a start rule
  (organization/validation.rs:97-103). On a stage's task, 'independent by:
  task_creator' lets nobody start, because the governance key cannot sign an
  attempt (goal/fold.rs:315, host plan 681-682). 'offered to: task_creator'
  yields no offer, because recipients are members and the key is none
  (goal/flow.rs:272, goal/fold.rs:298).
- The dead selector spreads to subtasks. A subtask opened under a stage's task
  inherits its rules with task_creator frozen to the parent's creator, which
  after K1 is the governance key (goal/delegation.rs:11-21, 33-47). A subtask
  type that uses task_creator itself must fit inside the parent's set and so
  fails to narrow (goal/delegation.rs:75, 86-92, 127-152,
  goal/fold.rs:547-553). K1 speaks only of 'a stage task' (host plan
  1072-1073).
- A lone host who swaps agents loses the lone-member rule. Read from the
  plan text, not measured. After 'agent revoke maple', 'goal add juniper'
  succeeds and 'member remove maple' is refused (host plan 1005-1007). The
  goal's record then holds two members, because a second agent of the host's
  person is a second member (roles plan 2044-2051). Under the default rule
  Juniper's results need another member's approval (roles plan 2119-2121),
  and the only other member can never sign. The person must add a third
  agent (roles plan 4030) or bind 'open'. For shared files set up under the
  default rule only a third agent helps, because the tree keeps its rule and
  no command starts a new epoch (roles plan 4017-4021). E2's revoke plan
  says only 'The goal keeps running' (host plan 2869-2870). The undo
  proposed under S5 does not close this. Only a signed replacement of the
  starting agent would, which is the review's larger alternative.
- The remedy 'comes with the backup host' appears five times in the host plan
  (1022-1024, 1054-1055, 3319-3322, 3345-3347, 3355-3356) and can never apply
  to a goal made under v2, because replacing a host 'ends the goals made under
  v2' (master 235-236).
- The host plan still restates answer 3 in the words the owner replaced on 6
  October (host plan 93-94 against master 46-53), and still asks owner
  question 1 that the master withdrew (host plan 3313-3318 against master
  290-293).

## Rules and local settings (S6, S7, S8, readiness)

### S6 (first files, once per empty epoch) (partly)

The first-files exemption holds once per epoch that starts empty, not once per
goal. A host who empties the tree can share a new starting folder with no
approval. The review says this breaks answer 16 and must be tracked across
epochs.

**Where the review is imprecise.** It is filed under soundness, but no two
computers disagree and the host gets no power it lacks. It does not say that
the reset needs the raw API and that no plan command reaches it. It does not
answer the plan's stated objection (roles plan:4133-4137). Its fix reads
earlier acceptances. From Phase 4 to Phase 8 those are signed by the host's
agent, a member's log that can arrive late (roles plan:2360-2365; that case is
tested today at workspace_tests.rs:653-672). Only from Phase 9 are they in the
governance key's log (roles plan:3738-3741). So 'need not be a new signed
body' is true, but the fix is a new replay rule in two phases, not a free
tightening.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md | 'Assumed until the owner objects', after line 167 | Add one line: 'First files means the first files of an empty tree. A host who empties the shared files through the raw API can share a starting folder again with no approval.' The owner then sees the reading the plan gives answer 16 and can object. | sentence |
| roles-and-permissions-plan.md | Phase 4 lines 2208-2229 and 2672-2684, Phase 9 lines 3756-3760 and 4022-4024, 'Not built, and why' lines 4129-4141, test at 2528-2531 | Only if answer 16 must hold to the letter. From Phase 9 the first-files answer also requires that the governance key's log holds no acceptance in an earlier epoch of the goal. A folder shared after a reset then follows the tree's rule. The cost is the one the plan names at 2688-2690: a reset tree in a small review-panel goal needs a rule the members can meet. | phase edit |

### S6 (file rule after a rules change) (confirmed)

After `rules bind`, changes to the shared files still follow the rule that
`workspace init` printed. No command in the plan changes that rule.

**Where the review is imprecise.** Nothing in the trace. Its remedy leaves two
details open. No built-in formation carries a `workspace` part, so `rules
bind` must add one the way `initial_epoch` does (cli/workspace.rs:395-423). A
tree rule the host gave with `--completion` (roles plan:3820-3821) must not be
replaced silently.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 2 line 1106 (`rules bind` row), Phase 4 lines 2249-2257 (`rules_bind`), Phase 9 lines 3806-3815 and 4017-4021 | If the owner answers yes. In a goal with shared files, `rules bind` also starts a new epoch that carries the current files under the new rules, in the same confirmed run. The record type exists today (a `Revision` checkpoint). Its plan and result say that shared files follow the new rule and that changes not yet landed are proposed again. The new formation gets the `workspace` part the way `initial_epoch` adds it. `pending` lists an author's proposals from the old epoch as behind, so agents rebuild without a person. | phase edit |
| roles-and-permissions-plan.md | Line 1106, lines 3819-3821, lines 66-68; and docs/master-plan.md under 'Assumed until the owner objects' | If the owner answers no. `rules bind` prints 'Shared files keep the rule they were set up under: RULE.' `workspace init` prints 'Every later change follows this rule: RULE.' The master says that the rule is the one in force when the files were first shared. | sentence |

### S7 (confirmed)

The `Undo:` line printed after `role take` does not restore the earlier
holders when the take empties a group role. The host's agent stays a holder
after the undo.

**Where the review is imprecise.** Nothing in the trace. It is one of six give
and take cases. The plan states the outcome itself at roles plan:2334-2335, so
the defect is a label, a test name and a companion row that contradict a
sentence beside them.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 4 lines 2329-2335 and 2431-2436; companion docs/roles-and-permissions-plan-details.md line 298 | For a take that empties a group role, print no `Undo:` line. Print 'Give it back:' with the `role give` command, and one sentence that the host's agent keeps the role until it is taken, with that command. This follows the 'Invite again:' precedent (roles plan:1177-1178). Reword 2329-2330 and the companion row to say which case is the exception. | sentence |
| roles-and-permissions-plan.md | Phase 4 lines 2095-2099 (`RoleGive`), 2271-2275 and 2333-2335 | Alternative that keeps one command. The undo of that take is a `role give` that sets the list to exactly the earlier holders. The signed `RoleHolders` body already carries the whole list (roles plan:2065-2067) and the request already carries `expected`. It needs one field or flag. Phase 2's own rule, that a command applies at once only when one command undoes it (roles plan:1144-1148), and answer 1 favour this one. | phase edit |

### S8 (confirmed)

A task allowance is not cleared when the task finishes. If the only approval
is later withdrawn, the same round can be taken again and the old allowance
counts again, although the person was told it ends when the task is finished.

**Where the review is imprecise.** 'Never clears' is too strong. Leaving,
removal and `allow --revoke` delete the record, and a want replaces one that a
revision ended (roles plan:1713-1715, 1718-1722 and 1977-1978). The plan
states the revival itself (roles plan:1973-1976), so this is a conflict
between sentences, not an unnoticed behaviour. The last sentence is overtaken.
Answer 18 as restated has a trusted agent approve a door task and asks no
person (master-plan.md:107-112 and 272-276), so that check no longer rests on
this allowance.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md and roles-and-permissions-plan.md | Master line 154; roles plan lines 347, 1601-1603, 1765-1766, 1973-1976, 3031 and question 17 at 4213-4217 | Say what the design does. An allowance is for the task as it stands. It lasts until the host revises the task or the person revokes it. A finished task cannot be taken. If it opens again because an approval was withdrawn, the allowance still holds. The printed line becomes 'AGENT may take "TASK" in "T" until the host revises it.' This is the reading that asks less of the person (answer 1). | sentence |
| roles-and-permissions-plan.md | Phase 3 lines 1963-1976 and the test named at 1808 | Only if the owner wants to be asked again. This daemon deletes the allowance when it sees the round finished, and the agent's next start is recorded as wanted. It asks more of the person. | phase edit |

### Readiness: documents setting (confirmed)

Phase 8's optional `documents` setting is set by `peer-review` and
`review-panel` only. `open` and `pipeline` get no current plan text, although
the master assumes one rule for the plan and the files under every formation
that names no decider.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 8 lines 3553-3566 and 3684-3693, question 15 at 4209-4211; companion docs/roles-and-permissions-plan-details.md lines 454-461; research/joinable-farms-rewrite-contract-2026-10-05.md lines 409-413 | Drop the `documents` part. In `resolve`, a document scope whose rules name no selection authority is agreed and its recorder is the governance key. Delete the note that the two phases disagree and close question 15 as assumed. No formation hash changes for this part (roles plan:3630-3633 goes). This matches the master sentence for every formation, hand-written ones included. | phase edit |
| roles-and-permissions-plan.md | Phase 8 lines 3557-3560 | Smaller alternative. `open` and `pipeline` set the part too, and master line 157-158 is narrowed to 'every built-in formation'. A hand-written formation with no decider still gets no plan. | sentence |

### Readiness: opinion review under open (confirmed)

The master assumes a review can be recorded under `open` as an opinion that
does not count. No phase changes who may review, and today a review outside a
review rule is excluded.

**Where the review is imprecise.** It names Phase 3's row only. The exclusion
is in the fold, so the change belongs in Phase 4 with the other signed-format
changes, and Phase 3's row follows it.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 3 lines 1634 and 1919; Phase 4 lines 2170-2176 and 2631-2633; Phase 5's review display; question 14 at 4204-4208 | Write the rule. A member's review of a result is recorded whether or not the rule asks for it. It counts only where the rule admits it, which `predicate` already tests on its own (fold.rs:883-896). Views show it apart from counting verdicts. Say whether an opinion by a member who no longer qualifies withdraws that member's earlier approval. Close question 14 as assumed. | phase edit |
| master-plan.md | Lines 155-156 | Stopgap until the phases are edited. Add 'not yet in a phase; roles plan question 14', so line 144 stays true. | sentence |

### Readiness: combined proposal in a goal of two (confirmed)

A file change that combines both members' changes in a two-member goal can
never meet a review rule that excludes authors. Rebuilding one's own change
avoids it.

**Where the review is imprecise.** Nothing. It restates a limit the plan
already names, tests and reports, so it asks for no change.

### Readiness: selector_scope and a joinable tree (confirmed)

Phase 9 says `selector_scope` already refuses open selectors in the tree rule
of a joinable goal. The code check does not do that.

**Where the review is imprecise.** Its last sentence, that the future door
needs its own check, is already planned in the contract's J2. The review does
not cite that. The contract cites the roles plan sentence by old line numbers
(3226-3227).

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 9 lines 4031-4035 | Replace '`selector_scope` already refuses open selectors there' with: 'The door's safety check refuses such a rule (J2 of the public-goals contract). `selector_scope` only refuses `task_creator` in the tree's rule.' Update the line reference at contract lines 93-95 and 1219-1220. | sentence |

Also found in this area:

- After `rules bind` the shared plan follows the new rules at once and the
  shared files keep the old ones. A document context is the current rules
  binding (crates/locust-core/src/goal/rules.rs:38-43;
  crates/locust-core/src/goal/fold.rs:669-680;
  docs/roles-and-permissions-plan.md:3549-3552 and 3677-3680). A file context
  is the epoch's pinned binding (rules.rs:44-49). So
  docs/master-plan.md:157-158 fails after any rules change. Carrying the files
  to the new rules, as under S6, closes it.
- `workspace init` binds new rules in order to add the `workspace` part
  (crates/locust/src/cli/workspace.rs:395-423; kept by roles plan:849 and
  2366-2368). Under Phase 8 a plan revision is in line only when posted under
  the current rules (roles plan:3549-3552). So running `workspace init` puts
  every waiting plan revision behind. Neither Phase 8 nor Phase 9 says so.
  This is read from the text, not run.
- Phase 9 tells a host who wants the last look to write a required approver
  into the tree's rule (roles plan:3988-3989; question 12 at 4199-4200). After
  `workspace init` no command changes the tree's rule (roles plan:4020-4021).
  Same root as S6.
- The undo of `allow` is not exact either. `task.disallow` also drops the
  agent's want (roles plan:1769-1771). After `allow` and then its `Undo:`
  line, a 'wants to take' line that was there is gone until the agent tries
  again. Same class as S7. The plan states it and still calls the line
  `Undo:`.
- A file change that nobody may approve is reported only in `goal status`
  (roles plan:3868-3871). `pending`, the list an agent works from, gains only
  `stale_files` (roles plan:3806-3809 and 3814-3815). The authoring agent gets
  no signal in its own list and can wait for an approval that cannot come. One
  sentence adding the same line to the author's `pending` closes it.
- Writing the opinion review needs a decision the plans do not mention. Phase
  4 says a member whose role was taken can no longer review, so its last
  review while it held the role is the one read (roles plan:2631-2633). If any
  member may record an opinion, that member can record a later reject. The
  plan must say whether it withdraws the earlier approval.
- `role take` of the host's agent from a group role it alone holds is not
  specified. The list empties and falls back to the same holder (roles
  plan:2271-2275). The companion covers only the deciding case
  (docs/roles-and-permissions-plan-details.md:288). The daemon should answer
  that nothing changed and sign nothing.

## Ending and versions (S9)

### S9: an end held behind a missing record leaves the page open (partly)

E1 gives an end three states: in force, held behind a missing earlier record,
and cut by a fork. All three stop signing on that computer, but only the first
marks the public page ended. So a host's computer that holds its end behind a
missing record would say "ended" in status while its page reads open and keeps
checking in.

**Where the review is imprecise.** Three details. First, the review sets the
missing-record case beside the fork case as a condition before the first door.
On the one computer that publishes, ordinary sync does not produce it. It is a
hole in E1's definitions that is cheap to close, not something a person meets.
Second, "unacknowledged suspension" is not accurate. E1 says a copy that is
ended then halted "shows a blank page" (plan 2676 to 2678). What E1 lacks is
the deletion date of that blank page, and its retention sentence is stale (see
the deadline finding). Third, the review misses that E1 contradicts itself
here. Line 2590 to 2591 says an end stops page check-ins on any computer that
holds it. Lines 2355 to 2358 with 2366 to 2368 keep them for a held end.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | E1, Changes, the node/farm.rs bullet, lines 2355 to 2368 | Replace "`state.ended` set gives `FarmGoalState::Ended`" with "`Goal::end_held()` set gives `FarmGoalState::Ended`". Add one sentence: a copy that is ended, then halted still fails `eligible` on the halt and suspends the page, as today. The page then agrees with status, check-ins stop on every copy that holds an end as line 2590 already claims, and the first ended upload starts the 30 days. | phase edit |
| host-safety-and-ending-plan.md | E1, Tests, the tests/farm.rs bullet, lines 2512 to 2518 | Add one test: the page reads ended and check-ins stop while the end is held behind a missing record, and the page stays ended when the record arrives. | phase edit |
| host-safety-and-ending-plan-details.md | line 229, "What the public page reads for ended" | Say `Goal::end_held()` in place of `State.ended`. | sentence |

### S9: a forked host log suspends the page, so the count is too broad (confirmed)

When a fork of the host's log cuts an end, today's publisher suspends the
page. The complexity count says the page reads open in that case, which is
wrong for the fork.

**Where the review is imprecise.** Nothing wrong, one thing incomplete. The
claim is about what the host's daemon decides. Whether the service's page
changes is a further step, and in the restored-host case the service is likely
to refuse the request (see the third item under also found).

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| research/v2-complexity-count-2026-10-06.md | "The smaller ones", first bullet, lines 297 to 299 | Split the sentence. An end held behind a missing record has no page state: the page reads open. An end cut by a fork blanks the page; it is removed on the original date only if it had already read ended. | sentence |

### S9: a fork does not hide later records from sync between members (confirmed)

Ordinary sync between two computers that are still members passes on every
record of a forked log, including the records after the fork and so the end
record. It does not reach a computer that is no longer a member.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | E1, Tests, `a_restored_host_that_signs_before_recovery_cuts_the_end_on_both_computers`, lines 2505 to 2508 | Replace "the host's reads the same once the end record has reached it" with a firm expectation: after one exchange in each direction the host's daemon holds the end record and reads ended, then halted. | sentence |
| host-safety-and-ending-plan.md | E1, "Read and inferred", lines 2794 to 2797 | Move the fork clause from "Inferred and not run" to read: a record beyond a fork of the governance key's log reaches every computer that is still a member on the copy that serves it (read in outbox.rs, initiator.rs and screen.rs; not run). | sentence |
| host-safety-and-ending-plan-details.md | "Not settled", line 298 | Delete the line, or restate it as settled by reading with the limit above. | sentence |

### S9: suspending an ended page keeps its deletion deadline (confirmed)

The farm service keeps the first ended time when a page is suspended and
deletes the page at that time whether it is up or blank. The companion says
the opposite and is stale.

**Where the review is imprecise.** It cites only companion line 296. The same
stale claim is in the plan body at lines 2627 to 2631 and in the companion at
line 253, and the plan body is what a builder reads.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | E1, Risks and notes, "The page can still be blanked after the end", lines 2627 to 2631 | Replace the last two sentences with: a blanked page shows no names. The service keeps the first ended time across a suspension and deletes every page past that time, up or blank, so a page blanked after it read ended is removed on its original date. A page that never read ended has no date and stays blank until `farm off`. | sentence |
| host-safety-and-ending-plan-details.md | line 296 and the last clause of line 253 | Replace both with the same fact and name the test `suspended_ended_farm_expires_at_original_deadline`. | sentence |

### Version boundary: E1 is not the last change of the event format (confirmed)

The plans call E1's end record the last change of the event format. The public
door, which is part of v2, later adds a signed field to the admission record.
The two cannot both be true.

**Where the review is imprecise.** It understates the case. The contract adds
two more signed fields besides `via`: `DisclosurePolicy.joining`, which is
inside the policy digest, and `PublicationSet.goal_proof` (contract 228 to
230). It also misses a third numbering: the roles plan tells the door rewrite
to take "versions 8, not 7" (roles-and-permissions-plan.md 3241 to 3242),
while the master gives 8 to replacing a host and the contract says 8 only if a
roles release ships first.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md | Build order, the "Versions" paragraph, lines 239 to 242 | Replace "E1's end record is the last change of the event format" with: E1's end record is the last change of the event format in the sixteen phases. The door's first admission phase changes signed bytes once more (`MemberAdmitted.via`, `PublicationSet.goal_proof`, `DisclosurePolicy.joining`). Then state one of two lines, by the owner's answer below. If nothing is released before the door: that change is inside 7 and the format is frozen at the door. If the sixteen phases are released first: the door takes 8 and replacing a host takes 9. | sentence |
| roles-and-permissions-plan.md | Implementation sequence, lines 638 to 642, and the note for the door rewrite, lines 3241 to 3242 | Carry the same wording, and make "versions 8, not 7" follow the master's line. | sentence |
| host-safety-and-ending-plan.md | K1 "Depends on", lines 561 to 564, and E1 "Versions", lines 2730 to 2733 | Carry the same wording. | sentence |

Also found in this area:

- E1 contradicts itself on check-ins. docs/host-safety-and-ending-plan.md 2590
  to 2591 says an end stops page check-ins on any computer that holds it.
  Lines 2355 to 2358 with 2366 to 2368 tie check-ins to `state.ended`, so a
  held end keeps them. The edit proposed under the first finding removes the
  contradiction.
- The stale retention claim is in the plan body too, not only in the
  companion: docs/host-safety-and-ending-plan.md 2627 to 2631, and
  docs/host-safety-and-ending-plan-details.md 253.
- In the one route E1 gives to a cut end, a restored host, the page at the
  service probably does not change at all. The publisher numbers its requests
  from a counter in the data folder, and G1 says that counter is behind the
  service's after a restore (docs/host-safety-and-ending-plan.md 1850 to
  1856). The service answers 409 to a different request at a used number and
  to any number at or below the stored one (crates/locust-farm/src/lib.rs 870
  to 885, 901 to 903), and it never deletes a receipt. The daemon retries the
  same request for good (crates/locust-core/src/node/farm.rs 882 to 889, 953
  to 957). So the Suspend is refused and the page keeps what the service last
  accepted before the loss. If that was the ended upload, the page reads
  ended and is removed after 30 days, which is what answer 23 asks. If the
  ended upload never left the lost computer, the page reads open and quiet.
  E1's "shows a blank page" (2676 to 2678) and the count's "reads open" (297
  to 299) are each right for one branch only. Read, not run.
- Signed bytes also change after E1 inside the sixteen phases. R8 changes
  every formation's hash, regenerates the constants in vectors.rs and rewrites
  `signed_current_protocol_vectors_are_frozen`
  (docs/roles-and-permissions-plan.md 3630 to 3633 and 3650). The master's "R4
  and E1 change signed bytes inside 7" (docs/master-plan.md 239 to 240) leaves
  R8 out. No number is affected, because nothing is released in between.
- E1's phrase "reaches every member by ordinary sync"
  (docs/host-safety-and-ending-plan.md 2796 to 2797) is wider than the code
  allows. A member admitted at or after the fork position is no longer a
  member on a copy that holds both branches, and its frames are refused
  (crates/locust-core/src/sync/responder.rs 116).

## Whether each phase can land (section 2)

### R1 row (confirmed)

R1 removes interfaces that the guide's executable recipes and the script
harnesses call, and the plan leaves them failing until R6. CI runs those
recipes and the Python tests on every push to main today.

**Where the review is imprecise.** The row says R1 must also update "generated
contracts". R1 already does: the generated files and site.json are in R1's
changes (ROLES 1021-1023). "No as scoped" is a judgement. Under the owner's
stated tolerance it is a known blind spot, not a blocker.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | R1 notes and exit criteria (970-974, 1013-1023); the matching notes of R2 (1488-1490), R3 (1994-1996) and R4 (2704-2706) | Closes it. Each of R1 to R5 rewrites, for the interfaces it removes, the four marked recipes and the harness code the three real-binary Python test files reach (ProductionDaemon, and the enroll, seed and prepare helpers they import). Each adds the recipe run and the Python test run to its exit criteria. The "fail from here until Phase 6" notes become change lists. | phase edit |
| roles-and-permissions-plan.md | R6 recipes, scripts table, tests and notes (3189-3218, 3284-3292, 3330-3337) | R6 keeps the prose, the site, the skill and the scripts CI does not run. Its recipe and harness rows shrink to what R1 to R5 did not already do. Line 3335 is corrected. | phase edit |
| roles-and-permissions-plan.md | Implementation sequence, lines 624-625 and 642-643 | The cheaper alternative, which accepts the risk and only makes the plan true. Say that from R1 until R6 the CI steps "Executable manual recipes" and "Python helper tests" fail, that check_docs.py therefore does not run in CI, and that agents run check_docs.py and the cargo checks locally in that time. | sentence |

### R3 row (partly)

R3 adds a pending-list field that shows each member's latest review. Counting
by latest review arrives in R4, two slots later. In between, a member who
approved and then rejected still counts while the list shows the reject.

**Where the review is imprecise.** The row offers two remedies. The second,
"make R3 truthful about its temporary behavior", is already met at the very
lines it cites (ROLES 1983-1986). The row reads as a landing condition and is
not one.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | R4, the views.rs sentence at 2187-2188 | Optional. Say that verdicts and approvals both read Verifier::latest_review, so R3's scan in views.rs (1746-1749) and R4's function in fold.rs (2172-2173) are one implementation of "latest". | sentence |

### R4 row (confirmed)

R4 builds the rule that roles are read at each record's position. The formal
model of that rule is written only in R7, seven phases later. Latest-review
counting and the only-member rule are in no model at all.

**Where the review is imprecise.** Nothing in the facts. "If retaining model
before code" assumes the master's sentence covers the roles plan. See the
finding "Model promise".

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | R4 changes (replace the note at 2385-2387); R7 model bullets (3402-3447) and exit criterion (3505-3507) | Move the Organization.tla change, the three scenarios and the seven cases to the start of R4. R7 keeps one run of the whole suite. The case counts change order: 33, K1's two, R4's seven, then E1's seven. | phase edit |
| roles-and-permissions-plan-details.md | verification matrix rows at 439-442 | "Phase 7: model cases" becomes Phase 4. | sentence |
| host-safety-and-ending-plan.md | Models written first, 3220-3221, 3244-3246 and 3273-3281 | The order of model changes becomes K1's change, R4's role holders, G1's model, E1's cases. E1's cases are written on the transcript that has the role events. | sentence |

### R9 row (partly)

R9's exit check greps the whole repository for the removed integrate names and
exempts only research/ and the plan file. It would also match planning
documents it does not exempt.

**Where the review is imprecise.** It is not a check for the word integrator.
It is five exact strings, and the one for integrator needs the double quotes
of a JSON key. The bare word in MASTER 97 and in the host plan does not match.
So the check does not reach historical prose in general. It reaches the
companion's cleanup list and one table row. research/ is already exempt.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | R9 exit criteria, line 3966 | "outside research/ and this plan" becomes "outside research/, this plan and its companion". | sentence |
| roles-and-permissions-plan-details.md | R9 prose list, line 483 | Add the workspace integrate row of docs/shared-file-tree-plan.md (line 498) to the sentences R9 rewrites, since the command is gone. | sentence |

### E3 row (confirmed)

E3, slower dialing for a quiet goal, needs neither E1 nor E2 and could be
deferred. It should not hold up release only because it sits at slot 12.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | E3 Depends on, line 2974, and E1's note at 2785-2786 | If E3 stays at slot 12: correct the two sentences, because the door does wait for E3 through R7 and R10. | sentence |
| roles-and-permissions-plan.md | table row 7 (653) and R7 Depends on (3353) | If E3 is deferred: "E1 to E3" becomes "E1 and E2". | sentence |
| master-plan.md | build order row 12 (225) and the assumption at 181-182 | If E3 is deferred: move the row below R10 under "Afterwards" and mark the assumption as later work. | sentence |

### Model promise (partly)

The master plan says each piece is modelled before it is built. The role model
comes after R4's code and the plan stream has no model, so the sentence is not
met.

**Where the review is imprecise.** The review treats the sentence as a promise
about the roles plan. In the master it is listed among the host plan's
assumptions. Its two examples are true facts about the roles plan, but they
show that the master overstates, not that a scheduled model is missing from
the plan the sentence belongs to.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md | line 183 | Replace with what is planned: the signing key, the restore guard and the end record are each modelled before they are built. Role holders are modelled in R4 (or R7, as chosen under "R4 row"). Latest-review counting, the only-member rule, first files, the plan stream and file landing rest on Rust tests only. | sentence |

### Version boundary (confirmed)

The plans call E1's end record the last change of the signed event format. The
public door later adds a signed field to the admission record. Both cannot be
true.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md | Versions, lines 238-242 | Say that E1's end record is the last change of the event format within the sixteen phases, that the door adds via to the admission record, and which number that takes. If nothing is used for real goals before the door, it stays inside 7 and replacing a host keeps 8. If the sixteen phases are released first, the door takes 8 and replacing a host 9. | sentence |
| roles-and-permissions-plan.md | lines 640-641 and 3241-3242 | Same correction, and make the joinable note's "versions 8" agree with the master. | sentence |
| host-safety-and-ending-plan.md | lines 115-116, 563, 2732 | Drop "before replacing a host"; say "within the sixteen phases". | sentence |

### R7 row (confirmed)

R7's two-computer swarm run and its reading test with people are real outside
work, not automated tests, and R10 repeats them. They could be merged into
R10.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | R7 (3344-3525) and R10 (4063-4112) | Optional. Keep the recipes and command budgets in R7. Run the swarm on two computers and the reading test once, in R10, on the finished explanation. | phase edit |

### R2 row (confirmed)

R2 binds each confirmation to the goal's governance head, so any change to
members, roles or rules between showing the plan and the yes makes the command
fail.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | R2, the review fields at 1088-1091 and the goal end row at 1111 | Hash only what the command's effect depends on. Keep current_rules for rules bind and the head as expected for goal end. Drop governance_head from the review of the other commands. | phase edit |

Also found in this area:

- CI steps run in order with no always-condition (.github/workflows/ci.yml
  26-48). From R1 to R6 the failed recipe step also stops the Python tests and
  check_docs.py from running in CI. The review does not say this.
- The phases after R6 already own the scripts and recipes they break (ROLES
  634-635; HOST 1955-1956; HOST 2421; ROLES-D 482). The R1 to R5 gap is the
  exception to the plan's own method.
- ROLES 3335 says the unit and site tests pass between phases. On the macOS
  runner the Python tests that start the real binary do not
  (scripts/tests/test_production_qualification.py 94, 121, 138;
  test_shared_workspace_models.py 192; test_collaboration_acceptance.py 98;
  scripts/client_qualification/production.py 259, 277-283).
- HOST 2974 and HOST 2785-2786 say the first door does not wait for E2 or E3.
  MASTER 226, 229, 233-234 with ROLES 653 and 656 make it wait for both,
  through R7 and R10.
- Two pieces claim protocol 8: the joinable rewrite (ROLES 3241-3242) and
  replacing a host (MASTER 241-242).
- Five rules that every computer must apply alike have no model in any phase:
  latest review and only-member (ROLES 3448-3458), first files (ROLES
  2388-2391), the plan stream (ROLES 3703) and file landing by the governance
  key (ROLES 4055-4056).
- R3 computes each member's latest review in views.rs (ROLES 1746-1749) and R4
  adds Verifier::latest_review in fold.rs (ROLES 2172-2173). The plan does not
  say the view calls the fold's function, so one rule may get two
  implementations.
- R8, R9, R10 and the door wait on the shape of the takeover record (ROLES
  654, 3545-3547; MASTER 227, 316-317), which belongs to work the master
  places after v2 and whose design still has eight unapplied fixes (MASTER
  198, 297-305). Slots 14 to 16 cannot start until that is decided. This is
  the R8 row's claim and the text confirms it.
- docs/shared-file-tree-plan.md 498 still describes workspace integrate and is
  in no R9 rewrite list (ROLES-D 483).

## Cuts, and the comparison with the count (sections 3 and 4)

### C1 (confirmed)

The optional automatic-plan setting can be cut. Plan recording can be derived
whenever a formation names no decider, with no separate `documents` part.

**Where the review is imprecise.** 'The same completion rule as files' is not
exact. A tree can carry its own rule through `workspace init --completion`
(roles:3820-3821, 4021); the plan is judged by the goal's rule
(rules.rs:38-42). The review also does not say that the cut changes `open` and
`pipeline`: there an author's own declaration makes a revision count, so any
member replaces the plan alone (roles:3684-3686). R9 already accepts that for
files (roles:4006-4008).

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 8, Changes, lines 3553-3566 and 3629-3633 | Drop the `documents` part, the preset edits and the hash regeneration. `resolve` treats a document scope as agreed whenever the pinned formation's `decisions.selection` is none, and names the governance key as the recorder. | phase edit |
| roles-and-permissions-plan.md | Phase 8, Risks and notes, lines 3681-3693 | Replace 'the host binds rules without `documents`' with 'the host binds rules that name a decider'. Delete the paragraph that leaves the presets to the owner. | sentence |
| roles-and-permissions-plan-details.md | lines 454-461 | Remove the regenerated `documents` files, cases and guide rows. | sentence |
| research/joinable-farms-rewrite-contract-2026-10-05.md | lines 404-405, 414, 1142-1147, 1675-1677 | `public` carries no `documents` part. It gets a plan because it names no decider. | sentence |
| master-plan.md | Assumed until the owner objects, lines 157-158 | Say 'by the goal's rule' in place of 'by the same rule as the shared files', because a tree can carry its own rule. | sentence |

### C2 (partly)

R7 and R10 qualify the same workflow twice with real agents and people. The
final effort can be merged, the models moved earlier, and the journey count
made to include confirmations.

**Where the review is imprecise.** The human test is not duplicated by the
plan. It is repeated 'only if a sentence of the explanation changed after it'
(roles:4107-4108), which the review itself notes in its section 2. The review
does not weigh what the early run is for: it is the first observation that
agents pull work by themselves (roles:3519-3523), and a miss is fixed in the
skill and the wait tool, not in R8 or R9.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 7 lines 3388-3401 and 3459-3482; Phase 10 lines 4070-4071 and 4105-4108 | Record the swarm run and the comprehension test once, in Phase 10. Phase 7 keeps the recipes, the journey count and an unrecorded smoke run. | phase edit |
| roles-and-permissions-plan.md | Phase 7 lines 3402-3439, moved to Phase 4 | Write the role model with the phase that builds roles. | phase edit |
| roles-and-permissions-plan.md | Phase 7 table, lines 3382-3386 | Add a column for confirmations and for the second run that `--confirm` needs outside a terminal. | sentence |

### C3 (confirmed)

E3, idle goals dialing less often, can be deferred. Deferral also avoids an up
to one hour delay and a flickering page label.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md | Build order row 12 (line 225) and assumption at lines 181-182 | Move E3 out of the sixteen phases to after v2 and drop the assumption. | sentence |
| host-safety-and-ending-plan.md | E3 heading, line 2964, and roles plan line 653 | Mark E3 deferred. Phase 7 then depends on E1 and E2 only. | sentence |

### C4 (partly)

Phases 8 to 10 need not wait for the takeover record's shape, and the event
format should be frozen once at the real release.

**Where the review is imprecise.** There is no 'intermediate format-freeze
ceremony' to cut. The frozen-vector test exists today
(crates/locust-proto/src/vectors.rs:149) and fails on any signed change, so
every phase that changes signed bytes must regenerate its constants. 'Freeze
actual release bytes once' can only mean correcting the sentence in the
master.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md | lines 227 and 316-317 | Remove 'and the takeover record's shape decided' and the sentence that R8, R9, R10 and the door wait on it. | sentence |
| roles-and-permissions-plan.md | lines 267-270, 282-285, 654, 3545-3547, 3723-3725 | Replace the wait with: the recording key is read through one accessor, which host replacement changes later. | sentence |
| master-plan.md | Versions, lines 238-242 | Replace 'E1's end record is the last change of the event format' with a statement of where the format is fixed, and give J1's signed change a place in the ladder. | sentence |

### C5 (partly)

Disconnecting the agent a goal was started with strands the goal's first files
and a public page's consent. The founding agent's special cases should be
replaced.

**Where the review is imprecise.** Transport identity is not stranded by a
revoke. The agent's admission stays and its endpoint is the computer's; that
is the reason the agent cannot be removed, not a consequence of disconnecting
it (host:1046-1058). A signed host endpoint belongs with replacing a host, as
the plan says. The review also calls this a redesign without noting that the
key is kept on revoke. A forked log of that agent is not fixed by reconnecting
and stays until host replacement (host:1035-1042).

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 2, command table row `agent revoke` (line 1109) and the two tiers (lines 1150-1170) | Add a person's command that connects a disconnected agent again under the same name and key with a new credential. `agent revoke` then has an undo and can move to the commands that apply at once. | phase edit |
| host-safety-and-ending-plan.md | Intended behavior lines 52-55; question 9, lines 3349-3356; E2's `agent revoke` lines | Replace 'has no undo' and the two stranding sentences with the reconnect line. Withdraw question 9. | sentence |

### C6 (partly)

The first-files exemption renews in every epoch that starts empty. It should
be once per goal, derived in replay.

**Where the review is imprecise.** It is listed as a cut but adds a rule.
'Resetting files does not unexpectedly bypass the normal rule' describes a raw
API path that the host can already use to set any rule. The review does not
mention that the plan considered and rejected this (roles:4129-4137).

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md | Assumed until the owner objects, after line 167 | Add: first files means once per start of a tree; a tree starts again empty only through the raw API. | sentence |

### C7 (confirmed)

Confirmations are bound to the whole governance head, so an unrelated
admission voids a yes. Bind material facts instead, and merge the related
sharing reviews into one plan.

**Where the review is imprecise.** Nothing material. Two limits on the claim:
plan and file recordings and stage steps are not governance kinds and do not
move the head (chain.rs:63-65), so a busy private swarm does not trigger it;
and the review does not say that the daemon's own compare on `goal end` has
the same problem (see also_found).

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 2, lines 1089-1092, and companion line 129 | Drop `governance_head` from the fields every `review` holds. Each command binds what its plan shows and what its act changes. Reword the test to change a field the plan shows. | phase edit |
| research/joinable-farms-rewrite-contract-2026-10-05.md | J1, J2 and J4 in the revision | Apply findings 7 and 8 of the public review: define J4's `review` fields, and give the host one publish and open plan that names a rule change when one is needed. | phase edit |

### C8 (confirmed)

The first public surface can be trimmed: aliases, QR codes and extra roster
figures deferred; the name-uniqueness refusal, the refusal of `goal join` for
a door address, the 30 day door cap and mandatory ask mode for listed farms
dropped.

**Where the review is imprecise.** Nothing material. The contract is marked
'revise before use' (contract:18-26), so these are inputs to its rewrite and
not edits to a ready phase.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| research/joinable-farms-rewrite-contract-2026-10-05.md | J1 lines 200-201, 238-241; J3 lines 509-511, 519-520; J4 lines 625-627; J5 lines 735-740, 761-762, 771-772, 782-786; decisions 22 and 25 | Remove the alias routes and short code, `NameTaken`, the descriptor refusal in `goal join`, the 30 day cap, the listed ask rule and the extra roster figures from the first door. | phase edit |

### C9 (partly)

Vocabulary cleanup should be scoped to current product text. Repository-wide
string bans should not force rewriting research or companion descriptions of
removed behavior.

**Where the review is imprecise.** The lines it cites for Phase 6
(roles:2991-3009) are the goal sentence, 'every script and recipe in the
repository'. The binding gate is the exit criteria, which are already scoped.
I found no phase whose functional landing is delayed by a word ban, so 'no
delayed functional landing merely to make a historical word vanish' has no
instance in the plans.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 9, Exit criteria, lines 3964-3966 | Scope the grep to `crates`, `sites`, `skills`, `scripts`, `docs/guide` and `docs/reference`, or add 'and its companion' to the exemption. | sentence |

### C10 (partly)

The first door should follow a usable core milestone inside v2, with its own
acceptance boundary, and takeover and removed-member notices deferred.

**Where the review is imprecise.** It presents as a reordering what the master
already orders. It mentions the format consequence only in its section 5, not
in the C10 row.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md | Build order lines 208-209, Afterwards lines 233-236, Versions lines 238-242 | Say whether the sixteen phases are released by themselves. If yes, give the door's signed change its own number and say that core goals end at it. If no, say the format is fixed at the door. | sentence |

### A0 signing contract (confirmed)

The review agrees with one written contract for every unattended signing and
adds that a retry gives the same bytes only when predecessor, anchor and
evidence are unchanged, and that automatic effects are signed with time zero.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| research/v2-complexity-count-2026-10-06.md | lines 322-326, the retry identity bullet | State that R8 and R9 sign with time zero like stage steps, and that a retry is the same record only while predecessor, anchor and pinned evidence are unchanged. | sentence |

### A1 defer R8 (confirmed)

The plan author proposes deferring R8. The review disagrees: deferral leaves
the default formation with no current plan, R9 still needs R8's recorder, and
the public scope uses R8. Cut only the optional setting.

**Where the review is imprecise.** 'Removes a visible useful outcome' is
relative to the plan, not to today: nothing a person has today is removed. The
review's table row does not mention the cost of keeping R8 that its own
section 2 names: R8 says 'The formal model does not cover document streams'
(roles:3702-3703), against the master's assumption that each piece is modelled
first (master:183).

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| research/v2-complexity-count-2026-10-06.md | cut table line 343 and condition lines 353-357 | Withdraw 'Defer R8'. Replace it with the cut of the `documents` part (C1). | sentence |
| roles-and-permissions-plan.md | Phase 8, lines 3702-3703 | Add one model case for a document stream, or state in the master that R8 is covered by tests only. | sentence |

### A2 defer J6 (partly)

The plan author proposes deferring J6, the newcomer brief. The review partly
agrees: defer the new brief type and fetch scheduler, but not useful-work
readiness or historical task provenance.

**Where the review is imprecise.** 'Useful-work readiness' is listed as
something to keep building. It is satisfied by deferring J6's wait
instruction. The review does not say which phase should own provenance once J6
is deferred.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| research/joinable-farms-rewrite-contract-2026-10-05.md | J6, lines 809-870 | Defer the phase whole. Move the rule 'who opened a task and how its author came in, read at the task record's own anchor' into the phase that builds the check for answer 18. | phase edit |
| research/joinable-farms-rewrite-contract-2026-10-05.md | J8, acceptance cases | Add a newcomer who starts a ready task while unrelated objects are still missing. | sentence |

### A3 defer E3 (confirmed)

Both agree to defer E3. The review adds that deferral also avoids the longer
recovery delay and the flickering label.

### A4 defer short code (confirmed)

Both agree to defer the 8 letter short code. The review adds that QR can go
too and that neither is an owner decision, while the Join fold is.

### A5 cut R3's second rules check (partly)

The review agrees in direction but says today's trial signs first, so a check
that never signs a rejected record needs a new unsigned boundary.

**Where the review is imprecise.** It accepts the count's 'never signed'
wording and concludes a new unsigned boundary is required. Relaxing the
wording to 'never stored, marked or sent' is met by today's order and by G1 as
written.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| research/v2-complexity-count-2026-10-06.md | lines 358-363 | Reword the second condition: a rejected record is never stored, marked or sent. Add that the handler must run the trial before it answers, so the rules refusal still comes before the level refusal. | sentence |
| roles-and-permissions-plan.md | Phase 3, lines 1612-1620 and 1642-1644 | If the cut is taken: `allowed` runs the fold's own trial on the candidate and reads a structured reason. Keep the level, content and private path checks outside it. | phase edit |

### A6 cut E2's leave line (confirmed)

Both agree the host has nothing to do when a member leaves. The review adds
that a truthful pending status is still needed and that automatic removal must
be committed and retried safely.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | E2, lines 2805-2822, in its rewrite | State the leaver's status line until removal, and what the host's computer does with a held leave while it is catching up, halted or ended, with the retry identity of the removal and its new key. | phase edit |

### A7 cut the two Check fields (confirmed)

Both agree to cut `Check.count` and `Check.exclude_author` for this release.
The review adds that 'nothing' is true only for the built-in formations.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 4, lines 2077-2079, 2131-2142, 2309-2313, 2668-2669 | Remove the two fields on `Check` and the sentences and editor changes that follow them. | phase edit |

### P1 clean intermediate commits (partly)

The count misses that the R1 to R6 interval with broken scripts conflicts with
CI, not only with later qualification.

**Where the review is imprecise.** It quotes the rule and not the risk. Its
section 2 rates R1 'No as scoped' on this ground alone. The owner's stance is
not recorded anywhere in the repository today, so the reviewer could not have
seen it; the plans should state it.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 1, Risks and notes, lines 1013-1016 | Name the CI steps that fail from Phase 1 to Phase 6 and say this is accepted for unreleased work. | sentence |
| master-plan.md | Assumed until the owner objects | Add: between phases the guide's recipes and the scripts may fail; every phase passes the cargo checks; Phase 6 lands them clean. | sentence |

### P2 mark failure semantics (confirmed)

The count keeps the marks design without resolving unsynced or torn mark
updates, and its durability contract must cover that.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | G1, lines 1328-1332 and residual 7 at lines 1784-1786 | Either sync a changed mark before the commit returns, or write a durable 'mark uncertain' state that makes the next start treat the key as behind. Measure the cost of the sync first. | phase edit |
| research/v2-complexity-count-2026-10-06.md | lines 325-326 | Add to the durable-writes bullet: whether the mark is on disk before the record can reach a peer. | sentence |

### P3 service false release (confirmed)

A false release through the farm service is a separate problem from the
abandoned-member stall. The count names only the stall.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| host-safety-and-ending-plan.md | G1, lines 1448-1462 | `Current` no longer deletes `UNHEARD`. The service answer is used only as evidence that a copy is behind. | phase edit |
| research/v2-complexity-count-2026-10-06.md | gap 1, lines 247-251 | Add the false release beside the stall. | sentence |

### P4 local contradictions (confirmed)

The role undo that does not undo, the allowance that comes back, the renewed
first-files exemption and the tree rule that cannot be changed are concrete
costs the command total does not capture.

**Where the review is imprecise.** The four are not equal. The first-files
renewal is reachable only through the raw API and gives the host nothing new
(roles:2674-2682; see C6). The tree rule is the one an ordinary host will
meet: they change the rules and file changes keep the old rule for good.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 4, lines 2329-2335 | When a take empties a role, print the undo as a restore of the exact earlier holders, or do not call the line an undo. | sentence |
| roles-and-permissions-plan.md | Phase 3, lines 1601-1604 and 1973-1976; master line 154 | Say the allowance covers the whole round, including work resumed after a rejection, or clear it when this computer sees the task finished. | sentence |
| roles-and-permissions-plan.md | Phase 9, lines 4017-4021, and Phase 2's `rules bind` row at line 1106 | A confirmed `rules bind` on a goal with shared files also starts an epoch that carries the current files under the new rule, and its plan says so. | phase edit |

### P5 stale claims (confirmed)

Three claims have source answers now: records beyond a fork are exchanged, a
suspended ended page keeps its deletion deadline, and R8 and R9 name the
governance key. R4's member-signed acceptance until R9 is deliberate.

**Where the review is imprecise.** Two of the three stale claims are the host
companion's, not the count's. The point is placed under 'what the count
misses'.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| master-plan.md | line 316 | Delete 'R4, R8 and R9 still describe the host's agent as the signer.' | sentence |
| host-safety-and-ending-plan-details.md | lines 296 and 298 | Correct both: a suspension keeps the end time; records beyond a fork reach surviving members by ordinary sync. | sentence |
| host-safety-and-ending-plan.md | Known gap 4, lines 42-44, and count lines 297-299 | Narrow to: a page whose end is held behind a missing record reads open; a page whose end a fork cut is suspended; in both the 30 days never start. | sentence |

### P6 no extra permission ceremony (partly)

Automatic recording of an approved change must not gain an extra yes. Applying
files to a folder and running code are separate local acts that need their own
stated rules.

**Where the review is imprecise.** Its premise is out of date. It reads answer
18 as 'requires a yes before a door-authored task'. The owner restated 18: a
trusted agent approves and 'No person is asked' (master:107-112). The review
flags this itself in its opening note.

| Document | Where | Change | Size |
| --- | --- | --- | --- |
| roles-and-permissions-plan.md | Phase 3, the `level_needed` rule at lines 1645-1648 | State the level that lets an agent apply recorded files to a folder its person connected. Under answer 26 it must not wait for a person. | sentence |

Also found in this area:

- CI hides two checks for six phases. A GitHub job stops at its first failing
  step. The recipe step (.github/workflows/ci.yml:43-44) fails from R1 to R6
  (roles:1013-1016), so the helper tests and the documentation index and link
  check after it (ci.yml:45-48) do not run in CI at all in that interval,
  while every phase edits documents. Moving the recipe step last in R1 keeps
  both running. This is the real risk behind the review's point about clean
  commits.
- `goal end` can lose a race with unattended signing. It sends the plan's
  `governance_head` as `expected` (host:2386-2388; roles:1111). An admission
  on an open invitation, a door admission or a removal after a leave moves
  that head with nobody present (crates/locust-core/src/goal/chain.rs:63-65,
  218). On a busy public goal the one command that must work can be refused
  repeatedly. C7 covers the plan id, not this compare in the daemon. Smallest
  change: `goal.end` takes no expected head. A sentence in E1.
- The master is stale against the takeover design written after the review. It
  still lists 'the takeover record's shape decided' as a need of R8
  (master:227) and says R8, R9, R10 and the door wait on it (master:316-317).
  The design note says the shape exists and that Phases 8 and 9 need three
  sentences and one accessor
  (research/replacing-a-host-design-2026-10-06.md:36-43, 2171-2174).
- C3 and C10 interact. E3 was to slow a removed computer's retries
  (host:3088-3095) and J7 was to tell it that it was removed (contract:884).
  If both are deferred, J0's failure backoff to 15 minutes (contract:143-144,
  161-167) is the only thing that quiets a removed computer. It must stay in
  the first door.
- The owner's stance on broken intermediate commits is recorded only in a
  session memory file, not in the repository (a search of docs, research and
  AGENTS.md finds nothing). AGENTS.md still reads as if every commit passes.
  An independent reviewer will keep raising it until one sentence in the
  master or AGENTS.md states it.
- A revoke keeps the agent's signing seed and only sets a flag
  (crates/locust-core/src/node/identity.rs:74-84, 165-167;
  crates/locust-core/src/node/requests/daemon.rs:133-152). 'Disconnecting an
  agent has no undo' (host:52) is therefore a product choice and not a fact
  about the data. Neither the plans nor the review say so.
- G1 writes marks inside the store commit, after the in-memory trial
  (host:1308-1311; crates/locust-core/src/node/commit.rs:208-224). So a record
  the trial rejects is never marked. Neither the count's condition
  (count:361-362) nor the review notes this, and it is what makes the cheaper
  form of the rules-check cut safe.
- The journey budget in Phase 7 compares 7 commands today with a budget of 2
  (roles:3382-3386), but both budgeted commands ask for a yes
  (roles:1150-1153). The comparison a person feels is 7 lines against 4.

## Recovering under a new key

One reader scoped this option: when the host's computer finds it was started
from a copy and cannot prove what it signed since, it never signs with the
old key again. It goes on under a new key from the last record it holds, and
whatever the old key signed later is dropped on every computer when it shows
up. The mechanism is the change-of-host record from
[the backup-host design](replacing-a-host-design-2026-10-06.md), used by the
host on itself. This is a scoping study, not a design.

**Verdict.** Sound in principle for the one failure v2 cannot recover from
(two records at one position of the host key's log), but only on four
conditions, and not as the host-change design stands. (1) The start on a copy
must be detected; nothing here helps when it is not. (2) The new key must be
fresh for each detected restore. The design computes it from the agent's seed,
the goal and the base (design note 720-742), so one copy restored twice makes
the same key and the same record, and the second start then signs position 1
of the new log a second time. That is the design's own break 7 (374-402) one
level up, and its only defence is another 'heard from' rule, which is the
guess S2 rejects. (3) With fresh keys, two restores from copies that do not
contain each other give two live change records on one term. The earlier one's
key is gone, so the design's withdrawal (signed by the change's own key,
744-772) can never settle it. A new rule is needed; the one I can make work is
that the running computer withdraws its own change and continues from the
other's tip, once. That rule is mine and is not modelled. (4) No unattended
signature may fail inside start or landing (break 6). With these, every wait
becomes an optimisation: a wrong guess drops records and never forks. That is
the 'key-replacement protocol, with its safety and availability trade stated'
that review S2 asks for (review 96-99). It does not cover agents' own keys, a
copy older than the goal, a rollback that goes unnoticed, or two running
copies, where it turns a permanent halt into a silent change of which copy
hosts. Signed eagerly at start it also throws away records that G1 with kept
marks would have got back from members. Worth a full design round, scoped to
'a host continuing its own history' and nothing else of the backup host: it
removes the marks, the release rules and the continue command, answers S1 and
S2 for the host key, fits answer 26, closes Known gap 3, and gives v2 its only
way out of a forked host log. The price is that more than half of draft phase
B1, the chain builder rewrite included, moves before the first door, and four
of the eight serious breaks (3, 5, 6, 7) sit inside the part moved. Everything
about K1, G1, G2, E1 and B1 is plan text; I verified today's code by reading
it at 9342514 and ran nothing.

**How it would work.**

Files: plan docs/host-safety-and-ending-plan.md (G1 at 1178-1940, G2 at
1942-2164), review research/v2-plan-review-2026-10-06.md (S1 38-70, S2
72-109), design note research/replacing-a-host-design-2026-10-06.md, master
docs/master-plan.md.

1. Notice the copy. At `Node::open` the daemon asks one question from G1's
   first table (plan 1238-1246): is the database the file this installation
   last used. Known gap and unknown age are no longer told apart for the host
   key, so the marks are not needed for it.

2. Retire and replace. For each goal this computer hosts, in one commit:
   record that the key it held signs no more records, draw a fresh key, store
   its seed as K1 stores record `K` (plan 700-707), and sign one change record
   at position 0 of the new key's log. The base is the last record of the old
   key's log whose `prev` links are all held. The record names the same host's
   agent, which the design calls a continuation and already authorizes in
   replay from any base (design note 1184-1189); only the takeover command
   limits it to a forked log (1376-1378). The endorsement must come from a
   secret every copy of the data holds: the goal's first key or the host's
   agent's seed.

3. Replay on every computer. The chain is the first key's log up to the base
   read by `prev`, then the change record, then the new key's log. Whatever
   the old key signed outside the base's ancestry is set aside whenever it
   arrives; a member's record anchored there is excluded and that member's
   next record counts (design note 1204-1209, 1304-1316). `Goal::next` gives
   the old key no place. Today's code does none of this: `Chain::build`
   follows one key's usable prefix and reports a fork as a halt
   (crates/locust-core/src/goal/chain.rs:52-61, 222-234), and `screen` keeps
   only the first key's records and those of keys it admitted
   (crates/locust-core/src/goal/screen.rs:36-40). Verified by reading.

4. Go on as host at once. The daemon admits, removes and records under the new
   key. When a removal the old key signed arrives as set aside, it signs the
   removal again (the design's B2 rule, 774-800). Tickets issued under the old
   key stop working.

5. Tell the dropped. A computer whose admission was set aside is refused by
   everyone and must be told why. That needs the 'asked why' half of the
   design's notice (830-856, scenario S8).

6. Meeting another change it did not sign. If the daemon finds a second live
   change record on the term it continued from, it withdraws its own and signs
   a new change on top of the other's held tip, once per start on a copy. If
   it finds a change that continues its own term, it is no longer the host,
   says so, and does nothing by itself. My rule, not the design's; without the
   'once' and the 'does nothing', two running copies answer each other until
   the 4,096-author frontier limit stops all sync
   (crates/locust-proto/src/limits.rs:35, sync.rs:404).

7. Sign late. Safety does not depend on when step 2 runs. Signing it at the
   first needed host signature, or after one round of dials to the computers
   the copy lists has ended either way, lets the old key's later records
   arrive first and be kept. Today `Node::open` drives automatic steps before
   any peer is heard (crates/locust-core/src/node/mod.rs:180-188), which would
   force the change record at once.

8. Agents' keys are untouched by all of this and keep a reduced form of G1's
   hold.

**What a person sees.**

As host, today's plan. Data folder put back with the marks kept: status says
'Catching up', lists the missing records and the computers not yet heard from,
and prints the continue command; the host's own commands and its agents are
refused with `read_only`; a joiner is told the host is catching up; it ends by
itself when a member's computer returns the records, and nothing is lost
(mockups G-1 and G-4, plan 225-261, 318-349). Whole computer restored or
moved: every goal 'may be an old copy'; a shared goal waits for every other
computer; a goal run alone sits under 'Waiting for you' until `goal continue`,
which shows a warning and asks (G-2, G-3, plan 263-316). A copy older than the
first member waits until a member's computer calls (G-5).

As host, under the option. No waiting state and no command. The goal works as
soon as the daemon is up. Status carries one block per goal until the next
ordinary start, saying that this computer went on from a copy and what that
dropped: who was admitted since and is out, who was removed since and was
removed again, which recordings were made again and which change has to be
rebuilt, and how many invitations stopped working. The line about levels,
folders and disconnected agents stays. A record of the old key that was
dropped prints as `host (dropped)`. If another copy of the data took the seat,
this computer says it no longer hosts and prints one command to take the goal
back. These texts are mine, modelled on the design note's B-6 and B-11; none
is in a plan.

As member, today's plan. They see the host's computer catching up, a join that
asks again by itself, and afterwards nothing missing.

As member, under the option. Nothing waits. They may see that some of their
agent's recent records no longer count because the host's computer was
restored from an older copy; the agent can post again at once. A stage task
they were working on may open again as a new round. A file change built on a
dropped recording is reported behind and must be rebuilt. Someone removed last
week may be a member again for a while. A member admitted after the copy is no
longer a member and needs a new ticket; with the notice built their status
says so, and without it every computer refuses theirs with no reason given and
a new ticket answers 'joined' from their own stale copy
(invitations.rs:271-281).

**What is lost.**

Everything the old key signed after the base, on every computer, whenever it
arrives.

- Admissions: those members are out. Their computers hold the history and keys
  they had. They join again with a new ticket, which is a person's act on both
  sides. The design has no rule that admits them again by itself.
- Removals: undone until signed again. Records the removed member signed after
  the original cutoff count again, because the repeat computes a later cutoff
  (design note 782-785, 796-800).
- Rule changes, role changes, publication changes, task revisions, a files
  reset, and the end of the goal: gone; the person redoes them.
- What the host's computer recorded by itself: a plan text is recorded again;
  of file changes the first lands again and one built on it must be rebuilt
  and approved again; a stage task opened after the base is opened again as a
  new round and work on the old round is lost (design note 816-820). Under G1
  an identical replay of a stage step was harmless; under a new key it is a
  different record.
- Members' work anchored at a dropped members-or-rules record: it stops
  counting. It stays on disk, and its author can sign again at once.
- Text sealed under a key that a dropped removal opened stays readable only
  where that key is held.
- Pending invitations, and every ticket of the retired key.
- One author in every frontier and one key epoch per restore, for the life of
  the goal.

Against the plan today the important loss is this: with the marks kept and a
member online, G1 gets every record back and loses nothing. The option signed
at start drops the same records even though a member could have returned them
a second later. Signing late removes most of that.

People who must act: anyone admitted after the copy, and the host who invites
them; the host for any rule change, role change or end made after the copy;
the host once more if a removal made after the copy reached nobody.

**Size.**

Added: about 2,100 to 3,300 lines of production Rust and 3,000 to 4,500 of
tests, plus two models of 600 to 900 lines together. By part: signed formats
and frames 250-400; the chain walk, kept log, screen, standings and
`Goal::next` 700-1,000; the readers in fold, rules, flow and projection
100-250; the start-on-a-copy branch, stored seeds, ticket issuer and keys by
opening record 350-550; the unattended acts and their one guard 200-350; the
notice with its way back 300-450; views and texts 200-300.

Removed from the plan: about 600 to 900 of the 1,100 to 1,500 production lines
of G1 and G2 (the marks and their store plumbing, the host-key holds and
release rules, the call-back, the continue command and its texts) and 800 to
1,100 of their roughly 1,600 test lines.

Net against the plan as it stands: +1,200 to +2,700 production lines and
+1,900 to +3,700 test lines, which is 3 to 6 percent on top of v2's 33,200 to
57,500 added lines (master 250-252). The backup-host work that follows v2
shrinks by about what was moved, so across both the total falls by roughly the
G1 and G2 lines removed.

The count understates the cost in one way: the chain builder rewrite, three
changed sync frames and a new exchange move ahead of the first public door,
where the review's C4 and C10 wanted takeover work kept out (review 384, 390),
and the models must exist before chain.rs is touched.

Basis: the plan writers' own judgements (G1 800-1,100 and 1,200, plan
1939-1940; G2 300-400 and 400, plan 2163-2164; B1 3,600-5,000 and 5,500-7,000,
design note 1635-1636), the sizes of today's files (chain.rs 538 lines,
history.rs 211, screen.rs 46, peers.rs 400, replica.rs 428), and my split of
B1 into what a continuation needs. All judgement; nothing was built or
measured.

**What it replaces in the restore guard, and what must stay.**

- The marks as proof of what was signed: marks.rs, `Mark`, `MarkWrite`,
  `Marks`, `Commit.marks`, `Store::marks`, `MemMarks`, the `shared` flag, the
  25 call sites of `SqliteStore::open` and doctor's marks check (plan
  1296-1348; companion 104-105, 147). Review S1 goes with them for the host
  key, because no durability claim rests on a mark any more.
- The split of the first table into kept and lost marks (plan 1241-1246). One
  question is left: is this database the file last used.
- Every row of the second table that releases the host key: behind with the
  marked record back, 'never shared', unheard until every other computer was
  heard from (plan 1253-1265), and `guard_attest(Current)` (1448-1462). Review
  S2 and Known gap 3 (plan 38-41) go with them. G1's residual 4, the unknown
  copy whose listed computers all lack the later records (1770-1775), becomes
  an ordinary dropped admission instead of a fork.
- The rule that every agent on the host's computer is held while the host key
  is (plan 1235-1236, 1406-1409). The chain every computer computes is the
  copy's view, so the host's agents are not working on a view others disagree
  with. The cost is in the breaks: a copy older than the second admission
  makes the host's agent the only member for everyone.
- `goal continue`: the request and response (plan 1385-1387), the command, its
  plan and mockup G-3 (2016-2033, 289-316), `--all`, the 'Waiting for you'
  entry (2002-2008), the rule that a held host command shows no plan
  (2035-2046), and with it the wait of answer 13. It can go entirely only if
  agents' keys never need an override.
- The host's 'catching up' state: `Halt::SignerRecovery` for the host key,
  `GuardView.by_host`, the host parts of mockups G-1, G-2 and G-5,
  `Node::admission_hold`, `Refusal::CatchingUp` and the joiner's sentence
  (plan 1349-1353, 1527-1535, 2157-2160).
- The call-back of unknown callers, `note_caller` and `Guard::callers` (plan
  1536-1540, 1551-1555). The notice does this job.
- `RestoreGuard.tla` as specified and the simulator's restriction of lost-mark
  restores to runs with no later admission (plan 3240-3271; companion 175),
  which S2 objects to (review 104-109). A replay model of the change record
  and a node model with copy and restore actions take their place.
- Residuals 3, 5, 7 and 8 for the host key (plan 1769-1789): no person can
  continue too early, and no member can hold the host by withholding records.
- What must stay: noticing a start on a copy (`FileId`, `Identity.file`, the
  classification in `Node::open`, plan 1298-1299, 1472-1484) and the
  file-identity note (1748-1754), which now matters in both directions. A
  missed restore forks as today. A false alarm at every start adds one term
  per start.
- What must stay: something outside the folder if an overwrite in place is to
  be caught. G1 caught it only through the marks (plan 1243, 1248). One number
  written inside and beside the folder at each start, synced once per start,
  would do; that is my suggestion and is in no plan.
- What must stay: holds on agents' keys (`Behind`, `Unheard`, `Admitted`),
  `Halt::SignerConflict`, `Why::ThisComputer` for an agent, the heard and
  `reconciled` plumbing those rules read, the restored line about local
  settings, revoking pending invitations (or refusing an earlier key's
  tickets), the Backups section, and E3's `catching_up` hook.
- What changes without being replaced: G2's line for a copy older than goals
  reads the marks (plan 2011-2015). Without marks it needs a small list of
  goal ids beside the folder, or it goes.

**What it needs from the backup-host design.**

- `Body::HostChanged` in its continuation form only (the named member is the
  term's own host's agent), its `Header::check` rule and the domain
  `HOST_CHANGE_SIGNATURE` (design note 1223-1236). Not `BackupSet`, not
  `HostYielded`, and no revocation by descent: no record of the old key can
  revoke a continuation, because a removal of the host's agent never takes
  effect.
- `Body::TakeoverWithdrawn`, `Body::is_marker` and `Goal::marker_place`
  (1228-1230, 1332-1334), but only so that two restores settle by themselves.
  The rule for when the daemon signs one is new.
- The definitions: term, change record, base, kept log, path, placed,
  authorized (the host's-agent half), withdrawn, live, in force, disputed, set
  aside, undecided, marker (1173-1217).
- The rewrite of `Chain::build` as a walk over terms: steps 1, 2, 4, 5 and 6
  of the design's list and the withdrawn half of step 3 (1274-1297); `step`;
  `Chain::fold_path`; `Snapshot.governance`, `Snapshot.host`, `Snapshot.key`;
  `State.terms`. This is the largest single piece and cannot be cut down.
- `History::kept`, the walk by `prev` (1267-1273). A later fork of the old key
  at or below the base would otherwise shrink the usable prefix
  (history.rs:78-86) and move the chain. `History::descends` is not needed.
  `History::tips` and `Goal::takeover_bases` are needed only if a fork that
  still happens is to be repaired.
- The `authorize_base` rules (1304-1316): a host record with no standing is
  set aside; a host step is judged by the term of its anchor and must be in
  that key's kept log; an anchor that is set aside excludes; an ancestor
  anchored at a set-aside record is skipped; an undecided anchor keeps a
  record pending.
- `Exclusion::SetAside`, `Withdrawn`, `Unplaced`, a name for 'not the host's
  agent', and `Halt::HostDispute` (1258-1262).
- `Goal::next` answering nothing for a host key that is not the last term's,
  or under a dispute (1330-1332).
- `screen` keeping known host keys, the keys they admitted, and a change
  record by the exact test (1339-1345).
- K1's two replay rules reading the term (1301-1303), and the end rule at E1's
  plug: a change whose kept log holds an end never counts, and `cut_end` reads
  the last term only (plan 3184-3203; design note S9).
- The readers in fold.rs, rules.rs, goal/flow.rs and projection.rs going
  through the snapshot at the anchor; one stand-in author for all host keys in
  the decision index; `desired_selections` ignoring set-aside decisions; the
  order of `current_round` (1317-1329). R8 and R9 land after this and would be
  written against it.
- Content keys stored and asked for by the record that opened them:
  `Space::Key` by opening record, `KeyRequest { opened_by }`, `Key {
  opened_by, key }`, `offer_key` taking a change record as proof (1250-1251,
  1349-1356). Needed as soon as a removal is set aside and signed again, since
  two keys then share one epoch number; today there is one key per number
  (replica.rs:216-218, 243-268).
- `Invitation.issuer` and the checks that read it (1245-1249, 1415-1420).
  `plan_join` already refuses a ticket whose key is not the goal's current one
  (peers.rs:360), so old tickets die without new code; the field is for
  tickets issued after the change.
- `Node::key_for` and `Node::hosts` for a later term, with a stored seed where
  the design computes one, and the local record `T` (1346-1348, 1400-1404).
  `historical_endpoints` reading every known host key's admissions (1421).
- The notice, 'asked why' half only: `SyncMessage::Notice`, `NoticeAnswer`,
  `Host::evidence`, `receive_notice`, `notice_answered` (1252-1257,
  1424-1448). It needs the way back that fix 3 names, so that a refused
  computer can hand over the old key's records it holds. The 'told' half is
  not needed: the earlier host is the same computer.
- From B2, not B1: the repeat of a set-aside removal, with its own cutoff
  (774-800, S6), and the status lists for what was dropped and repeated.
- Models: `HostChange.tla` cut to continuations and withdrawals, and
  `HostChangeNodes.tla` with Copy, RestoreStore and RestoreAll and without
  Override (2077-2150). None exists yet.
- Not needed: `BackupSet` and naming, `HostYielded`, revocation,
  `goal.takeover.ask`, `.plan` and `.withdraw` as commands, `goal give` and
  `goal keep`, the host's view of its backup, clearing the publication and
  passing roles (a continuation keeps both), `Refusal::Replaced`, and protocol
  8.
- Serious breaks inside this part: 3 (a dispute in an early term strands
  members who worked in a later one, 261-292), which appears as soon as a
  third restore meets two earlier terms; 5 (the key and `hosts()` depend on an
  active host's agent, 318-339), unless the seed is stored and the endorsement
  does not need a connected agent; 6 (unattended signatures failing inside
  landing, 341-372); 7 (the hold on a new key after a restore, 374-402), which
  is this option's own problem one level up and is what forces fresh keys.
- Serious breaks outside it: 1 and 2 (both need a named backup and its
  removal), 4 (`goal keep`), 8 (the takeover plan's id). Break 1's shape
  returns in a milder form: a computer the change dropped can only receive, so
  the host cannot get its own later records back from it without fix 3.
- Minor findings inside it: 9 (a withdrawal that reads what happens to be
  held, 428-456); 10 as honest growth, one author per restore against a limit
  of 4,096; 12f, 12h and 12i (529-537); and three items of 'Not settled': a
  fresh joiner across terms (2522-2525), the publisher under a continuation
  (2551-2553), and connected folders when the files head moves back
  (2559-2561).

**What stays unsolved.**

- An agent's own key after a restore, on a member's computer and on the
  host's. An agent's key is its membership, so there is no new key to continue
  under. A second record at a used position still costs that agent its place
  in the goal (plan 1035-1041), and for the host's agent, which cannot be
  removed, it also costs the roles only it holds and the first files. The hold
  that protects it stays a guess with bounded damage (plan 1257, 1260,
  residual 6).
- A copy older than the goal. It holds neither the goal nor any key of it, so
  there is no base and nothing to endorse with (plan 1022-1024, 1799-1807).
  The host seat is lost until a backup host exists.
- A rollback that is not noticed: a disk image, a virtual machine snapshot, or
  an overwrite in place with nothing kept outside the folder (plan 1764-1769).
  The old key signs at a used position and the goal halts for good, as today,
  unless the continue-from-one-branch part is also built.
- Two running copies. If the copy was noticed as a copy it takes the seat and
  the original's later records are dropped; taking the seat back needs one
  command from the person. If it was not noticed, the fork is today's. Both
  copies also share the endpoint secret (identity.rs:40) and every agent seed.
- A removal that no other computer held is gone and nobody is told. A removal
  others held is undone on every computer until the host's computer receives
  it and signs it again.
- An end signed after the copy is dropped and the goal is alive again, unless
  the design round makes an end outlast a continuation. E1's question 24
  recommends that an end stays an end (plan 3421-3424); the host-change rule
  says an end after the base is void (plan 3198-3200).
- What the copy brings back on the host's own disk: levels, allowed tasks,
  folders, disconnected agents, leaves (plan 1808-1818). Unchanged.
- The public page: the publisher's counter is behind the service's after a
  restore (plan 1850-1863), the door's ticket was issued by the retired key,
  and nobody has checked the publisher under a changed host key (design note
  2551-2553). The public-goals phases are not written.
- Members' connected folders when a recorded file change is set aside and the
  head moves back. Not designed anywhere (design note 1055-1056, 2559-2561).
- How this meets the backup host later: a restored old host's automatic change
  record and a backup's takeover are two live changes on one term between two
  people, and answer 12 has to be read against it.

**Owner decisions it touches.**

- 13 (wait for one command after a whole-computer restore with nobody to ask):
  replaced. Nothing waits. The master says it stays unless the owner says
  otherwise (master 285-288).
- 26 (the swarm never stops to ask a human): met for the host's records. A
  person is still needed in three rare cases: two running copies, a member
  admitted after the copy who needs a new ticket, and an agent whose own key
  was forked.
- 3 (nothing shared decided by clock, timeout, arrival order or identifier
  comparison): the base is whatever this copy holds, signed into one record
  every computer follows, the same reading the takeover base already has.
  Signing late after dials have ended either way is an unattended use of this
  computer's own clock to choose that base; the owner should confirm it falls
  under the narrowed wording (master 46-53).
- 10 (agreement is used only when a host is replaced; a takeover fixes the
  last host record its signers hold and what came after is void): the same
  rule is used for a restore by the host itself, with nobody else agreeing.
- 9 (a key that does nothing else, apart from the working agent, in the data
  folder with no passphrase): there is a new key per restore. If the change is
  endorsed with the host's agent's key, as the design does, that agent's key
  gains a say over members and rules. Endorsing with the goal's first key
  keeps the answer as worded.
- 8 (hostile members are the host's to remove): a restore puts a removed
  member back on every computer until the removal is signed again, and for
  good if no other computer held it.
- 5 (a goal's only member needs no approval): a copy older than the second
  admission makes the host's agent the only member again for everyone.
- 14 and 15 (a leave removes the member by itself; an approved change is
  recorded by itself): both kinds of record are dropped when signed after the
  copy and are signed again by the host's computer; a change built on a
  dropped recording is rebuilt and approved again.
- 22 (the first door comes after the restore guard and the first step of
  ending, before the backup host): the restore guard becomes a different
  thing, and more than half of the backup host's first phase moves before the
  door.
- 21 (the farm service may help notice a restore): still useful as evidence
  that a copy is behind, which now triggers the new key. Its use as proof of
  being current is no longer needed.
- 23 and question 24 of the host plan (an ended goal stays ended): a restore
  from before the end revives the goal unless the round decides otherwise.
- 12 (the host's removal of its backup wins): not touched in v2, but an
  automatic change by a restored old host has to be read against it when the
  backup host is built.
- 2 (no migration): untouched if this lands before anything is released; the
  backup host still takes its own protocol number.
- 1 (the design that asks least of the person wins): the reason to do this at
  all.
- Assumed until the owner objects (master 169-183): that the guard holds the
  host's own agents and refuses the person's commands while catching up; the
  words 'catching up' and `goal continue`; and that each piece is modelled
  before it is built, which here means two models before chain.rs is touched.

**Other ways to recover when history cannot be proved.**

- Sign late. The same mechanism, but the change record is signed when a host
  signature is first needed or after one round of dials to the computers the
  copy lists has ended either way, so the old key's later records usually
  arrive first and are kept. Nothing waits for good and a wrong guess still
  only drops records; this is a refinement I would fold into the option, not a
  rival.
- Proof where there is proof, a new key otherwise. Keep a mark that is synced
  before a record is released (the repair S1 asks for) and go on with the old
  key only when the marked record is held again; use the new key only for a
  copy of unknown age. It loses nothing in the common restore, but keeps the
  marks, a sync per signature and two mechanisms to explain.
- The same key with a recovery marker, the first round's 'Move the key'
  resolver (research/replacing-a-host-2026-10-05.md:560-572). The restored
  computer signs a marker at its tip, replay reads the log as a tree by
  `prev`, and the branch with the deepest marker wins; there is no new key and
  no term. That round's reviewers found that two restores of one copy tie and
  halt, and that a stale copy overturns settled history (same note 613-620).
- Keep both branches. Let the host key's log hold a fork and let the host sign
  one record that names both tips; admissions and removals merge and that
  record chooses between conflicting rules. Nothing is dropped, but
  membership, cutoffs and key epochs would all have to be folded over a graph,
  which neither design round examined; this is my inference and I did not
  build the cases.
- A successor goal. The restored host starts a new goal that names the old one
  and the base, admits the same members and tells them over the fork-proof
  route, and the old goal is left as it is. Replay does not change at all, but
  the work history stays behind under the old identifier and every member
  joins again.
- Wait for the person, with S2's repair. No automatic release for a copy of
  unknown age: either a durable mark proves the log is whole or the person
  runs one command. It is sound and the smallest change, but it is the stop
  that answer 26 rules out, and a public goal with a member who never returns
  waits for a person every time.
- A witness. The farm service, or members, hold the host's latest position and
  tell a restored computer it is behind. That is evidence of being behind and
  never proof of being current (review 96-99), so it can trigger the new key
  sooner but cannot replace it.

**Not settled.**

- What I verified and what I inferred. Read in today's code at 9342514:
  chain.rs, history.rs and screen.rs whole; goal/mod.rs; node/authoring.rs,
  commit.rs, peers.rs, replica.rs, flow.rs; the start loop of node/mod.rs;
  `goal_join` in requests/invitations.rs; the membership test in
  sync/responder.rs; limits.rs. Not read: fold.rs, projection.rs, farm.rs
  beyond two fields, workspace.rs, the driver and initiator, the store.
  Nothing was built or run. Every behaviour after K1, G1, G2, E1 or B1 is
  inferred from plan text.
- The rule 'withdraw your own change and continue from the other's tip, once,
  and never react to a change that continues your own term' is mine. I traced
  it by hand for two and three restores and for two running copies, where it
  ends in a stable halt that needs a person. It is not in the design and not
  modelled, and I may have missed an order of events.
- Whether file identity tells a copy from the original on real systems: APFS,
  ext4, `cp -R`, `rsync -a`, Time Machine, Migration Assistant, snapshots.
  Nobody has measured it (plan 1748-1754; companion 199). The option depends
  on it more than G1 does, because a false alarm now costs a term each time
  and a miss still forks.
- What two daemons with one endpoint secret do to each other on the transport,
  and so which of two running copies members actually reach.
- Whether a fresh joiner, or a member far behind, converges across several
  terms when frames carry one author each and the new key sorts before the old
  one. The design note leaves it untraced (2522-2525).
- Break 3 rests on reading 'set aside' by its letter; the design's own reader
  was unsure (652-654). If the definition already covers a path with a
  disputed change, that break is not inside the part.
- Whether a repeated removal can safely reuse the original cutoff so that
  records signed after the first removal do not count again. The design
  rejected copying the cutoff because it can be anchored at a set-aside record
  (796-798); a cutoff anchored inside the kept chain may be reusable. Not
  worked through.
- Whether the host's computer should also sign again, by itself, an admission
  of its own that was set aside. It would remove most of 'joins again', but
  the design has no such rule, and it interacts with a later removal of the
  same member arriving in another order.
- The public page, the door and the publisher's counter after a restore. The
  public-goals phases are not written, and I read only the two fields in
  farm.rs (lines 23 and 34).
- What a member's daemon does with local state built on records that are later
  set aside: deliveries, claims, sessions, feed entries and connected folders.
  Not read.
- Whether ending by a restore from before the end should stay an end. The two
  plans' assumptions point opposite ways (plan 3198-3200 against 3421-3424).
- Every size is a judgement built on the plan writers' own judgements. The
  split of B1 into what a continuation needs is mine.
- Whether signing late counts as deciding something shared from this
  computer's own clock under answer 3. The design uses a 20-second wait in the
  takeover command and argues it decides nothing shared (2562-2564); here the
  act is unattended.

**Cases worked through.**

| Case | Outcome |
| --- | --- |
| The copy is older than a later admission. The copy holds g0 to g4; afterwards g5 admitted Birch on computer D. The host signs the change at base g4. | The host's log holds: g5 is set aside on every computer and no fork exists. Three things break around it. D is refused as not a member by every computer that holds the change (responder.rs:116) and ordinary sync never gives it the change, so D shows a goal whose host refuses it and no reason. A new ticket does not fix that: `goal_join` answers 'joined' from D's own records and sends nothing (invitations.rs:271-281; the plan cites the measurement at 1870-1873). So 'joins again' works only with the design's notice in v2. Work by any member anchored at g5 or later stops counting. |
| The same, at the extreme: the copy is older than every other member's admission. | The host's chain lists no other computer, so it dials nobody (peers.rs:179-198) and refuses every caller, and the members never receive the change. Two groups, each consistent, that never meet, unless the notice or G1's call-back is kept. On every computer that does hold the change, the host's agent is the goal's only member again and its results count with no approval under answer 5. G1 held the host's agents to prevent exactly that (plan 1819-1826). |
| The copy is older than a removal. The copy holds g5; afterwards g6 removed Birch and opened a new content key, and members B and C hold it. | Breaks unless the removal is signed again. After the change Birch is a member on every computer and is served the new key. Records Birch signed after the original cutoff count again. The repeat is in B2, not B1, picks a later cutoff, and can fail inside landing unless break 6's fix is applied. A removal that reached no other computer is lost and nobody is told. The dropped removal and the repeated one use the same epoch number with different keys, which today's one-key-per-number store cannot hold (replica.rs:216-218). |
| The old key's later records reach some members before the new key's first record and others after. | Holds as far as the design's last check goes: that reader could not break 'set aside in any arrival order' (design note 553-555), and standings are a function of the held set. It is argued from hand traces; no model exists (2526-2528). I could not show two things. A member that receives the new key's records before the old key's log is complete drops the change record and needs another exchange; convergence of a fresh joiner is 'not traced' (2522-2525). And members that already acted on records later set aside (a delivery, a claim, a folder that followed a recorded file change) have local state the replay does not undo. |
| One copy restored twice, with the design's computed key. | Breaks. Both starts compute the same key and sign the same change record. The second holds only position 0 of the new log and signs position 1, where the first start's record already sits on members' computers. The new log is forked and the goal halts. The design's defence is a hold until 'heard' (scenario S5a, break 7), which is the unprovable wait this option exists to remove. |
| Restored twice from two copies that do not contain each other, with a fresh key each time. | Breaks under the design's rules. Two live change records on one term dispute it: no host key signs and everyone who signed under either waits (design note 744-772). A withdrawal must be signed by the change's own key, and the first start's key was in the folder the second restore replaced, so the dispute can never end. It needs a new rule: the running computer withdraws its own change and continues from the other's tip. With a third restore, break 3 applies: members who worked in the later term lose their keys in the goal when the dispute settles, unless fix 1 is applied. If the second copy was made after the first change, there is no break: the terms simply chain. |
| The copy is started while the original is still running, and is noticed as a copy. | No fork, but the wrong computer may win. The copy signs a change; the original's later records are dropped everywhere; the original finds a change it holds no key for and is no longer the host. Right after a move to a new computer, wrong when an old copy was started by accident, and nobody was asked. Taking the seat back needs one command. If either copy reacted by itself to the other's change, they would answer each other without end, one author per round, until the 4,096-author limit stops sync for good (limits.rs:35). Both copies also share the endpoint secret (identity.rs:40); what the transport does with that was never read (companion 203). Both hold every agent's seed, so the agents' logs can still fork. |
| The copy is started while the original is still running, and is not noticed (a cloned disk or a snapshot). | Breaks exactly as today: both sign with the old key and the goal halts for good. The option changes nothing here. |
| A member's computer, which holds no host key, is restored. | The option does not apply. If the agent signs at a position it had used, its log is forked and that agent signs nothing more in the goal; the person joins with another agent. Preventing it needs G1's hold for agent keys, whose release without marks is a guess (plan 1260, residual 6). The damage is one agent in one goal. |
| A goal the person runs alone, restored from any copy or moved. | Holds, and is the clearest gain: no wait and no command where the plan today waits for `goal continue` (plan 1281, mockup G-2). Nobody else holds anything, so nothing is dropped. Each restore adds one author and one key epoch. Two things I could not show: the page publisher after a restore (its counter is behind the service's, plan 1850-1863), and the agents' keys if the goal had in fact been shared after the copy was made, which the copy cannot know. |
| The copy is older than the end of the goal. | The end is outside the base's ancestry, so by the host-change rule it is void and the goal is alive again on every computer; agents resume work. E1 calls the end final and recommends that it survive even a fork (plan 3421-3424). The design round must choose. |
| The change record, a repeated removal or a withdrawal is signed by the daemon itself inside start or inside landing a received batch. | If the record is not effective on the daemon's own copy, `advance` answers a conflict, `land` fails, a received batch is answered as a protocol error and `Node::open` fails, at every later start too (commit.rs:140-152, 294-314; replica.rs:212; node/mod.rs:186-187). This is the design's break 6. Every unattended act here needs the one guard its fix names: passed over, never an error. |
| The host's agent was disconnected before the restore, and the new key or its endorsement comes from that agent's seed through an 'active agent' test. | After the first change the daemon no longer counts as host and the goal admits, removes and records nothing, for good: the design's break 5. K1's promise that disconnecting that agent stops no host command (plan 1085-1090) would hold only until the first restore. Store the term's seed and read the endorsing seed whether or not the agent is connected, or endorse with the goal's first key. |
| The change record is signed at start, before any exchange, while every member is online and holds the old key's later records. | Not a safety break, a needless loss. G1 with kept marks would hold for two exchanges and then extend the old log with nothing lost (plan 1733-1734). The option signed eagerly drops the same records for everyone. `Node::open` drives automatic steps before the transport exists (node/mod.rs:180-188), so the first due stage step forces the change at once unless that is changed. |
| A file system where a file's identity changes at every mount, so every start looks like a copy (plan 1794-1798). | Under G1 this cost a wait. Here each start retires a key and adds a term, an author and a key epoch, and revokes invitations. After enough starts the frontier limit is reached and the goal stops syncing for good. |

## Questions the readers would put to the owner

As the readers wrote them, each about what a person using Locust would see.
They have not been put to the owner, and several are the plan author's to
settle.

**1. From S2a: the host's hold ends after hearing from the computers the old
copy lists.**

You restore your whole computer from a backup, or move to a new computer. For
a goal you host that has other members, should Locust carry on by itself once
the teammates' computers it knows about have answered, or wait until you type
one command?

Carry on by itself: you type nothing when every teammate in the backup comes
online. If one of them never does, the goal stays 'catching up' until you type
the command anyway. In one rare case nothing warns you. If, since that backup,
you removed every teammate it lists, or they were all away, and you brought in
new people, your computer goes on from the old list. The people you removed
are back in, the new people are shut out, and if the two versions ever meet
nobody can change members or rules in that goal again.

Wait for the command: after any whole-computer restore or move, status lists
each goal you host under 'Waiting for you' with one command that covers all of
them. Until you type it your computer keeps collecting what teammates hold,
admits nobody and records nothing. Teammates' agents keep working. The command
shows who has answered. This is what answer 13 already does for a goal with
nobody else in it.

**2. From S2b: the farm service's answer Current ends a hold.**

For a goal with a public page: after you restore or move your computer, may
the goal carry on by itself as soon as the farm service shows that your
computer published nothing newer than the copy?

Yes: after a move a public goal usually carries on within a minute with
nothing typed. It can break for good if you changed members or rules while the
page was not updating, because the service was down or the page was paused,
and then restored.

No: the service is used only to notice an old copy, as answer 21 says, and a
public goal follows the same rule as any other goal you host.

**3. From S4b.**

This touches only people who write their own rules. If your rules have stages
and say that 'the task's creator' must start, approve or sign off a stage's
task, should Locust refuse those rules when you set them, or accept them and
treat the agent you started the goal with as that creator? If it refuses, you
see an error that names the stage and tells you to name a role or the members
instead. If it accepts, you see nothing, the stage's task waits for that one
agent, and it waits for good if you have disconnected it.

**4. From S5.**

After you disconnect an agent, should one command be able to connect it again?
If yes: disconnecting applies at once and prints the command that undoes it.
If you disconnect the agent you started a goal with and later need it, one
command brings it back, and you can then share that goal's first files or give
its consent to a page. If no: disconnecting stays final and asks you first.
For the agent you started a goal with, that goal's first files can then never
be shared and a page it had not consented to is never published, for as long
as the goal exists.

**5. From S6 (file rule after a rules change).**

You shared files in a goal and later change the goal's rules. Should changes
to the shared files follow the new rules too? If yes: the one command that
changes the rules also moves the files to them and says so, and file changes
that were still waiting are proposed again by their agents. If no: the files
keep the rule they had when you first shared them, the command says so, and
there is no ordinary command to change that rule later.

**6. From S8.**

You set an agent to ask and allowed it one task. The task finished. Later a
reviewer rejects the result, so the task is open again. Should your agent take
it again on its own, or ask you again? On its own: you do nothing, and the
allow line reads 'until the host revises it'. Ask again: status shows that the
agent wants to take the task and you run allow once more. Your first answer,
least friction, points to the first.

**7. From Version boundary: E1 is not the last change of the event format.**

Will people be given a version of Locust v2 before the public door is ready?
If yes, every goal they start with that version stops taking new work on the
day the door version arrives, and they start those goals again. If no, the
first version anyone gets already has the door, and nothing they made stops
until the later version in which a host can be replaced.

**8. From E3 row.**

When nothing is happening in a goal, should Locust check for news less often
in v2, or should that wait until later? If it is in v2: a quiet goal uses less
network. Its public page reads "Quiet" most of the time and "Receiving
updates" for two minutes after each check-in. A computer that was asleep when
something new was signed can take up to an hour to get it. If it waits: every
goal keeps checking every 30 seconds, the page reads "Receiving updates"
steadily while the host's computer is on, and nothing arrives late. Either way
the person types nothing.

**9. From Version boundary.**

The private part of v2 will be ready before the public door. Do you want to
start real goals on it right away? If yes: you can use private goals sooner,
and every goal made then stops working on the day the door's release is
installed, because the door changes the signed format. Locust would say so
when you start a goal. If no: nothing is used for real goals until the door is
done too, and goals made then last until the backup-host release.

**10. From C1.**

In a goal with the 'open' rules, where a member's own word makes their work
count, should the shared plan work the same way? If yes, every goal with no
named decider always shows one current plan, and under 'open' any member can
replace it alone, as they can already replace the shared files. If no, a goal
under 'open' or 'pipeline' never shows a current plan, only proposals, unless
the host changes the rules to name someone who picks.

**11. From C5.**

After you disconnect an agent, should you be able to connect it again under
the same name? If yes, disconnecting is easy to undo: the agent gets a new
credential and carries on, and nothing in its goals is stuck. If no, as
planned, disconnecting the agent you started a goal with means that goal can
never get its first shared files, and a page waiting for that agent's consent
is never published, until backup hosts exist.

**12. From C8.**

May two members of a public goal show the same name? If yes, nobody is turned
away for the name they chose; the page can show two people called Maple, and
commands tell them apart by a short key. If no, a joiner who picks a name
already in use is refused and must join again with another name.

**13. From C10.**

When the private part of v2 is built and tested, should it be released before
public goals are ready? If yes, people can use private goals sooner, and goals
they start then stop working when the public goals release arrives, with a
notice that says so. If no, nothing is released until public goals are ready;
no goal is cut off at that step and everything arrives later.

**14. From A1 defer R8.**

Should a goal with no lead get a plan that updates by itself in v2, or later?
If in v2, when another member approves a plan change it becomes the plan on
every computer, in private and public goals alike. If later, a goal started
with the default rules shows only proposed plans and never a current one, and
a public goal has no plan for newcomers to read.

**15. From P3 service false release.**

After you restore your whole computer from a backup, should a public goal you
host carry on by itself once the farm service has accepted a check in, or wait
for one command from you? If it carries on, you do nothing, but if your
computer admitted someone or recorded a change in the last moments before it
was lost, the goal's membership and rules can stop for good. If it waits, you
run `locust --owner goal continue` once, as already decided for a goal with
nobody else to ask, and nothing breaks.

**16. From P4 local contradictions.**

When you change a goal's rules, should later changes to the shared files
follow the new rules too? If yes, one rule applies everywhere after the
change. If no, as planned, the files keep the rule they were first shared
under for the life of the goal, and the change-rules plan would have to say
so.
