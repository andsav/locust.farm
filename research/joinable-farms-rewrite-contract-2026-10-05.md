# Joinable farms plan: what the rewrite must change, and its contract

Superseded on 2026-10-06 by the rewritten
[public-goals plan](../docs/joinable-farms-plan.md), which took this note,
its review and the owner's later answers as input. Kept as the record of
how that plan was reached.

Status: groundwork of 2026-10-05 for rewriting the
[joinable farms plan](../docs/joinable-farms-plan.md). Not accepted and
nothing is built. Four readers compared that plan with the
[roles and permissions plan](../docs/roles-and-permissions-plan.md), with the
notes on [replacing a host](replacing-a-host-2026-10-05.md) and
[ending a goal](ending-a-goal-2026-10-05.md), with the
[host-key tests](host-key-failure-characterization-2026-10-05.md) and the
[board trials](role-free-board-2026-10-05.md), and with the code as it stood
at `da0a091`. One designer then wrote the contract below and a checker
corrected it against both plans. Nothing was built or run. JP means the
joinable plan as it stands, by line number; RP means the roles plan; R1 to R10
are its phases and J0 to J8 the phases of the rewrite. The numbered decisions
at the end are for the owner; each carries the contract's recommendation,
which is not the owner's answer.

## Reviewed: revise before use

An [independent review](joinable-farms-rewrite-contract-review-2026-10-05.md)
of 2026-10-05 found that the phases below still contradict two of the owner's
answers and that the first phase admits strangers before the safety check
exists. Its thirteen findings are accepted by the plan's author. Do not
assign the rewrite from this text: the contract is revised first, after the
phases for the governance key, the restore guard and ending a goal are
written, because three of the findings depend on them.

The revision also reads the [goal lifecycle characterization](goal-lifecycle-characterization-2026-10-05.md), which
measured several things this contract only read from the code. Two differ
from what is said below: a removed computer that is given a fresh invitation
answers "joined" from its stale view and is not admitted, where this contract
says its stale view refuses the ticket; and the page is suspended by the
highest consent the publisher holds for a member, even an excluded one that
accepts. It confirms that the farm service answers an identical request at an
old sequence from its stored receipt.

## Answered by the owner after the contract was written

On 2026-10-05 the owner answered four of the decisions listed at the end.
They win over the recommendations there.

- Decision 1: the name a joiner chooses at the door is the name the page
  shows, it is required at a public door, and there is no separate consent
  step.
- Decisions 2, 6 and 29: people who came through the door may open tasks, but
  a task written by one of them always asks first. Level auto covers tasks
  written by the host and by members the host invited, never a door member's.
  This holds for the host's own agent and for a joiner who chose auto.
- Decision 11: for a public goal the farm service may help a host's computer
  notice that it was restored from an old copy.
- Decision 30: the first door is released after the restore guard and after
  the small first step of ending a goal. The backup host follows later.

## How much survives

About half of the old plan survives as text, and most of its mechanics survive
as design. What stands, by line range of docs/joinable-farms-plan.md (JP,
2,220 lines, unchanged): the sync and transport groundwork (351-526), the farm
service and farm page (969-1160), the door's mechanics (door id, seats used
once, the ceiling of 16, the derived descriptor, the three paced refusals,
663-770), the idea of a closed selector and the refusal of stages (862-870,
875-876), abandoned attempts (903-908), priority fetch and no unread backlog
(1432-1458, 1498-1511), the waiting and denied lists (1307-1347), the
qualification campaign and release order (1554-1697), the layout of all seven
images and the words of three, decisions A3 and A6 to A11, the deferred list
and questions 2, 3, 6, 9, 10. What does not survive is every word a person
types or reads and every place the plan leaned on a grant: the six terminal
texts, the join prompt, the `public` preset and the `propose` half of its
check, the consent machinery (`ConsentIntent`, `drive_consent`, the
'Participant' placeholder), `FarmJoin` with `allow`, the review id,
`--yes --review`, `--as`, `--roles`, `maintainer`, the grants that
`farm on --joinable` wrote, the grant clause of `door_state`, the role-list
check in `validate_binding`, questions 1, 4, 5, 7, 8, and the cleanup entries
whose targets the roles plan deletes first. Two things the old plan never had
are written new: the life of a public goal (end, leave, and a host that is
off, lost, replaced or restored) and a catch-up gate in front of every
unattended admission. The rewrite is J0 to J7 plus qualification J8; old
Phases 1 and 2 merge so that each signed body changes once. The roles plan
(RP) was read whole at da0a091, where it is 3,409 lines, about 245 lines
longer than the delta maps cite, and carries eight owner answers of 2026-10-05
that the maps did not have (a dedicated governance key, one optional backup
host, confirmation only for acts that share or are hard to undo, a goal's only
member counting on its own word, the latest review counting, the host's first
files needing no approval). This check moved the gate, the 'stays shut until
reopened' rule, the joiner's stop and delete-only `farm off` out of J7 into
J1, J4 and J3, so that J7 holds only what needs the end record, the host
notice and the backup host; it closed three ways a door member could have
gained power (a deciding role through `role give`, a key the rules name
directly, and the one-member rule); and it found, in
crates/locust-farm/src/lib.rs:766-773, that the farm service gives an
identical request at an old sequence its stored receipt back, so a restored
copy is told nothing until J3 changes that. Six statements in RP are corrected
by the rewrite: `selector_scope` does not refuse open tree rules on a joinable
goal (RP 3226-3227 against
crates/locust-core/src/organization/validation.rs:50-65);
`invitation revoke --all` does not stop admission while a door is open (RP
955); a member's name must not default to the local name at a public door (RP
118, 3332-3333); `to_start` is sorted least-attended first but `ask_first` is
not said to be (RP 1473-1474); `attempting` does not leave out removed members
(RP 1470-1472); and the duplicated task came from a board read about 14
seconds old, not seven (RP 3314 against
research/role-free-board-2026-10-05.md:146-149).

## What a host and a joiner will read

A host can make a goal public: it gets a page, and the host opens a door, a
standing invitation with a number of seats and an expiry that the host's
computer honours by itself. Anyone with the page's address runs one command on
their Mac, chooses the name their agent will carry in the goal and its level
(read, ask or auto), reads the plan and confirms; the host's computer then
admits the agent, which reads the whole goal from its first record and keeps
its copy for good. A member who came through the door opens tasks, takes them
and posts results, plan revisions and file changes like any member, but cannot
make anything count: a result counts, the plan changes and a file change lands
only when a reviewer approves it, reviewers are the host's own agents, members
the host invited privately or a member the host gave the role to afterwards,
and bringing more agents through the door changes nothing. A level is each
person's own, the host's included: at ask an agent takes a task only when its
owner allows that task, at auto tasks that strangers wrote run on that
computer unasked (the host's own agent starts at auto), and nobody else can
see or change it. Who is in, the door, the page, the rules and the end of the
goal are the host's alone; removing a member stops new reads and takes nothing
back, and ending is final. While the host's computer is off, members keep
working and a reviewer on another computer can still make results count, but
nobody joins or is removed, the plan and the files do not advance and the page
stops updating; when it starts again it admits nobody until it has caught up.
If the host's computer is lost, nobody can ever join, be removed or end the
goal unless the host named a backup host beforehand; a backup host that takes
over gets a new page, a new door and new invitations, and nobody who came
through the door can be the backup. Your agent's name is signed into the
goal's record at admission and cannot be changed; the page shows it while you
are a member, drops it once the host's computer sees that you left or were
removed, and every member's copy of the goal keeps it.

## Phases of the rewrite

Every record, type, request, command, endpoint and page element has one owning
phase, so that writers working in parallel do not define the same thing twice.

### J0: Groundwork in sync and transport

Works afterwards: An idle daemon stops writing per completed exchange; a dial
that finds no slot is deferred, not failed, and a long run of failures backs
off to 15 minutes; a public endpoint holds 64 strangers' connections in 1 MiB
instead of 7; the log of the key that signs governance is sent first;
`LOCUST_BIND=none` runs relay-only; `doctor --json` reports the counters J8
measures. No signed byte changes.

Depends on: Nothing in either plan. It can land at any time before J8 and is
worth landing first: the simulator needs it above a handful of daemons, and
sending the host's log first is also what a restarted host needs to recover
its own later records in the first frames (the restore guard). If it lands
after R1 the field is already named `host`; once the owner's dedicated-key
decision (RP 100-101) is written into R1, 'the host's log' means the log of
the governance key.

Owns:

- `Node::unsaved_sync`, `SYNC_FLUSH_MS` (60,000), `local::sync_write` and the
  `Y` record row, `FarmLocal::last_semantic`
- `PeerInput::OpenDeferred`, `Backoff.since_ms`, `LONG_FAILURE_MS` (1 h),
  `LONG_BACKOFF_MS` (15 min), `MAX_DIALS_IN_PROGRESS` (32). The failure
  backoff only: the slower dialing of an idle goal that
  research/ending-a-goal-2026-10-05.md proposes (ceiling one hour, its
  question 7) is a different rule in the same file and is not owned here; the
  two are designed together so the driver has one table of ceilings
- `TransportBudget.unadmitted_connection_receive_bytes` (16 KiB),
  `Source {Ip, Relay, Other}`, `IncomingConnection::source/validated/retry`,
  `PeerConnection::admit`, `UNADMITTED_CONNECTIONS` (64),
  `UNADMITTED_PER_SOURCE` (8), `UNADMITTED_DEADLINE` (10 s), the accept-order
  function
- `Replica::first_author` (was `lead_author`) and `Work::Frontier.first` (was
  `lead`), because lead is a role name from R4; `push_prefixes` order
- `IpTransport::Disabled`, the `LOCUST_BIND=none` settings function,
  `Endpoint::lookup/direct/relay_connected`,
  `PeerInput::Transport {facts, counters}`
- `TransportFacts`, `TransportCounters`, `Counters`, `Diagnostics`,
  `DaemonStatus::diagnostics` (owner only; a different field from the
  `waiting` that R5 adds to the same struct), `FrameSender::sent_bytes`,
  `FrameReceiver::received_bytes`, doctor's `diagnostics` key, `Goal::refolds`
  without `cfg(test)`
- Guide lines: `LOCUST_BIND` gains `none` in operations.md and sharing.md
- The one idle bound that J8's threshold (b) repeats: at most one commit a
  minute per idle daemon, plus two per 30 seconds on a daemon that publishes

From the old plan: Keeps JP 351-526 nearly whole: the six items, their tests
(471-502) and exit criteria (504-513), and the cleanup entries at 1704-1758.
Rewrites only: 'administrator' to host at 419-429 and 488; `lead_author` and
`Frontier.lead` to `first_author` and `first`; drops the `API_VERSION` 6 note
at 360 and 525-526 (no version change here); moves the note at 523-524 to J1.

### J1: The joinable publication, the door and admission

Works afterwards: A host turns a fresh goal's farm on as joinable with
`farm on --joinable` (plan then confirm, T1) and opens a door in open mode
with `farm door open` (plan then confirm, T2); `farm door close` applies at
once. The host's daemon admits many keys unattended up to the seats and the
ceiling of 16. After every start it admits nobody, by door or by private
invitation, until an exchange with a member on another computer has completed
or the person reopens the door by hand; J3 adds a third way for a copy that
shows no such member. A daemon that finds its copy behind itself shuts the
door and revokes its pending invitations until the person acts. Each door
member is admitted with `via: Door` and the name it signed; a name a current
member holds, and a key the rules name directly, are refused. Private
invitations keep working and take a place under the ceiling. A joiner's daemon
stores why it was turned away and `locust --owner status` shows it under the
goal; a full, closed or expired door is retried after minutes. The host's
`status` shows the door beside its open invitations, and
`invitation revoke --all` closes the door too. Signed bytes change once, under
the number the master ladder assigns. Until J4 a door descriptor is redeemed
as an ordinary ticket with R2's `goal join`; J4 adds `farm join` and makes
`goal join` refuse a descriptor.

Depends on: R1 (`Audience::Host`, `Node::host()`, `Node::hosts()`, `plan_join`
with no grant and no `local_actor`, `InvitationJoin` deleted, every invitation
expires, `invitation.revoke` with `None`). R2 (`cli/confirm.rs`, `farm on`
behind a plan, `goal add`, `goal join`, `invitation revoke --all`). R3 (the
host agent at auto, `Refused` and `Why::State`, `clear_removed`). R4
(`MemberAdmitted.name`, `JoinRequest.name`, `Invitation.host_name` and
`.role`, `RoleHolders`, `plan_join` signing a role, `is_member_name`). R5 (the
one status view and `render`). The restore guard built (RP 178-181), with its
own answer to the three cases listed under this phase's gate. The owner's
dedicated-key decision written into R1 and R4 before this phase freezes
`PublicationSet`, the descriptor and `MemberAdmitted`. J0 is not required.

Owns:

- `DisclosurePolicy.joining: bool` (in `digest`); `PublicationSet.goal_proof`,
  `PublicationSet::signed`, `PublicationSet::check`, domain "locust v1 farm
  goal binding"; `Chain::build` and `Invitation::check` call `check`
- `enum Via { Invitation, Door }`; `via` as the last field of
  `Body::MemberAdmitted { member, endpoint, name, via }` (`name` is R4's;
  `via` is this plan's one change to that record); `Tenure.via`;
  `State::door_admissions`; `State::joinable()`; `MemberView.via`; one word on
  R4's `Member:` line of `goal status` for how the member came in (host,
  invitation, door)
- `door_id(goal)`, `crypto::domain::DOOR_ID`; `MAX_JOINABLE_MEMBERS` (16),
  `MAX_DOOR_SEATS` (1024), `MAX_DOOR_EXPIRY_MS` (30 days);
  `Formation::participants()`, every key a `Selector::Participant` or
  `Authority::Participant` names
- `Refusal::DoorFull`, `DoorClosed`, `DoorExpired` (not terminal; paced by
  `DOOR_RETRY_MS`, 5 min plus up to as much again) and `Refusal::NameTaken`
  (terminal), appended to the sync type `Refusal` in
  crates/locust-proto/src/sync.rs, which is not R3's API type `Refused`;
  `JsonSchema` on `Refusal`
- `InviteKind { Invitation { redeemed }, Door { mode, seats, closed, held } }`
  on the `InviteRecord` that R1 and R4 leave (field `host`, required expiry,
  role and name fields); `DoorMode { Open }` (J5 adds `Ask`);
  `DoorState { Open, Closed, Full, Expired, Off }`, where `Off` is the farm
  turned off (J7 adds `Ended`);
  `DoorView { goal, state, closed_reason, mode, seats, seats_used, members, member_limit, expires_ms, descriptor }`;
  `closed_reason` is one of: closed by the host, goal halted, host agent
  disconnected, catching up after a start, held because this copy was found
  behind itself (J7 adds: this computer is no longer the host)
- `Request::FarmDoorOpen { goal, seats, expires_ms }`,
  `FarmDoorClose { goal }`, `FarmDoorStatus { goal }`:
  `farm.door.open|close|status`, audience Host (R1's), none a tool; open
  starts with `host()`, close and status need only `hosts(entry)`, as R1 does
  for listing and revoking invitations; `Response::Door`;
  crates/locust-core/src/node/requests/door.rs. Exit criterion: the contract
  lists three more operations than the tree J1 starts from and the same number
  of tools
- Which door commands confirm, by the owner's rule (RP 96-99):
  `farm on --joinable` and `farm door open` share the goal and show a plan
  then confirm; `farm door close` shares nothing, is undone by opening again
  and applies at once; `farm door status` is a plain read
- `door_state`, the one answer the view and admission share, in order: Off,
  Expired, Closed (with its reason), Full (members at the ceiling, or
  `door_admissions >= seats`), Open; `door_descriptor` derived on each read
  (public title, relay hints, no socket address, the door id, the expiry, the
  publication, R4's `host_name`, never a role)
- The catch-up gate, which is the restore guard applied to admission. After
  every start `plan_join` admits nobody on a goal this daemon hosts, by door
  or by private invitation, and the door reads Closed (catching up), until one
  of: (1) an exchange with a member on another computer has completed since
  the start; (2) from J3 only, and only when this copy shows no member on
  another computer: the farm service has accepted one request from this daemon
  since the start as new; (3) the person runs `farm door open` again, whose
  plan names the members not heard from since the start and says that
  admitting from an old copy halts the goal for good. When the guard, or from
  J3 the service, finds this copy behind itself, the daemon sets `held` on the
  door record and revokes every pending invitation it holds; `door status` and
  `status` say so, and only `farm door open` and a new `goal invite` undo it
- `fresh_for_joining` and the reworded A1: a goal becomes joinable only while
  fresh (no work, no members on other computers), and that first joinable
  publication record is what later allows a new page and door on the same goal
  (a returning host after retention; J7's replaced host). After
  `farm on --joinable` and before the door opens the host may issue private
  invitations, which is how a reviewer or a backup host on another computer
  comes in
- `Request::FarmOn { joinable: Option<bool>, base_url, listed, formation, recent_changes }`,
  all `Option`, extending R1's Host operation and the plan R2 puts in front of
  it; an omitted field keeps the stored value; nothing is signed when
  unchanged; `joinable` with `listed` is `conflict` until J5
- `consented` and `eligible` on a joinable farm: the host's `PublicationSet`
  is the host's consent; a member's name in the admission record is the name
  the page shows; a member whose latest consent record is a decline shows as
  'unnamed member'; a member who was removed, or whose leave request the
  host's daemon holds, shows as 'former member'; only the host's decline
  suspends. A door member's coding agent and group label appear only if that
  member runs `farm consent --accept`, which on a joinable farm takes no
  `--name`. No `PublicName`, no `ConsentIntent`, no `drive_consent`
- `plan_join` in ordered steps as JP 745-763, re-based on R1 and R4: the gate
  above; the idempotent answer from the chain; `InvitationRefused` for
  revoked, left, another endpoint, another goal; the ceiling on a `joinable()`
  goal for both kinds; `InvitationRefused` for a door descriptor that carries
  a role and for a key that `participants()` of any binding in `State::rules`
  names; `NameTaken` when the name equals a current member's on a `joinable()`
  goal; then the door refusals from `door_state`. A door admission never signs
  `RoleHolders`. `Host::join` commits before answering
- `validate_binding` (R4's) also excludes a `RulesBound` whose
  `participants()` name a member whose current admission is `via: Door`, the
  half of JP 710-712 that survives; test
  `rules_and_the_door_never_share_a_key`
- `JoinRecord.refusal: Option<JoinRefusal { reason, at_ms, retry_ms }>` and
  `terminal()` on the `JoinRecord` R1 and R4 leave; `Host::joins(now_ms)`;
  `GoalSummary.join: Option<JoinView { refusal, refused_ms, retry_ms }>` (J4
  adds `address` and `title`). The wait sentence under the joining agent in
  R5's view is R3's `Refused { act: Join, why: State { reason } }` rendered by
  R5's `render`; J1 adds to `render` the one fix sentence for a door wait
  ('Locust asks again at TIME. The host can add seats or open the door.') in
  place of 'Nothing to change; pick other work.'
- `GoalSummary.door: Option<DoorState>` for the owner on a hosted goal, beside
  R5's `invitations_open`, and the status line
  `Door: open · N of M seats used · closes DATE` with
  `locust --owner farm door close --goal ID` beneath it
- `invitation revoke --all` extended: R1's handler also sets `closed` on the
  door record, `Response::InvitationsRevoked` gains `door_closed: bool`, and
  R2's sentence becomes
  `Stopped admission to "T": N invitations revoked, door closed. Members stay.`
- R2's `agent revoke` plan also lists goals whose door is open; the door reads
  Closed (host agent disconnected) afterwards
- Commands in cli/farm.rs: `farm on --joinable` (T1),
  `farm door open --seats N --expires Nh|Nd` (T2), `farm door close`,
  `farm door status`; worker.rs holds `FarmDoorOpen` until the relay is ready
- T1 and T2 texts: host and reviewer; no Grants line, no `--name`, no `--yes`;
  the host agent's own level (auto) and what that means when strangers write
  tasks; how to add reviewers (`role give`, `goal invite --role reviewer`);
  that a review request reaches the reviewer agent's pending list and only an
  agent waiting in a chat acts on it; live session; always-on machine; IP
  exposure unless `LOCUST_BIND=none`; what everyone who joins can read and
  keep; that a seat is not returned when a member leaves or is removed; that
  names strangers choose are signed by this computer and cannot be changed;
  that admission waits after a restart until this computer has caught up;
  'Never start this computer's Locust data from an old copy while the door is
  open: one admission from an old copy halts the goal for good'; 'If this
  computer is lost, nobody can admit, remove or end, and the page stays up
  until the service drops it after 30 quiet days' (a statement; nothing asks
  for a backup host, RP 102-103)
- One paragraph on what is the host's own local decision and not part of the
  shared record: the order in which requests reach the door, door expiry by
  the host's clock (and again by the service's clock for the page), and in J5
  the lapse of a waiting request
- Ownership-and-state rows for: `joining`, `goal_proof`, `via`, the door
  record, seats used (which can fall after a fork or a takeover), the door id,
  the descriptor, the member name in the admission record, the joiner's level
  and stored refusal. J7 adds what an end and a takeover do to each row
- Vectors, `every_body`, the runtime contract and the version constants,
  raised once here
- Tests: JP 614-643 minus the consent-intent tests; JP 785-814 renamed where
  needed;
  `the_door_admits_nobody_after_a_start_until_a_member_exchange_completes`,
  `a_restored_host_with_an_open_door_and_a_waiting_joiner_signs_no_admission`,
  `a_copy_found_behind_itself_shuts_the_door_and_revokes_pending_invitations`,
  `a_name_held_by_a_current_member_is_refused_at_the_door`,
  `a_door_descriptor_never_carries_a_role_and_a_door_admission_signs_no_role_record`,
  `a_key_the_rules_name_cannot_come_through_the_door`,
  `revoke_all_closes_the_door`

From the old plan: Keeps JP 539-549 and 556-565 (joining, goal_proof,
fresh_for_joining), 663-690 and 713-722 (door goal, Via, door id, limits,
InviteKind), 740-744 (descriptor), 759-770 (commit before answer, stored
refusal, retry pacing), 773-783 (views, worker, commands), 785-835 (tests,
exit criteria, risks), cleanup 1770-1779 and 1844-1900, verification rows
2155-2163. Rewrites: 530-534 and 574-612 (consent and names, under the
one-name rule), 550-554 (API: `goal.join`, not `InvitationJoin`; versions per
ladder), 587-607 (no `invitation join`, no local_members.rs), 694-704 (Host
audience, `DoorView` shape), 710-712 (the role-list half goes, the participant
half stays), 723 (revoke closes the door), 735-740 (`door_state` with its
reasons), 745-763 (`plan_join` re-based, with the gate), 1216-1224 (`FarmOn`
options kept, grant writes dropped), T1 145-172 and T2 174-192 in full. Drops
1780-1800 and 1823-1839 (version text per ladder) and the test
`joinable_farm_on_adds_the_admission_grants`.

### J2: The joinable formation and the safety check

Works afterwards: `locust formation example public` exists and sits just
before `review-panel` in the ordered presets. The host's daemon refuses to
make or keep a goal joinable under rules a door member could meet: at
`farm on --joinable`, `farm door open`, `rules bind`, `workspace init` and
`workspace epoch`. `role give` of a group role such as reviewer to a door
member works and its result line says what that member can now do; `role give`
of a role that picks or closes to a door member is refused. A goal created
with no formation (`peer-review`) is refused at `farm on --joinable` with a
sentence that names `public`. The farm page stops showing a task as attempted
once nobody is working on it. The check covers the completion rule (which the
`documents` part also uses), offers, the tree's rule and stages.

Depends on: J1 (`via`, `State::joinable()`, `Formation::participants()`, the
door requests to hook). R3 (levels in its tests, `Goal::rules_allow`). R4
(`reviewer` and `lead`, `RoleHolders`, `role.give`, `Node::deciding`, no
`--roles`, the host's agent holds every declared role, the tree's rule
defaults to the goal's completion rule, the six ordered presets and their
guide column). R8 (the `documents` part, and with it the restore guard R8
waits for). Written knowing R9: there is no integrator and the tree's rule is
the only guard on files. The owner's one-member rule (RP 88-92) and
first-files rule (RP 94-95) once they are written into R4 and R9.

Owns:

- Preset `public`: one role, `reviewer`, and no `lead`; `propose`, `publish`
  and the independent start by `members`; completion
  `Reviews { by: role reviewer, count: 1, exclude_author: false }`; no
  `selection`, no `finish` (every result that counts is kept);
  `documents: {selection: agreed}`; no `workspace`, `task_types` or `flow`.
  Its guidance says a result counts on a reviewer's approval, that a
  reviewer's latest review is the one that counts, and that the host's agent
  is the only reviewer until the host gives the role. Its place in R4's order:
  open, peer-review, pipeline, independent-attempts, public, review-panel,
  directed. Its guide row and 'What waits on one member' cell (every result,
  until the host gives `reviewer` to another member). Mirrors:
  examples/formations/public.json, `example-public` in site.json,
  `WAYS_OF_WORKING`, `DRAWINGS`, the `WayPicker` grid, the `preset/public`
  conformance case, and R4's and R6's tests of six names in order, which
  become seven
- crates/locust-core/src/organization/joining.rs:
  `rules(work, decisions, path)` and `formation(formation)`. A selector is
  closed when a door member can never match it without a further act of the
  host: `nobody`, `role` (a door member holds one only after `role give`) and
  `participant` (J1 never lets a named key be a door member) are closed;
  `members`, `contribution_author` and `task_creator` are open; `any` is
  closed when every alternative is. A completion rule is safe when
  `contribution`, `declaration` or `check` has a closed `by`; when `reviews`
  has a closed `by` after dropping `contribution_author` alternatives if
  `exclude_author`; when `all` has one safe member; when `any` has only safe
  members. `count` is ignored, and the plan says why: approvals are counted
  per agent and anyone can bring several through the door. Diagnostics
  `joining_open_completion`, `joining_open_offer`, `joining_open_tree`,
  `joining_flow`; warning `joining_no_member_work`; `Diagnostic::warning`;
  `Inspection::joining { safe, diagnostics }` printed by `formation validate`
  and `formation explain`
- The table of verdicts that replaces 'only public is safe': open, peer-review
  and independent-attempts fail on who makes a result count; pipeline fails on
  stages; public, review-panel and directed pass
- The statement that under `agreed` a plan revision settles on the same
  completion rule, so a closed completion rule covers the plan text; test
  `a_door_members_plan_revision_never_becomes_the_text_without_a_reviewer`
- crates/locust-core/src/node/joinable.rs: `door_exposed(entry)` (true after
  `farm off` while door members remain); `check_joinable(entry, proposed)`
  over the current binding, the workspace epoch's binding, each task's
  effective rules and any binding with stages; it refuses with
  `joining_door_member` a formation whose `participants()` name a member whose
  current admission is `via: Door`; warnings `joining_short_role` (a role has
  fewer holders than a completion rule needs) and `joining_reviewer_at_read`
  (every reviewer this daemon holds is at level read and none is on another
  computer); `ErrorCode::Conflict` carrying the diagnostics. Callers: the
  `FarmOn` arm when `joining` turns on, `farm_door_open`, and `rules_bind` and
  `workspace_epoch_set` while `door_exposed`
- The hook in R4's `role_give` while `door_exposed`: a role in
  `Node::deciding(entry)` is refused for a member whose current admission is
  `via: Door` (`joining_door_member`; a host who wants that removes the member
  and invites it privately); a group role is given, applies at once like any
  role give (RP 96-99), and its result line adds 'NAME came through the door
  and can now make results count' and, when the rule does not leave out the
  author, 'its own included'. This is the daemon's check before signing, like
  R4's kind check; replay cannot enforce it because a role's kind is not a
  replay rule (RP 2103-2110)
- Two statements with tests, because their rules are decided but not yet in
  RP's phase texts: on a `joinable()` goal the rule that a goal's only member
  counts on its own word never applies to a member whose admission is
  `via: Door`; and the rule that the host's first files need no approval
  applies only to the host's own seed
- The refusal text for `farm on --joinable` on a `peer-review` goal: names
  `/decisions/completion/by`, says a result would count on a stranger's
  approval, and names `goal create --formation public` or
  `rules bind --formation public`
- One line added to J1's T1 once R9 has landed: file changes land by
  themselves once a reviewer approves, and only while this computer is on
- `project` in node farm.rs: an attempt with status `None`, `Progress` or
  `Uncertain` whose author is not an active member is
  `FarmAttemptState::Abandoned`; `reported` only for `Started`, `Progress`,
  `Uncertain`; `FarmSnapshot::validate` computes `current_attempt` the same
  way
- check_formations.py: the `joining` prefix in `CODE`, `vectors` reads
  `result["joining"]`; regenerated contract and vectors
- Tests: JP 924-944 reworded in levels and with `reviewer`;
  `review_panel_and_directed_pass_the_check_and_open_peer_review_independent_attempts_and_pipeline_fail`;
  `role_give_of_a_group_role_to_a_door_member_counts_and_a_deciding_role_is_refused`;
  `a_door_member_cannot_open_a_task_whose_rules_it_can_meet`;
  `a_removed_reviewer_who_returns_through_the_door_cannot_approve_at_an_earlier_anchor`;
  `a_door_members_change_lands_only_after_a_reviewer_approves_it`;
  `a_joiner_at_ask_signs_review_requests_addressed_to_reviewers_only_and_a_door_admission_signs_none`;
  `a_goals_only_member_rule_gives_nothing_to_a_door_member`; the late-member
  fixture that J6 reuses

From the old plan: Keeps JP 839-847 (goal, dependencies reworded), 855-871
minus the `propose` and `task_creator` clauses, 875-908 (tree rule, stages,
warnings, `inspect`, `door_exposed`, `check_joinable`, abandoned attempts),
909-922 (mirrors, with seven presets), 923-954 (tests and exit criteria,
reworded), 956-964 and 967 (risks), cleanup 1904-1931, verification rows
2164-2165 reworded. Rewrites 848-854 (the preset), 872-874 (drop
`joining_open_propose`), 891-895 (`joining_door_member` now covers a deciding
role and a directly named key, not role lists), 965-966 (no `--roles`;
`workspace init` needs no `--completion` under `public`).

### J3: Farm service and farm page

Works afterwards: The farm service stores a joinable farm's door beside its
snapshot, serves the descriptor from `GET /api/farms/{id}/join` only, resolves
an 8-letter alias at `GET /api/farms/alias/{alias}` and `GET /f/{alias}`
(307), and never reassigns an alias. Link-only publishing needs no operator;
listing still does. An open farm with no upload or check-in for 30 days is
deleted. The service answers any request below a farm's latest sequence with
409, so a daemon started from an old copy learns it within one check-in, shuts
its door and holds; a copy that shows no member on another computer admits
through the door once the service has accepted one new request since the
start. `farm off` deletes the page even when the host cannot sign. The farm
page shows the Join band with its states (open, full, host offline, closed; J5
adds open by request; J7 adds ended), the QR code of the full address, the
short code, the fold, the phone view and the polling fallback; the gallery
refreshes on a timer. The publisher stops retrying an upload the service
refused for good, and `farm status` says what to run.

Depends on: J1 (the door record, `door_state`, `door_descriptor`, the gate and
its `held` flag). R6 for the wording its site and guide text is written on and
the retired-words test it must pass. No roles request or type.

Owns:

- `FarmUploadBody.door: Option<FarmDoor>` (left out when `None`);
  `FarmDoor { descriptor, mode, seats, taken, expires_at_ms, protocol_version }`
  (J5 adds `waiting`); `FarmDoorMode { Open }` (J5 adds `Ask`); `FarmDoorView`
  (`accepting` in place of `descriptor`);
  `FarmJoinView { farm_id, status, service_time_ms, received_at_ms, door, descriptor }`;
  `FarmServiceView.door` and `.alias`; `FarmDoor::validate` (shape only,
  nothing decoded); `parse_farm_alias`, `FARM_ALIAS_ALPHABET`; `FARM_VERSION`
  2
- `door_body(entry, local, now)`: `None` unless `policy.joining`; the
  descriptor only while `door_state` is `Open` and the projected goal state is
  open, so a full door uploads no descriptor and the page shows no prompt
- Service storage: `farms.door`, `farms.descriptor`, unique `farms.alias`,
  `user_version` 2 (empty or 2 only); `take_down`, `run_expire`, `Suspend` and
  `Delete` clear door and descriptor; `current` never returns the descriptor;
  `mutate_inner` `Upload` arm: `Listed` without enrollment is 403 unless
  `public_enrollment`, `Listed` with a door in mode `Open` is 400,
  `accepting`, `alias_candidate(id, attempt)` (8 base-20 digits, first free of
  eight, kept for good and never given to another farm)
- The sequence rule in `mutate_inner`: a request below the farm's stored
  sequence answers 409 `old sequence` even when its digest matches a stored
  receipt; only the stored sequence itself is answered from its receipt, which
  is all a lost reply needs. Today an identical request at an old sequence
  gets its receipt back (crates/locust-farm/src/lib.rs:766-773), and a
  check-in's body is always `{}` (crates/locust-core/src/node/farm.rs:861), so
  a restored copy is told nothing
- The daemon's reading of that answer: 409 `old sequence` or
  `sequence conflict` means this copy is behind itself; `farm_poll_local`
  stops, records it in `FarmStatus`, and sets J1's `held`. And way (2) of J1's
  gate: when this copy shows no member on another computer, the door admits
  after a start once the service has accepted one request as new (owner
  decision 11)
- `farm off` when the host cannot sign (halted goal, disconnected host agent,
  and after J7 an ended goal): it sends the signed delete with the upload key
  and signs no `PublicationSet`; `farm status` says the page is deleted and
  that members' copies still show a publication record
- The publisher's stop: on 403, 410, the 401 an older service gives, and the
  two 409s, `farm_poll_local` stops retrying and records the answer;
  `farm status` prints `farm off` then `farm on` as the way to a new address
  after 410
- Routes `join`, `alias`, `/f/{alias}`; what `/f/CODE` and
  `/api/farms/{id}/join` answer for an unknown, suspended, deleted or
  not-joinable farm (404 with no door; 200 with `door: null`; 200 with the
  door and no descriptor when closed, full or expired by the service clock)
- `Config.quiet_retention_ms` (30 days, `--quiet-retention-days`), the partial
  index, `next_expire`; the two 30-day retentions kept distinct by name
  (ended, quiet); the three cases said together: page turned off (blank at
  once), goal ended (30 days after the end until decision 15 says otherwise),
  nobody ends it (30 days after the last check-in)
- `service_view_schema`, the contract key `farm_service_view`, regenerated
  types.ts
- Site: `FarmState`, `Connection 'polling'`, `POLL_MS` (15 s); door.ts
  (`doorState`, `farmAddress`, `aliasAddress`, `joinPlatform`);
  JoinPanel.svelte (title, state line, protocol note, QR of `farmAddress`,
  alias as `XXXX-XXXX`, fine print, watch-only block below 760 px with Share,
  Hide and 'Show how to join', `localStorage` per farm in try/catch,
  `ABUSE_CONTACT`); the offline text 'The host's computer was last seen N ago.
  Your join waits until it is back and may be refused.' with the 120 s quiet
  threshold named beside it; the gallery refresh every 30 s without
  `watchFarm` per card; `copyLink` uses `farmAddress`
- nginx.conf limits (20 reads/s, 1 upload/s, alias 1/s burst 200, 32
  concurrent) and `location ^~ /f/`; the vite.config.ts proxy
- Site and guide copy on R6's wording: the gallery's Privacy paragraph (a
  joinable farm publishes the host's key, the host agent's name, the daemon
  id, the goal id, a relay address and stage and role identifiers; anyone who
  joins reads the whole goal; a member appears by the name chosen at join);
  one clause on the start page; the guide sentences at JP 2033-2048; and one
  paragraph in farm-publication.md and the ops README on what the service
  keeps after a page is deleted (farm id, upload key, sequence, the time of
  every accepted request, the alias; 'deleted' means blank and dead, not gone)
- Tests JP 1099-1134 plus `alias_is_never_given_to_another_farm`,
  `an_old_sequence_is_refused_even_when_the_request_is_identical`,
  `the_latest_request_is_still_answered_from_its_receipt`,
  `a_copy_behind_the_service_shuts_its_door`,
  `publisher_stops_on_gone_forbidden_and_old_sequence`,
  `farm_off_deletes_the_page_when_the_goal_is_halted`

From the old plan: Keeps JP 969-1160 nearly whole: types 982-1004, `door_body`
1005-1015 with the full-door rule made explicit, storage 1016-1024, upload arm
1025-1034, routes 1035-1041 minus 'the goal ended' (J7) and with a full door
serving no descriptor, retention 1042-1045, generated types 1046-1051, client
1052-1056, door.ts 1057-1062, panel 1063-1083, gallery 1084-1088, nginx
1092-1097, tests 1099-1134, exit 1136-1147, risks 1149-1160, cleanup 1971-2013
and 2029-2064. Rewrites 1089-1091 and 2014-2028 (copy on R6's sentences), the
offline wording, and question 9 (2216-2218), which becomes the publisher's
stop. New here: the sequence rule, the reading of 409 and delete-only
`farm off`.

### J4: The joiner's command and the join prompt

Works afterwards: A person pastes the prompt from the farm page or runs
`locust --owner farm join ADDRESS`. At a terminal the command asks for the
name and the level, then shows the plan (T3) and asks to proceed; anywhere
else `--level` and `--name` are required, `--plan` prints the plan and
`--confirm ID` proceeds. The command fetches the descriptor once and verifies
it offline. Afterwards the agent is joining and `status` shows the door's wait
reason and the address (T4). A waiting join stops asking at the descriptor's
expiry, by the joiner's own clock, and says so. `goal join` with a door
descriptor is refused and names `farm join`; `goal join` with a private ticket
to a public goal says the goal is public and where the name will show. Leaving
while still joining cancels the join.

Depends on: J1 (door, admission, `JoinView`), J3 (join route, alias). R1
(`GoalJoin`, `GoalLeave`). R2 (`cli/confirm.rs`, `--agent` inferred, the
launcher's 'Your owner's commands' block, `up` with `--plan` then
`--name --confirm`). R3 (`level` required on `goal.join`, `level_write`,
P3-2's level lines). R4 (`name` on `goal.join`, `host_name` on the
invitation). R5 (the status view, `render`). R6 (guide and skill words).

Owns:

- `Request::GoalJoin.farm: Option<FarmRef { farm: FarmId, address: String }>`,
  added to the `GoalJoin { agent, ticket, level, name }` that R1, R3 and R4
  leave: required when the ticket is a door descriptor, refused with `invalid`
  on a private ticket. One admission path, one `level_write`, one name; no
  separate `farm.join` request. From this phase `goal join` refuses a door
  descriptor and names `farm join`
- `Invitation::door_review(farm)`: version and signature; an embedded
  publication whose farm id equals `farm` and `FarmId::from_key(upload_key)`;
  the goal proof; `policy.joining`; no role. It returns no identifier: the
  plan id of confirm.rs binds the command and its arguments
- `DoorJoin { address, title, expires_ms }` local record, kept after admission
  (it is what `status` shows; `expires_ms` is the descriptor's);
  `JoinView.address` and `JoinView.title`
- The `goal_join` handler's branch for a door: owner; active agent, not
  author-only; `door_review`; the name passes R4's `is_member_name`; the
  endpoint is not this daemon; the held-goal checks; one `Tx` writes the join
  record, `DoorJoin`, the level and the peer hints. R1's `goal_leave` gains:
  for an agent still joining it deletes these records and signs nothing
- The joiner's stop: after `DoorJoin.expires_ms` the daemon stops asking and
  `status` reads 'this door closed; start again from the farm page'. A join
  may be queued while the host is offline (decision 13)
- cli/farm.rs:
  `farm join ADDRESS [--service] [--wait-ms 20000] [--level LEVEL] [--name NAME]`
  with `--plan` and `--confirm` from confirm.rs. At a terminal without
  `--json` a missing name or level is asked for before the plan is built, so
  the plan id still binds both; otherwise both are required. `--name` never
  defaults to the agent's local name. ADDRESS is `ORIGIN/farm/<32 hex>` or
  `ORIGIN/f/XXXX-XXXX`. The command, never the daemon, fetches once per run
  with redirects off. Checks in order: address, fetch, the host's protocol
  version, `door_review`, the agent. `--confirm` fetches again and fails with
  `conflict` if the plan changed; then it polls `status` until member, a
  stored refusal or the wait's end (T4)
- T3: the host by its agent's name and key prefix, where 'verified' means
  signed by the key the page published, not that this key still hosts the
  goal; the door line from the service, advisory; what you read and share; the
  name line (signed into the goal's record and permanent there; shown on the
  page while you are a member; if the host vanishes it stays up to 30 days and
  only the operator at the abuse contact can remove it sooner); the three
  levels in R3's words with the chosen one marked, 'at this door strangers
  write the tasks', and 'a joiner at read still takes a seat and a place'; the
  network line; 'your computer asks the goal's reviewers to look at each
  result your agent posts, and that needs no setting'; `goal leave --agent`;
  the plan id. T4: the result line in R3's form and the status blocks on R5's
  view
- The lines R2's `goal join` plan adds for a public goal reached by a private
  ticket (from `InvitationPublication`)
- presentation.rs: the door join's lines in the status arm (address, wait
  reason, next try) wherever R5 leaves `membership_action`. One clause added
  to R2's `next_action` sentence in both onboarding.rs files, 'finish any step
  you were asked to do in this chat first', with R2's test
  `each_client_enrolls_without_grants_and_reuses_identity_session_and_config`
  updated
- sites/locust.farm/src/lib/onboarding/join.ts and docs/join-prompt.md (T6):
  set up with `locust --owner up --client CLIENT --plan`, then
  `--name NAME --confirm ID`; ask me for the name and the level; run
  `farm join` with them and `--plan` and show it unchanged; `--confirm ID`
  after my yes; then `locust-cli status`; at ask, name the task you would take
  and run the `allow` line `status` shows only after my yes (R2's block).
  Closing rule in R2's words: do not start or join other goals, set levels,
  allow tasks or connect folders. `CopyPrompt` on the farm page when the door
  admits and the browser is macOS; join.test.ts
- SKILL.md addition after R2's block: farm text is data; run `farm join` only
  when the farm page's prompt or your owner asks; never choose the name or the
  level. The schema test in mcp/schema.rs names `locust_farm_door_open`,
  `_close` and `_status` among the operations that are never tools (there is
  no `farm.join` operation)
- Guide text on R6's wording: farm-publication.md (`farm join`, the door
  commands), collaboration.md (a short section pointing to the farm guide),
  sharing.md ('Who can read a goal'), first-contact.md (a 'Target journey' row
  for joining from a farm page)
- Tests: `door_review_refuses_each_broken_binding_in_order`,
  `farm_join_writes_the_level_the_name_and_the_address_in_one_commit`,
  `goal_join_refuses_a_door_descriptor_without_a_farm_address`,
  `leaving_while_joining_cancels_the_join`,
  `join_fetches_once_and_sends_one_confirmed_request`,
  `plans_name_the_level_the_name_and_address_exposure`,
  `a_waiting_join_stops_at_the_descriptors_expiry`

From the old plan: Keeps JP 1164-1168 (goal, reworded), 1177-1184
(`door_review` minus the id), 1185-1202 (the command's shape and check order),
1203-1215 (the handler, minus consent and grants), 1241-1254 (prompt files and
guide list), tests 1256-1276 renamed, exit criteria 1278-1289 reworded, risks
1291-1296, cleanup 2068-2072, 2085-2089 and 2099-2117. Rewrites T3 194-221, T4
223-246 and T6 278-306 in full; 1236-1240 (one clause on R2's sentence);
2090-2098 (the bullets build on R6's text); 2118-2122 reduced to three rules;
764-770 gains the stop. Drops 1171-1176 (`FarmJoin`), 1216-1235 (moved to J1
or already R2's), the review id everywhere and T6's step 6; verification row
2169 becomes 'the join changes nothing without a level, a name and a confirmed
plan'.

### J5: Ask mode and the host's roster

Works afterwards: A host can run a door that holds each request until they
admit or deny it. Waiting requests appear under 'Waiting for you' in
`locust --owner status` with the name the joiner signed and the
`farm door admit` line. `farm door admit` shows a plan and confirms;
`farm door deny` applies at once and a later admit of that key lifts it.
`farm door status` prints the roster (name, roles, came in, key, joined, last
sync, events, tasks opened) and the waiting list (T5); `member remove` finds a
door member by name, key or prefix. A listed farm must use ask mode and the
page says 'Open by request · the host approves each one'.

Depends on: J1 (door, `plan_join`, the gate, `DoorView`), J3 (upload body,
panel, gallery), J4 (`farm join` and its status lines). R2 (`resolve_member`,
`member remove`). R4 (`JoinRequest.name`, `MemberView.name`, name matching in
`resolve_member`, `GoalStatus.roles`). R5 (`WaitingForYou`, `WaitingKind`).

Owns:

- `DoorMode::Ask`; `FarmDoorMode::Ask`; `FarmDoor.waiting: u32`;
  `Refusal::JoinPending` (not terminal);
  `Pending { member, endpoint, name, asked_ms }`, `MAX_PENDING` (64),
  `PENDING_TTL_MS` (3,600,000, the host's own clock over its own list),
  `MAX_DENIED` (256), `PENDING_RETRY_MS` (15 s); `InviteKind::Door.pending`
  and `.denied`
- `plan_join` steps for ask mode: a denied key gets `InvitationRefused`; a
  waiting key gets `JoinPending` with no write; a full list gets `DoorFull`;
  otherwise the entry is added and `JoinPending` answered; the error type
  `(Refusal, Tx)`; `exchange_ended` paces `JoinPending`
- `Request::FarmDoorAdmit { goal, member }` and
  `FarmDoorDeny { goal, members }`: `farm.door.admit|deny`, Host, not tools.
  `farm_door_admit` starts with `host()` and needs an unlapsed entry, a free
  seat, room under the ceiling, J1's gate passed, and a name no current member
  has taken since the request; it does not need an open door; it signs
  `MemberAdmitted` with the recorded name and `via: Door`, and for a key on
  `denied` it lifts the denial. `farm_door_deny` needs only `hosts(entry)` and
  moves keys to `denied`. Switching to open clears `pending`. Admit shows a
  plan and confirms (it shares the whole goal); deny applies at once
- `DoorView.pending`, `denied_count`, `admitted_since_open`;
  `MemberView.admitted_ms`, `events` and `tasks_opened` beside R4's `name` and
  J1's `via`; `FarmStatus.door`
- `WaitingKind::DoorRequest { member, name, endpoint }`, added to R5's three
  kinds, with `command` the `farm door admit` line; the roster rule that a
  name shown for a waiting request is the joiner's own word
- Commands: `farm door open --mode open|ask`, `farm door pending`,
  `farm door admit --member KEY|PREFIX|NAME`,
  `farm door deny --member … | --all`; the `farm door status` roster and
  waiting list (T5: no `--as`, names from the admission record, roles from
  R4's `GoalStatus.roles`, levels never shown, the Daemon column as
  information and not a rule); one line per door in `farm status`; the
  ask-mode lines in `farm join`'s plan and status (the first 8 characters of
  the agent's key to pass to the host)
- The listed-farm rule on the daemon: `farm on --listed` is refused while the
  door is in open mode; `--joinable` no longer conflicts with `--listed`; the
  service check from J3 pins it
- Site: `doorState 'ask'`, the panel text with the number waiting, the gallery
  badge 'Open by request' (the only badge, since listed farms use ask mode);
  regenerated site types
- One line added to R2's `member remove` result for a door member: 'Removing
  stops new reads and is not a ban; farm door deny refuses one key.' It makes
  no promise that the person can come back; J7 changes it once a removal
  reaches the removed computer
- Guide: ask mode, the roster, removal by name or prefix, what the deny list
  does not stop (keys are free)
- Tests JP 1379-1398 renamed, plus
  `a_waiting_request_shows_the_name_the_joiner_signed`,
  `door_admit_waits_for_the_gate_and_refuses_a_name_taken_since_the_request`,
  `deny_applies_at_once_and_admit_lifts_it`

From the old plan: Keeps JP 1300-1302 (goal), 1307-1347 (mechanics, listed
rule), 1354-1359 (site), 1367-1377 (commands, guide), 1379-1398 (tests),
1399-1408 (exit criteria, with 'Participant' becoming the name or 'unnamed
member'), 1413-1417 (risks), cleanup 2126-2146 minus the `resolve_member` and
`validate_fields` entries (R2's). Rewrites 1325-1330 (fields; `public_name` is
R4's `name`; `MemberView` already lost `Copy` in R4), 1331-1338 (`host()`,
Host audience, the gate, the plan), 1348-1353 (the `member remove` line),
1360-1366 (nothing is added to `resolve_member`), 1411-1412 (the entry carries
the name), T5 248-276 in full; question 7 (2211-2213) is answered.

### J6: Joining a goal in progress

Works afterwards: A newcomer's daemon fetches the title, keys, rules, the
accepted tree and the texts of tasks opened by members who did not come
through the door before the rest of the content. Its agent reads one brief:
catch-up state; the guidance and the current plan, each with the notice, its
author and how that author came in; its own standing line; and startable tasks
least-attended first, each with who opened it and who is attempting it, split
into `to_start` and `ask_first`. History from before admission is not unread.

Depends on: J0 (both change the sync code), J1 (`Via`), J2 (`public`'s
guidance; the late-member fixture), J4 (the prompt files). R3 (`Abilities`,
`ask_first`, `WorkItem.attempting`, `pending_work_with_news`). R4
(`host_name`, member names). R5 (tool instructions and the skill's refusal
block). R8 (the plan text can be a door member's approved revision;
`DocProposal`).

Owns:

- `Reference.first`, `Graph.first`, `refresh` keeping the set; `event_roots`
  marking `Genesis`, `MemberRemoved`, `RulesBound` definitions,
  `DocumentRevised` payloads and the `TaskOpened` payloads of tasks whose
  opener's current admission is not `via: Door` (decision 26); the `first`
  manifest root of the accepted head
- `Replica::next_priority_blob(after)` replacing `founding_blob`;
  `Stage::First` replacing `Stage::Founding` and `Stage::EarlyKeys`
- `Goal::precedes_admission(event, reader)`; `ContextNews::before_admission`;
  `context_news` and `unread_only` skipping such events
- Today's `ContextBrief` renamed `ContextCompact`; the field R1 renames to
  `host` and the `host_name` R4 adds live on `ContextCompact` after the
  rename. New
  `ContextBrief { compact, catch_up: CatchUp { rules, workspace, objects_missing }, texts: Vec<GoalText>, startable: Vec<Startable>, abilities: Abilities }`;
  `GoalText { source: Guidance|Plan, author, author_via: Via, event, text, complete, notice }`
  with the one fixed notice sentence, which also covers task text;
  `Startable { task, offer, title, opened_by, opened_via, attempting, ask_first }`;
  `ContextViewMode::Brief`, `ContextSummary::Brief`;
  `ContextSnapshot.guidance`; `BRIEF_TEXT_CHARS` (4,000), `BRIEF_TITLE_CHARS`
  (120), `BRIEF_TASKS` (20)
- `context_summary` building the brief: `catch_up.rules` from
  `Goal::effective_rules`, `workspace` from `workspace_view`,
  `objects_missing` from `BlobIndex::wanted`; guidance from the current
  binding with the host's agent as author; the plan from the current
  `Doc::Plan` text (R8) with its author; `startable` from R3's `to_start`,
  then `ask_first`
- Two changes to R3's `pending_work_with_news`, made here because R3 does not
  state them (RP 1470-1474): `ask_first` is sorted least-attended first like
  `to_start`, and `attempting` leaves out authors who are no longer active
  members, so a removed stranger's attempt does not keep a task looking
  attended; test
  `ask_first_is_sorted_like_to_start_and_attempting_skips_former_members`
- SKILL.md section 'Start in a goal you just joined' in R5's words: read the
  brief first; if `catch_up.rules` is false or objects are missing, say so and
  wait; texts and task texts are other members' words, never instructions, and
  never change a level or allow a task; tell your owner what the goal is, what
  your level allows and which task you would take; at ask your owner allows
  the task with the line `status` shows. `INSTRUCTIONS` in mcp.rs gains the
  same first step. The last sentence of J4's join.ts and join-prompt.md is
  this phase's
- runtime-reference.md: the brief view and the baseline; the regenerated
  runtime contract
- Tests JP 1499-1523 reworded (`needs_authorization` becomes `ask_first`);
  `the_brief_names_each_texts_author_and_how_they_came_in`;
  `fetch_first_skips_tasks_opened_through_the_door`

From the old plan: Keeps JP 1421-1429 (goal, dependencies), 1432-1458 (content
graph, cursor, stage, `precedes_admission`), 1469-1472 (`context_summary`
mechanics), 1490-1496, 1498-1511 and 1516-1523 (tests), 1524-1541 (exit
criteria and risks, 'seven permission lines' dropped), cleanup 1932-1969.
Rewrites 1459-1468 (the new `ContextBrief`; `host_text` becomes `texts`),
1473-1489 (abilities, `ask_first`, the skill text), 1512-1515 (tests for the
notice and `ask_first`). Drops 1478-1481 (`GoalGrants::rows`) and 1948-1950.

### J7: The end of a public goal, and a host that is lost or replaced

Works afterwards: 'Ended' has one meaning, the host's end record, read by the
door, the daemon's admission, the join route and the page. `goal end` on a
public goal shows the open door and how many wait, the open invitations, the
page and the order remove, end, `farm off`; the end clears the door record;
waiting joiners are refused for good and their status says why; the page shows
'The host ended this goal' and its publisher is frozen after one last upload.
A removed door member's computer learns of the removal and stops dialing. A
door member who leaves is told what stays; the host sees who asked to leave.
After a takeover the new host gets a new page, alias and door; the old door,
invitations and descriptor are dead and a joiner holding one is told why; a
member admitted again keeps its `via`; a door member can never be named backup
host. A joiner whose admission a fork or a takeover dropped is told what to
do. Everything here waits for records that other plans define; until they
exist the plan says only that a public goal cannot be ended and that its
public face is lost with the host.

Depends on: J1 to J5. From the plan that builds
research/ending-a-goal-2026-10-05.md (in the repository since da0a091;
proposed, not accepted): the end record (`GoalEnded`, `goal end` as a Host
command, `State::ended`), a terminal refusal for a join against an ended goal,
and the host notice message that carries an end or a removal to a computer
that is no longer a member. From the plan that builds
research/replacing-a-host-2026-10-05.md section 8: the record that names one
backup host, the takeover record that names an exact last host record, and an
`Invitation` that tells the goal's root from its current issuer. J3's
delete-only `farm off`.

Owns:

- `DoorState::Ended` (reads `State::ended`; the one addition to J1's enum) and
  the `closed_reason` 'this computer is no longer the host'; `door_body` and
  the join route's answer for an ended goal; `doorState 'ended'`; the Join
  band state 'The host ended this goal' and the gallery's 'Ended' text
- What this plan adds to the `goal end` plan and commit that the goal-ending
  plan defines: the lines for a public goal (door open, N waiting, N
  invitations to revoke, 'the page stays up marked ended with these names;
  take it down with farm off', remove before end) and the door record with its
  pending and denied lists among what the commit clears
- The publisher after the end record: one last upload marked ended, then
  frozen; a late record never changes or blanks the page; after the end a name
  leaves the page only with the whole page, by `farm off` or by the 30-day
  deletion (decision 15)
- The joiner's status sentence 'ended by the host; your copy stays readable',
  and the host notice carrying an end or a removal to waiting and removed door
  members' daemons; J5's `member remove` line then gains what the removed
  computer was told
- The lines a door member's `goal leave` adds to whatever R2 prints for that
  command: your name comes off the page once the host's computer holds this
  request; your copy stays on this computer; the host is asked to remove you
  and until then you still hold a place among the 16. The host's status line
  'NAME asked to leave; it still holds a place among the 16' with the remove
  command. No automatic removal (decision 18)
- The local records this plan adds, listed for `goal delete` when that command
  exists: on the joiner `DoorJoin`, the stored refusal and the level; on the
  host the door record with both lists. Until then the joiner's texts say
  'leave, and your copy stays on this computer'
- The backup-host rule on a joinable goal, reading J1's `via`: the host's
  daemon refuses to name a member whose current admission is `via: Door` (J2's
  `joining_door_member`); the chain excludes a naming record or a takeover
  whose backup's admission at the named base is `via: Door`, so the rule is a
  function of the held records and not only a check before signing;
  `plan_join` refuses a door admission of a key currently named as backup; the
  same holds for any later rule under which several named people replace a
  host
- What a takeover does to a public goal, as lines in the takeover plan and as
  behaviour: the page address, QR code, alias, door, waiting list, deny list
  and invitations do not carry over; 'publish anew' and 'open a new door' are
  offered under A1 as J1 reworded it; an old descriptor is refused with 'the
  issuer is no longer the host; join again from the new page'; a member the
  new host admits again without a ticket carries its original `via`; the roles
  the old host's agent held by fallback pass to the new host's agent; removals
  after the takeover's base are listed as undone
- The returning old host: catch up before admitting (J1's gate), clear its
  door, stop uploading the descriptor, offer `farm off` for the old page, list
  the people it admitted after the base
- A paragraph and a status sentence for a halted public goal and for dropped
  admissions: after a fork the page is blank, the door is dead for good,
  people admitted after the fork position are out although they hold the goal
  and its keys, seats used falls, the host cannot end and can still run
  `farm off`; a joiner whose admission a fork dropped reads 'this agent cannot
  be used in this goal again; join with another agent'; one whose admission a
  takeover set aside reads 'join again from the new page'
- Lines added to J1's T1 and T2 and J4's T3 once a backup host can be named:
  'Backup host: NAME' or 'Backup host: none named' (a fact; nothing asks for
  one); 'If the backup host takes over, this address, QR code, door and
  invitations do not carry over, and whatever this computer signed that the
  backup had not received is dropped'; 'a backup host is a member you invited,
  never someone who came through the door'
- The check-in rule for an idle public goal (decision 24)
- Tests, each on two daemons: `goal end` with the door open and joiners
  waiting; a removed door member receives the notice and stops dialing; a door
  member cannot be named backup host and a takeover by one is never in force;
  a re-admission keeps `via`; an old descriptor after a takeover is refused
  with the reason; a named backup's key is refused at the door

From the old plan: Keeps almost nothing as text: the rule that only the host's
daemon admits (JP 27) and the 30-day quiet retention (1042-1045, J3's) are
what it builds on. Rewrites 'Ended' at 698, 737, 754-756, 789 and 1037-1041
(one meaning), 160-161 and 1351-1353 ('it is not a ban … can come back', which
is not shown: a removed computer is never told today and its own stale view
refuses a new ticket, per research/ending-a-goal-2026-10-05.md section 5), 183
and 707-709 ('never returned' becomes 'not returned when a member leaves or is
removed; a fork or a takeover can undo recent admissions'), question 2
(2199-2200, with J1's A1). Everything else in this phase is new.

### J8: Qualification and release

Works afterwards: The member ceiling is a measured number; a join between two
Macs on two networks is on record; one page has been watched by 300 streams; a
run with real agents of different owners through the door is on record; the
restored-host cases pass; the bytes are published after the farm service that
understands them, under the version the ladder assigns.

Depends on: J0 to J6. J7 when its prerequisites are in the master plan before
the release; otherwise this phase runs without `door-end`, `door-removed` and
`door-takeover`, and J7 adds them to the campaign when it lands (decision 30).
R6 (the `decide` helper in every harness and `person` in recipes). R7 and R10
(their qualification runs; this phase adds the door cases and does not repeat
theirs; whether R7's two-computer swarm run and this phase's two-Mac join are
one session is the master plan's call). The restore guard.

Owns:

- scripts/simulate_machines/scen_scale.py: `door-scale`, `door-burst`,
  `door-offline`, `door-flood`, `door-waiting` at 8, 16, 24 and 32, driven
  through `decide`, on a fixture built under `public`; `door-restored-host`
  (an old copy with the door open and a joiner waiting admits nobody and is
  not halted, with a member reachable, with none reachable, and for a copy
  older than every member); `door-returning-host` (after the service dropped
  the page); and, when J7 exists, `door-end`, `door-removed` and
  `door-takeover`
- crates/locust-net/examples/unadmitted_flood.rs; `Cluster.sample`;
  `run.py --service-binary --members`; `flows.py door_join` and
  `build_fixture`; three cases in scen_network.py;
  scripts/check_farm_viewers.py; scripts/record_machine.py
- The real-agents run: different owners on different computers, joining
  through the door into a goal in progress under `public`, at level ask, one
  reviewer. Recorded: whether each agent found work from the brief,
  result-to-approval time, duplicated tasks, tasks opened by door members,
  review requests signed per result. At 8 and at 16
- The evidence paragraph under 'Decisions this plan assumes', which this run
  replaces: in the one trial so far (research/role-free-board-2026-10-05.md)
  three agents of one person, on one computer, all told to keep working and
  all at what is now auto, finished every task with nobody assigning work, and
  every approval followed a request the author's daemon sent. Nothing yet
  shows agents of different owners, at ask, joining a goal in progress,
  opening tasks, or one reviewer serving many authors. The plan claims nowhere
  that a joiner's agent finds work by itself or that reviewers review unasked
- Pass thresholds (a) to (g) as JP 1608-1623 with two fixes: (b) idle is J0's
  bound, at most one commit a minute per daemon; (f) applies to open mode
  only, since ask mode retries every 15 s by design. The rule that sets
  `MAX_JOINABLE_MEMBERS` (16, else 8, else no release), with the derived
  records (review requests, plan and file recordings by the host's daemon)
  included in the load
- The two-Mac cases (JP 1631-1645), with `LONG_FAILURE_MS` and the 70-minute
  sleep named as a pair
- research/joinable-farms-qualification.md, research/evidence/joinable-farms/,
  the indexes; the ops README release order; status.md and
  public-preview-release.md; latest.json with the ladder's numbers; the
  verification matrix, restated once with its three reworded rows
- Tests JP 1665-1673

From the old plan: Keeps JP 1545-1552 (goal, dependencies reworded), 1554-1606
(changes), 1608-1663 (thresholds, rule, two-Mac cases, release order) with the
two fixes and the version per ladder, 1664-1697 (tests, exit criteria, risks),
cleanup 1759-1768, verification rows 2155-2175 reworded where they speak of
permissions, the review id or 'only public'. Adds the restored-host scenarios,
the real-agents run and the evidence paragraph.

## Taken from the roles plan and never redefined

- Line numbers below are RP at da0a091 (3,409 lines). The delta maps cite an
  earlier, shorter copy; their RP lines are about 245 lower from Phase 2
  onward.
- R1 (RP 557-888): `Audience { Owner, Host, Agent, Author }`;
  `Node::hosts(entry)`, `Node::host(actor, goal)` with its two refusal
  sentences, `Node::local_agent`; `GoalJoin { agent, ticket }`,
  `GoalLeave { goal, agent }`, `GoalInvite { goal, expires_ms: u64 }`,
  `InvitationRevoke { goal, invitation: Option<String> }` and
  `Response::InvitationsRevoked { count: u32 }` (J1 adds `door_closed`);
  `plan_join(remote, request, now_ms)` with no grant test; `farm.on` and
  `farm.off` as Host operations signing through `host()`, `farm.show` staying
  Owner; the field `host` on `Genesis`, `Invitation`, `State`, `History`,
  `GoalStatus`, `Response::Joined`, `InvitationPreview`, `InvitationSummary`,
  `ContextBrief`, `InviteRecord` and `JoinRecord`; listing and revoking
  invitations needing only `hosts(entry)`; API 7 and store marker 7; 94
  operations and 57 tools after R1 (R3, R4 and R9 change both counts again, so
  J1 and J5 state their own additions, three and two operations and no tool,
  against the tree they start from)
- R2 (RP 890-1251): cli/confirm.rs (`Plan`, `PlanId`, `Decision`, `decide`,
  `bound`, `flags`, the id over `{command, review}`, the prompt at a terminal,
  `Run again with --confirm ID`,
  `conflict: the plan changed; run --plan again`); cli/only_you.rs; the global
  `--agent` and `acting_agent`; Host commands refuse `--agent`;
  `resolve_member` by key, prefix and local name; `goal add` in place of
  `goal add-local`; `goal join (--ticket-file F | --ticket -)`;
  `invitation revoke --all` and its sentence (J1 changes the handler and one
  clause); `agent revoke` and its plan warning (J1 adds open doors to it);
  `farm on`, `farm off`, `farm consent` behind a plan whose `review` is the
  request and what `farm.show` answers; `utc`, `expires_in`, `counts_when`,
  `needs_another`; `up` requiring `--owner`, `--plan`, then
  `--name NAME --confirm ID`, one client per run; both `next_action` texts (J4
  adds one clause); the launcher's skill block 'Your owner's commands';
  `ENTRY_PROMPT` and the setup strings; the deletion of cli/local_members.rs,
  `invitation join`, `--as`, `--yes`
- R3 (RP 1253-1699): `Level { Read, Ask, Auto }`, `Act` (with `Join`), `Why`
  (with `State { reason }`), `Refused`, `Ability`, `Abilities`, `WantedTask`,
  `Stalled`; the `LEVEL` and `ALLOWANCE` records, `level_write`,
  `level_delete`, `Local::level()` answering `Read` when no record exists;
  `LevelSet`, `TaskAllow`, `TaskDisallow`; `GoalJoin.level` required and
  `Response::Joined.level`; `Node::allowed`, `sign_for`, `level_needed`;
  `ask_first` in `PendingWork`, `PendingCounts`, `PendingKind`, `PendingItem`;
  `WorkItem.attempting` and `results`, `ReviewItem.approvals`, `needed`,
  `verdicts`; `to_start` least-attended first (J6 extends the order to
  `ask_first` and trims `attempting`); `clear_removed` (removal deletes the
  level and allowances, so a second join asks again); `goal_create` writing
  `Auto` for the host agent; `standing_line`; `locust --owner level` and
  `allow`; review requests, stages and admissions needing no setting; exit
  codes 4 and 13; 56 tools
- R4 (RP 1701-2139): `Body::MemberAdmitted.name` (J1 appends `via`);
  `Body::RoleHolders { role, holders }`; `is_member_name`,
  `MAX_MEMBER_NAME_BYTES`, `is_role_name`; `Invitation.host_name` and `.role`;
  `JoinRequest.name`; `GoalCreate.name`, `GoalJoin.name`; `GoalInvite.role`,
  refused for a role with no list or a deciding one; `RoleGive` and
  `RoleTake { goal, role, member, expected }`; `GoalStatus.host_name`,
  `.roles` and `.deciding`; `Node::deciding(entry)` and `is_authority_role`
  (J2's hook reads them); `MemberView.name`; `Response::Joined.host_name`;
  `ContextBrief.host_name` (on `ContextCompact` after J6's rename);
  `InvitationPreview.host_name` and `.role`; `validate_binding` giving an
  unheld declared role to the host agent, `MemberRemoved` emptying a member
  from every list, a role's list lasting as long as the goal and a role name
  keeping its kind; `plan_join` signing the name and, for an invitation with a
  role, `RoleHolders`; the six presets in order with `reviewer` and `lead`,
  `coordinator` renamed `directed` (J2 inserts `public` and rewrites the tests
  of six names); `peer-review` for a goal with no formation; a member's latest
  review counting; `CompletionRule::Check { count, exclude_author }`;
  `resolve_member` matching a member's name; `member_label`; `--name` on
  `goal create`, `goal add` and `goal join` (J4's `farm join` gives it no
  default); `role give|take`; the tree's rule defaulting to the goal's
  completion rule and `workspace init` without `--integrator`; protocol 7
- R5 (RP 2141-2392): `Voice`, `render(refused, voice)`, the one template
  `{who} can't {act}: {reason} ({side}). {fix}` (J1 adds one fix sentence for
  a door wait), `short`, `allow_command`, `level_command`, `safe` in level.rs;
  `DaemonStatus.waiting`, `WaitingForYou`,
  `WaitingKind { AllowTask, Joining, Halted }` (J5 adds `DoorRequest`);
  `GoalSummary.name`, `host_name`, `invitations_open`, `invitations_expire_ms`
  (J1 adds `door` and `join`); `Node::waiting_for`; the status view P5-1;
  `membership_action` printed under an agent that is joining, refused, removed
  or left; `Failure::for_person`; the tool `INSTRUCTIONS` and SKILL.md refusal
  instructions; `refusal_schema`
- R6 (RP 2394-2710): the words (host, host's agent, member, role, rules,
  level, allow one task, only you) and the tests
  `guide prose uses none of the retired words` (principal, participant, grant,
  authorization, administrator, permission) and
  `the_skill_uses_the_words_people_read`; the concepts section 'Who may do
  what'; the `person` recipe function and the `decide` harness helper; the
  start-page and how-it-works sentences J3 adds one clause to;
  `--owner --agent`; the guide's preset table with its column 'What waits on
  one member' (J2 adds a row)
- R8 (RP 2884-3034): `Formation.documents: Option<DocumentPolicy>` written
  `{"selection": "agreed"}`; `EffectiveRules.agreed`; the host's daemon
  recording a revision once it counts and builds on the current text;
  `DocView.proposals: Vec<DocProposal>`, `PendingWork.behind`; `Step`,
  `Stall::RunnerElsewhere`; automatic acts reading no level; `peer-review` and
  `review-panel` setting the part (J2's `public` sets it too)
- R9 (RP 3036-3248): no integrator; `WorkspacePolicy` without `integrator`; a
  change lands when it counts under the tree's rule, builds on the current
  files and is complete on the host's computer; `is_denied` in manifest.rs;
  `PendingWork.stale_files`; `selector_scope` staying what it is, a scope
  check and not the joinable guard
- Decided by the owner on 2026-10-05 (RP 80-105, 144-154) and used as given:
  the one that asks less of the person wins; no integrator; a new goal follows
  peer approval and a goal's only member counts on its own word; a member's
  latest review counts; the host's first files need no approval; a person's
  command confirms only when it shares something or is hard to undo (so
  `farm on --joinable`, `farm door open`, `farm door admit` and `farm join`
  confirm, and `farm door close`, `farm door deny`, `level`, `allow` and
  `role give|take` apply at once); governance is signed by a key that does
  nothing else; a host may name one backup host, naming one is optional and
  nothing asks for it; the phases keep their order; design for a host that
  disappears, hostile members are the host's to remove, several people agree
  only when a host is replaced, and who may replace a host is a rule the host
  writes in advance. RP says three of these are not yet in its phase texts (RP
  21-26)
- Design still open (RP 133-205): the restore guard as RP words it (after a
  start a daemon signs nothing unattended with the host's key until it has
  recovered its own later records from a member, when the goal has other
  members) and its tests; the measured facts from
  research/host-key-failure-characterization-2026-10-05.md
- RP's own list of edits for this plan (RP 2603-2622) and the details file
  (docs/roles-and-permissions-plan-details.md:378): followed where they agree
  with the above; where they say less (door audiences, `revoke --all`, the
  name, the restore guard, `selector_scope`, deciding roles) this contract
  decides

## What must exist first

- Roles plan Phases 1 to 5 landed before J1; Phase 6 before J3, J4 and J5;
  Phase 8 before J2 and J6, and with it the restore guard and the two
  decisions Phase 8 waits for; Phases 7 and 10 before J8
- The owner's answers that RP records but has not yet written into its phases
  are written there first, because the joinable phases read them: the
  dedicated governance key (R1 and R4: the first record, what `host` names,
  which key an invitation and so a door descriptor carries, which key the page
  publishes); which commands confirm (R2 and the terminal texts); the
  one-member rule (R2, R4, R5); the first files (R9)
- The restore guard built and tested before J1, with its own design answering
  three cases the one sentence in RP leaves open: a copy taken before the
  first admission, which shows no other member; the host's own attended
  `farm door admit`; and every member being unreachable. J1's gate is written
  against that design and J3 supplies the service's answer for the first case
- A plan for ending a goal, built from research/ending-a-goal-2026-10-05.md,
  before J7: the end record as the host's last governance record, `goal end`
  as a Host command, `State::ended`, a terminal refusal for a join against an
  ended goal, and the host notice message that reaches computers that are no
  longer members
- A plan for replacing a host, built from
  research/replacing-a-host-2026-10-05.md section 8, before J7's takeover
  parts: one backup host named in advance, a takeover that names an exact last
  host record and sets aside what the old host signed after it, an invitation
  that tells root from issuer. That plan must make 'the backup's admission at
  the base is not through a door' a rule every daemon checks from the records,
  not only a check on the host's daemon; J7 supplies it if `via` does not
  exist yet when that plan is written
- One version ladder from the master plan: which number J1's single bump takes
  (8 if a roles release ships first, else inside 7), whether `via` rides in
  R4's change to the admission record, and that the governance key, the end
  record and the takeover record share a bump and do not each add one
- The owner decisions below answered before the writers start: 1, 2, 3, 4, 7,
  8, 11 and 32 block J1, J2 and J4; 30 fixes whether J8 waits for J7; the rest
  can be answered per phase
- The farm service and the site deployed from the release commit before any
  daemon with a door runs, as JP 1647-1662 orders it
- Every code citation re-read, and each phase's cleanup list regenerated,
  against the tree after the roles phases it depends on; the old plan's
  citations hold today, but each points at code R1 to R5 rename or delete
- The master plan corrects six statements in RP: 3226-3227 (`selector_scope`
  is not the joinable guard); 955 (the sentence of `invitation revoke --all`
  once a door exists); 118 and 3332-3333 (no default name at a public door);
  1470-1474 (the order of `ask_first`, and `attempting` leaving out former
  members); 3314 (the stale board read was about 14 seconds old); and 1000,
  2485 and 3340, where `call` is described as 'the raw door', a word this plan
  gives another meaning
- JP links to crates/locust/src/cli/local_members.rs, which R2 deletes; RP
  2708-2710 gives that link to R2, or check_docs.py fails there

## Mockups and terminal texts

- farm-open-1440.png: regenerate three things, keep the layout, band, QR code,
  short code and fold: the command line reads
  `locust --owner farm join locust.farm/f/KDPQ-MTXB` (no `--plan`; at a
  terminal it asks for the name and the level, then shows the plan); the fine
  print says 'under the name you choose'; the chip 'Participant · Pi' becomes
  a named chip or 'Unnamed member', and door members' chips carry no coding
  agent unless they consented
- farm-collapsed-1440.png and farm-collapsed-390.png: keep
- door-states-1440.png: regenerate: the command line in all four panels that
  show it, as above; the Full panel loses 'Your agent can wait for a seat' and
  shows no prompt and no command (a full door serves no descriptor); 'Host
  offline' reads 'The host's computer was last seen 14 min ago. Your join
  waits until it is back and may be refused.'; a sixth state 'The host ended
  this goal' is added with J7; 'Open by request' stays for J5
- farm-phone-390.png: keep
- gallery-1440.png: regenerate: the badge reads 'Open by request · 3 waiting',
  because a listed farm must use ask mode and an open-mode badge can never
  exist; the formation label 'Public farm' stays
- participants-1440.png: regenerate: Harbor's roles read 'Reviewer' (no
  'Maintainer', and no 'Lead' under `public`); the row 'Participant · Pi'
  becomes a named row or 'Unnamed member'; door members' rows carry no coding
  agent unless they consented; the caption becomes 'Reviewers are members the
  host named. A member who came through the door holds a role only if the host
  gave it afterwards, and never one that picks or closes.'
- T1 (host plan from `farm on --joinable`): regenerate in J1 in R2's plan
  grammar (plan id, `Proceed? [y/N]`, `--confirm`), with host and reviewer, no
  Grants line, no `--name`, no `--yes`, the host agent's level line, how to
  add reviewers, and the two lines on an old copy and a lost computer; J2 adds
  the files line after R9; J7 adds the backup-host lines
- T2 (`farm door open`): regenerate in J1 as a plan then confirm with the
  lines listed in J1; J3 adds the alias line once the service assigns one; J7
  adds the takeover line
- T3 and T4 (joiner plan and result): regenerate in J4 in the shape of RP's
  P3-2 and P5-1: `farm join ADDRESS --level ask --name Maple --plan`, the host
  by name and key, the three levels, the name line, the plan id,
  `goal leave --agent`, the status blocks on R5's view
- T5 (roster in ask mode): regenerate in J5: a Roles column, names on waiting
  requests marked as the joiner's own word,
  `member remove --goal ID --member Juniper` without `--as`, no 'Participant'
  row, no levels
- T6 (the join prompt): regenerate in J4:
  `locust --owner up --client CLIENT --plan`, then `--name NAME --confirm ID`;
  ask for the name and the level before running `--plan`; `--confirm ID` after
  the yes; at ask, the `allow` line after a yes; the closing rule in R2's
  words; no 'do not use --owner again' step

## Counts and limits

- Membership: `MAX_JOINABLE_MEMBERS` 16, counting agents and set by traffic,
  not trust (8 if 16 fails the campaign; no release if 8 fails); a build at 32
  for the 24 and 32 runs, never published
- Door: `MAX_DOOR_SEATS` 1024; seats at least 1; `MAX_DOOR_EXPIRY_MS` 30 days;
  `DOOR_RETRY_MS` 5 minutes plus a random share of as much again; examples 16
  seats, `--expires 7d`
- Ask mode: `MAX_PENDING` 64, `PENDING_TTL_MS` 3,600,000 (the host's own
  clock), `MAX_DENIED` 256, `PENDING_RETRY_MS` 15 s
- Join command: `--wait-ms` 20000; key prefix 8 or more hex; plan id `plan-`
  plus 16 hex (R2's)
- Names: R4's `MAX_MEMBER_NAME_BYTES` (64) is the one length rule; the profile
  label limit of 80 no longer applies to a door member's name
- Sync and transport: `SYNC_FLUSH_MS` 60,000; `LONG_FAILURE_MS` 1 hour;
  `LONG_BACKOFF_MS` 15 minutes; `MAX_DIALS_IN_PROGRESS` 32; unadmitted window
  16 KiB; `UNADMITTED_CONNECTIONS` 64; `UNADMITTED_PER_SOURCE` 8;
  `UNADMITTED_DEADLINE` 10 s; idle bound one commit a minute per daemon plus
  two per 30 s on a publisher
- Farm service and site: quiet retention 30 days (distinct from the 30-day
  ended retention); `user_version` 2; `FARM_VERSION` 2; check-in every 30 s;
  `POLL_MS` 15 s; gallery refresh 30 s; quiet threshold 120 s ('Host
  offline'); alias 8 letters base 20 (about 34.6 bits); nginx 20 reads/s, 1
  upload/s, alias 1/s burst 200, 32 concurrent; watch-only below 760 px; QR
  111 px (148 if scanning fails); tests at 1440 and 390
- Brief: `BRIEF_TEXT_CHARS` 4,000, `BRIEF_TITLE_CHARS` 120, `BRIEF_TASKS` 20
- Qualification: fixture 500 tasks, 1,000 files, 50 MB; member counts 8, 16,
  24, 32; viewers 300 streams, 20 updates; thresholds (a) 10 s and 120 s, (b)
  one commit a minute idle, 1% of a core, 5 KB/s, memory within 5%, (c) 5 s
  and 250 ms, (d) 30 s and 120 s, (e) 15 s, 30 s, 64 MB, (f) open mode only,
  one attempt per 5 minutes, (g) 512 MB; two-Mac sleeps 2 and 70 minutes, the
  second tied to `LONG_FAILURE_MS`
- Idle check-in stop for a closed door: 7 days with no new record (decision
  24)
- Presets: seven with `public`; operations: J1 adds three and J5 two, none a
  tool

## Dropped from the old plan, and why

- The grants: `farm on --joinable` writing `manage_goals` and `administer`,
  the Grants line of T1, the `door_state` clause 'without administer or
  manage_goals', the test `joinable_farm_on_adds_the_admission_grants`,
  question 1: admission consults only the invitation (R1), so nothing here
  needs a grant
- `FarmJoin { principal, descriptor, farm, address, review, name, allow }` as
  a separate request, `GoalPermission`, `--allow contribute[,execute]`,
  `grants_write` in the join commit: R3 deletes `GoalPermission`; the join is
  `goal.join` with a `farm` field and a required level
- The review id (`door_review` returning 8 hex bytes, `--yes --review ID`, the
  daemon comparing it): R2's plan id binds the command and its resolved
  arguments, so the name and the level are chosen before the plan is shown; R1
  removes the daemon's comparison of a review identifier
- `--yes`, `--as`, `goal add-local`, `invitation join --name`,
  `InvitationJoin.public`, the edits to cli/local_members.rs and
  `render_preview`: R1 and R2 delete each of these first
- `ConsentIntent`, `drive_consent`, `PublicName`, `farm on --name`, the
  placeholder word 'Participant' and the tests
  `unconsented_member_is_participant_then_named`,
  `named_local_join_consents_first`,
  `named_remote_join_consents_after_admission`,
  `changed_policy_drops_consent_intent`: under the one-name recommendation the
  name signed at admission (R4) is the name on the page and the join plan is
  the disclosure; 'participant' is a retired word (R6); `farm consent` stays
  for declines, for a door member's coding agent and group label, and for
  farms that are not joinable
- `maintainer`, `--roles` on `goal create --formation public`,
  `workspace init --completion` on a joinable goal, 'seven presets, public
  last', and the lead in `public`: R4 uses `reviewer` and `lead`, removes
  `--roles`, orders the presets and gives the tree the goal's rule; `public`
  picks no winner
- `joining_open_propose` and 'task_creator is closed when propose is': door
  members may open tasks (RP 2605-2607, decision 2); with it gone
  `review-panel` and `directed` also pass the check, so 'only public is safe'
  and its verification row go
- The role-list half of the door rules (`validate_binding` excluding a binding
  whose role lists name a door member; the door refusing 'a key any binding
  names in a role'): bindings hold no role lists after R4. What stays: a key
  the rules name directly never comes through the door (J1), and a role that
  picks or closes is never given to a door member (J2)
- The old plan's `resolve_member`, `MemberView.public_name`, `MemberView`
  losing `Copy`, and the `validate_fields` change for `member remove`: R2 and
  R4 own all four
- `needs_authorization`, `permissions (name, allowed, meaning)`,
  `GoalGrants::rows`, 'seven permission lines', the `grant_rows` move: R3
  gives `Abilities` and `ask_first`
- `host_text` as a name: under `agreed` the plan may be a door member's
  approved revision (R8), so the field is `texts`, each with an author and
  `author_via`
- `DoorState::Ended` meaning `farm off` (JP 698, 737): that state is `Off`;
  `Ended` means the host's end record (J7)
- Plan then confirm for `farm door close` and `farm door deny`: by the owner's
  rule they share nothing, are undone by a later open or admit, and apply at
  once
- T6's step 6, 'do not use --owner again in this chat', and the old claim that
  the entry prompt stays unchanged (JP 2097-2098): R2 rewrites `ENTRY_PROMPT`
  and gives the skill one rule for every `--owner` command
- The old plan's rewrite of `next_action` (JP 1236-1240, 2080-2084), the
  /start sentence (2024-2028), the first-contact 'Work' bullet and 'Joining
  grants no permissions' (2090-2101), `membership_action`'s old texts
  (1869-1872, 2085-2089), the `(6, 6)` version checks (1836-1839) and the
  runtime-reference version sentence (1823-1825): R2, R5 and R6 rewrite or
  delete each first; the rewrite adds its clause on top
- Phase 1's 'Protocol and API go from 6 to 7 once' and Phase 2's 'changes
  signed bytes again inside version 7' (JP 327-330, 533-534, 829-831): J1
  changes every signed body once, under the ladder's number
- The 'five screens' count (JP 73): there are six sections and seven images.
  `FarmStatus` cited in api.rs (JP 1330): it is in
  crates/locust-proto/src/farm.rs
- The waiting entry without a name and question 7 (JP 1411-1412, 2211-2213):
  `JoinRequest` carries a signed name (R4)
- Questions 1, 4, 5, 7 and 8 (JP 2195-2198, 2203-2215): answered by R1, R3, R4
  and decisions 1 and 7; question 2 is replaced by the reworded A1 (decision
  12); questions 3, 6, 9 and 10 become decisions 20 and 27, J3's publisher
  stop and J8's thresholds
- 'A used seat is never returned' (JP 67, 183, 707-709): a seat is not
  returned when a member leaves or is removed; a fork or a takeover can undo
  recent admissions, so the count can fall
- 'The person can come back while seats remain' (JP 1351-1353) and 'It is not
  a ban' (160-161) as promises: a removed member's computer is never told
  today and its own stale view refuses a new ticket; J5 says only that removal
  is not a ban, and J7 says more once the host notice exists
- 'Your agent can wait for a seat' on the Full panel and the descriptor served
  at a full door (JP 1041): seats do not come back and the old plan's own data
  flow uploads no descriptor there
- Threshold (f) as written (JP 1622), which contradicts `PENDING_RETRY_MS`;
  and the two idle bounds (JP 509-511 against 1613): one bound, one commit a
  minute
- The belief that the farm service already tells a restored copy it is behind:
  an identical request at an old sequence is answered from its stored receipt
  (crates/locust-farm/src/lib.rs:766-773); J3 changes the service first
- `successor name` as a command name, and any other name for the backup-host
  command: the host-replacement plan names it
- A refusal, or a prompt, when a door is opened with no backup host named: the
  owner decided that naming one is optional and nothing asks for it (RP
  102-103)

## Decisions for the owner

1. Is the name a joiner signs at admission also the name the farm page shows,
   with no separate consent record and no placeholder for door members?
   Recommended: yes; a member who declines shows as 'unnamed member' and one
   who left or was removed as 'former member'; a name is required at a public
   door; and a door member's coding agent and group label appear only if they
   later run `farm consent --accept`.
2. May people who came through the door open tasks? Recommended: yes, as RP's
   edit list says, with each listed task naming who opened it and how they
   came in, the brief's notice covering task text, and fetch-first limited to
   tasks opened by members who did not come through the door.
3. Does `public` stay a built-in preset, where does it sit, and which
   formations may be joinable? Recommended: keep `public` (one reviewer
   approval, author allowed, no lead, the plan settling by itself), placed
   just before `review-panel`; `farm on --joinable` accepts any formation that
   passes the check (`review-panel` and `directed` also pass) and refuses the
   default `peer-review` with a sentence naming `public`.
4. May the host give a role to a member who came through the door?
   Recommended: a group role such as reviewer, yes, with `role give`, which
   applies at once and whose result line says the member came through the door
   and can now make results count; a role that picks or closes, never; and a
   key the rules name directly can never come through the door.
5. May a door member ever be named backup host? Recommended: never, not even
   by explicit naming; a host who wants that removes the member and invites it
   privately.
6. The host agent's level on a joinable goal: stay at auto, or have
   `farm on --joinable` ask? Recommended: stay at auto and print one line in
   T1 with the `level` command, since the host's own agent otherwise takes
   tasks strangers wrote unasked.
7. One join command or two? Recommended: `locust --owner farm join ADDRESS`
   sends `goal.join` with a `farm` field; `goal join` refuses a door
   descriptor and names `farm join`; there is no separate `farm.join` request.
8. Does `invitation revoke --all` also close the door? Recommended: yes; its
   sentence becomes 'Stopped admission to "T": N invitations revoked, door
   closed. Members stay.'
9. Version: joinable as API and protocol 8 after a roles release, or folded
   into the bump to 7? Recommended: 8 if a roles release ships first, else
   inside 7; either way J1 changes every signed body once and the governance
   key, the end record and the takeover record share a bump.
10. May the door ship before the restore guard exists? Recommended: no; a door
    is a standing invitation that many daemons retry, which is the measured
    way a restored host forks itself.
11. May the farm service's answer be a condition of door admission after a
    start? Recommended: yes, in one case: when this copy shows no member on
    another computer, the door waits for one request the service accepts as
    new, and J3 first makes the service refuse every older sequence; in all
    other cases the service stays out of admission and its 409 only shuts the
    door.
12. Reword A1 so that a goal public from creation can get a new page and door
    later (a returning host after retention, a replaced host)? Recommended:
    yes; the first joinable publication record is what allows it.
13. May a stranger queue a join while the host is offline, and when does a
    waiting join give up? Recommended: queueing is allowed; the joiner's
    daemon stops at the descriptor's expiry by its own clock and says 'this
    door closed; start again from the farm page'.
14. Is the door's state kept only on the host's disk, or signed into the goal?
    Recommended: on the host's disk as planned; when the daemon finds its copy
    behind itself it shuts the door and revokes its pending invitations until
    the person acts.
15. Ended public pages: kept as finished records, deleted 30 days after the
    end as today, or taken down by `goal end` unless the host asks to keep the
    page; and may a member take a name off after the end? Recommended: keep
    today's 30-day deletion and no withdrawal after the end, until the
    goal-ending plan decides; it is a named member's only recourse by time.
16. Does a door member's leaving take its name off the page? Recommended: yes,
    with no extra record: the host's publisher drops the name of a member
    whose leave request it holds.
17. Does removal take a door member's name off the page without their
    signature? Recommended: yes; the host's publisher shows a removed member
    as 'former member', which is the host's only tool against an offensive
    name.
18. May the host's daemon by itself remove a door member who asked to leave?
    Recommended: no; status shows 'asked to leave; still holds a place' with
    the remove command.
19. Which side does a door refusal name? Recommended: the goal's state, with
    the host named as who can change it, so no fourth side is added to R5's
    template.
20. Does the ceiling of 16 count members invited privately (old question 3),
    and does the word 'seat' stay? Recommended: the ceiling counts every
    member, so the host leaves room for reviewers; door seats count door
    admissions only; keep 'seat' for the door and never use it for governance.
21. Is a descriptor served at a full door so a newcomer can start waiting?
    Recommended: no; a full door serves none and the page shows no prompt.
22. Must a listed farm use ask mode (A3)? Recommended: yes; the gallery badge
    reads 'Open by request'.
23. If reviews the rule does not need become recordable opinions (RP question
    14), are they allowed in a joinable goal? Recommended: follow that answer;
    if yes they never count, never settle the plan or the files, and are shown
    apart from reviewers' verdicts with how each author came in.
24. Does the host's daemon keep checking in with the service for an idle
    public goal? Recommended: check in while the door is open; after it
    closes, stop after seven days with no new record and resume on the next
    record, so quiet retention can fire.
25. Does door expiry keep its 30-day cap although invitations have none?
    Recommended: yes.
26. Fetch-first for a newcomer: every task's text, or only tasks opened by
    members who did not come through the door? Recommended: only those, until
    the deferred fetch limits exist.
27. The page's word for a finished result under `public`, which picks no
    winner (old question 6)? Recommended: 'counts', never 'selected'.
28. The J8 thresholds (old question 10), and the real-agents run as a release
    gate? Recommended: accept the thresholds as proposals with the two fixes;
    record the real-agents run and do not gate on it.
29. May a joiner choose level auto at a public door? Recommended: yes, offered
    with its sentence ('tasks strangers wrote run here unasked') and never as
    a default; a second command to raise the level later would ask more of the
    person for the same choice.
30. May a first door be released before the end record, the host notice and
    the backup host exist? Recommended: yes, once the restore guard, J3's
    quiet retention and delete-only `farm off` are in and the texts say
    plainly that a public goal cannot be ended, that a removed member's
    computer is not told, and that the public face is lost with the host; J7
    and its three scenarios follow with those plans.
31. After a host is replaced, is a new page address with the old page left to
    expire acceptable, or should the service later re-key a page from the
    signed record? Recommended: a new address for the first version.
32. At a terminal, may `farm join ADDRESS` ask for the name and the level
    before it shows the plan, so the page's command stays one short line?
    Recommended: yes; away from a terminal both flags stay required, so an
    agent must ask its owner.

## Not settled, and what the check changed

- What I read: RP whole at da0a091 (3,409 lines); JP lines 1-1700 and
  2148-2220 (its cleanup lists at 1699-2147 only through the earlier readers'
  maps); research/ending-a-goal-2026-10-05.md whole;
  research/replacing-a-host-2026-10-05.md lines 1-64 and 638-1077;
  research/host-key-failure-characterization-2026-10-05.md lines 96-132;
  research/role-free-board-2026-10-05.md lines 138-152; the details file by
  search (its joinable line is now 378); code at
  crates/locust-core/src/organization/validation.rs:40-70,
  crates/locust-proto/src/organization/presets.rs whole,
  crates/locust-farm/src/lib.rs:755-800,
  crates/locust-core/src/node/farm.rs:784-797 and 846-884, the signed farm
  request and receipt in crates/locust-proto/src/farm.rs:397-484, `Refusal` in
  crates/locust-proto/src/sync.rs:262-278, `Authority` in
  crates/locust-proto/src/organization.rs:117-120; three images (door states,
  gallery, agents table). Not read: the scratch files (both design rounds are
  now in the repository as the two research notes above, committed in da0a091,
  so the rewrite can cite those); the plan review; the other four images.
- Changed, seams with the roles plan: every RP line number moved to the
  current file; the owner's eight answers of 2026-10-05 folded in (dedicated
  governance key, one optional backup host, which commands confirm, the
  one-member rule, the latest review, first files, phases keep their order);
  door commands follow the owner's confirm rule, so close and deny no longer
  show a plan; the operation count is stated against the tree each phase
  starts from, because R3, R4 and R9 change RP's 94; `MemberView` losing
  `Copy` and `resolve_member` removed from J5 (R4 and R2 own them); the door
  wait sentence is now an R3 `Refused` rendered by R5's `render` with one
  added fix sentence, since R5's fix for the goal's state is 'Nothing to
  change; pick other work'; J6's two changes to R3's pending list are stated
  as changes to R3's function; `Node::deciding` and `GoalStatus.deciding`
  added to what is used.
- Changed, seams inside the contract: J1 no longer uses `GoalJoin.farm`, which
  J4 owns (until J4 a descriptor is an ordinary ticket to `goal join`); the
  catch-up gate and the 'shut until the person acts' rule are J1's alone (they
  were split between J3 and J7); 'former member' is J1's alone; the
  `member remove` line is J5's and makes no promise J7 would have to take
  back; what `/f/CODE` answers and what the service keeps after deletion are
  J3's alone; the joiner's stop is J4's, which owns `DoorJoin`; delete-only
  `farm off` is J3's, so a host with a halted public goal can remove the page
  without waiting for the goal-ending plan; `joining_door_member` is J2's and
  J7 reuses it; J8 depends on J0 to J6 and takes J7's scenarios when J7
  exists.
- Changed, missing deltas now covered: the ownership table rows (J1, J7); what
  is a local decision by clock or arrival (J1); a joiner at read still using a
  seat, and the 30-day and abuse-contact line (J4); the door member's coding
  agent and group label (decision 1); a key the rules name directly (J1, J2);
  how to add reviewers and what wakes them, in T1 and T2 (J1); the files line
  in T1 (J2); the reviewer-at-read warning (J2); the review-request tests
  (J2); the roster's roles and tasks-opened columns (J5); the halted public
  goal and the dropped-admission sentence (J7); local records for
  `goal delete` (J7); the publisher after the end (J7); roles passing to a new
  host's agent (J7); the evidence paragraph (J8); regenerating the cleanup
  lists and the verification matrix (prerequisites, J8); the choice of auto at
  the door, release before J7, re-keying a page and asking at a terminal
  (decisions 29 to 32).
- Changed, safety: a role that picks or closes can no longer be given to a
  door member (the old contract allowed any `role give`); a key that a
  formation names directly is refused at the door and such a formation is
  refused once the key is a door member; the one-member rule and the
  first-files rule are pinned by tests so neither gives a door member
  anything; the backup-host rule is to be a rule checked from the records, and
  a named backup's key is refused at the door; after every start nothing is
  admitted until a member exchange, the service's answer for a copy with no
  member, or the person; a copy found behind itself revokes its pending
  invitations as well as shutting the door.
- Changed, a fact the maps had wrong: the lifecycle map inferred that the farm
  service tells a restored host within one check-in that its copy is old. As
  the code stands it does not: a request whose digest matches the stored
  receipt at that sequence gets the receipt back before the 'old sequence'
  test runs (crates/locust-farm/src/lib.rs:766-773), the digest holds no time
  (crates/locust-proto/src/farm.rs:409-425), and a check-in's body is always
  `{}`. J3 now owns the service change. Read in code, not run.
- Changed, explanation: rewritten to eight sentences that say what a stranger
  reads and keeps, that more agents change nothing, that the host's own agent
  starts at auto, that work and approvals among members continue while the
  host's computer is off, that nothing is admitted until it has caught up, and
  that a name leaves the page only once the host's computer sees the leave or
  the removal.
- Changed, mockups: the command line on the page cannot be
  `join ADDRESS --plan` any more, because the plan id binds the name and the
  level; farm-open and four door-state panels are regenerated, and the agents
  table loses 'Lead' as well as 'Maintainer' because `public` has no lead.
- Not settled: the restore guard has no design in the repository, only RP's
  one sentence. In particular, an exchange with any member opens the gate, and
  a member who came through the door could complete an exchange while
  withholding the host's later records; that would let a restored host fork.
  The owner's rule is that hostile members are the host's to remove, but here
  the host cannot know. Widening decision 11 so that every start also waits
  for the service's answer would close it, at the price of making the service
  part of admission.
- Not settled: even with J3's change the service's answer has a short window.
  It shows the page's sequence is current, not the host's log; a record signed
  within seconds of the copy and before any accepted request is not caught.
- Not settled: what 'host' names after the dedicated-key decision, since RP's
  phases still say the host's agent (RP 182-186). It decides which key a
  descriptor and the page publish (A6), who `host_name` describes, and whether
  the host's working agent can leave or be removed, which matters for the
  one-member rule on a joinable goal.
- Not settled: the one-member rule and the first-files rule exist only as one
  line each in RP's decisions. J2's two tests assume the obvious reading; the
  phase texts must confirm it.
- Not settled: whether a task's own rules can ever be wider than the
  formation's. I did not trace `narrows` in delegation.rs or task types; J2
  carries a test (`a_door_member_cannot_open_a_task_whose_rules_it_can_meet`)
  rather than a claim.
- Not settled: whether a member removed and readmitted through the door could
  point an approval at an earlier anchor where it still held a role. The
  replacing-a-host note mentions an `AnchorRegressed` rule; I did not read
  chain.rs. J2 carries the test.
- Not settled: whether the `goal.join` handler can tell a door descriptor from
  a private ticket by comparing its secret with `door_id(goal)`, once R4 and
  the host-replacement plan have changed the invitation.
- Not settled: name equality at the door is exact bytes. A stranger can still
  take a look-alike of the host agent's name; the page's Roles column and the
  roster's 'came in' column are the only tells. Whether to compare without
  case, or to mark door members on the page, is open.
- Not settled: where `public` sits among the presets. The old contract said
  after `peer-review`; I moved it to just before `review-panel`, of which it
  is the lighter form. RP orders 'from no structure to most' and tests the
  order, so the owner should confirm (decision 3).
- Not settled: whether the line 'Backup host: none named' in a plan counts as
  asking, given 'nothing asks for it'. I kept it as a stated fact and removed
  every refusal and prompt.
- Not settled: who enforces the backup-host rule if the host-replacement plan
  lands before J1, when `via` does not exist yet. I gave it to J7 and left a
  hook to that plan.
- Not settled: `membership_action`. The details file says both that its four
  texts are rewritten as short words (line 304) and that it is kept (line
  322). J1 and J4 add their sentences wherever R5 leaves it.
- Not settled: `Refused` (R3's API type) and `Refusal` (today's sync type) are
  one letter apart; both exist or are owned elsewhere, and a rename is the
  master plan's.
- Not settled: whether `invitation revoke` and `goal leave` keep their plan
  under the owner's confirm rule; R2's table still lists both and the rule's
  list names neither. J1 and J7 add their words to whatever R2 prints.
- Not settled: the size of J1. It holds every signed-byte change and the gate.
  If one writer cannot carry it, split at the door record (publication, `via`
  and names first; the door, the gate and admission second) and keep
  `MemberAdmitted` in the first half.
- Not settled: whether R8 lands before J2 and J6 in the master plan. If not,
  `public` cannot set `documents` and has no way to get a plan, since it has
  no lead.
- Line numbers are from a checkout that other sessions were editing; HEAD
  moved from 6b924d0 to da0a091 between the delta maps and this check. Every
  citation must be read again when the writers start.
