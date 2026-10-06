# Review of phase K1 as built, and of three parallel experiments

Status: research finding, 6 October 2026. A review by reading. Nothing was
built or run for it, apart from one assessor re-running the restore guard's
model and one re-running the file-identity script.

Phase K1 of the [v2 plan](../docs/master-plan.md) landed in `cb1acaa`, with
its model change in `0a4bbc2` and its
[build notes](v2-phase-k1-build-notes-2026-10-06.md). Five readers each took
one line through the change: the signed format and replay, the daemon's own
paths, conformance to the plan, what a person sees, and what was removed and
what the tests show. A second reader then tried to refute each finding.
Three more readers assessed the experiments that ran beside K1: the
[restore guard's model](restore-guard-model-2026-10-06.md), the
[file-identity measurements](restore-file-identity-2026-10-06.md) and the
[review of the public-goals plan](public-goals-plan-review-2026-10-06.md).
What all of them returned is kept in the
[evidence folder](evidence/v2-phase-k1-review-2026-10-06/README.md).

## Result for K1

The core of K1 held. The reader of replay could not break the first record,
the rule that the goal's key is never a member, the cost of a fork of the
host's agent, or "same records, same answer". No finding has two computers
disagree.

The readers reported 44 findings, which are 32 distinct places. The second
readers refuted none: they confirmed 35 reports and narrowed 9. None of the
code is changed yet.

| Kind | Where | Finding | Check |
| --- | --- | --- | --- |
| a goal stops for good | `locust-core/src/goal/chain.rs:451` | After a fork in the governance key's log, a member whose own log holds one record anchored on a cut governance record can never have another record count, and K1's cost statements and its test comment say only that members and rules stop. | confirmed |
| a goal stops for good | `locust-core/src/node/flow.rs:49` | drive_flow returns an error when one unattended step cannot be signed or landed, and start, a join and a received batch all pass that error on instead of passing the step over. | confirmed |
| security or trust boundary | `locust-core/src/node/requests/goals.rs:270` | A member's daemon refuses `goal leave` for any local agent the first record names as the host's agent, and a first record can name any key. | partly |
| security or trust boundary | `locust/src/cli/invitations.rs:233` | The ticket review tells the person the signature was checked against a key the goal identifier commits to, and inspection cannot check that. | confirmed |
| wrong result for a person | `locust-core/src/node/access.rs:258` | A rules refusal's JSON carries the governance key in the field named host, because Why::Rules.host is still filled from state().governance. | confirmed |
| wrong result for a person | `locust-core/src/node/requests/daemon.rs:144` | After agent.reconnect, deliveries that arrived while the agent was disconnected stay unreceived, because land_once projects deliveries before the cleared flag reaches memory. | confirmed |
| wrong result for a person | `locust/src/cli/workspace.rs:301` | workspace init typed with an agent's credential tells that agent the host's agent is disconnected when it is not. | confirmed |
| wrong result for a person | `sites/locust.farm/src/lib/formation-editor/ui/problems.ts:77` | The formation editor shows the sentence about "The author of the result" for K1's new stage diagnostic. | confirmed |
| work waits on a human | `locust-core/src/goal/flow.rs:266` | Automatic offers are built only for a stage task's opening round, so after a revision a start rule offered by task_creator can be met by nobody, where before K1 the creator was a member who could offer by hand. | confirmed |
| work waits on a human | `locust-core/src/goal/fold.rs:313` | A task revision can give a stage's task rules that name task_creator where a member must act, and replay accepts it, so K1's promise that no stage task is left that nobody can finish holds only at binding. | confirmed |
| work waits on a human | `locust-core/src/node/requests/tasks.rs:127` | `task revise` can move a stage's task onto rules that name `task_creator`, which since K1 is a key no member holds, and neither the daemon nor replay refuses it. | confirmed |
| work waits on a human | `locust/src/cli/mod.rs:591` | Once the Undo line has scrolled away, nothing a person meets names agent reconnect, and several refusals for a disconnected agent now say something untrue. | partly |
| departs from the plan | `locust-core/src/goal/flow.rs:40` | The stage runner is read from history.governance in two places, where the plan says chain.state.governance and says replay readers go through State. | confirmed |
| departs from the plan | `locust-core/src/node/definitions.rs:162` | In the daemon a rules binding whose formation fails validation is pending for a missing definition, not excluded as invalid, and it becomes the current rules. | confirmed |
| departs from the plan | `locust-core/src/node/requests/goals.rs:356` | rules bind and goal create refuse a stage rule that names task_creator only after the plan and the yes, with a generic sentence that names neither the diagnostic nor what to write. | confirmed |
| departs from the plan | `locust/src/cli/presentation.rs:559` | Two rendered texts print the governance key in full, and the test meant to catch this renders five hand-built responses that avoid both. | confirmed |
| departs from the plan | `locust/src/cli/presentation.rs:616` | `event show` of a goal's first record prints the governance key in text, and the test named for that rule does not render it. | confirmed |
| dead or leftover code | `locust-proto/src/api.rs:1241` | Two comments still say the key is shown to the joiner as a fingerprint, and one of them is published in the generated contract. | confirmed |
| test gap | `locust-core/src/goal/tests.rs:1913` | Three of the K1 replay tests run forward only, no test replays an automatic offer signed by the governance key, and no test forks the key's log between two stage steps. | partly |
| test gap | `locust-core/src/node/replica_tests.rs:239` | `host_operations_refuse_a_daemon_that_does_not_hold_the_host_key` does not read `hosted_here`, and no test asserts it false on a real daemon. | confirmed |
| test gap | `locust-core/src/node/requests/daemon.rs:145` | `agent_reconnect` refreshes a goal's deliveries before the agent counts as connected, and the plan's assertion that a waiting step is signed after a reconnect was not written. | confirmed |
| test gap | `locust-core/src/node/requests/levels.rs:118` | The new governance branch of `Node::stalled` has no test. | confirmed |
| test gap | `locust-core/src/node/tests/authorization.rs:195` | The plan's test `disconnecting_the_hosts_agent_stops_no_host_command` does not exist, the build notes do not say so, and five host operations are never run with the host's agent disconnected. | confirmed |
| test gap | `locust-core/src/node/tests/authorization.rs:643` | The plan's test `disconnecting_the_hosts_agent_stops_no_host_command` does not exist and the build notes do not say so. | partly |
| test gap | `locust-core/src/node/tests/authorization.rs:695` | The reconnect test does not assert that a step that waited for the agent is signed, and no test runs task revise, workspace epoch or farm on with the host's agent disconnected. | confirmed |
| test gap | `locust-core/src/node/tests/farm.rs:406` | `publication_needs_no_consent_from_the_governance_key` passes with K1's exemption removed, because in its fixture the key signs only kinds that never needed consent. | confirmed |
| test gap | `docs/reference/conformance/organization.cases.json:265` | The site mirror of the stage check is pinned by one shared case, and the reconnect command's no-change answer has no test. | confirmed |
| test gap | `research/tla/Organization.tla:253` | The Organization model has no record kind for a host step, so the one new path by which a signer that is no member has work count is not modelled, and the new invariant says more than replay does. | confirmed |
| wording | `locust-core/src/node/authoring.rs:208` | At a halt of the goal's key, host commands that sign through next_place answer `unavailable` with `the goal's history has not arrived yet`. | confirmed |
| wording | `locust/src/cli/presentation.rs:151` | goal status prints Host: you to whoever asks, an agent's credential included. | confirmed |
| wording | `docs/guide/help.md:31` | The guide still defines the host as a member and says the starting agent becomes the host. | partly |
| wording | `docs/master-plan.md:3` | The tracked plans still say K1 is not built. | partly |

## The findings in full

### `crates/locust-core/src/goal/chain.rs:451` (a goal stops for good)

After a fork in the governance key's log, a member whose own log holds one
record anchored on a cut governance record can never have another record
count, and K1's cost statements and its test comment say only that members and
rules stop.

**How it goes wrong.** Keys: K (governance), Maple (host's agent), Juniper on
a second computer. K's log: 0 first record, 1 admits Maple, 2 rules, 3 admits
Juniper, 4 admits Cedar, 5 opens a stage's task. Juniper's log position 0 is a
result anchored at record 4, the head on her computer. The host's computer is
restored from a copy that ends at position 3. Before it hears from Juniper its
daemon signs the stage step at position 4 with nobody present
(node/flow.rs:42, node/authoring.rs:165-173). Every computer that holds both
records at position 4 cuts the chain to positions 0 to 3
(goal/history.rs:80-86, chain.rs:66, chain.rs:235-247). Juniper's result is
Pending(Anchor) because record 4 has no position (chain.rs:409-412). Every
later record Juniper signs, also one anchored at record 3, walks back to that
result and is Pending(Anchor) (chain.rs:451-455). The test at
goal/tests.rs:2178-2188 pins exactly this. Goal::next still gives her a place
(goal/mod.rs:264-279, tests.rs:2181), so her agent keeps working and nothing
it signs counts. She cannot be removed and admitted again, because the key
signs nothing more. The plan's plain-words cost (plan lines 344-345) and K1's
risk note (plan lines 1466-1472) list only joins, removals, rules and ending.
tests.rs:1981 says the halt is not a blanket exclusion and shows it with a
member who had signed nothing after the cut. The replay rule is older than K1.
K1 keeps it and puts records the daemon signs by itself in that log. Read, not
run.

**After the check.** Holds as stated. The plan already says governance stops
for good. What it does not say is that every member whose own log holds a
record anchored on a cut governance record is silenced for good while its
agent keeps signing. That is every member who signed anything after receiving
a governance record that the fork later cuts. Add plan lines 3430-3432 to the
places to correct: they say work signed in a halted goal counts everywhere.

**Smallest fix.** Say it where the cost is stated: plan lines 340-350, K1's
risk note at line 1466, G2's halt line, and the comment at goal/tests.rs:1981.
Count it when the one key or two keys choice is taken again: a fork in a
separate log of host steps cuts no governance record and silences no member.
Changing replay so a record anchored in the surviving chain is judged without
its ancestor on a cut record is a design change, not a small fix.

### `crates/locust-core/src/node/flow.rs:49` (a goal stops for good)

drive_flow returns an error when one unattended step cannot be signed or
landed, and start, a join and a received batch all pass that error on instead
of passing the step over.

**How it goes wrong.** Verified by reading. The figure of 500 is computed, not
run. A hosted goal has about 500 members or more. The host binds the built-in
pipeline formation, whose stages name `members` as recipients
(crates/locust-proto/src/organization/presets.rs:99 and 107). The first
stage's step lists every member at the binding as a recipient
(crates/locust-core/src/goal/flow.rs:63-70). At 32 bytes a key its header
passes 16 KiB (crates/locust-proto/src/limits.rs:11). The gate in
flow.rs:22-34 tests only that this computer hosts and that Goal::next answers,
so the step is chosen. author_alone reaches Event::sign, which answers
TooLarge (crates/locust-proto/src/event.rs:876-878), and flow.rs:49 returns
it. What follows: (1) `rules bind` answers an error after the binding is
committed (commit.rs:172-175, requests/mod.rs:138). (2) Every later commit in
that goal does the same. Host::join answers InvitationRefused after the
admission is durable (peers.rs:257-258), the joiner marks its ticket refused
(peers.rs:277-288), and the host lists the joiner as a member. (3) A received
batch is stored and answered ProtocolError (replica.rs:215). (4) At the next
start Node::open fails for the whole daemon (mod.rs:210-211), so every goal on
the host's computer is down and no command repairs it. The same holds for any
other reason a step cannot be signed or does not come out effective (the
Conflict from commit.rs:340-359). The two `?` predate K1. K1's rule point 2
says such a record is passed over and is never an error, and names this path
as the place that enforces it. I found no second trigger in normal use.

**After the check.** The defect holds as stated. Two corrections to its
support. First, rule point 2 does not cover this case. Its text says an end, a
hold of the restore guard and a halt are tested before the signature and are
passed over (docs/host-safety-and-ending-plan.md:288-292). A step that can
never be signed is not in that list, so this is a robustness defect older than
K1 and not a breach of K1's rule as written. Second, the trigger needs a
private goal with about 500 current members at the moment a stage that names
`members` is bound. The plans test nothing above 16 members and their ceiling
of 16 is for public goals and is not built yet (docs/master-plan.md:275,
docs/joinable-farms-plan.md:2460-2462, 2559-2560 and 4979). The cost when it
does happen is as the finding says, and after a restart it takes every goal on
the host's computer.

**Smallest fix.** In drive_flow, when author_alone fails, or land_once fails
while self.failed is not set, keep the effect id in a skipped set for this
pass, go on to the next desired step, and return Ok. Show the skipped step
through `stalled`. Add a test with a step that cannot be signed and assert
that open, Host::join and Replica::receive still succeed.

### `crates/locust-core/src/node/requests/goals.rs:270` (security or trust boundary)

A member's daemon refuses `goal leave` for any local agent the first record
names as the host's agent, and a first record can name any key.

**How it goes wrong.** Read, not run. It needs a host whose client departs
from the daemon. Header::check asks only that `host` differs from `governance`
(crates/locust-proto/src/event.rs:786-795). A host starts a goal whose first
record names Juniper's key as `host`. Juniper is another person's agent and
its key is visible in a goal both share. The host admits its own agent and
invites Juniper's person. After Juniper joins, `goal leave --agent juniper` on
Juniper's computer answers conflict, `the host's agent cannot leave its own
goal` (goals.rs:270-273). Replay excludes every removal of that key
(goal/chain.rs:134-137). So Juniper can neither leave nor be removed from that
goal. A ticket for a second agent on Juniper's computer is refused at
invitations.rs:246-255, because the host's admission it compares with is
Juniper's own. The plan says cannot leave is the host daemon's refusal. The
code applies it on every computer.

**After the check.** A member's daemon does refuse `goal leave` for a local
agent that a first record names as host's agent, replay never removes that
key, and a first record may name any key but the goal's own. Reaching it needs
a host that builds a first record its own daemon never builds, which owner
answer 8 puts outside the design. The code matches the plan's Changes text, so
it is not a departure. It is a hardening gap, not a trust boundary the plan
promises. The proposed test `&& self.hosts(entry)` is harmless and closes it
until E2, when replay itself refuses such a leave.

**Smallest fix.** Refuse only where this daemon holds the goal's key:
`entry.state().host == Some(principal) && self.hosts(entry)`. Add a test in
which a goal hosted elsewhere names a local agent as host's agent and that
agent leaves.

### `crates/locust/src/cli/invitations.rs:233` (security or trust boundary)

The ticket review tells the person the signature was checked against a key the
goal identifier commits to, and inspection cannot check that.

**How it goes wrong.** Verified by reading. Someone builds a ticket that
carries the real identifier of "Parser cleanup", names their own key and their
own endpoint, and signs it with that key. Invitation::verify checks the
signature only against the key the ticket itself names
(crates/locust-proto/src/invite.rs:261). The identifier hashes governance,
host, definition and salt (crates/locust-proto/src/event.rs:84 to 90) and a
ticket carries only the first, so inspect cannot recompute it. `locust
invitation inspect` still prints "Signature: verified against the goal's key,
which the identifier commits to." The person joins, the daemon presents the
ticket at the forger's endpoint, and the agent stays joining. The mismatch is
found only when a first record arrives
(crates/locust-core/src/node/replica.rs:204), and the test at
crates/locust-core/src/node/tests/invitations.rs:490 says such a ticket is
"redeemed in good faith". Before K1 the review printed the key as a
fingerprint the person could compare and claimed only "verified against the
host key". The plan's K1-5 text says only "The ticket's signature is
verified." (docs/host-safety-and-ending-plan.md:460). The build notes declare
the new line (research/v2-phase-k1-build-notes-2026-10-06.md:157) as a
replacement of wording and do not say it asserts a check that is not made. The
same line, and docs/guide/collaboration.md:164, also give the person a word
for the key ("the goal's key"), against plan lines 871 and 893 to 894.

**After the check.** Three texts a person reads name the key or the word
governance, against the plan's rule that a person reads no word for it; two
were written by K1 and are disclosed in the build notes. The inspect line
states the identifier commits to the key, which inspection cannot check, but
the next printed line says goal authority is confirmed during joining, and a
join with a mismatched key does not complete (replica.rs:204,
event.rs:786-795). So this is wording only, with no wrong result.

**Smallest fix.** Print the plan's sentence, "Signature: verified.", and keep
the next line that says goal authority is confirmed during joining. In
docs/guide/collaboration.md:164 say the ticket contains the goal title and
your IP addresses.

### `crates/locust-core/src/node/access.rs:258` (wrong result for a person)

A rules refusal's JSON carries the governance key in the field named host,
because Why::Rules.host is still filled from state().governance.

**How it goes wrong.** Verified by reading. Goal "Parser cleanup" under
peer-review, hosted here, members Maple and Juniper. Juniper's agent calls
locust_review_record on its own result through MCP. allowed_keeping builds
Why::Rules { host: entry.state().governance.unwrap_or(principal), host_name:
None } (access.rs:254-260). Since K1 state().governance is the goal's own key.
The MCP bridge hands the details to the model
(crates/locust/src/mcp.rs:551-553) and --json prints them
(crates/locust/src/cli/mod.rs:159). So the agent, and any script, reads
error.details.why.host as the governance key. That key is in no
members[].member and is not the host that goal status reports. K1 moved the
sibling Abilities.host to state().host (requests/levels.rs:94). The plan says
a key-typed field called host always means the host's agent
(docs/host-safety-and-ending-plan.md:902) and that Why::Rules.host stays a key
of that kind (lines 980-981). git blame puts the line in Phase 3 (3196be84);
K1 did not touch it. No test reads the field: every test matches Why::Rules {
.. }. The phase that prints the host's name in a refusal will look this key up
among the members and find nobody.

**Smallest fix.** Write host: entry.state().host.unwrap_or(principal) at
access.rs:258. Add one assertion in node/tests/levels.rs that a rules
refusal's host equals goal status's host and is not its governance.

### `crates/locust-core/src/node/requests/daemon.rs:144` (wrong result for a person)

After agent.reconnect, deliveries that arrived while the agent was
disconnected stay unreceived, because land_once projects deliveries before the
cleared flag reaches memory.

**How it goes wrong.** Verified by reading. Maple is disconnected. The host
binds pipeline and the first stage's task opens with Maple as a recipient. The
delivery record is written with received false, because delivery.rs:82-87 asks
principals.active. The person runs `agent reconnect --agent maple`.
agent_reconnect writes the record and touches the goal (daemon.rs:142-149).
land_once calls project_deliveries at commit.rs:242 while self.principals
still says revoked. The flag is absorbed only at commit.rs:251-255. land then
runs drive_flow and nothing else (commit.rs:173-175). Maple's pending work
leaves the delivery out (views.rs:510-513). If Maple is on a member's
computer, the host sends the delivery again at each exchange and gets no
receipt, because replica.rs:129-141 needs record.received. It stays so until
another record lands in that goal or the daemon restarts (mod.rs:206-208). The
task itself is still listed under to_start. The comment at daemon.rs:132-133
says the touch is what makes waiting things happen. That holds for steps and
not for deliveries.

**After the check.** Holds. One narrowing and one widening. If the reconnect
lets drive_flow sign a step in that goal, the step's own land_once
(node/flow.rs:50) projects deliveries with the cleared flag and the gap closes
at once. So it shows only when no step was waiting, which is the pipeline case
in the finding. And the gap closes at any later commit that names the goal,
not only a goal record: a level change, a delivery receipt
(replica.rs:162-165) or a received batch all reach commit.rs:242.

**Smallest fix.** In Node::land, after land_once and before drive_flow, run
project_deliveries again for each goal and land the result, as Node::open does
at mod.rs:206-208. It writes nothing when nothing changed. Add to
a_disconnected_agent_is_connected_again_with_its_name_and_key a delivery that
arrives while the agent is disconnected and assert it is received right after
the reconnect.

### `crates/locust/src/cli/workspace.rs:301` (wrong result for a person)

workspace init typed with an agent's credential tells that agent the host's
agent is disconnected when it is not.

**How it goes wrong.** Verified by reading. On the host's computer Juniper's
agent, a second local member, runs locust workspace init --goal "Parser
cleanup" --empty with its own credential. run() calls hosts_agent for every
caller (workspace.rs:174-176), before the owner test at line 235. hosts_agent
asks status with the caller's credential, and the daemon lists only the caller
to an agent (requests/daemon.rs:18-26). Maple's key is not in that list, so
agent.is_none_or(..) is true and the command prints <Maple's 64-hex key> is
disconnected; only <key> can share this goal's first files, and Connect it
again: locust --owner agent reconnect --agent <key> (workspace.rs:299-311).
Maple is connected. The agent relays this to its person, who runs the command
and reads maple is not disconnected. Nothing changed. The skill tells agents
that init is the person's command, so the case needs an agent that tries it
anyway.

**Smallest fix.** On the workspace.init path, refuse a caller without --owner
with the owner-only sentence before hosts_agent runs. Or, in hosts_agent,
treat an agent missing from the list as unknown and not as disconnected.

### `sites/locust.farm/src/lib/formation-editor/ui/problems.ts:77` (wrong result for a person)

The formation editor shows the sentence about "The author of the result" for
K1's new stage diagnostic.

**How it goes wrong.** Verified by reading. K1 mirrors the stage check in
rules.ts with the message `Stage "s" opens its task from the host's computer,
which is no member, so task_creator at … can be met by nobody`
(sites/locust.farm/src/lib/formation-editor/contract/rules.ts:231). The
editor's plain sentence for selector_scope picks its text with
`diagnostic.message.includes('task creator')`. The new message spells it
`task_creator`, so the test is false. A person loads a formation whose step
lets the task creator declare. The problem list reads: "The author of the
result" can only be used in a "when does a result count" rule. That is about
another selector. "Show me" goes to the step's add point, because the path is
/flow/NAME with no third part (problems.ts:50 to 51), and the rule at fault is
under work or counts. The conformance test compares only the contract layer,
so nothing fails. Inferred, not run: that the editor can load such a formation
(decode.ts accepts task_creator; the UI offers no button for it).

**Smallest fix.** In `sentence`, branch first on the stage message (for
example `includes("host's computer")`) with a sentence that says a step's task
is added by the host's computer, so name members or a role.

### `crates/locust-core/src/goal/flow.rs:266` (work waits on a human)

Automatic offers are built only for a stage task's opening round, so after a
revision a start rule offered by task_creator can be met by nobody, where
before K1 the creator was a member who could offer by hand.

**How it goes wrong.** Formation: stage research uses a type whose only start
is offered by task_creator to members. K1's check allows this on purpose
(validation.rs:175-176, plan line 1032). K opens the task and signs four
offers. The person runs task revise with the same type. offer_templates takes
the round from stage_instance, which returns the opening record
(flow.rs:89-99, 266-270), so every offer still names the old round. An attempt
in the new round must name the current round (fold.rs:368, 818) and an offer
whose context equals it (fold.rs:375-378), so no old offer can be accepted. A
member's hand-made offer is refused because the member is not the creator
(fold.rs:348-361, rules.rs:205). K cannot sign a WorkOffered
(chain.rs:421-423, event.rs:524-526), and an Offer step by K for the new round
is refused as not configured (flow.rs:313-321). Nobody can start the task
until the person revises to another type or binds rules again. The plan names
this limit for subtasks (plan line 1047) and not for a revised round. No
preset uses an offered start, so only hand-written formations meet it. Read,
not run.

**After the check.** Holds as stated, with one correction. A preset does use
an offered start (coordinator, by a role), but no preset combines a stage with
a start offered by task_creator, so only hand-written formations meet the dead
end. With such a stage, one task revise leaves a round that nobody can start
or post a result in until the person revises to another type or binds rules
again.

**Smallest fix.** Build the offers for the round that is current at the step's
anchor (current_round) instead of the opening record. If that is too much for
this phase, write the limit beside the subtask limit in the plan and in
docs/formations.md.

### `crates/locust-core/src/goal/fold.rs:313` (work waits on a human)

A task revision can give a stage's task rules that name task_creator where a
member must act, and replay accepts it, so K1's promise that no stage task is
left that nobody can finish holds only at binding.

**How it goes wrong.** Formation: the goal's own completion rule is a
declaration by task_creator. Task type staged sets its own completion, reviews
by members. Stage draft uses type staged. This passes K1's check, which reads
the type's decisions when the type sets them
(organization/validation.rs:189-195). The host's computer opens the draft task
with key K. The person runs locust --owner task revise for that task with no
--task-type. The CLI sends task_type None (cli/only_you.rs:104, 1275-1279) and
the daemon writes it into the binding (node/requests/tasks.rs:127). Replay
accepts the revision: the TaskRevised arm resolves the binding and runs no
stage check (fold.rs:313-324, fold.rs:679-692). The new round falls back to
the goal's own rules. Its creator is K, found by walking back to the opening
step (goal/rules.rs:56-62, 163-189). A member's declaration is refused because
the member is not the creator (rules.rs:205). K's declaration is NotAMember
(chain.rs:418-423). The round never completes and later stages never open
until the person revises again with --task-type staged. The same happens with
--task-type naming any type whose rules name task_creator. Read, not run.

**After the check.** Holds as stated. One narrowing: later stages wait only if
they require completion of the revised stage, or evidence that depends on it.
The revised task itself can never complete either way. The way out is a second
revise that names the type, which a person must notice and run.

**Smallest fix.** In the TaskRevised arm, when the task's creator is the
governance key, refuse a binding whose resolved rules name task_creator in an
independent start, in the to of an offered start or in a completion criterion.
And in task_revise keep the round's current task type when none is given.

### `crates/locust-core/src/node/requests/tasks.rs:127` (work waits on a human)

`task revise` can move a stage's task onto rules that name `task_creator`,
which since K1 is a key no member holds, and neither the daemon nor replay
refuses it.

**How it goes wrong.** Verified by reading, not run. A formation sets
`decisions.completion` to `declaration by task_creator` for ordinary tasks and
gives its stage `draft` a task type with its own review rule.
`stage_task_creator` reads only the rules the stage's own type gives
(organization/validation.rs:177-195), so the binding is valid. The host runs
`locust --owner task revise --task <the draft stage's task>` with no
`--task-type`. The CLI sends `task_type: None` (cli/only_you.rs:1254) and the
daemon writes it over the stage's type (tasks.rs:127). Replay accepts the
revision: the `TaskRevised` arm checks the round and the binding and no
selector (goal/fold.rs:313-324, 679-727). The new round resolves with the
formation's own completion rule and with the governance key as creator
(goal/rules.rs:56-63). `task_creator` then matches only that key
(rules.rs:205), and a declaration signed by it is `NotAMember`
(goal/chain.rs:418-423). No member can finish the task and the next stage
never opens. `--task-type` naming any type that no stage uses and whose rules
name `task_creator` does the same. Before K1 the creator was the host's agent,
a member, and it could declare. The agents read `not eligible under this
goal's rules` until the host notices and revises again. K1's own rewrite of
`revised_stage_round_keeps_its_runner_as_creator_despite_a_member_copy_of_the_effect`
(goal/tests.rs:1708) shows the creator stays the key after a revision.

**Smallest fix.** In `task_revise`, refuse a revision of a task whose creator
is `state().governance` when the resolved rules name `task_creator` in an
independent start's `by`, an offered start's `to` or a completion criterion.
Reuse `names_task_creator` and `completion_task_creator` from validation.rs.
Put the same test in the `TaskRevised` arm of fold.rs so every computer
agrees, with one replay test.

### `crates/locust/src/cli/mod.rs:591` (work waits on a human)

Once the Undo line has scrolled away, nothing a person meets names agent
reconnect, and several refusals for a disconnected agent now say something
untrue.

**How it goes wrong.** Verified by reading. A host with one agent runs agent
revoke --agent maple and returns days later. locust --owner status says
Participant maple (...) · credential revoked (presentation.rs:497). goal
status lists Maple as an ordinary member with its level line and, for a
waiting review request, Step ... stalled for maple (...): runner revoked
(presentation.rs:532; proto api/level.rs:152). locust --owner task open --goal
"Parser cleanup" 'x' answers: none of your agents is in Parser cleanup; add
one with locust --owner goal add --goal 3d9b6f20 --agent NAME
(cli/mod.rs:563-593). Maple is in it. Following that advice, goal add --agent
maple answers not_found: select an active enrolled local agent
(only_you.rs:586-591). Naming Maple on farm consent, level, allow or any agent
command gives no enrolled principal has that key (callers.rs:137-141,
authoring.rs:193) or no active enrolled principal has that key
(access.rs:352). locust up answers the credential is unknown or revoked, or
the onboarding name belongs to a different or revoked credential; preserve the
journal and identity (node/mod.rs:331; installation/onboarding.rs:441-451).
locust doctor advises: For a revoked agent, review owner enrollment
(cli/doctor/profile.rs:184). locust agent --help lists reconnect with no
description (only_you.rs:107-108 give no about). The agent itself reads the
credential was revoked (callers.rs:170). Only the revoke's own output and the
workspace init refusal name the command. Until the person finds it, Maple's
work, the roles only it holds, the first files and its page consent wait (plan
lines 1553-1566). No phase owns these texts. E2 adds lines to agent revoke
only.

**After the check.** Not a case of work waiting on a human by the system's
doing, and not a departure from K1's plan. It is a wording and findability gap
that no phase owns: after agent revoke, texts that predate agent reconnect do
not name it, and one is now untrue. For a goal whose only local member is
disconnected, an owner command says none of your agents is in the goal and
advises goal add, which then answers not_found (cli/mod.rs:581-593,
cli/only_you.rs:585-590). status still says credential revoked, and doctor and
the onboarding conflict speak of a revoked credential as final.

**Smallest fix.** One shared sentence: NAME is disconnected. Connect it again:
locust --owner agent reconnect --agent NAME. Use it in acting_agent when the
named agent is revoked or the only agents in the goal are revoked, in
add_plan, in the doctor remediation and in the onboarding conflict. Print
disconnected in status. Give agent revoke and agent reconnect an about line.

### `crates/locust-core/src/goal/flow.rs:40` (departs from the plan)

The stage runner is read from history.governance in two places, where the plan
says chain.state.governance and says replay readers go through State.

**How it goes wrong.** Plan line 1019 says stage_template and review_templates
take the runner from self.chain.state.governance. Plan line 1629 says replay
readers go through State.governance, and names screen and the join check as
the only two that read the first record itself. The code reads
self.history.governance at flow.rs:40-43 and flow.rs:216-218. The K1 diff
changed only the comment at flow.rs:39. Today both values are equal on every
computer (chain.rs:56-59), so nothing differs in v2. When replacing a host
lands and Chain::build takes a new key at the place the plan left for it,
these two readers would still name the first record's key as the runner. Every
computer would then refuse the new host's stage steps with 'effect signer is
not its configured runner' (flow.rs:323-325) and accept the old key's.

**After the check.** Holds as a departure from the plan's text with no effect
in v2. The refused-steps scenario can only arise in a later release that
changes Chain::build and still leaves these two readers, and the plan says
that release does not read v2 goals.

**Smallest fix.** Read self.chain.state.governance in both places.

### `crates/locust-core/src/node/definitions.rs:162` (departs from the plan)

In the daemon a rules binding whose formation fails validation is pending for
a missing definition, not excluded as invalid, and it becomes the current
rules.

**How it goes wrong.** Verified by reading. A host whose daemon does not run
checked_definition binds a formation in which a stage's task lets task_creator
declare. That is another build inside protocol 7 or a changed client. On every
other computer Definitions::read drops the formation because inspected.valid
is false (definitions.rs:160-167). validate_binding then finds no definition
and answers Pending(Definition) (goal/chain.rs:493-496), never reaching
InvalidDefinition at chain.rs:497-498. chain.rs:187-196 makes a binding that
is not excluded the current rules. Work under the earlier rules stops and
waits for a definition that is already held. K1's text says such a binding is
InvalidDefinition on every computer, which would keep the earlier rules. The
one test of it,
rules_whose_stage_task_names_the_task_creator_are_excluded_in_replay
(goal/tests.rs:2146), passes because its fixture hands replay the formation
directly and never goes through Definitions::read. An honest K1 host cannot
sign such a binding (requests/goals.rs:149 and 351-358). The same path decides
every later tightening of the validator inside protocol 7.

**After the check.** Holds. Two corrections. First, the smallest fix does not
work as written. inspect returns no normalized formation and no hash when
validation reports anything (crates/locust-core/src/organization.rs:65-73 and
116-119), so the `inspected.valid` test at definitions.rs:162 is already
redundant and dropping it changes nothing. A fix has to decode, normalize and
hash the formation without the validity run, or carry an 'held but invalid'
mark that validate_binding reads. Second, what stops is narrower than 'work
under the earlier rules'. Goal and document posts and new top-level tasks must
name the rules current at their anchor (goal/fold.rs:282-296 and 823-832) and
then wait on the definition. Tasks already open resolve through the rules
their own binding names (goal/rules.rs:109-117) and go on.

**Smallest fix.** In Definitions::read keep a formation that decodes and whose
hash matches even when inspected.valid is false, so valid_definition at
chain.rs:497 excludes the binding. Add a node test in which a daemon receives
such a binding and reads `excluded` with the earlier rules still current.

### `crates/locust-core/src/node/requests/goals.rs:356` (departs from the plan)

rules bind and goal create refuse a stage rule that names task_creator only
after the plan and the yes, with a generic sentence that names neither the
diagnostic nor what to write.

**How it goes wrong.** Verified by reading, and the build notes say so
(research/v2-phase-k1-build-notes-2026-10-06.md:145-153). The host runs locust
--owner rules bind --goal "Parser cleanup" --formation-json "$(cat f.json)"
where the draft stage's task counts on a declaration by task_creator.
rules_bind builds and shows the plan first
(crates/locust/src/cli/only_you.rs:1143-1161). The person answers y. The
daemon answers invalid: the formation is invalid; validate it for diagnostics
(goals.rs:351-357), exit 6. The person must then find and run locust formation
validate f.json, which prints the inspection as JSON. There the message and
the correction are good: Stage "draft" opens its task from the host's
computer, which is no member, so task_creator at ... can be met by nobody, and
Name members, a role or a participant in the rules this stage's task uses
(organization/validation.rs:218-220). The plan's exit criterion says the bind
itself is refused with the diagnostic selector_scope (plan lines 1443-1445).
One confirmation is spent on a command that cannot succeed, and the refusal
the person reads does not say what to write. goal create --formation-json
behaves the same way.

**Smallest fix.** In rules_bind and goal_create, run
locust_core::organization::inspect on the chosen source before building the
plan (cli/formation.rs already links it) and fail with the first diagnostic's
code, message and correction.

### `crates/locust/src/cli/presentation.rs:559` (departs from the plan)

Two rendered texts print the governance key in full, and the test meant to
catch this renders five hand-built responses that avoid both.

**How it goes wrong.** Verified by reading, not run. In any goal with a stage,
`locust task show --task <stage task>` prints `Effective rules:
{..."creator":"<64 hex digits of the governance key>"...}`. `task_detail`
serializes `EffectiveRules` with its `creator` field (node/views.rs:183-189,
goal/rules.rs:12-20). A stage task's creator is the signer of its opening step
(rules.rs:64-75), and goal/tests.rs:2034 asserts that it is the governance
key. presentation.rs:559 prints the JSON as text. In every goal, `locust event
show --event <first record>` prints `Author: host` and then `Record:
{"genesis":{"governance":"<the key>",...}}`, because only three body kinds
have their own arm (presentation.rs:612-616) and `Genesis` carries the key
(locust-proto/src/event.rs:71-81). Both happen on every member's computer. The
plan says no rendered text prints the key
(docs/host-safety-and-ending-plan.md:893-894 and 1168-1169).
`rendered_text_never_names_the_governance_key` passes because its Task fixture
has `effective_rules_json: "{}"` (presentation.rs:808) and its Event fixture
is a stage step (presentation.rs:795). The plan describes that test as
rendering every response fixture (plan line 1383).

**Smallest fix.** In the `Response::Task` arm, print the rules without
`creator` when `view.by_host`, or print `host` in its place. Add a
`Body::Genesis` arm to the `Response::Event` match that prints the host's
agent and no key. Give the test a real `Genesis` body and the effective rules
JSON of a replayed stage task.

### `crates/locust/src/cli/presentation.rs:616` (departs from the plan)

`event show` of a goal's first record prints the governance key in text, and
the test named for that rule does not render it.

**How it goes wrong.** Verified by reading. `events` lists `genesis · host ·
effective`. The person runs `locust --owner event show` on that identifier.
The Event arm prints every body it has no sentence for as `Record: {JSON}`.
For the first record that is `Record:
{"genesis":{"governance":"a94f…","host":"51c2…",...}}`: the key in full, under
the word governance. The plan says no rendered text prints the key
(docs/host-safety-and-ending-plan.md:1168 to 1169) and that the test "renders
every response fixture as text" (line 1383).
`rendered_text_never_names_the_governance_key` (presentation.rs:852) renders
five hand-built responses from `hosted()` (line 719). Its one event detail is
an effect_materialized body, which holds no key. So the test passes while a
real command prints the key. The build notes do not declare this.

**Smallest fix.** Give Body::Genesis its own line in the Event arm (the host's
agent's label, no key), add a genesis EventDetail to `hosted()`, and have the
test also fail on the key under any field name.

### `crates/locust-proto/src/api.rs:1241` (dead or leftover code)

Two comments still say the key is shown to the joiner as a fingerprint, and
one of them is published in the generated contract.

**How it goes wrong.** Verified by reading. `Response::Joined.governance` is
documented "The host's key, to show as a fingerprint." and
`Invitation.governance` "Shown to the joiner as a fingerprint"
(crates/locust-proto/src/invite.rs:115 to 117). K1 removed both displays
(cli/mod.rs join result, cli/invitations.rs ticket review) and the companion
lists "Any printed host key in the ticket review and the join result" as
removed (details line 50). The first comment is a schema description, so
docs/reference/generated/runtime.contract.json:16563 tells a script author
that this field is the host's key and is meant for display. It is the goal's
key and the plan forbids displaying it.

**Smallest fix.** Reword both comments (the goal's signing key; in JSON only,
never printed) and regenerate the contract.

### `crates/locust-core/src/goal/tests.rs:1913` (test gap)

Three of the K1 replay tests run forward only, no test replays an automatic
offer signed by the governance key, and no test forks the key's log between
two stage steps.

**How it goes wrong.** Plan lines 1261-1262 say each new replay test runs
forward, reversed and reloaded.
a_fork_in_the_governance_log_retracts_later_governance_and_preserves_prefix_work
(tests.rs:1913-1988) and
a_review_fork_by_the_hosts_agent_costs_what_a_members_fork_costs
(tests.rs:1991-2031) apply the fork after the rest and compare with a fresh
forward run only. a_stage_task_names_the_governance_key_as_its_creator
(tests.rs:2034-2108) runs forward only and asserts desired offers, not signed
ones. A grep of every test file finds EffectAction::Offer only at
tests.rs:2103, so the path from an offer step signed by the key to a member's
accepted attempt (flow.rs:313-321, fold.rs:374-379, 738-744) is exercised by
no test. The fork test's stage step variant sits against an admission. A fork
between two stage steps with no governance record after it leaves the chain
order unchanged and reuses the closure index (goal/mod.rs:117-122). I traced
that case by hand and found no fault, but a regression there would let two
computers that received the two records in different order disagree, and no
test would see it.

**After the check.** Two new K1 replay tests that the plan says run three ways
run forward only: the governance fork test and the stage creator test. The
review fork test also runs forward only, but the plan sets no three-way rule
for it. No test replays a signed automatic offer and its acceptance. One node
test does fork the key's log between two stage steps
(node/tests/delivery.rs:275-342), but it checks deliveries after one arrival
order and a restart, not that two arrival orders agree.

**Smallest fix.** Run the two fork tests through Fixture::replays. Add one
test that signs an offer step with the governance key and has the recipient
accept it, and one that forks two stage steps at the last position of the
key's log, each through replays.

### `crates/locust-core/src/node/replica_tests.rs:239` (test gap)

`host_operations_refuse_a_daemon_that_does_not_hold_the_host_key` does not
read `hosted_here`, and no test asserts it false on a real daemon.

**How it goes wrong.** Verified by reading. The plan says the test keeps its
name "and also reads hosted_here false on the member's daemon and true on the
host's" (docs/host-safety-and-ending-plan.md:1345 to 1348) and the companion
gives it as the proof that a daemon says whether it hosts (details line 77).
K1 changed nothing in that test (the diff of replica_tests.rs touches only
line 1092). At HEAD every assertion on GoalStatus.hosted_here in locust-core
is `assert!(status.hosted_here)`. `goal add`, `workspace init` and the `Host:`
line all decide on the false value, and they are tested only against stub
servers that set the field by hand. The build notes do not declare the
omission.

**Smallest fix.** In that test, call goal.status on peers[0] and peers[1] and
assert true and false.

### `crates/locust-core/src/node/requests/daemon.rs:145` (test gap)

`agent_reconnect` refreshes a goal's deliveries before the agent counts as
connected, and the plan's assertion that a waiting step is signed after a
reconnect was not written.

**How it goes wrong.** Verified by reading, not run. Maple, the host's agent,
is disconnected. The host binds `pipeline`. The first stage's step names Maple
as a recipient, and `project_deliveries` records the delivery with `received:
false` because `principals.active` finds no Maple (node/delivery.rs:82-110).
The person runs `agent reconnect`. Its commit runs `project_deliveries`
(node/commit.rs:239-243) before the cleared flag is absorbed
(commit.rs:251-255), so the delivery stays unreceived. `land` then runs only
`drive_flow` (commit.rs:173-175), which commits nothing when no step is due.
`pending` lists a delivery only when it is received (node/views.rs:510-513),
so Maple's deliveries lack the opened task until some other record lands in
that goal. When the recipient is on another computer, the sender's
`DeliverEffect` is answered `received: false` on every exchange for the same
reason (node/replica.rs:129-141). The task still shows under `to_start`. The
test `a_disconnected_agent_is_connected_again_with_its_name_and_key` asserts
only that the record count is unchanged after each reconnect
(node/tests/authorization.rs:735 and 768). The plan asked it to show that a
step that waited is signed (docs/host-safety-and-ending-plan.md:1367). By
reading, no test depends on the loop at daemon.rs:145-149.

**Smallest fix.** Let `project_deliveries` read a principal record planned in
the same transaction, as it already reads planned `Space::Pending` writes
(delivery.rs:57-68). In the test, post a result whose review request the
disconnected agent must sign, reconnect, then assert that the step exists and
that the delivery made while it was disconnected is listed.

### `crates/locust-core/src/node/requests/levels.rs:118` (test gap)

The new governance branch of `Node::stalled` has no test.

**How it goes wrong.** Verified by reading. The plan asks that the stalled
test stall "a stage step only by a halt"
(docs/host-safety-and-ending-plan.md:1349 to 1350). The build notes say the
named test does not exist and that the nearest one passes unchanged (line
212). That test (context_views.rs:881) stalls only a member's review request.
Lines 118 to 126 are new: the key's step reports Halted on the host's computer
and nothing on any other. No test reaches them with a halt. The only assertion
near it is `status.stalled.is_empty()` with the agent revoked
(crates/locust-core/tests/organizations.rs:429). If the branch returned
nothing under a halt, a person whose goal has halted would see no stalled step
in `goal status` and no test would notice.

**Smallest fix.** Add a fifth case to
goal_status_reports_each_stalled_runner_condition: bind a stage, fork the
governance log, assert one Stalled with runner equal to status.governance and
reason Halted, and none on a member's daemon.

### `crates/locust-core/src/node/tests/authorization.rs:195` (test gap)

The plan's test `disconnecting_the_hosts_agent_stops_no_host_command` does not
exist, the build notes do not say so, and five host operations are never run
with the host's agent disconnected.

**How it goes wrong.** Verified with git grep at a6664a1. The plan names the
test at docs/host-safety-and-ending-plan.md:1339-1341 and the details document
lists it as evidence (docs/host-safety-and-ending-plan-details.md:64). The
build notes record one deleted test and one missing test and not this one.
With the agent disconnected, the tests run `goal.invite`, `member.remove`,
`rules.bind`, `goal.invitations` and an admission
(node/tests/lifecycle_characterization.rs:9, node/tests/invitations.rs:391,
authorization.rs:643, tests/organizations.rs:395). They never run
`task.revise`, `workspace.epoch`, `farm.on`, `farm.off` or `invitation.revoke`
in that state.
`host_operations_need_no_grant_and_sign_with_the_governance_key`
(authorization.rs:195) runs them only with the agent connected. A later change
that looks up the host's agent inside one of those handlers, as `farm_request`
does for a consent (node/farm.rs:624), would refuse the command for a host
whose agent is disconnected, and no test would fail.

**Smallest fix.** Add the test. Revoke the host's agent after `setup()`, send
each `Audience::Host` request once, and assert each signed record's author is
the governance key. Reuse the request list of
`no_credential_and_no_on_behalf_reaches_the_governance_key`
(authorization.rs:527), which already fails when a new host operation is not
listed.

### `crates/locust-core/src/node/tests/authorization.rs:643` (test gap)

The plan's test `disconnecting_the_hosts_agent_stops_no_host_command` does not
exist and the build notes do not say so.

**How it goes wrong.** Verified by search. The plan names it
(docs/host-safety-and-ending-plan.md:1341) and the companion lists it first
for the behaviour "Disconnecting the host's agent stops no host command"
(docs/host-safety-and-ending-plan-details.md:64). `git grep` at HEAD finds
neither it nor the Phase 1 test it was to replace. The build notes list the
renames that differed (line 191 on) and omit this one. With the host's agent
disconnected, other tests cover goal.invite, member.remove, an admission,
rules.bind and a stage step. Nothing covers invitation list and revoke, farm
on and off, workspace.epoch or task.revise in that state. A later phase that
puts an agent test back into one of those handlers would pass every test.

**After the check.** The named test is absent and the build notes do not say
so. With the host's agent disconnected, invitation list is exercised
(invitations.rs:397 and 411 to 414). No test sends invitation.revoke, farm.on,
farm.off, workspace.epoch or task.revise in that state. Every command the
companion names for this behaviour is covered by the other tests.

**Smallest fix.** Add the test: revoke the host's agent, then send every
Audience::Host request of OPERATIONS (the list at authorization.rs:531 to 573
already exists) and assert none is refused for that reason.

### `crates/locust-core/src/node/tests/authorization.rs:695` (test gap)

The reconnect test does not assert that a step that waited for the agent is
signed, and no test runs task revise, workspace epoch or farm on with the
host's agent disconnected.

**How it goes wrong.** The plan's test list asks
a_disconnected_agent_is_connected_again_with_its_name_and_key to show that a
step that waited for the agent is signed. The test at authorization.rs:695-772
checks the credential, the key and the record count only. The plan's
disconnecting_the_hosts_agent_stops_no_host_command does not exist. Its cases
are spread over other tests, and those cover invite, admit, remove and rules
bind. If a later change stops agent_reconnect from touching goals, or makes
one of the three uncovered host commands read the agent again, every K1 test
still passes. By reading, all three go through host() and work today
(requests/tasks.rs:110, requests/workspace.rs:126, farm.rs:664).

**Smallest fix.** In the reconnect test use the pause of
node/tests/context_views.rs:926-941: land the trigger with land_once, revoke,
reconnect, then assert the review request is signed by the agent. After the
revoke in the_hosts_agent_cannot_leave_or_be_removed_and_can_be_disconnected
add TaskRevise, WorkspaceEpochSet and FarmOn.

### `crates/locust-core/src/node/tests/farm.rs:406` (test gap)

`publication_needs_no_consent_from_the_governance_key` passes with K1's
exemption removed, because in its fixture the key signs only kinds that never
needed consent.

**How it goes wrong.** Verified by reading, not run. `setup()` founds the goal
under the `coordinator` preset, which has no stages
(locust-proto/src/organization/presets.rs:33-51). The key's records are then
the first record, the host's admission, the rules, the publication policy and
the forged consent. All five kinds are in the list `eligible` already skipped
before K1 (node/farm.rs:139-147), and the forged consent is excluded as well.
Delete the `continue` at farm.rs:133-135 and the key is still not a required
author, so every assertion in the test holds. The case the plan describes, a
goal with a stage step, a tree epoch or a task revision
(docs/host-safety-and-ending-plan.md:1125-1140), is not in this test.
`the_page_keeps_changes_signed_by_the_governance_key` (farm.rs:492) does bind
a stage and would fail without the exemption, but it lands no consent by the
key. So no test holds both an effective step by the key and a consent by the
key, which is the one state where that consent could suspend the page.

**Smallest fix.** In this test, bind a formation with one stage before
`activate`, assert the step's author is the key, then land the forged consent
and assert the page stays eligible.

### `docs/reference/conformance/organization.cases.json:265` (test gap)

The site mirror of the stage check is pinned by one shared case, and the
reconnect command's no-change answer has no test.

**How it goes wrong.** Verified by reading. The one new conformance case
(lines 264-267) covers a typed stage with an independent start inside any, an
offered start by the creator, and a declaration. The Rust test covers fourteen
rule shapes, each under a task type and under the formation's own rules
(crates/locust-core/src/organization/tests.rs:173-234). Nothing runs rules.ts
on a stage with no task type, where the formation's rules apply; on the to of
an offered start; on a criterion nested in all; or on a type that sets only
work or only decisions. I read both sides and they agree on each of those
today (organization/validation.rs:177-223 against contract/rules.ts:203-235).
A later edit to one side of those branches passes npm test and
check_formations.py while the editor and the daemon disagree. Separately, no
test types agent reconnect for an agent that is connected: the string is not
disconnected. Nothing changed (crates/locust/src/cli/only_you.rs:1384) appears
in no test, and the plan names that answer (plan lines 1211-1212).

**Smallest fix.** Add four cases to organization.cases.json (the formation's
own rules, offered to, nested all, a type with decisions only) and regenerate
the vectors. Add a reconnect of a connected agent to
agent_revoke_applies_at_once_and_prints_the_command_that_undoes_it.

### `research/tla/Organization.tla:253` (test gap)

The Organization model has no record kind for a host step, so the one new path
by which a signer that is no member has work count is not modelled, and the
new invariant says more than replay does.

**How it goes wrong.** GovernanceKeyIsNoMember (Organization.tla:253) says no
record in view.ordinary is identity 0's. The model defines ordinary as every
valid record that is not governance and not a pick (Organization.tla:206). In
the Rust replay an EffectMaterialized signed by the key is not governance and
is Effective (chain.rs:418-423, event.rs:524-526). The model's kinds are
genesis, admit, remove, rules, task, contribution, review and select. So the
model cannot see the branch K1 added: no tenure test, the walk back through
governance records, and a stage step whose fork halts governance. The scenario
governance-key-work covers a contribution and a review only. Read, not run.

**After the check.** The model has no host step, so the branch that lets the
key's step count rests on Rust tests alone. That is the scope the plan set for
K1's model change, not a departure, and the invariant matches replay for the
kinds the model has. What is worth recording is the limit: host steps, a step
forked against a governance record, and a governance record that follows a
step in the key's log are outside Organization.tla, and neither its header
(lines 15-19) nor organization.md says so.

**Smallest fix.** Add a step kind that only identity 0 may sign, state the
invariant as 'no record of identity 0 other than a step is ordinary', and add
one scenario with two steps at one position of identity 0's log.

### `crates/locust-core/src/node/authoring.rs:208` (wording)

At a halt of the goal's key, host commands that sign through next_place answer
`unavailable` with `the goal's history has not arrived yet`.

**How it goes wrong.** Verified by reading. The host's log holds two records
at one position. `member remove`, `rules bind`, `task revise`, the workspace
epoch and `farm on` all reach next_place (goals.rs:160 and 314, tasks.rs:129,
workspace.rs:141, farm.rs:764) and get the sentence at authoring.rs:208-213.
The sentence is false on the host's own computer, and `unavailable` invites a
retry that can never succeed. `goal invite` on the same goal answers `halted`
with `the goal's authority is halted` (invitations.rs:168-173). The build
notes record the two codes. G1 plans a Halted answer from next_place for an
agent's key.

**After the check.** Holds. One point for the fix. Goal::next is also empty
for the goal's key when its log has a gap and no halt (goal/mod.rs:267;
node/tests/delivery.rs:738-767). There the sentence is roughly true and a
retry can succeed once the missing record arrives. So the change must test
host_halt, as the finding proposes, and leave the gap case as it is. The test
at delivery.rs:725-734 must change with it.

**Smallest fix.** In next_place, when the author is the goal's key and
evaluation().host_halt is set, return ErrorCode::Halted with goal_invite's
sentence.

### `crates/locust/src/cli/presentation.rs:151` (wording)

goal status prints Host: you to whoever asks, an agent's credential included.

**How it goes wrong.** Verified by reading. On the host's computer Juniper's
agent, with a shell client, runs locust goal status --goal "Parser cleanup".
render receives principal = Some(juniper)
(crates/locust/src/cli/mod.rs:448-451) and uses it three times to choose by
your owner over by you (presentation.rs:582, 611, 625). host_line takes no
principal and prints Host: you (presentation.rs:149-155, called at 508).
Juniper is neither the host nor the host's agent. An agent that takes the line
at its word tries host commands and is refused. Agents on MCP get JSON and are
not affected.

**Smallest fix.** Tell host_line whether the connection's caller is an agent
and print Host: your owner in that case.

### `docs/guide/help.md:31` (wording)

The guide still defines the host as a member and says the starting agent
becomes the host.

**How it goes wrong.** Verified by reading. docs/guide/help.md:31 reads Host:
the member who manages membership and rules. docs/guide/concepts.md:6-7 reads
naming the agent that becomes its host. docs/guide/collaboration.md:7 reads
naming its host agent. After K1 the host is the person, the key that keeps
members and rules is no member, and the named agent is the host's agent (plan
lines 892-902). A person who reads the glossary and then disconnects that
agent expects the goal to have lost its host, which is the belief K1 removes.
The guide has no line on agent revoke or agent reconnect. The build notes say
only the two sentences about the key were changed (notes lines 122-125).

**After the check.** The guide's host sentences are stale after K1, but by the
plans' own rule they belong to Phase 6 of the roles plan, which already lists
the glossary change. This is a note for Phase 6, not a K1 defect. The one
guide sentence K1 did edit, the goal's key at collaboration.md:164, is covered
by the finding at index 7.

**Smallest fix.** help.md: Host: the person who started the goal. Their
computer keeps who is in and the rules. concepts.md: naming the agent that
becomes the host's agent. Add one sentence on disconnecting an agent and
connecting it again.

### `docs/master-plan.md:3` (wording)

The tracked plans still say K1 is not built.

**How it goes wrong.** Verified by reading. The master plan, the one document
the owner approves, says "Phases 1 to 3 of the build order are built … the
rest is proposed" (lines 3 to 4) and its row 4 for K1 (line 319) lacks the
word Built that rows 1 to 3 carry. docs/host-safety-and-ending-plan.md:4 and
the companion at line 5 say "Not accepted and nothing here is built." The
build notes say the plan files belong to another session and were not edited
(line 13). For Phases 1 to 3 a commit recorded the state (b860be2). A reader,
or an agent starting Phase 4 from the master plan, is told that goals are
still signed by the creating agent and that the protocol is 6. The joinable
plan also still says of the sync order change that neither name exists in the
code today (docs/joinable-farms-plan.md:1995 to 1997), and K1 built
`Replica::first_author`.

**After the check.** The master plan (lines 3 to 4 and row 4 at 319) and the
headers of the two host safety documents still say K1 is not built; the
builder left them on purpose and said so. The joinable plan's sentence at 1995
to 1998 is not wrong as written: it says the old names `lead_author` and
`lead` do not exist. It could still note that K1 already sends the governance
key's log first.

**Smallest fix.** One docs commit: mark K1 built with cb1acaa in the master
plan's status line and row 4, change the two plan headers, and note in the
joinable plan that the governance key's log is already sent first.

## What held in K1

From the reader of replay:

- The first record. Only the key it names as governance can author it, at
  position 0, with no anchor and no parents. Its host must differ from that
  key. The goal identifier hashes governance, host, definition and salt in
  that order (event.rs:84-91, 785-796). Signing, decoding and loading from the
  store all run this check. The two new tests in event.rs cover a wrong
  author, host equal to governance, and each key changed or swapped.
- Two first records. Both must hash to the goal's identifier, so both name the
  same governance key and the same host's agent. History::insert overwrites
  governance and host from any first record it holds (history.rs:174-177), and
  the values are the same whichever arrives first. The two records sit at
  position 0 of the key's log, so the chain is empty and the goal halts on
  every computer (history.rs:80-86, chain.rs:66, 235-247). A first record of
  another goal cannot get in: Goal::apply and Replica::receive test the
  header's goal (goal/mod.rs:104, node/replica.rs:196). Read, not run.
- The governance key signing outside its kinds. A task, a result, a review, a
  pick, a leave, a consent or a delivery receipt by the key is NotAMember
  before any rule is read (chain.rs:418-423, fold.rs:201-214). A pick's pins
  cannot rescue it, because authorize returns the base standing first
  (chain.rs:361-363). A pick that names such a result fails at fold.rs:936.
  One detail: if the record's anchor is not in the chain, the answer is
  Pending(Anchor) or BadEpoch before the NotAMember test (chain.rs:406-417).
  It still never counts. tests.rs:1786-1841 runs this forward, reversed and
  reloaded.
- The key as a member, a role holder, an authority, a reviewer, a recipient or
  a task creator. Its admission is excluded and still takes a position and a
  snapshot (chain.rs:98-101, 227-233). A rules binding that names it in a role
  or as an authority is excluded, because both must be admitted members
  (chain.rs:500-510, 533-542). Recipients and reviewers are drawn from the
  member snapshot, so it is never one (flow.rs:63-70, 226-236). It is a task's
  creator only through a stage step. A member's copy of that step at a lower
  log position never becomes the creator (rules.rs:64-75, 163-189,
  tests.rs:1708-1783).
- A stage step signed by a member, and a review request for an ordinary task
  signed by the key, are both refused as not the configured runner
  (flow.rs:323-325, tests.rs:1844-1874).
- An admission or a removal that names the key or the host's agent. A removal
  of the host's agent is excluded, the epoch does not advance and later
  governance goes on (chain.rs:134-137, tests.rs:1877-1910). A removal that
  names the key is excluded as not naming a current admission
  (chain.rs:138-141). A second admission of the host's agent is excluded as
  already admitted (chain.rs:102-105).
- A fork of the host's agent's log. Chain::build reads only the governance
  key's log (chain.rs:61-66). Goal::next refuses the governance key only for
  its own log or a halt (goal/mod.rs:264-272). Members and rules stand
  (tests.rs:1991-2031 and the two acceptances_by_the_hosts_agent tests in
  workspace_tests.rs). For the public page it costs what a member with work
  costs: a removal keeps the member's effective work
  (node/requests/goals.rs:319-330), so that member's consent stays required as
  well (node/farm.rs:126-152). I read only eligible in farm.rs.
- A fork of the governance key's log, as two computers see it. The usable
  prefix is a function of the held set and not of arrival order
  (history.rs:78-128), so the cut is the same everywhere. A stage step against
  an admission halts governance and keeps earlier work (tests.rs:1913-1988,
  forward only). I traced by hand a fork between two stage steps with nothing
  after it: the chain order is unchanged, the reused closures do not depend on
  forks (commitments.rs:261-315), both steps are Pending(ForkProof), work on
  that stage's task waits, and a pick whose proof holds one branch stands.
  Read, not run.
- Anchors of host steps. An honest daemon anchors at the head
  (node/authoring.rs:199-219, goal/mod.rs:265), and the walk back through the
  key's own governance records never reports a regression for it
  (chain.rs:439-482). A modified host daemon can anchor a step one governance
  record behind the record that precedes it in the log, also behind the rules
  it names. That needs a hostile host, which is not assumed.
- The stage rule check. task_creator in propose and publish was already
  refused everywhere (validation.rs:57, 91-93). The new check covers the by of
  an independent start, the to of an offered start and every completion
  criterion, alone or inside an any, for the type's rules or the goal's own
  (validation.rs:177-223, 314-338). Replay runs it on every binding and on
  every rules lookup (chain.rs:497-499, rules.rs:118-120), so a refused
  binding is InvalidDefinition on every computer (tests.rs:2111-2152).
- Subtasks under a stage's task inherit the rules with the creator fixed to
  the key (delegation.rs:10-48). That is the limit the plan states, and I
  found nothing beyond it.
- Same records, same answer. The fold visits records in arrival order
  (fold.rs:1113), but each standing is memoised per record and I found no
  cycle through stage steps, offers, review requests or revisions
  (flow.rs:89-100, 256-275, fold.rs:768-794). The opening record the
  projection picks for a stage's task is the one current_round picks
  (projection.rs:52-63, 86-109, fold.rs:777), because anchors never go
  backward along one log. Three K1 tests compare forward, reversed and
  reloaded runs for equality.
- Sync screening. A computer keeps records only from the governance key and
  from keys named in an admission that key signed (screen.rs:10-45). A key
  that was never admitted is dropped. A dropped record is not counted, so it
  is sent again at the next exchange. The key's log is sent first in both
  directions (sync/outbox.rs:17-29, 119-160, sync/initiator.rs:368), so a
  joiner holds the admissions before any member's records arrive. A first
  record that names another key than the ticket's is refused
  (node/replica.rs:199-207). A halt proof needs two records signed by the
  governance key or by a key it admitted (node/peers.rs:149-166), so a removed
  member can fork only its own log.
- The signed format. Protocol byte 7 is tested before anything else is decoded
  (event.rs check_size_and_version). The first record's fields are governance,
  host, definition, salt in that order, and the frozen vector's first header
  shows them so (vectors.rs HEADER0). An admission and a stage step signed
  with nobody present carry clock field 0 and no payload
  (node/authoring.rs:165-173), so the same act from the same records is the
  same bytes.

From the reader of node:

- No path signs members or rules with an agent's key. I listed every call of
  signer, author, sign_at, sign_for, author_alone and next_place in node.
  Rules, removals, task revisions, tree epochs, the publication policy and
  invitations take the key from host() (goals.rs:145 and 303, tasks.rs:110,
  workspace.rs:126, farm.rs:664, invitations.rs:167-178). goal_create signs
  its three records with the new key (goals.rs:52-122).
- No path signs work with the goal's key. Every sign_for caller takes its
  principal from member(), which needs an enrolled agent. FarmConsent checks
  signer(&agent) first (farm.rs:624). on_behalf of an unknown key is not_found
  (callers.rs:136-142). Replay excludes anything else the key signs
  (goal/chain.rs:418-423).
- No host command reads the host's agent. host() tests only hosts()
  (access.rs:128-141). plan_join tests hosts() and no principal
  (peers.rs:338-345). The governance branch of drive_flow tests hosts() only
  (flow.rs:24-25). goal_invite reads the title as the owner
  (invitations.rs:177). A disconnected, left or absent host's agent cannot
  stop any of them.
- The key does not leak. The seed is written once (goals.rs:124,
  local.rs:198-200) and read once (local.rs:272-275). Keypair's Debug prints
  the public key (crypto.rs:119-123). Local is not serializable. Only
  Node::open scans Space::Goal (mod.rs:178-181). Sync frames carry events,
  objects and content keys. Keypair::seed is called only at enrollment
  (requests/daemon.rs:90). Core, store and proto log nothing.
- I found no way to get a second record at a used position of the goal's key
  in normal running. The node is one writer. Memory is advanced before the
  commit, rolled back on failure, and the node then answers nothing more
  (commit.rs:246-250). Records are offered to peers only after the commit
  (commit.rs:262-274). A retry of a join already admitted is answered before
  any test and any signature (peers.rs:324-337). The ticket's record goes into
  the commit that admits (peers.rs:357-359).
- An admission and a stage step are the same bytes if signed twice from the
  same records. author_alone passes no text and clock 0
  (authoring.rs:165-173). The admission's body comes from the request
  (peers.rs:350-353). A step's body comes from desired_effects, built from
  sorted sets. drive_flow reads the goal again before each step and lands one
  step per commit (flow.rs:12-50).
- author_alone has two callers, peers.rs:347 and flow.rs:42, and no other path
  signs a log record with nobody present. The farm poll signs with the page's
  key and Host::joins with the joining agent's key. Neither is a log record.
- Founding is one commit: three records, the seed, the content key, the title,
  the part record and the level (goals.rs:66-127). If replay refuses a record,
  rollback removes the new entry and nothing is stored (commit.rs:233-237 and
  293-295).
- The places the daemon signs at match what replay asks. The anchor is the
  chain head and prev is the tip of the signer's log (authoring.rs:214-219,
  goal/mod.rs:264-279), which is what goal/chain.rs:71 and the walk at
  chain.rs:439-482 require, with host steps between governance records.
- Reconnecting behaves as specified for the cases I tried. An agent that was
  never disconnected gets Done and no write (daemon.rs:141-154). An unknown
  key is not_found (daemon.rs:135-138). An agent that left stays left, because
  Local.part is untouched and next_place refuses it (authoring.rs:201). The
  host's agent takes the same path. The credential index keeps revoked
  entries, so the stored credential works again (identity.rs:126-133 and
  153-155, mod.rs:325-331).
- The replayed copy from sign_for is adopted only when the definitions did not
  change, the revision is the one the copy was taken at, and the transaction
  holds exactly the copy's events in order (commit.rs:319-332). Records signed
  with the goal's key never carry a copy, because host commands use author and
  drive_flow builds a fresh Tx. The one request that adds a second event after
  sign_for (claims.rs:364-389) fails the event test and is replayed in full.
- Publication treats the key as specified. It is left out of the authors whose
  consent is required (farm.rs:132-135) and its records stay in the page's
  changes with no agent (farm.rs:492-500).
- Tickets and joins hold. An invitation is signed with the goal's key
  (invitations.rs:178-199). A joiner refuses a first record that names another
  key (replica.rs:199-207). goal_join compares the ticket's endpoint with the
  host's agent's admission (invitations.rs:246-255). plan_join refuses a
  request that names the goal's key as the member (peers.rs:341).
- The removal draws its content key in the request, signs through next_place
  with the goal's key, and stores the key in the same commit
  (goals.rs:314-346). A failed commit leaves nothing a peer has seen, so a
  retry with a fresh key is not a second record.

From the reader of plan:

- Protocol crate, built as written (verified by reading the diff): Genesis {
  governance, host, definition, salt }; goal_id hashes the four in that order
  (event.rs:84 to 90); Header::check refuses an author other than governance
  and host equal to governance as BadAnchor; Body::host_may_sign is
  is_governance or EffectMaterialized; PROTOCOL_VERSION 7 with API 7
  unchanged; versions.protocol 7 in docs/site.json; no field change in
  invite.rs; GoalStatus gains governance and hosted_here and an optional host;
  ContextBrief.host optional; EventView and TaskView gain by_host;
  agent.reconnect is audience Owner, not a tool, answered Done; testkit
  genesis and found_goal take and admit the host's agent's key.
- Frozen vectors: HEADER0 starts with 07 and carries the body in the order
  governance, host, definition, salt. I read the constants and did not run the
  test.
- Replay, built as written: Chain::build sets state.host; an admission of the
  key and a removal of the host's agent are excluded with the plan's two
  sentences and still take a position; authorize_base answers NotAMember for
  the key unless host_may_sign, skips tenure and cutoff, and keeps the anchor,
  epoch and ancestry checks. goal/flow.rs reads history.governance where the
  plan says chain.state.governance; Chain::build copies one into the other, so
  the value is the same.
- The stage check: validation.rs reports selector_scope for task_creator in an
  independent start's by, an offered start's to and every completion
  criterion, alone or inside any, reading the task type's rules or the
  formation's as rules.rs:132 to 140 resolves them; an offered start's by
  passes; rules.ts mirrors it branch for branch; the one new case is in
  organization.cases.json with two diagnostics in the generated vectors;
  formations.md has the clause. The validator test covers all seven shapes,
  both wrappings, both sources, the passing case and every preset.
- Signing on the node: key_for, next_place (member and part tests skipped for
  the key only) and author_alone are as written; at HEAD author_alone is
  called only from peers.rs:347 and node/flow.rs:42; every other signature
  with the key goes through author or next_place except goal_create's three
  sign_at calls; no code path outside a request handler signs with it except
  those two.
- Removals: at HEAD git grep finds none of 'host agent is disconnected',
  'freeze for everyone', 'no command undoes', 'act as the host' in crates;
  none of the nine old test names the companion lists; no direct signer call
  for the goal's key in member_remove, goal_invite or farm_request; the agent
  test in host() and in plan_join is gone; goal add's local-agent test is
  gone; agent revoke has no --plan or --confirm (the CLI test asserts exit 2
  for both).
- plan_join keeps the retry answer for a redeemed ticket ahead of the three
  new tests and of any signature, signs with clock field 0, and keeps
  redeemed_ms in the ticket's record.
  an_admission_signed_twice_for_one_request_is_one_record does compare two
  copies of one store under different clocks, and a second joiner gives
  another identifier.
- The other named tests exist and assert what their names say, by reading: the
  two in event.rs; the six replay tests and the rewritten review-fork test in
  goal/tests.rs; the two acceptance tests;
  a_restored_hosts_agent_forks_only_its_own_log and the three restored_host
  tests; the two sqlite tests;
  revoking_the_hosts_agent_stops_that_agent_and_not_governance (all seven
  facts the plan lists);
  host_operations_need_no_grant_and_sign_with_the_governance_key;
  goal_create_is_the_owners_act_and_names_the_host_agent;
  the_governance_key_is_stored_with_the_goal_and_signs_after_a_restart;
  no_credential_and_no_on_behalf_reaches_the_governance_key (it fails when a
  new host operation is not covered); the two ticket tests;
  stage_steps_are_signed_while_the_hosts_agent_is_disconnected; the two farm
  tests; the four CLI tests and
  init_refuses_while_the_hosts_agent_is_disconnected.
- Farm: eligible leaves the key out where required authors are built; a
  consent signed by the key is excluded and never read; project keeps the
  key's records with agent None; farm_request signs the policy through author
  with the key host() returns.
- Declared departures I judged sound readings: goal add answers denied on a
  computer that does not host; goal leave for the host's agent refuses before
  its plan (the exit criterion needs exit 7); workspace init refuses with
  denied on a computer that does not host; agent revoke always sends and
  always prints Undo; agent reconnect on a connected agent sends nothing and
  prints the plan's sentence; both acceptances Disputed at once; Halted for
  goal.invite and Unavailable for member.remove under a halt; the
  workspace_lifecycle test's construction; availability.json.
- The sync change (governance key's log first) is declared, moves no signed
  byte and changes nothing a computer judges; it only orders frames, and the
  frontier frame stays ascending. The joinable plan asks for the same under
  Replica::first_author. Its test covers the responder; the initiator's use at
  initiator.rs:368 has no test of its own, and I did not count that as a
  finding.
- `rules bind` with a refused stage rule: declared. The daemon refuses with
  the generic sentence and exit 6 after the plan is shown, and only `formation
  validate` prints selector_scope. The exit criterion's words 'refused with
  the diagnostic selector_scope' are met only in that second command. It is
  the same for every invalid formation and the notes say so, so I left it as
  declared.
- Model: record 2 admits identity 5 and no other id moved; the two scenarios,
  their configs and their rows in cases.json exist with the claims the plan
  states; organization.md says identity 0 governs and is not a member and
  identity 5 is the host's agent.
- Scripts and recipes: the three harnesses pin (7, 7); the scripts that read
  goal_status.host still compare it with the creating agent's key, which is
  what the field holds; no recipe under docs/guide and no script calls agent
  revoke or binds a stage rule with task_creator; skills/locust/SKILL.md has
  no sentence K1 makes false.
- Guide prose that K1 makes loose (help.md:31 calls the host 'the member who
  manages membership and rules'; concepts.md:7 says the agent 'becomes its
  host') is Phase 6's by the roles plan (roles-and-permissions-plan.md:1169 to
  1170), so I did not report it.

From the reader of surface:

- The count. A host who disconnects the agent they started a goal with and
  later wants it back types two commands and answers zero confirmations: agent
  revoke --agent NAME, then agent reconnect --agent NAME. No plan id, no
  restart, no new enrolment, no goal record. Revoke only sets a flag and keeps
  name, seed and credential digest
  (crates/locust-core/src/node/requests/daemon.rs:109-128), so the level, the
  roles and the client's credential come back as they were. The count assumes
  the person still has the Undo line; see the fourth finding. Read, not run.
- An agent cannot call agent.reconnect and it is not a tool. The table row is
  Owner with tool false (crates/locust-proto/src/api.rs:832). The MCP list
  keeps only tool rows and admits no Owner or Host row for an agent
  (crates/locust/src/mcp/schema.rs:13 and 32). tools/call looks names up among
  tool rows only (crates/locust/src/mcp.rs:493), so locust_agent_reconnect is
  an unknown tool. In the daemon a revoked agent is refused at hello
  (node/mod.rs:331) and per request (callers.rs:169-170), an active agent gets
  the owner-only refusal (callers.rs:172-178), and an author's credential is
  refused (callers.rs:182-190). So a disconnected agent cannot connect itself
  again. The core test asserts Denied (node/tests/authorization.rs:701-702).
- agent reconnect in each state it can be typed in. Never disconnected: it
  reads status, sends nothing, prints NAME is not disconnected. Nothing
  changed, exit 0 (only_you.rs:1371-1385). Disconnected twice: the flag is one
  boolean and one reconnect clears it (daemon.rs:134-153). The host's agent:
  same path, no goal record. An agent in no goal: no goal is touched. An agent
  that left a goal: the flag clears, the membership stays left, and connected
  again is true of the agent. Unknown name or key: a not_found sentence.
  Without --owner: a usage error. Each answer is true. Read, not run.
- Disconnecting applies at once and prints the command that undoes it. The
  command has no --plan and no --confirm (only_you.rs:107), sends one
  agent.revoke and prints the Undo line last (only_you.rs:1326-1355). The
  printed line parses as printed because --agent is a global flag
  (cli/args.rs:182-185). The text matches K1-2.
- The old refusals are gone. git grep on a6664a1 finds neither host agent is
  disconnected nor freeze for everyone nor Host fingerprint in crates, skills,
  docs/guide or the site. host() no longer tests the agent
  (node/access.rs:128-141) and plan_join no longer tests it. goal invite, goal
  add, member remove, rules bind and task revise depend on hosts(entry) alone.
  workspace init carries the new refusal with the reconnect line, in the
  plan's words (cli/workspace.rs:305-309).
- Host lines for the person. goal create's result reads Started "..." (...).
  Host: you. This computer keeps who is in and the rules (only_you.rs:570), as
  K1-1. goal status prints Host: you when hosted here, the host's agent as
  label prints it elsewhere, and Host: on another computer before the first
  record (presentation.rs:149-155).
- Records by the key print host with no key in the event list, the board, the
  task header and a stalled step (presentation.rs:625, 545, 552, 532). The
  exceptions are the two lines in the third finding.
- JSON and plans carry the key only where K1 says. Plans' review JSON holds
  host as the host's agent (only_you.rs:624, 876, 986, 1061, 1129). status
  --json has no governance field. Abilities.host and ContextBrief.host are the
  host's agent (requests/levels.rs:94; the context_views.rs diff). The join
  plan and the join result print no key (only_you.rs:753; cli/mod.rs:642).
  invitation list prints no key in text. The exception is Why::Rules.host, the
  first finding.
- The public page data holds no key. Records by the key enter the change list
  with agent None and a fixed text (node/farm.rs:489-560), and FarmSnapshot
  has no key field (crates/locust-proto/src/farm.rs:193-208).
- member remove and goal leave that name the host's agent refuse before any
  plan, with the daemon's own sentences (only_you.rs:862-867 and 1052-1057). A
  disconnected host's agent still resolves by its local name, because local
  means held, not active (requests/goals.rs:225).
- The formation check, Rust against its site mirror, case by case: the type
  lookup and the fallback to the formation's rules, work and decisions chosen
  separately, independent by, offered to, offered by allowed, nesting inside
  any, the four completion kinds, nested all and any with their index paths,
  the message, the correction, the diagnostic path and the order. They agree
  in each (organization/validation.rs:177-223 and 314-338;
  contract/rules.ts:203-235 and 321-340). The choice of rules matches what
  replay resolves (goal/rules.rs:122-142). No preset names task_creator. The
  diagnostic's own text tells an author what to write.

From the reader of removed-and-tests:

- Replay cannot make the governance key a member. Its own admission is
  excluded and keeps its position (goal/chain.rs:98-101). Its tasks, results,
  reviews and picks are NotAMember (chain.rs:418-423). The one other kind it
  signs is effect_materialized, and that must still match the configured
  runner and template (goal/flow.rs:323-333). goal/tests.rs:1786 and 1844
  would fail if either rule went.
- The member test K1 skips for the key is the only test it skips. The anchor
  must be held, the payload epoch must match, the walk along the key's own log
  runs, and the usable prefix applies (chain.rs:403-417, 428-470, 360-378). A
  step after a fork in that log is pending, and the fork halts governance
  (goal/tests.rs:1913, both variants).
- A second first record cannot name another key or another host's agent. The
  identifier hashes both keys (locust-proto/src/event.rs:84-91), Header::check
  refuses a mismatch and a first record whose host equals its key
  (event.rs:786-795), and a joining daemon refuses a first record whose key
  differs from its ticket's (node/replica.rs:199-207).
- The host's agent cannot be removed or leave. Replay excludes the removal
  with the epoch unchanged and later governance effective (chain.rs:134-137,
  goal/tests.rs:1877). The daemon refuses both requests on state().host
  (requests/goals.rs:270 and 304). A removal that names the key itself falls
  to 'not a member' (goals.rs:309).
- No request reaches the key. on_behalf needs an active enrolled principal
  (node/callers.rs:137). local_agent does too (node/access.rs:348-352). A farm
  consent checks signer first (node/farm.rs:624). key_for is reached only
  through host(), plan_join and drive_flow; author_alone has two callers
  (node/flow.rs:42, node/peers.rs:347) and sign_at with the key only
  goal_create and member_remove.
- plan_join lost only the two tests of the host agent. The retry answer still
  comes first (node/peers.rs:324-337). The tests that follow are: this daemon
  hosts the goal, expiry, the ticket's key against the goal's, the joiner is
  not the key, the joiner is not already a member (peers.rs:338-345). The
  admission carries clock 0, and node/tests/invitations.rs:425 would fail if a
  clock reading came back.
- The ticket's endpoint check now reads the host's agent's admission
  (requests/invitations.rs:246-255). The test at node/tests/invitations.rs:532
  fails if the lookup still used the key.
- Sync membership is untouched. speaks_for_member, peers, reachable and
  historical_endpoints read the member list and the admissions in the key's
  log only (peers.rs:31-50, 179-216, 366-379), and the responder's three
  checks are as before (sync/responder.rs:135, 156, 187). The host's computer
  stays reachable because its agent's admission cannot be removed. The new
  send order changes only the order of event frames; the frontier frame is
  unchanged (sync/outbox.rs:17-30, 125, 162). A record that screen drops is
  sent again at the next exchange.
- Publication consent. The key is left out where required authors are built,
  the host's agent is still required as a member, and the key's records reach
  the page with no agent (node/farm.rs:126-135, 498, 557). The test at
  node/tests/farm.rs:492 fails without the exemption.
- hosts() cannot be true without the seed, and the seed is absorbed at start
  from record K (node/access.rs:118-124, node/local.rs:272-275,
  node/tests/lifecycle.rs:1166). Keypair's Debug prints the public key only
  (locust-proto/src/crypto.rs:119-123). I found no command that reads
  Space::Goal out.
- Every other guard K1 deleted has a replacement or is an intended removal.
  host()'s test of the agent is the intended removal. next_place skips
  membership for the key only (node/authoring.rs:199-201). drive_flow and
  stalled gained a branch for the key and kept the agent tests
  (node/flow.rs:24-31, requests/levels.rs:127-136). goal add's test of a local
  host agent became hosted_here (cli/only_you.rs:593). The compact context
  lost its fallback to the ticket because host is now optional. The simulator
  compares host with Some of the creating agent (node/sim/check.rs:88).
- The simulator scenarios go through the API and needed no change. check.rs
  still compares members, endpoints, governance heads and the host on every
  machine. None of the scenario files signs with the founder's key directly
  (search of node/sim).
- The stage-rule check as bound. propose and publish can never name
  task_creator (organization/validation.rs:92-93), so starts and completion
  are the whole surface for a stage's task at binding. The Rust check and the
  site's rules.ts agree branch for branch. Replay runs it on every binding
  (chain.rs:487-498), and goal/tests.rs:2111 would fail if it did not.
- agent.reconnect is refused to an agent's credential and to an unknown key,
  writes nothing for a connected agent, and signs no goal record
  (requests/daemon.rs:134-155, node/tests/authorization.rs:695-716). A step
  whose runner is the reconnected agent is signed, because drive_flow runs
  after the flag is absorbed (node/commit.rs:172-175).

## The three experiments

### The restore guard's model

**What the assessor read and ran.** VERDICT ON G1. Phase G1 cannot be built
exactly as its text stands. One rule needs an exception (first table, row 3:
marks lost while a restore is still being caught up; first finding). Three
sentences its tests rest on are false as worded (property 1 with the simulator
invariant, the agent exemption on a member's computer, property 4). Everything
else in the two tables held in every run I made, including runs at wider
bounds than the registered ones. READ IN FULL: research/tla/RestoreGuard.tla
(261 lines); research/tla/restore-guard.md;
research/restore-guard-model-2026-10-06.md;
docs/host-safety-and-ending-plan.md lines 1671 to 2548 (all of G1), 4236 to
4346 (Models written first) and 4395 to 4440; docs/master-plan.md lines 1 to
180; scripts/check_tla.py; research/tla/README.md; the diff of 0ba1f31 to the
runner; all 31 restore entries of research/tla/cases.json, each with its
config. SKIMMED: the summary fields of
research/tla/restore-guard-results.json, not its stored traces. At HEAD, only
the membership checks in crates/locust-core/src/node/peers.rs:179-216,
crates/locust-core/src/sync/responder.rs:95-200, driver.rs:425-437,
initiator.rs:124-210, and the first test of
crates/locust-core/src/node/sim/lifecycle_characterization.rs. NOT OPENED:
scripts/tests/test_check_tla.py; the G2, E1 and E2 sections;
docs/host-safety-and-ending-plan-details.md beyond grep hits at lines 117,
136, 172, 180, 183; Organization.tla; the K1 commits. RAN IN THE REPOSITORY:
/opt/homebrew/bin/python3 scripts/check_tla.py --suite restore, at a6664a1.
All 31 cases matched their registered expectation. State counts equal the
retained file (restore-p1-store 266,314 generated and 32,526 distinct;
restore-p1-store-extended 10,696,110 and 1,168,878; restore-p2-all-extended
7,970,928 and 906,470). The two temporal failures loop as registered (back to
state 13 and state 12). Evidence:
output/tla/runs/20261006T201858Z-33420e6c/results.json. The runner itself
exited 1. It recorded source_changed_during_run true, because another session
edited tracked files under crates/ during the five minutes. RestoreGuard.tla,
the restore configs and the runner were unmodified (module sha256 312f3ca3,
equal to the retained one). The retained results match the registry and the
plan's hash at HEAD. RAN OUTSIDE THE REPOSITORY: copies of the model with
one-line variants and extra configs, with the pinned TLC and JDK from
output/tla/tools, under scratch/rg (each .cfg has its .log). Each result is
cited in the finding or hold it supports. I wrote nothing in the repository
except the runner's own output directory. Claims about Rust are by reading
HEAD and are not run. G1 is not built, so nothing about G1 was run in Rust.

| Kind | Where | Finding | Smallest fix |
| --- | --- | --- | --- |
| a goal stops for good | `docs/host-safety-and-ending-plan.md:1745` | A behind hold is remembered only by the marks, so if the marks are lost while a key is behind, row 3 calls the next start ordinary and the key signs at a position another computer holds. | Row 3 gains one exception: in a goal that still holds its RESTORED record, lost marks make every local key unheard, as in row 4. Node::open decides this before it rewrites marks or deletes RESTORED. On a host that goal then waits for goal continue. On a member's computer it ends when the host's computer is heard. Checked in a scratch copy with that rule (a store-carried bit set by a start that finds a restore, cleared by an ordinary start with nothing held) and LoseMarks allowed at any time: StoreNoFork, AgentFenced and NoGiveWhileHeld pass at 3 records (115,959 distinct states) and at 5 records with one initial peer (4,488,917). |
| test gap | `research/tla/RestoreGuard.tla:102` | The model forbids losing the marks after a restore and never restarts a restored daemon, so it cannot see the failure above, and the guide's exclusion list does not say so. | Remove the guard on line 102, add Restart to GeneralNext, carry a restore-found bit in the store part of the state, and register one case named for the trace. |
| test gap | `docs/host-safety-and-ending-plan.md:2289` | On a member's computer the agent exemption is wrong as worded: a removal the host signed before the copy, which the copy does not hold, gives the same fork, and no exhaustive case runs a member's daemon. | Say 'no removal that the restored copy does not hold' in place of 'no Remove after the copy' at plan lines 2289 and 4281, at details line 183, and in removedAfterCopy (RestoreGuard.tla:75). Register general cases with Host = "peer1". With that wording the member's daemon passes in scratch at 4 records (294,288 distinct states) and at 5 records (2,359,149). |
| test gap | `research/tla/RestoreGuard.tla:79` | The case for 'an agent key given up while the governance key is still behind' fails only a flag that restates the rule. The fork the rule prevents needs two admissions, which the one-admission bound forbids. | Let one registered case admit twice (the agent and one peer) and check NoFork under give-held. Change 'one admission' at plan line 4272 to two for that case. |
| test gap | `research/tla/configs/restore-p1-store-extended.cfg:5` | Every exhaustive case starts with the agent admitted and at least one peer, so two rules are exercised only by single directed traces: hearing only on an exchange that brought nothing, and the never-shared release. | Register two general store cases: InitialAgent FALSE with both peers at 4 records, and InitialPeers {} at 5 records. Both pass unmutated in scratch (221,062 and 276,242 distinct states). The second fails under empty-shared, as it should. |
| wording | `docs/host-safety-and-ending-plan.md:4281` | Finding F1 is real as a contradiction in the text and is not a hole in the rules: property 1 and the simulator invariant say no used position is signed again, while two rows of the release table give up a record on purpose. | No rule changes. In property 1 (line 4281), in the invariant (lines 2282-2289) and at details line 183, replace 'a used position' with 'a position where any computer still holds a record by that key'. StoreNoFork (RestoreGuard.tla:243-244) is that statement and it passes. |
| wording | `docs/host-safety-and-ending-plan.md:4287` | Finding F2 is not a hole in G1's rules, and its trace rests on two peer behaviours the code at HEAD does not have. Property 4 is still worded too strongly. | No rule changes. Reword property 4 (line 4287) and lines 1775-1776 to 'a computer that still exchanges with this one holds the marked record'. In the model, drop the delivery of a removal to the removed peer, or label both behaviours as assumptions. WillingHoldsEnd is then the property. If a later phase builds the notice to removed computers (line 2480), decide then that such a computer still answers the host's computer. |
| test gap | `research/tla/RestoreGuard.tla:229` | The liveness result assumes every peer becomes reachable for good and the goal goes quiet. The real system guarantees neither, and no case keeps the run where one listed computer never answers. | Add one expected-violation temporal case in which one peer never becomes reachable, so lines 1780-1782 are a kept trace. Add one sentence to restore-guard.md that liveness is claimed only for a quiet goal whose listed computers all return. |
| test gap | `scripts/check_tla.py:328` | Two checker processes started at the same moment can fail to parse, because TLC unpacks its standard modules into one shared temp directory. | Add -Djava.io.tmpdir set to the case's own run directory to the command at line 328. |
| dead or leftover code | `research/tla/cases.json:3500` | restore-p5-agent-fenced repeats restore-p1-store with fewer invariants, and three RestoreAll configs carry RestoreKind = "store". | Drop restore-p5-agent-fenced and point property 5 at restore-p1-store. Set RestoreKind = "all" in the three configs. |
| wording | `docs/host-safety-and-ending-plan.md:4238` | The plan still says of the models 'None is written yet', and the guide overstates what restore-p4-setup checks. | Update line 4238 to say which models exist. Make RecoveryReached use RecoveryEligible and register it for the other two scenarios, or correct line 106. |

What held, and what the assessor concluded:

- The registered suite reproduces. All 31 restore cases matched expectation in
  my run, with state counts equal to the retained results file. The retained
  file's case definitions equal the registry's, and its plan hashes equal the
  plan at HEAD.
- First table, row by row, against Start (RestoreGuard.tla:106-118), by
  reading. Row 1 (kept, same file): nothing changes, and a mark ahead of the
  store is behind by the same predicate (line 35). Row 2 (kept, another file):
  behind is derived from the kept marks. Row 3 (lost, same file): marks
  rewritten from the store, nothing held. Row 4 (lost, another file): unheard.
  A mark below the store's tip is raised. Not modelled, and stated or
  harmless: restore_found and revoked invitations, guard_attest, the in-place
  overwrite as its own action, the torn write. The model does not model
  something easier here, except for the missing stop and start after a restore
  (findings 1 and 2).
- Second table, row by row, against Settle (lines 144-162) and the hold
  predicates (lines 35-39), by reading. Marked record held again: derived.
  Never-shared release: govGive with NeverShared. Agent give-up after every
  other computer is heard and governance is not held: agentGive. Host's agents
  held with governance: AgentHeld. Unheard on a member's computer:
  memberHeard, with the 'at least one' clause. Admitted: host heard. Unheard
  on the host ends only on Continue (line 155). GovHeld and AgentHeld match
  Node::hold as written at plan lines 1935-1944.
- Each of the six properties has a case. Property 1: StoreNoFork passes; the
  literal form StoreNoReuse is kept as two expected failures (finding F1).
  Property 2: AllNoFork and AllStayHeld, host only; an admission after the
  copy fits only in the extended case. Property 3: OrdinaryStart passes, but
  by construction, since the 'ordinary' test at line 112 uses the same
  predicates Start sets; it is run only on a scenario with no copy and no
  sync. Property 4: HoldsEnd and WillingHoldsEnd as real temporal formulas,
  AllStayHeld as an invariant. Property 5: AgentFenced. Property 6: six
  mutations and two residuals, all failing as registered.
- The hold that ends is checked as liveness, not as a finite witness. The
  configs use PROPERTY with SPECIFICATION FairSpec or FairGeneralSpec. The TLC
  logs show temporal checking of the complete state space, and both expected
  failures are lassos. The directed positive cases are not vacuous (scratch
  check of the antecedent). I widened the general case from 3 records to 4 and
  5 with one initial peer, so an admission and a removal fit: WillingHoldsEnd
  still passes (181,107 and 1,168,878 distinct states). A member's daemon,
  which no registered temporal case covers, also passes 'restored and running
  leads to agent not held' for RestoreStore and RestoreAll at 3 records
  (29,102 and 33,629).
- Wider safety bounds I could not break with all rules in place: two
  admissions at 5 records (1,367,347 distinct states); a local agent admitted
  after the copy on the host at 4 records (221,062); a goal that starts with
  no other computer, explored exhaustively at 5 records (276,242); a member's
  daemon at 4 and 5 records once the removal exemption is worded by what the
  copy holds (294,288 and 2,359,149).
- The six named counterexamples each fail when their rule is removed, and the
  two residuals fail with every rule present, as the plan asks. Two caveats
  are findings: give-agent-held shows a flag and not a fork, and the hear rule
  and the never-shared row are not reached by any exhaustive case.
- Two things I tried to break by reasoning only, not by a run. First, 'heard'
  stays set for the whole start (plan line 1930) while records can still be
  relayed between peers; I found no fork without a removed holder, because
  every current holder must itself be heard and would bring the record.
  Second, the model's exchange is atomic while the real daemon can sign in the
  middle of one (plan lines 2515-2517); for the rules as written I found no
  fork, because a behind hold ends only when the marked record itself is held.
  The second is exactly why removing the give-up rule is harmful, so it
  deserves a model with a split exchange later.
- Model simplifications I checked and consider safe for the safety results:
  peers can sync to the local daemon even when its copy does not list them,
  and a removed peer that does not know of its removal sends everything. Both
  give the model more behaviours than the code, so real runs are among those
  explored. Copy and Restore do not carry the unheard and admitted records
  although the plan keeps them in the data directory (lines 2009-2014); with
  one copy and one restore I found no run where that hides a signature.

### The file-identity measurements

**What the assessor read and ran.** Read in full:
research/restore-file-identity-2026-10-06.md;
research/evidence/restore-file-identity-2026-10-06/README.md,
measure_macos.sh, measure_linux_container.sh, run_all.sh, results-macos.txt,
results-linux.txt; docs/host-safety-and-ending-plan.md lines 1671 to 2534
(all of G1: goal, changes, tests, exit criteria, risks and notes). Read in
part: docs/host-safety-and-ending-plan-details.md lines 104 to 114 and 200 to
212; docs/master-plan.md lines 20 to 50, 119 to 160, 412 to 432;
crates/locust-store/src/connection.rs lines 1 to 80;
crates/locust-core/src/node/requests/invitations.rs lines 87 to 127; short
excerpts of research/v2-plan-review-2026-10-06.md and
research/v2-plan-review-verification-2026-10-06.md around their file-identity
mentions. Searched with rg: VACUUM and every use of database_path under
crates; FileId under crates (G1 is not built, only the plan defines it). Not
opened: K1's sections and code, the TLA model, the K1 build notes, the rest
of the crates. Ran: measure_macos.sh once. I gave it a path that does not
exist as its argument, so the cross-volume case was skipped and it wrote only
under /tmp/locust-fileid.XMQvQG, which it removed on exit. Without an
argument it picks the first writable volume under /Volumes (measure_macos.sh
lines 23 to 32) and writes a scratch directory there, so I did not run it
that way and did not run run_all.sh, which writes into the repository. My
output: scratch/fileid/rerun-macos.txt. I also ran a small probe of my own on
APFS in the same folder (probe.sh, output probe-out.txt). It is not part of
the committed evidence, and every finding that rests on it says so. Ran
python3 scripts/check_docs.py: it passes. Did not run the Linux script,
Docker, cargo or npm. The working tree holds uncommitted changes from other
sessions (research/tla, README.md, crates/locust/src/cli/only_you.rs). I did
not make them and did not review them. The files I reviewed equal HEAD
a6664a1. I wrote nothing in the repository.

| Kind | Where | Finding | Smallest fix |
| --- | --- | --- | --- |
| a goal stops for good | `docs/host-safety-and-ending-plan.md:1863` | G1 identifies the marks by the FileId of their directory, and a directory keeps its inode and creation time when an older marks file is restored into it, so a restore of the whole home over a surviving home reads the stale marks as kept. | Put the FileId of the marks file itself in its header, read after the file is created, in place of the FileId of its directory. `read` answers None when the file's FileId differs from the header's. A restore with rsync, tar or ditto then reads as lost and the start is row 4. Add a store test that puts an older marks file back into the same directory, and a drill form that restores both folders with rsync -a into the surviving directories. Drop 'rare on a laptop' from residual 2: a restore that writes both folders in place stays undetected for private goals. |
| security or trust boundary | `docs/host-safety-and-ending-plan.md:1743` | Row 1 treats only the goal whose mark is ahead as restored, though an overwritten database rolls back every goal in the file, and the note calls the other goals 'invisible by design'. | In row 1, when any mark is ahead of the store, treat the start as row 2 for every goal and run restore_found for each, because one file holds them all. The cost: on storage that lies about a sync (lines 2393 to 2397) one mark ahead revokes pending invitations in every hosted goal, not one. Add a residual line: an in-place overwrite with no signature in any goal since the copy brings revoked invitations back. Correct the note's sentence at lines 129 to 131. |
| work waits on a human | `research/restore-file-identity-2026-10-06.md:145` | On APFS the creation time of the same file or directory drops when anything sets an earlier modification time, so FileId can change with no copy; the note records this clamp only for new copies and concludes there is no path to a false alarm. | Record the mechanism in the note and add one sentence to the plan's item 'The first table rests on file numbers' (lines 2400 to 2404). The rule itself can stay: Linux needs the creation time. No code change. |
| departs from the plan | `research/restore-file-identity-2026-10-06.md:5` | The note calls itself the note G1's exit criteria ask for, but that criterion requires a reboot and Linux ext4 to be measured and a new residual line where a tool keeps FileId, and none of the three is done. | Say in the note's opening that it meets the criterion in part, and name what is owed: a reboot on APFS, native ext4 (reboot, rename, cp -R, rsync -a), and rows for the marks directory. When they are measured, update plan lines 2358 to 2386, 2400 to 2404, 2528 to 2529 and companion line 207, each with a link to the note. |
| dead or leftover code | `research/evidence/restore-file-identity-2026-10-06/measure_linux_container.sh:74` | The Linux verdict parses the epoch creation time into bb and ab and never uses them, and both scripts pass an unused first argument to verdict. | Delete the two assignments and the unused argument in both scripts. |
| test gap | `research/restore-file-identity-2026-10-06.md:131` | The miss on a file system with inode reuse and no creation time is put together from two separate probes and was never measured, and G1's text says nothing about a FileId whose created_ms is None. | Measure native ext4 with 256-byte and with 128-byte inodes, using the reuse probe and the replace-directory restore. If the default reports a creation time, add one residual line for file systems that report none. If it does not, the rule needs a second discriminator before G1 is built. |
| test gap | `research/restore-file-identity-2026-10-06.md:78` | The note concludes that a copy of both folders lands in row 4, but it measured that only for copies to a new place; a restore of both into the surviving home is neither measured nor listed as unmeasured. | Add restore cases that put the data directory and the marker back together into the surviving base with rsync -a, tar, ditto and cp -R, on both systems, and add their rows to the table. |
| test gap | `research/evidence/restore-file-identity-2026-10-06/measure_linux_container.sh:47` | The Linux fixture is 8 KiB of random bytes, not a SQLite database, with no content check and no ordinary-use case, while the note says the rollback was verified by row counts on both systems. | Install sqlite3 in the container, reuse the macOS fixture with its ordinary-use section, and print row counts. Or state in the note that the Linux fixture is a plain file and that ordinary use on Linux is unmeasured. |
| test gap | `research/restore-file-identity-2026-10-06.md:21` | The rsync measured on macOS is a user-installed 3.2.3 in /usr/local/bin, not the stock /usr/bin/rsync, and the note does not say so. | Run the rsync cases with /usr/bin/rsync as well and name both binaries in the note. |
| wording | `research/restore-file-identity-2026-10-06.md:85` | The first reading bullet says detection of a copy never has to rest on the creation time, and the note's own Linux row shows a restore detected on the creation time alone. | Restrict the bullet to copies that exist beside their source, and say that on Linux a replaced file is told apart by the creation time alone. |
| wording | `research/restore-file-identity-2026-10-06.md:61` | The cross-volume row calls the new creation time the move time; the raw value is the source's own time cut to whole seconds. | Write 'the source's time, cut to whole seconds by HFS+' in that cell. |
| wording | `research/restore-file-identity-2026-10-06.md:163` | Two statements in the note have no line in the raw results: the output of tmutil listbackups, and that the Docker VM is backed by ext4. | Record tmutil listbackups and the backing file system in the scripts, or mark both statements as observed by hand. |

What held, and what the assessor concluded:

- Plain answer. The rule for the database file (inode and creation time of
  locust.db against Identity.file) can stand as written on APFS and on Linux
  file systems that report a creation time. The rule for the marks must
  change: identify them by the marks file's own FileId, not by its directory's
  (first finding). Two measurements are still missing before G1 is built: a
  reboot, and native ext4 including one without a creation time (fourth and
  sixth findings).
- Every macOS row of the note's table follows from results-macos.txt: ordinary
  use lines 20 to 37, copies 40 to 81, rename 82 to 86, cross-volume 87 to 96,
  restores 99 to 128. I checked each inode and creation-time cell and each
  'source's mtime' reading against the printed mtimes. One parenthesis is
  wrong (the wording finding on line 61).
- My re-run on the same Mac reproduced every same or CHANGED verdict of the 15
  cases that need no second volume: 60 identity lines and all row counts. The
  compared verdicts differ only by the cross-volume case, which I skipped on
  purpose.
- Every Linux row follows from results-linux.txt: copies lines 27 to 78,
  rename 79 to 91, restores 94 to 158, reuse probe 13 to 24. Not re-run.
- The note's reading of G1 is right for each measured database case against
  the first table (plan lines 1743 to 1746). A new inode or a new creation
  time gives 'another file', and with the marker untouched that is row 2. An
  in-place overwrite is row 1 and rests on a mark being ahead. With the marks
  lost as well it is residual 3 (plan line 2365). With both written in place
  it is residual 2 (line 2360). The one error in that reading is the second
  finding.
- Missed restores, complete list as I see it. (1) Database overwritten in
  place, marks kept: G1 catches each goal with a mark ahead; it does not cover
  the other goals (second finding). (2) Database overwritten in place, marks
  lost: residual 3, G1 does nothing. (3) Both folders written in place:
  residual 2, only the farm service can add a hold, and only for public goals.
  (4) Both folders restored by a tool that replaces files inside the surviving
  directories: G1 does nothing and the plan does not list it (first finding).
  (5) Inode reuse with no creation time: unmeasured, no line in the plan
  (sixth finding).
- False alarms, complete list as I see it. (1) Ordinary use on APFS: none
  found. Stop and start, a WAL write with checkpoint, VACUUM and rename keep
  inode and creation time, in the committed run and in mine. (2) A move to
  another volume and back reads as a copy of unknown age (results-macos.txt
  lines 92 to 96). Nothing was lost, yet hosted goals wait for one command. G1
  and answer 13 accept that (docs/master-plan.md line 125). (3) File systems
  that do not keep file numbers across a remount: the plan accepts a false
  alarm at each start (lines 2400 to 2404); unmeasured, and the note says so
  (line 178). (4) Creation time lowered on the same inode: third finding. (5)
  Reboot: unmeasured, the note says so. (6) Ordinary use on Linux: unmeasured,
  the note does not say so (eighth finding).
- Locust never replaces locust.db, as the note claims at lines 142 to 144. No
  VACUUM exists anywhere under crates. database_path is only opened
  (crates/locust-store/src/store.rs lines 49 to 51). connection.rs sets WAL at
  line 28 and runs wal_checkpoint(FULL) at line 59.
- APFS did not reuse an inode number in what I could try: five create and
  delete cycles in my probe gave five rising numbers, and the rm plus cp -R
  restore got new numbers (results-macos.txt lines 101 to 102). So on APFS the
  inode alone separated every measured copy and every file-replacing restore.
  This is not proof that APFS never reuses one.
- A rename is not a copy. A home renamed aside and renamed back has lost
  nothing, so 'the file last used' is the right answer there, for the database
  and for the marks.
- Identity.file lives inside the database (plan line 2019), so a copy carries
  the original's record. I checked that the note's before and after comparison
  equals what G1 would compare in each restore case, including the older copy
  renamed into place.
- Leaving the device number out of FileId: no measured case needed it, and the
  cross-volume move changed the inode anyway.
- The note lists the unmeasured cases it knows: Time Machine, Migration
  Assistant, a reboot, native ext4, Finder copies, .restore and VACUUM INTO,
  cloud folders, network and removable disks, Windows (lines 160 to 179). The
  scripts write where the note says they write. Links and index entries are in
  place and scripts/check_docs.py passes.

### The review of the public-goals plan

**What the assessor read and ran.** Repository: .. Nothing was edited, built
or run. Read in full: research/public-goals-plan-review-2026-10-06.md (540
lines); docs/joinable-farms-plan.md (all 5,219 lines); docs/master-plan.md
(all 490 lines). Read in part: docs/host-safety-and-ending-plan.md lines
1-360, 1725-1818, 2345-2474 and 2549-2823, the rest by search only;
docs/roles-and-permissions-plan.md lines 1215-1262, 2296-2330, 2360-2414,
2640-2660 and 4070-4092 only. Code read at HEAD:
crates/locust/src/cli/confirm.rs (whole), crates/locust/src/cli/only_you.rs
360-400, crates/locust-proto/src/api.rs 430-445 and 1692-1785,
crates/locust-core/src/node/requests/goals.rs 60-135 plus searches,
crates/locust-core/src/node/requests/sessions.rs 20-60,
crates/locust-core/src/sync/outbox.rs 150-240,
crates/locust-proto/src/limits.rs 20-45, crates/locust-core/src/node/peers.rs
296-372, crates/locust-proto/src/event.rs 515-535,
crates/locust-farm/src/lib.rs 395-425 and 810-920,
crates/locust-farm/src/main.rs by search. Code read at the review's commit
03fe830 through git show: sync/responder.rs 90-180, organization/validation.rs
118-165, goal/fold.rs 278-300 and 815-835. Searched only:
crates/locust/src/cli/presentation.rs, goal/chain.rs, skills/locust/SKILL.md,
research/v2-phase-k1-build-notes-2026-10-06.md. Evidence folder
research/evidence/public-goals-plan-2026-10-06: README read, round.json only
counted (two checks, 34 findings each, as the review says). Not opened: the
site code, the mockups, the TLA models, the details companion of the host
safety plan, most of the roles plan. One fact that frames everything below:
git log shows docs/joinable-farms-plan.md last changed in b860be2, before the
review's snapshot, and git diff 03fe830..HEAD touches none of the four plan
documents. So the review's line numbers still hold.

| Kind | Where | Finding | Smallest fix |
| --- | --- | --- | --- |
| a goal stops for good | `docs/joinable-farms-plan.md:4200` | Review finding 1. J5 lists a hold on the host's key, with the marks kept, under "Waiting for you" before every computer has answered. A door member's computer that is still off can be the only holder of the missing record, and a host who then continues forks the host log for good. | No change inside v2 closes it; the owner set the redesign aside (master-plan.md:420-431). Smallest change that stops the plan from prompting toward it: delete the new listing condition in item 2 (4200-4207) so G2's rule stands, and say there what it costs. If the owner prefers the prompt, keep the condition and change the block's words at 1791 to the form the plan already uses for an agent's key at 1807, "Waiting for you, or for Pike's computer." Either way add the test where the last door member's computer returns the record after a continue. |
| security or trust boundary | `docs/joinable-farms-plan.md:2589` | Review finding 2. The plan id never reaches the daemon, and farm.door.open and farm.door.admit carry nothing of what the plan showed, so the daemon can apply a rules outcome or a waiting request other than the one the host confirmed. | Give farm.door.open an expected field: the rules binding the plan showed, as rules.bind has, and the door settings as read. The daemon compares before it signs and answers conflict. Give farm.door.admit the name and endpoint the plan showed, or a digest of the waiting entry. Say at 2535-2540 that a waiting entry keeps the name and endpoint of its first ask. Add one test with a change after the command's last read. |
| wrong result for a person | `docs/joinable-farms-plan.md:916` | Review finding 3. A goal made with no flags can be made public only before any task is opened, and the host's own agent at auto can open one between goal create and farm door open with nobody acting. The refusal then tells the host to start a new goal the same way. | In the refusal sentence (916-920) and the host's text (85-87) name locust --owner goal create --formation public. Count the journey from that command at 3816 and 4603, as two commands and two yeses. Add one J4 test that opens a task under public before farm door open. No new command is needed. |
| departs from the plan | `docs/joinable-farms-plan.md:2700` | Review finding 4. One trusted agent's reject removes a door member's task from every other trusted agent's ordinary to_approve list, though the plan says a no blocks nothing. | At 2700-2703 leave out only what this agent itself reviewed. List tasks that another agent rejected after the never-reviewed ones, marked with the reject, inside the same 20 and the same turn by author, and rewrite the test at 2860. Or keep the rule and say at 288-291 and in question 2 (5120-5124) that one reject in practice declines a task, because no list offers it to another agent. |
| work waits on a human | `docs/joinable-farms-plan.md:4887` | Review finding 5. The promise that no person is waited for holds under more conditions than the wait table and the release gate show: the gate covers task approval only, some formations need staffed roles, and status infers a running agent from an open connection. | Widen gate 7 (4887-4893) to the whole default workflow, from a door member's task through result approval, plan settling and file landing, with zero approvals by a person in a chat. Add two rows to the table at 164-177 for review-panel and directed. Say "no session attached" at 2745-2746, and at 4278-4279 say that after the reconnect the wait is on a running session. |
| wrong result for a person | `docs/joinable-farms-plan.md:2467` | Review finding 6. Seven places where the stated order of door states, or a printed sentence, does not give the answer another part of the plan promises. | In test 7 (2463-2471) answer a denied key JoinDenied after ended and halted and before every other door state, with a test of a denial at a closed, full and catching-up door. State at 2541 what the 257th denial does. Give a full waiting list its own sentence beside 670-671. Reword 678 to "This join expired; start again from the goal's page." Print the kept formation's own name at 1164-1165. Correct 612 and 5102. Leave 1619-1621 as it is. |
| test gap | `docs/joinable-farms-plan.md:1931` | Review finding 7. J1 promises that a joiner holds the first record, the members and the rules after the first frame of records. A batch stops at 256 events, and the formation text is separate content. | At 1931-1933 promise only that the governance key's log is sent first. Rename the test at 2046 and run it with a host log of more than 256 records and a formation text that has not arrived yet. |
| departs from the plan | `docs/joinable-farms-plan.md:2122` | Review finding 8. J2 says it holds no command line and also that every command its status prints runs as printed. J3's exit runs farm door open. Both commands arrive in J4. | Move the parser entries and thin wrappers for farm door open, close, admit and deny into J2, with the expected fields of finding 2, and leave the plan texts, the join and the prompt in J4. Or say in J2 that the command lines under the Door line and under "Waiting for you" are added in J4, and make J3's exit at 3510-3513 call the operation. |
| test gap | `docs/joinable-farms-plan.md:4927` | Review finding 9. The release order deploys the new farm service before any gate is known, and after a fallback or a lower ceiling it reruns too few cases, so some gates rest on bytes other than the published ones. The model property in J2 is worded too widely. | Reorder 4920-4937. Qualify against a local or staging service. Make both runs with real agents and the flood before the campaign. Name the final candidate, and after a fallback or a ceiling change run again, on those bytes, every case that reads the ceiling or the changed code. Deploy the service and the site after the gates and before the daemons are published. Add "that needs approval" at 2788-2790. |
| wording | `research/public-goals-plan-review-2026-10-06.md:286` | An unnumbered claim of the review: that the host's clock and arrival order at the door are quietly treated as a fixed owner answer, and should be recorded in the master plan as an interpretation of answer 3. | None in the plan. Strike the request from the review's lines 282-289 and 430-432. |

What held, and what the assessor concluded:

- Overall the review is careful and its citations are accurate. Every plan
  line range I rebuilt says what the review says it says. Of nine numbered
  findings I confirm seven (1, 2, 3, 4, 7, 8, 9) and find two partly right (5
  and 6). I refuted none of the nine outright. I refuted one of the seven rows
  of finding 6 and one unnumbered claim.
- The review's code reads hold where I checked them. The responder serves
  HaltProof before the not-a-member refusal
  (crates/locust-core/src/sync/responder.rs:110-113 at 03fe830). Formation
  validation rejects a zero review count and an empty rule group
  (organization/validation.rs:125-152 at 03fe830). Replay requires a new
  task's rules to be current at its anchor (goal/fold.rs:282-296 and 822-832
  at 03fe830). The farm service lets an existing row past the enrollment
  precheck and answers an identical old request from its receipt before the
  old-sequence test (crates/locust-farm/src/lib.rs:814-846, 870-885, 901-903).
- The review's soundness answer stands as far as I attacked it. I tried four
  routes by reading and broke none: many door identities approving each other
  (count is ignored and every rule needs a closed selector,
  joinable-farms-plan.md:2270-2281); a door member as the only member; a key
  named in the rules coming through the door (2457-2459, 2296-2299); a task
  approval signed by the host key. Two of these rest on K1, which is now
  built: replay excludes a removal of the host's agent
  (crates/locust-core/src/goal/chain.rs:136), the handlers refuse its leave
  and its removal (crates/locust-core/src/node/requests/goals.rs:270-306), and
  host_may_sign admits only governance records and a host's steps, so no
  review (crates/locust-proto/src/event.rs:524-526). plan_join still reads the
  ticket first, as J2 assumes K1 leaves it
  (crates/locust-core/src/node/peers.rs:313-339). This is reading, not a
  proof.
- Its journey counts hold: two commands and two yeses for a host who starts a
  goal and makes it public, against the plan's one and one that begins at an
  existing goal (joinable-farms-plan.md:3816, 4603, 4607-4608;
  master-plan.md:102-105).
- Its two notes on stale status lines hold. The public-goals plan says the
  master plan still lists Phase 3 as being built
  (joinable-farms-plan.md:16-18, 3029-3031), and the master plan says phases 1
  to 3 are built in one place and only 1 and 2 in another (master-plan.md:3-4,
  485-486). Since K1 landed, three more lines in the public-goals plan are
  stale: "today the constants read protocol 6" (507-508, 1927-1928, 2198-2199)
  against crates/locust-proto/src/lib.rs:34, which reads 7.
- Where the review rates too high: finding 2's rules half needs the host's own
  second command inside milliseconds and both outcomes are safe rules, and
  finding 3 is friction and not a safety matter. Both are still cheap to fix
  and worth fixing before J2 and J4 are written.
- What the review missed, by finding. Finding 1: the rule it wants back also
  does not wait for a member admitted after the copy was made
  (host-safety-and-ending-plan.md:1766-1769, 2381-2384), and its fix leaves a
  public host silently held when a door member never returns
  (host-safety-and-ending-plan.md:2793-2796). A wording-only alternative
  exists in the plan's own text (joinable-farms-plan.md:1807). Finding 2: the
  built precedent, rules.bind with expected
  (crates/locust-proto/src/api.rs:438-441,
  crates/locust-core/src/node/requests/goals.rs:146), and the unstated
  question whether a waiting entry's name can change on a later ask
  (joinable-farms-plan.md:2535-2540). Finding 4: a door member with any role
  can hide other door members' tasks by rejecting them, and the review's fix
  must keep the flood bound. Finding 8: the built test
  crates/locust/src/cli/presentation.rs:976 that would fail on a J2 tree.
  Finding 9: the fallback's local items meet the task flood's load and the
  flood is not rerun, and the cases that need a goal at its ceiling cannot run
  with 8 members on the build whose ceiling is 16.
- On the owner questions I agree with the review's main point: that a goal can
  be made public only before its first task is something a person sees and
  loses, and the plan lists it as not asked
  (joinable-farms-plan.md:5159-5160). With finding 3's fix the loss shrinks to
  one flag at creation, so the question can be short. I also agree that
  question 6's "the other choice costs one more command after every restore"
  is too strong, since the plan itself lists another choice (5055-5057).
- The review's verdict is: build after named changes. I agree. None of the
  nine argues against the design, and none blocks the phases being built now.
  The changes sort by phase: finding 7 into J1; findings 2, 4, 6 rows 1 to 3
  and 8 into J2; finding 3 and row 7 into J4; finding 1 into J5; finding 9
  into J6. Two need the owner and not only the author: the choice in finding 1
  between an early prompt and a silent hold, and whether one reject should in
  practice decline a task (finding 4). I would drop row 5 of finding 6 and the
  request about answer 3 from the list of changes.
