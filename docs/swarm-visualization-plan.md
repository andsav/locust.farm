# Live farm pages and the farms gallery

**Status: first development implementation, 2026-10-04.** The publisher, signed
consent, service and public routes are implemented; see the [operator and user
guide](guide/farm-publication.md). Production deployment and the two-physical-machine,
real-harness rehearsal remain unverified. The [farm](../research/evidence/swarm-visualization/farm.html),
[gallery](../research/evidence/swarm-visualization/gallery.html) and
[controls](../research/evidence/swarm-visualization/controls.html) are mockups with
invented data. They establish a visual direction; this document supersedes their
claims, identity model and single-task-per-agent assumptions.

## Goal and demo scope

The owner wants a live view on locust.farm, enabled by a goal's creator. Member
daemons share the necessary records with the creator's daemon, which publishes
a restricted view. Anyone with the link can view it; listing in `/farms` is a
separate choice. The page has no owner controls.

The revised demo target is **several real agents, using different harnesses,
across two physical machines owned by the same person, solving one problem**.
It must show actual collaboration and a working result. Two machines do not mean
two people, and two sessions of one harness do not demonstrate two harnesses.
A small Slack-style app is the proposed problem, detailed below; its exact scope
and harness roster remain rehearsal choices, not claims of completed work.

The first version therefore includes remote public names, harness labels and
minimal publication consent. It no longer assumes that the creator owns only
agents on its own daemon, or that unnamed remote agents can be published without
consent. Rich presence and model reporting remain outside the demo requirement.

## What is already implemented

These are source-backed foundations. Implementation and verification boundaries
are documented in the [farm guide](guide/farm-publication.md):

| Available behavior | Source and boundary |
|---|---|
| Goal creation records its administrator | [Goal requests](../crates/locust-core/src/node/requests/goals.rs); the creator's daemon is the proposed publisher |
| Members, rules, attempts, contributions and decisions replicate | [Peer handling](../crates/locust-core/src/node/peers.rs) and [sync protocol](../crates/locust-proto/src/sync.rs); the creator has a local, potentially stale view |
| Current task rounds and evaluated completion | [State](../crates/locust-core/src/goal/state.rs) and [projection](../crates/locust-core/src/goal/projection.rs); completion, selection and closure are distinct |
| Configured stages and prerequisite evidence | [Formation types](../crates/locust-proto/src/organization.rs) and [flow evaluator](../crates/locust-core/src/goal/flow.rs); the administrator materializes stages, subject to its local grants |
| Revision waits and a durable event feed | [API](../crates/locust-proto/src/api.rs) and [feed](../crates/locust-core/src/node/feed.rs); feed positions include standing changes and retractions, not just new events |
| Local client/session information | [Session API](../crates/locust-proto/src/api.rs) and [managed clients](managed-clients.md); names, claims and client records are not remote presence |
| Snapshots, signed contributions and explicit application | [Coding workflow](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/t2-workflow.md) and [application guide](guide/apply.md); each worker can use a separate workspace |

The [local demo](demo.md) and its [qualification](../research/demo-qualification.md)
provide setup and scripted-client evidence. They do not establish a complete
real-model collaboration across two physical machines. Its `peer-review`
[preset](../crates/locust-proto/src/organization/presets.rs) has no configured
stages or finish authority and is not the formation for this demo.

The [schema](../crates/locust-proto/src/farm.rs), [publisher](../crates/locust-core/src/node/farm.rs),
[service](../crates/locust-farm/src/lib.rs), public routes and signed public profiles/consent
now implement the first slice. The complete two-machine rehearsal remains open.
A daemon does not know
remote tool use, prompts, tokens, costs or process liveness from work events.

## Changes to earlier proposals

This replaces the static export/no-hosted-publisher/no-motion parts of the
historical [last-mile plan](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/last-mile-implementation-plan.md) and its
[research](../research/last-mile-experience.md). The owner chose locust.farm for
the farm page, gallery and service, rather than a separate origin. Motion follows
observed changes only.

The earlier version of this plan split anonymous remote publication into stage 1
and consent into stage 2. That split is superseded: **the demo includes consent
and remote identity from the start**. There is one publication policy and runtime,
not an interim local policy to replace later. Protocol/API changes use the
repository's current-format rules; no migration or compatibility runtime is added.

Consent is explicit, never inferred from admission, shared ownership or a common
endpoint. Published copies cannot be recalled. Farm data is untrusted content,
rendered as text; CSP prevents browser execution, not semantic prompt injection.
Agent-facing documentation must not promote farm text into trusted instructions.

## Public data and projection contract

Add a versioned `FarmSnapshot` in `locust-proto`, a JSON Schema generated with
`schemars`, and generated site types. Reject unknown fields at every nested
boundary and validate lengths, references and combinations at runtime. Schema
annotations alone are not validation. The only producer is a dedicated
`locust-core` projection; private read models are not serialized into it.

The contract contains farm-scoped identifiers, approved display labels, formation
stages/dependencies, task rounds and status, attempt associations, per-candidate
completion evidence summaries, counts, recent changes, observation times,
publication eligibility, and goal closure state. Public snapshots contain no
raw protocol ids, endpoint ids, signing secrets or workspace content. The upload
public key is transport authentication metadata, not an agent identity.

### Map source facts to what the page says

| Public view | Rule |
|---|---|
| Agent | A participating principal with a consenting public profile, not a process or a human. Multiple sessions do not silently become additional agents |
| Locust group | Members admitted at the same endpoint, assigned a farm-scoped number. It identifies a daemon group, not proof of a physical machine or owner |
| Owner/machine label | Explicitly approved display metadata. The demo may label its two daemon groups as Machine A/B; this is owner-reported, not protocol-proven |
| Harness | A canonical harness label reported by the member's daemon from its bound session/adapter. Unknown or multiple bindings remain explicit; never parse arbitrary client text into a public label |
| Stage | The task round's actual `TaskBinding::stage` and pinned rules. Unstaged tasks get an unstaged group, never the first stage as a fallback |
| Stage edges | Actual prerequisite evidence; a topological layout with stable tie-breaking, including parallel branches. A started child is not the definition of every prerequisite's truth |
| Agent placement | Associations with all applicable attempts/tasks. One agent may appear in several stages with the same number; many agents may attempt one task |
| Attempt | Last reported state: started, progress, completed, failed, abandoned or uncertain. Started/progress does not mean the process is running or that a task is exclusively held |
| Task completion | Evaluated completion of the current round. Attempt completion is not task acceptance, and acceptance is not selection or local application |
| Reviews/checks | Evidence for an exact candidate and pinned rule. Distinct eligible reviewers only; never pool approvals across candidates. Negative reviews are recorded evidence, not an implicit veto |
| Goal ended | An effective close decision for the current goal scope. Empty boards and all-tasks-complete do not imply goal closure |

Use separate completion, closure, selection and attempt fields rather than a
single ambiguous state. For a mutually exclusive task-count summary, prioritize
closed, then completed, then awaiting required evidence when a candidate exists,
then reported attempts, then open. Expose the underlying facts in the table;
failed/abandoned/uncertain attempts remain visible. A closed unfinished task is
not counted as completed. Rules without a simple review threshold show the
appropriate requirement, not an invented `approvals / required` fraction.

Revisions, removal cutoffs, missing proofs and authority conflicts can change an
earlier outcome. Apply the existing evaluator's standing, show unavailable or
disputed state explicitly, and retain a change explaining any retraction. Old
rounds do not move a current-round task or donate approvals to it. Unattached
contributions can appear in the changes feed without inventing a task.

### Identifiers, text and time

- Persist independent per-farm mappings for agents, daemon groups, stages, tasks
  and candidates. Allocate numbers on first accepted observation, using admission
  order for the initial membership where available. Never renumber/reuse them on
  removal, late arrival or restart. A new farm gets fresh mappings.
- Task display refs start at four characters but are allocated collision-free:
  derive candidates from a per-farm secret salt, resolve collisions during durable
  allocation, and lengthen if needed. They are display labels, not authorization.
  Do not publish raw ids or a stable cross-farm identity.
- Public names, formation/stage/role labels and optional goal title are explicit
  publication choices. No automatic export of descriptions, rule JSON, participant
  selectors, input labels or local agent names. The owner previews the exact
  snapshot. Reusing a chosen name across farms is permitted but is not anonymity.
- No task bodies/titles, results, code, patches, prompts, tool activity, models,
  tokens, costs, keys, endpoints, addresses, hostnames, paths, tickets, invitations,
  client versions or explanations guessed from a waiting agent in this version.
- Generate changes from typed templates and approved labels only. Never forward
  raw event text or private errors. An explicit recent-change window (default 50,
  configurable) states when older entries are omitted; it does not cap agents,
  tasks, traversal or execution.
- Record creator observation times durably when changes are observed, including
  standing changes. Existing event timestamps are author-supplied, not arrival
  times. History predating observation records has unknown observation time; do
  not stamp imported history as newly performed work.
- Show service receipt time, creator observation time, and per-goal peer-sync age
  separately. The existing peer timestamp is keyed by endpoint; add goal-scoped
  sync records before presenting it as freshness for this goal. Unknown stays
  unknown. A heartbeat can be fresh while the creator's peer data is old.

## Publication authority and consent

Introduce minimal signed publication records in the shared protocol:

- Creator governance record `PublicationSet`: off/link/listed, farm id, upload
  public key and a versioned disclosure policy covering labels, grouping, work
  state and optional goal title. Upload private keys and ref salts stay local.
- Member record `PublicationConsent`: exact policy digest, accept/decline and a
  public profile approved by its local owner. The profile includes a display name,
  optional owner/group label and daemon-reported canonical harness label. These
  are signed reports, not independently verified human identities.
- Owner-only local commands authorize publication and consent. An agent's general
  work grants or possession of an MCP session must not enable these writes.
  Implement and test the authorization checks, not just omission from MCP tools.
- On both demo machines, the same person approves the local agents explicitly.
  No separate human-identity/account system is required to demonstrate this.
- A changed disclosure policy requires matching consent. A profile update under
  the same policy requires local owner approval and a new signed record. Remote
  consent cannot be fabricated by the creator's settings.
- Invites surface the current policy and reconcile it against signed history on
  join; accepting an invite does not itself consent to publishing.

For the first version, require matching consent from every active member and
from authors whose retained work contributes to the proposed public snapshot.
Do not try to anonymize an unconsenting participant by publishing its counts,
roles or task activity. This deliberately avoids a partial-redaction runtime.
Joining a new member, declining/revoking consent, missing policy proof or an
invalidating authority conflict makes the farm ineligible until resolved.
Re-evaluate on every relevant event and immediately before each upload/check-in,
not just at `farm on`.

On loss of eligibility, stop publishing snapshots/check-ins and durably enqueue a
signed suspension. The service hides the page and removes its gallery entry when
suspension arrives. Keep the local reason visible to the owner. Suspension may be
reversed under the same farm id only after eligibility is restored. If the service
cannot be reached, previously published data may remain accessible until delivery;
report pending suspension honestly. Remote revocation cannot be acted on before
the creator receives it. Neither mechanism recalls external copies.

Protocol work includes fold/authority rules, deterministic consent resolution,
replay and fork tests, updated schemas/vectors and an explicit assessment of the
existing [formal models](tla-verification-plan.md). No general model-reporting or
presence protocol is needed for this slice.

## Page and gallery

`/farm/<id>` contains an optional approved goal title, public formation label,
agent and daemon-group counts, freshness box, stage map, task table, changes,
and an explanation of publication and evidence boundaries. Do not infer a people
count from daemons or principals. The demo can display “one owner, two machines”
as approved descriptive text.

Port the mockups' visual language, but replace the layout's single `holder`
lookup, unknown-stage fallback and fabricated status data with the contract
above. The task table is the full keyboard/screen-reader representation. Multiple
attempts and parallel stages must remain readable at 1440 and 390 pixels.

Marks change only for semantic snapshot changes. An association moves or appears
once per observed change; heartbeats do not animate/reset it. Respect
`prefers-reduced-motion`. Time labels and freshness thresholds can update on a
clock without suggesting agent activity.

Page states:

- **Receiving updates:** authenticated service receipt within two minutes. This
  describes the publisher connection; peer-sync age is a separate indicator.
- **Quiet:** no receipt for over two minutes. Keep the last snapshot and its age;
  do not infer why it stopped.
- **Ended:** current goal scope is explicitly closed. Continue observing locally;
  a valid reopen or replacement current scope resumes publishing. Ending is not
  deletion and does not imply every process stopped.
- **Unavailable:** unknown id, suspended publication, deletion or takedown. Do not
  disclose the private reason. Clear the old snapshot when an open browser receives
  invalidation or an unavailable response; a disconnected browser cannot learn of
  revocation until it reconnects and must show its stale connection state.

Unavailable takes precedence over ended; ended takes precedence over receipt age.

`/farms` lists only eligible listed farms, with all/receiving/quiet/ended filters.
Use paginated results and stable latest-semantic-change order per gallery visit;
heartbeats do not reorder cards. Cards share the page's layout implementation.
Link-only farms never appear in gallery responses or discovery metadata.
Revalidate visible cards and remove unlisted/unavailable farms without reshuffling
the remaining cards. Public farm state is not stored in an offline browser cache.

## Publisher and service contract

### Local state and commands

Store the upload key, salt, mappings, policy/consent projection, observation records,
next sequence, pending request, accepted receipt and desired publication state
in durable local storage. Only the creator's daemon publishes. Extract a coherent
snapshot through the engine's single-writer boundary; do network I/O outside that
thread. Farm settings, consent, profile, closure and freshness changes must wake
the publisher as well as task changes.

Proposed CLI (not implemented commands):

- `locust farm show --goal <goal>` previews the exact public JSON and eligibility.
- `locust farm on --goal <goal> [--listed] [--title]` establishes the policy and
  prints the link plus pending/active status. It does not imply remote consent.
- `locust farm consent --goal <goal> --agent <local-agent> --accept|--decline`
  reviews the policy/profile and signs locally; public-name/group options are
  included in the reviewed request. Apply separately to each participating agent.
- `locust farm off --goal <goal>` records off and durable deletion intent; report
  pending deletion until the service acknowledges it.
- `locust farm status` reports publication, consent, last receipt and pending
  work, with no upload secrets.

Send whole snapshots, debounced at most once per two seconds, plus check-ins every
30 seconds while eligible and open. Retry transient failure with backoff up to
60 seconds without blocking agent work. These are delivery cadences, not agent
execution limits. Measure encoded sizes; the old 20 KB estimate is not evidence.

### Signed requests, sequencing and recovery

Define a versioned, domain-separated signature envelope using the existing
[signing primitives](../crates/locust-proto/src/crypto.rs). Sign protocol version,
operation, farm id, sequence and digest of the exact transmitted body bytes.
Bind listing/visibility and all other mutable metadata into that body. Specify
the canonical envelope encoding and publish cross-language test vectors.

Derive farm ids from a sufficiently long upload-public-key digest, specifying the
encoding and length in the schema; use at least 128 bits of digest. Check the key
binding on every mutation. This prevents unauthorized updates to an existing
farm; it does not prove an uploader is honest about a swarm or prevent spam.

All mutations, including check-in, suspend and delete, share one durable increasing
sequence. Persist intent and bytes before sending, serialize requests per farm,
and retry identical bytes after ambiguous failures. The service atomically records
the sequence, digest and result with the mutation. Same sequence/same digest
returns the prior receipt; same sequence/different digest or older sequence is
rejected. Duplicate retries do not advance freshness or emit duplicate changes.
Recover by reconciling authenticated receipts, never resetting a sequence to zero.

An off/suspend transition fences queued older work and becomes the next durable
control operation. An in-flight upload may land before that operation; the UI must
not claim deletion/suspension until acknowledgment. A delete/takedown/retention
expiry leaves a minimal tombstone that rejects all future uploads for that id.
Erase its public snapshot, close or invalidate open streams, and remove it from
gallery caches. Turning publication on after deletion uses a fresh key/id. A
suspension hides and clears public data but preserves ordering for a later eligible
resume. Persist and retry control operations across daemon/service restarts.

### Service and endpoints

Add `crates/locust-farm`: an axum service with SQLite for farm state, latest
snapshot, sequencing receipts, visibility, service timestamps and tombstones.
Keep service records separate from the distributed goal store. New dependencies
and lockfile updates have one integration owner.

- `PUT /api/farms/<id>` accepts a signed, validated snapshot and visibility.
- `POST /api/farms/<id>/check-in` accepts a signed eligible-publisher check-in.
- `POST /api/farms/<id>/suspend` hides publication while retaining sequence state.
- `DELETE /api/farms/<id>` permanently deletes that farm id.
- `GET /api/farms/<id>` returns the latest available snapshot, service time,
  receipt time and stream version, or a generic unavailable response.
- `GET /api/farms/<id>/events` sends full snapshot/check-in/status envelopes with
  monotonically ordered event ids. On initial connection and reconnect, atomically
  subscribe and send current state so a GET/subscribe race cannot lose an update.
  `Last-Event-ID` is a resynchronization hint, not a promise of retained event
  history. Clients ignore older versions and replace state from the newest full
  snapshot, including after service restart.
- `GET /api/farms` returns paginated listed entries only.

Enforce runtime schema checks, configurable/advertised request-size limits,
request rates and connection admission. Oversized snapshots must produce an
explicit local publication error; never silently omit agents or tasks. Initial
operator defaults can retain the proposed 256 KB body and one upload/second per
farm, subject to measured fixtures. Per-key limits alone do not prevent key-spam;
provide an operator-controlled enrollment mode for the demo deployment and define
public enrollment policy before unrestricted hosting.

Provide `locust-farm take-down <id>` and an abuse contact. Proposed retention is
30 days after acknowledged closure; reopen before expiry clears that timer.
The creator continues watching after ending; reopen after expiry needs a new
farm. Unreachable quiet farms have no fabricated end time. Service health, disk
failure and database recovery must be observable to the operator.

## Two-machine working demo

### Proposed roster and problem

Use four distinct principals/sessions across two daemon homes on two physical
machines. Four is a rehearsal roster, not a product limit. Proposed allocation:

| Machine | Harness | Role |
|---|---|---|
| A, publisher | Codex | Coordinator/integrator; owns the shared contract and assembles selected contributions |
| A | Claude Code | Frontend worker |
| B | Codex | Backend worker |
| B | Pi, or another qualified installed harness | Independent reviewer/tester |

The owner may change this allocation. The acceptance minimum is several agents,
at least two different real harnesses, and actual work on both physical machines.
A third harness adds useful coverage if it passes preflight; it is not a reason to
substitute simulated participation. Record exactly which roster ran. Shared
ownership is sufficient; independent humans/accounts are not a demo requirement.

Proposed problem: **build a small Slack-style team chat app** with channels,
a display-name selector, message history, and messages delivered live between two
browser sessions. Persist messages across an app restart. Use synthetic demo
content. Authentication, Slack integration, payments, uploads, search, threads,
production hosting and production security are outside this exercise. The output
is a runnable local application, not a claim of a production Slack replacement.

Keep the problem statement fixed during rehearsal. Agree on the API/event schema,
data model, repository layout, test commands and startup command in the first
contribution so frontend/backend work can proceed independently.

### Dedicated formation and collaboration path

The [team-chat definition](../examples/demos/team-chat.json) implements the
current-format demo rules with explicit roles, task types, non-author review,
selection of exact prerequisite candidates and a single finish authority bound
to the coordinator. It passes offline formation validation; live bindings and
execution still require qualification. The owner invokes the finish authority
for the final close after examining the result. Its staged graph is:

```text
contract -> frontend --\
          -> backend  ---> integration -> verification
```

Encode both branch prerequisites explicitly. Select exact branch contributions
before integration; merely seeing an arbitrary review/publication is insufficient.
Choose each stage's completion rule to match its deliverable and assign a
non-author reviewer where needed. Use the evaluator's configured stage tasks,
not task-title heuristics. Child/unstaged work stays visibly distinct. The
administrator's daemon must remain available to materialize newly ready stages.

Each coding worker gets a separate workspace materialized from the agreed base.
No shared writable checkout across agents. Exchange findings, exact source
references, snapshots and signed patch contributions through Locust. The integrator
applies the selected patches to its integration workspace, resolves any conflicts,
runs checks and publishes the exact combined candidate. The reviewer independently
materializes that candidate, tests it and records evidence against those bytes.
If it fails, report the finding and revise the appropriate work explicitly; do
not silently recycle approval of an older candidate. Local application and final
owner acceptance remain distinct from replicated completion.

Keep the native sessions open and have them inspect pending work/context through
their supported Locust tools. Qualify delivery or explicit resume for each chosen
harness; no assumption of automatic wake, remote execution or universal hooks.
The owner supplies initial objectives and local permissions, but does not carry
findings or patches manually between chats. Record any manual resume or steering.

### Run and show

1. Build identifiable compatible artifacts for both hosts; record commit,
   binary hashes, platform, harness versions, selected models and effective local
   permissions privately. Preflight real-provider access and native Locust tool
   discovery on both machines. These are future run prerequisites; this plan
   revision does not launch clients or spend provider credits.
2. Create a fresh goal and admit the roster before activating the dedicated
   staged rules and execution grants. Bind roles to the actual member keys;
   validate that setup does not materialize work under incomplete bindings.
   Inspect those bindings and confirm real cross-machine event and artifact
   exchange. Approve the publication policy/profiles locally on both hosts and
   inspect the resulting public JSON before enabling the demo page.
3. Open the farm in an unauthenticated browser, alongside the eventual application
   preview. Show the two approved machine groups and harness labels. Start real
   work and observe the contract, parallel branches, cross-machine review,
   integration and verification arriving as actual signed events.
4. Demonstrate the finished app in two browser sessions: select different names,
   send/receive messages, switch channels, reload and restart to verify history.
   Record a remote participant's finding or contribution that materially affected
   the final result. Distinct agent chips alone are not collaboration evidence.
5. Separately rehearse recovery: disconnect B while A can still publish (fresh
   publisher, stale peer), reconnect B and converge; stop A's publisher long
   enough to show quiet, then restart it with the same farm and mappings. Check
   for duplicate tasks, contributions or application. Do not equate stopping the
   daemon with stopping native clients. Recovery need not interrupt the live
   presentation once it has been recorded against the same candidate.
6. After inspecting exact candidate/test evidence, the owner closes the goal via
   its declared authority. Show ended. Exercise reopen/resume and final deletion
   separately, including offline deletion and an already-open viewer.

“Real time” means the farm updates as collaboration records reach the creator and
service. Measure event-to-browser delay across both hops during rehearsal, recording
clock alignment or measuring each hop with one clock; do not promise a latency
bound from the two-second upload cadence. The page does not
stream terminal output, thinking, tool calls or unreported edits. A recording or
scripted fixture is clearly labeled and never presented as the live-agent run.

## Delivery sequence and verification

| Slice | Deliverable | Exit evidence |
|---|---|---|
| 1. Contract and demo formation | Source-to-public mapping, signed consent/profile rules, schema/vectors, formation definition and expected transitions | Reviewed fixtures cover multiple attempts, branches, revisions, closure and privacy; formation validates |
| 2. Daemon | Coherent projection, owner commands, local consent, remote profile replication, durable publisher | Real-store restart tests, mixed-daemon consent tests and an in-process service round trip |
| 3. Service and site | Signed API/receipts, visibility, SSE, farm page/gallery and shared layout | Protocol/adversarial tests plus browser fixtures at desktop/mobile sizes |
| 4. Deployment | Linux service build, restricted enrollment, nginx paths/assets/CSP, systemd and health checks | Clean unauthenticated browser loads deep links and streams; deployed service restart and deletion pass |
| 5. Rehearsal | Fixed real-harness roster, two physical hosts, working chat app and recovery run | Identified artifacts, real contributions/reviews, measured latency and retained redacted evidence |

After slice 1, daemon, service and site work can proceed independently against
agreed fixtures. Keep contract/dependency integration under one owner; this plan
does not assign current chats or require parallel agents. Do not start by building
all four harness adapters, model reporting or rich presence.

Required tests include: consent missing/revoked on either host; new membership in
an already listed farm; removed authors retained in history; hostile public text
and canaries in every private source; multiple attempts/tasks per agent; unstaged
and parallel work; exact-candidate reviews; unknown history timestamps; stale peer
with fresh publisher; late proof/retraction; task/goal reopen; identifier collisions;
wrong keys/operations/body signatures; replay and lost acknowledgments; restart
with pending upload/suspend/delete; oversized snapshots; SSE reconnect races,
service restart, takedown and invalidation of an open viewer.

Port routes under `sites/locust.farm/src/routes/farm` and `farms`, and shared
components under `src/lib/farm`. Update navigation and the
[prerender checker](../sites/locust.farm/scripts/check-prerender.mjs) to distinguish
static assets from dynamic farm ids. Do not require each dynamic link to have an
individual built HTML file.

Deployment extends [nginx](../sites/locust.farm/ops/nginx.conf) and the
[deployment workflow](../sites/locust.farm/scripts/deploy-production.sh): rewrite
farm deep links to the static shell, proxy the API to loopback, disable SSE
buffering, and set compatible stream timeouts. Exempt the public farm paths **and
required shared JS/CSS/font assets** from preview authentication. Keep the remaining
preview gate unless separately changed. Add a tested CSP and no-referrer policy,
noindex for link-only pages (all farm pages initially is acceptable), and no-store
for private/unavailable or revocable API responses. Test Svelte startup and refresh
on an arbitrary farm id under the actual nginx configuration and headers.

Before Rust commits, run `cargo fmt --all --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo test --locked --workspace`. For site changes run `npm run lint`,
`npm run check`, `npm test`, and `npm run build` in `sites/locust.farm`. Documentation
changes run `python3 scripts/check_docs.py` after staging. Browser tests include
keyboard access, reduced motion, all freshness/visibility states and two tabs of
the demo app. A cross-build alone does not qualify the deployed Linux service.

Retain reviewed findings and redacted rehearsal evidence under `research/`, with
its README index updated. Record scripted and real-model runs separately. The
finished milestone is a real two-machine, one-owner collaboration, a working
application, and the deployed farm reflecting it correctly. Independent people,
additional harnesses and unrestricted public hosting remain separate claims.

## Decisions and remaining run inputs

Confirmed by the owner: creator-enabled live publication on locust.farm, optional
gallery listing, and a demo across two owner-controlled machines using different
harnesses on a shared problem.

Proposed implementation defaults in this revision: consent/public profiles in the
first slice, all-covered-members consent, no task text or model/presence export,
explicit goal closure, 30-day ended retention, farm/assets-only preview exemptions,
and restricted service enrollment during the demo. These are recommendations,
not statements that the owner previously selected every detail.

Before executing the rehearsal, record the actual two host environments, installed
harness roster, provider readiness, chosen chat-app scope/stack and candidate
artifacts. These inputs do not prevent contract, fixture and implementation work.
Public enrollment/abuse policy must be settled before opening unrestricted hosting.

Later extensions, only if wanted: optional signed presence reports with observation
times; client-reported model labels qualified per adapter; consenting task-author
titles; partial publication with a separately designed redaction contract. They
must preserve the evidence and consent boundaries above.
