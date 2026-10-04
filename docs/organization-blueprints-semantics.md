# Organization blueprint semantics and removal inventory

Date: 2026-10-04. **Status: accepted signed semantics and replacement inventory.**
This document develops [the accepted direction](organization-blueprints.md) and
[implementation plan](organization-blueprints-implementation-plan.md). API 3 /
protocol 3 is implemented. The [execution ledger](organization-blueprints-status.md)
records exact source checks and remaining qualification; this specification is
not itself proof that every acceptance campaign has passed. The removal inventory
records the completed replacements and their enforcing sources. Historical
protocol-1 findings remain labeled evidence rather than active runtime support.

## Accepted authority boundaries

Each goal has one administrator for membership and rules. Work organization,
review and local execution are separate concerns. Rule-authorized members can
contribute without asking the administrator for each event. A goal can explicitly
name different authorities for a task's exclusive reservation and for selection
of one output. Neither authority is required for non-exclusive work.

An unavailable administrator blocks changes requiring that administrator. An
unavailable reservation or selection authority blocks its dependent decision.
Previously authorized independent work and non-exclusive evidence may continue
when their necessary proofs are available. Missing authorization evidence is not
permission: that individual action waits for verification. This preserves the
owner's availability requirement without inventing authority from local absence.

A role is a selector resolved through authenticated bindings, not a self-asserted
permission. A blueprint never grants local filesystem, tool, spending or sharing
rights. Goal membership remains the read boundary; separate topics or roles do
not make private subgroups.

## Accepted daemon-driven flow

D14 requires configured transitions to advance and ready work to be durably
delivered by the daemon without an agent requesting each step. Readiness is a
pure projection; materialization is an authorized signed mutation. A configured
materializer signs child-task, review-request and handoff effects. The logical
effect ID binds the current rule revision, trigger, scope and intended action.
The event, effect deduplication record and delivery outbox are committed together.
Restart resumes pending work and retries unacknowledged deliveries with the same
identity. Recipient acknowledgment and local execution start are separate from
delivery. Execution continues to require local permission grants.

YAML and exclusive reservations remain deferred; reservation stories below are
O13 requirements only if that package is selected. D14 does not bring O13 forward.

## Evidence and context required by the signed implementation

The following are protocol obligations, not field names or already implemented
wire structures. Exact encodings and transition rules must be reviewed in O0/O3
before signing or accepting these records.

| Context | Required distinction |
| --- | --- |
| Definition | Exact semantic identity and supported interpretation; presentation changes do not rewrite executable identity |
| Governance | Authenticated membership/rule administrator history and admission/removal context |
| Rule and bindings | The exact scope/round, effective rule and eligible identities used for this action |
| Encryption | Content-key epoch, independent of authority and rule revisions |
| Subject | Exact contribution/attestation identity and relevant task/attempt, never a mutable title or latest-result pointer |
| Decision stream | Named authority, scope, predecessor and generation when uniqueness is required |
| Proof closure | Typed dependencies, signed ancestry and retained content needed to reproduce a verdict |

A receiver must distinguish invalid evidence, missing proof, unsatisfied
criteria, satisfied criteria, and disputed authority. It must not turn missing
proof into a negative review or treat first arrival as a final decision. With the
same admissible evidence and context, replay must produce the same projection.
Positive evidence can be monotonic within a fixed context; a context affected by
a discovered fork or unresolved removal cutoff cannot claim irrevocable finality.

Approval attaches to an exact candidate. Multiple candidates can independently
satisfy a review threshold. Selecting one is a separate optional decision, and
applying it to a checkout is a separately authorized local operation. Neither
approval nor selection asserts that a remote process has stopped.

## Concrete scenario expectations

These are golden-story requirements to turn into executable transcripts, not
reports of passed tests. Names denote distinct authenticated identities; the
administrator is Ada, participants are Bea and Chen, and reviewer Dev is distinct
from both. Scenarios assume stated proof dependencies are locally available
unless they explicitly describe missing evidence.

| Scenario | Events and expected outcome | Required proof or pending issue |
| --- | --- | --- |
| Open, no task | Ada admits Bea and Chen under open rules. Ada goes offline. Bea publishes a finding; Chen publishes an independent finding. Both remain contributions, with no assignment, accepted head or closure required | Each contribution binds to authorized rule/membership context; the usable-prefix and exact-cutoff rules below govern forks/removal |
| Human/external input | Bea records a human-supplied artifact with provenance. The signed claim is Bea's attribution; it is not silently a signature by the human or an eligible review from them | Exact artifact hash, attribution and signer are separate; imported text cannot mint membership |
| Coordinator arrangement | The task delegates assignment and selection to Chen. Bea accepts an offer and submits C1. Chen selects C1 under that task's rule | Chen's scope and exact subject are proven; the rule does not make Chen authority for unrelated open research |
| Peer review | Bea submits C1. Dev approves C1 under a rule requiring one eligible non-author reviewer. Bea's own approval does not count | Pin reviewer eligibility, subject and rule context; a later C2 does not inherit C1's review |
| Review panel | Bea's C1 receives Chen and Dev's eligible approvals under a threshold of two distinct identities. Duplicate Dev signatures count once | Eligible distinct principals must be determined from pinned bindings; matching role names do not prove distinct humans or organizations |
| Competing results | C1 and C2 each meet the panel rule. Both qualify. If selection is absent, both remain; if Chen is the selection authority and offline, only selection waits | No winner from timestamps, event hash or arrival order; threshold approval is not consensus on one winner |
| Exclusive pickup | Bea and Chen request the same task reservation. The named authority durably confirms one generation for Bea. Chen cannot infer ownership from a timeout | Atomic reservation/receipt persistence and predecessor checking; an uncertain reply is queried/retried with the same request identity |
| Lost reservation reply | The authority commits Bea's grant, crashes before reply, then restarts. Bea retries. The same durable outcome is returned | Current-model recovery cannot issue a second effective grant or treat a retry as a fresh acquisition |
| Replacement and stale worker | The authority explicitly replaces a reservation with a new generation. An old-generation submission cannot exercise rights reserved to the current holder | Local execution-session fencing is separate. The old process may still run; contributions outside reserved rights follow their own explicit rule |
| Authority fork | Two incompatible successors of the same reservation/selection predecessor are discovered | Halt/dispute the affected scope; retain fork evidence. No silent hash-order winner and no global stop for unrelated authorized work |
| Pipeline | C1's exact approved evidence makes a downstream task ready. Chen receives and accepts the handoff offer | The daemon creates and delivers the configured effect without an agent request. Readiness is not launch or receipt acknowledgment; replay must not duplicate the child task or offer |
| Mixed goal | Open research proceeds while code awaits Dev's review and benchmark candidates await Chen's choice | The waiting choice does not freeze research or review. Each scope's evidence and authority are independent |
| Missing proof | C1 and Dev's review arrive before Dev's binding proof | Review is pending verification, not accepted, rejected, or discarded; fetching the proof permits deterministic reevaluation |
| Removal and re-admission | Ada removes Dev while a review is disconnected, then later re-admits Dev | The signed admission tenure and exact cutoff ancestry settle eligibility; missing ancestry waits. Re-admission never legitimizes excluded old-tenure history |
| Changed rules | A task pinned to two-review approval has one review when Ada changes future defaults to one reviewer | The existing task stays under its pinned rule. An explicit authorized revise/reopen transition must name the new context; prior signatures retain their original meaning |
| Different subgroup | A separate goal with different members receives explicitly exported inputs and returns a contribution | Parent acceptance remains its own rule. Topic labels cannot stand in for confidentiality or export consent |

## Revision, concurrency and recovery obligations

Draft edits use compare-and-swap on their current revision. If Bea and Chen both
edit revision R, only one update can replace R; the loser receives a conflict
with enough information to preserve and reconcile their work. Validation is tied
to the exact revision validated. Publishing must not race a later edit and
silently publish different bytes. None of these operations launches work.

A published definition is immutable. Updating future defaults, rebinding roles,
revising active task rules and reopening a closed round are explicit transitions
within the supported current model. They must name the expected prior context
and authority. They are not schema migrations and do not reinterpret existing
signatures. The initial offline authoring slice does not implement this catalog,
CAS, publication, or runtime revision lifecycle.

Fresh state initializes directly to the supported schema. Restart reopens that
same schema and recovers its durable decisions, receipts, claim generations and
proofs. Unsupported state is rejected clearly before mutation without deleting
it, converting it, loading an old decoder or falling back to another runtime.
Recovery must preserve uncertain outcomes and permit idempotent queries/retries.

## Concrete protocol-2 implementation contract

This section fixes implementation choices under D11/D12 and the corrected D14;
it is the proposed signed-contract handoff for implementation review, not a claim
that the existing runtime enforces them. No parallel protocol reader is added.
The cutover replaces protocol 1's event variants and fold in place. Exclusive
reservation events are absent until O13 is selected.

### Header, genesis and context

Retain the existing exact-byte canonical codec, signature domains, event IDs,
per-author `seq`/`prev`, causal `parents`, diagnostic `at_ms`, sealed `payload`,
and goal binding. Increment the protocol marker and refuse every other marker
before decoding. Reinterpret `anchor` as the exact administrator-chain event
known when authoring; it no longer names a work decision. The administrator's
next governance event anchors to its governance predecessor. Work events may
anchor to any verified governance position that does not regress along their
chosen author branch. Content epoch is derived at that anchor and must match
sealed references. A wall clock never grants rights or decides races.

`Genesis` pins administrator key, random salt and initial definition semantic
hash. Its canonical fields derive `GoalId`. The administrator signs genesis,
which establishes authority but admits nobody. Genesis, the administrator's
self-admission and initial `RulesBound` commit together. Initial role bindings,
input references and the sealed definition object belong to that binding event. The semantic
hash is domain-separated canonical encoding of the normalized typed definition;
the encrypted object hash is separate. The same plaintext definition may have
different encrypted object identities after resealing.

Every scoped work event names `Context { scope, round }`. `TaskId` explicitly
distinguishes `Authored(EventId)` from `Derived(EffectId)`; there is no ambiguous
byte-string namespace. The initial task-open event is the first round identity; taskless contributions use
the goal definition/binding revision as their round. The referenced round pins
its definition, role bindings, creator, inputs and effective task variation.
These values are not inferred from the latest definition. Membership, removal
cutoffs and key epoch derive from governance; pinned roles alone cannot make a
removed principal a current member. A review names both exact contribution and
round. Concrete authority roles must resolve to exactly one authenticated key.

Governance retains one serial stream for admission, removal, role bindings,
future-default changes, explicit task-round changes and key rotation. A task
creator may select an already delegated variation at creation; changing active
rules requires an administrator-authored new round naming its expected previous
round. Changing future defaults affects only new scopes. No old signature is
reinterpreted as an action in a later round.

### Minimal event set and authority

The names below are intended Rust variants; descriptive field groups can become
small structs. Keep authority classification explicit by matching variants,
not enum declaration order.

| Event | Required subject/context | Signer and effect |
| --- | --- | --- |
| `Genesis` | Administrator, salt, initial definition hash/object | Pinned administrator; establishes the goal |
| `MemberAdmitted` | Member, endpoint, new admission event identity | Administrator; opens a distinct tenure |
| `MemberRemoved` | Member, exact admission ID, optional last accepted `AuthorPoint` | Administrator; closes that tenure and rotates content epoch |
| `RulesBound` | Definition/object, authenticated role map, input map, expected rules revision | Administrator; changes future defaults; initial binding is committed at creation |
| `TaskRevised` | Task, expected round, new definition/variation and bindings | Administrator; creates a new round without reinterpreting prior evidence |
| `TaskOpened` | Pinned goal rules revision, variation, inputs, criteria payload, optional parent | Selector authorized to propose; creates a task and initial round |
| `WorkOffered` | Task/round, recipient, optional predecessor offer | Selector authorized to offer; makes directed work available |
| `AttemptStarted` | Task/round, optional accepted offer, optional exact closure-stream position, participant | Eligible independent starter or offered recipient; creates an attempt only at a causally open position |
| `AttemptReported` | Attempt, progress/completed/failed/abandoned/uncertain status | Attempt author; reports facts without proving process liveness |
| `WorkDeclined` | Exact offer | Recipient; records refusal |
| `CancelRequested` / `CancelAcknowledged` | Attempt and exact cancellation request | Rule-authorized requester / attempt author; request does not prove cessation |
| `ContributionPublished` | Optional task/round and attempt; typed artifact/base/patch references and provenance | Authorized publisher; taskless publication needs no invented assignment |
| `CompletionDeclared` | Task/round and exact candidate where applicable | Completion selector; contributes positive evidence |
| `ReviewRecorded` / `CheckAttested` | Exact candidate, round, verdict/check name | Eligible reviewer/check selector; immutable evidence |
| `ScopeDecided` | Scope/round, decision predecessor, action, exact evidence roots | Named selection/closure authority; selects output or closes/reopens under its rule |
| `DocumentRevised` | Document name, optional exact base revision, new content reference | Authorized publisher; revisions coexist unless a document selection scope chooses one |
| `EffectMaterialized` | Logical effect ID, rule transition, trigger, explicit action and witnesses | Configured materializer; daemon-created child, review request or offer |
| `DeliveryAcknowledged` | Exact logical effect and recipient | Recipient; explicit agent acknowledgment, distinct from the daemon transport receipt and execution start |
| `LeaveRequested` | Requester's admission tenure | Member; routes to administrator, grants no self-authored membership mutation |

`ScopeDecided` selection and closure use separate stream purposes when their
configured authorities differ. `ScopeKey = (goal, scope, round, purpose)` fixes
which chain a predecessor belongs to. A stream's first predecessor is absent;
subsequent decisions name its exact current predecessor. Two signed successors
of one predecessor halt that stream at the predecessor, even if their author-log
sequence numbers differ. A discovered fork is never repaired by hash ordering,
timeout, or a second signer silently taking over.

An attempt records the exact closure-stream position observed for its round.
The daemon fills this typed reference when authoring a start. An observed `Close`
prevents that start; an observed `Reopen` permits it under the other pinned rules.
Replay rejects a closure reference from another scope or purpose, and rejects
omission or regression relative to closure decisions already present in the
attempt's authenticated author ancestry and typed dependency closure. Missing
referenced evidence stays pending. A concurrent offline start with no authenticated
observation of the close remains valid when that close arrives later; wall clocks
and arrival order do not retroactively cancel it. `Header.parents` remains causal
hints, not an authority source. These constraints are enforced by
[closure evaluation](../crates/locust-core/src/goal/closure.rs), with received-event
[regressions](../crates/locust-core/src/goal/tests.rs) and public-Engine
[authoring/restart tests](../crates/locust-core/tests/organizations.rs).

A new administrator-authorized round can deliberately continue work after a
scope halt, naming the halted round and the new bindings. It does not select a
winner in the old fork or change its historical status. Administrator-chain
forks cannot be repaired under that same disputed authority: keep the verified
prefix and expose the halt. Work authorized by an unaffected verified prefix
continues; new governance depending on the fork waits.

### Tenures, cutoffs, fork pins and retained proofs

Keep the author usable-prefix calculation in `history.rs`: a contiguous,
uniquely signed `seq`/`prev` chain up to the first fork or missing predecessor.
An ordinary event past a fork supplies no authority. Evidence depending on it
returns to pending fork proof, not satisfied. The signed bytes remain retained
as attribution and conflict evidence. No ordinary work event may pin its own
fork to make itself valid.

A removal names the admission ID it closes and either no accepted events or an
exact `(seq, event_id)` cutoff. Accepted history for that tenure is the cutoff's
verified same-author ancestry intersected with that tenure, not every event with
a lower sequence number. Missing cutoff ancestry is pending; a known wrong
signer, sequence, goal or tenure makes the retention cutoff invalid and preserves
no old-tenure events through that cutoff. Closing a valid current tenure is
independent of the retention proof: removal and key rotation apply immediately,
and an invalid/missing retention proof never reopens permission. Expose the
cutoff error separately from the member's removed status.
Re-admission opens a new tenure, so old-anchor events cannot use it. No timestamp
can backdate a later event into the old tenure.

An effective scoped decision pins the exact same-author ancestry and typed
semantic dependencies of its selected subject and evidence roots. Build that
closure with the current `commitments.rs` traversal pattern, including signature,
predecessor, governance, definition, task/round and eligibility checks. A pin
bypasses only the ordinary author-fork exclusion. It cannot bypass nonmembership,
a removal cutoff, wrong epoch, wrong rule, invalid subject or an authority-stream
fork. Validate the candidate decision against its tentative pins before exposing
it as effective; missing dependencies leave only that scope pending.

Pin maps belong to a decision's scope/proof context, not one goal-wide winner
map. Each map must assign at most one event to `(author, seq)`. Two incompatible
roots in one proof cannot combine. Two unrelated scopes can retain different
exact branches as their own evidence without rewriting global author history.
A downstream decision combining those incompatible branches is pending/disputed
and cannot act until it uses compatible evidence or a new authorized round.
An administrator cutoff is a global tenure restriction; a conflicting scoped pin
cannot override it. Scoped pins are never a way to choose a branch of the
administrator chain or to bypass the scope's own double-successor halt.

A decision is authorized against its signed governance anchor, while admissible
member evidence remains constrained by known tenure cutoffs. Consequently later
cutoffs or newly discovered authority forks may invalidate a previously observed
outcome. Preserve its record and expose the changed status; do not describe it
as irreversible finality. Member forks alone do not destroy an otherwise valid
scoped decision's exact retained proof. Unpinned non-exclusive completion may
return to pending when its evidence forks. There is no administrator or selection
signature required for the initial non-exclusive completion verdict.

An author-log fork and a scoped-chain fork are different facts. The latter halts
that scope only. A fork in an author's global log affects any later evidence from
that author under D11, including multiple scopes depending on it, but cannot
halt unrelated authors or unrelated authority streams. Keep both explanations
visible rather than claiming every key compromise affects only one task.

### Threshold evaluation and deterministic projection

For each candidate and pinned round, resolve the eligible principal set from the
round's authenticated bindings and selector. Validate each review's exact subject,
round, membership/tenure, anchor, signature and branch evidence. Count an eligible
principal at most once; apply author exclusion before counting. A negative review
is recorded evidence, not an implicit veto or retraction of a positive one. Rules
requiring veto or mutable votes remain unsupported. Two candidates may both
qualify. Reports/checks satisfy only the explicitly named predicate; their labels
do not prove independent verification or real execution.

Use these passes in the pure goal engine:

1. Index exact held events, author logs and referenced objects.
2. Build the verified administrator chain, tenures, cutoff restrictions, binding
   snapshots and content epochs. Invalid actions grant nothing; missing content
   or proof produces explicit pending dependencies.
3. Resolve task rounds and unambiguous scoped-stream prefixes. Construct exact
   proof closures and tentative local pin maps; validate decisions with those
   maps without recursively choosing new pins from their own result.
4. Evaluate append-only work and candidate predicates from effective events and
   their context. Dependency cycles or incompatible proof closures cannot count.
5. Produce projection deltas and desired flow effects as data. The pure fold does
   not access keys, send messages, launch processes or mutate durable storage.

`Evaluation` returns projection, event standings, scope halts, missing dependencies,
retained proof roots and desired effects. Preserve a stable ordering for output
and tests; no winner derives from that ordering. The runtime passes decoded,
semantic-hash-verified definitions to the evaluator through a read-only object
lookup. Missing definition bytes yield pending-definition status, never guessed
defaults. This replaces the existing assumption that headers alone contain every
rule needed to authorize work.

### Effect identity, materializer and durable delivery

A goal/round binding identifies one materializer principal for each configured
transition plus its target recipients. This is an explicit role of that scope,
not a default universal coordinator. Its daemon must hold the signing key and
an appropriate standing local grant. Other replicas derive the same desired
effect and can show that the authorized materializer is unavailable. They cannot
impersonate it. Availability blocks only this transition's materialization.

Compute `EffectId = H(domain, goal, source_scope, source_round, transition_id,
trigger_identity, action_kind, target_slot)`. Use canonical encoding. A review
trigger names the exact contribution; a once-per-round completion trigger names
the completed scope/round; a dependency join names the downstream stage instance.
Equivalent threshold witnesses do not change the logical effect ID. A child task
created by an effect uses the effect ID as logical task identity; signed event ID
remains its evidence. Explicitly requested tasks use their creation event ID.
Action fields and recipients are deterministically resolved from the pinned rule
and trigger, so two competing payloads cannot claim the same valid identity.
Proof witness lists may differ while proving the same action.

`EffectMaterialized` carries the logical ID, resolved action and exact evidence
roots. Verify authorization and readiness through the same evaluator used for
incoming events. It is not an extra pinning authority: an effect resting on
unpinned forked evidence becomes pending/disputed with that evidence. Stop any
undelivered execution offers affected by that change and surface the status to
recipients; never silently undo a filesystem change or claim a running process
has stopped. A later new round creates new effect identities explicitly.

Use the existing single-writer `Node::land`/`Store::commit` seam:

1. After event ingestion, local action and startup replay, enqueue desired effects
   automatically. No agent poll or explicit next-step request drives this loop.
2. For each authorized pending effect, check its durable logical ID record,
   revalidate current evidence/permissions, allocate the author's next position,
   and sign once. Commit event, proof retention roots, logical effect mapping,
   feed/revision updates and recipient outbox entries in one transaction.
3. Send only committed entries. The recipient durably records its inbox entry
   before acknowledging receipt. Duplicate deliveries reuse the same logical ID.
   Delivery attempts can repeat; one logical action and inbox item are exposed.
4. Acknowledgments commit durably before marking outbox work delivered. Lost
   acknowledgments cause retries, not new effects. After restart, rebuild desired
   effects and resume existing outbox entries. A failed/uncertain store commit
   stops further signing until store reopen/replay establishes its outcome.
5. A transport receipt retires the sender's outbox retry. Explicit signed agent
   acknowledgment remains a separate inbox observation. Receipt, accept/decline,
   local session claim and observed process start are distinct. The daemon may start/resume a supported adapter only with an existing
   applicable local grant; otherwise work stays durably ready with the missing
   permission shown. An offline or closed client retains deliverable work.

A duplicate valid materialization with the same logical ID maps to one action;
retain all signed evidence. A payload conflict is invalid or disputed, never a
second child. Do not use a process-local set or an in-memory feed cursor as the
only duplicate protection. Do not call the network exactly-once: delivery is
retryable and acknowledgment-driven, with durable logical deduplication.

The current [recipient record implementation](../crates/locust-core/src/node/delivery.rs)
uses `Space::Pending`, keyed by goal, logical effect and recipient. Records retain
current endpoint, durable receipt state and availability. Event projection and
these records share [one commit](../crates/locust-core/src/node/commit.rs).
[Peer reconciliation](../crates/locust-core/src/node/replica.rs) sends only
committed currently available entries, checks the authenticated recipient endpoint,
and commits positive receipts before retiring retries. Missing proof or content
returns a negative receipt for later anti-entropy. All replicas may relay an
already verified signed effect; only the named materializer signs it. A recipient
local to the same daemon receives its inbox record in the materialization commit.

[Encoded driver tests](../crates/locust-core/src/node/tests/delivery.rs) exercise
lost receipts, duplicate attempts, both-daemon restart, wrong endpoints and fork
retraction. [Durability fault tests](../crates/locust-core/src/node/tests/failure.rs)
cover failure before and after effect/outbox, recipient event/inbox and sender
receipt commits. This is deterministic component evidence, not physical-machine
network qualification.

### Evaluator interface for the runtime cutover

The node supplies decoded definitions through an immutable lookup; the evaluator
checks normalized semantic identity rather than trusting a catalog key. Missing
objects remain pending and can be supplied later without inserting a fake event.
The initial public seam is:

```rust
pub trait DefinitionLookup {
    fn definition(&self, hash: &DefinitionHash) -> Option<&Blueprint>;
}

pub struct Evaluation {
    pub state: State,
    pub standings: BTreeMap<EventId, Standing>,
    pub admin_halt: Option<Halt>,
    pub scope_halts: BTreeMap<ScopeKey, Halt>,
    pub missing: BTreeSet<Dependency>,
    pub retained: BTreeSet<EventId>,
    pub desired_effects: BTreeMap<EffectId, DesiredEffect>,
}

impl Goal {
    pub fn new(id: GoalId) -> Self;
    pub fn load<S: Store, D: DefinitionLookup + ?Sized>(
        store: &S, id: GoalId, definitions: &D,
    ) -> Result<Self, StoreError>;
    pub fn apply<D: DefinitionLookup + ?Sized>(
        &mut self, events: &[Event], definitions: &D,
    ) -> Changes;
    pub fn refresh<D: DefinitionLookup + ?Sized>(
        &mut self, definitions: &D,
    ) -> Changes;
    pub fn evaluation(&self) -> &Evaluation;
    pub fn state(&self) -> &State;
}
```

`DefinitionHash` and `EffectId` are separate 32-byte identifiers. `Dependency`
distinguishes missing event, definition and content proof. `DesiredEffect`
contains logical ID, scope/round, transition/trigger, resolved action, materializer,
recipients and exact witness roots. It contains no signing key or local grant.
`Changes` must report all changed standings, including formerly effective facts
that become pending/disputed, so a durable feed and delivery outbox cannot miss
retractions. Keep `event`, `holds`, `frontier`, `screen` and `next` as current-model
primitives; `next` returns author sequence/predecessor and the current verified
administrator anchor. Replace the old global `halt()` assumption with explicit
administrator and scope halts.

`refresh` reruns the same evaluator after definitions/proof objects arrive; it
does not sign. Node startup loads definitions, replays events and automatically
reconciles desired effects with durable effect/outbox records. A failed lookup
never selects `Blueprint::default()` for an already pinned scope.

### Reusable code and immediate implementation split

Retain canonical codec/signature/sealed-object validation, author-log indexing
and frontier reconciliation, the store transaction abstraction, durable object
installation, idempotency records and fail-stop-on-uncertain-commit behavior.
These already express useful invariants. Rework their typed dependencies and
references for the new contract instead of wrapping an old runtime.

Replace the global coordinator `Trail`/`Chain` assumptions, one accepted-head
projection, enum-order authority classification and assignment-only API/session
identities. Generalize exact dependency traversal into scope-local proof contexts;
do not retain `Commitments` as a second coordinator path. Add effect/inbox/outbox
records through current store namespaces/layout and remove superseded layouts.

Suggested file ownership after contract agreement: protocol lane owns event
structs/test vectors plus goal history/governance/proof/fold; integration lane
owns node authoring/requests/commit/recovery and local records; agent lane owns
API mappings/CLI/MCP/workspace callers; site lane owns released-contract docs.
Agree `Evaluation`, `Context`, `ScopeKey`, `EffectId` and current event variants
before editing consumers. No lane needs an old decoder or fallback runtime.

## Source-based replacement inventory

The runtime cutover at `c88e3bc` replaced the pre-authoring implementation. The
final source audit, including `4bc417d`, `c731e71`, `6c93b3f` and `19ced9e`, found no remaining item in
this superseded-executable inventory. Existing filenames are sometimes retained
for a reusable invariant; they contain the current implementation, not a second
runtime. Current recovery and unsupported-format refusal are tested separately
from successful decoding of old state, which is not supported.

| Package | Completed replacement | Enforcing source / evidence |
| --- | --- | --- |
| O3/O5 | Removed coordinator assignment/acceptance event variants and global decision classification | [Current signed events](../crates/locust-proto/src/event.rs), [wire tests](../crates/locust-proto/src/vectors.rs), [exported runtime contract](reference/generated/runtime.contract.json) |
| O3/O5 | Separated administrator membership/rules history from scoped work and decisions | [Governance chain](../crates/locust-core/src/goal/chain.rs), [single fold](../crates/locust-core/src/goal/fold.rs), [goal tests](../crates/locust-core/src/goal/tests.rs) |
| O3/O5 | Replaced coordinator commitment roots with exact scope-local evidence and authenticated closure | [Proof closure](../crates/locust-core/src/goal/commitments.rs), [decision rules](../crates/locust-core/src/goal/rules.rs), [fork/closure tests](../crates/locust-core/src/goal/tests.rs) |
| O4/O5 | Replaced universal assignment/result/accepted-head projection with independent attempts, contributions, completion and scoped selection | [State](../crates/locust-core/src/goal/state.rs), [projection](../crates/locust-core/src/goal/projection.rs), [API tests](../crates/locust-core/tests/organizations.rs) |
| O3/O4 | Replaced coordinator-only work authorization and assignment-only claims | [Access](../crates/locust-core/src/node/access.rs), [attempt claims](../crates/locust-core/src/node/requests/claims.rs), [session fencing tests](../crates/locust-core/tests/organizations.rs) |
| O3/O4 | Replaced content roots with current definition/contribution/evidence admission | [Content graph](../crates/locust-core/src/node/content_graph.rs), [content requests](../crates/locust-core/src/node/requests/content.rs), [content tests](../crates/locust-core/src/node/tests/content.rs) |
| O2/O8 | Removed the migration runner; initialize the sole current schema directly and refuse unsupported markers | [Schema](../crates/locust-store/src/schema.rs), [open sequencing](../crates/locust-store/src/connection.rs), [store tests](../crates/locust-store/src/tests.rs) |
| O2/O8 | Replaced local bindings and records directly; no old-record fallback decoder | [Record codec](../crates/locust-core/src/node/records.rs), [sessions](../crates/locust-core/src/node/sessions.rs), [durable delivery](../crates/locust-core/src/node/delivery.rs), [failure/recovery tests](../crates/locust-core/src/node/tests/failure.rs) |
| O7/O8 | Removed obsolete assignment-only API/CLI/MCP operations and aliases | [API registry](../crates/locust-proto/src/api.rs), [CLI arguments](../crates/locust/src/cli/args.rs), [MCP schema](../crates/locust/src/mcp/schema.rs), [generated current contract](reference/generated/runtime.contract.json) |
| O8 | Replaced assignment-only submission and mandatory distributed selection before local application | [Workspace CLI](../crates/locust/src/cli/workspace.rs), [application](../crates/locust-workspace/src/apply.rs), [Open tutorial](guide/apply.md). The internal `accepted_head` application input names an exact artifact, not a universal goal head |
| O8 | Removed setup-v1 ownership and pending-journal readers; package one current strict three-payload format | [Setup parser](../crates/locust/src/installation/setup.rs), [non-mutation refusal tests](../crates/locust/src/installation/setup/tests.rs), [package contract](packaging.md) |
| O9/O11 | Rewrote active guides, examples, formal models and qualification fixtures; removed unused proposal limit | [Current manual manifest](site.json), [models](../research/tla/organization.md), [client workflows](../scripts/client_qualification/production.py), [current limits](../crates/locust-proto/src/limits.rs) |

The audit checked current source names and callers, Rust module declarations,
CLI/MCP/generated schemas, dependency manifests and active guide claims.
Refusal tests deliberately contain unsupported markers; historical research and
[protocol-1 documentation](protocol-v1.md) retain their historical names. Neither
is an executable compatibility path. The final combined cleanup checks passed
formatting, strict workspace Clippy, 624 Rust tests (12 explicit ignores), 195
Python tests and documentation checks. Native campaigns exposed and removed two
stale Python expectations: canceled task state and integration after export.
The corrected four-client lifecycle and six operational cases pass against the
unchanged candidate. This inventory closes V21's source-removal
boundary; it does not qualify physical networking, providers or public releases.
