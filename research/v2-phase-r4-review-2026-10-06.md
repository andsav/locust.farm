# Review of phase R4 as built: names and roles in the goal

Status: research finding, 6 October 2026. A review by reading. Nothing was
built or run for it, apart from the tests of the one fix it made.

Phase R4 of the [v2 plan](../docs/master-plan.md) is the
[roles plan](../docs/roles-and-permissions-plan.md)'s "Phase 4: Names and
roles in the goal", fifth in the build order. It landed in `8c086c1`, with
its model change in `3a56930` and its
[build notes](v2-phase-r4-build-notes-2026-10-06.md). Eighteen readers each
took one part of the change or one line through it: the signed format; the
host's chain; rules and counting; steps, closing and the shared files in
replay; the daemon's role and rules requests; invitations and joining; views
and the API; the terminal texts; formations and the site's editor; the tests
against the plan; the plan's changes in two halves; the model; guides and
scripts; and three lines through the whole change, as a hostile member, for
agreement between computers, and against the earlier phases. A critic then
named what nobody had read, and one more reader sized the cost of replay.
Each report went to two second readers who tried to refute it, and the
stricter verdict is the one kept; the three cost reports went to one. What
all of them returned is in the
[evidence folder](evidence/v2-phase-r4-review-2026-10-06/README.md).

File and line numbers are those of `8c086c1`. The
[K1 fixes](v2-phase-k1-fixes-2026-10-06.md) landed in `5b3fe29` while this
review ran and moved some lines; where a second reader also checked that
commit, the finding says so.

## Result

The design held. Role holders are read at each act's own host record. A
member's latest review is taken from that member's own log order. Decisions
after a change of lead are ordered by host record first. An agent's request
is still replayed before its level is applied. No reader found two computers
that hold the same records and the same formations disagreeing.

One finding was serious, and it is fixed in `15a8aac`. A completion rule
read the rules at each witness record's host record before checking that the
record counted. So any member could sign one review or declaration naming a
host record nobody holds, and the result it pointed at stopped counting on
every computer, under any rule with a declaration part. Five readers reported
it independently. The fix skips a record that is not effective before its
rules are read, and
`a_record_naming_an_unheld_host_record_withdraws_no_other_members_result`
fails without it. With the fix, format, clippy and the whole workspace test
suite pass (1,104 passed, 0 failed, 14 ignored).

The readers made 98 reports, which are 52 distinct
findings. No second reader refuted any: 77 reports were confirmed and
20 narrowed. One report, the cost reader's copy of the serious
finding, was not sent to a second reader because five others already had
been. The cost findings are by reading; nothing was measured.

Who acts on the rest:

- **Phase 6 (R5, the one view and refusals)** takes 10 findings
  that sit in the status view, the `pending` list and the refusals it
  rewrites.
- **A fix session** takes 39: replay and steps, the member selector,
  `workspace init`, `rules bind`, two scripts, the model and the tests.
- **The plan** changes for 6, listed under
  [Corrections to the plan](#corrections-to-the-plan).
- **The guides phase (R6)** takes 2.

| Kind | Where | Finding | Check | Who acts |
| --- | --- | --- | --- | --- |
| wrong result | `locust-core/src/goal/fold.rs:1103` | Any member can stop another member's result from counting under a rule with a declaration part, by signing one record that names a host record nobody holds. | confirmed | fixed in `15a8aac` |
| wrong result | `locust-core/src/goal/flow.rs:412` | Adding a member or giving a role makes the host's computer sign an offer for every stage task ever opened, finished and closed ones included. | confirmed | fix session, and the plan |
| wrong result | `locust-core/src/node/views.rs:419` | `pending` lists results for review on closed, decided and revised tasks, though nothing asks anyone to review them. | confirmed | phase 6 (R5) |
| wrong result | `locust/src/cli/selectors.rs:113` | `--member` picks another person's member by its signed name before the person's own agent of that name, and a member named like a key prefix is picked by that prefix. | confirmed | fix session, and the plan |
| wrong result | `locust/src/cli/workspace.rs:551` | `workspace init` rewrites a formation's own integrator role and the daemon then refuses it as a change of the role's kind. | confirmed | fix session |
| wrong result | `locust-core/src/goal/fold.rs:968` | The shared files' first acceptance without review can happen a second time once the accepting role has moved to another member. | confirmed | fix session |
| wrong result | `locust-core/src/goal/chain.rs:115` | A role's holders can differ on a computer that has not yet received an earlier formation. | confirmed | fix session, and the plan |
| wrong result | `locust-proto/src/event.rs:507` | A role held by more than about 500 members can no longer be given or taken, and review-panel's reviewer role reaches that size by itself. | confirmed | fix session, and the plan |
| wrong result | `locust-core/src/goal/fold.rs:563` | A member whose role was taken can still approve results posted after the take, by naming an older host record; picks and closes have a bound that stops this and reviews do not. | partly | fix session, and the plan |
| wrong result | `locust-proto/src/event.rs:1011` | A member's chosen name can imitate the separators of the status lines and so appear to hold a role. | confirmed | phase 6 (R5) |
| wrong result | `locust-proto/src/event.rs:1011` | Two members can carry names that read the same, and nothing the host sees tells them apart. | partly | phase 6 (R5) |
| work waits on a human | `locust/src/cli/only_you.rs:1329` | Changing a goal that already has members to rules that need reviewers leaves the host's agent as the only reviewer, and nothing says so. | confirmed | fix session, and the plan |
| work waits on a human | `locust-core/src/goal/flow.rs:494` | Adding a member asks a departed author's agent for a review request it can never sign, and status shows it as stalled. | partly | fix session |
| work waits on a human | `locust-core/src/node/requests/invitations.rs:317` | A waiting join cannot be run again without retyping the same `--name`, and a name chosen by mistake cannot be corrected. | partly | fix session |
| work waits on a human | `locust-core/src/node/requests/goals.rs:168` | `rules bind` refuses a formation that lists a role no rule uses, and the refusal does not say which role. | partly | fix session |
| cost | `locust-core/src/goal/mod.rs:349` | Every `pending` read rebuilds the whole verifier and walks every held record once per result waiting for review. | confirmed | phase 6 (R5) |
| cost | `locust-core/src/goal/fold.rs:1091` | The latest-review rule scans every review of a result once per review, so one member's flood of reviews slows every computer in the goal. | confirmed | fix session |
| cost | `locust-core/src/goal/chain.rs:268` | Role lists are copied twice per host record on every replay, and one of the copies is never read. | partly | fix session |
| cost | `locust-core/src/goal/fold.rs:112` | Replay resolves the rules, and validates the whole formation again, once per task and host record pair, though nothing in that work reads the host record. | confirmed | fix session |
| cost | `locust-core/src/goal/flow.rs:427` | Each replay works out the wanted review requests by scanning all held records per reviewer, and the new default asks every other member to review every result. | partly | fix session |
| cost | `locust-core/src/goal/projection.rs:416` | The projection resolves every picked task again at the head on each replay, only to read its task binding. | confirmed | fix session |
| untrue text | `locust/src/cli/presentation.rs:81` | The terminal prints an approvals-plus-check rule without the grouping the website shows, so it reads as if one approval were enough. | confirmed | phase 6 (R5) |
| untrue text | `locust-core/src/node/requests/goals.rs:170` | Role refusals never show the person which role, or the roles the goal has; the name is only in a JSON details field. | confirmed | phase 6 (R5) |
| untrue text | `locust/src/cli/presentation.rs:618` | `goal status` tells people who do not host the goal, and agents, that members "you add or invite" become reviewers. | confirmed | phase 6 (R5) |
| untrue text | `locust/src/cli/presentation.rs:188` | While an agent is still joining, status shows the ticket's host name as the host with nothing marking it as the ticket's word. | confirmed | phase 6 (R5) |
| untrue text | `locust/src/cli/only_you.rs:957` | Adding or joining an agent that is already a member, with a new name or role, says it happened under that name and role; nothing changed. | confirmed | fix session |
| untrue text | `locust-core/src/goal/chain.rs:575` | A formation that names a participant who is not a member is refused with a sentence about a role's holders. | confirmed | fix session |
| untrue text | `locust/src/cli/only_you.rs:1316` | The `rules bind` plan counts file changes "to propose again" that need no new proposal, and the host's agent cannot propose first files again. | confirmed | fix session |
| untrue text | `locust/src/cli/roles.rs:189` | `role give` and `role take` print "A ROLE here: ." for a role no rule names. | confirmed | fix session |
| untrue text | `sites/locust.farm/src/lib/formation-editor/ui/PointBox.svelte:449` | The site's editor still says a rejection never takes a counted result away. | confirmed | fix session |
| untrue text | `locust-core/src/organization/explanation.rs:8` | What Locust says about a formation still calls roles "bound", and the schema says members are bound at creation. | confirmed | fix session |
| scripts and guides | `scripts/live_farm_demo.py:289` | `live_farm_demo.py prepare` still declares the first files done and now aborts; its unit test cannot notice. | confirmed | fix session |
| scripts and guides | `scripts/check_operations.py:218` | `check_operations.py` still declares file changes done, which the new default rule for the files refuses. | confirmed | fix session |
| scripts and guides | `skills/locust/SKILL.md:149` | The agent skill and the apply guide still describe the old default for shared files. | partly | the guides phase (R6) |
| scripts and guides | `docs/guide/formation-authoring.md:103` | The authoring guide tells the host to run `role give` after a review-panel join, which now answers that nothing changed. | partly | the guides phase (R6) |
| departs from the plan | `locust-core/src/node/callers.rs:95` | An agent that asks for `role give` or `role take` is told `person_command`; the refusal never carries the act the plan names. | confirmed | phase 6 (R5) |
| leftover | `locust-core/src/goal/mod.rs:337` | `Goal::role_holders` is added and nothing calls it. | confirmed | phase 6 (R5) |
| leftover | `locust-core/src/node/requests/invitations.rs:26` | `InviteRecord.host_name` is written to the store and never read. | confirmed | fix session |
| leftover | `locust-proto/src/invite.rs:133` | The doc comment of the ticket's signature now sits on the new `role` field, and four nearby comments no longer match the code. | confirmed | fix session |
| leftover | `research/tla/organization.md:30` | The model's property map cites two tests that do not exist, and the model header names the old baseline. | confirmed | fix session |
| model gap | `locust-core/src/goal/closure.rs:65` | The order of closes and reopens after a change of lead has no test and is not in the model. | confirmed | fix session |
| model gap | `research/tla/Organization.tla:307` | The model's `RolesNeverEmpty` always holds, and the host-agent start, the one-holder rule and the validity of a role record are never exercised. | confirmed | fix session |
| test gap | `locust-proto/src/event.rs:778` | Three refusals of a bad name have no test: an admission with a bad role name, a ticket with a bad host name, and a signed join request with a bad name. | confirmed | fix session |
| test gap | `locust/tests/cli.rs:2581` | What `role give` and `role take` print is tested only for the Undo of a group role: a lead's Undo, the earlier-rules line and the sentences about what a role does are not. | confirmed | fix session |
| test gap | `locust/tests/workspace.rs:567` | No test pins the rule `workspace init` gives the files; the plan's test is absent and the nearest assertion passes with the old default. | confirmed | fix session |
| test gap | `locust/tests/cli.rs:2917` | The `rules bind` test omits the refusal for a rule only the task's creator could meet and the line counting file changes. | confirmed | fix session |
| test gap | `locust/tests/cli.rs:2817` | The test named for the role that `goal add` carries never sends the request past its plan. | confirmed | fix session |
| test gap | `locust-core/src/node/tests/roles.rs:290` | No test covers a name or a role crossing two computers. | partly | fix session |
| test gap | `locust-core/src/goal/tests.rs:2837` | The test that review requests follow roles checks only what is wanted at the head, never a signed request. | partly | fix session |
| test gap | `locust-core/src/node/tests/roles.rs:199` | The test for a role only earlier rules declare passes whether `role give` replaces the lead or adds to it. | confirmed | fix session |
| test gap | `locust-core/src/goal/workspace_tests.rs:1095` | Two first-files tests the plan names assert less than the plan says. | confirmed | fix session |
| test gap | `locust-core/src/goal/delegation.rs:122` | No test fails if a subtask may name a different picker or closer than its parent. | confirmed | fix session |

## Corrections to the plan

Where the code follows the plan and the plan is what falls short, the plan
changes. This session owns the plans and applies these to the
[roles plan](../docs/roles-and-permissions-plan.md):

- **Offers for stage tasks.** An automatic offer is wanted only for a stage
  task that is still open, as a review request already is.
- **A change of rules in a goal that has members.** When the new rules count
  on a group of reviewers the host's agent cannot supply alone, `rules bind`
  gives that role to the members already in the goal, in the same commit, and
  says so. This is what adding and inviting already do, and it follows the
  owner's rule that work never waits for a person.
- **A role named by an admission.** An admission that names a role with no
  list starts the list with the host's agent and the new member, so a
  computer that does not yet hold an earlier formation reads the same list.
- **Reviews by a member whose role was taken.** A review, a check or a
  declaration counts only when its host record is not earlier than its
  subject's. The risk note says what is left: such a member can still review
  results posted at or before the host record it stays on, until the host
  removes it.
- **The size of a role.** One role holds about 500 members, because a role
  record carries the whole list. Above that the list still grows by
  admission and cannot be edited; the command says so in plain words.
- **Choosing a member by name.** `--member` takes a key, then a unique key
  prefix, then a name, where a name is a member's signed name or the name of
  one of the person's own agents; two different members under one name is
  refused with both listed.
- **Smaller wording.** `Goal::role_holders` is not added; an invitation's
  record keeps no host name of its own; the `rules bind` plan counts only
  the file changes the change of rules strands; a waiting join can be run
  again under another name; `role give` has a sentence for a role no rule
  names.

One finding needs no change to the plan: it already says there is no
integrator role, and the fix is for a formation's check to refuse a role
named as the one who accepts file changes.

## What this review did not cover

- The builder's notes record that local-network qualification was waived by
  the owner after a discovery failure that reproduces on the commit before
  this phase. This review did not look at it.
- Nothing was measured. The cost findings give growth by reading the code.
- The site's editor was read against the Rust it mirrors, not opened in a
  browser.

## The findings in full

### `crates/locust-core/src/goal/fold.rs:1103` (wrong result)

Any member can stop another member's result from counting under a rule with a declaration part, by signing one record that names a host record nobody holds.

**How it goes wrong.** 1. Goal under `open` (Formation::default, completion = Declaration by the result's author; presets.rs:30, organization.rs:218-223). Member A publishes X and declares it; X is approved and its round completed.
2. Member M, with a modified daemon, signs CompletionDeclared (or ReviewRecorded / CheckAttested) with subject X and header anchor = X's own id (any id that is not a governance record works). Header::check only requires the anchor to be Some (event.rs:821). screen keeps it because M was admitted (screen.rs:36-44). History never drops events.
3. On every computer Verifier::new indexes it under X with no standing test (fold.rs:53-59). In predicate, allowed is None and the rule is a Declaration, so `_ => Some(candidate.id())` (fold.rs:1097) lets it through; `self.resolve(context, candidate.header().anchor.unwrap())?` (fold.rs:1103) finds no snapshot and returns Err(Pending(Waiting::Anchor)) (fold.rs:116-119), leaving the whole predicate before A's declaration is counted.
4. projection.rs:209 turns the Err into approved = false with empty evidence and `round.completed |= approved` stays false; the same at :236 for document revisions and :262 for shared-file proposals. task_available (goal/mod.rs:435) reports the task as takeable again. Under `independent-attempts` the lead's pick is built from that empty evidence (requests/tasks.rs:296-304), so the pick fails with Waiting::Evidence (fold.rs:979-984) and X cannot be picked; a shared-file proposal M targets is refused at requests/workspace.rs:556.
5. Removing M changes nothing: the record stays in history, stays indexed, and its anchor still has no snapshot. M can repeat it for every result.
Corrections to the original wording: (a) the flow.rs:210 propagation into stage readiness is not reached by the built-in `pipeline`, because ship requires draft's completion and draft uses Reviews/Contribution, where latest_review already filters to effective records; it applies to custom formations whose required stage completes by declaration. (b) The no-hostile-member transient is narrower than stated: a sync exchange sends the governance log first (sync/outbox.rs:14-29), so it needs a relay that itself holds the member's record without the governance record it names. Results already picked are unaffected (pinned path, projection.rs:363).

**What the second readers concluded.** Under any completion rule that is a Declaration (`open`, `independent-attempts`, the ship task of `pipeline`, and shared-file changes that inherit that rule), one held review, declaration or check record about a result whose anchor is not a record on the host's chain makes `Verifier::predicate` return an error, so the result stops counting on every computer and stays that way. This is a regression introduced by 8c086c1: the per-candidate `resolve(...)?` runs before the candidate's effectiveness test.

**Smallest fix.** Change the code, not the plan. In `Verifier::predicate` (crates/locust-core/src/goal/fold.rs), move the test `self.status(candidate.id(), proof) == Standing::Effective` ahead of the `self.resolve(context, candidate.header().anchor.unwrap())?` line and `continue` when it is not effective; then drop it from the final `if eligible && ...`. An effective witness always has an anchor snapshot (chain.rs authorize_base:441-447) and names the subject's own context (fold.rs `subject`, 749-757), so its resolve already succeeded in `check` and cannot fail here. Add a signed replay test in crates/locust-core/src/goal/tests.rs: under Formation::default, worker 0 publishes and declares X; worker 1 signs a CompletionDeclared and a ReviewRecorded of X via `self.workers[1].event(self.id, Some(x), body)`; assert X stays approved and its round completed in all of `f.replays()` (forward, reversed, reloaded).

**What a person sees.** In a goal where a member's own word makes a result count, a finished result goes back to "not approved" for everyone and its task shows as open to take again, after one bad record from any member. In a goal where a lead picks among results, the lead can no longer pick that result, and a shared-file change can no longer be taken in. Removing the member does not bring any of it back; the only way out is to post the result again, which the same member can spoil again.

Reported by: model-fidelity:1, plan-changes-first-half:1, rules-counting:1, flow-closure-workspace:1, lens-convergence:1. Who acts: fixed in `15a8aac`.

### `crates/locust-core/src/goal/flow.rs:412` (wrong result)

Adding a member or giving a role makes the host's computer sign an offer for every stage task ever opened, finished and closed ones included.

**How it goes wrong.** 1. Hand-written formation: a flow stage with `recipients: members` whose task type has `starts: [offered { by: task_creator (or members), to: members }]`. The built-in presets never trigger this (pipeline stages use independent starts; directed's `by` is the lead role, which the host's goal key does not hold).
2. Stage tasks open (one per stage per rules binding, so several stages or several bindings give several tasks). The host signs the open-task record and one offer per member at that time. Most of the tasks are then completed, picked or closed.
3. The host admits a member. `desired_effects` runs `offer_templates(binding, stage, None, head)` for every stage of every effective `RulesBound`. `stage_template` at the head selects recipients from `snapshot(head).members`, now including the newcomer; `stage_instance` only requires that the stage was materialized; `resolve` of the task context does not look at closed, completed or selected.
4. The offer's identity hashes `target_slot` = recipient, so each (task, newcomer) pair is a new effect id; `insert` skips only an id that is already materialized.
5. `drive_flow` on the host signs every one. `validate_effect` accepts them: it re-derives the same template at the record's anchor and, unlike the manual `WorkOffered` arm, calls neither `active_context` nor any openness test.
6. Result: one host-signed `EffectMaterialized{Offer}` per non-open stage task per new matching member, plus a delivery outbox entry for each. The same happens when `role give` makes a member match a stage's `recipients` role.

Correction to the reported visible effect: the offers do not show up as startable work, because `pending_work` filters through `can_start` and so `task_available`. They show as delivery lines awaiting acknowledgment.

**What the second readers concluded.** Confirmed. Since 8c086c1, `desired_effects` computes automatic stage offers with recipients taken at the head and never tests whether the stage task is still open, so each newly admitted member (or new holder of the stage's recipient role) makes the host sign one offer per stage task that was ever opened, including completed, picked and closed ones. Review requests got an openness guard in the same function; offers did not. The plan asks for exactly this, so plan and code agree and both need the guard. It is still unguarded at HEAD 5b3fe29, which only changed the offer to name the task's current round.

**Smallest fix.** Code and plan both change.

Code: in `Verifier::desired_effects` (crates/locust-core/src/goal/flow.rs, the `RulesBound` arm around line 412 at 8c086c1, 416 at HEAD), skip an offer unless its context's round is open: not closed, not completed, nothing selected. `Goal::task_available` itself is a `Goal` method over projected state and is not callable from the verifier as the finding implies. But `evaluate` in fold.rs runs `projection::project` before `desired_effects`, so pass `&evaluation.state` in and apply the same predicate via `state.task_round(effect.context)`. Guard only the wanting side, as for review requests; leave `validate_effect` alone so an already-signed offer keeps its standing. The "current round" half of the proposed fix already landed in 5b3fe29 (`offer_templates` uses `current_round(task, anchor, proof)`).

Add a goal test beside `review_requests_skip_results_that_count_and_tasks_that_are_not_open`: complete or close an offered stage task, admit a member, and assert no `EffectAction::Offer` is desired.

Docs: add the offer guard next to the review-request sentence in docs/roles-and-permissions-plan.md (about lines 2475-2484 and 3117-3118) and in docs/formations.md "Automatic steps and delivery", which today states the guard only for review requests.

**What a person sees.** Only with hand-written rules where the host hands out pipeline tasks by offer. Each time someone joins or is given the matching role, their agent's pending list gains one "Delivery: offer, awaiting acknowledgment" line for every old pipeline task that is already finished, picked or closed, and it is told to acknowledge each one. Those tasks never appear as work it can start. The goal's history gains one host-signed record per old task, and one more per task if the agent acknowledges. Nothing is lost or blocked.

Reported by: flow-closure-workspace:2, lens-convergence:4, lens-earlier-phases:1. Who acts: fix session, and the plan.

### `crates/locust-core/src/node/views.rs:419` (wrong result)

`pending` lists results for review on closed, decided and revised tasks, though nothing asks anyone to review them.

**How it goes wrong.** `directed` goal, Harbor lead and reviewer, Maple reviewer. Task T has results A, B, C. Maple approves A; Harbor picks A (or closes T, or the host runs `task revise` on T). 1) `desired_effects` skips B and C: any Select in a task context (flow.rs:456-467), last close/reopen is a close (468-485), or round not current (486-493). 2) The view filter keeps them: B and C are not approved, `selected` is true only for A itself (views.rs:429-433), and `Goal::can_review` (goal/mod.rs:436-477) only asks whether the member is admitted by the Reviews rule of the result's own round, which stays Effective after a pick, close or revise and is resolved with the role holders at the head (mod.rs:372-389). So B and C stay in Harbor's and Maple's `to_review`, and `PendingCounts.to_review` (api/context.rs:179) counts them. 3) A reviewer admitted afterwards holds the role at the head, so it is listed B and C too. 4) `review record` on B is signed and is effective: the `ReviewRecorded` arm (fold.rs:563-588) calls `subject` and the reviewer test only, never `active_context` or a closed test. An item leaves one reviewer's list once that reviewer has reviewed it, and everyone's list once the result counts. One correction to the report: the review is not always without effect. An approval of B lets the lead re-pick B (a successor Select is valid, fold.rs:938-985), and on a closed task in its current round it can still satisfy a stage prerequisite (flow.rs:140-222). Only on a revised round does it feed nothing current. The engine has still chosen not to ask in all three states, so the view should not ask either.

**What the second readers concluded.** From this commit the engine signs no review request for a result on a task that is picked, closed or revised, but `pending.to_review` (and the `to_review` count in the brief) still lists every such result that does not count to every member who may review it and has not, including a reviewer added later. The filter itself was not touched by this commit and matches the plan's own wording for the view (plan lines 1973 and 2507), so the plan and the code both miss the "task is open" test that the request rule gained.

**Smallest fix.** Change both code and plan. Code: in `pending_work_with_news` (crates/locust-core/src/node/views.rs:419-436) add the open-task test to the `candidates` filter. For a task-scope context keep the item only when `entry.goal.current_context(context.scope) == Some(context)` and `entry.state().task_round(context)` has `!closed && selected.is_none()`. The projection already holds these three facts from the same decision order the request rule sorts by (projection.rs:114-137 and 343-455), so no per-candidate Verifier is needed; for goal and document scopes mirror the close test with the last closure decision in `state().decisions`. A shared `wants_review` helper as proposed is fine if it stays this cheap. Add a node test that picks, closes and revises a task and asserts the other results are in nobody's `to_review`, including a reviewer joined afterwards. Plan: add "and whose task is open" to the `to_review` sentences at docs/roles-and-permissions-plan.md lines 1973 and 2507.

**What a person sees.** After a task is decided, closed or revised, `locust pending` and the review count in the brief keep saying "Review needed" for the results that lost. Each reviewing agent spends a turn (and signs a review record) on each of them, and a reviewer added later is shown the whole backlog of such results. Nothing is blocked and nothing is lost; it is wasted agent work and a misleading list.

Reported by: views-api:1. Who acts: phase 6 (R5).

### `crates/locust/src/cli/selectors.rs:113` (wrong result)

`--member` picks another person's member by its signed name before the person's own agent of that name, and a member named like a key prefix is picked by that prefix.

**How it goes wrong.** 1. Host runs `locust --owner goal add --goal G --agent claude-juniper-77aa0c52 --name Juniper` (the plan's own P4-1 example): the admission signs name "Juniper"; the enrolled local name stays `claude-juniper-77aa0c52`. 2. Another person redeems a ticket with `goal join --name claude-juniper-77aa0c52`. The host's daemon admits it automatically with that name: `plan_join` only checks `is_member_name` (non-empty, at most 64 bytes, no outer spaces or control characters) and never compares names. The joiner has to know the enrolled name; it is not in this goal's records, but it is the default signed name in any other goal that agent joined without `--name`, and hand-picked enrolled names such as `worker` can collide by accident. 3. Host types `locust --owner role give --goal G --member claude-juniper-77aa0c52 lead` (the name every `--agent` option takes and that status prints beside "Juniper"). The value is not a key and not all-hex, `member_by_name` finds exactly one signed name (the remote member) and returns it; the local-agent tier is skipped. 4. `role_change` signs `RoleHolders { lead: [remote] }` at once (role give has no plan or confirmation). 5. Output: `claude-juniper-77aa0c52 is a lead in "G".` then `Leads now: claude-juniper-77aa0c52 (<remote key prefix>).` The only tell is the 8-character key. The same resolver serves `member remove --member` (which shows a plan first) and the generic `--member`/`--recipient` fields such as `work offer`.

**What the second readers concluded.** `resolve_member` returns the one member whose signed name equals the typed value before it ever looks at local agents' enrolled names, so when a member on another computer carries a signed name equal to the enrolled name of one of the person's own agents (which sits in the goal under a different signed name), `--member <enrolled name>` silently selects the other member. The code follows the companion plan's stated order (key, prefix, signed name, local name), so the hole is in the plan and the code together; no check, test or declared departure covers it.

**Smallest fix.** Change both plan and code. In `resolve_member` (crates/locust/src/cli/selectors.rs), after the key and key-prefix tiers, build one candidate set: members whose signed name equals the value, plus local members whose enrolled name (from `status`) equals the value. Zero is `not_found`, one distinct key resolves, more than one is `invalid` and lists each candidate as its signed name with its unique key prefix (reuse `member_key_prefix`). The same agent matching both ways (default signed name equals enrolled name) dedups to one key, so nothing changes for the ordinary case. Extend the selector unit test with a remote member whose signed name equals a local agent's enrolled name. Reword the companion row at docs/roles-and-permissions-plan-details.md:312 and the sentence at docs/roles-and-permissions-plan.md:2702 so signed name and local name are matched together rather than in order.

**What a person sees.** They give a role to what they believe is their own agent, using the name they always type for it. The command says it worked and prints that same name back, but the role (for example lead, who picks and closes for everyone) now belongs to a member on someone else's computer. Only the short key in the holders line shows it; the printed Undo line puts it back if they notice.

Reported by: cli-texts:1, plan-changes-second-half:4, tests-vs-plan:1, cli-texts:4. Who acts: fix session, and the plan.

### `crates/locust/src/cli/workspace.rs:551` (wrong result)

`workspace init` rewrites a formation's own integrator role and the daemon then refuses it as a change of the role's kind.

**How it goes wrong.** State 1. `goal create --formation-json` with `roles:{integrator:{},reviewer:{}}` and `workspace:{integrator:{kind:role,name:integrator},completion:...}` (the valid conformance case at docs/reference/conformance/organization.cases.json:394). Creation passes: validation accepts it and replay seeds the role with the host's agent, one holder (chain.rs:536-578). No workspace epoch exists yet, so `workspace init` reaches `initial_epoch` (workspace.rs:424-427) after the person's yes and after the files were captured. There `policy.integrator != host_authority` is true, the part is rewritten to the host's key (the role stays declared in `roles`), and `Request::RulesBind` is sent. In `rules_bind` (goals.rs:160-176) `deciding` still holds `integrator` because `deciding_known` reads every binding including the first (goals.rs:270-289) and validation.rs:234 makes a workspace integrator an authority; `is_authority_role(new, "integrator")` is now false; the daemon answers conflict "this role picks or closes in this goal and has one holder; these rules make it a group. Use another role name." The `?` returns before `WorkspaceEpochSet`, so no tree starts. Re-running gives the same answer every time; only rebinding rules that drop the role name gets past it. If the role is also the picker or closer the bind succeeds and the integrator is silently the host's agent (that part is the plan's intent).
State 2. Goal with no workspace part and `decisions.completion` naming `task_creator` (valid at goal level). `workspace init` without `--completion` copies that rule into `/workspace/completion` (workspace.rs:560-566); validation.rs:235 checks it with task=false and reports `selector_scope`; `checked_definition` (goals.rs:545-551) answers invalid "the formation is invalid; validate it for diagnostics". The refusal itself is planned (plan 3171-3172: init then needs `--completion`), but the message does not say so and comes after the yes. No bundled preset reaches either state; both need a hand-written or editor-carried formation.

**What the second readers concluded.** In a goal whose own formation names a role as the shared files' integrator, `locust --owner workspace init` rewrites that integrator to the host's agent and the host daemon's new role-kind check then refuses the rebinding, so the first files can never be shared under those rules. Separately, when the goal's rule names the task's creator and no `--completion` is given, init is refused with the generic "the formation is invalid" text instead of being told to pass `--completion`. One correction to the report: always writing the host's agent is the plan's own instruction (plan 2714-2719, companion line 242), not only build note 13, so this is a gap between two plan-mandated pieces rather than a builder departure.

**Smallest fix.** Plan and code both need a line. The plan says `initial_epoch` always writes the host's agent (2714-2719) and also that `rules_bind` refuses a change of role kind (2552-2557), and never says what happens to a role named as integrator; meanwhile `rules bind` and docs/formations.md:100-102 use a formation's own workspace part as written. Smallest code change: in `initial_epoch` (crates/locust/src/cli/workspace.rs) rebind only when `formation.workspace` is missing or `--completion` is given, and when rebinding keep an existing part's integrator instead of overwriting it; make `verify_pinned_initial_policy` stop forcing the host's key over a pinned part's own integrator. That matches what `rules_plan` already does and removes the kind flip; amend plan line 2716 to "writes the host's agent when the formation has no workspace part". For state 2, run the same `/workspace` `selector_scope` inspection that `rules_plan` runs (only_you.rs:1271-1283) inside `init_plan`, so the refusal comes before the yes and reads along the lines of "This goal's rule cannot apply to shared files; pass --completion". Add a CLI test for init on a role-integrator formation; none exists today.

**What a person sees.** Someone who wrote their own rules with a named role for accepting file changes tries to share the goal's first files, says yes, and is told that a role they never touched "picks or closes ... these rules make it a group. Use another role name." The files are not shared, trying again gives the same answer, and nothing says the way out is to bind rules without that role. Someone whose rule mentions the task's creator is told their formation is invalid, though it validates fine, when all they needed was to add --completion.

Reported by: cli-texts:2, plan-changes-second-half:3. Who acts: fix session.

### `crates/locust-core/src/goal/fold.rs:968` (wrong result)

The shared files' first acceptance without review can happen a second time once the accepting role has moved to another member.

**How it goes wrong.** 1. Host runs `workspace init` (epoch E0, host's agent pinned as integrator) and, before any acceptance, `rules bind` with a hand-written formation whose `workspace.integrator` is `{"kind":"role","name":"keeper"}`. The formation is valid and bound as written; `rules_bind` signs epoch E with checkpoint `Unseeded`. keeper defaults to the host's agent, or the host gives it to A.
2. Host runs `workspace init` again; the first files F1 are posted in E with no parent. A accepts F1 as R1 (`previous: None`), then reviewed changes as R2 (`previous: R1`) and R3.
3. Host runs `role give --member B keeper`. For a deciding role this replaces the holder with one `RoleHolders` record; no new epoch is signed.
4. B signs `ScopeDecided { context: E, previous: None, action: Select { subject: F1 } }` anchored at or after the role record. An honest daemon does this through `workspace integrate --expected-empty` only if it has not yet received R1 (for example A accepted while offline); a modified daemon can do it at any time.
5. Replay: B is the one holder at that anchor; the successor lookup under (B, key, None) finds only B's record; the parent test passes because F1's parent is None and E starts empty; F1 counts without review. B's record is effective and no scope halt is recorded. R1 to R3 stay effective.
6. `project` sorts the epoch's acceptances by anchor position, so B's is last and is the head; the lineage walked from it is one revision. R2 and R3 are no longer in the files and nothing is marked disputed.
The same gap exists without first files: B can accept any approved change whose parent is an earlier revision Rk that A already built on (`previous: Rk`). An honest B that has not received A's R(k+1) does exactly this, and no dispute follows when it arrives.

**What the second readers concluded.** When a formation names a role as the files' integrator, replay now reads the integrator as that role's one holder at each acceptance's own anchor, but competing acceptances are still detected per signer. After the host moves the role, the new holder's acceptance on a starting point the earlier holder already built on (including the host's first files, which need no review) is effective, raises no dispute and becomes the head, so the earlier holder's later acceptances drop out of the files; the plan's "once per such epoch ... disputes the files" (plan lines 2523-2526) holds per signer, not per epoch.

**Smallest fix.** Smallest: refuse a role at `/workspace/integrator` in `Validator::run` (crates/locust-core/src/organization/validation.rs:233-236). That one check feeds both the daemon (`checked_definition` in node/requests/goals.rs) and replay (`valid_definition` in goal/mod.rs:44), restores one signer per epoch, and matches plan line 130 ("There is no integrator role") until Phase 9 removes the setting. It needs `workspace_policy_requires_explicit_valid_integrator_and_completion` (organization/tests.rs:18) and the site mirror (formation-editor contract/rules.ts:246, model.test.ts:489-520) changed with it.
If role-named integrators must stay until Phase 9: in `Verifier::new` and `Verifier::decision` (fold.rs:60-75, 907-937), for `Scope::Workspace` look successors up by (ScopeKey, previous) across authors and raise `Halt::Successors` when two exist. Count a candidate only if its author is the integrator at the candidate's own anchor; the current filter checks membership only, so without that any member could dispute the files.
Either way the plan sentence at lines 2523-2526 should say the guarantee rests on one signer per epoch.

**What a person sees.** Only in a goal whose hand-written rules let a role (not the host's agent) accept file changes: after the host gives that role to another member, the shared files can go back to the starting folder, or to an earlier version, with no dispute shown. Changes that were approved and accepted before the hand-over are no longer part of the shared files; they show as out of date and their authors must propose them again. Each computer's folder follows at its next update. Every computer shows the same result.

Reported by: flow-closure-workspace:3. Who acts: fix session.

### `crates/locust-core/src/goal/chain.rs:115` (wrong result)

A role's holders can differ on a computer that has not yet received an earlier formation.

**How it goes wrong.** Honest host. 1) `goal create` with review-panel: genesis, host agent admitted with role None, B1 bound (requests/goals.rs:91-134); on a full replica B1's fill gives reviewer = [host] (chain.rs:536-539). 2) `goal add`/`goal invite` for X and Y default to the counting role `reviewer` (cli/roles.rs:89-109; the invite's role is copied into the admission at peers.rs:353-358) giving [host, X, Y]. 3) Host runs `rules bind` with directed; the kind check passes because reviewer is a group role in both (requests/goals.rs:162-176); B2 declares lead and reviewer. 4) Computer Z (any member catching up: just joined, or offline since before B2) holds all events; definition blobs are fetched one at a time in blob-hash order and each landing triggers a full refold (replica.rs:320-338, 403-412; commit.rs:206-219, 308-316), so whenever B2's blob sorts before B1's there is a window with B2's definition held and B1's not. In that window on Z: validate_binding returns Pending(Waiting::Definition) for B1 before the fill (chain.rs:529-532), B1 still becomes snapshot.rules (chain.rs:221-222) so B2's compare-and-swap passes; X's admission runs `entry(role).or_default()` and yields [X], then [X, Y] (chain.rs:114-118); B2's `or_insert_with(|| vec![host])` finds the list and keeps it (chain.rs:537-538), lead gets [host], B2 is Effective. Z has reviewer = [X, Y]; everyone else [host, X, Y]. Work under B2 resolves with only B2's definition (rules.rs:128-130) and the snapshot's roles (fold.rs:116-127), so the host agent's ReviewRecorded under directed is Excluded on Z with a rule refusal (fold.rs:563-589, rules.rs:215-218, 303-316) and Effective elsewhere; anything pinned to that approval (a pick, a stage) is judged wrong on Z too. When B1's blob lands, Goal::refresh rebuilds the chain (goal/mod.rs:112-134) and Z agrees. Z's list is always the true list minus the host's agent, so Z never accepts something others refuse, and the kind check stops an honest host from turning the role into a one-holder role.

**What the second readers concluded.** Confirmed as written, at low severity: a role's holder list at a later binding depends on whether an earlier binding's formation blob is held, because the host-agent fill is skipped for a waiting binding while a role-carrying admission still starts the list without the host's agent. The code follows the plan's text literally (docs/roles-and-permissions-plan.md:2431-2438), so the plan's claim that "none of these rules reads another binding's definition" is what fails; both plan and code need the one-line change.

**Smallest fix.** In `Chain::build`, `Body::MemberAdmitted` arm (crates/locust-core/src/goal/chain.rs:115), replace `snapshot.roles.entry(role.clone()).or_default()` with `.or_insert_with(|| vec![history.host.expect("founded host")])` before the push/sort/dedup. I checked this closes it: with an honest host every list on Z is then either equal to the full replica's or absent only where the true list is exactly [host] and no held definition reads it (a valid definition must declare the roles it names, validation.rs:36-38, so a held binding that reads the role fills it with [host]); RoleHolders sets the list identically on both; removal is already symmetric. It changes nothing on a full replica, because `goal_invite` refuses a role with no list (requests/invitations.rs:180) and lists are never removed. Change the plan too: docs/roles-and-permissions-plan.md:2431-2433 should say an admission naming a role with no list starts it with the host's agent and the new member. Add a replay test in crates/locust-core/src/goal/tests.rs: review-panel, admit two workers with Some("reviewer"), rebind to directed, apply the events with only the second definition and then with both, assert the same `state().roles["reviewer"]` and the same standing for a host-agent review both times.

**What a person sees.** On a computer that is still catching up (just joined, or back online after the host changed the rules), `goal status` can list a role's holders without the host's agent, and the host's approvals show as refused, so a result looks not counted and a pick or next step built on it looks like it has not happened, while everyone else sees it counted. Nothing is lost and nothing wrong gets signed; it corrects itself as soon as the earlier formation finishes downloading, usually within seconds, longer if the connection drops mid-download.

Reported by: model-fidelity:5. Who acts: fix session, and the plan.

### `crates/locust-proto/src/event.rs:507` (wrong result)

A role held by more than about 500 members can no longer be given or taken, and review-panel's reviewer role reaches that size by itself.

**How it goes wrong.** 1. The host creates a private goal with the `review-panel` preset; `reviewer` is a group (non-deciding) role, so `goal add` and `goal invite` carry it by default. 2. 506 members join; each admission is a small record and replay pushes the member onto the `reviewer` list, which now holds the host's agent plus 506 = 507 keys. 3. The host admits one member with `--no-role` and later runs `role give --member X reviewer`: `role_change` builds a 508-key list, the header is 153 + 508*32 = 16,409 bytes, `Event::sign` answers TooLarge and the command prints `invalid: header exceeds the admitted size`. 4. If admissions have taken the list to 509 or more, `role take` on any holder fails the same way (the list after the take is still 508 or more). 5. The only way to shrink the list is `member remove`, which strips the member from every list in replay; once the list is back to 508 `role take` works again, and at 506 `role give` does. Private goals have no member cap in the code; the 16-member ceiling is planned for public goals only and is not built.

**What the second readers concluded.** A group role's holder list grows by one per admission with no bound, but `role give` and `role take` must restate the whole list in one 16 KiB header, so for a role named `reviewer` `role give` fails once the list holds 507 members and `role take` fails once it holds 509 or more, with the raw text `header exceeds the admitted size`. It is a capacity edge the plan itself designed in (whole-list record) and never stated, not a departure from the plan and not a regression; nothing is signed unattended and nothing durable is damaged.

**Smallest fix.** Smallest: in `role_change` (crates/locust-core/src/node/requests/goals.rs), catch the size refusal before or at `self.author` and answer in plain words, with code `limit_exceeded` rather than `invalid`: the role holds the most members one record can carry; remove a member from the goal or use another role. Add one test at the edge, and add a line to the plan's Risks beside the role-name note (docs/roles-and-permissions-plan.md:3165-3167) stating that one role holds about 500 members and that above it the list still grows by admission but cannot be edited. The code matches the plan (line 2350-2351 asks for the whole list), so the plan gains the stated limit; the code gains only the message. Signing one member added or removed instead of the whole list is the real cure if roles of that size are meant to work, and is a design change for the owner to weigh, not part of the small fix.

**What a person sees.** In a goal where about 500 or more members hold the same role, giving that role to one more member, or taking it from someone, fails with the words "header exceeds the admitted size", which say nothing about what to do. The only way left to take the role from someone is to remove them from the goal. Smaller goals, and nothing an agent does by itself, are affected.

Reported by: signed-format:2, chain-snapshot:3. Who acts: fix session, and the plan.

### `crates/locust-core/src/goal/fold.rs:563` (wrong result)

A member whose role was taken can still approve results posted after the take, by naming an older host record; picks and closes have a bound that stops this and reviews do not.

**How it goes wrong.** Goal under `directed`. Reviewer list at governance position 7 is Maple (host's agent) and Juniper. Juniper's newest record is anchored at 7 (true whenever Juniper signed nothing after the change, or its altered daemon never advances its anchor). Position 8: `role take --member Juniper reviewer`. Cedar's honest daemon posts result R anchored at 8. Juniper's altered daemon signs an approval of R anchored at 7. Replay: Juniper is a member at 7 and its anchors do not go backward, so the record is authorized; R is effective; the review arm reads the role list at position 7 and finds Juniper a holder; `predicate` reads the same list for the count. R shows approved on every computer. `member remove` then sets the cutoff to Juniper's latest effective record, which includes that approval, so it stays. Correction to the finding's "what remains": with the proposed bound in place Juniper can still, under `directed` (reviewer may approve its own result, and a result needs no attempt), post its own new result anchored at 7 on any task whose round is current at 7 and approve it itself at 7; and it can still approve any result anchored at or before 7. Under `review-panel` (author excluded, count 2) the bound does close new results, leaving only results that existed at the take.

**What the second readers concluded.** The scenario is reachable exactly as written: replay judges a review, check attestation or completion declaration only at its own anchor and never compares that anchor with its subject's, so a member with altered software who keeps anchoring where it still held `reviewer` can approve a result posted after the role was taken, and a later removal keeps that approval. It is not a departure from the plan, which specifies this reading and names the risk; it is a cheap hardening plus a plan-wording correction, and the proposed bound closes only other members' later results, not everything the finding says it closes.

**Smallest fix.** Change the code, then the plan's risk note. In `Verifier::check` (crates/locust-core/src/goal/fold.rs), for `ReviewRecorded`, `CheckAttested` and `CompletionDeclared` (simplest: give `Verifier::subject` the judged record's anchor), exclude the record when `chain.position(its anchor) < chain.position(subject's anchor)`. `predicate` needs no change because it already requires the witness to be effective. Add one replay test beside `an_ex_holder_anchored_before_the_change_still_counts_and_after_it_is_excluded`: result posted after the `RoleHolders` change, approval anchored before it, excluded forward and reversed. Then rewrite docs/roles-and-permissions-plan.md:3119-3127 to say what is left: an ex-holder that stays on an old anchor can still review results anchored at or before it, which under `directed` includes its own new results, until the host removes it, and removal keeps the reviews it had already signed. The wider test (offer, start, result against the record that opened the round) is optional and does not close the own-result path for tasks already open.

**What a person sees.** After you take the reviewer role from a member, nothing changes if their computer runs the normal program. If it runs altered software, it can keep approving results that others post afterwards, those results show as approved, and removing the member stops new approvals but does not take back the ones already given. With the fix, such a computer could no longer approve other members' later results; under `directed` it could still approve its own until you remove the member.

Reported by: lens-hostile-member:1. Who acts: fix session, and the plan.

### `crates/locust-proto/src/event.rs:1011` (wrong result)

A member's chosen name can imitate the separators of the status lines and so appear to hold a role.

**How it goes wrong.** 1. Host starts a review-panel goal and runs `goal invite` (the ticket carries `reviewer` by default: roles.rs:97-109 `selected_role` -> `counting_role`). 2. The invited member, key prefix 9f3b77aa, runs the stock CLI: `goal join --name 'Zed (0a0b0c0d) · lead Maple' TICKET`. The name is 28 bytes, trimmed, no control character, so only_you.rs:157, `Request::check` (api.rs:1043) and `JoinRequest::verify` (invite.rs:447) all accept it. 3. The host daemon's `plan_join` (peers.rs:309-362) checks nothing else about the name and signs `MemberAdmitted { name, role: reviewer }` with no person involved; replay adds the key to `reviewer`, holders sorted by key (chain.rs:113-118). 4. On any computer `goal status` prints `Roles: reviewer Harbor (02020202), Zed (0a0b0c0d) · lead Maple (9f3b77aa)`, the exact layout of a second role `lead` held by Maple. With the name `Zed (0a0b0c0d), Maple` the same line, and `Reviewers now:` in `role give` (roles.rs:303-323), shows three reviewers where there are two. Correction to the report: the `Member:` line is not forged "the same way". It prints `Member: Zed (0a0b0c0d) · lead Maple (9f3b77aa) · remote · endpoint XXXXXXXX · reviewer`; the real `(prefix) · remote · endpoint` always trails the name, so that line never matches the true layout, and no `Member: Zed` line exists. The count line (`N more reviewers needed`) is computed from the real list and stays right.

**What the second readers concluded.** A member's signed name may hold the same punctuation the CLI uses to lay out `goal status` and `role give` output (` · `, `, `, `(`, `)`), and `member_label` prints it bare, so one invited member can make the `Roles:` line read as an extra role or an extra holder. It is display only and the code matches the plan as written (the plan specifies both the name rule and the unquoted `NAME (prefix)` label); the member's own `Member:` line cannot be forged cleanly and looks wrong to a careful reader.

**Smallest fix.** Change the printing in `member_label` (crates/locust/src/cli/presentation.rs:164), not the signed rule. Do not quote every name as the report proposes: that changes every line of mockups P4-1 and P4-2 and the pinned strings at presentation.rs:1083, 1088 and 1124. Instead follow the pattern `quote_role` already uses (roles.rs:146-155): print the name bare when it is plain (alphanumeric characters, single inner spaces, `-`, `_`, `.`), otherwise inside double quotes with `"` and `\` escaped, e.g. `"Zed (0a0b0c0d) · lead Maple" (9f3b77aa)`. An allow-list also covers look-alike separators such as `•` or fullwidth brackets. Use the same helper for the bare name at roles.rs:285 (`NAME is a reviewer in ...`). While there, roles.rs:317 builds the host annotation with `label.trim_end_matches(')')`, which should not depend on the label's last character. Add one presentation test with such a name and one sentence to the plan's `member_label` paragraph (plan line 2704). Leave `is_member_name` alone: vectors.rs:357 and the plan's cli test deliberately keep punctuation such as `;` legal in names.

**What a person sees.** In `goal status`, someone you invited can pick a name that makes the Roles line look as if there is an extra role, or as if they hold a role such as lead, or as if there is one more reviewer than there really is. Who can actually pick, close or approve does not change, and the line that says how many reviewers are still needed stays correct. Their own Member line looks odd if you read it closely.

Reported by: signed-format:1. Who acts: phase 6 (R5).

### `crates/locust-proto/src/event.rs:1011` (wrong result)

Two members can carry names that read the same, and nothing the host sees tells them apart.

**How it goes wrong.** 1. Goal "Parser cleanup" under `directed`, members Maple (host's agent) and Juniper (remote). The host has a second invitation outstanding.
2. The second invitee joins with `--name Juniper`, or `Juniper` plus U+FE0F. Both pass `is_member_name` (only Cc controls, outer white space and length are refused). `plan_join` checks the key is not already a member but never looks at existing names, and signs the admission with the governance key.
3. Same bytes: `role give --member Juniper lead` is refused with "member name Juniper is shared; choose a key prefix: Juniper (51c2e9aa), Juniper (9f3b2c1d)", in key order. `goal status` prints two `Member: Juniper (<8 hex>) · remote · endpoint <8 hex>` lines, also in key order. The keys differ, but nothing says which is the earlier Juniper. The only other hint is `goal invitations`, which shows the full key that redeemed each ticket.
4. With U+FE0F: `--member Juniper` matches byte for byte and reaches the original, so no refusal appears. `safe` does not escape U+FE0F, so both print as "Juniper". Copying the newcomer's printed name, or its prefix, into `--member` reaches the newcomer.
5. The first result line of `role give` prints the name alone, but the holders line and the `Undo:` line of the same output carry the key prefix.
6. A homoglyph such as `Junipеr` with a Cyrillic "е" behaves like step 4, and no invisible-character filter stops it.

**What the second readers concluded.** A joiner can pick a name that is byte-identical to, or prints the same as, an existing member's, and the host's computer admits it unattended. Every listing still tells the two apart by key prefix, but nothing in `goal status` or the shared-name refusal says which one joined first, and `safe` prints several invisible code points raw. The code matches the plan here, so this is a gap in the plan, not a coding error.

**Smallest fix.** Plan and code should both change; the code currently does what the plan says.

1. Show join order, which covers exact duplicates and every look-alike. Add the admission's position on the host's chain to `MemberView`, filled in `goal_status` (crates/locust-core/src/node/requests/goals.rs:411) from `Member.admission`. Print members in that order in `presentation::goal_status`, and mark the later one in `member_by_name`'s refusal (crates/locust/src/cli/selectors.rs:155). The order is host-signed and display only, so nobody waits.
2. Extend `safe` (crates/locust/src/cli/presentation.rs:144) to escape default-ignorable code points: U+00AD, U+034F, U+115F-1160, U+180B-180F, U+3164, U+FE00-FE0F, U+FFA0, U+E0000-E0FFF. Add them to `terminal_controls_and_bidi_never_reach_the_terminal`. This matches how U+200B-200F are already handled: admitted, but printed as escapes.

Refusing those code points in `is_member_name` is optional. It cannot close look-alikes, and it would refuse emoji names that use U+FE0F. Either way, correct that function's comment, which says "visible" while the check does not test visibility.

**What a person sees.** Two members called Juniper in `goal status`, each with a different 8-character key after the name, and nothing saying which one joined later. If the names are identical, `--member Juniper` stops and asks for a key prefix. If the newcomer's name has a hidden character, `--member Juniper` still reaches the original, but the list shows two Junipers. Picking the wrong key, or copying the wrong line, gives the role to the newcomer. The result's "now:" line shows the key, and the printed Undo command reverses it.

Reported by: lens-hostile-member:2. Who acts: phase 6 (R5).

### `crates/locust/src/cli/only_you.rs:1329` (work waits on a human)

Changing a goal that already has members to rules that need reviewers leaves the host's agent as the only reviewer, and nothing says so.

**How it goes wrong.** 1. A peer-review goal (no roles declared) has the host's agent and four members. 2. The host runs `locust --owner rules bind --goal G --formation review-panel` and confirms. The plan and result print only `Bind "T" (id) to review-panel. Open tasks keep their old rules until revised.` plus shared-file lines. 3. The node signs the binding (and a workspace epoch if shared files are on) and no role record; replay creates the `reviewer` list as [host agent]. 4. Tasks that were already open keep peer-review and still count. Every task opened or revised afterwards, every goal-level finding, and (if shared files are on) every file change from then on needs two approvals from reviewers other than the author. A member's result can get one (the host agent's); the host agent's own result gets none. 5. The four members' agents get no review requests (`pending.to_review` lists only what the caller may review), and a review they try is refused as against the rules. 6. `goal status` prints `Roles: reviewer Harbor (...)` and `2 more reviewers are needed; members you add or invite become reviewers.` Nothing counts until the host runs `role give ... reviewer` for at least two of the existing members; results posted meanwhile can be approved after that, so nothing is lost.

**What the second readers concluded.** Binding rules with a counting role (review-panel) on a goal that already has members leaves the host's agent as the only reviewer, and neither the `rules bind` plan nor its result says so; `goal status` then advises adding or inviting members instead of `role give` for the ones already there. The code follows the plan's work list exactly, so this is a gap in the plan (it covers only `goal create`, `goal add` and `goal invite`), not a departure by the builder, and it also stalls the shared files, which the finding did not mention.

**Smallest fix.** Plan first: docs/roles-and-permissions-plan.md Phase 4 is silent on a rules change that first declares a counting role in a populated goal, and the code matches it. Two sizes of fix. Smallest (CLI only, no new records): in `rules_plan` and `rules_bind` (crates/locust/src/cli/only_you.rs:1241, 1340) add a reviewers line when `roles::counting_role(&formation)` is set and `roles::missing_reviewers` is above zero for the holders in `observed.roles` (the host's agent alone when the list does not exist yet), with the exact `role give` command for each current member; and in `presentation::goal_status` (presentation.rs:607-623) print the `role give` advice instead of "members you add or invite" when the goal has members who do not hold the role. Fuller, and closer to the owner's rule that agents' work never waits for a person: have `rules_bind` in crates/locust-core/src/node/requests/goals.rs sign a `Body::RoleHolders` record for the counting role in the same commit as the binding (as it already does for the workspace epoch), giving it to the current members, shown in the plan with `--no-role` as the way out. That second option changes who becomes a reviewer, so it needs a plan decision in the style of row 18.

**What a person sees.** After switching a running goal to review-panel, the command reports success and says nothing about reviewers. From then on new tasks' results, and any change to the shared files, never count, and the other members' agents are not asked to review anything. The status page says two more reviewers are needed but tells the host to add or invite people, when the cure is to give the reviewer role to two members who are already in the goal. Nothing is lost; work just sits until the host works that out.

Reported by: plan-changes-first-half:2, plan-changes-second-half:1. Who acts: fix session, and the plan.

### `crates/locust-core/src/goal/flow.rs:494` (work waits on a human)

Adding a member asks a departed author's agent for a review request it can never sign, and status shows it as stalled.

**How it goes wrong.** 1. A `peer-review` goal on one computer has Maple (the host's agent) and Juniper. Juniper posts result S on an ordinary (non-stage) task. Juniper's daemon signs the request to Maple at once. Nobody approves S.
2. Juniper runs `goal leave`: it signs `LeaveRequested` and sets the local left flag. Nothing consumes the leave request, so Juniper is still a signed member at the head. Alternatively the host removes Juniper; S stays effective because the removal's `last_accepted` cutoff keeps it.
3. The host adds Cedar. `desired_effects` calls `review_templates(S, head)`; Cedar is a member at the head and is not the author, so a request to Cedar is wanted with runner Juniper. Its effect id is new because `target_slot` is the recipient.
4. `drive_flow` will not sign it, and `next_place` would refuse anyway. `stalled` reports it as `runner_left` (or `runner_not_member` after removal), and `goal status` prints the line.
5. Every later member or role holder who may review S adds one more line; so does each other unapproved result of Juniper's.
6. Cedar still sees S in `to_review`, which is built from state and not from requests, so no work waits. If Cedar or Maple approves S the lines disappear; if they reject it, the lines stay until the host revises the task or Juniper is removed and re-admitted (a left agent cannot rejoin while it is still a signed member).

**What the second readers concluded.** The core is real and still present at HEAD: since 8c086c1 the fold wants a review request to every eligible member at the head, signed by the result's author, so once that author has left or been removed each later `goal add`, join or `role give` adds one unsignable step per unapproved result, and `goal status` on the computer holding that agent lists it as stalled. Two parts of the report are wrong: the line is not permanent (it clears when the result is approved, the task is revised, or the agent is re-admitted), and the proposed fix would leave `Stall::RunnerLeft` and `Stall::RunnerNotMember` with no producer and break a pinned test.

**Smallest fix.** Code should change, in one place: `Verifier::desired_effects` (crates/locust-core/src/goal/flow.rs:494-495). Pass the head to `review_templates` only when the request's runner can still sign by signed facts: the governance key (stage task), or an author who is a member at the head and has no effective `LeaveRequested` naming its current admission. Otherwise pass the result's own anchor, as before this commit. This stops new steps appearing after a departure and keeps Phase 3's report of requests the author never sent.

Do not take the fix as written. Review requests on non-stage results are the only member-run steps (stage and offer steps run under the governance key), so dropping non-member runners in `desired_effects` and left runners in `stalled` leaves `Stall::RunnerNotMember` and `Stall::RunnerLeft` unreachable and fails `goal_status_reports_each_stalled_runner_condition` (node/tests/context_views.rs:882-996), which pins both reasons on a review request.

Add a node test: Juniper posts, then leaves (and, separately, is removed); the host adds Cedar; `goal status` lists no stalled step and Cedar's `to_review` lists the result.

**What a person sees.** On the computer that ran the departed agent, `locust goal status` shows a line like "Step 3f... stalled for Juniper: runner_left" although nothing is actually waiting: the new member can already see and review the result. One more such line appears for each unapproved result of Juniper's every time a member is added or given the reviewing role. The lines go away when someone approves the result; if it was rejected or abandoned they stay until the host revises the task or brings Juniper back.

Reported by: lens-convergence:3. Who acts: fix session.

### `crates/locust-core/src/node/requests/invitations.rs:317` (work waits on a human)

A waiting join cannot be run again without retyping the same `--name`, and a name chosen by mistake cannot be corrected.

**How it goes wrong.** 1. Host's computer is off. The person runs `locust --owner goal join --ticket-file t --agent codex-maple-1a2b3c4d` with no --name. The plan reads "Join ... as codex-maple-1a2b3c4d"; they confirm. The daemon stores a JoinRecord with that name and prints "Joining ... as codex-maple-1a2b3c4d".
2. They re-run with `--name Maple`, or the reverse order (first `--name Maple`, later without it, since `member_name` substitutes the enrolled name). `Node::goal_join` finds the same-ticket pending join, sees `join.name != name` and returns conflict, exit 7: "the pending admission already has a different name". The message does not say which name is waiting and no later view shows it.
3. No way out while waiting: `goal leave` answers unavailable for a joining agent, a different ticket answers "another invitation is being redeemed", and nothing deletes the JoinRecord except admission. When the host returns, the agent is admitted under the first name for good.
4. Corrections to the finding: `level set` works for a joining agent, so the level is not stuck; the name was printed by the plan and the result of the first run; if the ticket was refused and the name differs, the join command gives the name message, but `status` still says the invitation was refused and to ask for a fresh one, and a fresh ticket after a refusal does take a new name.

**What the second readers concluded.** Real, but smaller than claimed: while a join waits for the host, re-running `goal join` with the same ticket and a different name (given or defaulted) is refused with conflict, and no command can change or drop the waiting name. The level can still be changed with `level set`, the person was shown the name before it was stored, and the name being fixed after admission is the plan's own assumption 8, so severity is low.

**Smallest fix.** Code should change, and the plan should gain one sentence saying a waiting join can be re-run with another name. Smallest change, in `Node::goal_join` (crates/locust-core/src/node/requests/invitations.rs:313-341): test `join.refused` first, then on a same-ticket retry write the JoinRecord again with the requested name beside the level write instead of returning conflict. That treats the name the way this branch already treats the level (a re-run applies what the command says, default included); `joins()` re-signs from the stored record on the next exchange. With that, `plan_join` (crates/locust-core/src/node/peers.rs:331-335) must drop `member.name == request.name` from the already-redeemed branch, otherwise a rename that races the host's admission is refused, the joiner is marked refused and stops asking. Add one node test for a same-ticket re-run with a new name and one for the refused order. Making the request's name optional so an omitted --name keeps the stored one is a nicer variant but not needed to close this.

**What a person sees.** While waiting for the host's computer, running the join command again fails with "the pending admission already has a different name" unless the name matches the first run exactly, including when --name was simply left off or added. A name picked or defaulted on the first run cannot be changed before the host admits the agent, and after that the goal keeps it for good unless the host removes and re-invites the agent. Nothing is lost otherwise: the join still goes through and the level can be changed with `level set`.

Reported by: invitations-join:1. Who acts: fix session.

### `crates/locust-core/src/node/requests/goals.rs:168` (work waits on a human)

`rules bind` refuses a formation that lists a role no rule uses, and the refusal does not say which role.

**How it goes wrong.** 1. Create a goal with `--formation independent-attempts`. Replay gives `lead` a list holding the host's agent, and `lead` is a deciding role.
2. Bind custom rules that keep `"roles": {"lead": ...}` but set `decisions.selection` to null. The editor produces exactly this when "Is one result picked?" is set to Nobody: `setPick` never removes the role and the exported text keeps every declared role. Note `--formation-json` takes the JSON text, not a file.
3. `Node::rules_bind` loops over declared roles that have a list. For `lead`, `was_deciding` is true and `is_authority_role(new, "lead")` is false, so it answers `conflict`. Nothing is signed.
4. The person sees `locust: conflict: this role picks or closes in this goal and has one holder; these rules make it a group. Use another role name.` The role name is only in `details`, which human output never prints.
5. Deleting `lead` from the roles map, or renaming it, makes the bind go through. Renaming leaves a junk role list in `goal status` for the life of the goal.

Binding would have been harmless: the new rules name no authority, so `validate_binding` accepts them; lists are untouched; and `deciding` is the union over all bindings, so `lead` stays deciding for give, take and invitations.

Reverse case, correctly refused: bind rules that declare `judge` and name it nowhere, then bind rules where `judge` picks. In between, `judge` is a group to the daemon: `role_give` adds holders and `goal_invite` will put it on a ticket. `plan_join` gives a ticket's role with no kind test, so allowing the switch on "list has one member" would let an outstanding ticket add a second holder, after which nobody decides.

**What the second readers concluded.** `rules bind` wrongly refuses new rules that still declare a previously deciding role (such as `lead`) but name it in no rule, with a message that says the rules "make it a group" and never says which role. The reverse case in the finding (a declared, never-used role later made the picker) is also refused, but that refusal is protective and should stay; the proposed fix for it is unsafe.

**Smallest fix.** Change the code; the plan needs one word.

- In `Node::rules_bind` (crates/locust-core/src/node/requests/goals.rs:158-161), also skip a declared role the new formation names in no rule, for example by adding `.filter(|role| !crate::organization::role_duties(&formation, role).is_empty())`. `role_duties` is already exported.
- Keep the refusal of a never-used listed role becoming deciding. Do not adopt the finding's "exactly one member" rule: outstanding invitations can still carry that role.
- In the CLI's `rules_bind` (crates/locust/src/cli/only_you.rs), when the conflict's details carry `role`, print it through `presentation::safe` so the person is told which role. This closes the gap left by build-notes departure 4.
- Add the declared-but-unused case to `rules_bind_is_refused_when_a_role_would_change_kind`: the bind succeeds and `lead` is still deciding with its holder.
- In docs/roles-and-permissions-plan.md line 2554, change "declares a role that has a list and gives it the other kind" to "names, in a rule, a role that has a list and gives it the other kind".

**What a person sees.** A host who edits their rules so that nobody picks a result, but leaves the old "lead" role in the list, is told no when they apply the change. The message says the new rules turn a role into a group, which is not true, and does not say which role it means. Nothing is lost. Removing the unused role from the rules, or renaming it, makes the change go through; renaming leaves a pointless extra role showing in the goal's status from then on.

Reported by: formations-editor:1. Who acts: fix session.

### `crates/locust-core/src/goal/mod.rs:349` (cost)

Every `pending` read rebuilds the whole verifier and walks every held record once per result waiting for review.

**How it goes wrong.** 1. A goal holds N events. A member's agent has C results in to_review. A result stays a candidate until it is approved, picked, or reviewed by this caller; State.contributions is insert-only (projection.rs:211), so C is not bounded.
2. The agent calls pending, reads the context brief, or is parked in wait and the goal's revision moves (requests/reading.rs:154, 217, 287; context_views.rs:43, 143).
3. views.rs:437-448 loops over the C candidates and calls entry.goal.latest_reviews(subject, ..) for each.
4. Each call (goal/mod.rs:349-372) runs Verifier::new, which clones and refreshes the closure index and walks all N events to index witnesses and decisions (fold.rs:50-76). It then walks all N events again into a BTreeSet of authors (mod.rs:360-364), although Goal::authors() at mod.rs:246 already gives the log keys.
5. For every author it calls latest_review, which runs status(id, None) with an empty memo (fold.rs:1039). That re-judges the review, its subject and the subject's attempt. Those checks do their own scans: current_round over the governance chain (fold.rs:792) and no_prior_offer_answer over all events (fold.rs:772). The same standings are already in self.evaluation.standings from the fold (fold.rs:1179-1183).
6. Total is roughly C x (3 or more passes over N) per answer, per waiting agent, per goal change. Before the commit it was one pass over the logs for all candidates, reading stored standings.

**What the second readers concluded.** pending_work calls Goal::latest_reviews once per result waiting for the caller's review, and each call builds a fresh fold::Verifier (clones the closure index, walks every held event), walks every event again to collect authors, and re-judges the reviews from an empty memo. This commit replaced a single pass per call with about three or more passes per candidate. It is a cost regression on the pending / wait / context path, not a wrong answer.

**Smallest fix.** Code should change; the plan stands. Smallest change: make Goal::latest_reviews (crates/locust-core/src/goal/mod.rs:349) take all wanted subjects and return a map per subject. It builds one fold::Verifier for the call and iterates self.authors() instead of re-walking events. Call it once in pending_work_with_news (crates/locust-core/src/node/views.rs:437) before the candidates loop, and skip it when there are no candidates. That restores one pass per call, shares one memo, and keeps Verifier::latest_review as the only implementation of "latest", which is what plan line 2472 asks for. Recording the latest review per (subject, member) in State during project would remove the read-side verifier entirely, but it is a larger change and not needed to close this.

**What a person sees.** Nothing in a small goal, and never a wrong answer. In a goal that has run a long time, an agent with many results still waiting for its review gets slower replies from pending, wait and the context brief. The delay grows with the size of the goal times the size of that backlog, and other calls to the same daemon queue behind it.

Reported by: chain-snapshot:2, views-api:3. Who acts: phase 6 (R5).

### `crates/locust-core/src/goal/fold.rs:1091` (cost)

The latest-review rule scans every review of a result once per review, so one member's flood of reviews slows every computer in the goal.

**How it goes wrong.** 1. A goal runs under any rule with a `Reviews` or `Check` part; the default `peer-review` is `Any{Reviews{members,1,exclude_author}, Contribution{only_member}}` (presets.rs:179-191), so the reviews part runs first.
2. One admitted member signs W `ReviewRecorded` records naming one effective result (its own or anyone's). Each has a new seq so a new id; no rule refuses a repeat, the local path signs without a dedupe (requests/tasks.rs:226-251), and `screen` keeps anything from an ever-admitted author (screen.rs:23-43). Self-reviews under peer-review are excluded but still held.
3. Every fold, `Verifier::new` indexes all W by subject whatever their standing (fold.rs:51-59).
4. `project` calls `approval(id, None, None)` for the result (projection.rs:209 or 236) and `desired_effects` calls it again (flow.rs:427); a workspace proposal gets one call (projection.rs:262). Each reaches `predicate` with `allowed == None`.
5. `predicate` walks the W records (fold.rs:1085) and for each calls `latest_review` (1091-1101), which walks all W again doing a hash lookup, an author compare and, for same-author records, a memoised-standing BTreeMap lookup (1026-1041). There is no early exit once `count` is met. W = 5,000 is 25 million inner steps per call, twice per fold (arithmetic only; not run).
6. `Goal::apply` refolds from scratch on every landed batch (mod.rs:97-134, commit.rs:338) and on every trial before signing (access.rs:246-249), on every member's computer.
7. Removal changes nothing: history is append-only and the index is rebuilt from all held records.
Correction to the report's detail only: the square is over all records naming the result, by any author; one author's flood is simply the cheapest way to get there.

**What the second readers concluded.** When no evidence is pinned, `Verifier::predicate` calls `Verifier::latest_review` once per held review, declaration or attestation of a result, and each call re-walks every such record of that result, so the work per fold grows with the square of the number of records naming one result. This is new in 8c086c1 (the loop was one pass before), nothing caps or dedupes repeated reviews, and the same per-record call sits in `stage_ready` (flow.rs:202-208) for a stage with a `review` prerequisite; it is unchanged at HEAD 5b3fe29.

**Smallest fix.** Make `Verifier::latest_review` (fold.rs:1020) answer from a table built once per fold per (subject, check): one pass over `witnesses[subject]` keeping, per author, the effective record with the highest (seq, id), cached in a `RefCell` map on the `Verifier` beside `resolved` and `proofs`. That closes all three callers at once: `predicate`, `stage_ready` (flow.rs:205, which a fix placed only inside `predicate` would miss) and `Goal::latest_reviews` (mod.rs:349-372). Caching is sound because the unpinned path is reached only from `project`, `desired_effects` and `Goal::latest_reviews`, after standings are final (pinned callers at fold.rs:980, flow.rs:329 and projection.rs:363 pass `Some`). For the test, count scans with a `#[cfg(test)]` counter as `cutoff_traversals` does (chain.rs:43, tests.rs:2254), not wall-clock time: the repo's performance workload is deliberately ungated (tests/organization_performance.rs:1). The plan needs no change; it fixes the meaning of latest, not how it is computed.

**What a person sees.** Nothing looks wrong on screen. But once one member, or one agent stuck in a loop, has posted a few thousand approve/reject reviews of the same result, every new record in that goal takes seconds to go through on everyone's computer, including each person's own actions, and it stays that slow after the member is removed.

Reported by: lens-hostile-member:3. Who acts: fix session.

### `crates/locust-core/src/goal/chain.rs:268` (cost)

Role lists are copied twice per host record on every replay, and one of the copies is never read.

**How it goes wrong.** 1. A review-panel goal: `selected_role` (crates/locust/src/cli/roles.rs:97-108) gives each added member the counting role unless `--no-role` is passed, so M admissions give M governance records and a reviewer list of up to M keys.
2. Every `Goal::refresh` calls `Chain::build` (goal/mod.rs:117). Each governance record runs `chain.snapshots.insert(event.id(), snapshot.clone())` (chain.rs:262) and then `chain.state.roles = snapshot.roles.clone()` (chain.rs:268).
3. The snapshot copy is needed: fold.rs:126 and :714 and flow.rs:66 read roles at an anchor. The state copy is overwritten on the next record and nothing in the loop reads it.
4. Cost per rebuild: about 16*M*M bytes kept in snapshots and another 16*M*M allocated and dropped. At 1,000 reviewers that is 16 MB kept and 16 MB wasted.
5. The same Snapshot already clones `members: BTreeMap<PublicKey, EventId>` per record (64 bytes per entry, about 32 MB at 1,000, plus tree nodes). The role copies are flat Vec copies added to a cost that was already quadratic.
6. The 5,000-reviewer figures are outside the envelope: a frontier lists at most 4,096 authors (limits.rs:35, sync.rs:404), and members alone already cost about 800 MB per copy there before this commit.
7. The resolve copies (rules.rs:161, fold.rs:113) are not new. Before the commit the same line was `roles: binding.roles.clone()` and `Resolved.binding` carried a second copy; the commit removed that second copy.
8. In a small goal the cost is a few thousand small allocations per refold, which is noise beside the full re-evaluation of history in `fold::evaluate`.

**What the second readers concluded.** Chain::build does deep-copy the role map twice per governance record on every refold, and the copy into chain.state.roles is dead for every record but the last. It is a waste, not a correctness bug: it adds a constant factor to the per-record members-map copy that predates this commit, so it is low severity and closes with a one-line move.

**Smallest fix.** The code should change; the plan stays as it is. In `Chain::build` (crates/locust-core/src/goal/chain.rs), delete line 268 from inside the loop and assign once after it: `chain.state.roles = snapshot.roles;`. The snapshot is not used after the loop, so this is a move.

Behaviour is identical on every path: the two early returns leave both maps empty, the BrokenChain break leaves the snapshot at the last applied record, and excluded or pending records do not change `snapshot.roles`.

The proposed Arc in Snapshot and EffectiveRules is not needed to close this. `EffectiveRules` is public and Serialize (rules.rs:12-21), so an Arc there ripples outward. If goals with a thousand or more members become a target, sharing the per-record members and roles maps belongs in one separate change, since members is the larger pre-existing cost.

**What a person sees.** Nothing in any goal of ordinary size. In a goal where a thousand or more members share one role, each step an agent signs and each batch received is slightly slower and uses more memory than it needs to. Such a goal was already slow for the same reason before this change, because the member list is copied the same way.

Reported by: chain-snapshot:1. Who acts: fix session.

### `crates/locust-core/src/goal/fold.rs:112` (cost)

Replay resolves the rules, and validates the whole formation again, once per task and host record pair, though nothing in that work reads the host record.

**How it goes wrong.** 200 top-level finished tasks, about 55 host records, honest daemons anchoring at the head.

Before the commit, per replay:
- about 202 misses in Verifier::resolve (one per context)
- plus about 200 uncached resolve_binding calls from task_binding, one per task-opening record
- so about 400 formation validations, not 202

After the commit, per replay:
- a floor of about 400 misses: each finished task resolves once at its records' anchor and once more at the head, because projection.rs:416 resolves every selected task at the head only to read its binding
- plus one miss for each (task, host record) pair where a member joined while the task was in flight, at most 55 times the number of tasks open at each join
- so about 400 to 650 misses and 600 to 850 validations, roughly 1.5 to 2 times the validations before; the reviewer's upper figure of 800 misses needs about 4 tasks in flight at every join

A subtask at depth D still pays D+1 validations per miss, as before, now multiplied by its anchors.

Size of one validation: the only recorded baseline (research/performance-cost-pass.md:43, measured 2026-10-04) is 20.4 ms to replay 512 tasks, about 40 microseconds per task with two validations in it, assuming the 2026-10-04 code had the same two calls. One extra resolution therefore costs at most about 20 microseconds with a default formation. For 200 tasks that is roughly 8 ms per replay becoming at most 12 to 17 ms.

The existing measurement cannot show this: crates/locust-core/tests/organization_performance.rs:51-57 anchors every record at one host record that is also the head.

**What the second readers concluded.** Commit 8c086c1 changed the per-replay rules cache in Verifier::resolve from one entry per context to one per (context, anchor), although the anchor only supplies the role-holder map and only-member value that are copied into the result; every extra anchor reruns the full resolution, including one formation re-validation per task level. Two parts of the report need correcting: the deep copy on a cache hit was already there before this commit, and the cost does not grow with member count; it is a constant factor bounded by the number of records. Nothing was measured; all counts are by reading.

**Smallest fix.** In Verifier::resolve (crates/locust-core/src/goal/fold.rs:112), key the `resolved` cache by Context again and run rules::resolve once per context per replay. On each call, look up chain.snapshot(&anchor) first, as now, and set effective.roles and effective.only_member on the returned copy from that snapshot.

This gives the same results: rules::resolve, resolve_binding and delegation::inherit never read the two values, and none of their errors depend on them. The roles and only_member parameters of rules::resolve and resolve_binding can then be dropped, with the callers filling the two fields in.

Sharing the role map instead of copying it is a separate, older cost and is not needed for this fix.

**What a person sees.** Nothing you would notice today. In a goal with a few hundred finished tasks, once anyone has joined after those tasks were done, every command re-checks the rules about one and a half to two times as often as before. By reading, that is a few thousandths of a second per command at 200 tasks, and it scales with the number of tasks, not with the number of members.

Reported by: gap-replay-cost-per-anchor:2. Who acts: fix session.

### `crates/locust-core/src/goal/flow.rs:427` (cost)

Each replay works out the wanted review requests by scanning all held records per reviewer, and the new default asks every other member to review every result.

**How it goes wrong.** All numbers are by reading; nothing was run or measured. Default (peer-review) goal, 50 members, 199 earlier results each with its 49 requests signed, so S is about 9,750 step records (the finding's "near 5,000" matches about 100 results, not 200). A member posts result 200. Its daemon replays the goal 50 times: once for the post, then once per signed request, because drive_flow signs one request per loop turn and each land_once is a full fold. In every replay the new result wants 49 requests, and each insert walks history.events from the start (arrival order, so the new result's records are at the tail) hashing every held step record: about 49 x 9,750 = 478,000 hashes per replay, about 24 million for the post (12 million at S = 5,000, as the finding said). Every other result still waiting (not approved, including rejected ones, on an open task's current round) adds 49 x its position among step records per replay, about 12 million more per post each at mid-history. If one identity hash (postcard encode into a fresh Vec plus a blake3 derive-key hash of roughly 200 bytes) costs around half a microsecond, which is a guess, that is around ten seconds for the new result alone. At 5 members and 50 results the same sum is a few thousand hashes, so it only bites at tens of members and hundreds of results; it grows as members cubed times results. A goal with one member wants no requests and is unaffected. Before the commit: default goal 0 hashes; a goal that named peer-review paid R x (M-1) x position per replay for all R results, approved or not, so for those goals the commit made this cheaper. The new decision scan is U x E cheap comparisons per replay with no hashing, small next to the hashing.

**What the second readers concluded.** The cost is real, but the per-request scan of all held records in desired_effects' insert closure is not new: it is byte-identical before the commit, and this commit narrowed it from every published result to only results that do not yet count. What 8c086c1 added is (a) a goal created with no formation now follows peer-review, so a default goal with two or more members goes from wanting zero requests to M-1 requests and about M full replays per post, and (b) a new filter-and-sort over all held records for each waiting result.

**Smallest fix.** In desired_effects (crates/locust-core/src/goal/flow.rs:370) take the projection's map of signed steps as an argument and replace the scan inside the insert closure (flow.rs:377) with `if materialized.contains_key(&id) { return; }`; call it at fold.rs:1185 as `verifier.desired_effects(&evaluation.state.effects)`. That map is built on the line before (fold.rs:1184, projection.rs:86-98) from exactly the effective step records, keyed by the same Effect::id, so the test is equivalent and each held step is hashed once per replay instead of once per wanted request. Second, smaller step in the same function: read evaluation.state.decisions for the context's Selection and Closure keys (already grouped and sorted the same way at projection.rs:343-356) instead of the filter and sort at flow.rs:434-455. This does not remove the M-1 replays per post (drive_flow, node/flow.rs:11-50, unchanged by this commit) or the rebuild of M-1 templates for every held step record on every replay (fold.rs:646-647 into review_templates, flow.rs:227-284, same shape before the commit); those keep a replay at about S x M work and are separate, older costs that the new default also exposes.

**What a person sees.** In a goal with dozens of members and a few hundred results, posting a result can keep the poster's computer busy for several seconds or more before the result shows up for review, and it gets slower as more results sit unapproved. Small goals and goals with one member are not affected.

Reported by: gap-replay-cost-per-anchor:3. Who acts: fix session.

### `crates/locust-core/src/goal/projection.rs:416` (cost)

The projection resolves every picked task again at the head on each replay, only to read its task binding.

**How it goes wrong.** A goal with 200 picked tasks, then one host record (an admission, removal, role change, rules binding, task revision, workspace epoch or publication set all move the head).

- **Before the commit:** about 400 formation validations per replay. Each task cost one uncached `resolve_binding` in `task_binding` and one first `resolve` of its context; the projection lookup was a hit.
- **After the commit:** the same 400, plus one per extra distinct anchor among a task's records during judgement, plus up to 200 more from projection. That is roughly 600, so projection adds about half again to this component.
- **Per miss:** one `valid_definition` call (serialise the formation to JSON, strict parse, validate, normalise, hash, build the explanation, compare), plus one more per parent-task level, because the parent recursion in `resolve_binding` is uncached.
- **How often:** `project` runs once per full fold, which is every `Goal::apply` (each committed batch, each trial of a candidate record) and every `Goal::load`.
- **Shared-files lookup at projection.rs:499:** one resolution per replay, so negligible.
- **Why the existing measurement misses it:** `tests/organization_performance.rs` anchors every record at the same head, so there the lookup is a hit.

The miss count is exact only for task rounds with no record anchored at the current head; in a long goal that is nearly all of them.

**What the second readers concluded.** Confirmed by reading (nothing measured): since 8c086c1, `project` resolves every picked task round again at the chain head only to read its task binding, which is the same at every anchor. Once any host record lands after a task's last record, that lookup is a cache miss on every replay and re-runs the full formation validation, where before the commit it was always a hit. The growth is linear, one extra resolution per picked task round, and the fix is one line.

**Smallest fix.** In `project` (crates/locust-core/src/goal/projection.rs:415-418), resolve at the picked decision's own anchor instead of the head:

`v.resolve(key.context, v.history.get(&last.id).expect("decision exists").header().anchor.unwrap())`

This is a guaranteed cache hit, because `Verifier::decision` resolved exactly that pair to make the decision effective. `.task` and the success of the resolve do not depend on the anchor, so behaviour is unchanged.

Do not read the binding from `out.state.tasks` instead: a pick's round is required under the decision's own proof, so it may be absent from the ordinary projection.

Optionally do the same at projection.rs:499 with the epoch record's anchor, which `workspace_epoch` already caches.

The plan (docs/roles-and-permissions-plan.md:2445-2446) says `project` passes the head, so that sentence needs the matching amendment.

**What a person sees.** Nothing looks different. On a long-running goal with hundreds of picked tasks, once someone joins or a role changes, each new record takes a little longer to process than it needs to. How much longer has not been measured.

Reported by: gap-replay-cost-per-anchor:4. Who acts: fix session.

### `crates/locust/src/cli/presentation.rs:81` (untrue text)

The terminal prints an approvals-plus-check rule without the grouping the website shows, so it reads as if one approval were enough.

**How it goes wrong.** 1. On the formations page tick "It has approvals" and "A check is reported as passed"; countsRule (model/line.ts:170-190) writes all[ any[ reviews(members, 1, exclude_author), contribution by only_member ], check "tests" ]. The page shows: When (it has 1 approval, not the author's, or the goal's only member posts it) and the check "tests" is reported as passed.
2. Save it as team.json and run `locust --owner goal create --title T --formation-json "$(cat team.json)"` (the route docs/guide/formation-authoring.md:100 gives). Core validation accepts the rule (validation.rs has no shape limit on only_member).
3. create_plan (only_you.rs:571-579) prints, before "Proceed? [y/N]" (confirm.rs:102): A result counts when it has 1 approval, not the author's, or the goal's only member posts it and the check "tests" is reported as passed.
4. Replay requires the check in every case (fold.rs predicate, All arm 1061-1071), so an approved result without the check does not count.
Correction to the reported scenario: `rules bind` prints this sentence only when the goal's shared files are on (only_you.rs:1260-1303, inside `if let Some(workspace)`); with no shared files its plan prints no counts sentence at all.

**What the second readers concluded.** counts_clause in crates/locust/src/cli/presentation.rs prints an `All` whose part is an `Any` without brackets, so the editor's approvals-plus-check rule reads in the terminal as "1 approval, or (only member posts it and the check passes)" while the enforced rule is "(1 approval or only member posts it) and the check passes". The same commit added the brackets to the site's countsClause and not to the Rust copy, breaking the plan's "word for word" pairing of the two.

**Smallest fix.** In `counts_clause` (crates/locust/src/cli/presentation.rs), change the `CompletionRule::All` arm to wrap any part that is a `CompletionRule::Any` in parentheses before joining with " and ", exactly as `countsClause` does at sites/locust.farm/src/lib/formation-editor/model/words.ts:96-99. Add a unit test beside the one at presentation.rs:1102 that feeds All[Any[Reviews{Members,1,exclude_author:true}, Contribution{OnlyMember}], Check{"tests"}] to `counts_when` and expects `A result counts when (it has 1 approval, not the author's, or the goal's only member posts it) and the check "tests" is reported as passed.` The code should change, not the plan. Adjacent, same function, optional: the Rust only-member arm matches an `Any` of any length while the site's withoutOnlyMember needs exactly two parts, so the two also differ for three-part rules, and a one-part Any[only_member] prints "A result counts when , or the goal's only member posts it."; limiting the arm to exactly two parts would close that too.

**What a person sees.** A host who built "approvals and a check" on the website and starts a goal with it is asked to say yes to a sentence that reads as though one approval is enough, with the check only mattering for a goal of one. In fact every result also needs the check reported as passed, so approved results stay uncounted until someone reports the check. The website shows the same rule with brackets that make this clear; the terminal does not. Nothing is lost, and the real rule is the stricter one.

Reported by: formations-editor:3, plan-changes-first-half:3. Who acts: phase 6 (R5).

### `crates/locust-core/src/node/requests/goals.rs:170` (untrue text)

Role refusals never show the person which role, or the roles the goal has; the name is only in a JSON details field.

**How it goes wrong.** 1. Host has a `directed` goal (roles `lead` deciding, `reviewer` group, both held by the host's agent). 2. `locust --owner rules bind --goal G --formation-json F`, F declaring lead/reviewer/tester and naming `lead` only in a review selector. The CLI's rules_plan (only_you.rs:1241-1338) makes no kind check, prints the plan and asks `Proceed?`; the person answers y. 3. The daemon's rules_bind (goals.rs:162-176) returns conflict with details {"role":"lead"}; the terminal shows `locust: conflict: this role picks or closes in this goal and has one holder; these rules make it a group. Use another role name.` and exits 7. No role is named, and human `goal status` does not mark which role is deciding either (presentation.rs:586-593), so the person can only find it with `--json` or by reading the formation. 4. `locust --owner role give --goal G --member Juniper reviewers`: roles::run (roles.rs:226) reads `view.roles`, falls back to an empty `expected` and sends anyway; the daemon (goals.rs:310-312) answers invalid with details {role, roles}; the terminal shows `locust: invalid: this goal has no such role` with no list. Same pattern at invitations.rs:180-191 for `goal invite --role`.

**What the second readers concluded.** In human mode the four role refusals added by 8c086c1 (kind change on rules bind, unknown role on role give/take, unknown or deciding role on goal invite) print only a generic sentence; the role name and the goal's role list exist only in `details`, which the CLI prints only with `--json`. Build-note departure 4 declares the generic daemon message, but its stated counterpart ("CLI output sanitizes names") does not exist for refusals, so the plan's "ROLE picks or closes..." and "this goal has no role ROLE; roles here: ..." never reach the terminal in any form.

**Smallest fix.** Code should change, in one place: in `print_failure` (crates/locust/src/cli/mod.rs:156-165), human branch, when `details_json` carries a string `role` and optionally an array `roles`, append them through `presentation::safe`, for example `... Use another role name. Role: lead` and `this goal has no such role: reviewers. Roles here: lead, reviewer`. That covers all four daemon sites (goals.rs:174, goals.rs:312, invitations.rs:183, invitations.rs:191) and keeps departure 4's generic daemon message. Add a CLI test in crates/locust/tests/cli.rs that pins the role name in stderr. Optional nicety, not required to close it: have `rules_plan` compare the new formation's roles against `observed.deciding` so the refusal comes before `Proceed?`. Then reword departure 4 in research/v2-phase-r4-build-notes-2026-10-06.md to say the CLI names the role from details, and note the changed wording against plan lines 2555-2568 and companion rows 303/306.

**What a person sees.** After saying yes to a rules change, the person is told "this role" clashes with the goal and to use another role name, but not which of their roles it is; goal status does not show it either. After mistyping a role in role give or goal invite they are told the goal has no such role, with no list of the roles it does have. Nothing is lost or changed; they have to rerun with --json or read the rules file to find the answer.

Reported by: cli-texts:3, views-api:2, plan-changes-first-half:4, plan-changes-second-half:2, node-roles-rules-bind:1, lens-earlier-phases:2. Who acts: phase 6 (R5).

### `crates/locust/src/cli/presentation.rs:618` (untrue text)

`goal status` tells people who do not host the goal, and agents, that members "you add or invite" become reviewers.

**How it goes wrong.** 1. Ana hosts a `review-panel` goal (completion = 2 approvals by role `reviewer`, author excluded; crates/locust-proto/src/organization/presets.rs:68-78) with two reviewer holders (her agent plus one member admitted with the default role).
2. On the other member's computer, the owner (or the member agent with its own credential) runs `locust goal status --goal G`. `goal.status`, `event.show` and `blob.get` are all read-only Agent-audience operations gated only by `readable` (api.rs operations table; requests/goals.rs:370-371, reading.rs:139, content.rs:78), so `roles::current_formation` returns the review-panel formation and `view.roles` carries the two holders.
3. `presentation::goal_status` computes missing = 2 + 1 - 2 = 1 and prints `1 more reviewer is needed; members you add or invite become reviewers.` with no look at `view.hosted_here`.
4. That reader cannot do either: `goal add` there fails with "this goal is hosted on another computer; request an invitation from its host" (cli/only_you.rs add_plan), and `goal invite` is a Host operation refused by `Node::host` with "this goal is hosted on another computer; its host decides" (node/access.rs:127-141, requests/invitations.rs:171).
Note: an agent on the hosting computer also reads "you", but that matches the existing `Host: you` line, which is likewise printed to agents there; the clear error is the non-host computer.

**What the second readers concluded.** `goal status` prints "N more reviewer(s) needed; members you add or invite become reviewers." on every computer and for every caller, including computers that do not host the goal, where neither the owner nor an agent can add or invite anyone. The count is correct and planned; only the second half of the sentence is wrong off the host.

**Smallest fix.** Code, not plan. In `goal_status` (crates/locust/src/cli/presentation.rs, the `missing > 0` branch) choose the tail by `view.hosted_here`, as `host_line` already does: keep "members you add or invite become {role}s." when hosted here, otherwise print "the host adds or invites them." (or just the count). No caller parameter is needed; `hosted_here` alone matches the convention `Host: you` already uses. Extend `goal_status_names_the_host_members_and_roles_and_counts_missing_reviewers` (it already renders with hosted_here = false) to assert the non-host wording and add one hosted_here = true assertion. The create-plan copy in `roles::initial_reviewers` is host-only and stays as is.

**What a person sees.** Someone who joined another person's goal checks its status and is told that members they add or invite will become reviewers. They cannot add or invite anyone to that goal; if they try, Locust refuses and tells them the goal is hosted on another computer. Nothing is lost, but the status line points them at a step only the host can take.

Reported by: cli-texts:6, invitations-join:4, plan-changes-second-half:5, lens-earlier-phases:3. Who acts: phase 6 (R5).

### `crates/locust/src/cli/presentation.rs:188` (untrue text)

While an agent is still joining, status shows the ticket's host name as the host with nothing marking it as the ticket's word.

**How it goes wrong.** 1. A person joins with a ticket for a goal on another computer (`goal_join`, node/requests/invitations.rs:397-421): a `JoinRecord` is stored with `host_name` copied from the ticket; no event is held, so `state().host` and `state().governance` are `None` (the K1 test at node/tests/invitations.rs:497-520 pins this state for a forged ticket, where the first record is then refused at replica.rs:199-206 and the agent stays joining).
2. The person runs `locust --owner goal status --goal <identifier>`. `readable` lets the owner through (access.rs:77-93; the joining agent's own call is refused as unavailable at access.rs:88-91, so only the person sees this). `goal_status` sets `host: None` and `host_name: Self::host_name(entry)` (goals.rs:394-396), which falls back to `entry.local.joins.values().next().host_name` (goals.rs:242-256, commented as "the ticket's claim").
3. `host_line` (cli/presentation.rs:184-199) prints `Host: Harbor · another computer`. The rest of that output is `Title unavailable (ID)`, the level line and the peer note: no line says the agent is joining, and the ticket's title is not shown, so the host name is the one ticket-sourced value printed as fact.
4. With a forged ticket (real goal identifier, forger's key and endpoint, signed by the forger's key; `Invitation::verify` checks only against the key the ticket names, invite.rs:272-284) the name is whatever passes `is_member_name` (event.rs:1011-1016: non-empty, 64 bytes, trimmed, no control characters). `Ana's Harbor` works, and so does `Harbor (51c2e9aa)`, which prints exactly like a confirmed host. The line stays for as long as the join record does, including after a refusal.

**What the second readers concluded.** While an agent is still joining (or its invitation was refused) and no record of the goal is held, the owner's `goal status` prints the host name written in the ticket as `Host: NAME · another computer`, with nothing saying it is only the ticket's word; before this commit that case printed `Host: on another computer`. Because a host name may be any text up to 64 bytes, a forged ticket can even name the host `Harbor (51c2e9aa)` and get a line byte-identical to a confirmed host.

**Smallest fix.** In `host_line` (crates/locust/src/cli/presentation.rs:184), when `view.host` is `None` and the goal is not hosted here, stop printing `view.host_name` in the confirmed form: print it as a claim, for example `Host: on another computer · the ticket names it Harbor; not confirmed until admission arrives`, and add a rendering test for `host: None, host_name: Some(..)`. Fix the stale doc comment above `host_line`, which still says it prints nothing before the first record is held. Add one clause to the plan at docs/roles-and-permissions-plan.md:2387-2389 saying status words the ticket's name as unconfirmed. `Node::host_name` and the `GoalStatus.host_name` field can stay as the plan has them. The uncommitted working tree already rewords this line to `Host: on another computer · NAME` but keeps the same fallback, so the fix belongs there too.

**What a person sees.** While waiting to be let into a goal, `goal status` shows a host name as if it were known, though it is only what the invitation said. With an honest invitation the name is right. With a forged one the person keeps seeing whatever name the forger picked, possibly dressed up to look confirmed, and nothing on that screen says the agent is still waiting.

Reported by: invitations-join:5. Who acts: phase 6 (R5).

### `crates/locust/src/cli/only_you.rs:957` (untrue text)

Adding or joining an agent that is already a member, with a new name or role, says it happened under that name and role; nothing changed.

**How it goes wrong.** 1. Host runs a review-panel goal G and adds `codex-maple-1a2b3c4d` with `goal add --goal G --agent codex-maple-1a2b3c4d --name Maple --no-role` (the P4 mockup's own flow). Goal status shows "Member: Maple (key8)" with no role.
2. Later the person runs `locust --owner goal add --goal G --agent codex-maple-1a2b3c4d --name Oak` (or with `--role reviewer`). `add_plan` finds `joined == true` but still writes the human plan `Add codex-maple-1a2b3c4d to "G" (id) as Oak, a reviewer, at level auto. ...`; only the JSON review carries `already_member: true`. (The ", a reviewer" words need `--role` or a goal whose rules have a counting role, i.e. review-panel; in a default peer-review goal the plan reads "as Oak, at level auto.")
3. After confirming, `goal_add` returns before any daemon call with `Oak is already a member of "G" · auto.` The name comes from `plan.review["name"]`, which is the typed `--name`.
4. Join variant: `locust --owner goal join --ticket-file T --agent codex-maple-1a2b3c4d --name Oak` with any valid ticket for G (also the agent-caller form without --owner). The plan says `Join "G" (id) as Oak. ... Joins as a reviewer.` (if the ticket carries a role); the daemon's already-member branch returns `Joined { membership: Member, level: <existing> }` with `Tx::none()` and no name check; the CLI prints `Oak joined "G" · auto.`
5. `goal status` still prints `Member: Maple (key8)` with no role; `role give --member Oak reviewer` finds no such member; in a review-panel goal the missing-reviewers line is unchanged.

**What the second readers concluded.** When an agent is already a member, `goal add` still shows the plan "Add AGENT to "G" (...) as NAME[, a ROLE], at level L" built from the typed flags and then answers "NAME is already a member of ..." with the typed name; `goal join` answers "NAME joined "G" · level" with the typed name. The daemon changes nothing in either case, so the name (and any role or level in the plan) is untrue: the signed name stays what it was and no role is given.

**Smallest fix.** Code, CLI only (the daemon's read-only repeat join is intended and pinned by `joining_again_keeps_the_level_the_person_chose`; do not add a daemon name refusal, since a plain retry without `--name` would then conflict for an agent that joined under a custom name).
- `add_plan` / `goal_add` in crates/locust/src/cli/only_you.rs: when `joined` is true, answer before building the plan (the same place the `hosted_here` refusal returns), taking the name from `goal_status.members` (`MemberView.name` for `agent`) instead of `--name`, e.g. `codex-maple-1a2b3c4d is already in "G" as Maple · auto.`; when `--name` was typed and differs add `A name cannot change.`; when `--role` was typed (or the default counting role is not held) add the `locust --owner role give --goal G --member KEY8 ROLE` line. No Proceed for a no-op.
- `goal_join` in the same file: read the agent's standing before the call (`join_plan` already computes `standing`; do it for both callers) and, when it was already `Member`, print the same already-in sentence with the signed name from goal status instead of `{typed name} joined`. At minimum print the signed name from goal status rather than the typed one.
- Add a CLI test in crates/locust/tests/cli.rs for both already-member answers; none exists today.

**What a person sees.** Someone who tries to rename an agent already in a goal, or to make it a reviewer by adding or joining it again, is asked to confirm "Add ... as Oak, a reviewer" and then told "Oak is already a member" or "Oak joined". Nothing was changed: the goal still lists the agent as Maple with no role, commands that name "Oak" find nobody, and a goal that was waiting for reviewers keeps waiting. Nothing is lost; the person is misled until they look at goal status.

Reported by: cli-texts:7, invitations-join:3, views-api:5, plan-changes-second-half:6, lens-earlier-phases:4. Who acts: fix session.

### `crates/locust-core/src/goal/chain.rs:575` (untrue text)

A formation that names a participant who is not a member is refused with a sentence about a role's holders.

**How it goes wrong.** 1. A host writes formation JSON with no `roles` and with `decisions.selection`, `decisions.finish` or `workspace.integrator` set to `{"kind":"participant","key":"<64 hex>"}`, where the key is an agent not yet in the goal (or one that was removed). 2. They run `locust --owner goal create --title T --formation-json '<json>'` or `locust --owner rules bind --goal G --formation-json '<json>'`. (`--formation f.json` would stop earlier with "unknown formation f.json; use formation examples", since `--formation` takes a bundled name only.) 3. The CLI's `chosen_formation` only parses the JSON; the daemon's `checked_definition` runs `organization::inspect`, which checks only that the key is 64 hex characters. 4. `goal_create` signs Genesis, the host's admission and RulesBound with `sign_at`; `rules_bind` signs with `self.author`. Neither runs a trial replay or a membership check first. 5. On commit, `advance` replays; `validate_binding`'s Participant arm fails because the key is not in `snapshot.members`, and the shared exclusion text is returned. 6. `advance` turns that into `ApiError(Conflict, reason)`, the commit is rolled back, and the CLI prints `conflict: a role that picks or closes must have exactly one holder`.

**What the second readers concluded.** Rules that name a deciding `participant` key that is not a current member of the goal are refused at signing with the sentence written for roles, "a role that picks or closes must have exactly one holder", even when the rules declare no role. The only thing wrong in the report is the flag: hand-written rules go in through `--formation-json`, not `--formation f.json`.

**Smallest fix.** Code and companion both change. In `validate_binding` (crates/locust-core/src/goal/chain.rs), pick the exclusion text by which arm failed: keep "a role that picks or closes must have exactly one holder" for `Authority::Role`, and return a separate reason for `Authority::Participant`, for example "the participant that picks or closes must be a member of the goal". Add one goal test beside `rules_bound_fills_unheld_declared_roles_with_the_host_and_requires_one_holder_per_authority` that binds rules naming a non-member participant and pins the new text. Amend docs/roles-and-permissions-plan-details.md line 235 (and the exit-criterion row at line 298) to name both sentences.

**What a person sees.** A host who names a specific agent by key as the one who picks, closes or accepts files, before that agent is in the goal, is told "conflict: a role that picks or closes must have exactly one holder". Their rules have no role in them, so the message points at nothing they wrote, and it never says the real cause: the agent they named has to be added to the goal first. Nothing is lost or saved wrongly; the goal or rules change simply does not happen and the person has to guess why.

Reported by: chain-snapshot:4, node-roles-rules-bind:3, lens-earlier-phases:6. Who acts: fix session.

### `crates/locust/src/cli/only_you.rs:1316` (untrue text)

The `rules bind` plan counts file changes "to propose again" that need no new proposal, and the host's agent cannot propose first files again.

**How it goes wrong.** (a) Epoch E, head H0. Members A and B each publish a change on H0. A's is integrated (head R1). B's proposal Pb is now stale; B runs `workspace compose --source Pb` (or updates and proposes afresh), and that successor is integrated (head R2). Pb still has no revision naming it, and its round is E. The host runs `locust --owner rules bind --formation peer-review --plan`: `rules_plan` counts Pb and prints "1 file changes that have not landed must be proposed again by their authors." Nothing needs proposing again. Every lost race in the epoch's life adds one more, and so does any proposal a member signs that replay excludes or leaves pending, because `workspace_proposals` lists held records whatever their standing. (b) The host runs `workspace init --root ... --publish` (epoch E1, unseeded, seed proposal P by the host's agent, parent none) and, before `workspace integrate --expected-empty`, runs `rules bind`. The plan prints the same "1 file changes ... by their authors" line. The daemon signs epoch E2 with checkpoint Unseeded. P cannot land in E2 (`integrate` answers "proposal must be composed against the exact expected epoch and head"; `compose` needs an accepted revision and refuses seed sources). The agent cannot post a parentless change (denied without the owner's act) and cannot have a folder to propose from, since registering or connecting one needs an accepted tree. The only way forward is the person running `workspace init` again, which accepts an existing ready unseeded epoch.

**What the second readers concluded.** The `rules bind` plan (and its result, which repeats the line) counts every held proposal of the current epoch that has no revision of its own, so it includes losers of a race whose work already landed through a successor, and proposals that replay excludes. When the only stranded change is the host's not-yet-accepted first files, the line tells the host that the "author" must propose again, but only the person re-running `workspace init` can do that. The code follows the plan's own wording (plan line 2648-2650), so this is a flaw in the plan that the code inherited, not a builder departure.

**Smallest fix.** Change both the plan sentence (docs/roles-and-permissions-plan.md:2648-2650) and `rules_plan` in crates/locust/src/cli/only_you.rs (filter at 1309-1314, text at 1316). Count only proposals the rules change actually strands: `!proposal.stale && proposal.integrated_as.is_empty()` with effective standing (equivalently status AwaitingEvidence, Ready, or a non-stale ContentUnavailable). `stale` already means "another epoch or a parent that is not the head", so superseded and excluded proposals drop out. When a counted proposal has `parent == None` (the tree has no files yet), print instead that the first files have not landed and the host shares them again with `locust --owner workspace init`. While there, print "1 file change ... its author" for a count of one. A member can still post many live changes on the head, but then the number is true. Add a CLI test with a stale and an excluded proposal in the `WorkspaceProposals` answer (the existing test at crates/locust/tests/cli.rs:2968 returns an empty list, so nothing pins the count today).

**What a person sees.** Before changing a goal's rules, the host is told that some number of file changes "must be proposed again by their authors". In a busy goal that number includes old changes whose work already landed, so it can be far larger than the truth and may put the host off a harmless rules change. If the host had shared a starting folder but not yet accepted it, the message points at their agent, which cannot redo it; nothing tells them the real step is to run `workspace init` once more, and the errors they meet when they try to accept or compose the old change do not say so either.

Reported by: node-roles-rules-bind:2. Who acts: fix session.

### `crates/locust/src/cli/roles.rs:189` (untrue text)

`role give` and `role take` print "A ROLE here: ." for a role no rule names.

**How it goes wrong.** 1. Bind a custom formation whose `roles` has `helper` and whose rules never name it (the editor's "+ Role" makes exactly this; all six built-in presets name every role they declare, so only a hand-made or editor-made formation reaches it). 2. The binding seeds `helper` with the host's agent, so `goal status` lists it. 3. Host runs `locust --owner role give --goal G --member Juniper helper`. 4. The daemon records the change and the command prints: `Juniper is a helper in "G".` / `A helper here: .` / `Helpers now: ...` / `Undo: ...`. `role take` of that member prints the same second line.

**What the second readers concluded.** For a role the current formation declares but no rule slot names, `role give` and `role take` print the second result line as "A ROLE here: ." because `duties` joins an empty duty set. Nothing earlier stops it: the validator accepts such a formation, the binding seeds the role with the host's agent, and the daemon accepts the change.

**Smallest fix.** In `duties` (crates/locust/src/cli/roles.rs:182), take `role_duties(formation, role)` first and, when the set is empty, return a sentence of its own, for example "No rule in the current rules names helper, so it changes nothing yet."; keep the "A ROLE here: ..." form for a non-empty set. Add one assertion for that line (no test reads the duties line with a formation present today). The plan names only the "not in the current rules" alternative, so add this third sentence to the plan paragraph at docs/roles-and-permissions-plan.md:2685-2687 as well.

**What a person sees.** After giving or taking a role that no rule uses, the answer's second line reads "A helper here: ." — a sentence with nothing after the colon — instead of telling the person that this role does nothing under the current rules. The role change itself is recorded correctly.

Reported by: formations-editor:5. Who acts: fix session.

### `sites/locust.farm/src/lib/formation-editor/ui/PointBox.svelte:449` (untrue text)

The site's editor still says a rejection never takes a counted result away.

**How it goes wrong.** 1. A visitor opens the formations page; DEFAULT_WAY is 'peer-review', so countsAnswer returns a list with approvals set and PointBox renders the final {:else} note (lines 448-451): "A result counts as soon as it has everything ticked. A rejection, or a report that the check failed, is recorded and does not take that away." 2. In a real peer-review goal with three members, A posts a result, B approves it: it counts and the task round is completed. 3. B then records a reject of the same result. With no pinned evidence, predicate skips B's approval because it is no longer B's latest effective review, and the reject is not eligible; matches is empty, the result stops counting, round.completed is false, and task_available/can_start let the task be taken again. The same path runs for CompletionRule::Check when the member who reported "passed" later reports "failed". A reject by a different member, or any reject after a pick pinned the approval, changes nothing, so the note is only wrong for the approver's own later reject.

**What the second readers concluded.** The formation editor's note under "When does a result count?" still says a rejection or a failed-check report "is recorded and does not take that away", but from 8c086c1 replay reads each member's latest review or attestation, so an approver's own later reject (or a reporter's own later "failed") does stop the result counting. The commit corrected the same claim in docs/formations.md and docs/guide/formations.md and left this sentence, which is the only remaining copy and is not on the plan's list of sentences for Phase 6 to fix.

**Smallest fix.** Reword the one note in sites/locust.farm/src/lib/formation-editor/ui/PointBox.svelte (the last {:else} branch of the counts block, lines 448-451) to match the wording the commit already put in docs/guide/formations.md, for example: "A result counts while it has everything ticked. Each member's latest review or report is the one read: a member who approved and later rejects, or who later reports the check as failed, no longer counts toward it. Another member's rejection does not take it away, and a result that was already picked stays picked." No engine change; the code follows the plan (answer 19, plan lines 2454-2470). An assertion on the text is optional; no existing test pins the old sentence.

**What a person sees.** On the website, while choosing rules, a person reads that a rejection is only a note and can never undo a result that already counts. In a real goal, the member who approved a result can reject it afterwards; the result then stops counting and the task is open for work again (unless it was already picked). Nothing is lost, but the page tells them the opposite of what Locust does in that case.

Reported by: formations-editor:2. Who acts: fix session.

### `crates/locust-core/src/organization/explanation.rs:8` (untrue text)

What Locust says about a formation still calls roles "bound", and the schema says members are bound at creation.

**How it goes wrong.** 1. `locust formation explain examples/formations/directed.json` prints `members bound to role "lead" may offer work...`, `...from members bound to role "reviewer"...` and `the single member bound to role "lead" may select...`. Review-panel prints the `members bound to role "reviewer"` line. The formations page shows the same text under "What Locust will say" (Summary.svelte:30, fed by explain.ts).
2. `formation explain` on pipeline prints `Stage "draft": the host's computer runs this stage: it creates the configured task and durably delivers ready work to goal members.`
3. Every explanation still lists the check `Check authority availability and local execution, filesystem, spending and sharing permissions separately.`
4. The `Shared tree: ... may integrate an exact candidate after ... Host signing permission is separate.` line prints only for a formation with a `workspace` part. None of the six shipped examples has one, so this needs a hand-written formation.
5. `locust formation schema` (and `formation contract`) prints "A reusable role slot. Actual members are bound when creating an instance." and "Eligible recipients of durable ready-work delivery, resolved at binding."
6. A formation whose review count exceeds its fixed identities gets the `impossible_threshold` hint "...or use a role whose membership is checked when binding the template."

**What the second readers concluded.** Commit 8c086c1 applied only two of the sentence changes the Phase 4 cleanup list gives for the formation explanation: the opening line and contextual check 0, and check 0 in the builder's own shorter words. The role, shared-tree, stage and check-3 sentences keep the old wording in both explanation.rs and explain.ts. Two schema comments and one validation hint also became false with this commit; the plan does not list those three.

**Smallest fix.** Change the code, not the plan.
- In crates/locust-core/src/organization/explanation.rs, reword `selector` (line 8), `authority` (line 23), the shared-tree and stage lines in `explain` (109, 129) and contextual check 3 (166) to the companion's sentences, with check 3 saying "its owner".
- Make the same edits in sites/locust.farm/src/lib/formation-editor/contract/explain.ts (lines 22, 40, 119, 134, 106).
- Reword the doc comments on `Role` and `Stage.recipients` in crates/locust-proto/src/organization.rs (56, 239).
- Reword the `impossible_threshold` correction in `Validator::completion` (crates/locust-core/src/organization/validation.rs:144) and its copy in rules.ts:184.
- Regenerate organization.vectors.json, organization.schema.json and organization.contract.json with `scripts/check_formations.py --write`. The contract embeds the same two descriptions at lines 669 and 810.
- Optional: either bring check 0 to the companion's wording or record the shorter sentence as a departure.

**What a person sees.** When a person or an agent asks Locust to explain a formation, and on the formations page, roles are still described as members "bound to" a role, although everywhere else Locust now says a member holds a role and the host gives or takes it. An agent reading the schema while writing a formation is told that members are put into roles when the goal is created and that a step's recipients are fixed when the rules are bound. Both are false now: the host's agent holds every role until the host gives it away, and a step goes to whoever matches when the step opens. Nothing behaves wrongly; only the words are stale.

Reported by: formations-editor:4. Who acts: fix session.

### `scripts/live_farm_demo.py:289` (scripts and guides)

`live_farm_demo.py prepare` still declares the first files done and now aborts; its unit test cannot notice.

**How it goes wrong.** 1. Run `live_farm_demo.py prepare` on fresh state. The coordinator creates the goal (so it is the host's agent), the other three agents are added, and team-chat.json is bound with no `workspace` part (lines 229-245). The role give/take calls at 246-251 are valid against cli/roles.rs and `role_change`.
2. `workspace init` at line 278 passes no `--completion`. `initial_epoch` (crates/locust/src/cli/workspace.rs:521-570) now writes the tree's rule as `formation.decisions.completion`, which for team-chat is `reviews` by `members`, count 1, exclude_author true. Before this commit it wrote `CompletionRule::default()`, the author's declaration.
3. Publish (line 282) and `workspace review` (line 288) succeed.
4. Line 289 sends `completion declare` with the coordinator's credential. The daemon replays the candidate before signing; `may_declare` is false for a `Reviews` rule, the record is excluded with a rule refusal, and the call returns `not_eligible`.
5. `Demo.call` raises RuntimeError. `seed_revision` is never saved, so each re-run re-enters the same branch at line 287 and fails the same way.

**What the second readers concluded.** `Demo.prepare` in scripts/live_farm_demo.py still runs `completion declare` on the first files (line 289), and since this commit the daemon refuses that call, so prepare aborts on every fresh run and every re-run. Its unit test answers `{}` to the declare and to the new `role give`/`role take` calls, so it cannot notice either.

**Smallest fix.** In `Demo.prepare` (scripts/live_farm_demo.py), delete line 289. The host's first files already count when posted, and `Demo.integrate` works as written: `--expected-empty` is optional because `integrate` in cli/workspace.rs pins the current empty head by default. In `test_prepare_recovers_frozen_seed_and_registered_checkouts_after_interruption`, make the stubbed `call` raise on `completion declare` and assert `role give` and `role take` are each called for frontend, backend and reviewer. The code should change, not the plan.

**What a person sees.** Someone setting up the four-agent live demo sees prepare stop with "['completion', 'declare']: not_eligible: coordinator is not eligible under this goal's rules". Running it again gives the same error, so the shared files are never accepted and no agent gets a folder. Ordinary use of Locust is unaffected.

Reported by: guides-scripts:2. Who acts: fix session.

### `scripts/check_operations.py:218` (scripts and guides)

`check_operations.py` still declares file changes done, which the new default rule for the files refuses.

**How it goes wrong.** 1. `python3 scripts/check_operations.py --binary ... ` (either workflow). 2. `Qualification.create_goal` (scripts/check_t1.py:349-354) creates the goal from `formation example directed`: completion is one approval by role `reviewer`, `workspace` is null, and no role holders are named, so the host's agent (the lead) holds `reviewer` and the two workers hold nothing. 3. `workspace init ... --publish` with no `--completion` (check_operations.py:212-213, or 471) reaches `initial_epoch`, which now binds the tree with the goal's rule (reviews by `reviewer`) instead of the author's declaration. 4. Line 218 (or 473) sends `completion.declare` for the first files as the lead's agent. Replay excludes a declaration under a reviews-only rule, the signing trial turns that into `not_eligible` before the level check, and `Qualification.cli` raises CheckFailure because the code is not in `expected_errors`. Phase is `workspace_seed` in the default workflow and `seed` in the two-daemon one. 5. If only that line were removed, the workers' declares at 268 and 485 would be refused the same way and the waits on `approved` at 276-278 and 487 could never be met, since only a review by the lead counts.

**What the second readers concluded.** The scenario holds as written: scripts/check_operations.py was not updated for the new tree default, so under the `directed` goal it inherits from check_t1.py every `completion.declare` it sends on shared files is refused, starting with the first files at line 218 (and line 473 in `--workflow workspace`). It is a stale harness, not a product fault, and the code is doing what the plan asks; I grade it low rather than medium. The same stale seed declare sits at scripts/live_farm_demo.py:289 under team-chat.json (read, not run).

**Smallest fix.** Change the script, not the code. Smallest change: in `Operations.flow` (line 212) and `WorkspaceSmoke.flow` (line 471) pass `--completion '{"kind":"declaration","by":{"kind":"contribution_author"}}'` to `workspace init`. That is the old default (`CompletionRule::default()`), it makes all four declares valid again, and it keeps the cases the script names ("exact candidate declarations arrive", `proposal_declared_and_integrated`). The two seed declares (218, 473) can also be dropped, since the host's first files now count when posted. Plan side: add check_operations.py (and the seed declare at scripts/live_farm_demo.py:289) to Phase 4's list of harnesses to rewrite (plan lines 2726-2741, details line 285), and correct the build notes' line 112 claim that affected harnesses follow current workspace behavior. Phase 6's exit criteria already require one passing run of this script (plan line 3898-3901).

**What a person sees.** Nothing changes for someone using Locust. The project's own three-computer operations check (interrupted transfers, shared files, cancelling, leaving, removal) and its two-computer shared-files variant now stop at their first shared-files step with a "not eligible" error, so those checks cannot vouch for anything until the script is updated.

Reported by: guides-scripts:1. Who acts: fix session.

### `skills/locust/SKILL.md:149` (scripts and guides)

The agent skill and the apply guide still describe the old default for shared files.

**How it goes wrong.** 1. A goal is created with no formation named, so it gets peer-review (node/requests/goals.rs:57-64). The host runs plain `locust --owner workspace init`; `initial_epoch` pins the tree's rule to the formation's workspace rule or else the goal's completion rule (cli/workspace.rs:557-567), not `CompletionRule::default()` as before this commit.
2. An agent that read SKILL.md:148-149 assumes the tree counts a change on its author's declaration and calls `locust_completion_declare` on its own proposal. The skill does not tell it to do so: line 164-165 says completion follows the epoch's pinned rule and to use declare "only within their authority". The call is replayed, `may_declare` (goal/rules.rs:324) finds no Declaration part in peer-review, review-panel or directed, fold.rs:543-560 records a rule refusal, and access.rs:176 returns not_eligible with structured details naming who qualifies. Nothing is stored; the cost is one refused call. On `open` and `independent-attempts` goals the skill sentence is still true by coincidence, because the goal's rule is the author's declaration.
3. apply.md: the commit rewrote lines 28-31 and 37-39 to say twice that the first files need no approval and deleted the `completion declare` line, but kept line 49. "Other" used to mean "other than the default author declaration"; that antecedent is gone, so the sentence now dangles. It is ambiguous rather than a flat contradiction: a host who reads the whole paragraph is told twice the first files need nothing (true: fold.rs:1004-1008 answers at once for the host agent's parentless, source-free change under any rule). The "host waits for two approvals" outcome needs the reader to ignore both statements.

**What the second readers concluded.** Both sentences are stale at 8c086c1 exactly as cited, but neither is an untracked defect: the companion plan already lists both, by exact text, as Phase 6 prose work. What this commit itself introduced is narrower: by pulling half of the apply.md rewrite forward (build-note departure 8) it left "Other completion rules require their specified evidence." at docs/guide/apply.md:49 with no antecedent, and it made the skill's "an author completion declaration" (skills/locust/SKILL.md:149) false for every goal that is not `open` or `independent-attempts`.

**Smallest fix.** No code change. Either is acceptable, and the plan does not need to change: (a) leave both on Phase 6's list, where they already sit with exact quotes (docs/roles-and-permissions-plan-details.md:279, 280, 422, 423, and line 441 "Phase 6 keeps the prose, the site and the skill"); or (b) since departure 8 already opened that paragraph, finish it now with two one-line text edits: apply.md:49 "Other completion rules require their specified evidence." becomes "Later changes need whatever the tree's rule asks."; SKILL.md:148-149 "an author completion declaration" becomes "the goal's completion rule; the first files a host shares need no approval". I would take (b) for apply.md line 49 because this commit left that paragraph half-rewritten; the skill line can equally wait for Phase 6, which also adds the first-files sentence to the skill.

**What a person sees.** Until the wording pass in Phase 6, an agent that trusts the skill's description of the default may try to mark its own file change as done and get a clear "not eligible" answer that says who can approve it; it loses one call, nothing else. A person reading the guide's "Start and inspect a tree" section meets one leftover sentence about "other completion rules" that no longer refers to anything, right after being told twice that the starting files need no approval.

Reported by: guides-scripts:3. Who acts: the guides phase (R6).

### `docs/guide/formation-authoring.md:103` (scripts and guides)

The authoring guide tells the host to run `role give` after a review-panel join, which now answers that nothing changed.

**How it goes wrong.** 1. Host runs the two commands the guide shows: `goal create --formation review-panel --plan` then `--confirm`. 2. Host runs `locust --owner goal add --goal G --agent maple` (no flags). `add_plan` calls `roles::selected_role`, which with neither `--role` nor `--no-role` returns `counting_role` = `reviewer` for the panel preset (Reviews by role reviewer, count 2, exclude_author true; reviewer is not an authority role). `goal_add` sends that role in `Request::GoalInvite`, and replay of `MemberAdmitted { role: Some("reviewer") }` pushes the member into `snapshot.roles["reviewer"]`. 3. Host reads the guide's next sentence, "After others join, use `locust --owner role give --goal GOAL --member MEMBER ROLE` to give a role", fills in the only role the preset has, and runs `role give --goal G --member maple reviewer`. 4. `roles::run` sends `RoleGive` with expected = [host, maple]; `role_change` passes the compare-and-set and membership checks, then hits `if holders.contains(&member) { return Err(conflict("the member already holds this role")) }`; the CLI maps Conflict to exit 7. Nothing is signed or changed. The scenario does not occur if the host reads the sentence as optional, or added the member with `--no-role`.

**What the second readers concluded.** The authoring guide's "Start a goal" section shows only review-panel, says the host's agent holds every role, and then says "After others join, use `role give ... ROLE` to give a role" without saying that under review-panel `goal add` and `goal invite` already make the new member a reviewer; a host who infers they must give `reviewer` gets exit 7 "the member already holds this role". The sentence is incomplete rather than wrong (it names a placeholder ROLE and is not written as a required step), and the missing review-panel sentence for this exact page is already scheduled in the plan's Phase 6, so this is a one-sentence doc gap, not a Phase 4 departure.

**Smallest fix.** Doc-only, no code change: in docs/guide/formation-authoring.md lines 103-104, add the sentence the plan already assigns to this page in Phase 6 (docs/roles-and-permissions-plan.md:3609-3625): "Members you add or invite to a review-panel goal become reviewers unless you pass `--no-role`." and keep the `role give` sentence for any other role. Either land it now, since Phase 4 already rewrote these two lines, or leave it to Phase 6 where it is listed; the plan and the code (`selected_role` in crates/locust/src/cli/roles.rs, `role_change` in crates/locust-core/src/node/requests/goals.rs) are both right as they stand.

**What a person sees.** Someone following the authoring guide with the review-panel example may think each agent they add still needs to be made a reviewer by hand. If they run the command, Locust answers "the member already holds this role" and exits with an error, although the goal was already set up correctly and nothing is lost or changed.

Reported by: guides-scripts:4. Who acts: the guides phase (R6).

### `crates/locust-core/src/node/callers.rs:95` (departs from the plan)

An agent that asks for `role give` or `role take` is told `person_command`; the refusal never carries the act the plan names.

**How it goes wrong.** 1. An agent connection (Caller::Agent, active principal, no on_behalf) sends a well-formed `Request::RoleGive` or `Request::RoleTake` (valid role name, ascending `expected`, so `Request::check` passes). 2. `Node::handle` calls `callers::resolve` (requests/mod.rs:73) before any planning. 3. Both operations are `Audience::Host` (locust-proto api.rs:867-868), so `resolve` returns `only_you(.., host = true)` (callers.rs:175-181). 4. The match in `only_you` (callers.rs:85-96) has no arm for either request and falls to `_ => Act::PersonCommand`. 5. The agent receives `Denied`, message "this request is the owner's to make", details `Refused { act: person_command, goal: Some(goal), why: OnlyYou { operation: "role.give" | "role.take", host: true } }`. `Act::GiveRole` (level.rs:62) is never built anywhere in crates at 8c086c1, nor in the current working tree.

**What the second readers concluded.** An agent credential that sends `role.give` or `role.take` is refused with `act: person_command`, because `callers::only_you` has no arm for the two new requests. The roles plan (Phase 3, line 2250-2252) says `GiveRole` gets its arm with the role operations from Phase 4, the Phase 4 commit did not touch callers.rs, and none of the 15 declared departures covers it, so `Act::GiveRole` is declared and never constructed.

**Smallest fix.** Change the code, not the plan: in `only_you` in crates/locust-core/src/node/callers.rs add `Request::RoleGive { .. } | Request::RoleTake { .. } => Act::GiveRole` before the wildcard arm, and add one row for each request (expected_host true, expected_act GiveRole) to the table in `owner_only_commands_report_why_and_do_not_use_operation_names_in_messages` (node/tests/levels.rs:312-335). Note for Phase 5: both give and take map to the one variant, so its phrase should read correctly for a take as well (the plan names no separate take verb).

**What a person sees.** Nothing today: no command prints this field yet, and the refusal still names the operation and says only the host may do it. Once Phase 5 words refusals from it, an agent that tries to give or take a role would be told it "can't run this command" instead of "can't give a role"; the line pointing at the host's `role give` command would still be right.

Reported by: views-api:4, node-roles-rules-bind:4. Who acts: phase 6 (R5).

### `crates/locust-core/src/goal/mod.rs:337` (leftover)

`Goal::role_holders` is added and nothing calls it.

**How it goes wrong.** 1. 8c086c1 adds `pub fn role_holders(&self) -> &BTreeMap<String, Vec<PublicKey>> { &self.state().roles }` to impl Goal. 2. A search of the whole tree at 8c086c1 for `role_holders()`, `.role_holders` and `::role_holders` finds one hit only, the plan's own sentence. The other mentions of the string are two test function names (goal/tests.rs:2389, 2553), the wire kind string (locust-proto/src/event.rs:540, vectors.rs:206) and docs or evidence files. 3. The readers that would use it go around it: node/farm.rs:276, node/requests/goals.rs:165, 310, 312 and 397, node/requests/invitations.rs:180, node/requests/levels.rs:31, plus the tests in goal/tests.rs and node/tests/roles.rs. (The original finding names access.rs; that file does not read roles, levels.rs does.) 4. State.roles is a pub field (goal/state.rs:209) and Goal::state() is pub (mod.rs:166), so the accessor adds nothing. 5. Goal is pub in a library crate and the workspace sets no unreachable_pub or similar lint, so neither rustc nor clippy reports it. 6. HEAD and the working tree are the same: still no caller.

**What the second readers concluded.** Goal::role_holders, added by 8c086c1 at crates/locust-core/src/goal/mod.rs:337-339, has no caller in the repository; every reader of the holder map reads the public field state().roles directly. It is not a departure from the plan (the plan asked for it at docs/roles-and-permissions-plan.md:2491 and names no caller); it is dead code the plan itself introduced, against the AGENTS.md rule "Do not leave dead code".

**Smallest fix.** Change both the code and the plan. Delete `Goal::role_holders` (crates/locust-core/src/goal/mod.rs:337-339) and remove "; new `Goal::role_holders`" from docs/roles-and-permissions-plan.md:2491. Deleting is smaller than routing about nine state().roles readers through a one-line wrapper over a public field, and no later phase of the plan names a caller for it.

**What a person sees.** Nothing. A person using Locust sees no difference whether this helper exists or not; it only matters to whoever maintains the code.

Reported by: chain-snapshot:5, plan-changes-first-half:5, lens-earlier-phases:7. Who acts: phase 6 (R5).

### `crates/locust-core/src/node/requests/invitations.rs:26` (leftover)

`InviteRecord.host_name` is written to the store and never read.

**How it goes wrong.** 1. The host runs `goal invite` (or `goal add`, whose CLI path at crates/locust/src/cli/only_you.rs:785 sends the same `Request::GoalInvite`). 2. `goal_invite` reads the host agent's name with `Self::host_name(entry)` (invitations.rs:194), signs it into the ticket (:212) and also puts it in the stored `InviteRecord` (:237). 3. Every later reader of that record ignores the field: `InviteRecord::summary` (:36-60) copies goal, title, governance, role, times and redemption; `goal_invitations` (:80) and `invitation_revoke` (:107, :131) read `goal`, `redeemed`, `revoked_ms`; `plan_join` (peers.rs:325-364) reads `goal`, `revoked_ms`, `redeemed`, `expires_ms`, `governance`, `role`. 4. The joiner gets the name from the ticket (`invitation.host_name`, an `Invitation`, at :306/:335/:382/:395/:416) and the host gets it from its own admission (`Node::host_name`, goals.rs:242-256, which falls back to `JoinRecord.host_name`, never `InviteRecord`). The stored copy is never decoded into anything a person or another computer sees.

**What the second readers concluded.** `InviteRecord.host_name`, added by 8c086c1, is written by `goal_invite` on every `goal invite` and `goal add` and read by nothing: it is write-only local state that the serde derive hides from the dead-code lint. It follows a loose reading of plan line 2590 ("store the new fields") but breaks the repository's no-dead-code rule; no later plan gives it a reader.

**Smallest fix.** Both change, code first. In crates/locust-core/src/node/requests/invitations.rs delete `pub host_name: String` from `InviteRecord` (:26), and in `goal_invite` move `host_name` into `Invitation::signed` (drop the `.clone()` at :212) and drop it from the record literal (:237). No reader or test touches the field, so nothing else moves (the `host_name: "host".into()` test literals build `Invitation`/`JoinRecord`/API types, not `InviteRecord`). Then the plan's owner rewords docs/roles-and-permissions-plan.md:2590 to say `InviteRecord` stores `role`, and `JoinRecord` in node/local.rs stores `host_name` and `name`. Optional tidy in the same pass: research/joinable-farms-rewrite-contract-2026-10-05.md:252 describes the record R4 leaves as having "role and name fields"; it should say role only.

**What a person sees.** Nothing. The host's name still shows on the invitation preview, on the join result and in status; only an unused copy saved on the host's own computer goes away.

Reported by: invitations-join:7. Who acts: fix session.

### `crates/locust-proto/src/invite.rs:133` (leftover)

The doc comment of the ticket's signature now sits on the new `role` field, and four nearby comments no longer match the code.

**How it goes wrong.** 1. crates/locust-proto/src/invite.rs:133-136 reads: the comment "Host signature over every preceding field, including the capability digest. Verified before any preview or join intent.", then `pub role: Option<String>,`, then `pub signature: Signature,` with no comment. The new fields `host_name` (line 119) and `JoinRequest.name` (line 397) also have no comment. The uncommitted working tree still has this at the same lines.
2. invite.rs:312 says `check` "Checks the version and the contact hints". It now checks `host_name` and `role` first (315-322), then the version, the publication (already unmentioned before this commit) and the hints. The name-before-version order has no visible effect: `from_ticket` rejects another version from the first byte before decoding (373-377), and `signed` always writes the current version.
3. invite.rs:406 and 432 say the join signature covers goal, member, endpoint and secret. `join_digest` (457-472) now also hashes the name with its length, and `verify` (439-454) also answers false when `is_member_name(name)` fails.
4. crates/locust-proto/src/sync.rs:25-27 says a `Join` is answered with the `Frontier` "also when the same key repeats it". `plan_join` (crates/locust-core/src/node/peers.rs:330-339) now also requires `member.name == request.name`. Only a non-conforming client can hit the difference: the honest joiner refuses to change the name of a pending join (crates/locust-core/src/node/requests/invitations.rs:317) and `joins` re-signs with the stored name (peers.rs:245-251).
5. crates/locust-proto/src/event.rs:1010 says names are "bounded, visible, and unpadded". `is_member_name` (1011-1016) tests non-empty, at most 64 bytes, `trim() == name` and no `char::is_control`. U+200B is neither Unicode whitespace nor a control character, so a name made only of U+200B passes. The CLI's `safe` (crates/locust/src/cli/presentation.rs:144-153) prints it as `\u{200b}`.

**What the second readers concluded.** Doc-only, and it stands as reported: in commit 8c086c1 the new `Invitation.role` field was inserted between the signature's doc comment and `pub signature`, so rustdoc describes `role` as the host signature and leaves `signature` undescribed. Four nearby comments (`Invitation::check`, `JoinRequest.signature` and `JoinRequest::verify`, the `Join` line of the sync.rs module doc, and `is_member_name`) still describe behaviour from before the name was added. No behaviour is wrong.

**Smallest fix.** Comments only, in the code; the plan is already right.
- `Invitation` struct (invite.rs:132-136): move `pub role` above the signature's comment and give it one line of its own (the role the admission will carry, signed by the host). Add one line each for `host_name` and `JoinRequest.name`.
- `Invitation::check` comment (312): say it checks the names, the version, the publication and the hints.
- `JoinRequest.signature` (406) and `JoinRequest::verify` (432): add the name to the list of what is signed, and say `verify` is also false for an unusable name.
- sync.rs:25-27: say "also when the same key repeats it from the same endpoint with the same name".
- `is_member_name` comment (event.rs:1010): drop "visible". The plan's rule (docs/roles-and-permissions-plan.md:2352-2353) is "1 to 64 bytes; no outer spaces or control characters" and does not ask for visibility, so the code should not change.

**What a person sees.** Nothing. A person using Locust sees no difference; only someone reading the source or its generated documentation is misled about which field is the signature and what the join signature covers.

Reported by: signed-format:4. Who acts: fix session.

### `research/tla/organization.md:30` (leftover)

The model's property map cites two tests that do not exist, and the model header names the old baseline.

**How it goes wrong.** 1. A reader opens research/tla/organization.md row "Taskless work does not require selection" (line 30) and searches for `open_taskless_work_needs_no_administrator_decision`; goal/tests.rs has only `open_taskless_work_needs_no_host_decision` (line 387). 2. Same for row "Cutoffs are exact global tenure restrictions" (line 35): `scope_proof_cannot_retain_evidence_past_the_administrator_cutoff` is cited, the function is `scope_proof_cannot_retain_evidence_past_the_host_cutoff` (line 1125). The only remaining hits for the old names are the plan's own rename list. 3. The reader opens Organization.tla and reads line 5, "Current organization protocol subset at c88e3bc", although the same file already models the governance key and role holders, and README.md lines 3-6 and organization.md line 3 say protocol/API 7. I checked every other backticked test name in organization.md against `fn` definitions at 8c086c1: all exist, so it is exactly these two.

**What the second readers concluded.** The property map research/tla/organization.md cites two Rust tests under names that no longer exist (the `administrator` forms, renamed to `host` in 65aecf1), and the comment header of research/tla/Organization.tla still calls the model the subset at c88e3bc while the map, the README and every organization case in cases.json say Phase 4 role-holder subset, protocol/API 7. The plan gave the table rename to this phase, so the first half is a missed plan item, not only inherited drift; neither file is in 8c086c1's own diff (they belong to the phase's model commit 3a56930) and both are still stale at HEAD 46cc3b3.

**Smallest fix.** In research/tla/organization.md change the two citations to `open_taskless_work_needs_no_host_decision` (line 30) and `scope_proof_cannot_retain_evidence_past_the_host_cutoff` (line 35); this is the plan's own item and touches no checked input. For Organization.tla line 5, reword to the Phase 4 role-holder subset, protocol/API 7, but note one cost the original fix leaves out: the retained run research/evidence/tla/organization/phase4.json records this file's sha256 (4dd0cf7b..., equal to the file today) and the evidence README says the same frozen model hash was checked before and after the Rust work. A comment edit changes that hash. So either make the header edit together with the next model change (E1's seven cases, which needs a rerun anyway), or make it now and add one sentence to the evidence README saying the change is comment-only, the way the cases.json baseline pin was recorded as metadata-only. No code changes.

**What a person sees.** Nothing. A person using Locust sees no difference; only someone reading the research notes to trace a model rule to its test would search for two test names that are not there and read an out-of-date baseline line.

Reported by: model-fidelity:6. Who acts: fix session.

### `crates/locust-core/src/goal/closure.rs:65` (model gap)

The order of closes and reopens after a change of lead has no test and is not in the model.

**How it goes wrong.** Goal whose finish authority is the role `lead` (e.g. `directed`). Lead A, with a long log (close at seq 50), closes task T, anchored before the change. The host signs `RoleHolders` moving `lead` to B. B, whose log is short (seq 3), reopens T with `previous`/`expected` = A's close (the way the node tests reopen, lifecycle.rs:1133). W then starts T naming B's reopen as `closure` (which is what the node declares: claims.rs:115 takes the projection's last closure decision). In `directed` W also needs B's offer first, but `open_at_observed_closure` runs before the offer check (fold.rs:383), so the result is the same. The walk finds both decisions through W's dependency on the reopen and the reopen's dependency on the close. With today's key (anchor position, author, seq, id) the reopen wins and the start is effective. With the replaced `ah.seq > *seq`, A's close (50) beats B's reopen (3), `latest != declared`, and the start is excluded on every computer with "attempt omits or regresses its authenticated closure position"; a second close and reopen by B would not help (still lower seq), only a task revision would. No test would fail: every close/reopen pair in the suite is signed by one fixed author, and no test combines a role change with a close, a reopen and a start.

**What the second readers concluded.** The code is right, but the closure ordering this commit added for a change of lead (closure.rs:65-83, and the separate copy of the same sort in flow.rs `desired_effects`) is pinned by no test and no model case. The plan's own check table promises "pick or close" order and cites a test that only exercises a pick; the TLA model excludes closure/reopen and says Rust regressions cover it, and none does.

**Smallest fix.** Keep the code. Add one replay test in crates/locust-core/src/goal/tests.rs that drives `Verifier::open_at_observed_closure`: a formation with `decisions.finish = Authority::Role { lead }` and independent starts (or `directed` plus an offer by the new lead); the first lead signs padding records then a `Close`; `RoleHolders` moves `lead`; the new lead's first record is a `Reopen` with `previous: Some(close)`; an `AttemptStarted { closure: Some(reopen) }` must be `Effective` across `f.replays()` (forward, reversed, reloaded), the round must not be `closed`, and with an unapproved result under a review rule `desired_effects` must ask for reviews again (this also pins the flow.rs sort and its `Reopen` branch, which no test reaches today). Then correct the docs: docs/roles-and-permissions-plan.md:2492-2493 should name closure.rs and flow.rs beside projection.rs, and docs/roles-and-permissions-plan-details.md:296 should cite the new test for the "or close" half. Optional: one shared `decision_order` helper used by projection.rs, closure.rs and flow.rs so the three copies cannot drift.

**What a person sees.** Nothing today; it works. If this ordering were ever broken by a later change, then after the lead role is handed to someone else and the new lead reopens a closed task, agents would be refused when they try to start that task and it would sit idle until someone revised it, and no test would warn anyone first.

Reported by: lens-convergence:2, model-fidelity:3, model-fidelity:2, flow-closure-workspace:4. Who acts: fix session.

### `research/tla/Organization.tla:307` (model gap)

The model's `RolesNeverEmpty` always holds, and the host-agent start, the one-holder rule and the validity of a role record are never exercised.

**How it goes wrong.** All four parts and the mutation note check out by reading; nothing was run.

1. `RolesAt` ends `IF remaining = {} THEN {5} ELSE remaining` (Organization.tla:154), so `RolesNeverEmpty` (307-308) is true by construction. The row at organization.md:26 still has real evidence in the `role-removal` witness (Organization.tla:267). No invariant says a holder is a member, and the model has no guard against removing identity 5, which chain.rs:147-150 excludes.

2. The `latest = 0` default (line 149) is never load-bearing. Role records 20 and 21 sit before rules record 7 in `Founding` (45-47), and every work record is anchored at 7 or 14. One imprecision in the finding: under `governance-fork` the default is also read at anchor 7 once the fork retracts the suffix, but nothing anchored there is eligible, so outcomes do not change. The daemon only ever founds a goal in the opposite order: genesis, admission, rules, and role records later (requests/goals.rs:86, 100, 121, 357). That is the one order no transcript has.

3. `lead` is always a single holder: {4}, {3} in `lead-change`, or {5} by fallback. So `HoldsLead` (line 157) written as membership gives the same outcomes, including `organization-authority-mutation`, where author 1 fails either way. In Rust the rule is real: `rules::authority` returns `None` unless the list has exactly one holder (rules.rs:239-247, used at fold.rs:888-890). No test pins it: tests.rs:2581-2600 builds a two-holder `lead` but asserts only the binding exclusion from chain.rs:556.

4. `RoleEvent` takes any holder set and `RolesAt` reads `ByID[latest].holders` unchecked; an empty set would fall to 5. chain.rs:184-196 excludes such a record and keeps the earlier list. No transcript has one.

5. The mutation config runs only `role-change`, which has no selection; it is caught by review 16 (identity 4 anchored at 7, before record 14). Under the same mutation `lead-change` selects nothing once 14 is held: selection 12 fails `HoldsLead` at 14, and 15 fails because its prior, 12, is invalid.

**What the second readers concluded.** The finding stands as a low-severity gap in the evidence, not in behaviour: `RolesNeverEmpty` cannot fail, and no model case exercises the host-agent start, the one-holder rule or role-record validity, while organization.md lists none of the three as left out. The Rust code does all three correctly, and the model was built as the plan prescribed, so this is a plan and model-doc weakness rather than a builder departure.

**Smallest fix.** No change to replay code. Smallest change, in research/tla/organization.md:

- Row 26: say `RolesNeverEmpty` is true by construction and the evidence is the `role-removal` witness.
- The "outside this subset" paragraph (lines 41-45): add the start with the host agent, the one-holder rule, and role-record validity as Rust evidence only, naming `rules_bound_fills_unheld_declared_roles_with_the_host_and_requires_one_holder_per_authority` and `role_holders_must_be_distinct_admitted_and_non_empty`.
- Mirror the same sentence in the plan's model section (docs/roles-and-permissions-plan.md:2796-2815), since the plan prescribed the fallback and the invariant name (2757-2776).

Two corrections to the reviewer's proposed fix:

- The two named tests do not cover a pick by one of two holders. Either say so, or add one replay test in crates/locust-core/src/goal/tests.rs beside the test at line 2581: after the two-holder `lead` record, a `ScopeDecided` by either holder is excluded. That pins `rules::authority`.
- The proposed stronger invariant ("every holder `RolesAt` returns at a governance anchor is a member there") fails as worded in the first state of every case: at anchor 1 `RolesAt` returns {5}, but 5 is admitted by record 2. Quantify it over anchors at or after the first rules record, or have `RolesAt` return no list before one.

The other proposed model additions are sound if the model is extended: a first role record after the rules record with a pick by 5 between them, a two-holder `lead` witness, and a selection by the new lead anchored before the change in the mutation scenario.

**What a person sees.** Nothing. Locust behaves the same for people and agents; only the written evidence claims slightly more than the model checks, so a later change to these rules could slip past the model and the tests unnoticed.

Reported by: model-fidelity:4. Who acts: fix session.

### `crates/locust-proto/src/event.rs:778` (test gap)

Three refusals of a bad name have no test: an admission with a bad role name, a ticket with a bad host name, and a signed join request with a bad name.

**How it goes wrong.** 1. In crates/locust-proto/src/event.rs (at 8c086c1) remove lines 778-780, the `|| role.as_ref().is_some_and(|role| !is_role_name(role))` half of the MemberAdmitted guard. No test builds a MemberAdmitted with an unusable role: vectors.rs:330-360 mutates only `name` (line 341) and leaves role None; organization/tests.rs:308-353 exercises the separate RoleHolders arm (event.rs:784) and an invitation role; every other MemberAdmitted built in tests has role None or the valid "workers" (goal/tests.rs:2844). 2. In invite.rs remove `!crate::event::is_member_name(&self.host_name) ||` at line 315. Every invitation built in tests carries a usable host name: "host" (invite.rs:496), "another host" (563), 64 x "h" (708), "Maple" (organization/tests.rs:343), "Host" (cli/invitations.rs:278, durable_tests.rs:764, tests/cli.rs:2029, tests/invitations.rs:20). The four BadName assertions in the tree (vectors.rs:347 and 354, organization/tests.rs:334 and 350) cover a bad admission name, a bad RoleHolders role and a bad invitation role only. I did not run the mutation (read-only); the conclusion rests on there being no test input that reaches either clause. 3. The companion row (plan-details line 307) for member_names_are_signed_at_admission_and_the_host_name_rides_the_ticket says a request with a bad name is refused and admits nobody; that test (node/tests/roles.rs:290-317) uses only "Harbor" and "Maple". The refusal itself is pinned one layer down by api.rs:2155 and Request::check runs first in node/requests/mod.rs:72, so only the node-level assertion is missing.

**What the second readers concluded.** Two refusals the plan specifies are implemented but pinned by no test: BadName for an admission whose role fails is_role_name (the role half of the MemberAdmitted arm in Header::check), and BadName for a ticket whose host_name fails is_member_name (the first clause of Invitation::check). Both guard host-signed data only, so this is a low-severity regression-protection gap, not a live bug.

**Smallest fix.** Add tests only; no code or plan change. In `an_admission_with_a_bad_name_is_not_an_event` (crates/locust-proto/src/vectors.rs) add a second loop that keeps the name valid, sets `role` to Some(""), Some("  ") and Some("a\nb"), and expects EventError::BadName from both Event::sign and Event::decode. In crates/locust-proto/src/invite.rs add a test shaped like `unusable_hints_are_refused_when_writing_and_when_reading`: build an Invitation with host_name "", 65 bytes, " padded" and "a\nb", re-sign it, and expect InviteError::BadName from verify, from to_ticket, and from from_ticket fed hand-encoded bytes (`format!("{TICKET_PREFIX}{}", Hex(&codec::encode(&invitation)?))`). The reviewer's wording needs that one correction: to_ticket runs the same check, so from_ticket cannot be reached through to_ticket for a bad name.

**What a person sees.** Nothing today. These checks only matter if a host's own computer signs a blank or garbled role or host name, which an honest host cannot do; the gap is that a future change could drop either check without any test noticing. Even then the host name is cleaned before it is printed, so the worst visible effect would be an odd or blank role label.

Reported by: signed-format:3, lens-hostile-member:4. Who acts: fix session.

### `crates/locust/tests/cli.rs:2581` (test gap)

What `role give` and `role take` print is tested only for the Undo of a group role: a lead's Undo, the earlier-rules line and the sentences about what a role does are not.

**How it goes wrong.** 1. Both role tests (crates/locust/tests/cli.rs:2528 and 2581) build their status from hosted_goal_status (1228-1259), which has deciding empty and current_rules None. Neither test changes those two fields, and both stubs panic on Request::Event or Request::BlobGet. So current_formation returns None at roles.rs:30-32, duties always takes the None arm (197), and `deciding` (229) is always false.
2. Delete the `give && deciding` arm at roles.rs:339-340. Maple (not the host's agent) holds `lead`; the host gives it to Juniper. The command now prints `Undo: locust --owner role take ... --member <Juniper> lead`. Run as printed, role_change (crates/locust-core/src/node/requests/goals.rs:339-349) empties the list and puts the host's agent in, not Maple. Every test still passes because no test has a deciding role.
3. Change the text at roles.rs:185 or 189, or a word in duty_words (168-181): every test still passes. Only the duty set is pinned (crates/locust-core/src/organization/tests.rs:289); the wording and both sentences are not.
4. The conflict test at 2528 gets an error back and asserts only exit 7, so it cannot read any result line, though the plan says it reads the one for an undeclared role.

**What the second readers concluded.** At 8c086c1 no retained test runs three branches of crates/locust/src/cli/roles.rs: the Undo of a give on a deciding role (339-340, with its holders line at 265-266), the sentence for what a declared role does (188-196), and the sentence for a role the current rules do not declare (184-187). The plan (docs/roles-and-permissions-plan.md 2848-2849, 2859-2861) says cli.rs asserts the first and the third; it does not, the build notes do not list the omission, and the code itself reads correct, so this is a coverage gap and not a present defect.

**Smallest fix.** Test only, no production change. In `role_undo_lines_put_the_holders_back_and_name_the_member_by_key` (crates/locust/tests/cli.rs:2581): set `view.current_rules`, answer `Request::Event` with `formation_event` and `Request::BlobGet` with the review-panel preset's bytes, add `lead` to `view.roles` held by a third member and to `view.deciding`, and make the stub's RoleGive replace the holder for `lead`. Assert that giving `lead` prints `Undo: locust --owner role give --goal 04040404 --member <third member's prefix> lead` and that running that line restores the holder; that the same run prints `lead is not in the current rules. It still applies to work under earlier rules.`; and that a `reviewer` give prints `A reviewer here: approves results.` Raise the server's connection count to match. Then correct plan line 2848-2849, which credits the undeclared-role line to the conflict test.

**What a person sees.** Nothing today: the commands print the right lines. The loss is protection later. A future change could make the Undo printed after handing the lead to someone else give the lead to the host's agent instead of the member who had it, or garble the sentence that says what a role does, and no check would notice.

Reported by: cli-texts:5, tests-vs-plan:5, formations-editor:6. Who acts: fix session.

### `crates/locust/tests/workspace.rs:567` (test gap)

No test pins the rule `workspace init` gives the files; the plan's test is absent and the nearest assertion passes with the old default.

**How it goes wrong.** 1. At 8c086c1, `initial_epoch` (crates/locust/src/cli/workspace.rs:561-566) falls back to `formation.decisions.completion.clone()` when the formation has no workspace part and `--completion` is absent. This matches the plan (docs/roles-and-permissions-plan.md:2720-2722).
2. Replace line 566 with `CompletionRule::default()` (declaration by the contribution's author, crates/locust-proto/src/organization.rs:218-224).
3. Only three cargo tests reach `initial_epoch`, and none can tell the two apart:
   - crates/locust/tests/workspace.rs: the stub's goal formation is `Formation::default()` (line 80), whose `decisions.completion` is `CompletionRule::default()`. The one assertion on the default (lines 567-570) still compares against `CompletionRule::default()`, and the commit did not touch it. The stub's `RulesBind` arm asserts only `formation.workspace.is_some()`. `lost_initial_epoch_reply_accepts_only_equivalent_pinned_policy` always passes `--completion` (line 1221).
   - crates/locust/tests/t2_flow.rs:262-290 uses `independent-attempts`, whose completion rule equals the default, and declares completion explicitly.
   - crates/locust/tests/managed_checkout.rs:109-165 uses a peer-review goal but integrates only the first files, which count under any rule; the agent's later change is published and never integrated.
4. The core test `the_host_accepts_its_first_files_with_no_approval_and_its_next_change_waits` (crates/locust-core/src/node/tests/workspace_lifecycle.rs:805-880) builds the workspace policy by hand in `bind_peer_files`, so it never goes through the command line's default.
5. So the cargo suite stays green by reading. Outside cargo, the recipe in docs/guide/apply.md:168-180 (peer-review goal of one, `workspace init` with no `--completion`, second change integrated with no `completion declare`) would fail on the revert. The two-member exit criterion (plan lines 3080-3087: the next change exits 7 until the other member approves) has no automated check through the CLI default.

**What the second readers concluded.** The plan's test `init_pins_the_lead_role_and_the_goals_completion_rule` was not written and nothing in `cargo test` stands in for it: no cargo test would fail if `initial_epoch` went back to `CompletionRule::default()` when `--completion` is absent. The code itself is correct, and the guide recipe `shared-workspace-loop` (run by scripts/check_documentation.py, a phase exit gate but not one of AGENTS.md's required cargo checks) does catch that revert for a goal of one.

**Smallest fix.** Add one test to crates/locust/tests/workspace.rs. The plan's name mentions a "lead role" the plan text itself dropped, so a name like `init_pins_the_hosts_agent_and_the_goals_completion_rule` fits better.
- Seed the stub (`State::new`, or a variant) with the `peer-review` preset instead of `Formation::default()`.
- Run `workspace init --empty` with no `--completion`.
- Read the formation bound by `RulesBind`, as lines 562-566 already do.
- Assert `workspace.completion == formation.decisions.completion`, that it is not `CompletionRule::default()`, and that `workspace.integrator == Authority::Participant { key: PRINCIPAL.to_string() }`.
A second member in the stub is not needed, because `--integrator` is gone and the code reads only `status.host`. No production code or plan change is needed.

**What a person sees.** Nothing today; the shared files follow the goal's rule as planned. The risk is only that a later code change could quietly put the old "author's own word" default back, so a member's file change would stop waiting for another member's approval, and `cargo test` would not notice.

Reported by: tests-vs-plan:8, plan-changes-second-half:8, cli-texts:8. Who acts: fix session.

### `crates/locust/tests/cli.rs:2917` (test gap)

The `rules bind` test omits the refusal for a rule only the task's creator could meet and the line counting file changes.

**How it goes wrong.** 1. At 8c086c1 the test (crates/locust/tests/cli.rs:2917) makes four runs, all with `--formation peer-review`: plan and confirm with the tree on, plan and confirm with it off. 2. The refusal needs a formation whose `decisions.completion` names `task_creator` with no `workspace` part while the goal's tree is on: `rules_plan` copies that rule into `/workspace/completion`, `inspect` reports `selector_scope` there (validation.rs:235 calls `completion(..., false)`; :54-59 raises it), and only_you.rs:1273-1283 answers `invalid` with the sentence. Such a formation is valid by itself (organization tests assert a task-creator rule outside a stage is valid), so this is reachable with `--formation-json`; no run does it. 3. The stub answers `WorkspaceProposals` with `vec![]` (cli.rs:2966), so `pending` at only_you.rs:1308-1317 is always 0 and the line is never built; the result's repeat at only_you.rs:1399-1406 therefore never fires either. The filter (current epoch, not yet landed) is also unpinned. 4. `git grep` at the commit finds both sentences only in only_you.rs and the plan; the only other `rules bind` in CLI tests is absent (workspace.rs and t2_flow.rs hits are `workspace bind`). The staged, uncommitted work in the tree adds `invalid_stage_rules_are_explained_before_create_or_bind_has_a_plan`, which covers a different refusal (a stage's task creator) and does not close this.

**What the second readers concluded.** The plan lists "a goal rule that names the task's creator is `invalid`" as part of `rules_bind_moves_the_shared_files_to_the_new_rules_and_says_so`, but the test as landed never runs that case, and its stub returns no proposals, so neither the refusal sentence nor the "N file changes that have not landed..." line (in the plan or repeated in the result) is produced by any test. The production code for both is present and reads correctly; this is a missing-coverage gap only, and no declared departure covers it.

**Smallest fix.** Test-only change in `rules_bind_moves_the_shared_files_to_the_new_rules_and_says_so` (crates/locust/tests/cli.rs); no production change. (a) Add a run with the tree on and `--formation-json '{"schema_version":2,"decisions":{"completion":{"kind":"declaration","by":{"kind":"task_creator"}}}}' --plan`; assert the invalid exit (6) and the sentence "These rules cannot apply to the shared files. Give the files a rule in the formation's workspace part." This run needs only `GoalStatus` from the stub; raise the `server(home.path(), 4, ...)` connection count to match. (b) Have the stub return unlanded proposals of the current epoch plus one that must not count (already landed or of another epoch), and assert the count line in the plan and again in the confirmed result. Use two countable proposals rather than one: the code prints "1 file changes" for a single one, and the test should not pin that wording.

**What a person sees.** Nothing wrong today: a host who binds rules that cannot apply to the shared files is told so and told what to do, and a host with file changes still waiting is told their authors must propose them again. The risk is later: if an edit breaks either message, no check would notice. At this commit the host would then see a plan saying the shared files will follow the new rules, say yes, and get only "the formation is invalid; validate it for diagnostics"; or the warning about waiting file changes would silently stop appearing.

Reported by: tests-vs-plan:7, plan-changes-second-half:7. Who acts: fix session.

### `crates/locust/tests/cli.rs:2817` (test gap)

The test named for the role that `goal add` carries never sends the request past its plan.

**How it goes wrong.** 1. `goal_add` reads the role back from the reviewed plan (crates/locust/src/cli/only_you.rs:766) and passes it in `Request::GoalInvite` (only_you.rs:788-792). Today this is right.
2. Suppose a later edit made that request send `role: None`. The stub test (crates/locust/tests/cli.rs:2817) runs `goal add` with `--plan` only; its `--confirm` branch is guarded by `if command == "invite"` (cli.rs:2885), and the stub would panic on the `InvitationInspect` and `GoalJoin` a confirmed add sends (cli.rs:2863). The only assertion for add is `plan["role"]` (cli.rs:2907), which still passes.
3. The only confirmed `goal add` against a real daemon is `reviewed_local_membership_uses_names_and_defaults_to_auto_without_tickets` (crates/locust/tests/t2_flow.rs:740), which creates the goal with `peer-review` (t2_flow.rs:753), where no role is expected. It still passes.
4. The core tests that prove an invite with `reviewer` makes a reviewer (crates/locust-core/src/node/tests/roles.rs:320) send `Request::GoalInvite` directly through the `join` helper (roles.rs:40), not through the CLI. They still pass.
5. Result: every suite is green while a review-panel `goal add` prints "as Juniper, a reviewer" and admits the agent with no role. `--role OTHER` is likewise never exercised for either command (the only flag the tests pass is `--no-role`, cli.rs:2880 and 2897).

**What the second readers concluded.** The gap is real: `add_and_invite_carry_the_role_the_rules_count_on` checks the role in the request only for `goal invite`; for `goal add` it checks the plan JSON alone, and no other automated test runs a confirmed `goal add` on a review-panel goal. The code is correct today, so this is a missing regression guard for a plan-specified assertion ("the request carries it"), not a present defect, and I rate it low rather than medium.

**Smallest fix.** Smallest and strongest: add a real-daemon case beside `reviewed_local_membership_uses_names_and_defaults_to_auto_without_tickets` in crates/locust/tests/t2_flow.rs. Create a goal with `--formation review-panel`, run `approved_cli(&["--owner"], &["goal","add","--goal",..,"--agent","bob"])`, and assert `goal status` lists bob under `goal_status.roles.reviewer`; then add a third agent with `--no-role` and assert it is not listed. That checks the request and the plan's exit criterion ("goal add of two more agents makes each a reviewer with no role give") in one place.

The reviewer's stub route in `add_and_invite_carry_the_role_the_rules_count_on` also works but needs more than written: the server count rises from 6, the stub must answer `InvitationInspect` with a full preview whose goal and governance match, answer `GoalJoin`, and then return a `GoalStatus` that lists the worker as a local member only after the join (only_you.rs:842 fails otherwise, and only_you.rs:692 refuses the next plan if it is listed before). The `invites == 0` counter at cli.rs:2857 would need rework too. One `--role other` case on `goal invite` in the existing stub is a cheap addition either way. No plan change is needed; the plan already asks for this.

**What a person sees.** Nothing today: adding an agent to a review-panel goal does make it a reviewer. The risk is only that a future change could break this without any test noticing; then the add would say "a reviewer" but the agent would hold no role, and results would stay uncounted until the host noticed and ran `role give`.

Reported by: tests-vs-plan:3, invitations-join:2. Who acts: fix session.

### `crates/locust-core/src/node/tests/roles.rs:290` (test gap)

No test covers a name or a role crossing two computers.

**How it goes wrong.** 1. Change peers.rs:248 so `joins()` signs any fixed valid name instead of `join.name`. Every cross-daemon test joins as the literal "member" or "Member" and none reads the admitted name back, so the host admits the wrong name and nothing fails. The plan (line 195) says a name cannot be changed after admission, so the wrong name would stay.
2. Delete `&& member.name == request.name` at peers.rs:334. Every test that retries a redeemed ticket re-sends the same name (tests/invitations.rs:327-334, replica_tests.rs:461-476), so nothing fails.
3. Delete the conflict at requests/invitations.rs:317-321. No test sends a second goal.join with another name while the first is pending, and no test mentions its message.
Not reachable as written: "a ticket's role dropped on the network redemption path only". `Host::join` (peers.rs:257-264) only calls `plan_join`, which takes the role from the stored `InviteRecord` (peers.rs:357), not from the request or the joiner's `JoinRecord`. The same-daemon test at roles.rs:238-265 reads `role == Some("reviewer")` out of the admission record that this shared function signs, so dropping the role there fails a test.

**What the second readers concluded.** The name half stands: no test would notice a change to the three lines that carry a joiner's chosen name between two computers (the re-sign in `joins`, the name comparison on a retried redeemed ticket, and the pending-name conflict in `goal_join`). The role half and the bad-name half do not stand as gaps: a ticket's role never travels with the joiner and is read from the host's own invite record in the one `plan_join` both paths call, and a bad name is refused at two tested layers.

**Smallest fix.** Tests change; code and plan stay. Add one test to crates/locust-core/src/node/replica_tests.rs on its existing two-daemon `Peer` + `reconcile` harness: peers[1] sends goal.join as "Maple"; a second goal.join on the same ticket as "Juniper" answers `conflict` (pins invitations.rs:317); `peers[1].restart()`; `reconcile`; assert `state().members[&agent].name == "Maple"` on both peers (pins peers.rs:248 and the stored `JoinRecord.name`); then `Host::join` on peers[0] with a `JoinRequest` re-signed as "Juniper" is `InvitationRefused` and re-signed as "Maple" is `Ok` with no new record (pins peers.rs:334). Putting `role: Some("reviewer")` on that ticket and a 65-byte-name request in the same test is cheap but optional; both are already pinned elsewhere.

**What a person sees.** Nothing today; the code does the right thing. The risk is later: if a change broke this path, someone joining from another computer under a name they chose could appear to everyone in the goal under a different name that cannot be changed afterwards, or a retried join could be refused, and the test suite would stay green.

Reported by: invitations-join:6. Who acts: fix session.

### `crates/locust-core/src/goal/tests.rs:2837` (test gap)

The test that review requests follow roles checks only what is wanted at the head, never a signed request.

**How it goes wrong.** All line numbers are at 8c086c1; this is from reading only, nothing was run.

1. The test's review half (tests.rs:2871-2876) posts a result on the stage task, admits workers[1], and asserts only that `f.goal().evaluation().desired_effects` holds a RequestReview for workers[1]. No request is signed and `f.replays()` is not used. That part of the finding is correct.

2. "Went back to the code this phase removed" does not pass silently.
   - The removed line was inside `review_templates`, which both `desired_effects` and `validate_effect` use. Restoring it drops workers[1] from the wanted set, and line 2875 of this same test fails.
   - If only the argument at flow.rs:343 changes to the result's anchor, the daemon signs a request it then excludes. `drive_flow` (node/flow.rs:39-49) signs each wanted effect, and `advance` (node/commit.rs:340-359) refuses any self-authored record its own replay does not find Effective, returning Conflict. That error travels through `land` (commit.rs:174) to the request (requests/mod.rs:138).
   - Two daemon tests admit a reviewer after a result that does not yet count, and their `GoalJoin` would fail: `review_panel_starts_with_the_host_as_sole_reviewer_and_counts_nothing_until_two_more` (node/tests/roles.rs:320, result at 323, joins at 325 and 336) and `stages_review_requests_and_admissions_need_no_local_work_setting` (node/tests/context_views.rs:1003, join at 1052).

3. What stays unpinned: if flow.rs:343 read the head instead of the effect's anchor, signing would still succeed everywhere. A request already signed to a member who is later removed, or whose role is taken, would become excluded. I found no test that would notice. The stage half of this test does exactly that check for a stage step (`opened` stays Effective after the role change); the review half has no equivalent.

4. The exit criterion at plan lines 3059-3061 ("events shows one new effect_materialized") has no test that counts it, and the build notes' walkthrough list omits it. "For the second result only" is covered at the wanted-effects level by tests.rs:2970 and tests.rs:2991.

**What the second readers concluded.** The test the companion names for this row (crates/locust-core/src/goal/tests.rs:2837) never signs a review request, so replay's rule for a signed request is not checked directly. A slip back to the removed behaviour would still be caught, by this test or by two daemon tests; what no test pins is that a signed request stays valid after its recipient is removed or loses the role.

**Smallest fix.** Test-only change; no code or plan change.

In `stage_recipients_and_review_requests_follow_roles_at_the_materialization_anchor` (tests.rs:2837 at the commit, 2844 in the tree now), after `admit_worker(&mut f, 1, None)`:
1. Take the wanted RequestReview for workers[1] and sign it with `f.host(Body::EffectMaterialized { effect })`. The runner on a stage task is the governance key, not the author.
2. Sign `f.host(Body::MemberRemoved { member: workers[1], admission: later, last_accepted: None })`.
3. Assert in `for goal in f.replays()` that the request's standing is `Standing::Effective`.

That one assertion fails if `validate_effect` reads the result's anchor (workers[1] was not yet a member) and fails if it reads the head (workers[1] is gone).

Optional: in roles.rs:320, after Maple joins, assert that `state().effects` holds a RequestReview addressed to her. That gives the exit criterion a direct check.

**What a person sees.** Nothing today. If the code slipped back to the old behaviour, adding an agent to a goal with a result awaiting review would fail with a conflict error and tests would go red at once; a reviewer would not be silently left unasked. The uncovered case is narrower: a review request already sent to someone who is then removed, or loses the reviewer role, could quietly drop out of the record. That person can no longer review anyway, so a user would notice little.

Reported by: tests-vs-plan:2. Who acts: fix session.

### `crates/locust-core/src/node/tests/roles.rs:199` (test gap)

The test for a role only earlier rules declare passes whether `role give` replaces the lead or adds to it.

**How it goes wrong.** 1. Goal created under `directed` (lead=[host], reviewer=[host]); Maple joins with no role; rules rebound to `peer-review`, which declares no roles (presets.rs:58-59), and `lead` keeps its list (chain.rs:536-539 only adds).
2. Suppose `role_change` (goals.rs:321) were changed to decide `deciding` from the current formation instead of `Self::deciding(entry)`. `lead` would then be treated as a group.
3. Give: `next` = [host, Maple] instead of [Maple]. Replay accepts it, because chain.rs:184-196 checks only non-empty, ascending, admitted.
4. Take Maple: the `deciding && member == host` guard does not fire, `next` = [host], one `RoleHolders` is signed.
5. Every assertion still holds: `roles["lead"] == [host]` (roles.rs:206), `status.deciding.contains("lead")` (roles.rs:207; read from `deciding_known` at goals.rs:398, not from role_change), and the post-restart list (roles.rs:209).
With the correct code the path is [host] -> [Maple] -> [host], the same end state. A regression inside the shared `deciding_known` would be caught by roles.rs:207; only a regression at role_change's own call site escapes. In the two-holder state `authority()` (goal/rules.rs:240-246) returns None, so nobody picks or closes on tasks still judged under `directed`.

**What the second readers concluded.** The test `role_give_and_take_work_on_a_role_only_earlier_rules_declare` never reads the holders between the give and the take, so it passes whether `role.give` replaced the lead or appended to it. The code is correct today; this is a test gap only, and nothing else in the tree pins replacement after a change of rules.

**Smallest fix.** In `role_give_and_take_work_on_a_role_only_earlier_rules_declare` (crates/locust-core/src/node/tests/roles.rs), between the two `change(..)` calls at lines 203 and 204, add `assert_eq!(status(&mut d, owner, goal).roles["lead"], vec![member]);`. It compiles as written because the local `let status` that shadows the helper comes later, at line 205. No production code or plan change is needed.

**What a person sees.** Nothing today: giving the lead role after a change of rules correctly leaves one lead. The missing check only means that if this were broken later, the tests would stay green, and a person would find that tasks started under the earlier rules can no longer have a result picked or be closed, with two leads listed.

Reported by: tests-vs-plan:4, node-roles-rules-bind:5, lens-earlier-phases:5. Who acts: fix session.

### `crates/locust-core/src/goal/workspace_tests.rs:1095` (test gap)

Two first-files tests the plan names assert less than the plan says.

**How it goes wrong.** 1. workspace_tests.rs:1094-1132: the test builds `alone(peer-review)`, posts the host's first files, an acceptance, and one change with a parent, and asserts both are approved with themselves as evidence. It has no `admit_worker` call and no second epoch. Plan lines 3000-3003 also ask for "after a second admission the next change waits, and first files in a fresh empty epoch still count".
2. workspace_tests.rs:1048-1093: after the `RetainBefore` epoch the test asserts only `workspace_proposals[&first_again].approved`. No `integrate` follows. Plan lines 2996-2999 say the new first files "count and land". Build-notes departure 9 says the test checks "restored empty acceptance"; it checks approval, not an acceptance record. Departure 9 does declare the dropped "two first acceptances dispute" clause.
3. workspace_tests.rs:973-993: asserts approved, evidence equals the change itself, and `standing(&accepted) == Effective`. It has no `head(&goal)` assertion. Plan lines 2983-2987 say "is effective and is the head".
4. What still covers the logic: the second-admission cutoff is pinned for task results at tests.rs:2734-2752, through the same `Verifier::approval` -> `resolve` path that reads the single member at the anchor (fold.rs:112-133, 990-1018; rules.rs:92-109 only swaps in the tree's completion rule). A host change with a parent waiting among several members is pinned at workspace_tests.rs:1012-1046 and node/tests/workspace_lifecycle.rs:830-881. First files counting in a restored-empty epoch among several members is pinned at workspace_tests.rs:1076-1092. Head after a first-files acceptance is pinned indirectly at workspace_lifecycle.rs:846-849: the next capture takes its parent from the head, so a missing head would make the second change first files and fail the `!approved` assert.
5. Not pinned anywhere: a file change under a tree rule that carries the only-member part, posted by the host's agent after a later admission, waits. The node test at workspace_lifecycle.rs:830 admits the second member before any file exists. The plan's later phase schedules exactly this (details.md:517, `a_goal_of_one_lands_its_own_changes_until_a_second_member_joins`, Phase 9).

**What the second readers concluded.** Three of the five first-files replay tests in crates/locust-core/src/goal/workspace_tests.rs assert less than the plan lists for them, and only one of the cuts is declared: `first_files_and_the_only_member_part_never_disagree` stops after the goal-of-one half, `first_files_are_once_per_epoch_that_starts_empty` checks that the new first files count but never signs their acceptance, and `the_hosts_first_files_count_as_posted_whatever_the_trees_rule` checks the acceptance is effective but not that it is the head. The gap is thin, because every missing piece of logic is pinned by a neighbouring test on the same code path; no behaviour is wrong.

**Smallest fix.** The code needs no change; extend three tests in crates/locust-core/src/goal/workspace_tests.rs.
- `first_files_and_the_only_member_part_never_disagree`: after the existing asserts call `admit_worker(&mut f, 0, None)`, post `f.agent(WorkspaceProposed { context, parent: Some(accepted), .. })` and assert it is not approved while `later` still is. Then open `epoch(&mut f, Some(context.round), WorkspaceCheckpoint::RetainBefore { epoch: context.round })`, post a host change with no parent there, and assert it is approved with itself as evidence.
- `first_files_are_once_per_epoch_that_starts_empty`: add `let landed = integrate(&mut f, empty, None, first_again, vec![]);` and assert `standing(&landed) == Some(Standing::Effective)` and `head(&goal) == Some(landed)`.
- `the_hosts_first_files_count_as_posted_whatever_the_trees_rule`: add `assert_eq!(head(&goal), Some(accepted));` inside the replays loop.
I traced all three additions through `decision`, `workspace_parent`, `boundary` and `approval` and each should pass as written; I did not run them.
If the tests stay as they are, departure 9 in research/v2-phase-r4-build-notes-2026-10-06.md should say "restored empty approval" and list the two undeclared cuts.

**What a person sees.** Nothing. Shared files behave as planned today. The cost is only that a later change could break "once a second member joins, the host's own file changes wait for approval" without a test in this file failing.

Reported by: tests-vs-plan:6, flow-closure-workspace:5, lens-convergence:5. Who acts: fix session.

### `crates/locust-core/src/goal/delegation.rs:122` (test gap)

No test fails if a subtask may name a different picker or closer than its parent.

**How it goes wrong.** 1. A definition declares roles `lead` and `deputy` and two task types: `top` with decisions.selection = Role lead, `sub` with decisions.selection = Role deputy. `validate_binding` (chain.rs:536-577) accepts it and gives both roles to the host's agent, one holder each. 2. A member opens a `top` task, then a `sub` task under it. `task_binding` (fold.rs:708-730) calls `narrows`; today `Some(Role deputy) != Some(Role lead)` so the child is excluded with "child task type does not prove narrower parent authority and completion". 3. Mutate the closure back to the pre-commit form (`rules::authority(child, c).is_some() && rules::authority(child, c) == rules::authority(parent, p)`, still compilable since rules.rs:239 exists) or to `true`: the child is now accepted because both roles resolve to the host's key at the opening anchor. 4. `role give ... deputy` to another member replaces the single holder. `decision` (fold.rs:875-889) resolves the subtask's rules with the role holders at the pick's own anchor, so that member's pick or close in the subtask is effective while the parent's lead never changed. 5. No test notices: delegation.rs tests (lines 160-247) all use DecisionRules::default() or inherit, so selection and finish are None; goal/tests.rs:1243 uses `..Default::default()` decisions (None); goal/tests.rs:3074 opens a child with task_type None under `directed`, inheriting the identical `Role lead`, which old, new and `true` all accept; the remaining subtask openings (goal/tests.rs:1370, 1562, 1574; tests/organizations.rs:753, 779) use Formation::default or the `open` preset (None). Stage tasks (flow.rs:53) have a Goal-scope parent and never reach `narrows`.

**What the second readers concluded.** The selection/finish half of `narrows` (the `authority` closure at crates/locust-core/src/goal/delegation.rs:122-126) is correct as written but no test reaches it with a child authority that is set and differs from the parent's; replacing the closure with `true` or restoring the pre-commit resolved-key comparison leaves every test in crates/ green. The plan's named test `subtask_narrowing_treats_roles_symbolically` pins only the selector half (work.propose via `Atom::Role`).

**Smallest fix.** Code stays as is; add one unit test in the `tests` module of crates/locust-core/src/goal/delegation.rs that calls `narrows` directly: parent.decisions.selection = Some(Authority::Role lead), with roles `lead` and `deputy` both mapped to the same single key on both sides; assert `!narrows` for child Some(Role deputy), `!narrows` for child Some(Participant { that same key }), `narrows` for child None and for child Some(Role lead); assert `!narrows` when the parent is None and the child is Some; repeat the same assertions for `finish`. Either mutant (old key comparison or `true`) fails the first assertion. No plan change needed; the plan (docs/roles-and-permissions-plan.md:2485-2487, details line 240) already states equality of the two Authority values.

**What a person sees.** Nothing today: a subtask cannot name a different picker or closer than its parent task. The risk is only for the future: if someone later edits this check, a subtask could quietly end up with a different person able to pick the winning result or close it than the parent task's lead, and no automated check would catch the change.

Reported by: rules-counting:2. Who acts: fix session.

