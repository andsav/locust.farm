# First-user journey after formations

Date: 2026-10-04. **Status: baseline review with an implemented follow-up below.**
Reviewed source: `c1d27db68cce2dbc5f59208c84c62f9446485573`, API 2 / protocol 2.
This review compares the [original first-contact contract](../docs/first-contact.md)
and [historical last-mile plan](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/last-mile-implementation-plan.md) with the
current runtime, CLI, skill, manual and qualification records. It does not revive
protocol-1 APIs or the universal-coordinator model.

## Assessment and method

The formation implementation supplies much of the machinery the first journey
needed. It does not yet make that journey complete. The main gap is the handoff
from a configured client to understandable, useful collaboration: selecting local
participants, establishing scoped permission, finding relevant shared context,
seeing what needs attention, and reviewing an actual change.

The review used source inspection, read-only CLI help probes with a fresh
temporary home, and a browser walkthrough of the local homepage, `/start`,
documentation index and first-collaboration article. No personal profiles,
services, goals or client policies were changed. No new model campaign,
installation, physical-machine test or public-site verification was performed.
The old command counts and timing measurements were not repeated and must not be
presented as measurements of this version.

The local documentation is legible and navigable. A visual redesign is not the
priority. The larger issues are sequence, actionable state and completeness of
the person/agent handoff.

## Disposition of the earlier work

| Earlier requirement | Current disposition |
| --- | --- |
| W1: distribution and truthful entry | Local test-signed candidate installation, verified manual/license payload and release evidence exist. Public installation still has no published origin/installer; first-contact descriptions also need reconciliation. |
| W2: coherent context and explicit acknowledgment | Partly unresolved. Pinned rules, contributions, pending obligations and durable effect delivery exist; task context is fragmented and event pagination still changes a principal-shared cursor. |
| W3: resumable setup and bound clients | Substantially implemented: `up`, `agent add`, enrollment recovery, owned configuration and bound launchers. Display metadata/rename and integrated doctor checks remain deferred. |
| W4: human views and permission handling | Named goal grants, member/session/viewer operations and limited identity resolution exist. Readable board/task/pending views, cross-goal attention, watch and a first-goal handoff remain missing. |
| W5: useful agent communication | Organization-aware skill, runtime discovery, taskless findings and durable configured handoffs exist. A normal find/read/use context loop and coherent incremental brief are still missing. |
| W6: first useful real collaboration | Current installed native scripted campaigns pass within their recorded scope. Natural-language, separate-principal collaboration with observed use of another agent's finding remains unqualified; the existing model harness needs changes first. |
| W7: second person and invitation UX | Invitation redemption, optional expiry, stdin ticket input, membership and explicit subgroup export exist. Inspectable invitations, inventory/revocation, meaningful remote labels and a guided joining journey remain work. |
| W8: public demonstration/view | Still a separate follow-up. A recorded example should follow a proven private journey. A public swarm visualization is not required to repair first use. |
| W9: optional extensions | Open work, independent attempts and configurable review/closure have been addressed by the new model. Do not reintroduce the old proposals. Hooks, hosted publishing, exclusive reservations and broader execution integration remain separate decisions. |

The frozen [organization implementation plan](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/formations-implementation-plan.md)
explicitly says the old last-mile requirements must be revisited against the new
contract. Its completion does not automatically mark those requirements done.

## 1. Connect setup to a first useful goal

`up` deliberately enrolls agents without goal-management authority. Its final
handoff asks for fresh-chat tool discovery and leaves goal membership and work
authorization as separate owner actions. This preserves permission boundaries,
but does not lead the person through those next actions. See
[onboarding implementation](../crates/locust/src/installation/onboarding.rs) and
[onboarding guide](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/onboarding.md).

The current [first collaboration article](../docs/guide/collaboration.md) is a
valuable executable regression recipe: shell functions, JSON extraction,
principal enrollment, local ticket exchange, grants, claims and assertions. It
launches no agent process. Calling the section “Two local agents” can make a
newcomer expect a native-client workflow that the page does not provide.

**Proposed fix:** add an explicit, reviewed handoff after readiness. The person
states the goal, chooses enrolled local participants and workspace, sees the
proposed organization agreement and separate local permissions, then starts the
chosen sessions. A matching preset should be suggested from the intent; custom
formation authoring should remain available without becoming mandatory first-use
homework. Reuse existing authoring, enrollment and authorization primitives.
Do not silently grant work during setup.

Make the binding visible: multiple native chats using one configured profile
currently share one Locust principal and execution session. Opening another chat
does not create an independently identified participant. The supported separate
profile/binding path needs to be understandable within this handoff.

**Acceptance:** starting with a trusted installed candidate and two existing
clients, a person reaches real collaboration without writing JSON, transferring
local invitation tickets or copying principal/task identifiers. They can decline
a permission, understand the consequence and resume later.

## 2. Give people readable state and actions

The CLI's [human renderer](../crates/locust/src/cli/mod.rs) handles simple receipts,
join/claim and daemon status. Goal status, board, task and pending responses fall
through to pretty-printed JSON. Root help has no human `show`, `watch` or attention
inbox. A named `goal grant` exists, but still requires a JSON grant structure.
The current [doctor](../crates/locust/src/cli/doctor.rs) checks local transport and
credential readiness, not the complete installed-profile/skill/launcher journey.

Some earlier complaints are already resolved: goal prefixes have ambiguity
checks, `--as` can resolve a local enrolled name, and administration is not
restricted to a generic `call`. These do not provide general name resolution for
every agent/task argument or friendly goal inspection.

**Proposed fix:** render the current authoritative projections as concise tables
and detail views: who is involved, what is ready, what is waiting for permission
or review, why something is blocked, and the next action available to this person.
Keep JSON for automation. Add reviewed permission inspect/grant/revoke controls
and non-acknowledging observation before considering a new dashboard.

**Acceptance:** a person can identify a blocked task, authorize only the intended
work, inspect the resulting permission and revoke it without JSON construction.
Another view observing activity does not change an agent's unread state. Display
names remain labels, and observed activity is not represented as proof of a
running remote process.

## 3. Make shared context part of normal work

The [installed skill](../skills/locust/SKILL.md) prescribes status, goal status,
board, pending and task reads. `TaskDetail` carries task text, inputs and rules
with attempt/contribution IDs, but not an attributed progress/review/finding
thread. `GoalStatus` has no common observed-revision token for joining these reads.
See [API views](../crates/locust-proto/src/api.rs) and
[read handlers](../crates/locust-core/src/node/requests/reading.rs).

This is also a correctness issue for context consumption: `events(after)` writes
a stored cursor, and [cursor keys](../crates/locust-core/src/node/feed.rs) are shared
by a principal's sessions. A cursor position does not establish that each session
received and consumed the relevant content. The new durable effect receipt and
agent acknowledgment solve work delivery; they are not acknowledgment of arbitrary
findings and task discussion.

Board/contribution reads collect all matching records, with contributions loading
their full text. Event pages are bounded but expose kind/author/ID without a
task/subject thread. There is no current brief/news operation. The
[MCP schema builder](../crates/locust/src/mcp/schema.rs) also gives reads an
idempotency field and labels every write destructive, rather than describing its
actual effect accurately.

**Proposed fix:** introduce one coherent task/goal context read and explicit,
session-scoped delivery/acknowledgment semantics. Include attributed relevant
findings, exact review reasons, attempts, pending actions, revision and continuation.
Use explicit pagination/preview controls with full content retrievable; do not
introduce hidden content or execution budgets. Build on current contributions,
reviews and documents, not retired protocol-1 notes. Then update MCP descriptions
and the skill to make discovering, publishing and using findings a normal loop.
Correct tool effect annotations without changing client approval policy.

**Acceptance:** interleaved reads from two sessions, lost responses, late payloads
and restart cannot silently consume another session's context. A new task-relevant
finding is discoverable at the next normal work checkpoint and can be read without
reconstructing the whole event log.

## 4. Complete the invitation and second-person journey

The [invitation format](../crates/locust-proto/src/invite.rs) binds a goal,
administrator, endpoint/contact hints, secret and optional expiry. It has no
friendly goal/material preview. Current CLI/API discovery exposes mint/redeem,
not inspect/list/revoke or onboarding with an invitation. `goal join --ticket -`
already accepts stdin, so avoiding shell-history exposure does not require a new
input mechanism.

**Proposed fix:** give the recipient a readable preview before redemption, showing
authenticated facts separately from presentation labels and the actual sharing
boundary. Add invitation inventory/revocation and resumable joining. Offer a
reviewed local-participant addition path so the two-agent local demonstration
does not teach external invitation plumbing first. Names must not masquerade as
authenticated human identity. Preserve joining/admitted and membership/execution
as distinct states.

**Acceptance:** the person can explain what they are joining, who controls it and
what becomes readable; a declined/expired/revoked invitation has a clear recovery
path. Actual two-person acceptance still requires independent physical machines.

## 5. Reconcile the entry copy and prove the intended experience

The local browser journey currently ends truthfully at “report and stop” because
[availability](../docs/reference/availability.json) has no installer URL. That is
a release gate, not a reason to enable an unverified download. However,
[/start's route text](../sites/locust.farm/src/lib/onboarding/guide.ts),
[overview](../docs/guide/overview.md), [installation](../docs/guide/installation.md)
and parts of [first contact](../docs/first-contact.md) still describe earlier
qualification limits. The [current agents article](../docs/guide/agents.md) and
availability record already identify current native scripted-client evidence.
Visitors receive conflicting maturity descriptions. `/start` also inserts a
Polaris product explanation before the agent instructions; it contributes no step
to the standalone route.

**Proposed fix:** provide one current journey and consistent evidence-backed
status for new visitors, trusted-candidate evaluators and already-configured
clients. Keep detailed protocol/trust reference available without making the
first-use page a qualification ledger. Keep the executable CLI tutorial as a
developer example alongside an actual native-agent journey.

The [existing real-model harness](../scripts/check_t2_models.py) supplies numbered
operation sequences, credential-bound CLI prefixes, one principal with different
sessions, and permissive policy. Merely rerunning it with paid models would not
test unprompted first-use discovery or separate-principal collaboration. The
[current native evidence](organization-native-qualification.md) explicitly excludes
real models and interactive approvals.

**Acceptance:** prepare an installed-profile, separate-principal fixture with two
related pieces of work. One agent discovers a constraint that changes the correct
implementation of the other. Give natural-language objectives rather than tool
recipes; observe that the finding was read and affected the artifact. Complete
review, explicit application and independent verification while preserving
unrelated work. Record elapsed time, permission prompts, intervention and failure
points. Paid execution needs an explicit model/spend choice; a scripted control
only validates the fixture. Public distribution and an external user's run remain
separate final gates.

## Recommended next scope

First reconcile the current first-use documentation and design the reviewed
setup-to-goal handoff. Then implement readable owner state/permissions together
with coherent agent context and explicit acknowledgment. Qualify that local
two-agent journey before expanding the second-person flow or investing in public
visualization. Public installer/trust decisions can progress separately.

This is a follow-up over the current organization runtime. It requires no return
to a universal coordinator, no mandatory Polaris application and no revival of
the entire historical last-mile plan.

## Implemented follow-up: status, shared findings and invitations

The owner selected these three functional areas and explicitly declined further
onboarding/tutorial work. The review above preserves what was missing at its
identified source; its onboarding-first recommendation is superseded by that
scope decision. The follow-up uses API 3 / protocol 3 and store schema 3; older
formats are refused without migration.

- Human CLI views now show participant names, task titles, state, reasons and
  actions across status, boards, tasks, pending work and sessions. Owner `inbox`
  collects local work requiring attention; `watch` shows current state and waits
  for one change with a visible timeout. Explicit `permission inspect/allow/revoke`
  controls preserve unrelated grants and expose task-specific authorizations.
  Task allow preserves existing takeover rights; task revoke deletes every
  retained round authorization for that agent/task. Neither implies a running
  process has stopped.
- `context.read` gives a coherent goal/task brief and attributed findings, progress,
  review reasons, inputs, document references and pending actions. Reads never
  acknowledge content. Signed receipts acknowledge exact fully delivered content
  for one principal/session, survive restart and cannot be replayed by another
  session. Previewed and unavailable text stays unread. Pagination survives
  per-page acknowledgments; changed goal state requires a fresh read. The agent
  skill and MCP instructions make read/reuse/cite/publish/acknowledge normal work.
- Signed invitation inspection works offline without consuming the ticket.
  It exposes title provenance, issuer key, expiry and the actual whole-goal
  sharing boundary. Reviewed joining selects an existing local principal and
  confirms the exact ticket digest without adding grants. Inventory exposes no
  capability; unused invitation revocation is durable. Refused joins can recover
  with a fresh invitation. Ticket-bearing operations are absent from model tools.

The [runtime reference](../docs/guide/runtime-reference.md) documents the public
commands. The source and executable checks are in
[human rendering](../crates/locust/src/cli/presentation.rs),
[permission controls](../crates/locust-core/src/node/requests/permissions.rs),
[context checks](../crates/locust-core/src/node/tests/context.rs),
[invitation checks](../crates/locust-core/src/node/tests/invitations.rs),
[CLI/MCP flow](../crates/locust/tests/t2_flow.rs), and
[durable two-daemon checks](../crates/locust/src/daemon/durable_tests.rs).

Local source verification covers named permission edits, observational watch,
MCP publication/read/acknowledgment, independent session news, revoked-ticket
refusal, fresh-ticket remote admission and restart using two real SQLite/Unix/Iroh
daemons on one machine. This does not establish natural-language model uptake,
two physical people/machines, native client approval behavior, an updated Polaris
SDK/package, or published artifacts. Those retain their separate evidence gates.

Final source gates: `cargo fmt --all --check`, strict workspace Clippy, and
`cargo test --locked --workspace` passed: 663 tests, 12 explicit ignores.
Generated API/CLI contracts and all six formation exports match the binary;
documentation links and all four executable manual recipes pass. Independent
review found no blockers in receipt boundaries, pagination, invitation lifecycle
or owner permission controls.
